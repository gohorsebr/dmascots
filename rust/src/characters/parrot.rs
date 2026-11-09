//! Parrot: waddles, dances, whistles, preens, flies, and repeats what you've said to it.
//! Port of `characters/parrot/{brain,art,__init__}.py`.
//! Green parrot: yellow forehead, red shoulder, blue flight feathers, hooked beak.

use std::f64::consts::PI;

use rand::Rng;

use crate::art::{bubble, paint_stars, paint_zzz, pen, tapered, tapered_curve};
use crate::engine::{ease, lerp, lerp2, Bounds, Brain, Character, Core, Eyes, Params, Species};
use crate::gfx::{polyline, pt, Color, Painter, Path, Pt, Xf};
use crate::pose;

const REST_HEAD: (f64, f64) = (5.0, -56.0);
pub const WHISTLE_TIME: f64 = 1.8;
const PHRASES: &[&str] = &["Hello!", "Pretty bird!", "Who's a good bird?", "Hehehe.", "Cracker?", "Hello? Hello!"];

pose!(ParrotPose {
    head: (f64, f64) = REST_HEAD,
    head_angle: f64 = 0.0,
    beak_open: f64 = 0.0,
    wing: f64 = 0.0,            // -0.3 (down-stroke) .. 0 folded .. 1 raised
    leg_front: f64 = 4.0,
    leg_back: f64 = -4.0,
    tail: f64 = 0.0,            // degrees, positive = up
    notes: Option<f64> = None,  // music notes floating up (dancing / whistling)
    fluff: f64 = 0.0,           // 0..1 feathers puffed up
});

pub struct Parrot;

fn jerky(c: &Core, period: f64, amount: f64) -> f64 {
    const STEPS: [f64; 6] = [-1.0, 0.4, -0.6, 1.0, -0.2, 0.7];
    amount * STEPS[(c.clock / period) as usize % 6]
}

impl Species for Parrot {
    type Pose = ParrotPose;
    const PARAMS: Params = Params {
        steal_lines: &["Pretty pointer!", "SQUAWK!", "Mine! Mine!"],
        return_lines: &["Bye bye!", "Hehehe."],
        walk_speed: 32.0,
        jump_vy: -520.0,
        jump_vx: 120.0,
        airborne: Params::FLYER_AIRBORNE,
        fly_speed: 300.0,
        lines: &["SQUAWK!", "Hello!", "Pretty bird!", "Who's a good bird?", "Hehehe.", "Cracker?", "Hello? Hello!"],
        choices: &[
            (18.0, "walk", (2.0, 4.0)),
            (10.0, "dance", (3.0, 6.0)),
            (13.0, "mimic", (2.0, 2.0)), // real duration depends on what it says
            (8.0, "whistle", (WHISTLE_TIME, WHISTLE_TIME)),
            (8.0, "preen", (2.0, 3.5)),
            (10.0, "fly", (1.0, 1.0)),
            (6.0, "crouch", (0.14, 0.14)),
            (14.0, "idle", (1.5, 4.0)),
        ],
        ..Params::BASE
    };

