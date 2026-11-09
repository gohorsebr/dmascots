//! Shared mascot engine: base pose, physics and the behaviour state machine.
//!
//! Port of `desktop_mascots/engine.py`. No windowing here, so behaviour runs headlessly
//! (see tests/). Coordinates are screen pixels; (x, y) is the point between the feet.
//!
//! A character is a `Species`: a pose type, its `Params` (the Python class constants),
//! the `pose_*` hooks for the shared states, and its own states via `Species::state`.
//! Everything the Python `Brain` keeps on `self` lives in `Core`, which the species
//! hooks receive, so species code reads like the Python (`c.clock`, `c.go(..)`, `c.say(..)`).

use std::f64::consts::PI;
use std::ops::DerefMut;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::gfx::Painter;

pub const GRAVITY: f64 = 2600.0; // px/s²
pub const MAX_THROW: f64 = 1800.0; // px/s

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub left: f64, // leftmost x the feet may reach
    pub right: f64,
    pub top: f64, // highest y the feet may reach (keeps the head on screen)
    pub floor: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eyes {
    #[default]
    Open,
    Wide,
    Closed,
    Happy,
    Dizzy,
    Big,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasePose {
    pub bob: f64,    // vertical body offset, px (negative = up)
    pub squash: f64, // <1 squashed, >1 stretched (pivot: feet)
    pub lean: f64,   // whole-body tilt, degrees, positive = forward
    pub swing: f64,  // pendulum swing around the grab point (when dragged)
    pub eyes: Eyes,
    pub look: (f64, f64),       // pupil direction (screen space), -1..1
    pub stars: Option<f64>,     // dizzy stars orbit angle (radians)
    pub bubble: Option<String>, // speech bubble text
    pub grounded: bool,         // keep the lowest foot planted on the floor
}

impl Default for BasePose {
    fn default() -> Self {
        BasePose {
            bob: 0.0,
            squash: 1.0,
            lean: 0.0,
            swing: 0.0,
            eyes: Eyes::Open,
            look: (0.0, 0.0),
            stars: None,
            bubble: None,
            grounded: true,
        }
    }
}

/// Declare a character pose: a struct with a `base: BasePose` field that derefs to it,
/// so `p.bob` and `p.head` both work, like the Python dataclass inheritance.
#[macro_export]
macro_rules! pose {
    ($(#[$m:meta])* $name:ident { $($field:ident : $ty:ty = $default:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name {
            pub base: $crate::engine::BasePose,
            $(pub $field: $ty,)*
        }
        impl Default for $name {
            fn default() -> Self {
                $name { base: Default::default(), $($field: $default,)* }
            }
        }
        impl std::ops::Deref for $name {
            type Target = $crate::engine::BasePose;
            fn deref(&self) -> &$crate::engine::BasePose { &self.base }
        }
        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut $crate::engine::BasePose { &mut self.base }
        }
    };
}

pub type Choice = (f64, &'static str, (f64, f64));

/// The Python `Brain` class constants. Characters override with struct-update syntax:
/// `const PARAMS: Params = Params { walk_speed: 55.0, ..Params::BASE };`
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub walk_speed: f64,
    pub jump_vy: f64,
    pub jump_vx: f64,
    pub dizzy_impact: f64, // landing speed that leaves it seeing stars
    pub lines: &'static [&'static str],
    pub steal_speed: f64, // px/s while running off with the pointer
    pub steal_lines: &'static [&'static str],
    pub return_lines: &'static [&'static str],
    /// states where the feet are off the floor and the state itself moves the mascot
    pub airborne: &'static [&'static str],
    pub fly_speed: f64,  // px/s across the screen
    pub fly_range: f64,  // longest hop between perches, px
    pub fly_height: (f64, f64),
    /// (weight, state, (min_seconds, max_seconds)) picked whenever idle time runs out
    pub choices: &'static [Choice],
    pub max_steal: f64,
    pub chase_limit: f64, // after this long chasing, it just yanks the pointer over
    pub snatch_time: f64,
}

