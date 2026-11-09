//! Shared four-legged rig: pose, gaits, common behaviours and the IK painter.
//! Port of `quadruped/{brain,art}.py`.
//!
//! A `QuadPose` doesn't store joint angles. It says where the shoulders and hips are
//! and where each foot should be; the painter solves the knees with two-bone IK. So
//! "sit" is just "drop the hips and tuck the hind feet forward", and the legs fold
//! correctly for every species and size.
//!
//! A species is a `QuadKind` (behaviour: the Python `QuadBrain` subclass) that is also a
//! `QuadArt` (looks: the `QuadArt` subclass); `Quad<K>` turns it into an engine `Species`.

use std::cell::Cell;
use std::f64::consts::PI;

use crate::art::{bubble, paint_stars, paint_zzz, pen, tapered_curve};
use crate::engine::{ease, lerp, lerp2, Bounds, Core, Eyes, Params, Species};
use crate::gfx::{pt, Color, Painter, Path, Pt, Xf};
use crate::pose;

pub const FN: usize = 0; // front near
pub const FF: usize = 1; // front far
pub const HN: usize = 2; // hind near
pub const HF: usize = 3; // hind far

/// Where a foot goes: dx from the joint's rest x, lift, and whether it hangs from the joint.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Foot {
    pub dx: f64,
    pub lift: f64,
    pub hang: bool,
}

pub const fn foot(dx: f64, lift: f64, hang: bool) -> Foot {
    Foot { dx, lift, hang }
}

pub const STAND: [Foot; 4] = [foot(0.0, 0.0, false); 4];

#[derive(Clone, Copy, Debug)]
pub struct Rig {
    pub shoulder: (f64, f64), // rest joint at the top of the front legs (feet at y=0)
    pub hip: (f64, f64),
    pub front_leg: (f64, f64), // (upper, lower) bone lengths
    pub hind_leg: (f64, f64),
    pub head: (f64, f64), // rest head centre
    pub head_angle: f64,  // rest head angle, positive tips the nose down
    pub neck_base: (f64, f64), // from the shoulder
    pub tail_root: (f64, f64), // from the hip
    pub foot_y: f64,      // where the foot target sits when planted (paw thickness)
}

impl Rig {
    /// Defaults for the optional fields; use with struct-update syntax.
    pub const BASE: Rig = Rig {
        shoulder: (0.0, 0.0),
        hip: (0.0, 0.0),
        front_leg: (10.0, 10.0),
        hind_leg: (10.0, 10.0),
        head: (0.0, 0.0),
        head_angle: 0.0,
        neck_base: (2.0, -6.0),
        tail_root: (-6.0, -4.0),
        foot_y: 0.0,
    };
}

