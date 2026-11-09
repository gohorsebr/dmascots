//! Windows backend: per-pixel-alpha layered windows, a tray icon with a native menu,
//! and a small "Say something…" prompt. Plain Win32 through windows-sys.
//!
//! * `WS_EX_LAYERED` + `UpdateLayeredWindow`: the frame's alpha is the window shape, and
//!   fully transparent pixels already pass clicks through.
//! * `WS_EX_TOPMOST` (re-asserted every 2s), `WS_EX_NOACTIVATE` + `MA_NOACTIVATE` (never
//!   takes focus), `WS_EX_TOOLWINDOW` (not in the taskbar / Alt+Tab).
//! * Click-through mode toggles `WS_EX_TRANSPARENT`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC,
    GetMonitorInfoW, GetStockObject, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, COLOR_BTNFACE, DEFAULT_GUI_FONT, DIB_RGB_COLORS, HBITMAP,
    HDC, HMONITOR, MONITORINFO,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture, SetFocus};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::{Backend, Button, CursorShape, Event, Frame, MenuItem, Rect, Screen, WinId};

const TRAY_MSG: u32 = WM_APP + 1;

thread_local! {
    /// Events produced by the window procedures, drained by poll().
    static QUEUE: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    /// Cursor to show over each mascot window (WM_SETCURSOR).
    static CURSORS: RefCell<HashMap<isize, HCURSOR>> = RefCell::new(HashMap::new());
    /// The tray menu, read by the tray window procedure when the icon is clicked.
    static TRAY_MENU: RefCell<Vec<MenuItem>> = const { RefCell::new(Vec::new()) };
    /// Which mascot (0 = all) the open "Say something…" prompt is for.
    static PROMPT_TAG: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn push(e: Event) {
    QUEUE.with(|q| q.borrow_mut().push(e));
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn lo_word(l: LPARAM) -> u32 {
    (l as usize & 0xFFFF) as u32
}

fn screen_point(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    let mut p = POINT { x: (lparam & 0xFFFF) as i16 as i32, y: ((lparam >> 16) & 0xFFFF) as i16 as i32 };
    // SAFETY: valid window handle and point
    unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
    (p.x, p.y)
}

unsafe extern "system" fn mascot_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as WinId;
    match msg {
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            let (x, y) = screen_point(hwnd, lparam);
            if msg == WM_LBUTTONDOWN {
                SetCapture(hwnd); // keep getting moves while dragging off the window
            }
            let button = if msg == WM_LBUTTONDOWN { Button::Left } else { Button::Right };
            push(Event::Press { win: id, x, y, button });
            return 0;
        }
        WM_MOUSEMOVE => {
            let (x, y) = screen_point(hwnd, lparam);
            push(Event::Motion { win: id, x, y });
            return 0;
        }
        WM_LBUTTONUP | WM_RBUTTONUP => {
            let (x, y) = screen_point(hwnd, lparam);
            if msg == WM_LBUTTONUP && GetCapture() == hwnd {
                ReleaseCapture();
            }
            let button = if msg == WM_LBUTTONUP { Button::Left } else { Button::Right };
            push(Event::Release { win: id, x, y, button });
            return 0;
        }
        WM_SETCURSOR => {
            if let Some(c) = CURSORS.with(|c| c.borrow().get(&(hwnd as isize)).copied()) {
                SetCursor(c);
                return 1;
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn tray_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == TRAY_MSG {
        let event = lo_word(lparam);
        if event == WM_CONTEXTMENU || event == WM_RBUTTONUP || event == WM_LBUTTONUP {
            let menu = TRAY_MENU.with(|m| m.borrow().clone());
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            show_menu(hwnd, &menu, p.x, p.y);
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Build a native menu; item ids are indexes into `ids` (+1), so they stay small.
unsafe fn build_menu(items: &[MenuItem], ids: &mut Vec<u32>) -> HMENU {
    let menu = CreatePopupMenu();
    for item in items {
        match item {
            MenuItem::Action { id, label, checked } => {
                ids.push(*id);
                let flags = MF_STRING | if *checked == Some(true) { MF_CHECKED } else { MF_UNCHECKED };
                AppendMenuW(menu, flags, ids.len(), wide(label).as_ptr());
            }
            MenuItem::Submenu { label, items } => {
                let sub = build_menu(items, ids);
                AppendMenuW(menu, MF_POPUP, sub as usize, wide(label).as_ptr());
            }
            MenuItem::Separator => {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            }
        }
    }
    menu
}

/// Show a menu at (x, y) and queue the chosen action.
unsafe fn show_menu(owner: HWND, items: &[MenuItem], x: i32, y: i32) {
    let mut ids = Vec::new();
    let menu = build_menu(items, &mut ids);
    SetForegroundWindow(owner); // so the menu closes when you click elsewhere
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, x, y, 0, owner, std::ptr::null());
    DestroyMenu(menu); // also destroys the submenus
    if cmd > 0 {
        if let Some(&id) = ids.get(cmd as usize - 1) {
            push(Event::Menu(id));
        }
    }
}

// ------------------------------------------------------------------ "Say something…" prompt

const PROMPT_EDIT: i32 = 10;

unsafe extern "system" fn prompt_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            if id == IDOK || id == IDCANCEL {
                if id == IDOK {
                    let edit = GetDlgItem(hwnd, PROMPT_EDIT);
                    let mut buf = vec![0u16; 2048];
                    let n = GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32);
                    let text = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
                    if !text.trim().is_empty() {
                        push(Event::Said { tag: PROMPT_TAG.with(|t| t.get()), text });
                    }
                }
                DestroyWindow(hwnd);
                return 0;
            }
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ------------------------------------------------------------------ the backend

struct Win {
    hwnd: HWND,
    w: u32,
    h: u32,
    dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u8,
    shown: bool,
}

pub struct Win32 {
    hinstance: *mut c_void,
    mascot_class: Vec<u16>,
    prompt_class: Vec<u16>,
    wins: HashMap<WinId, Win>,
    next_id: WinId,
    cursors: [HCURSOR; 3],
    tray_hwnd: HWND,
    prompt: HWND,
    scale: f64,
}

unsafe extern "system" fn monitor_cb(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
    let out = &mut *(data as *mut Vec<Screen>);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(m, &mut info) != 0 {
        let r = |r: RECT| Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top };
        let s = Screen { geometry: r(info.rcMonitor), available: r(info.rcWork) };
        if info.dwFlags & 1 != 0 {
            out.insert(0, s); // MONITORINFOF_PRIMARY first
        } else {
            out.push(s);
        }
    }
    1
}

impl Win32 {
    pub fn open() -> Result<Win32, String> {
        // SAFETY: plain Win32 setup calls with valid arguments
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let hinstance = GetModuleHandleW(std::ptr::null());
            let register = |name: &[u16], proc_: WNDPROC| {
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: proc_,
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: std::ptr::null_mut(),
                    hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                    hbrBackground: (COLOR_BTNFACE + 1) as usize as _,
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: name.as_ptr(),
                    hIconSm: std::ptr::null_mut(),
                };
                RegisterClassExW(&wc)
            };
            let mascot_class = wide("DesktopMascot");
            let tray_class = wide("DesktopMascotsTray");
            let prompt_class = wide("DesktopMascotsPrompt");
            if register(&mascot_class, Some(mascot_proc)) == 0
                || register(&tray_class, Some(tray_proc)) == 0
                || register(&prompt_class, Some(prompt_proc)) == 0
            {
                return Err("couldn't register the window classes".into());
            }
            // a hidden (never shown) top-level window: owns the tray icon and the popup
            // menus, which need an owner that can become the foreground window
            let tray_hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                tray_class.as_ptr(),
                wide("Desktop Mascots").as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );
            let cursors = [
                LoadCursorW(std::ptr::null_mut(), IDC_HAND),
                LoadCursorW(std::ptr::null_mut(), IDC_SIZEALL),
                LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            ];
            Ok(Win32 {
                hinstance,
                mascot_class,
                prompt_class,
                wins: HashMap::new(),
                next_id: 1,
                cursors,
                tray_hwnd,
                prompt: std::ptr::null_mut(),
                scale: GetDpiForSystem() as f64 / 96.0,
            })
        }
    }

    fn tray_data(&self) -> NOTIFYICONDATAW {
        // SAFETY: NOTIFYICONDATAW is plain data; zeroed is a valid starting state
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.tray_hwnd;
        nid.uID = 1;
        nid
    }
}