impl Params {
    pub const BASE: Params = Params {
        walk_speed: 75.0,
        jump_vy: -720.0,
        jump_vx: 220.0,
        dizzy_impact: 1900.0,
        lines: &["Hi!"],
        steal_speed: 230.0,
        steal_lines: &["Mine!", "Hehehe!", "Gotcha!"],
        return_lines: &["Fine, have it back.", "Okay, okay!", "…here."],
        airborne: &["air", "dragged"],
        fly_speed: 340.0,
        fly_range: 900.0,
        fly_height: (70.0, 230.0),
        choices: &[(45.0, "walk", (2.5, 7.0)), (10.0, "crouch", (0.14, 0.14)), (25.0, "idle", (1.5, 4.0))],
        max_steal: 60.0,
        chase_limit: 3.0,
        snatch_time: 0.45,
    };
    /// Airborne states for flyers (crow, parrot).
    pub const FLYER_AIRBORNE: &'static [&'static str] = &["air", "dragged", "fly"];
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn lerp2(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

pub fn ease(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth 0 -> 1 -> 0 pulse of the given width around center.
pub fn bump(t: f64, center: f64, width: f64) -> f64 {
    let d = (t - center).abs() / (width / 2.0);
    if d < 1.0 {
        (d * PI / 2.0).cos().powi(2)
    } else {
        0.0
    }
}

/// Eased interpolation through `(time, [values...])` keyframes; ends are held.
pub fn keyframes<const N: usize>(t: f64, frames: &[(f64, [f64; N])]) -> [f64; N] {
    if t <= frames[0].0 {
        return frames[0].1;
    }
    for w in frames.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if t <= b.0 {
            let k = if b.0 > a.0 { ease((t - a.0) / (b.0 - a.0)) } else { 1.0 };
            let mut out = [0.0; N];
            for i in 0..N {
                out[i] = lerp(a.1[i], b.1[i], k);
            }
            return out;
        }
    }
    frames[frames.len() - 1].1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    /// dragging the pointer in, 0..1
    Pull(f64),
    /// holding it in the mouth/beak
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AirKind {
    Fall,
    Jump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StealPhase {
    Chase,
    Snatch,
    Carry,
}

/// Everything the Python Brain keeps on `self`, shared by every species.
pub struct Core {
    pub params: &'static Params,
    pub rng: SmallRng,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub facing: f64, // +1 / -1
    pub paused: bool,
    pub clock: f64,    // seconds since birth
    pub t: f64,        // seconds in the current state
    pub duration: f64, // planned length of the current state
    pub state: &'static str,
    pub cursor: Option<(f64, f64)>, // mouse, relative to the eyes
    pub heard: Vec<String>,         // recent messages sent to it (parrots repeat these)
    /// while stealing: Pull(0..1) dragging the pointer in, then Hold
    pub pointer: Option<Pointer>,
    pub pending_steal: Option<f64>,
    dt: f64,
    air_kind: AirKind,
    drag_target: (f64, f64),
    drag_v: (f64, f64),
    swing: f64,
    swing_v: f64,
    look: (f64, f64),
    next_blink: f64,
    blink_until: f64,
    bubble: Option<String>,
    bubble_until: f64,
    bubble_important: bool,
    steal_phase: StealPhase,
    steal_t0: f64,
    steal_dist: f64,
    steal_turn: f64,
    fly_from: f64,
    fly_to: f64,
    fly_h: f64,
}

impl Core {
    pub fn new(params: &'static Params, x: f64, y: f64, rng: SmallRng) -> Core {
        Core {
            params,
            rng,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            facing: 1.0,
            paused: false,
            clock: 0.0,
            t: 0.0,
            duration: 0.0,
            state: "air", // starts by dropping in
            cursor: None,
            heard: Vec::new(),
            pointer: None,
            pending_steal: None,
            dt: 0.0,
            air_kind: AirKind::Fall,
            drag_target: (x, y),
            drag_v: (0.0, 0.0),
            swing: 0.0,
            swing_v: 0.0,
            look: (0.0, 0.0),
            next_blink: 1.0,
            blink_until: -1.0,
            bubble: None,
            bubble_until: 0.0,
            bubble_important: false,
            steal_phase: StealPhase::Chase,
            steal_t0: 0.0,
            steal_dist: 0.0,
            steal_turn: 0.0,
            fly_from: x,
            fly_to: x,
            fly_h: 0.0,
        }
    }

    pub fn airborne(&self) -> bool {
        self.params.airborne.contains(&self.state)
    }

    /// True on the first update of the current state.
    pub fn entered(&self) -> bool {
        self.t <= self.dt + 1e-9
    }

    pub fn done(&self) -> bool {
        self.t >= self.duration
    }

    pub fn go(&mut self, state: &'static str, duration: f64) {
        self.state = state;
        self.t = 0.0;
        self.duration = duration;
    }

    pub fn go_idle(&mut self) {
        let d = self.uniform(1.5, 4.0);
        self.go("idle", d);
    }

    /// Show a speech bubble. Chatter never replaces an important message still showing.
    pub fn say(&mut self, text: &str, seconds: f64, important: bool) {
        if !important && self.bubble_important && self.clock < self.bubble_until {
            return;
        }
        self.bubble = Some(text.to_string());
        self.bubble_until = self.clock + seconds;
        self.bubble_important = important;
        if important {
            self.heard.push(text.to_string());
            if self.heard.len() > 6 {
                self.heard.remove(0);
            }
        }
    }

    /// Chatter: say one of the given lines for 1.6 s.
    pub fn chatter(&mut self, lines: &[&str]) {
        let line = self.choice(lines);
        self.say(line, 1.6, false);
    }

    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        if hi > lo {
            self.rng.gen_range(lo..hi)
        } else {
            lo
        }
    }
    pub fn random(&mut self) -> f64 {
        self.rng.gen::<f64>()
    }
    pub fn choice<'s, T: ?Sized>(&mut self, items: &[&'s T]) -> &'s T {
        items[self.rng.gen_range(0..items.len())]
    }
    pub fn coin(&mut self) -> f64 {
        if self.rng.gen::<bool>() {
            1.0
        } else {
            -1.0
        }
    }

    /// Walk horizontally, turning around at the screen edges.
    pub fn step_x(&mut self, b: &Bounds, dx: f64) {
        self.x += dx;
        if self.x <= b.left {
            self.x = b.left;
            self.facing = 1.0;
        } else if self.x >= b.right {
            self.x = b.right;
            self.facing = -1.0;
        }
    }

    fn pick_next(&mut self, b: &Bounds) {
        let choices = self.params.choices;
        let total: f64 = choices.iter().map(|c| c.0).sum();
        let mut r = self.random() * total;
        let mut pick = choices[choices.len() - 1];
        for &c in choices {
            r -= c.0;
            if r < 0.0 {
                pick = c;
                break;
            }
        }
        let (_, state, (lo, hi)) = pick;
        if state == "walk" {
            if b.right - self.x < 60.0 {
                self.facing = -1.0;
            } else if self.x - b.left < 60.0 {
                self.facing = 1.0;
            } else {
                self.facing = self.coin();
            }
        }
        let d = self.uniform(lo, hi);
        self.go(state, d);
    }

    /// The base Brain.poke: chatter and hop.
    pub fn default_poke(&mut self) {
        self.chatter(self.params.lines);
        if !self.airborne() && !matches!(self.state, "crouch" | "land" | "dizzy") {
            self.go("crouch", 0.14);
        }
    }

    pub fn grab(&mut self) {
        self.drag_target = (self.x, self.y);
        self.drag_v = (0.0, 0.0);
        self.swing = 0.0;
        self.swing_v = 0.0;
        self.go("dragged", 0.0);
    }

    pub fn drag_to(&mut self, x: f64, y: f64) {
        self.drag_target = (x, y);
    }

    pub fn release(&mut self) {
        self.vx = clamp(self.drag_v.0, -MAX_THROW, MAX_THROW);
        self.vy = clamp(self.drag_v.1, -MAX_THROW, MAX_THROW);
        if self.vx.abs() > 40.0 {
            self.facing = if self.vx > 0.0 { 1.0 } else { -1.0 };
        }
        self.air_kind = AirKind::Fall;
        self.go("air", 0.0);
    }

    /// Leap with the given velocity (a jump: no wide-eyed falling face).
    pub fn launch(&mut self, vx: f64, vy: f64) {
        self.vx = vx;
        self.vy = vy;
        self.air_kind = AirKind::Jump;
        self.go("air", 0.0);
    }

    /// Start a fall from the current position (used by tools/tests to throw it).
    pub fn throw(&mut self, vx: f64, vy: f64) {
        self.vx = vx;
        self.vy = vy;
        self.air_kind = AirKind::Fall;
        self.go("air", 0.0);
    }

    /// Run to the pointer, grab it, run around holding it for `seconds`.
    pub fn steal(&mut self, seconds: f64) -> bool {
        let seconds = clamp(seconds, 0.5, self.params.max_steal);
        if self.state == "dragged" {
            return false;
        }
        if self.airborne() {
            self.pending_steal = Some(seconds); // after landing
        } else {
            self.go("steal", seconds);
        }
        true
    }
}