pose!(QuadPose {
    shoulder: (f64, f64) = (0.0, 0.0), // offsets from the rig's rest joints
    hip: (f64, f64) = (0.0, 0.0),
    feet: [Foot; 4] = STAND,
    head: (f64, f64) = (0.0, 0.0),     // offset from the rig's rest head
    head_angle: f64 = 0.0,             // offset from the rig's rest angle
    mouth: f64 = 0.0,                  // 0 closed .. 1 wide open
    tongue: bool = false,
    ears: f64 = 0.0,                   // -1 pinned back .. 0 relaxed .. 1 perked
    tail: f64 = 0.0,                   // angle offset, degrees, positive = up
    tail_wag: f64 = 0.0,               // sideways swish, degrees
    tail_curl: f64 = 0.0,              // 0 straight .. 1 hooked up at the tip
    zzz: Option<f64> = None,           // asleep: phase of the floating z's
    action: f64 = 0.0,                 // species-specific extra
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Walk,
    Trot,
    Gallop,
}

/// The Python QuadBrain class constants.
#[derive(Clone, Copy, Debug)]
pub struct QuadParams {
    pub rig: Rig,
    pub stride: f64, // foot swing amplitude when walking, px
    pub lift: f64,   // foot lift when walking, px
    pub run_speed: f64,
    pub run_gait: Gait,
}

impl QuadParams {
    pub const BASE: QuadParams =
        QuadParams { rig: Rig::BASE, stride: 8.0, lift: 4.0, run_speed: 200.0, run_gait: Gait::Gallop };
}

// =================================================================== behaviour

/// The gait phase the Python brain keeps in `self._phase`.
#[derive(Default)]
pub struct Phase(Cell<f64>);

impl Phase {
    pub fn get(&self) -> f64 {
        self.0.get()
    }
    pub fn set(&self, v: f64) {
        self.0.set(v);
    }
}

/// Behaviour of one four-legged species (the Python QuadBrain subclass).
/// Every hook defaults to the shared QuadBrain behaviour (the `default_*` functions).
pub trait QuadKind: QuadArt + 'static {
    const PARAMS: Params;
    const QUAD: QuadParams;

    fn phase(&self) -> &Phase;

    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        breathe(c, p);
    }
    fn pose_walk(&self, c: &Core, p: &mut QuadPose, t: f64) {
        default_pose_walk(self, c, p, t);
    }
    fn pose_crouch(&self, c: &Core, p: &mut QuadPose, k: f64) {
        breathe(c, p);
        p.shoulder = (0.0, 5.0 * k);
        p.hip = (0.0, 5.0 * k);
        p.ears = k;
    }
    fn pose_jump(&self, c: &Core, p: &mut QuadPose) {
        hang(c, p, 0.0, 1.0);
        p.tail = 15.0;
        p.ears = 1.0;
    }
    fn pose_fall(&self, c: &Core, p: &mut QuadPose) {
        hang(c, p, 6.0, 0.0);
        p.mouth = 0.6;
        p.ears = -1.0;
        p.tail = 20.0 * (c.clock * 9.0).sin();
    }
    fn pose_land(&self, c: &Core, p: &mut QuadPose, bend: f64) {
        breathe(c, p);
        p.shoulder = (0.0, 7.0 * bend);
        p.hip = (0.0, 7.0 * bend);
        p.feet = [
            foot(4.0 * bend, 0.0, false),
            foot(2.0 * bend, 0.0, false),
            foot(-4.0 * bend, 0.0, false),
            foot(-2.0 * bend, 0.0, false),
        ];
    }
    fn pose_dizzy(&self, c: &Core, p: &mut QuadPose) {
        let c = c.clock;
        p.head = (2.0 * (c * 4.0).cos(), 2.0 * (c * 4.0).sin());
        p.head_angle = 10.0 * (c * 4.0).sin();
        p.ears = -0.5;
        p.tail = -20.0;
    }
    fn pose_dragged(&self, c: &Core, p: &mut QuadPose) {
        default_pose_dragged(c, p);
    }
    fn pose_run(&self, _c: &Core, p: &mut QuadPose, travelled: f64) {
        let q = &Self::QUAD;
        let (stride, lift) = (q.stride * 1.6, q.lift * 1.8);
        self.phase().set(travelled / stride);
        gait(p, self.phase().get(), stride, lift, q.run_gait);
        p.ears = -0.6;
        p.tail = 10.0;
        p.lean += 3.0;
    }
    fn poke(&mut self, c: &mut Core) {
        c.default_poke();
    }
    /// The species' own states (and overrides of the shared quad states).
    fn state(&mut self, _c: &mut Core, _dt: f64, _b: &Bounds, _p: &mut QuadPose) -> bool {
        false
    }
}

pub fn default_pose_walk<K: QuadKind + ?Sized>(k: &K, c: &Core, p: &mut QuadPose, t: f64) {
    let _ = c;
    let q = &K::QUAD;
    k.phase().set(t * K::PARAMS.walk_speed / q.stride);
    gait(p, k.phase().get(), q.stride, q.lift, Gait::Walk);
    p.tail_wag = 8.0 * k.phase().get().sin();
}

pub fn default_pose_dragged(c: &Core, p: &mut QuadPose) {
    hang(c, p, 3.0, 0.0);
    p.ears = -1.0;
    p.tail = -35.0;
    p.head_angle = -10.0;
}

