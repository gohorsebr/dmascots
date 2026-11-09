//! The herd of mascot windows and the main loop (port of `window.py` + the `Herd` in `app.py`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json::{json, Value};

use crate::art::render_icon;
use crate::characters;
use crate::engine::{Bounds, Character, Mascot, Pointer};
use crate::gfx::{pt, Color, Painter};
use crate::ipc::Request;
use crate::platform::{self, Backend, Button, CursorShape, Event, Frame, MenuItem, Rect, Screen, WinId};

pub const MAX_TEXT: usize = 500;
const FEET_FROM_BOTTOM: f64 = 6.0;
const FRAME: Duration = Duration::from_millis(16);
const RAISE_EVERY: Duration = Duration::from_secs(2);

// menu command ids
const CMD_SAY_ALL: u32 = 1;
const CMD_STAY: u32 = 2;
const CMD_THROUGH: u32 = 3;
const CMD_QUIT: u32 = 4;
const CMD_ADD: u32 = 100; // + index into characters::all()
const CMD_SAY_TO: u32 = 1 << 20; // | mascot id
const CMD_SHOO: u32 = 2 << 20; // | mascot id
const ID_MASK: u32 = (1 << 20) - 1;

/// Set from the Ctrl+C / SIGTERM handler.
pub static QUIT: AtomicBool = AtomicBool::new(false);

/// How long a bubble stays up: long enough to read, never forever.
pub fn reading_time(text: &str) -> f64 {
    (1.5 + 0.06 * text.chars().count() as f64).clamp(2.5, 15.0)
}

/// Collapse whitespace into one paragraph and cap the length (the bubble wraps it).
pub fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_TEXT).collect()
}

fn capitalize(s: &str) -> String {
    s[..1].to_uppercase() + &s[1..]
}

/// One mascot and its window.
pub struct MascotWin {
    pub id: u32,
    pub win: WinId,
    pub character: &'static Character,
    pub brain: Box<dyn Mascot>,
    pub click_through: bool,
    pub expires_at: Option<Instant>,
    s: f64, // scale
    w: u32,
    h: u32,
    screen: Screen,
    press: Option<(i32, i32)>,
    grab_offset: (f64, f64),
    dragging: bool,
    pull_from: Option<(f64, f64)>, // where a stolen pointer was snatched from
    last: Instant,
    frame: tiny_skia::Pixmap,
    bgra: Vec<u8>,
}

impl MascotWin {
    fn bounds(&mut self, screens: &[Screen], cursor: (i32, i32)) -> Bounds {
        let c = self.brain.core();
        let probe = if self.dragging { cursor } else { (c.x as i32, c.y as i32 - 5) };
        if let Some(s) = screens.iter().find(|s| s.geometry.contains(probe.0, probe.1)) {
            self.screen = *s;
        }
        let g = self.screen.available;
        let m = self.character.half_width * self.s;
        Bounds {
            left: g.x as f64 + m,
            right: (g.x + g.w) as f64 - m,
            top: g.y as f64 + self.character.height * self.s,
            floor: (g.y + g.h) as f64,
        }
    }

    /// Grip point in painter units (feet origin, unscaled, mirrored for facing).
    fn grip_local(&self) -> (f64, f64) {
        let (gx, gy) = self.character.grip;
        (gx * self.brain.core().facing, gy)
    }