/// One kind of mascot: its pose type, constants, pose hooks, extra states and painter.
pub trait Species: 'static {
    type Pose: Default + Clone + DerefMut<Target = BasePose>;
    const PARAMS: Params;

    fn pose_rest(&self, c: &Core, p: &mut Self::Pose) {
        p.bob = (c.clock * 2.2).sin() * 0.7;
    }
    fn pose_walk(&self, c: &Core, p: &mut Self::Pose, _t: f64) {
        self.pose_rest(c, p);
    }
    fn pose_crouch(&self, c: &Core, p: &mut Self::Pose, _k: f64) {
        self.pose_rest(c, p);
    }
    fn pose_jump(&self, _c: &Core, _p: &mut Self::Pose) {}
    fn pose_fall(&self, _c: &Core, _p: &mut Self::Pose) {}
    fn pose_land(&self, c: &Core, p: &mut Self::Pose, _bend: f64) {
        self.pose_rest(c, p);
    }
    fn pose_dizzy(&self, _c: &Core, _p: &mut Self::Pose) {}
    fn pose_dragged(&self, _c: &Core, _p: &mut Self::Pose) {}
    /// Running pose; travelled (px) drives the gait so feet keep pace with the ground.
    fn pose_run(&self, c: &Core, p: &mut Self::Pose, travelled: f64) {
        self.pose_walk(c, p, travelled / Self::PARAMS.walk_speed);
    }
    /// Pose while flying; prog runs 0 (take-off) -> 1 (touch-down).
    fn pose_flight(&self, _c: &Core, _p: &mut Self::Pose, _prog: f64) {}

    fn poke(&mut self, c: &mut Core) {
        c.default_poke();
    }

    /// The character's own states. Return false for states it doesn't know
    /// (the shared ones are handled by the engine).
    fn state(&mut self, _c: &mut Core, _dt: f64, _b: &Bounds, _p: &mut Self::Pose) -> bool {
        false
    }

    /// paint(painter, pose, facing, head_only): origin between the feet, y down, facing +x.
    fn paint(&self, p: &mut Painter, pose: &Self::Pose, facing: f64, head_only: bool);
}