    /// Poke a parrot and it talks back -- often with something you told it.
    fn poke(&mut self, c: &mut Core) {
        if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy") {
            c.go("mimic", 2.0);
        } else {
            c.say("SQUAWK!", 1.6, false);
        }
    }

    fn pose_rest(&self, c: &Core, p: &mut ParrotPose) {
        p.bob = (c.clock * 1.8).sin() * 0.4;
        p.head_angle = jerky(c, 1.1, 10.0);
        p.tail = 2.0 * (c.clock * 1.2).sin();
    }

    fn pose_walk(&self, _c: &Core, p: &mut ParrotPose, t: f64) {
        let ph = t * 8.0;
        let s = ph.sin();
        p.leg_front = 18.0 * s;
        p.leg_back = -18.0 * s;
        p.lean = 6.0 * s; // side-to-side waddle
        p.bob = -1.2 * s.abs();
        p.tail = 4.0 * s;
    }

    fn pose_crouch(&self, c: &Core, p: &mut ParrotPose, k: f64) {
        self.pose_rest(c, p);
        p.wing = 0.25 * k;
    }

    fn pose_jump(&self, c: &Core, p: &mut ParrotPose) {
        p.wing = 0.5 + 0.5 * (c.clock * 28.0).sin();
        p.leg_front = -25.0;
        p.leg_back = -35.0;
    }

    fn pose_fall(&self, c: &Core, p: &mut ParrotPose) {
        p.wing = 0.35 + 0.65 * (c.clock * 30.0).sin();
        p.beak_open = 0.7;
        p.fluff = 0.6;
        p.tail = 10.0 * (c.clock * 11.0).sin();
    }

    fn pose_land(&self, c: &Core, p: &mut ParrotPose, bend: f64) {
        self.pose_rest(c, p);
        p.wing = 0.6 * bend;
        p.fluff = 0.4 * bend;
    }

    fn pose_dizzy(&self, c: &Core, p: &mut ParrotPose) {
        let c = c.clock;
        p.head = (REST_HEAD.0 + 2.5 * (c * 4.0).cos(), REST_HEAD.1 + 2.0 * (c * 4.0).sin());
        p.head_angle = 14.0 * (c * 4.0).sin();
        p.fluff = 0.3;
    }

    fn pose_dragged(&self, c: &Core, p: &mut ParrotPose) {
        let c = c.clock;
        p.wing = 0.35 + 0.65 * (c * 26.0).sin();
        p.beak_open = (c * 5.0).sin().abs();
        p.fluff = 0.8;
        p.leg_front = 20.0 * (c * 9.0).sin();
        p.leg_back = -20.0 * (c * 9.0).sin();
        p.tail = 12.0 * (c * 13.0).sin();
    }

    fn pose_flight(&self, c: &Core, p: &mut ParrotPose, prog: f64) {
        let descending = prog > 0.5;
        p.wing = 0.35 + 0.65 * (c.clock * if descending { 16.0 } else { 24.0 }).sin();
        p.lean = if descending { 22.0 } else { 35.0 }; // body stretched out level
        let landing = ease((prog - 0.8) / 0.2);
        p.leg_front = lerp(-60.0, 8.0, landing);
        p.leg_back = lerp(-70.0, -6.0, landing);
        p.tail = -14.0 + 14.0 * landing;
        p.head = (REST_HEAD.0 + 3.0, REST_HEAD.1 + 3.0);
    }

    fn state(&mut self, c: &mut Core, _dt: f64, _b: &Bounds, p: &mut ParrotPose) -> bool {
        match c.state {
            "dance" => self.dance(c, p),
            "mimic" => self.mimic(c, p),
            "whistle" => self.whistle(c, p),
            "preen" => self.preen(c, p),
            _ => return false,
        }
        true
    }

    fn paint(&self, p: &mut Painter, pose: &ParrotPose, facing: f64, head_only: bool) {
        paint(p, pose, facing, head_only);
    }
}

impl Parrot {
    /// Head-bobbing to a beat only it can hear.
    fn dance(&self, c: &mut Core, p: &mut ParrotPose) {
        self.pose_rest(c, p);
        let beat = (PI * c.t / 0.5).sin().abs();
        p.head = (REST_HEAD.0 + 2.0 * beat, REST_HEAD.1 + 4.0 * beat);
        p.head_angle = 15.0 * beat;
        p.bob = 2.0 * beat;
        p.lean = 6.0 * (if (c.t / 1.0) as i64 % 2 == 1 { 1.0 } else { -1.0 }) * ease(beat);
        p.wing = 0.15 * beat;
        p.leg_front = 6.0 * beat;
        p.leg_back = -6.0 * beat;
        p.notes = Some(c.clock);
        if c.entered() && c.random() < 0.4 {
            c.say("♪ ♫", 1.4, false);
        }
        if c.done() {
            c.go_idle();
        }
    }

