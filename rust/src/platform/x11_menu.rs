//! A small popup menu for X11, drawn with our own painter (no toolkit to borrow one from).
//!
//! It grabs the pointer and keyboard while open: clicking an item chooses it, clicking
//! anywhere else or pressing a key closes it. Submenus are shown inline, as a labelled,
//! indented section, which keeps the "Add" list one click away.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::{
    ClipOrdering, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, GrabMode, ImageFormat, Rectangle,
    Window, WindowClass,
};
use x11rb::protocol::Event as XEvent;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{alpha_shape, rgba_to_bgra, MenuItem, Rect, Screen};
use crate::gfx::{pt, Color, Font, Painter, Path, Pen};

const PX: f64 = 13.0;
const ROW_H: i32 = 26;
const SEP_H: i32 = 9;
const HEADER_H: i32 = 22;
const PAD: i32 = 5; // above the first and below the last row
const SIDE: f64 = 14.0;
const CHECK_W: f64 = 20.0;
const INDENT: f64 = 12.0;
/// A release this soon after opening is the end of the click that opened the menu.
const OPEN_GRACE: Duration = Duration::from_millis(300);

enum Row {
    Item { id: u32, label: String, checked: Option<bool>, indent: bool },
    Header(String),
    Sep,
}

impl Row {
    fn height(&self) -> i32 {
        match self {
            Row::Item { .. } => ROW_H,
            Row::Header(_) => HEADER_H,
            Row::Sep => SEP_H,
        }
    }
}

pub enum Outcome {
    Open,
    Close,
    Chosen(u32),
}

pub struct Popup {
    pub xid: Window,
    gc: u32,
    rect: Rect,
    rows: Vec<Row>,
    hover: Option<usize>,
    opened: Instant,
    pressed_inside: bool,
}

fn flatten(items: &[MenuItem], indent: bool, rows: &mut Vec<Row>) {
    for item in items {
        match item {
            MenuItem::Action { id, label, checked } => {
                rows.push(Row::Item { id: *id, label: label.clone(), checked: *checked, indent })
            }
            MenuItem::Submenu { label, items } => {
                rows.push(Row::Header(label.clone()));
                flatten(items, true, rows);
            }
            MenuItem::Separator => rows.push(Row::Sep),
        }
    }
}

