//! Monkey: walks, scratches its head, eats bananas, hops.
//! Port of `characters/monkey/{brain,art,__init__}.py`.

use crate::art::{blob, bubble, paint_stars, pen, stroke, OUTLINE, WHITE};
use crate::engine::{bump, ease, lerp, Bounds, Brain, Character, Core, Eyes, Params, Species};
use crate::gfx::{pt, Color, FillRule, Painter, Path, Pt};
use crate::pose;

pub const EAT_TIME: f64 = 5.2;
const BITES: [f64; 3] = [1.2, 2.3, 3.4];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mouth {
    #[default]
    Smile,
    Big,
    O,
    Chew,
}

pose!(MonkeyPose {
    // limb angles: degrees from hanging straight down, positive toward the facing side;
    // each limb is (upper_segment, lower_relative)
    head_tilt: f64 = 0.0,
    arm_front: (f64, f64) = (12.0, 10.0),
    arm_back: (f64, f64) = (-12.0, -10.0),
    leg_front: (f64, f64) = (3.0, 0.0),
    leg_back: (f64, f64) = (-3.0, 0.0),
    tail: f64 = 0.0,           // sway, -1..1
    mouth: Mouth = Mouth::Smile,
    banana: i32 = -1,          // -1 = no banana, otherwise bites taken (0..3)
});

pub struct Monkey;

impl Species for Monkey {
    type Pose = MonkeyPose;
    const PARAMS: Params = Params {
        steal_lines: &["Ook ook!", "Mine!", "Eek eek!"],
        walk_speed: 75.0,
        lines: &["Ook!", "Ook ook!", "Eek!", "Banana?", "Hi!"],
        choices: &[
            (45.0, "walk", (2.5, 7.0)),
            (14.0, "scratch", (1.8, 3.2)),
            (10.0, "eat", (EAT_TIME, EAT_TIME)),
            (9.0, "crouch", (0.14, 0.14)),
            (22.0, "idle", (1.5, 4.0)),
        ],
        ..Params::BASE
    };

    fn pose_rest(&self, c: &Core, p: &mut MonkeyPose) {
        let s = (c.clock * 2.2).sin();
        p.bob = s * 0.7;
        p.tail = (c.clock * 1.4).sin();
        p.arm_front = (12.0 + 2.0 * s, 10.0);
        p.arm_back = (-12.0 - 2.0 * s, -10.0);
    }

    fn pose_walk(&self, c: &Core, p: &mut MonkeyPose, t: f64) {
        let ph = t * 9.0;
        let (s, co) = (ph.sin(), ph.cos());
        p.leg_front = (24.0 * s, -28.0 * co.max(0.0));
        p.leg_back = (-24.0 * s, -28.0 * (-co).max(0.0));
        p.arm_front = (12.0 - 22.0 * s, 12.0);
        p.arm_back = (-12.0 + 22.0 * s, 12.0);
        p.lean = 4.0;
        p.tail = (c.clock * 5.0).sin() * 0.8;
    }

    fn pose_crouch(&self, c: &Core, p: &mut MonkeyPose, k: f64) {
        self.pose_rest(c, p);
        p.leg_front = (28.0 * k, -56.0 * k);
        p.leg_back = (-28.0 * k, 56.0 * k);
        p.arm_front = (12.0 + 25.0 * k, 10.0);
        p.arm_back = (-12.0 - 25.0 * k, -10.0);
    }

    fn pose_jump(&self, c: &Core, p: &mut MonkeyPose) {
        p.tail = (c.clock * 8.0).sin();
        p.arm_front = (150.0, 15.0);
        p.arm_back = (-150.0, -15.0);
        p.leg_front = (35.0, -70.0);
        p.leg_back = (-35.0, 70.0);
        p.mouth = Mouth::Big;
    }

    fn pose_fall(&self, c: &Core, p: &mut MonkeyPose) {
        let c = c.clock;
        let w = (c * 22.0).sin();
        p.tail = (c * 8.0).sin();
        p.arm_front = (135.0 + 30.0 * w, 15.0);
        p.arm_back = (-135.0 + 30.0 * w, -15.0);
        p.leg_front = (20.0 * (c * 16.0).sin(), -25.0);
        p.leg_back = (-20.0 * (c * 16.0 + 1.3).sin(), 25.0);
        p.mouth = Mouth::O;
    }

    fn pose_land(&self, c: &Core, p: &mut MonkeyPose, bend: f64) {
        self.pose_rest(c, p);
        p.leg_front = (30.0 * bend, -60.0 * bend);
        p.leg_back = (-30.0 * bend, 60.0 * bend);
        p.arm_front = (12.0 + 50.0 * bend, 10.0);
        p.arm_back = (-12.0 - 50.0 * bend, -10.0);
    }