pub fn breathe(c: &Core, p: &mut QuadPose) {
    let s = (c.clock * 2.0).sin();
    p.shoulder = (0.0, -0.5 * s);
    p.hip = (0.0, -0.3 * s);
    p.tail_wag = 5.0 * (c.clock * 1.4).sin();
}

pub fn gait(p: &mut QuadPose, phase: f64, stride: f64, lift: f64, kind: Gait) {
    let offsets = match kind {
        Gait::Walk => [0.0, PI, PI * 1.5, PI * 0.5],
        Gait::Trot => [0.0, PI, PI, 0.0],
        Gait::Gallop => [0.0, 0.7, PI, PI + 0.7],
    };
    for (f, o) in p.feet.iter_mut().zip(offsets) {
        *f = foot(stride * (phase + o).sin(), lift * (phase + o).cos().max(0.0), false);
    }
    if kind == Gait::Gallop {
        // rocking-horse seesaw
        p.shoulder = (0.0, -lift * 0.5 * phase.sin());
        p.hip = (0.0, lift * 0.5 * phase.sin());
        p.lean = -3.0 * phase.sin();
    } else {
        let m = if kind == Gait::Walk { 2.0 } else { 1.0 };
        let bob = -lift * 0.3 * (phase * m).cos().abs();
        p.shoulder = (0.0, bob);
        p.hip = (0.0, bob);
    }
}

/// Walk/run at speed with a gait whose feet don't slide on the floor.
#[allow(clippy::too_many_arguments)]
pub fn move_<K: QuadKind + ?Sized>(
    k: &K,
    c: &mut Core,
    p: &mut QuadPose,
    b: &Bounds,
    dt: f64,
    speed: f64,
    kind: Gait,
    stride: Option<f64>,
    lift: Option<f64>,
) {
    let q = &K::QUAD;
    let stride = stride.unwrap_or(q.stride * if kind != Gait::Walk { 1.6 } else { 1.0 });
    let lift = lift.unwrap_or(q.lift * if kind != Gait::Walk { 1.8 } else { 1.0 });
    c.step_x(b, c.facing * speed * dt);
    k.phase().set(k.phase().get() + speed / stride * dt);
    gait(p, k.phase().get(), stride, lift, kind);
}

pub fn sit(r: &Rig, p: &mut QuadPose, k: f64) {
    let drop = 0.62 * -r.hip.1;
    p.hip = lerp2(p.hip, (6.0, drop), k);
    p.shoulder = lerp2(p.shoulder, (0.0, -2.0), k);
    p.feet[HN] = foot(8.0 * k, 0.0, false);
    p.feet[HF] = foot(6.0 * k, 0.0, false);
    p.head = lerp2(p.head, (-2.0, -3.0), k);
    p.tail = lerp(p.tail, -25.0, k);
}

pub fn lie(r: &Rig, p: &mut QuadPose, k: f64) {
    p.shoulder = lerp2(p.shoulder, (3.0, 0.68 * -r.shoulder.1), k);
    p.hip = lerp2(p.hip, (4.0, 0.66 * -r.hip.1), k);
    let reach = (r.front_leg.0 + r.front_leg.1) * 0.75;
    p.feet = [
        foot(reach * k, 0.0, false),
        foot(reach * 0.8 * k, 0.0, false),
        foot(10.0 * k, 0.0, false),
        foot(8.0 * k, 0.0, false),
    ];
    p.head = lerp2(p.head, (3.0, 0.55 * -r.shoulder.1), k);
    p.tail = lerp(p.tail, -30.0, k);
}

/// Play bow / stretch: chest down, rear up.
pub fn bow(r: &Rig, p: &mut QuadPose, k: f64) {
    p.shoulder = lerp2(p.shoulder, (2.0, 0.5 * -r.shoulder.1), k);
    p.hip = lerp2(p.hip, (0.0, -2.0), k);
    let reach = (r.front_leg.0 + r.front_leg.1) * 0.6;
    p.feet[FN] = foot(reach * k, 0.0, false);
    p.feet[FF] = foot(reach * 0.85 * k, 0.0, false);
    p.head = lerp2(p.head, (6.0, 0.45 * -r.shoulder.1), k);
    p.tail = lerp(p.tail, 25.0, k);
}