    /// Say something: usually a message it was sent, otherwise a stock phrase.
    fn mimic(&self, c: &mut Core, p: &mut ParrotPose) {
        self.pose_rest(c, p);
        if c.entered() {
            let text = if !c.heard.is_empty() && c.random() < 0.7 {
                let i = c.rng.gen_range(0..c.heard.len());
                c.heard[i].clone()
            } else {
                c.choice(PHRASES).to_string()
            };
            c.duration = (0.6 + 0.07 * text.chars().count() as f64).clamp(1.6, 5.0);
            let secs = c.duration + 0.4;
            c.say(&text, secs, false);
        }
        let talking = c.t < c.duration - 0.3;
        p.beak_open = if talking { (c.clock * 18.0).sin().max(0.0) * 0.7 } else { 0.0 };
        p.head_angle = jerky(c, 0.35, 18.0);
        p.head = (REST_HEAD.0 + 2.0, REST_HEAD.1);
        if c.done() {
            c.go_idle();
        }
    }

    fn whistle(&self, c: &mut Core, p: &mut ParrotPose) {
        self.pose_rest(c, p);
        if c.entered() {
            let line = c.choice(&["♪ Fweet fweeoo! ♪", "♪ Wheet-wheeoo ♪", "♪ Tweedle-dee ♪"]);
            c.say(line, 1.8, false);
        }
        let k = ease(c.t / 0.2) * ease((c.duration - c.t) / 0.3);
        p.head = lerp2(REST_HEAD, (REST_HEAD.0 - 1.0, REST_HEAD.1 - 3.0), k);
        p.head_angle = -20.0 * k;
        p.beak_open = 0.25 * k + 0.1 * (c.clock * 20.0).sin() * k;
        p.notes = Some(c.clock);
        p.fluff = 0.3 * k;
        if c.done() {
            c.go_idle();
        }
    }

    fn preen(&self, c: &mut Core, p: &mut ParrotPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.35) * ease((c.duration - c.t) / 0.35);
        let cl = c.clock;
        p.head = lerp2(REST_HEAD, (-5.0 + 2.0 * (cl * 14.0).sin(), -42.0 + 2.0 * (cl * 14.0).cos()), k);
        p.head_angle = 125.0 * k;
        p.wing = 0.12 * k;
        p.fluff = 0.5 * k;
        if k > 0.8 {
            p.eyes = Eyes::Closed;
        }
        if c.done() {
            c.go_idle();
        }
    }
}

// =================================================================== art

const GREEN: Color = Color::hex("#45AE49");
const GREEN_LIGHT: Color = Color::hex("#79CC5E");
const GREEN_DARK: Color = Color::hex("#2C8B3A");
const YELLOW: Color = Color::hex("#F4D23C");
const RED: Color = Color::hex("#D8352A");
const BLUE: Color = Color::hex("#2E66C6");
const TAIL_TIP: Color = Color::hex("#BFD94A");
const BEAK: Color = Color::hex("#DCCDA8");
const BEAK_DARK: Color = Color::hex("#8C7E62");
const EYE_RING: Color = Color::hex("#F2F1EC");
const IRIS: Color = Color::hex("#F39A1E");
const PUPIL: Color = Color::hex("#141010");
const WHITE: Color = Color::hex("#FFFFFF");
const LEGS: Color = Color::hex("#8C8C93");
const OUTLINE: Color = Color::hex("#173A1C");

const LINE_W: f64 = 1.6;
const HEAD_R: f64 = 11.5;
const NECK_BASE: Pt = pt(3.0, -44.0);
const PIVOT: Pt = pt(0.0, -50.0);
const HIP_BACK: Pt = pt(-3.0, -14.0);
const HIP_FRONT: Pt = pt(3.0, -14.0);
const LEG: f64 = 11.0;
const TAIL_ROOT: Pt = pt(-7.0, -19.0);

const HALF_WIDTH: f64 = 42.0;
const HEIGHT: f64 = 72.0;
const EYE_Y: f64 = 59.0;