    fn pose_dizzy(&self, c: &Core, p: &mut MonkeyPose) {
        p.mouth = Mouth::O;
        p.head_tilt = 7.0 * (c.clock * 3.2 + 1.2).sin();
    }

    fn pose_dragged(&self, c: &Core, p: &mut MonkeyPose) {
        let c = c.clock;
        p.arm_front = (28.0 + 8.0 * (c * 5.0).sin(), 12.0);
        p.arm_back = (-28.0 - 8.0 * (c * 5.0 + 0.6).sin(), -12.0);
        p.leg_front = (12.0 * (c * 7.0).sin(), -18.0);
        p.leg_back = (-12.0 * (c * 7.0 + 1.6).sin(), 18.0);
        p.mouth = Mouth::O;
        p.tail = (c * 3.0).sin();
    }

    fn state(&mut self, c: &mut Core, _dt: f64, _b: &Bounds, p: &mut MonkeyPose) -> bool {
        match c.state {
            "scratch" => self.scratch(c, p),
            "eat" => self.eat(c, p),
            _ => return false,
        }
        true
    }

    fn paint(&self, p: &mut Painter, pose: &MonkeyPose, facing: f64, head_only: bool) {
        paint(p, pose, facing, head_only);
    }
}

impl Monkey {
    fn scratch(&self, c: &mut Core, p: &mut MonkeyPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.3) * ease((c.duration - c.t) / 0.3);
        let up = (-162.0, -28.0 + 14.0 * (c.clock * 28.0).sin());
        p.arm_back = (lerp(p.arm_back.0, up.0, k), lerp(p.arm_back.1, up.1, k));
        p.head_tilt = -8.0 * k;
        let f = c.t / c.duration;
        if 0.35 < f && f < 0.8 {
            p.eyes = Eyes::Happy;
        }
        if c.done() {
            c.go_idle();
        }
    }

    fn eat(&self, c: &mut Core, p: &mut MonkeyPose) {
        self.pose_rest(c, p);
        let t = c.t;
        let k = ease(t / 0.5) * ease((EAT_TIME - t) / 0.5);
        let bring = BITES.iter().map(|&bt| bump(t, bt, 0.45)).fold(0.0, f64::max);
        p.arm_front = (lerp(p.arm_front.0, 30.0, k), lerp(p.arm_front.1, 130.0, k) + 50.0 * bring);
        let bites = BITES.iter().filter(|&&bt| t > bt).count() as i32;
        let finished = t >= 4.2;
        p.banana = if finished { -1 } else { bites };
        p.head_tilt = 4.0 * bring;
        if bring > 0.35 {
            p.mouth = Mouth::O;
        } else if finished {
            p.mouth = Mouth::Big;
            p.eyes = Eyes::Happy;
        } else if bites > 0 {
            p.mouth = if (c.clock * 16.0).sin() > 0.0 { Mouth::Chew } else { Mouth::Smile };
        }
        if t >= EAT_TIME {
            c.go_idle();
        }
    }
}

// =================================================================== art

const FUR: Color = Color::hex("#8B5A2B");
const FUR_SHADE: Color = Color::hex("#72461F");
const SKIN: Color = Color::hex("#EECDA3");
const MOUTH: Color = Color::hex("#7A2424");
const BANANA: Color = Color::hex("#F6CE3A");
const BANANA_EDGE: Color = Color::hex("#A9800F");
const BANANA_FLESH: Color = Color::hex("#FFF4C9");

const LIMB_W: f64 = 7.0;

// Skeleton
const HEAD_C: Pt = pt(0.0, -80.0);
const HEAD_R: f64 = 22.0;
const NECK: Pt = pt(0.0, -60.0);
const SCRUFF: Pt = pt(0.0, -96.0); // pivot when dangling from the cursor
const SHOULDER_X: f64 = 15.0;
const SHOULDER_Y: f64 = -56.0;
const HIP_X: f64 = 9.0;
const HIP_Y: f64 = -28.0;
const ARM_1: f64 = 15.0;
const ARM_2: f64 = 15.0;
const LEG_1: f64 = 13.0;
const LEG_2: f64 = 13.0;
pub const FACE_DX: f64 = 3.0; // face shifted toward the facing side (3/4 look)

const HALF_WIDTH: f64 = 46.0;
const HEIGHT: f64 = 112.0;
const EYE_Y: f64 = 84.0;

fn limb(start: Pt, (upper, rel): (f64, f64), l1: f64, l2: f64) -> (Pt, Pt) {
    let a1 = upper.to_radians();
    let a2 = (upper + rel).to_radians();
    let mid = pt(start.x + l1 * a1.sin(), start.y + l1 * a1.cos());
    let end = pt(mid.x + l2 * a2.sin(), mid.y + l2 * a2.cos());
    (mid, end)
}

