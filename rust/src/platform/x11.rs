//! Linux/X11 backend, pure Rust (x11rb talks the X protocol directly: no libX11/libxcb).
//!
//! Each mascot is a 32-bit ARGB override-redirect window: the window manager never
//! focuses, decorates or lists it, and a compositor blends its alpha. The XShape
//! input shape follows the mascot every frame, so clicks land on the windows underneath
//! everywhere else. The bounding shape is only set when no compositor is running:
//! changing it costs the server and the compositor ~10 ms per window per frame, and
//! with a compositor the alpha channel already hides the transparent pixels.

use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClipOrdering, ColormapAlloc, ConfigureWindowAux,
    ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, PropMode, Rectangle,
    StackMode, VisualClass, Window, WindowClass,
};
use x11rb::protocol::Event as XEvent;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::linux::{self, TrayIcon};
use super::x11_menu::{Outcome, Popup};
use super::{Backend, Button, CursorShape, Event, Frame, MenuItem, Rect, Screen, WinId};

struct Win {
    xid: Window,
    gc: u32,
    w: u32,
    h: u32,
    pos: Option<(i32, i32)>,
    mapped: bool,
    click_through: bool,
    cursor: Option<CursorShape>,
    /// the bounding shape last sent (None: the default, the whole window)
    bounding: Option<Vec<Rect>>,
    /// the input shape last sent (None: not sent yet, or must be re-sent)
    input: Option<Vec<Rect>>,
}

pub struct X11 {
    conn: RustConnection,
    screen: usize,
    root: Window,
    visual: u32,
    colormap: u32,
    managed: bool,
    has_shape: bool,
    wins: HashMap<WinId, Win>,
    by_xid: HashMap<Window, WinId>,
    next_id: WinId,
    cursors: HashMap<CursorShape, u32>,
    screens: Vec<Screen>,
    screens_at: Option<Instant>,
    /// _NET_WM_CM_S<screen>: owned by the running compositing manager, if any
    cm_atom: u32,
    composited: bool,
    max_request: usize,
    atoms: Atoms,
    popup: Option<Popup>,
    tray: Option<TrayIcon>,
    /// events from other threads (tray menu, text prompt)
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        _NET_WORKAREA,
        _NET_CURRENT_DESKTOP,
        _NET_WM_NAME,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _NET_WM_STATE,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        UTF8_STRING,
    }
}

fn err(e: impl std::fmt::Display) -> String {
    format!("X11: {e}")
}