/// A species plus its shared state: what the Python `Brain` instance is.
pub struct Brain<S: Species> {
    pub core: Core,
    pub sp: S,
    pub pose: S::Pose,
}

impl<S: Species> Brain<S> {
    pub const PARAMS: &'static Params = &S::PARAMS;

    pub fn new(sp: S, x: f64, y: f64, seed: Option<u64>) -> Brain<S> {
        let rng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => SmallRng::from_entropy(),
        };
        Brain { core: Core::new(Self::PARAMS, x, y, rng), sp, pose: S::Pose::default() }
    }

    /// Advance by dt seconds. cursor = mouse position relative to the eyes.
    pub fn update(&mut self, dt: f64, b: &Bounds, cursor: Option<(f64, f64)>) {
        let dt = clamp(dt, 0.0, 0.05);
        let c = &mut self.core;
        c.dt = dt;
        c.clock += dt;
        c.t += dt;
        c.cursor = cursor;
        c.pointer = None; // only the steal state holds the pointer, frame by frame
        if !c.airborne() {
            if c.y < b.floor - 2.0 {
                // floor moved away (screen/workarea change)
                c.vx = 0.0;
                c.vy = 0.0;
                c.air_kind = AirKind::Fall;
                c.go("air", 0.0);
            } else {
                c.y = b.floor;
                c.x = clamp(c.x, b.left, b.right);
                if let Some(s) = c.pending_steal {
                    if c.state != "dizzy" {
                        c.go("steal", s); // asked while in the air: do it now
                        c.pending_steal = None;
                    }
                }
            }
        }
        let mut p = S::Pose::default();
        self.run_state(dt, b, &mut p);
        self.face(dt, &mut p);
        self.pose = p;
    }

    fn run_state(&mut self, dt: f64, b: &Bounds, p: &mut S::Pose) {
        if self.sp.state(&mut self.core, dt, b, p) {
            return;
        }
        match self.core.state {
            "idle" => self.idle(b, p),
            "walk" => self.walk(dt, b, p),
            "crouch" => self.crouch(p),
            "air" => self.air(dt, b, p),
            "land" => self.land(p),
            "dizzy" => self.dizzy(p),
            "dragged" => self.dragged(dt, p),
            "steal" => self.steal_state(dt, b, p),
            "fly" => self.fly(b, p),
            other => {
                debug_assert!(false, "unknown state {other}");
                self.sp.pose_rest(&self.core, p);
                self.core.go_idle();
            }
        }
    }

    // ----------------------------------------------------------- shared states
    fn idle(&mut self, b: &Bounds, p: &mut S::Pose) {
        self.sp.pose_rest(&self.core, p);
        if self.core.done() && !self.core.paused {
            self.core.pick_next(b);
        }
    }

    fn walk(&mut self, dt: f64, b: &Bounds, p: &mut S::Pose) {
        let c = &mut self.core;
        if c.paused || c.done() {
            self.sp.pose_rest(c, p);
            c.go_idle();
            return;
        }
        let dx = c.facing * c.params.walk_speed * dt;
        c.step_x(b, dx);
        self.sp.pose_walk(c, p, c.t);
    }

    fn crouch(&mut self, p: &mut S::Pose) {
        let c = &mut self.core;
        let k = ease(c.t / c.duration);
        self.sp.pose_crouch(c, p, k);
        p.squash = p.squash.min(1.0 - 0.14 * k);
        if c.done() {
            c.vy = c.params.jump_vy * c.uniform(0.85, 1.1);
            c.vx = if c.paused { 0.0 } else { c.facing * c.params.jump_vx * c.uniform(0.3, 1.0) };
            c.air_kind = AirKind::Jump;
            c.go("air", 0.0);
        }
    }

    fn air(&mut self, dt: f64, b: &Bounds, p: &mut S::Pose) {
        let c = &mut self.core;
        c.vy += GRAVITY * dt;
        c.x += c.vx * dt;
        c.y += c.vy * dt;
        if c.x < b.left {
            c.x = b.left;
            c.vx = c.vx.abs() * 0.5;
            c.facing = 1.0;
        } else if c.x > b.right {
            c.x = b.right;
            c.vx = -c.vx.abs() * 0.5;
            c.facing = -1.0;
        }
        if c.y < b.top && c.vy < 0.0 {
            c.y = b.top;
            c.vy = -c.vy * 0.3;
        }

        p.grounded = false;
        p.lean = clamp(c.vx * c.facing * 0.012, -12.0, 12.0);
        if c.air_kind == AirKind::Jump {
            p.squash = if c.vy < 0.0 { 1.06 } else { 1.0 };
            self.sp.pose_jump(c, p);
        } else {
            p.eyes = Eyes::Wide;
            self.sp.pose_fall(c, p);
        }

        if c.vy > 0.0 && c.y >= b.floor {
            let impact = c.vy;
            c.y = b.floor;
            c.vx = 0.0;
            c.vy = 0.0;
            if impact > c.params.dizzy_impact {
                c.go("dizzy", 2.6);
            } else {
                c.go("land", 0.22);
            }
        }
    }

    fn land(&mut self, p: &mut S::Pose) {
        let c = &mut self.core;
        let bend = 1.0 - ease(c.t / c.duration);
        self.sp.pose_land(c, p, bend);
        p.squash = 1.0 - 0.26 * bend;
        if c.done() {
            c.go_idle();
        }
    }

    fn dizzy(&mut self, p: &mut S::Pose) {
        let c = &mut self.core;
        self.sp.pose_rest(c, p);
        p.eyes = Eyes::Dizzy;
        p.stars = Some(c.clock * 4.5);
        p.lean = 5.0 * (c.clock * 3.2).sin();
        self.sp.pose_dizzy(c, p);
        if c.done() {
            c.go_idle();
        }
    }

    fn dragged(&mut self, dt: f64, p: &mut S::Pose) {
        let c = &mut self.core;
        let (tx, ty) = c.drag_target;
        if dt > 0.0 {
            let a = clamp(dt * 14.0, 0.0, 1.0);
            c.drag_v = (lerp(c.drag_v.0, (tx - c.x) / dt, a), lerp(c.drag_v.1, (ty - c.y) / dt, a));
        }
        c.x = tx;
        c.y = ty;
        // pendulum: the body trails behind the direction of motion
        let target = clamp(c.drag_v.0 * 0.03, -40.0, 40.0);
        c.swing_v += ((target - c.swing) * 90.0 - c.swing_v * 9.0) * dt;
        c.swing = clamp(c.swing + c.swing_v * dt, -45.0, 45.0);
        p.grounded = false;
        p.swing = c.swing * c.facing; // painters mirror when facing left
        p.eyes = Eyes::Wide;
        self.sp.pose_dragged(c, p);
    }

    // ----------------------------------------------------------- pointer theft
    fn steal_state(&mut self, dt: f64, b: &Bounds, p: &mut S::Pose) {
        let c = &mut self.core;
        if c.entered() {
            c.steal_phase = StealPhase::Chase;
            c.steal_t0 = 0.0;
            c.steal_dist = 0.0;
            c.steal_turn = 0.0;
        }
        if c.steal_phase == StealPhase::Chase {
            let dx = c.cursor.map_or(0.0, |cur| cur.0);
            // pointer out of reach (other monitor, past the edge)? yank it over from where we are
            let blocked = (dx < 0.0 && c.x <= b.left + 1.0) || (dx > 0.0 && c.x >= b.right - 1.0);
            if dx.abs() < 25.0 || blocked || c.t > c.params.chase_limit {
                c.steal_phase = StealPhase::Snatch;
                c.steal_t0 = c.t;
                let line = c.choice(c.params.steal_lines);
                c.say(line, 1.2, false);
            } else {
                c.facing = if dx > 0.0 { 1.0 } else { -1.0 };
                let step = c.params.steal_speed * dt;
                c.step_x(b, c.facing * step);
                c.steal_dist += step;
                self.sp.pose_run(c, p, c.steal_dist);
                return;
            }
        }
        if c.steal_phase == StealPhase::Snatch {
            let k = (c.t - c.steal_t0) / c.params.snatch_time;
            self.sp.pose_rest(c, p);
            p.eyes = Eyes::Wide;
            c.pointer = Some(Pointer::Pull(clamp(k, 0.0, 1.0)));
            if k >= 1.0 {
                c.steal_phase = StealPhase::Carry;
                c.steal_t0 = c.t;
                c.steal_turn = c.t + c.uniform(0.8, 2.0);
            }
            return;
        }
        // carry: run around with it until the time is up
        if c.t - c.steal_t0 >= c.duration {
            let line = c.choice(c.params.return_lines);
            c.say(line, 1.6, false);
            self.sp.pose_rest(c, p);
            c.go_idle();
            return;
        }
        if c.t >= c.steal_turn {
            // zig-zag to keep you guessing
            c.facing = -c.facing;
            c.steal_turn = c.t + c.uniform(0.8, 2.5);
        }
        let step = c.params.steal_speed * dt;
        c.step_x(b, c.facing * step);
        c.steal_dist += step;
        self.sp.pose_run(c, p, c.steal_dist);
        c.pointer = Some(Pointer::Hold);
        if c.random() < dt * 0.15 {
            let line = c.choice(c.params.steal_lines);
            c.say(line, 1.0, false);
        }
    }

    /// Fly in an arc to another spot on the floor, then land.
    fn fly(&mut self, b: &Bounds, p: &mut S::Pose) {
        let c = &mut self.core;
        if c.entered() {
            let lo = b.left.max(c.x - c.params.fly_range);
            let hi = b.right.min(c.x + c.params.fly_range);
            let mut target = c.x;
            for _ in 0..8 {
                // somewhere at least a little way off
                target = c.uniform(lo, hi);
                if (target - c.x).abs() > 150.0_f64.min((hi - lo) / 3.0) {
                    break;
                }
            }
            c.fly_from = c.x;
            c.fly_to = target;
            let (h0, h1) = c.params.fly_height;
            c.fly_h = c.uniform(h0, h1).min((b.floor - b.top).max(0.0));
            c.duration = 0.8_f64.max((target - c.x).abs() / c.params.fly_speed);
            if target != c.x {
                c.facing = if target > c.x { 1.0 } else { -1.0 };
            }
        }
        let prog = clamp(c.t / c.duration, 0.0, 1.0);
        c.x = lerp(c.fly_from, c.fly_to, ease(prog) * 0.3 + prog * 0.7);
        c.y = b.floor - c.fly_h * (PI * prog).sin();
        p.grounded = false;
        self.sp.pose_flight(c, p, prog);
        if prog >= 1.0 {
            c.y = b.floor;
            c.go("land", 0.2);
        }
    }

    // ----------------------------------------------------------- face
    fn face(&mut self, dt: f64, p: &mut S::Pose) {
        let c = &mut self.core;
        let mut target = (0.0, 0.0);
        if let Some((dx, dy)) = c.cursor {
            if matches!(p.eyes, Eyes::Open | Eyes::Wide) {
                let d = dx.hypot(dy);
                let d = if d == 0.0 { 1.0 } else { d };
                let reach = (d / 80.0).min(1.0);
                target = (dx / d * reach, dy / d * reach);
            }
        }
        c.look = lerp2(c.look, target, clamp(dt * 10.0, 0.0, 1.0));
        p.look = c.look;

        if c.clock >= c.next_blink {
            c.blink_until = c.clock + 0.13;
            c.next_blink = c.clock + c.uniform(2.0, 6.0);
        }
        if c.clock < c.blink_until && p.eyes == Eyes::Open {
            p.eyes = Eyes::Closed;
        }
        if let Some(text) = &c.bubble {
            if c.clock < c.bubble_until {
                p.bubble = Some(text.clone());
            }
        }
    }
}