/// Feet dangle from the joints (airborne, dragged).
pub fn hang(c: &Core, p: &mut QuadPose, kick: f64, tuck: f64) {
    let cl = c.clock;
    for (i, f) in p.feet.iter_mut().enumerate() {
        let w = (cl * 11.0 + i as f64 * 1.7).sin();
        let side = if i < 2 { 4.0 } else { -4.0 };
        *f = foot(kick * w + side * tuck, tuck * 8.0 + kick.abs() * 0.3 * (1.0 + w), true);
    }
}

pub fn head_to_ground(r: &Rig, p: &mut QuadPose, k: f64, forward: f64, nose_down: f64) {
    p.head = lerp2(p.head, (forward, -r.head.1 - 9.0), k);
    p.head_angle = lerp(p.head_angle, nose_down, k);
}

/// Creep low toward the cursor; returns the remaining horizontal distance.
pub fn stalk<K: QuadKind + ?Sized>(k: &K, c: &mut Core, p: &mut QuadPose, b: &Bounds, dt: f64, speed: f64) -> f64 {
    let dx = c.cursor.map_or(0.0, |cur| cur.0);
    if dx.abs() > 4.0 {
        c.facing = if dx > 0.0 { 1.0 } else { -1.0 };
    }
    let r = &K::QUAD.rig;
    let speed = if dx.abs() > 8.0 { speed } else { 0.0 };
    move_(k, c, p, b, dt, speed, Gait::Walk, None, Some(K::QUAD.lift * 0.6));
    p.shoulder = (p.shoulder.0, p.shoulder.1 + 0.35 * -r.shoulder.1);
    p.hip = (p.hip.0, p.hip.1 + 0.3 * -r.hip.1);
    p.head = (4.0, 0.3 * -r.shoulder.1);
    p.head_angle = -5.0;
    p.ears = 1.0;
    p.eyes = Eyes::Wide;
    p.tail = -10.0;
    dx.abs()
}

// ----------------------------------------------------------- shared states

/// Zoomies: a fast run, bouncing off the screen edges.
pub fn run_state<K: QuadKind + ?Sized>(k: &K, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) {
    if c.paused || c.done() {
        k.pose_rest(c, p);
        c.go_idle();
        return;
    }
    let ramp = ease(c.t / 0.4) * ease((c.duration - c.t) / 0.4);
    let q = &K::QUAD;
    move_(k, c, p, b, dt, q.run_speed * ramp.max(0.25), q.run_gait, None, None);
    p.ears = -0.6;
    p.tail = 10.0;
    p.lean += 3.0;
}

pub fn sit_state<K: QuadKind + ?Sized>(_k: &K, c: &mut Core, p: &mut QuadPose) {
    breathe(c, p);
    sit(&K::QUAD.rig, p, ease(c.t / 0.4) * ease((c.duration - c.t) / 0.4));
    if c.done() {
        c.go_idle();
    }
}

pub fn nap_state<K: QuadKind + ?Sized>(_k: &K, c: &mut Core, p: &mut QuadPose) {
    breathe(c, p);
    let k = ease(c.t / 0.6) * ease((c.duration - c.t) / 0.5);
    lie(&K::QUAD.rig, p, k);
    if k > 0.95 {
        p.eyes = Eyes::Closed;
        p.zzz = Some(c.clock);
        p.ears = -0.3;
    }
    if c.done() {
        c.go_idle();
    }
}

pub fn sniff_state<K: QuadKind + ?Sized>(k: &K, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) {
    if c.paused || c.done() {
        k.pose_rest(c, p);
        c.go_idle();
        return;
    }
    let e = ease(c.t / 0.35) * ease((c.duration - c.t) / 0.35);
    let speed = K::PARAMS.walk_speed * 0.35 * e;
    move_(k, c, p, b, dt, speed, Gait::Walk, None, None); // nose to the floor, pottering
    head_to_ground(&K::QUAD.rig, p, e, 8.0, 45.0);
    p.head = (p.head.0 + 1.5 * (c.clock * 25.0).sin() * e, p.head.1);
    p.tail_wag = 10.0 * (c.clock * 6.0).sin();
}