fn leg_drop((upper, rel): (f64, f64)) -> f64 {
    LEG_1 * upper.to_radians().cos() + LEG_2 * (upper + rel).to_radians().cos()
}

fn limb_path(start: Pt, mid: Pt, end: Pt) -> Path {
    let mut path = Path::at(start.x, start.y);
    path.line_pt(mid);
    path.line_pt(end);
    path
}

pub fn paint(p: &mut Painter, pose: &MonkeyPose, facing: f64, head_only: bool) {
    p.save();
    p.translate(0.0, pose.bob);
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash);
    p.scale(facing, 1.0);
    if pose.swing != 0.0 {
        p.translate_pt(SCRUFF);
        p.rotate(pose.swing);
        p.translate_pt(-SCRUFF);
    }
    p.rotate(pose.lean);
    if pose.grounded && !head_only {
        // bending the legs lowers the hips; keep the lowest foot on the floor
        let drop = leg_drop(pose.leg_front).max(leg_drop(pose.leg_back));
        p.translate(0.0, (LEG_1 + LEG_2) - drop);
    }

    if !head_only {
        paint_tail(p, pose.tail);
        for (side, leg, color) in [(-1.0, pose.leg_back, FUR_SHADE), (1.0, pose.leg_front, FUR)] {
            let hip = pt(side * HIP_X, HIP_Y);
            let (mid, end) = limb(hip, leg, LEG_1, LEG_2);
            stroke(p, &limb_path(hip, mid, end), color, LIMB_W);
            blob(p, pt(end.x + 2.5, end.y - 0.5), 6.5, 3.6, SKIN);
        }
        blob(p, pt(0.0, -44.0), 19.0, 20.0, FUR);
        p.no_pen();
        p.set_brush(SKIN);
        p.draw_ellipse(pt(2.0, -42.0), 12.0, 15.0);
    }

    p.save();
    p.translate_pt(NECK);
    p.rotate(pose.head_tilt);
    p.translate_pt(-NECK);
    paint_head(p, pose, facing);
    if let Some(stars) = pose.stars {
        paint_stars(p, FACE_DX, -104.0, stars, 25.0, 5.0);
    }
    p.restore();

    if !head_only {
        for (side, arm) in [(-1.0, pose.arm_back), (1.0, pose.arm_front)] {
            let shoulder = pt(side * SHOULDER_X, SHOULDER_Y);
            let (mid, end) = limb(shoulder, arm, ARM_1, ARM_2);
            stroke(p, &limb_path(shoulder, mid, end), FUR, LIMB_W);
            if side == 1.0 && pose.banana >= 0 {
                paint_banana(p, end, arm.0 + arm.1, pose.banana);
            }
            blob(p, end, 4.6, 4.6, SKIN);
        }
    }
    p.restore();

    if let (Some(text), false) = (&pose.bubble, head_only) {
        bubble(p, text, facing * 14.0, -112.0 + pose.bob, facing, OUTLINE);
    }
}

fn paint_tail(p: &mut Painter, s: f64) {
    let mut path = Path::at(-12.0, -33.0);
    path.cubic_to(-32.0, -30.0 + 4.0 * s, -44.0 - 3.0 * s, -52.0, -38.0 - 4.0 * s, -68.0);
    path.cubic_to(-34.0 - 4.0 * s, -78.0, -24.0 - 3.0 * s, -77.0, -26.0 - 3.0 * s, -69.0);
    stroke(p, &path, FUR, 5.0);
}