// ------------------------------------------------------------------ type-erased mascots

/// What the windows, the herd and the tools need from any mascot.
pub trait Mascot {
    fn core(&self) -> &Core;
    fn core_mut(&mut self) -> &mut Core;
    fn update(&mut self, dt: f64, b: &Bounds, cursor: Option<(f64, f64)>);
    fn poke(&mut self);
    fn paint(&self, p: &mut Painter, head_only: bool);
    fn pose(&self) -> &BasePose;
    /// paint the default pose (tray / menu icons)
    fn paint_icon(&self, p: &mut Painter);
}

impl<S: Species> Mascot for Brain<S> {
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn update(&mut self, dt: f64, b: &Bounds, cursor: Option<(f64, f64)>) {
        Brain::update(self, dt, b, cursor);
    }
    fn poke(&mut self) {
        self.sp.poke(&mut self.core);
    }
    fn paint(&self, p: &mut Painter, head_only: bool) {
        self.sp.paint(p, &self.pose, self.core.facing, head_only);
    }
    fn pose(&self) -> &BasePose {
        &self.pose
    }
    fn paint_icon(&self, p: &mut Painter) {
        self.sp.paint(p, &S::Pose::default(), 1.0, true);
    }
}

/// A preview scene for tools/preview: (label, state, duration, at).
pub type Preview = (&'static str, &'static str, f64, f64);

/// Static description of a mascot kind (the Python `Character`).
pub struct Character {
    pub name: &'static str,
    pub half_width: f64, // feet -> furthest pixel sideways (screen-edge collisions)
    pub height: f64,     // feet -> top of head
    pub eye_y: f64,      // feet -> eyes (the gaze follows the cursor from here)
    pub icon_box: (f64, f64, f64), // (cx, cy, extent) framed by the tray icon
    pub size: (u32, u32), // window size at scale 1 (room for a wrapped bubble)
    pub previews: &'static [Preview],
    pub grip: (f64, f64), // where it holds a stolen pointer (mouth/beak), from the feet
    /// the species' constants (lines, choices, ...)
    pub params: &'static Params,
    pub make: fn(x: f64, y: f64, seed: Option<u64>) -> Box<dyn Mascot>,
}

impl Character {
    pub const DEFAULT_SIZE: (u32, u32) = (240, 226);
    pub const DEFAULT_GRIP: (f64, f64) = (10.0, -40.0);

    pub fn brain(&self, x: f64, y: f64, seed: Option<u64>) -> Box<dyn Mascot> {
        (self.make)(x, y, seed)
    }
}