impl Popup {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        conn: &RustConnection,
        root: Window,
        visual: u32,
        colormap: u32,
        x: i32,
        y: i32,
        items: &[MenuItem],
        screens: &[Screen],
    ) -> Option<Popup> {
        let mut rows = Vec::new();
        flatten(items, false, &mut rows);
        if rows.is_empty() {
            return None;
        }
        let font = Font::new(PX);
        let text_w = rows
            .iter()
            .map(|r| match r {
                Row::Item { label, indent, .. } => font.width(label) + if *indent { INDENT } else { 0.0 },
                Row::Header(label) => Font::new(PX - 1.0).width(label),
                Row::Sep => 0.0,
            })
            .fold(0.0, f64::max);
        let w = (SIDE * 2.0 + CHECK_W + text_w).ceil() as i32 + 8;
        let h = rows.iter().map(Row::height).sum::<i32>() + 2 * PAD;
        // keep it on the screen it was opened on, flipping left/up near the edges
        let area = screens
            .iter()
            .find(|s| s.geometry.contains(x, y))
            .map(|s| s.geometry)
            .unwrap_or(Rect { x: 0, y: 0, w: 1 << 15, h: 1 << 15 });
        let px = if x + w > area.x + area.w { (x - w).max(area.x) } else { x };
        let py = if y + h > area.y + area.h { (y - h).max(area.y) } else { y };

        let xid = conn.generate_id().ok()?;
        let aux = CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(colormap)
            .override_redirect(1)
            .event_mask(
                EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::KEY_PRESS
                    | EventMask::EXPOSURE,
            );
        conn.create_window(32, xid, root, px as i16, py as i16, w as u16, h as u16, 0, WindowClass::INPUT_OUTPUT, visual, &aux)
            .ok()?;
        let _ = conn.change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            xid,
            x11rb::protocol::xproto::AtomEnum::WM_NAME,
            x11rb::protocol::xproto::AtomEnum::STRING,
            b"Desktop Mascots menu",
        );
        let gc = conn.generate_id().ok()?;
        conn.create_gc(gc, xid, &CreateGCAux::new()).ok()?;
        let popup = Popup {
            xid,
            gc,
            rect: Rect { x: px, y: py, w, h },
            rows,
            hover: None,
            opened: Instant::now(),
            pressed_inside: false,
        };
        // map first: drawing into an unmapped window (no backing store) is thrown away
        conn.map_window(xid).ok()?;
        popup.render(conn);
        let mask = EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION;
        let _ = conn.grab_pointer(false, xid, mask, GrabMode::ASYNC, GrabMode::ASYNC, x11rb::NONE, x11rb::NONE, x11rb::CURRENT_TIME);
        let _ = conn.grab_keyboard(false, xid, x11rb::CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC);
        let _ = conn.flush();
        Some(popup)
    }

    /// The selectable row under a root-window point.
    fn item_at(&self, rx: i32, ry: i32) -> Option<usize> {
        if !self.rect.contains(rx, ry) {
            return None;
        }
        let mut top = self.rect.y + PAD;
        for (i, r) in self.rows.iter().enumerate() {
            let h = r.height();
            if ry >= top && ry < top + h {
                return matches!(r, Row::Item { .. }).then_some(i);
            }
            top += h;
        }
        None
    }

    fn render(&self, conn: &RustConnection) {
        let (w, h) = (self.rect.w as u32, self.rect.h as u32);
        let Some(mut pm) = tiny_skia::Pixmap::new(w, h) else { return };
        {
            let mut p = Painter::new(&mut pm);
            let mut bg = Path::new();
            bg.add_rounded_rect(0.5, 0.5, w as f64 - 1.0, h as f64 - 1.0, 7.0, 7.0);
            p.set_brush(Color::hex("#FBFBFA"));
            p.set_pen(Pen::new(Color::hex("#B8B8B4"), 1.0));
            p.draw_path(&bg);
            let font = Font::new(PX);
            let mut top = PAD as f64;
            for (i, r) in self.rows.iter().enumerate() {
                let rh = r.height() as f64;
                match r {
                    Row::Item { label, checked, indent, .. } => {
                        if self.hover == Some(i) {
                            let mut hl = Path::new();
                            hl.add_rounded_rect(4.0, top + 1.0, w as f64 - 8.0, rh - 2.0, 5.0, 5.0);
                            p.no_pen();
                            p.set_brush(Color::hex("#D9E6F7"));
                            p.draw_path(&hl);
                        }
                        let base = top + (rh + font.ascent() - font.descent()) / 2.0;
                        p.set_font_px(PX);
                        p.set_pen(Color::hex("#1F1F1F"));
                        if *checked == Some(true) {
                            p.draw_text(SIDE, base, "✓");
                        }
                        let x = SIDE + CHECK_W + if *indent { INDENT } else { 0.0 };
                        p.draw_text(x, base, label);
                    }
                    Row::Header(label) => {
                        let f = Font::new(PX - 1.0);
                        let base = top + (rh + f.ascent() - f.descent()) / 2.0;
                        p.set_font_px(PX - 1.0);
                        p.set_pen(Color::hex("#7A7A76"));
                        p.draw_text(SIDE + CHECK_W, base, label);
                    }
                    Row::Sep => {
                        p.set_pen(Pen::new(Color::hex("#DDDDD8"), 1.0));
                        let y = (top + rh / 2.0).floor() + 0.5;
                        p.draw_line(pt(10.0, y), pt(w as f64 - 10.0, y));
                    }
                }
                top += rh;
            }
        }
        let mut bgra = Vec::new();
        rgba_to_bgra(pm.data(), &mut bgra);
        // rounded corners stay transparent even without a compositor
        let rects: Vec<Rectangle> = alpha_shape(pm.data(), w, h, 0)
            .iter()
            .map(|r| Rectangle { x: r.x as i16, y: r.y as i16, width: r.w as u16, height: r.h as u16 })
            .collect();
        let _ = shape::rectangles(conn, SO::SET, SK::BOUNDING, ClipOrdering::YX_BANDED, self.xid, 0, 0, &rects);
        let _ = conn.put_image(ImageFormat::Z_PIXMAP, self.xid, self.gc, w as u16, h as u16, 0, 0, 0, 32, &bgra);
        let _ = conn.flush();
    }

    /// Feed an X event while the menu is open (it holds the grab, so it gets them all).
    pub fn on_event(&mut self, conn: &RustConnection, ev: &XEvent) -> Outcome {
        match ev {
            XEvent::MotionNotify(e) => {
                let hover = self.item_at(e.root_x as i32, e.root_y as i32);
                if hover != self.hover {
                    self.hover = hover;
                    self.render(conn);
                }
                Outcome::Open
            }
            XEvent::ButtonPress(e) => {
                if self.rect.contains(e.root_x as i32, e.root_y as i32) {
                    self.pressed_inside = true;
                    Outcome::Open
                } else {
                    Outcome::Close
                }
            }
            XEvent::ButtonRelease(e) => {
                let fresh = self.opened.elapsed() < OPEN_GRACE && !self.pressed_inside;
                match self.item_at(e.root_x as i32, e.root_y as i32) {
                    Some(i) if !fresh => match &self.rows[i] {
                        Row::Item { id, .. } => Outcome::Chosen(*id),
                        _ => Outcome::Open,
                    },
                    _ => Outcome::Open,
                }
            }
            XEvent::KeyPress(_) => Outcome::Close,
            XEvent::Expose(e) if e.window == self.xid && e.count == 0 => {
                self.render(conn);
                Outcome::Open
            }
            _ => Outcome::Open,
        }
    }

    pub fn close(self, conn: &RustConnection) {
        let _ = conn.ungrab_pointer(x11rb::CURRENT_TIME);
        let _ = conn.ungrab_keyboard(x11rb::CURRENT_TIME);
        let _ = conn.free_gc(self.gc);
        let _ = conn.destroy_window(self.xid);
        let _ = conn.flush();
    }
}
