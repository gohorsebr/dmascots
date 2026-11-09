//! The OS layer: transparent, topmost, never-focused windows plus pointer and screens.
//!
//! | | Linux X11 (`x11.rs`) | Windows (`windows.rs`) |
//! |---|---|---|
//! | On top | override-redirect window, re-raised every 2s | `WS_EX_TOPMOST`, re-asserted every 2s |
//! | No focus | override-redirect windows are never focused | `WS_EX_NOACTIVATE` + `MA_NOACTIVATE` |
//! | Not in taskbar | not managed by the WM at all | `WS_EX_TOOLWINDOW` |
//! | Clicks pass around it | XShape input/bounding shape from the alpha, every frame | layered window: transparent pixels pass clicks |
//! | Click-through mode | empty input shape | `WS_EX_TRANSPARENT` |

use std::time::Duration;

#[cfg(unix)]
mod linux;
#[cfg(unix)]
pub mod x11;
#[cfg(unix)]
mod x11_menu;
#[cfg(windows)]
pub mod windows;

pub type WinId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    }
}

/// A monitor: its full geometry and the part not covered by panels/taskbars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub geometry: Rect,
    pub available: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorShape {
    OpenHand,
    ClosedHand,
    Arrow,
}

/// A menu entry (tray menu and the right-click mascot menu). Choosing an action
/// arrives as `Event::Menu(id)`.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem {
    Action { id: u32, label: String, checked: Option<bool> },
    Submenu { label: String, items: Vec<MenuItem> },
    Separator,
}

impl MenuItem {
    pub fn action(id: u32, label: impl Into<String>) -> MenuItem {
        MenuItem::Action { id, label: label.into(), checked: None }
    }
    pub fn check(id: u32, label: impl Into<String>, on: bool) -> MenuItem {
        MenuItem::Action { id, label: label.into(), checked: Some(on) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Mouse events carry global (screen) coordinates.
    Press { win: WinId, x: i32, y: i32, button: Button },
    Motion { win: WinId, x: i32, y: i32 },
    Release { win: WinId, x: i32, y: i32, button: Button },
    /// A menu action was chosen.
    Menu(u32),
    /// Text typed into a prompt opened with `ask_text(tag, ..)`.
    Said { tag: u32, text: String },
    /// The session is going away (X server gone, WM_QUIT).
    Quit,
}

/// A frame to show: premultiplied BGRA rows (what both X11 ARGB visuals and
/// Windows layered windows expect), plus the input shape as row-banded rectangles.
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub bgra: &'a [u8],
    /// Rectangles (window coordinates) that take mouse input; None = whole window.
    pub shape: Option<&'a [Rect]>,
}

pub trait Backend {
    fn create_window(&mut self, width: u32, height: u32, title: &str) -> WinId;
    fn destroy_window(&mut self, win: WinId);
    /// Move the window to (x, y) (if it moved) and show the frame.
    fn present(&mut self, win: WinId, x: i32, y: i32, frame: &Frame);
    fn set_click_through(&mut self, win: WinId, on: bool);
    fn set_cursor(&mut self, win: WinId, shape: CursorShape);
    fn cursor_pos(&mut self) -> (i32, i32);
    fn set_cursor_pos(&mut self, x: i32, y: i32);
    fn screens(&mut self) -> Vec<Screen>;
    /// Pixels per logical pixel (Windows high-DPI); mascots are scaled by this.
    fn device_scale(&self) -> f64 {
        1.0
    }
    /// Climb back above other topmost windows (called every couple of seconds).
    fn raise(&mut self, win: WinId);
    /// Wait up to `timeout` for input and return what arrived.
    fn poll(&mut self, timeout: Duration) -> Vec<Event>;
    /// Install the tray icon (RGBA straight-alpha pixels, square) with its menu; false if there is no tray.
    fn tray(&mut self, _icon_rgba: &[u8], _size: u32, _menu: Vec<MenuItem>) -> bool {
        false
    }
    /// Replace the tray menu (check marks changed).
    fn set_tray_menu(&mut self, _menu: Vec<MenuItem>) {}
    /// Show a popup menu at a screen position.
    fn popup_menu(&mut self, _x: i32, _y: i32, _menu: Vec<MenuItem>) {}
    /// Ask for a line of text; the answer arrives later as `Event::Said { tag, .. }`.
    fn ask_text(&mut self, _tag: u32, _title: &str, _label: &str) {}
}

/// A backend with no screen: for tests and tools. One 1920x1080 monitor, nothing is shown.
#[derive(Default)]
pub struct Headless {
    next: WinId,
    pub cursor: (i32, i32),
    pub click_through: std::collections::HashMap<WinId, bool>,
}

impl Backend for Headless {
    fn create_window(&mut self, _w: u32, _h: u32, _title: &str) -> WinId {
        self.next += 1;
        self.next
    }
    fn destroy_window(&mut self, win: WinId) {
        self.click_through.remove(&win);
    }
    fn present(&mut self, _win: WinId, _x: i32, _y: i32, _frame: &Frame) {}
    fn set_click_through(&mut self, win: WinId, on: bool) {
        self.click_through.insert(win, on);
    }
    fn set_cursor(&mut self, _win: WinId, _shape: CursorShape) {}
    fn cursor_pos(&mut self) -> (i32, i32) {
        self.cursor
    }
    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        self.cursor = (x, y);
    }
    fn screens(&mut self) -> Vec<Screen> {
        let r = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        vec![Screen { geometry: r, available: r }]
    }
    fn raise(&mut self, _win: WinId) {}
    fn poll(&mut self, timeout: Duration) -> Vec<Event> {
        std::thread::sleep(timeout);
        Vec::new()
    }
}