pub fn paint(p: &mut Painter, pose: &ParrotPose, facing: f64, head_only: bool) {
    p.save();
    p.translate(0.0, pose.bob);
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash);
    p.scale(facing, 1.0);
    if pose.swing != 0.0 {
        p.translate_pt(PIVOT);
        p.rotate(pose.swing);
        p.translate_pt(-PIVOT);
    }
    p.rotate(pose.lean);

    let head = Pt::from(pose.head);
    let puff = 1.0 + 0.12 * pose.fluff;
    let mut skull = Path::new();
    skull.add_ellipse(head, HEAD_R * puff, HEAD_R * puff);
    let neck = tapered(
        (NECK_BASE.x, NECK_BASE.y),
        (NECK_BASE.x + 1.0, NECK_BASE.y - 4.0),
        (head.x - 2.0, head.y + 5.0),
        (head.x, head.y),
        21.0 * puff,
        19.0 * puff,
    );
    let mut sil = neck.united(&skull);
    let mut has_body = false;
    if !head_only {
        paint_tail(p, pose.tail);
        for (hip, angle) in [(HIP_BACK, pose.leg_back), (HIP_FRONT, pose.leg_front)] {
            paint_leg(p, hip, angle);
        }
        let mut body = Path::new();
        body.add_ellipse(pt(0.0, 0.0), 13.5 * puff, 19.0 * puff);
        let body = Xf::new().translate(0.0, -33.0).rotate(16.0).map_path(&body); // upright, chest forward
        sil = sil.united(&body);
        has_body = true;
    }

    p.no_pen();
    p.set_brush(GREEN);
    p.draw_path(&sil);
    p.save();
    p.set_clip_path(&sil);
    if has_body {
        p.set_brush(GREEN_LIGHT);
        p.draw_ellipse(pt(7.0, -28.0), 9.0, 14.0); // lighter breast
    }
    p.set_brush(YELLOW);
    p.draw_ellipse(pt(head.x + 3.0, head.y - 9.0), 8.0, 5.5); // yellow forehead
    p.restore();
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&sil);

    if !head_only {
        paint_wing(p, pose.wing);
    }
    paint_head(p, pose, head, facing);
    if let Some(stars) = pose.stars {
        paint_stars(p, head.x, head.y - 17.0, stars, 16.0, 4.0);
    }
    if let (Some(notes), false) = (pose.notes, head_only) {
        paint_zzz(p, head.x + 10.0, head.y - 6.0, notes, facing, "♪♫♪", BLUE);
    }
    p.restore();

    if let (Some(text), false) = (&pose.bubble, head_only) {
        bubble(p, text, facing * pose.head.0, pose.head.1 - 15.0 + pose.bob, facing, OUTLINE);
    }
}

fn paint_tail(p: &mut Painter, angle: f64) {
    p.save();
    p.translate_pt(TAIL_ROOT);
    p.rotate(-angle);
    let tail = tapered_curve((0.0, 0.0), (-7.0, 5.0), (-15.0, 10.0), (-23.0, 15.0), 11.0, 7.0, 10, 1.0);
    p.no_pen();
    p.set_brush(GREEN_DARK);
    p.draw_path(&tail);
    p.save();
    p.set_clip_path(&tail);
    p.set_brush(TAIL_TIP);
    p.draw_circle(pt(-23.0, 15.0), 7.0);
    p.restore();
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&tail);
    p.restore();
}

fn paint_leg(p: &mut Painter, hip: Pt, angle: f64) {
    let a = angle.to_radians();
    let (x, y) = (hip.x + LEG * a.sin(), hip.y + LEG * a.cos());
    p.no_brush();
    p.set_pen(pen(LEGS, 3.0));
    p.draw_line(hip, pt(x, y));
    p.set_pen(pen(LEGS, 2.0)); // zygodactyl: two toes forward, two back
    p.draw_path(&polyline(&[(x + 5.5, y + 2.4), (x, y + 1.6), (x - 4.5, y + 2.6)]));
    p.draw_line(pt(x, y + 1.6), pt(x + 4.0, y + 3.2));
}

fn paint_wing(p: &mut Painter, raised: f64) {
    p.save();
    p.translate(1.0, -44.0); // shoulder
    p.rotate(105.0 * raised);
    let mut wing = Path::at(4.0, -2.0);
    wing.cubic_to(-4.0, -6.0, -12.0, 4.0, -14.0, 26.0); // down the back to the tip
    wing.quad_to(-10.0, 24.0, -8.0, 27.0);
    wing.quad_to(-5.0, 22.0, -2.0, 22.0);
    wing.cubic_to(4.0, 14.0, 7.0, 4.0, 4.0, -2.0);
    p.no_pen();
    p.set_brush(GREEN_DARK);
    p.draw_path(&wing);
    p.save();
    p.set_clip_path(&wing);
    p.set_brush(BLUE);
    p.draw_circle(pt(-9.0, 26.0), 8.0); // blue flight feathers
    p.set_brush(RED);
    p.draw_ellipse(pt(1.0, 1.0), 5.0, 4.5); // red shoulder patch
    p.restore();
    p.set_pen(pen(GREEN_LIGHT, 0.9));
    p.draw_line(pt(-3.0, 10.0), pt(-9.0, 20.0));
    p.draw_line(pt(1.0, 11.0), pt(-4.0, 20.0));
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&wing);
    p.restore();
}