/// Turns a `QuadKind` into an engine `Species`.
pub struct Quad<K: QuadKind>(pub K);

impl<K: QuadKind> Species for Quad<K> {
    type Pose = QuadPose;
    const PARAMS: Params = K::PARAMS;

    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        self.0.pose_rest(c, p);
    }
    fn pose_walk(&self, c: &Core, p: &mut QuadPose, t: f64) {
        self.0.pose_walk(c, p, t);
    }
    fn pose_crouch(&self, c: &Core, p: &mut QuadPose, k: f64) {
        self.0.pose_crouch(c, p, k);
    }
    fn pose_jump(&self, c: &Core, p: &mut QuadPose) {
        self.0.pose_jump(c, p);
    }
    fn pose_fall(&self, c: &Core, p: &mut QuadPose) {
        self.0.pose_fall(c, p);
    }
    fn pose_land(&self, c: &Core, p: &mut QuadPose, bend: f64) {
        self.0.pose_land(c, p, bend);
    }
    fn pose_dizzy(&self, c: &Core, p: &mut QuadPose) {
        self.0.pose_dizzy(c, p);
    }
    fn pose_dragged(&self, c: &Core, p: &mut QuadPose) {
        self.0.pose_dragged(c, p);
    }
    fn pose_run(&self, c: &Core, p: &mut QuadPose, travelled: f64) {
        self.0.pose_run(c, p, travelled);
    }
    fn poke(&mut self, c: &mut Core) {
        self.0.poke(c);
    }
    fn state(&mut self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) -> bool {
        if self.0.state(c, dt, b, p) {
            return true;
        }
        match c.state {
            "run" => run_state(&self.0, c, dt, b, p),
            "sit" => sit_state(&self.0, c, p),
            "nap" => nap_state(&self.0, c, p),
            "sniff" => sniff_state(&self.0, c, dt, b, p),
            _ => return false,
        }
        true
    }
    fn paint(&self, p: &mut Painter, pose: &QuadPose, facing: f64, head_only: bool) {
        paint(&self.0, K::QUAD.rig, p, pose, facing, head_only);
    }
}

// =================================================================== art

/// Where everything ended up this frame (body coordinates).
pub struct Geometry {
    pub shoulder: Pt,
    pub hip: Pt,
    pub neck_base: Pt,
    pub head: Pt,
    pub head_angle: f64,
    pub tail_root: Pt,
    pub legs: [(Pt, Pt, Pt); 4], // per foot: (joint, knee, foot)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FootKind {
    Paw,
    Hoof,
}

/// The Python QuadArt class attributes.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub coat: Color,
    pub shade: Color, // far-side legs
    pub outline: Color,
    pub chest_r: f64,
    pub hip_r: f64,
    pub neck_w: (f64, f64),
    pub front_w: (f64, f64, f64), // leg widths: top, knee, foot
    pub hind_w: (f64, f64, f64),
    pub foot: FootKind,
    pub hoof_color: Color,
    pub line_w: f64,
    pub far_dx: f64, // far legs sit a little behind/ahead for depth
}

impl Style {
    pub const BASE: Style = Style {
        coat: Color::hex("#888888"),
        shade: Color::hex("#666666"),
        outline: Color::hex("#222222"),
        chest_r: 10.0,
        hip_r: 9.0,
        neck_w: (14.0, 10.0),
        front_w: (8.0, 5.0, 4.0),
        hind_w: (9.0, 5.0, 4.0),
        foot: FootKind::Paw,
        hoof_color: Color::hex("#2A2420"),
        line_w: 1.6,
        far_dx: 3.0,
    };
}

/// Two-bone IK: returns (knee, reachable foot). bend -1 = knee forward, +1 = backward.
pub fn ik(joint: Pt, target: Pt, a: f64, b: f64, bend: f64) -> (Pt, Pt) {
    let (dx, dy) = (target.x - joint.x, target.y - joint.y);
    let d0 = dx.hypot(dy);
    let d0 = if d0 == 0.0 { 1e-6 } else { d0 };
    let d = d0.max((a - b).abs() + 0.01).min(a + b - 0.01);
    let (ux, uy) = (dx / d0, dy / d0);
    let foot = pt(joint.x + ux * d, joint.y + uy * d);
    let theta = uy.atan2(ux);
    let alpha = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0).acos();
    let knee = pt(joint.x + a * (theta + bend * alpha).cos(), joint.y + a * (theta + bend * alpha).sin());
    (knee, foot)
}