fn paint_head(p: &mut Painter, pose: &MonkeyPose, facing: f64) {
    for side in [-1.0, 1.0] {
        blob(p, pt(side * 22.5, -81.0), 9.0, 9.0, FUR);
        p.no_pen();
        p.set_brush(SKIN);
        p.draw_circle(pt(side * 23.0, -81.0), 5.0);
    }
    blob(p, HEAD_C, HEAD_R, HEAD_R, FUR);

    let fx = FACE_DX;
    let mut face = Path::new();
    face.fill_rule = FillRule::Winding; // union of the overlapping ellipses
    face.add_ellipse(pt(fx - 7.5, -85.0), 9.5, 9.5);
    face.add_ellipse(pt(fx + 7.5, -85.0), 9.5, 9.5);
    face.add_ellipse(pt(fx, -71.0), 14.0, 10.0);
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(SKIN);
    p.draw_path(&face.simplified());

    let (lx, ly) = (pose.look.0 * facing, pose.look.1); // painter is mirrored; look is in screen space
    for ex in [fx - 7.5, fx + 7.5] {
        let c = pt(ex, -86.0);
        if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
            let wide = pose.eyes == Eyes::Wide;
            let er = if wide { 5.2 } else { 4.4 };
            blob(p, c, er, er, WHITE);
            let (r, k) = if wide { (2.2, 1.0) } else { (2.7, 1.5) };
            let pupil = pt(c.x + lx * k, c.y + ly * k);
            p.no_pen();
            p.set_brush(OUTLINE);
            p.draw_circle(pupil, r);
            p.set_brush(WHITE);
            p.draw_circle(pt(pupil.x - 0.9, pupil.y - 0.9), 0.9);
        } else {
            let mut path = Path::new();
            match pose.eyes {
                Eyes::Closed => {
                    path.move_to(c.x - 3.6, c.y);
                    path.quad_to(c.x, c.y + 2.6, c.x + 3.6, c.y);
                }
                Eyes::Happy => {
                    path.move_to(c.x - 3.6, c.y + 1.2);
                    path.quad_to(c.x, c.y - 3.4, c.x + 3.6, c.y + 1.2);
                }
                _ => {
                    path.move_to(c.x - 3.0, c.y - 3.0);
                    path.line_to(c.x + 3.0, c.y + 3.0);
                    path.move_to(c.x + 3.0, c.y - 3.0);
                    path.line_to(c.x - 3.0, c.y + 3.0);
                }
            }
            p.no_brush();
            p.set_pen(pen(OUTLINE, 1.7));
            p.draw_path(&path);
        }
    }

    p.no_pen();
    p.set_brush(OUTLINE);
    for nx in [fx - 2.3, fx + 2.3] {
        p.draw_circle(pt(nx, -75.0), 1.1);
    }

    let y = -69.5;
    let mut mouth = Path::new();
    match pose.mouth {
        Mouth::Big => {
            mouth.move_to(fx - 6.5, y);
            mouth.quad_to(fx, y + 9.0, fx + 6.5, y);
            mouth.close();
            p.set_pen(pen(OUTLINE, 1.3));
            p.set_brush(MOUTH);
            p.draw_path(&mouth);
        }
        Mouth::O => {
            p.set_pen(pen(OUTLINE, 1.1));
            p.set_brush(MOUTH);
            p.draw_ellipse(pt(fx, -67.0), 2.8, 3.3);
        }
        m => {
            if m == Mouth::Chew {
                mouth.move_to(fx - 4.0, y + 1.5);
                mouth.line_to(fx + 4.0, y + 1.5);
            } else {
                mouth.move_to(fx - 6.0, y);
                mouth.quad_to(fx, y + 4.5, fx + 6.0, y);
            }
            p.no_brush();
            p.set_pen(pen(OUTLINE, 1.6));
            p.draw_path(&mouth);
        }
    }
}

fn paint_banana(p: &mut Painter, hand: Pt, forearm_deg: f64, bites: i32) {
    p.save();
    p.translate_pt(hand);
    p.rotate((180.0 - forearm_deg) * 0.7); // point roughly along the forearm
    let length = 28.0 * (1.0 - 0.25 * bites as f64);
    if bites > 0 {
        // peel flaps hanging down
        p.set_pen(pen(BANANA_EDGE, 1.0));
        p.set_brush(BANANA);
        p.draw_polygon(&[pt(-4.0, -3.0), pt(-9.0, 5.0), pt(-5.0, 4.0)]);
        p.draw_polygon(&[pt(1.0, -3.0), pt(7.0, 6.0), pt(3.0, 4.0)]);
    }
    let mut body = Path::at(2.5, 2.0);
    body.cubic_to(-1.0, -length * 0.3, -1.0, -length * 0.75, -4.0, -length);
    body.cubic_to(-9.0, -length * 0.7, -8.0, -length * 0.25, -2.5, 2.0);
    body.close();
    p.set_pen(pen(BANANA_EDGE, 1.2));
    p.set_brush(if bites == 0 { BANANA } else { BANANA_FLESH });
    p.draw_path(&body);
    if bites == 0 {
        p.no_pen();
        p.set_brush(OUTLINE);
        p.draw_circle(pt(-4.0, -length), 1.2);
    }
    p.no_pen();
    p.set_brush(OUTLINE);
    p.draw_rect(-1.2, 1.5, 2.6, 3.5); // stem
    p.restore();
}

pub static CHARACTER: Character = Character {
    name: "monkey",
    half_width: HALF_WIDTH,
    height: HEIGHT,
    eye_y: EYE_Y,
    grip: (8.0, -66.0),
    icon_box: (FACE_DX / 2.0, -80.0, 64.0),
    size: Character::DEFAULT_SIZE,
    previews: &[
        ("scratch", "scratch", 3.0, 1.0),
        ("eat: bite", "eat", EAT_TIME, 1.2),
        ("eat: chew", "eat", EAT_TIME, 2.7),
        ("eat: done", "eat", EAT_TIME, 4.5),
    ],
    params: &Monkey::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Monkey, x, y, seed)),
};