impl X11 {
    pub fn open(managed: bool) -> Result<X11, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| {
            format!("can't open the X display ({e}). The mascots need an X11 session (or XWayland).")
        })?;
        let setup = conn.setup();
        let root_screen = &setup.roots[screen];
        let root = root_screen.root;
        let visual = root_screen
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| d.visuals.iter())
            .find(|v| v.class == VisualClass::TRUE_COLOR)
            .map(|v| v.visual_id)
            .ok_or("X11: no 32-bit (ARGB) visual; transparent windows aren't possible here")?;
        let colormap = conn.generate_id().map_err(err)?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual).map_err(err)?;
        let has_shape = conn.extension_information(shape::X11_EXTENSION_NAME).map_err(err)?.is_some();
        let atoms = Atoms::new(&conn).map_err(err)?.reply().map_err(err)?;
        let cm_atom =
            conn.intern_atom(false, format!("_NET_WM_CM_S{screen}").as_bytes()).map_err(err)?.reply().map_err(err)?.atom;
        let max_request = conn.maximum_request_bytes();
        let (tx, rx) = mpsc::channel();
        Ok(X11 {
            conn,
            screen,
            root,
            visual,
            colormap,
            managed,
            has_shape,
            wins: HashMap::new(),
            by_xid: HashMap::new(),
            next_id: 1,
            cursors: HashMap::new(),
            screens: Vec::new(),
            screens_at: None,
            cm_atom,
            composited: false,
            max_request,
            atoms,
            popup: None,
            tray: None,
            tx,
            rx,
        })
    }

    fn cursor(&mut self, shape: CursorShape) -> u32 {
        if let Some(&c) = self.cursors.get(&shape) {
            return c;
        }
        let names: &[&str] = match shape {
            CursorShape::OpenHand => &["openhand", "grab", "hand2"],
            CursorShape::ClosedHand => &["closedhand", "grabbing", "fleur"],
            CursorShape::Arrow => &["left_ptr", "default"],
        };
        let mut cursor = 0;
        if let Ok(db) = x11rb::resource_manager::new_from_default(&self.conn) {
            if let Ok(handle) = x11rb::cursor::Handle::new(&self.conn, self.screen, &db).and_then(|c| {
                c.reply().map_err(|_| x11rb::errors::ConnectionError::UnknownError)
            }) {
                for name in names {
                    if let Ok(c) = handle.load_cursor(&self.conn, name) {
                        if c != 0 {
                            cursor = c;
                            break;
                        }
                    }
                }
            }
        }
        self.cursors.insert(shape, cursor);
        cursor
    }

    fn query_composited(&self) -> bool {
        self.conn
            .get_selection_owner(self.cm_atom)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.owner != x11rb::NONE)
    }

    fn query_screens(&self) -> Vec<Screen> {
        let root = &self.conn.setup().roots[self.screen];
        let whole = Rect { x: 0, y: 0, w: root.width_in_pixels as i32, h: root.height_in_pixels as i32 };
        let mut geoms: Vec<Rect> = self
            .conn
            .randr_get_monitors(self.root, true)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| {
                // primary first, like QGuiApplication::primaryScreen
                let mut m = r.monitors;
                m.sort_by_key(|m| !m.primary);
                m.iter().map(|m| Rect { x: m.x as i32, y: m.y as i32, w: m.width as i32, h: m.height as i32 }).collect()
            })
            .unwrap_or_default();
        if geoms.is_empty() {
            geoms.push(whole);
        }
        // _NET_WORKAREA for the current desktop: the area not covered by panels
        let desktop = self
            .conn
            .get_property(false, self.root, self.atoms._NET_CURRENT_DESKTOP, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            .unwrap_or(0) as usize;
        let work = self
            .conn
            .get_property(false, self.root, self.atoms._NET_WORKAREA, AtomEnum::CARDINAL, 0, 4 * 64)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect::<Vec<u32>>()))
            .and_then(|v| {
                let i = if v.len() >= (desktop + 1) * 4 { desktop * 4 } else { 0 };
                (v.len() >= i + 4).then(|| Rect { x: v[i] as i32, y: v[i + 1] as i32, w: v[i + 2] as i32, h: v[i + 3] as i32 })
            });
        geoms
            .into_iter()
            .map(|g| Screen { geometry: g, available: work.and_then(|w| w.intersect(&g)).unwrap_or(g) })
            .collect()
    }

    fn put_image(&self, win: &Win, frame: &Frame) -> Result<(), x11rb::errors::ConnectionError> {
        // stay under the maximum request size: send the image in horizontal strips
        let stride = frame.width as usize * 4;
        let rows_per = ((self.max_request.saturating_sub(64)) / stride).max(1);
        let mut y = 0usize;
        while y < frame.height as usize {
            let rows = rows_per.min(frame.height as usize - y);
            self.conn.put_image(
                ImageFormat::Z_PIXMAP,
                win.xid,
                win.gc,
                frame.width as u16,
                rows as u16,
                0,
                y as i16,
                0,
                32,
                &frame.bgra[y * stride..(y + rows) * stride],
            )?;
            y += rows;
        }
        Ok(())
    }

    /// Send the window's shapes, skipping whatever hasn't changed since the last frame.
    fn set_shape(conn: &RustConnection, composited: bool, win: &mut Win, rects: Option<&[Rect]>) {
        let wanted: Vec<Rect> = match rects {
            Some([]) => vec![Rect { x: 0, y: 0, w: 1, h: 1 }],
            Some(r) => r.to_vec(),
            None => vec![Rect { x: 0, y: 0, w: win.w as i32, h: win.h as i32 }],
        };
        let to_x = |r: &[Rect]| -> Vec<Rectangle> {
            r.iter().map(|r| Rectangle { x: r.x as i16, y: r.y as i16, width: r.w as u16, height: r.h as u16 }).collect()
        };
        let bounding = if composited { None } else { Some(wanted.clone()) };
        if win.bounding != bounding {
            let _ = match &bounding {
                Some(r) => shape::rectangles(conn, SO::SET, SK::BOUNDING, ClipOrdering::YX_BANDED, win.xid, 0, 0, &to_x(r)),
                None => shape::mask(conn, SO::SET, SK::BOUNDING, win.xid, 0, 0, x11rb::NONE), // back to the whole window
            };
            win.bounding = bounding;
        }
        let input = if win.click_through { Vec::new() } else { wanted };
        if win.input.as_ref() != Some(&input) {
            let _ = shape::rectangles(conn, SO::SET, SK::INPUT, ClipOrdering::YX_BANDED, win.xid, 0, 0, &to_x(&input));
            win.input = Some(input);
        }
    }

    fn button(detail: u8) -> Button {
        match detail {
            1 => Button::Left,
            3 => Button::Right,
            _ => Button::Other,
        }
    }

    fn translate(&self, ev: XEvent) -> Option<Event> {
        match ev {
            XEvent::ButtonPress(e) => Some(Event::Press {
                win: *self.by_xid.get(&e.event)?,
                x: e.root_x as i32,
                y: e.root_y as i32,
                button: Self::button(e.detail),
            }),
            XEvent::ButtonRelease(e) => Some(Event::Release {
                win: *self.by_xid.get(&e.event)?,
                x: e.root_x as i32,
                y: e.root_y as i32,
                button: Self::button(e.detail),
            }),
            XEvent::MotionNotify(e) => Some(Event::Motion {
                win: *self.by_xid.get(&e.event)?,
                x: e.root_x as i32,
                y: e.root_y as i32,
            }),
            _ => None,
        }
    }
}