/// A straight tapered segment (bones, muzzles).
pub fn straight(a: Pt, b: Pt, w0: f64, w1: f64) -> Path {
    let p0 = (a.x, a.y);
    let p3 = (b.x, b.y);
    let p1 = (p0.0 + (p3.0 - p0.0) / 3.0, p0.1 + (p3.1 - p0.1) / 3.0);
    let p2 = (p0.0 + (p3.0 - p0.0) * 2.0 / 3.0, p0.1 + (p3.1 - p0.1) * 2.0 / 3.0);
    tapered_curve(p0, p1, p2, p3, w0, w1, 4, 1.0)
}

pub fn circle(c: Pt, r: f64) -> Path {
    let mut path = Path::new();
    path.add_ellipse(c, r, r);
    path
}

pub fn ellipse(c: Pt, rx: f64, ry: f64) -> Path {
    let mut path = Path::new();
    path.add_ellipse(c, rx, ry);
    path
}

pub fn polygon(pts: &[Pt]) -> Path {
    let mut path = Path::new();
    path.add_polygon(pts);
    path.close();
    path
}

/// Looks of one four-legged species (the Python QuadArt subclass).
pub trait QuadArt {
    fn style(&self) -> &Style;
    fn rig(&self) -> &Rig;

    fn geometry(&self, pose: &QuadPose) -> Geometry {
        let r = self.rig();
        let st = self.style();
        let s = pt(r.shoulder.0 + pose.shoulder.0, r.shoulder.1 + pose.shoulder.1);
        let h = pt(r.hip.0 + pose.hip.0, r.hip.1 + pose.hip.1);
        let mut legs = [(Pt::default(), Pt::default(), Pt::default()); 4];
        for (i, f) in pose.feet.iter().enumerate() {
            let front = i == FN || i == FF;
            let base = if front { s } else { h };
            let off = if i == FF { st.far_dx } else if i == HF { -st.far_dx } else { 0.0 };
            let joint = pt(base.x + off, base.y);
            let (a, b) = if front { r.front_leg } else { r.hind_leg };
            let rest_x = (if front { r.shoulder } else { r.hip }).0 + (joint.x - base.x);
            let target = if f.hang {
                pt(joint.x + f.dx, joint.y + (a + b) * 0.97 - f.lift)
            } else {
                pt(rest_x + f.dx, r.foot_y - f.lift)
            };
            let (knee, ft) = ik(joint, target, a, b, if front { -1.0 } else { 1.0 });
            legs[i] = (joint, knee, ft);
        }
        Geometry {
            shoulder: s,
            hip: h,
            neck_base: pt(s.x + r.neck_base.0, s.y + r.neck_base.1),
            head: pt(r.head.0 + pose.head.0, r.head.1 + pose.head.1),
            head_angle: r.head_angle + pose.head_angle,
            tail_root: pt(h.x + r.tail_root.0, h.y + r.tail_root.1),
            legs,
        }
    }

    fn head_transform(&self, g: &Geometry) -> Xf {
        Xf::new().translate(g.head.x, g.head.y).rotate(g.head_angle)
    }

    fn body_shape(&self, g: &Geometry) -> Path {
        let st = self.style();
        straight(g.hip, g.shoulder, 2.0 * st.hip_r, 2.0 * st.chest_r)
            .united(&circle(g.hip, st.hip_r))
            .united(&circle(g.shoulder, st.chest_r))
    }

    /// Bezier control points of the neck, neck base -> head.
    fn neck_spine(&self, g: &Geometry) -> [(f64, f64); 4] {
        let (nb, hd) = (g.neck_base, g.head);
        [
            (nb.x, nb.y),
            (nb.x + (hd.x - nb.x) * 0.25, nb.y + (hd.y - nb.y) * 0.55),
            (hd.x - (hd.x - nb.x) * 0.3, hd.y + (nb.y - hd.y) * 0.2),
            (hd.x, hd.y),
        ]
    }