/// Open the backend for this OS.
pub fn open(managed: bool) -> Result<Box<dyn Backend>, String> {
    #[cfg(unix)]
    {
        x11::X11::open(managed).map(|b| Box::new(b) as Box<dyn Backend>)
    }
    #[cfg(windows)]
    {
        let _ = managed;
        windows::Win32::open().map(|b| Box::new(b) as Box<dyn Backend>)
    }
}

/// Turn premultiplied RGBA (tiny-skia) into premultiplied BGRA, in place in `out`.
pub fn rgba_to_bgra(rgba: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(rgba.len());
    for px in rgba.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
}

/// Input shape from the alpha channel: every non-transparent pixel, grown by `grow` px
/// (keeps antialiased edges and covers the shape lagging a frame behind), as rectangles
/// banded by rows, with identical neighbouring rows merged.
pub fn alpha_shape(rgba: &[u8], width: u32, height: u32, grow: i32) -> Vec<Rect> {
    let (w, h) = (width as usize, height as usize);
    let g = grow.max(0) as usize;
    // horizontal dilation
    let mut row_mask = vec![false; w * h];
    for y in 0..h {
        let mut last_on: Option<usize> = None;
        for x in 0..w {
            if rgba[(y * w + x) * 4 + 3] != 0 {
                let lo = x.saturating_sub(g).max(last_on.map_or(0, |l| l + 1));
                let hi = (x + g).min(w - 1);
                for i in lo..=hi {
                    row_mask[y * w + i] = true;
                }
                last_on = Some(hi);
            }
        }
    }
    // vertical dilation
    let mut mask = vec![false; w * h];
    for y in 0..h {
        let y0 = y.saturating_sub(g);
        let y1 = (y + g).min(h - 1);
        for x in 0..w {
            mask[y * w + x] = (y0..=y1).any(|yy| row_mask[yy * w + x]);
        }
    }
    // runs per row, merging rows with identical runs
    let runs = |y: usize| -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        let mut x = 0;
        while x < w {
            if mask[y * w + x] {
                let start = x;
                while x < w && mask[y * w + x] {
                    x += 1;
                }
                v.push((start as i32, (x - start) as i32));
            } else {
                x += 1;
            }
        }
        v
    };
    let mut rects = Vec::new();
    let mut y = 0;
    while y < h {
        let r = runs(y);
        let mut y_end = y + 1;
        while y_end < h && runs(y_end) == r {
            y_end += 1;
        }
        for &(x, rw) in &r {
            rects.push(Rect { x, y: y as i32, w: rw, h: (y_end - y) as i32 });
        }
        y = y_end;
    }
    rects
}