    fn tick(&mut self, be: &mut dyn Backend, screens: &[Screen]) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f64();
        self.last = now;
        let cur = be.cursor_pos();
        let bounds = self.bounds(screens, cur);
        let c = self.brain.core();
        let eye_y = c.y - self.character.eye_y * self.s;
        let rel = (cur.0 as f64 - c.x, cur.1 as f64 - eye_y);
        self.brain.update(dt, &bounds, Some(rel));
        self.move_pointer(be, cur);
        self.render(be);
    }

    fn window_pos(&self) -> (i32, i32) {
        let c = self.brain.core();
        (
            (c.x - self.w as f64 / 2.0).round() as i32,
            (c.y - (self.character.size.1 as f64 - FEET_FROM_BOTTOM) * self.s).round() as i32,
        )
    }

    fn render(&mut self, be: &mut dyn Backend) {
        self.frame.fill(tiny_skia::Color::TRANSPARENT);
        let grip = self.grip_local();
        {
            let mut p = Painter::new(&mut self.frame);
            p.translate(self.w as f64 / 2.0, self.h as f64 - FEET_FROM_BOTTOM * self.s);
            p.scale(self.s, self.s);
            self.brain.paint(&mut p, false);
            if self.brain.core().pointer.is_some() {
                // a near-invisible dot under the held pointer, so clicking the thief hits it
                // (Windows only passes clicks through *fully* transparent pixels; X11's shape follows the alpha)
                p.no_pen();
                p.set_brush(Color::rgba(0, 0, 0, 2));
                p.draw_circle(pt(grip.0, grip.1), 9.0);
            }
        }
        let data = self.frame.data();
        platform::rgba_to_bgra(data, &mut self.bgra);
        let shape = if cfg!(unix) { Some(platform::alpha_shape(data, self.w, self.h, 2)) } else { None };
        let (x, y) = self.window_pos();
        be.present(self.win, x, y, &Frame { width: self.w, height: self.h, bgra: &self.bgra, shape: shape.as_deref() });
    }

    /// While this mascot is stealing the pointer, put the real pointer in its mouth.
    fn move_pointer(&mut self, be: &mut dyn Backend, cur: (i32, i32)) {
        let Some(ptr) = self.brain.core().pointer else {
            if self.pull_from.take().is_some() {
                be.set_cursor(self.win, CursorShape::OpenHand); // just let go
            }
            return;
        };
        let (gx, gy) = self.grip_local();
        let c = self.brain.core();
        let mut target = (c.x + gx * self.s, c.y + gy * self.s);
        let from = *self.pull_from.get_or_insert_with(|| {
            be.set_cursor(self.win, CursorShape::Arrow); // look like the ordinary pointer being carried
            (cur.0 as f64, cur.1 as f64)
        });
        if let Pointer::Pull(k) = ptr {
            let e = k * k * (3.0 - 2.0 * k);
            target = (from.0 + (target.0 - from.0) * e, from.1 + (target.1 - from.1) * e);
        }
        be.set_cursor_pos(target.0.round() as i32, target.1.round() as i32);
    }

    fn on_press(&mut self, x: i32, y: i32) {
        let c = self.brain.core();
        self.press = Some((x, y));
        self.grab_offset = (x as f64 - c.x, y as f64 - c.y);
        self.dragging = false;
    }

    fn on_motion(&mut self, be: &mut dyn Backend, x: i32, y: i32) {
        let Some((px, py)) = self.press else { return };
        if !self.dragging && (x - px).abs() + (y - py).abs() > 4 {
            self.dragging = true;
            self.brain.core_mut().grab();
            be.set_cursor(self.win, CursorShape::ClosedHand);
        }
        if self.dragging {
            let (ox, oy) = self.grab_offset;
            self.brain.core_mut().drag_to(x as f64 - ox, y as f64 - oy);
        }
    }

    fn on_release(&mut self, be: &mut dyn Backend) {
        if self.press.is_none() {
            return;
        }
        if self.dragging {
            self.brain.core_mut().release();
            be.set_cursor(self.win, CursorShape::OpenHand);
        } else {
            self.brain.poke();
        }
        self.press = None;
        self.dragging = false;
    }

    /// What `list` reports about this mascot.
    pub fn describe(&self) -> Value {
        let c = self.brain.core();
        let left = self.expires_at.map(|t| t.saturating_duration_since(Instant::now()).as_secs_f64());
        json!({
            "id": self.id,
            "name": self.character.name,
            "state": c.state,
            "x": c.x.round() as i64,
            "y": c.y.round() as i64,
            "leaves_in": left,
            "holding_pointer": c.pointer.is_some(),
        })
    }
}

pub struct Options {
    pub scale: f64,
    pub tray: bool,
    /// right-clicking a mascot opens its menu
    pub menu: bool,
    pub click_through: bool,
    /// set when started with --time: exit once all have left
    pub quit_when_empty: bool,
}

/// All live mascot windows plus the menus and commands that control them.
pub struct Herd {
    be: Box<dyn Backend>,
    pub windows: Vec<MascotWin>,
    scale: f64,
    menu_enabled: bool,
    stay_put: bool,
    click_through: bool,
    has_tray: bool,
    pub quit_when_empty: bool,
    pub quitting: bool,
    next_id: u32,
}