fn paint_head(p: &mut Painter, pose: &ParrotPose, head: Pt, facing: f64) {
    p.save();
    p.translate_pt(head);
    p.rotate(pose.head_angle);

    p.save(); // lower mandible hinges open
    p.translate(7.5, 3.0);
    p.rotate(25.0 * pose.beak_open.min(1.0));
    let mut lower = polyline(&[(0.0, -0.5), (6.5, 0.2), (5.5, 3.2), (0.0, 4.0)]);
    lower.close();
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(BEAK_DARK);
    p.draw_path(&lower);
    p.restore();

    let mut upper = Path::at(6.0, -6.5);
    upper.cubic_to(13.0, -8.5, 19.0, -3.0, 16.5, 7.0); // big hooked upper beak
    upper.quad_to(15.0, 3.5, 12.0, 2.6);
    upper.line_to(7.0, 3.2);
    upper.quad_to(4.5, -1.5, 6.0, -6.5);
    upper.close();
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(BEAK);
    p.draw_path(&upper);
    p.set_pen(pen(BEAK_DARK, 0.9));
    p.draw_line(pt(8.0, -5.0), pt(14.0, -3.0));

    let eye = pt(1.0, -3.0);
    let (lx, ly) = (pose.look.0 * facing, pose.look.1);
    p.no_pen();
    p.set_brush(EYE_RING); // bare white eye ring
    p.draw_circle(eye, 4.6);
    if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
        let r = if pose.eyes == Eyes::Wide { 2.9 } else { 2.6 };
        let c = pt(eye.x + lx * 0.6, eye.y + ly * 0.6);
        p.set_brush(IRIS);
        p.draw_circle(c, r);
        p.set_brush(PUPIL);
        p.draw_circle(c, r * 0.5);
        p.set_brush(WHITE);
        p.draw_circle(pt(c.x - 0.8, c.y - 0.9), 0.7);
    } else {
        let mut path = Path::new();
        let (x, y) = (eye.x, eye.y);
        match pose.eyes {
            Eyes::Closed => {
                path.move_to(x - 3.0, y);
                path.quad_to(x, y + 2.2, x + 3.0, y);
            }
            Eyes::Happy => {
                path.move_to(x - 3.0, y + 1.0);
                path.quad_to(x, y - 2.8, x + 3.0, y + 1.0);
            }
            _ => {
                path.move_to(x - 2.4, y - 2.4);
                path.line_to(x + 2.4, y + 2.4);
                path.move_to(x + 2.4, y - 2.4);
                path.line_to(x - 2.4, y + 2.4);
            }
        }
        p.no_brush();
        p.set_pen(pen(PUPIL, 1.5));
        p.draw_path(&path);
    }
    p.set_pen(pen(OUTLINE, 0.9));
    p.no_brush();
    p.draw_circle(eye, 4.6);
    p.restore();
}

pub static CHARACTER: Character = Character {
    name: "parrot",
    half_width: HALF_WIDTH,
    height: HEIGHT,
    eye_y: EYE_Y,
    grip: (20.0, -49.0),
    icon_box: (9.0, -56.0, 38.0),
    size: Character::DEFAULT_SIZE,
    previews: &[
        ("dance", "dance", 4.0, 0.5),
        ("mimic", "mimic", 2.0, 0.4),
        ("whistle", "whistle", WHISTLE_TIME, 0.6),
        ("preen", "preen", 3.0, 1.2),
        ("fly", "fly", 1.0, 0.45),
    ],
    params: &Parrot::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Parrot, x, y, seed)),
};