    fn neck_shape(&self, g: &Geometry) -> Path {
        let [a, b, c, d] = self.neck_spine(g);
        let (w0, w1) = self.style().neck_w;
        tapered_curve(a, b, c, d, w0, w1, 16, 1.6)
    }

    fn leg_shape(&self, leg: (Pt, Pt, Pt), widths: (f64, f64, f64)) -> Path {
        let (joint, knee, ft) = leg;
        let (top, mid, low) = widths;
        straight(joint, knee, top, mid).united(&straight(knee, ft, mid, low)).united(&circle(knee, mid / 2.0))
    }

    fn foot_shape(&self, f: Pt) -> Path {
        let st = self.style();
        match st.foot {
            FootKind::Hoof => polygon(&[
                pt(f.x - 3.5, f.y - 5.0),
                pt(f.x + 3.5, f.y - 5.0),
                pt(f.x + 5.0, f.y),
                pt(f.x - 4.0, f.y),
            ]),
            FootKind::Paw => ellipse(pt(f.x + 1.5, f.y - 0.2), st.front_w.2 * 0.75, st.front_w.2 * 0.48),
        }
    }

    // ----------------------------------------------------------- species hooks
    fn head_shape(&self, _pose: &QuadPose, g: &Geometry) -> Path {
        self.head_transform(g).map_path(&circle(pt(0.0, 0.0), 8.0))
    }
    fn paint_tail(&self, _p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {}
    /// Called clipped to the silhouette (patches, stripes, socks).
    fn paint_markings(&self, _p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {}
    /// Called clipped to the far legs.
    fn paint_far_markings(&self, _p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {}
    /// After the outline (manes, collars).
    fn paint_over(&self, _p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {}
    fn paint_head(&self, _p: &mut Painter, _pose: &QuadPose, _g: &Geometry, _facing: f64) {}

    /// Eye in head-local coordinates (painter already rotated to the head).
    #[allow(clippy::too_many_arguments)]
    fn paint_eye(
        &self,
        p: &mut Painter,
        pose: &QuadPose,
        at: Pt,
        facing: f64,
        r: f64,
        iris: Option<Color>,
        slit: bool,
        light: Option<Color>,
    ) {
        default_paint_eye(self.style().outline, p, pose, at, facing, r, iris, slit, light);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn default_paint_eye(
    outline: Color,
    p: &mut Painter,
    pose: &QuadPose,
    at: Pt,
    facing: f64,
    r: f64,
    iris: Option<Color>,
    slit: bool,
    light: Option<Color>,
) {
    let (lx, ly) = (pose.look.0 * facing, pose.look.1);
    let stroke = light.unwrap_or(outline);
    if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
        let wide = pose.eyes == Eyes::Wide;
        let rr = r * if wide { 1.25 } else { 1.0 };
        if let Some(iris) = iris {
            p.set_pen(pen(outline, 0.9));
            p.set_brush(iris);
            p.draw_ellipse(at, rr * 1.15, rr);
        }
        let c = pt(at.x + lx * r * 0.35, at.y + ly * r * 0.3);
        p.no_pen();
        p.set_brush(Color::hex("#141010"));
        if slit {
            let w = rr * if wide { 0.75 } else { 0.32 };
            p.draw_ellipse(c, w, rr * 0.95);
        } else {
            let k = if iris.is_some() { 0.6 } else { 1.0 };
            p.draw_circle(c, rr * k);
        }
        p.set_brush(Color::hex("#FFFFFF"));
        p.draw_circle(pt(c.x - rr * 0.3, c.y - rr * 0.35), rr * 0.3);
    } else {
        let mut path = Path::new();
        let (x, y, w) = (at.x, at.y, r * 1.3);
        match pose.eyes {
            Eyes::Closed => {
                path.move_to(x - w, y);
                path.quad_to(x, y + w * 0.8, x + w, y);
            }
            Eyes::Happy => {
                path.move_to(x - w, y + w * 0.4);
                path.quad_to(x, y - w, x + w, y + w * 0.4);
            }
            _ => {
                path.move_to(x - w * 0.8, y - w * 0.8);
                path.line_to(x + w * 0.8, y + w * 0.8);
                path.move_to(x + w * 0.8, y - w * 0.8);
                path.line_to(x - w * 0.8, y + w * 0.8);
            }
        }
        p.no_brush();
        p.set_pen(pen(stroke, 1.4));
        p.draw_path(&path);
    }
}

fn paint_hooves<A: QuadArt + ?Sized>(a: &A, p: &mut Painter, g: &Geometry, which: [usize; 2], color: Color) {
    for i in which {
        p.set_pen(pen(a.style().outline, 1.2));
        p.set_brush(color);
        p.draw_path(&a.foot_shape(g.legs[i].2));
    }
}

/// QuadArt.paint: far legs, silhouette (neck + head + body + near legs), markings, head.
pub fn paint<A: QuadArt + ?Sized>(a: &A, _rig: Rig, p: &mut Painter, pose: &QuadPose, facing: f64, head_only: bool) {
    let st = *a.style();
    p.save();
    let g = a.geometry(pose);
    p.translate(0.0, pose.bob);
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash);
    p.scale(facing, 1.0);
    if pose.swing != 0.0 {
        let pivot = pt(g.shoulder.x - 4.0, g.shoulder.y - st.chest_r); // the scruff
        p.translate_pt(pivot);
        p.rotate(pose.swing);
        p.translate_pt(-pivot);
    }
    p.rotate(pose.lean);

    let mut sil = a.neck_shape(&g).united(&a.head_shape(pose, &g));
    if !head_only {
        a.paint_tail(p, pose, &g);
        let mut far = Path::new();
        for i in [HF, FF] {
            far = far.united(&a.leg_shape(g.legs[i], if i == FF { st.front_w } else { st.hind_w }));
            if st.foot == FootKind::Paw {
                far = far.united(&a.foot_shape(g.legs[i].2));
            }
        }
        p.no_pen();
        p.set_brush(st.shade);
        p.draw_path(&far);
        p.save();
        p.set_clip_path(&far);
        a.paint_far_markings(p, pose, &g);
        p.restore();
        p.set_pen(pen(st.outline, st.line_w));
        p.no_brush();
        p.draw_path(&far);
        if st.foot == FootKind::Hoof {
            paint_hooves(a, p, &g, [HF, FF], st.hoof_color.darker(115.0));
        }
        sil = sil.united(&a.body_shape(&g));
        for i in [HN, FN] {
            sil = sil.united(&a.leg_shape(g.legs[i], if i == FN { st.front_w } else { st.hind_w }));
            if st.foot == FootKind::Paw {
                sil = sil.united(&a.foot_shape(g.legs[i].2));
            }
        }
    }

    p.no_pen();
    p.set_brush(st.coat);
    p.draw_path(&sil);
    p.save();
    p.set_clip_path(&sil);
    a.paint_markings(p, pose, &g);
    p.restore();
    p.set_pen(pen(st.outline, st.line_w));
    p.no_brush();
    p.draw_path(&sil);
    if !head_only && st.foot == FootKind::Hoof {
        paint_hooves(a, p, &g, [HN, FN], st.hoof_color);
    }
    if !head_only {
        a.paint_over(p, pose, &g);
    }
    a.paint_head(p, pose, &g, facing);
    if let Some(stars) = pose.stars {
        paint_stars(p, g.head.x, g.head.y - 18.0, stars, 17.0, 4.0);
    }
    if let (Some(z), false) = (pose.zzz, head_only) {
        paint_zzz(p, g.head.x + 4.0, g.head.y - 12.0, z, facing, "zzz", crate::art::OUTLINE);
    }
    p.restore();

    if let (Some(text), false) = (&pose.bubble, head_only) {
        let top = g.head.y.min(g.shoulder.y) - 16.0;
        bubble(p, text, facing * g.head.x, top + pose.bob, facing, st.outline);
    }
}