impl Herd {
    pub fn new(mut be: Box<dyn Backend>, opts: &Options) -> Herd {
        let scale = opts.scale * be.device_scale();
        let mut herd = Herd {
            be: Box::new(platform::Headless::default()),
            windows: Vec::new(),
            scale,
            menu_enabled: opts.menu,
            stay_put: false,
            click_through: opts.click_through,
            has_tray: false,
            quit_when_empty: opts.quit_when_empty,
            quitting: false,
            next_id: 1,
        };
        if opts.tray {
            let icon = render_icon(characters::all()[0], 64);
            let rgba: Vec<u8> = icon
                .pixels()
                .iter()
                .flat_map(|p| {
                    let c = p.demultiply();
                    [c.red(), c.green(), c.blue(), c.alpha()]
                })
                .collect();
            herd.has_tray = be.tray(&rgba, 64, herd.tray_menu());
        }
        herd.be = be;
        herd
    }

    pub fn has_tray(&self) -> bool {
        self.has_tray
    }

    // ----------------------------------------------------------- menus
    fn shared_menu(&self) -> Vec<MenuItem> {
        let add = characters::all()
            .iter()
            .enumerate()
            .map(|(i, c)| MenuItem::action(CMD_ADD + i as u32, capitalize(c.name)))
            .collect();
        vec![
            MenuItem::Submenu { label: "Add".into(), items: add },
            MenuItem::check(CMD_STAY, "Stay put", self.stay_put),
            MenuItem::check(CMD_THROUGH, "Click-through", self.click_through),
            MenuItem::Separator,
            MenuItem::action(CMD_QUIT, "Quit"),
        ]
    }

    fn tray_menu(&self) -> Vec<MenuItem> {
        let mut m = vec![MenuItem::action(CMD_SAY_ALL, "Say something…")];
        m.extend(self.shared_menu());
        m
    }

    fn mascot_menu(&self, m: &MascotWin) -> Vec<MenuItem> {
        let mut items = vec![
            MenuItem::action(CMD_SAY_TO | m.id, "Say something…"),
            MenuItem::action(CMD_SHOO | m.id, format!("Shoo the {}", m.character.name)),
            MenuItem::Separator,
        ];
        items.extend(self.shared_menu());
        items
    }

    fn refresh_tray(&mut self) {
        if self.has_tray {
            let menu = self.tray_menu();
            self.be.set_tray_menu(menu);
        }
    }

    pub fn set_stay_put(&mut self, on: bool) {
        self.stay_put = on;
        for m in &mut self.windows {
            m.brain.core_mut().paused = on;
        }
        self.refresh_tray();
    }

    pub fn set_click_through(&mut self, on: bool) {
        self.click_through = on;
        for m in &mut self.windows {
            m.click_through = on;
            self.be.set_click_through(m.win, on);
        }
        self.refresh_tray();
    }

    fn on_menu(&mut self, cmd: u32) {
        match cmd {
            CMD_SAY_ALL => self.be.ask_text(0, "Desktop Mascots", "Say:"),
            CMD_STAY => self.set_stay_put(!self.stay_put),
            CMD_THROUGH => self.set_click_through(!self.click_through),
            CMD_QUIT => self.quitting = true,
            c if c & !ID_MASK == CMD_SAY_TO => self.be.ask_text(c & ID_MASK, "Desktop Mascots", "Say:"),
            c if c & !ID_MASK == CMD_SHOO => {
                if let Some(i) = self.windows.iter().position(|m| m.id == c & ID_MASK) {
                    self.remove(i);
                }
            }
            c if c >= CMD_ADD => {
                if let Some(ch) = characters::all().get((c - CMD_ADD) as usize) {
                    self.spawn(ch, None, None);
                }
            }
            _ => {}
        }
    }

    // ----------------------------------------------------------- mascots
    /// Show a new mascot. lifetime: seconds until it leaves; say: its opening line.
    /// Returns its index in `windows`.
    pub fn spawn(&mut self, character: &'static Character, lifetime: Option<f64>, say: Option<&str>) -> usize {
        let s = self.scale;
        let (w, h) = ((character.size.0 as f64 * s).round() as u32, (character.size.1 as f64 * s).round() as u32);
        let screens = self.be.screens();
        let fallback = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let screen = screens.first().copied().unwrap_or(Screen { geometry: fallback, available: fallback });
        let g = screen.available;
        let margin = character.half_width * s;
        let (lo, hi) = (g.x as f64 + margin, (g.x + g.w) as f64 - margin);
        let x = if hi > lo { rand::thread_rng().gen_range(lo..hi) } else { lo };
        let mut brain = character.brain(x, g.y as f64 + character.height * s, None);
        brain.core_mut().paused = self.stay_put;
        let id = self.next_id;
        self.next_id += 1;
        let win = self.be.create_window(w, h, &format!("Desktop mascot: {} #{id}", character.name));
        self.be.set_cursor(win, CursorShape::OpenHand);
        self.be.set_click_through(win, self.click_through);
        let mut m = MascotWin {
            id,
            win,
            character,
            brain,
            click_through: self.click_through,
            expires_at: lifetime.map(|l| Instant::now() + Duration::from_secs_f64(l)),
            s,
            w,
            h,
            screen,
            press: None,
            grab_offset: (0.0, 0.0),
            dragging: false,
            pull_from: None,
            last: Instant::now(),
            frame: tiny_skia::Pixmap::new(w, h).expect("frame"),
            bgra: Vec::new(),
        };
        if let Some(text) = say {
            // with a lifetime the opening line stays up until the mascot leaves
            let text = clean_text(text);
            if !text.is_empty() {
                let secs = lifetime.unwrap_or_else(|| reading_time(&text));
                m.brain.core_mut().say(&text, secs, true);
            }
        }
        m.tick(self.be.as_mut(), &screens);
        self.windows.push(m);
        self.windows.len() - 1
    }