impl Drop for Win32 {
    fn drop(&mut self) {
        let mut nid = self.tray_data();
        // SAFETY: removing our own tray icon (a no-op if it was never added)
        unsafe { Shell_NotifyIconW(NIM_DELETE, &mut nid) };
    }
}

impl Backend for Win32 {
    fn create_window(&mut self, width: u32, height: u32, title: &str) -> WinId {
        let id = self.next_id;
        self.next_id += 1;
        // SAFETY: creating our own window and its backing DIB section
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                self.mascot_class.as_ptr(),
                wide(title).as_ptr(),
                WS_POPUP,
                -10000,
                -10000,
                width as i32,
                height as i32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                self.hinstance,
                std::ptr::null(),
            );
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, id as isize);
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(std::ptr::null_mut(), screen);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            SelectObject(dc, bitmap);
            self.wins.insert(
                id,
                Win { hwnd, w: width, h: height, dc, bitmap, bits: bits as *mut u8, shown: false },
            );
        }
        id
    }

    fn destroy_window(&mut self, win: WinId) {
        if let Some(w) = self.wins.remove(&win) {
            CURSORS.with(|c| c.borrow_mut().remove(&(w.hwnd as isize)));
            // SAFETY: destroying resources we created
            unsafe {
                DestroyWindow(w.hwnd);
                DeleteDC(w.dc);
                DeleteObject(w.bitmap);
            }
        }
    }

    fn present(&mut self, win: WinId, x: i32, y: i32, frame: &Frame) {
        let Some(w) = self.wins.get_mut(&win) else { return };
        let n = (w.w * w.h * 4) as usize;
        if frame.bgra.len() < n || w.bits.is_null() {
            return;
        }
        // SAFETY: bits points at the DIB's w*h*4 bytes; the DC has the bitmap selected
        unsafe {
            std::ptr::copy_nonoverlapping(frame.bgra.as_ptr(), w.bits, n);
            let pos = POINT { x, y };
            let size = SIZE { cx: w.w as i32, cy: w.h as i32 };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(w.hwnd, std::ptr::null_mut(), &pos, &size, w.dc, &src, 0, &blend, ULW_ALPHA);
            if !w.shown {
                ShowWindow(w.hwnd, SW_SHOWNOACTIVATE);
                w.shown = true;
            }
        }
    }

    fn set_click_through(&mut self, win: WinId, on: bool) {
        if let Some(w) = self.wins.get(&win) {
            // SAFETY: changing our own window's extended style
            unsafe {
                let ex = GetWindowLongPtrW(w.hwnd, GWL_EXSTYLE) as u32;
                let ex = if on { ex | WS_EX_TRANSPARENT } else { ex & !WS_EX_TRANSPARENT };
                SetWindowLongPtrW(w.hwnd, GWL_EXSTYLE, ex as isize);
            }
        }
    }

    fn set_cursor(&mut self, win: WinId, shape: CursorShape) {
        let Some(w) = self.wins.get(&win) else { return };
        let c = self.cursors[shape as usize];
        CURSORS.with(|m| m.borrow_mut().insert(w.hwnd as isize, c));
        // SAFETY: plain cursor calls
        unsafe {
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            if GetCapture() == w.hwnd || WindowFromPoint(p) == w.hwnd {
                SetCursor(c);
            }
        }
    }

    fn cursor_pos(&mut self) -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: valid out pointer
        unsafe { GetCursorPos(&mut p) };
        (p.x, p.y)
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        // SAFETY: plain call
        unsafe { SetCursorPos(x, y) };
    }

    fn screens(&mut self) -> Vec<Screen> {
        let mut out: Vec<Screen> = Vec::new();
        // SAFETY: the callback only writes into `out`, which outlives the call
        unsafe {
            EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(monitor_cb), &mut out as *mut _ as LPARAM);
        }
        out
    }

    fn device_scale(&self) -> f64 {
        self.scale
    }

    fn raise(&mut self, win: WinId) {
        // other topmost windows can end up above us; quietly climb back
        if let Some(w) = self.wins.get(&win) {
            // SAFETY: plain call on our own window
            unsafe { SetWindowPos(w.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE) };
        }
    }

    fn poll(&mut self, timeout: Duration) -> Vec<Event> {
        // SAFETY: standard message pump
        unsafe {
            let ms = timeout.as_millis().min(INFINITE as u128 - 1) as u32;
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, ms, QS_ALLINPUT);
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    push(Event::Quit);
                    break;
                }
                if !self.prompt.is_null() && IsWindow(self.prompt) == 0 {
                    self.prompt = std::ptr::null_mut();
                }
                // Enter / Esc / Tab in the prompt
                if !self.prompt.is_null() && IsDialogMessageW(self.prompt, &msg) != 0 {
                    continue;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()))
    }

    fn tray(&mut self, icon_rgba: &[u8], size: u32, menu: Vec<MenuItem>) -> bool {
        TRAY_MENU.with(|m| *m.borrow_mut() = menu);
        // SAFETY: building an icon from our own pixels and registering it with the shell
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let color = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            ReleaseDC(std::ptr::null_mut(), screen);
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
            for (d, s) in dst.chunks_exact_mut(4).zip(icon_rgba.chunks_exact(4)) {
                d.copy_from_slice(&[s[2], s[1], s[0], s[3]]); // straight-alpha BGRA
            }
            let mask = windows_sys::Win32::Graphics::Gdi::CreateBitmap(size as i32, size as i32, 1, 1, std::ptr::null());
            let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
            let icon = CreateIconIndirect(&info);
            DeleteObject(color);
            DeleteObject(mask);

            let mut nid = self.tray_data();
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = TRAY_MSG;
            nid.hIcon = icon;
            for (d, s) in nid.szTip.iter_mut().zip(wide("Desktop Mascots")) {
                *d = s;
            }
            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                return false;
            }
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &mut nid);
        }
        true
    }

    fn set_tray_menu(&mut self, menu: Vec<MenuItem>) {
        TRAY_MENU.with(|m| *m.borrow_mut() = menu);
    }

    fn popup_menu(&mut self, x: i32, y: i32, menu: Vec<MenuItem>) {
        // SAFETY: a modal native menu owned by our hidden message window
        unsafe { show_menu(self.tray_hwnd, &menu, x, y) };
    }

    fn ask_text(&mut self, tag: u32, title: &str, label: &str) {
        PROMPT_TAG.with(|t| t.set(tag));
        // SAFETY: creating a small top-level window with standard controls
        unsafe {
            if !self.prompt.is_null() && IsWindow(self.prompt) != 0 {
                SetForegroundWindow(self.prompt);
                return;
            }
            let s = |v: f64| (v * self.scale).round() as i32;
            let (w, h) = (s(340.0), s(130.0));
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            let dlg = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_TOPMOST | WS_EX_CONTROLPARENT,
                self.prompt_class.as_ptr(),
                wide(title).as_ptr(),
                WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                p.x - w / 2,
                p.y - h,
                w,
                h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                self.hinstance,
                std::ptr::null(),
            );
            let font = GetStockObject(DEFAULT_GUI_FONT);
            let child = |class: &str, text: &str, style: u32, x, y, cw, ch, id: i32| {
                let c = CreateWindowExW(
                    if class == "EDIT" { WS_EX_CLIENTEDGE } else { 0 },
                    wide(class).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    s(x),
                    s(y),
                    s(cw),
                    s(ch),
                    dlg,
                    id as usize as _,
                    self.hinstance,
                    std::ptr::null(),
                );
                SendMessageW(c, WM_SETFONT, font as usize, 1);
                c
            };
            child("STATIC", label, 0, 12.0, 12.0, 300.0, 18.0, -1);
            let edit = child("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL as u32, 12.0, 32.0, 300.0, 24.0, PROMPT_EDIT);
            child("BUTTON", "OK", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 152.0, 64.0, 76.0, 26.0, IDOK);
            child("BUTTON", "Cancel", WS_TABSTOP, 236.0, 64.0, 76.0, 26.0, IDCANCEL);
            SetForegroundWindow(dlg);
            SetFocus(edit);
            self.prompt = dlg;
        }
    }
}