impl Backend for X11 {
    fn create_window(&mut self, width: u32, height: u32, title: &str) -> WinId {
        let xid = self.conn.generate_id().expect("X11 id");
        let aux = CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(self.colormap)
            .override_redirect(u32::from(!self.managed))
            .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION);
        let _ = self.conn.create_window(
            32,
            xid,
            self.root,
            -10000,
            -10000,
            width as u16,
            height as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            self.visual,
            &aux,
        );
        let _ = self.conn.change_property8(PropMode::REPLACE, xid, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes());
        let _ = self.conn.change_property8(
            PropMode::REPLACE,
            xid,
            self.atoms._NET_WM_NAME,
            self.atoms.UTF8_STRING,
            title.as_bytes(),
        );
        if self.managed {
            // --managed: ask the WM nicely for what override-redirect gets us for free
            let a = &self.atoms;
            let _ = self.conn.change_property32(
                PropMode::REPLACE,
                xid,
                a._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                &[a._NET_WM_WINDOW_TYPE_UTILITY],
            );
            let _ = self.conn.change_property32(
                PropMode::REPLACE,
                xid,
                a._NET_WM_STATE,
                AtomEnum::ATOM,
                &[a._NET_WM_STATE_ABOVE, a._NET_WM_STATE_SKIP_TASKBAR, a._NET_WM_STATE_SKIP_PAGER],
            );
        }
        let gc = self.conn.generate_id().expect("X11 id");
        let _ = self.conn.create_gc(gc, xid, &CreateGCAux::new());
        let id = self.next_id;
        self.next_id += 1;
        self.wins.insert(
            id,
            Win {
                xid,
                gc,
                w: width,
                h: height,
                pos: None,
                mapped: false,
                click_through: false,
                cursor: None,
                bounding: None,
                input: None,
            },
        );
        self.by_xid.insert(xid, id);
        id
    }

    fn destroy_window(&mut self, win: WinId) {
        if let Some(w) = self.wins.remove(&win) {
            self.by_xid.remove(&w.xid);
            let _ = self.conn.free_gc(w.gc);
            let _ = self.conn.destroy_window(w.xid);
            let _ = self.conn.flush();
        }
    }

    fn present(&mut self, win: WinId, x: i32, y: i32, frame: &Frame) {
        if self.screens_at.is_none() {
            self.screens(); // also finds out whether a compositor is running
        }
        let Some(w) = self.wins.get_mut(&win) else { return };
        if w.pos != Some((x, y)) {
            let _ = self.conn.configure_window(w.xid, &ConfigureWindowAux::new().x(x).y(y));
        }
        if self.has_shape {
            Self::set_shape(&self.conn, self.composited, w, frame.shape);
        }
        let w = &self.wins[&win];
        let _ = self.put_image(w, frame);
        if !w.mapped {
            let _ = self.conn.map_window(w.xid);
        }
        let _ = self.conn.flush();
        let w = self.wins.get_mut(&win).unwrap();
        w.pos = Some((x, y));
        w.mapped = true;
    }

    fn set_click_through(&mut self, win: WinId, on: bool) {
        if let Some(w) = self.wins.get_mut(&win) {
            w.click_through = on;
            w.input = None;
        }
        // the input shape is re-sent on the next present()
    }

    fn set_cursor(&mut self, win: WinId, shape: CursorShape) {
        if self.wins.get(&win).map(|w| w.cursor) == Some(Some(shape)) {
            return;
        }
        let c = self.cursor(shape);
        if let Some(w) = self.wins.get_mut(&win) {
            w.cursor = Some(shape);
            let _ = self.conn.change_window_attributes(w.xid, &ChangeWindowAttributesAux::new().cursor(c));
            let _ = self.conn.flush();
        }
    }

    fn cursor_pos(&mut self) -> (i32, i32) {
        self.conn
            .query_pointer(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| (r.root_x as i32, r.root_y as i32))
            .unwrap_or((0, 0))
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        let _ = self.conn.warp_pointer(x11rb::NONE, self.root, 0, 0, 0, 0, x as i16, y as i16);
        let _ = self.conn.flush();
    }

    fn screens(&mut self) -> Vec<Screen> {
        // panels, monitors and the compositor rarely change: re-read them every couple of seconds
        if self.screens_at.map_or(true, |t| t.elapsed() > Duration::from_secs(2)) {
            self.screens = self.query_screens();
            self.composited = self.query_composited();
            self.screens_at = Some(Instant::now());
        }
        self.screens.clone()
    }

    fn raise(&mut self, win: WinId) {
        if let Some(w) = self.wins.get(&win) {
            let _ = self.conn.configure_window(w.xid, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        }
    }

    fn poll(&mut self, timeout: Duration) -> Vec<Event> {
        let mut out = Vec::new();
        let deadline = Instant::now() + timeout;
        loop {
            loop {
                match self.conn.poll_for_event() {
                    Ok(Some(ev)) => {
                        if let Some(popup) = self.popup.as_mut() {
                            // the open menu holds the grab: it gets the pointer and keys
                            match popup.on_event(&self.conn, &ev) {
                                Outcome::Open => {}
                                Outcome::Close => self.popup.take().unwrap().close(&self.conn),
                                Outcome::Chosen(id) => {
                                    self.popup.take().unwrap().close(&self.conn);
                                    out.push(Event::Menu(id));
                                }
                            }
                            continue;
                        }
                        out.extend(self.translate(ev));
                    }
                    Ok(None) => break,
                    Err(_) => {
                        // the X server went away (logout): nothing left to show
                        out.push(Event::Quit);
                        return out;
                    }
                }
            }
            out.extend(self.rx.try_iter());
            let left = deadline.saturating_duration_since(Instant::now());
            if !out.is_empty() || left.is_zero() {
                return out;
            }
            let mut fds = libc::pollfd { fd: self.conn.stream().as_raw_fd(), events: libc::POLLIN, revents: 0 };
            // short naps so tray clicks and prompt answers (other threads) are picked up promptly
            let ms = left.as_millis().clamp(1, 16) as i32;
            // SAFETY: one valid pollfd for the duration of the call
            unsafe { libc::poll(&mut fds, 1, ms) };
        }
    }

    fn tray(&mut self, icon_rgba: &[u8], size: u32, menu: Vec<MenuItem>) -> bool {
        self.tray = TrayIcon::new(icon_rgba, size, menu, self.tx.clone());
        self.tray.is_some()
    }

    fn set_tray_menu(&mut self, menu: Vec<MenuItem>) {
        if let Some(t) = &self.tray {
            t.set_menu(menu);
        }
    }

    fn popup_menu(&mut self, x: i32, y: i32, menu: Vec<MenuItem>) {
        if let Some(old) = self.popup.take() {
            old.close(&self.conn);
        }
        let screens = self.screens();
        self.popup = Popup::open(&self.conn, self.root, self.visual, self.colormap, x, y, &menu, &screens);
    }

    fn ask_text(&mut self, tag: u32, title: &str, label: &str) {
        linux::ask_text(tag, title, label, self.tx.clone());
    }
}