    pub fn remove(&mut self, i: usize) {
        let m = self.windows.remove(i);
        self.be.destroy_window(m.win);
        // An empty herd keeps running (more can be added with 'add' or the tray);
        // only an app started with --time exits once its mascots have all left.
        if self.windows.is_empty() && self.quit_when_empty {
            self.quitting = true;
        }
    }

    /// Indices of the windows matching a target: "all"/"any", a mascot id, or a mascot name.
    pub fn select(&self, target: &str) -> Vec<usize> {
        let all = 0..self.windows.len();
        if target.is_empty() || target == "all" || target == "any" {
            return all.collect();
        }
        if target.bytes().all(|b| b.is_ascii_digit()) {
            let id: u64 = target.parse().unwrap_or(0);
            return all.filter(|&i| self.windows[i].id as u64 == id).collect();
        }
        all.filter(|&i| self.windows[i].character.name == target).collect()
    }

    fn say(&mut self, text: &str, which: &[usize], seconds: Option<f64>) -> usize {
        let text = clean_text(text);
        if text.is_empty() {
            return 0;
        }
        let secs = seconds.unwrap_or_else(|| reading_time(&text));
        for &i in which {
            self.windows[i].brain.core_mut().say(&text, secs, true);
        }
        which.len()
    }

    /// Requests from the control socket (see ipc.rs).
    pub fn handle(&mut self, req: &Value) -> Value {
        fn as_target(v: &Value) -> Option<String> {
            match v {
                Value::String(t) => Some(t.clone()),
                Value::Number(n) => Some(n.to_string()), // ids may arrive as numbers
                _ => None,
            }
        }
        let s = |k: &str| req.get(k).and_then(Value::as_str);
        let f = |k: &str| req.get(k).and_then(Value::as_f64);
        let target = |k: &str, default: &str| {
            req.get(k).and_then(as_target).filter(|t| !t.is_empty()).unwrap_or_else(|| default.to_string())
        };
        match s("cmd") {
            Some("list") => {
                let mascots: Vec<Value> = self.windows.iter().map(MascotWin::describe).collect();
                json!({"ok": true, "count": mascots.len(), "mascots": mascots})
            }
            Some("spawn") => {
                let chars: Vec<&'static Character> = req
                    .get("mascots")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).filter_map(characters::get).collect())
                    .unwrap_or_default();
                let lifetime = f("lifetime").filter(|&l| l > 0.0);
                let say = s("say").map(str::to_string);
                let added: Vec<usize> = chars.iter().map(|c| self.spawn(c, lifetime, say.as_deref())).collect();
                let mascots: Vec<Value> = added.iter().map(|&i| self.windows[i].describe()).collect();
                json!({"ok": true, "count": mascots.len(), "mascots": mascots})
            }
            Some("remove") => {
                let mut doomed: Vec<u32> = Vec::new();
                for t in req.get("targets").and_then(Value::as_array).into_iter().flatten().filter_map(as_target) {
                    for i in self.select(&t) {
                        let id = self.windows[i].id;
                        if !doomed.contains(&id) {
                            doomed.push(id);
                        }
                    }
                }
                let mut gone = Vec::new();
                for id in doomed {
                    if let Some(i) = self.windows.iter().position(|m| m.id == id) {
                        gone.push(self.windows[i].describe());
                        self.remove(i);
                    }
                }
                json!({"ok": true, "count": gone.len(), "mascots": gone})
            }
            Some("say") => {
                let which = self.select(&target("to", "all"));
                let seconds = f("seconds").filter(|&x| x > 0.0);
                let count = self.say(s("text").unwrap_or(""), &which, seconds);
                json!({"ok": true, "count": count})
            }
            Some("steal") => {
                if self.windows.iter().any(|m| {
                    let c = m.brain.core();
                    c.state == "steal" || c.pointer.is_some() || c.pending_steal.is_some()
                }) {
                    return json!({"ok": false, "error": "someone is already running off with it"});
                }
                let to = target("to", "any");
                let thieves: Vec<usize> =
                    self.select(&to).into_iter().filter(|&i| self.windows[i].brain.core().state != "dragged").collect();
                if thieves.is_empty() {
                    let who = if to == "any" { "mascot" } else { &to };
                    return json!({"ok": false, "error": format!("no {who} available to steal it")});
                }
                let seconds = f("seconds").filter(|&x| x > 0.0).unwrap_or(5.0);
                let i = thieves[rand::thread_rng().gen_range(0..thieves.len())];
                self.windows[i].brain.core_mut().steal(seconds);
                json!({"ok": true, "count": 1, "mascots": [self.windows[i].describe()]})
            }
            Some("quit") => {
                self.quitting = true; // after the reply has gone out
                json!({"ok": true, "count": self.windows.len()})
            }
            other => {
                let shown = other.map_or("None".to_string(), |o| format!("'{o}'"));
                json!({"ok": false, "error": format!("unknown command: {shown}")})
            }
        }
    }

    fn on_event(&mut self, ev: Event) {
        let find = |herd: &Herd, win: WinId| herd.windows.iter().position(|m| m.win == win);
        match ev {
            Event::Press { win, x, y, button } => {
                let Some(i) = find(self, win) else { return };
                match button {
                    Button::Left => self.windows[i].on_press(x, y),
                    Button::Right if self.menu_enabled => {
                        let menu = self.mascot_menu(&self.windows[i]);
                        self.be.popup_menu(x, y, menu);
                    }
                    _ => {} // otherwise right-clicks are simply ignored
                }
            }
            Event::Motion { win, x, y } => {
                if let Some(i) = find(self, win) {
                    self.windows[i].on_motion(self.be.as_mut(), x, y);
                }
            }
            Event::Release { win, button: Button::Left, .. } => {
                if let Some(i) = find(self, win) {
                    self.windows[i].on_release(self.be.as_mut());
                }
            }
            Event::Release { .. } => {}
            Event::Menu(cmd) => self.on_menu(cmd),
            Event::Said { tag, text } => {
                let which: Vec<usize> = if tag == 0 {
                    (0..self.windows.len()).collect()
                } else {
                    self.windows.iter().position(|m| m.id == tag).into_iter().collect()
                };
                self.say(&text, &which, None);
            }
            Event::Quit => self.quitting = true,
        }
    }

    /// One pass of the main loop: wait for input until the next frame, answer requests, animate.
    fn step(&mut self, requests: Option<&mpsc::Receiver<Request>>, next_frame: &mut Instant, next_raise: &mut Instant) {
        let wait = next_frame.saturating_duration_since(Instant::now());
        for ev in self.be.poll(wait) {
            self.on_event(ev);
        }
        if let Some(rx) = requests {
            while let Ok(req) = rx.try_recv() {
                let reply = self.handle(&req.body);
                let _ = req.reply.send(reply);
            }
        }
        let now = Instant::now();
        if now < *next_frame {
            return;
        }
        let screens = self.be.screens();
        // mascots whose --time is up leave
        let mut i = 0;
        while i < self.windows.len() {
            if self.windows[i].expires_at.is_some_and(|t| now >= t) {
                self.remove(i);
            } else {
                i += 1;
            }
        }
        for m in &mut self.windows {
            m.tick(self.be.as_mut(), &screens);
        }
        if now >= *next_raise {
            for m in &self.windows {
                self.be.raise(m.win);
            }
            *next_raise = now + RAISE_EVERY;
        }
        *next_frame += FRAME;
        if *next_frame < now {
            *next_frame = now + FRAME; // fell behind (suspend, slow frame): don't try to catch up
        }
    }

    /// Run until quit. Returns the process exit code.
    pub fn run(mut self, requests: Option<&mpsc::Receiver<Request>>) -> i32 {
        let mut next_frame = Instant::now();
        let mut next_raise = Instant::now() + RAISE_EVERY;
        while !self.quitting && !QUIT.load(Ordering::Relaxed) {
            self.step(requests, &mut next_frame, &mut next_raise);
        }
        for m in std::mem::take(&mut self.windows) {
            self.be.destroy_window(m.win);
        }
        0
    }
}
