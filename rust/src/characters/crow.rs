//! Crow: struts, pecks, caws, tilts its head, flies, hoards shiny coins.
//! Port of `characters/crow/{brain,art,__init__}.py`.
//!
//! Body, neck, head and tail are unioned into one glossy silhouette;
//! the wing, beak, eye and any coin are layered on top.

use crate::art::{bubble, paint_stars, pen, tapered};
use crate::engine::{bump, clamp, ease, keyframes, lerp, lerp2, Bounds, Brain, Character, Core, Eyes, Params, Species};
use crate::gfx::{polyline, pt, Color, Painter, Path, Pt, Xf};
use crate::pose;

const REST_HEAD: (f64, f64) = (16.0, -54.0);
pub const CAW_TIME: f64 = 1.3;
pub const SHINY_TIME: f64 = 4.6;
const TILTS: [f64; 6] = [-22.0, 8.0, -30.0, 14.0, -12.0, 4.0]; // jerky, bird-like head angles

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coin {
    #[default]
    None,
    Ground,
    Beak,
}

pose!(CrowPose {
    head: (f64, f64) = REST_HEAD, // head centre, relative to the feet (facing +x)
    head_angle: f64 = 0.0,        // degrees, positive tips the beak down
    beak_open: f64 = 0.0,         // 0..1
    wing: f64 = 0.0,              // -0.3 (down-stroke) .. 0 folded .. 1 raised
    leg_front: f64 = 4.0,         // leg angles, degrees, positive = forward
    leg_back: f64 = -4.0,
    tail: f64 = 0.0,              // tail angle offset, degrees (positive = up)
    coin: Coin = Coin::None,
    glint: f64 = 0.0,             // 0..1 sparkle on the coin
});

pub struct Crow;

const LINES: &[&str] = &["CAW!", "CAW! CAW!", "Kraa!", "Caw caw?", "Shiny?"];

fn jerky_tilt(c: &Core, period: f64, scale: f64) -> f64 {
    TILTS[(c.clock / period) as usize % TILTS.len()] * scale
}

impl Species for Crow {
    type Pose = CrowPose;
    const PARAMS: Params = Params {
        steal_lines: &["Shiny!", "Mine.", "CAW!"],
        return_lines: &["Not shiny enough.", "Kraa. Fine."],
        walk_speed: 65.0,
        jump_vy: -560.0,
        jump_vx: 160.0,
        airborne: Params::FLYER_AIRBORNE,
        lines: LINES,
        choices: &[
            (28.0, "walk", (2.0, 5.0)),
            (14.0, "peck", (2.0, 4.0)),
            (9.0, "caw", (CAW_TIME, CAW_TIME)),
            (8.0, "shiny", (SHINY_TIME, SHINY_TIME)),
            (11.0, "fly", (1.0, 1.0)), // real duration is set on take-off
            (10.0, "crouch", (0.12, 0.12)),
            (8.0, "tilt", (1.5, 3.0)),
            (12.0, "idle", (1.5, 3.5)),
        ],
        ..Params::BASE
    };

    /// A poked crow caws back at you.
    fn poke(&mut self, c: &mut Core) {
        if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy") {
            c.go("caw", CAW_TIME);
        } else {
            c.chatter(LINES);
        }
    }

    fn pose_rest(&self, c: &Core, p: &mut CrowPose) {
        p.bob = (c.clock * 2.0).sin() * 0.4;
        p.head_angle = jerky_tilt(c, 1.3, 0.4); // birds look around in little jumps
        p.tail = 3.0 * (c.clock * 1.3).sin();
    }

    fn pose_walk(&self, _c: &Core, p: &mut CrowPose, t: f64) {
        let ph = t * 9.0;
        let s = ph.sin();
        p.leg_front = 24.0 * s;
        p.leg_back = -24.0 * s;
        let bob = (ph * 2.0).sin().max(0.0); // head thrusts forward each step
        p.head = (REST_HEAD.0 + 4.0 * bob - 1.0, REST_HEAD.1 + 1.5 * bob);
        p.tail = 4.0 * (ph * 2.0).sin();
        p.lean = 3.0;
    }

    fn pose_crouch(&self, c: &Core, p: &mut CrowPose, k: f64) {
        self.pose_rest(c, p);
        p.wing = 0.3 * k;
        p.tail = 10.0 * k;
    }

    fn pose_jump(&self, c: &Core, p: &mut CrowPose) {
        p.wing = 0.5 + 0.5 * (c.clock * 30.0).sin();
        p.leg_front = -30.0;
        p.leg_back = -40.0;
        p.tail = -6.0;
    }

    fn pose_fall(&self, c: &Core, p: &mut CrowPose) {
        let c = c.clock;
        p.wing = 0.35 + 0.65 * (c * 30.0).sin();
        p.beak_open = 0.6 + 0.4 * (c * 9.0).sin();
        p.leg_front = 15.0 * (c * 14.0).sin();
        p.leg_back = -15.0 * (c * 14.0).sin();
        p.tail = 10.0 * (c * 11.0).sin();
    }

    fn pose_land(&self, c: &Core, p: &mut CrowPose, bend: f64) {
        self.pose_rest(c, p);
        p.wing = 0.7 * bend;
        p.tail = 14.0 * bend;
    }

    fn pose_dizzy(&self, c: &Core, p: &mut CrowPose) {
        let c = c.clock;
        p.head = (REST_HEAD.0 + 2.5 * (c * 4.0).cos(), REST_HEAD.1 + 2.0 * (c * 4.0).sin());
        p.head_angle = 14.0 * (c * 4.0).sin();
        p.wing = 0.08;
    }

    fn pose_dragged(&self, c: &Core, p: &mut CrowPose) {
        let c = c.clock;
        p.wing = 0.35 + 0.65 * (c * 26.0).sin();
        p.head_angle = -15.0;
        p.beak_open = (c * 5.0).sin().abs();
        p.leg_front = 20.0 * (c * 9.0).sin();
        p.leg_back = -20.0 * (c * 9.0).sin();
        p.tail = 12.0 * (c * 13.0).sin();
    }

    fn pose_flight(&self, c: &Core, p: &mut CrowPose, prog: f64) {
        let descending = prog > 0.5;
        // strong flapping on the way up, slower glide-flaps coming down
        p.wing = 0.35 + 0.65 * (c.clock * if descending { 16.0 } else { 24.0 }).sin();
        p.lean = if descending { 4.0 } else { 12.0 };
        let landing = ease((prog - 0.8) / 0.2);
        p.leg_front = lerp(-55.0, 10.0, landing); // legs tucked back, then reach for the ground
        p.leg_back = lerp(-65.0, -5.0, landing);
        p.tail = -4.0 + 10.0 * landing;
        p.head = (REST_HEAD.0 + 2.0, REST_HEAD.1 + 2.0);
    }

    fn state(&mut self, c: &mut Core, dt: f64, _b: &Bounds, p: &mut CrowPose) -> bool {
        match c.state {
            "peck" => self.peck(c, p),
            "caw" => self.caw(c, p),
            "tilt" => self.tilt(c, p),
            "shiny" => self.shiny(c, dt, p),
            _ => return false,
        }
        true
    }

    fn paint(&self, p: &mut Painter, pose: &CrowPose, facing: f64, head_only: bool) {
        paint(p, pose, facing, head_only);
    }
}

impl Crow {
    fn peck(&self, c: &mut Core, p: &mut CrowPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.3) * ease((c.duration - c.t) / 0.3);
        let peck = (c.clock * 10.0).sin().max(0.0).powi(3);
        p.head = lerp2(p.head, (27.0, -25.0 + 4.0 * peck), k); // tips the whole body, not just the neck
        p.head_angle = lerp(p.head_angle, 62.0, k);
        p.lean = 10.0 * k;
        p.tail = lerp(p.tail, 12.0, k);
        if c.done() {
            c.go_idle();
        }
    }

    fn caw(&self, c: &mut Core, p: &mut CrowPose) {
        self.pose_rest(c, p);
        if c.entered() {
            let line = c.choice(&["CAW!", "CAW! CAW!", "Kraa!"]);
            c.say(line, 1.5, false);
        }
        let k = ease(c.t / 0.15) * ease((c.duration - c.t) / 0.2);
        let caw = bump(c.t, 0.3, 0.35) + bump(c.t, 0.85, 0.35);
        p.head = lerp2(p.head, (21.0, -55.0), k);
        p.head_angle = -14.0 * k - 6.0 * caw;
        p.beak_open = caw;
        p.squash = 1.0 - 0.05 * caw; // the whole body pumps with each caw
        p.wing = 0.12 * caw;
        p.tail = 4.0 + 10.0 * caw;
        if c.done() {
            c.go_idle();
        }
    }

    /// Curious: snaps its head between angles while its eye stays on the cursor.
    fn tilt(&self, c: &mut Core, p: &mut CrowPose) {
        self.pose_rest(c, p);
        p.head_angle = jerky_tilt(c, 0.45, 1.0);
        p.head = (REST_HEAD.0 + 1.0, REST_HEAD.1 - 1.0);
        if c.done() {
            c.go_idle();
        }
    }

    /// Spot a coin, pick it up, admire it, tuck it under the wing.
    fn shiny(&self, c: &mut Core, dt: f64, p: &mut CrowPose) {
        self.pose_rest(c, p);
        let t = c.t;
        let [hx, hy, angle] = keyframes(
            t,
            &[
                (0.0, [REST_HEAD.0, REST_HEAD.1, 0.0]),
                (0.5, [24.0, -40.0, 35.0]), // notice it
                (0.8, [27.0, -25.0, 62.0]), // pick it up
                (1.0, [27.0, -25.0, 62.0]),
                (1.35, [19.0, -57.0, -10.0]), // hold it up
                (3.4, [19.0, -57.0, -10.0]),
                (3.9, [-2.0, -47.0, 140.0]), // tuck it under the wing
                (4.15, [-2.0, -47.0, 140.0]),
                (SHINY_TIME, [REST_HEAD.0, REST_HEAD.1, 0.0]),
            ],
        );
        p.head = (hx, hy);
        p.head_angle = angle;
        p.lean = 10.0 * clamp((hy + 40.0) / 15.0, 0.0, 1.0); // lean in as the head goes down
        if 1.35 < t && t < 3.4 {
            p.head_angle += jerky_tilt(c, 0.5, 0.9);
            p.glint = bump(t, 1.7, 0.4).max(bump(t, 2.7, 0.4));
            let ph = (t - 1.35).rem_euclid(1.0);
            if 0.6 < ph && ph < 0.9 {
                p.eyes = Eyes::Happy;
            }
        }
        if 3.4 < t && t < 4.15 {
            p.wing = 0.15;
        }
        p.coin = if t < 0.9 {
            Coin::Ground
        } else if t < 4.0 {
            Coin::Beak
        } else {
            Coin::None
        };
        if t - dt < 1.4 && 1.4 <= t {
            let line = c.choice(&["Shiny!", "Mine.", "Ooh."]);
            c.say(line, 1.8, false);
        }
        if c.done() {
            c.go_idle();
        }
    }
}

// =================================================================== art

const BODY: Color = Color::hex("#24252E");
const GLOSS: Color = Color::rgba(110, 125, 190, 70); // blue-violet sheen on black feathers
const WING: Color = Color::hex("#2D2F3B");
const FEATHER_LINE: Color = Color::hex("#565C75");
const OUTLINE: Color = Color::hex("#0C0C10");
const BEAK: Color = Color::hex("#2A2A31");
const BEAK_RIDGE: Color = Color::hex("#575864");
const LEGS: Color = Color::hex("#3A3A41");
const EYE_RING: Color = Color::hex("#F2F2F2");
const PUPIL: Color = Color::hex("#140E0B");
const WHITE: Color = Color::hex("#FFFFFF");
const GOLD: Color = Color::hex("#F2C230");
const GOLD_LIGHT: Color = Color::hex("#FFE47A");
const GOLD_EDGE: Color = Color::hex("#9A7410");

const LINE_W: f64 = 1.6;
const HEAD_R: f64 = 11.0;
const NECK_BASE: Pt = pt(8.0, -44.0);
const PIVOT: Pt = pt(0.0, -46.0); // grab point when dangling from the cursor
const HIP_BACK: Pt = pt(-6.0, -21.0);
const HIP_FRONT: Pt = pt(2.0, -21.0);
const LEG: f64 = 17.0;
const TAIL_ROOT: Pt = pt(-19.0, -31.0);
const TAIL_DROOP: f64 = 12.0; // degrees the resting tail points below horizontal

const HALF_WIDTH: f64 = 50.0;
const HEIGHT: f64 = 70.0;
const EYE_Y: f64 = 57.0;

pub fn paint(p: &mut Painter, pose: &CrowPose, facing: f64, head_only: bool) {
    p.save();
    p.translate(0.0, pose.bob);
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash);
    p.scale(facing, 1.0);
    if pose.coin == Coin::Ground && !head_only {
        paint_coin(p, pt(33.0, -1.6), 4.2, 1.7, 0.0); // on the floor, not leaning with the bird
    }
    if pose.swing != 0.0 {
        p.translate_pt(PIVOT);
        p.rotate(pose.swing);
        p.translate_pt(-PIVOT);
    }
    p.rotate(pose.lean);
    if pose.grounded && !head_only {
        let drop = pose.leg_front.to_radians().cos().max(pose.leg_back.to_radians().cos()) * LEG;
        p.translate(0.0, LEG - drop);
    }

    let head = Pt::from(pose.head);
    let mut skull = Path::new();
    skull.add_ellipse(head, HEAD_R, HEAD_R);
    let mut sil = neck_path(head).united(&skull);
    if !head_only {
        sil = sil.united(&body_path()).united(&tail_path(pose.tail));
        for (hip, angle) in [(HIP_BACK, pose.leg_back), (HIP_FRONT, pose.leg_front)] {
            paint_leg(p, hip, angle);
        }
    }

    p.no_pen();
    p.set_brush(BODY);
    p.draw_path(&sil);
    p.save(); // sheen along the back and crown
    p.set_clip_path(&sil);
    p.set_brush(GLOSS);
    if !head_only {
        p.draw_ellipse(pt(-6.0, -46.0), 20.0, 5.0);
    }
    p.draw_ellipse(pt(head.x - 2.0, head.y - 7.0), 7.0, 3.0);
    p.restore();
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&sil);

    if !head_only {
        paint_wing(p, pose.wing);
    }
    paint_head(p, pose, head, facing);
    if let Some(stars) = pose.stars {
        paint_stars(p, head.x, head.y - 17.0, stars, 17.0, 4.0);
    }
    p.restore();

    if let (Some(text), false) = (&pose.bubble, head_only) {
        bubble(p, text, facing * pose.head.0, pose.head.1 - 14.0 + pose.bob, facing, OUTLINE);
    }
}

fn neck_path(head: Pt) -> Path {
    let p0 = (NECK_BASE.x, NECK_BASE.y);
    tapered(p0, (p0.0 + 3.0, p0.1 - 5.0), (head.x - 3.0, head.y + 6.0), (head.x, head.y), 19.0, 15.0)
}

fn body_path() -> Path {
    let mut body = Path::new();
    body.add_ellipse(pt(0.0, 0.0), 22.0, 13.5);
    Xf::new().translate(-2.0, -33.0).rotate(-18.0).map_path(&body) // chest held high
}

fn tail_path(angle: f64) -> Path {
    let mut fan = Path::new();
    fan.add_polygon(&[
        pt(2.0, -4.0),
        pt(-23.0, -6.0),
        pt(-27.0, -1.0),
        pt(-28.0, 4.0),
        pt(-25.0, 10.0),
        pt(2.0, 4.0),
    ]); // narrow root, wide end
    fan.close();
    Xf::new().translate(TAIL_ROOT.x, TAIL_ROOT.y).rotate(TAIL_DROOP - angle).map_path(&fan)
}

fn paint_leg(p: &mut Painter, hip: Pt, angle: f64) {
    let a = angle.to_radians();
    let (x, y) = (hip.x + LEG * a.sin(), hip.y + LEG * a.cos());
    p.no_brush();
    p.set_pen(pen(LEGS, 2.4));
    p.draw_line(hip, pt(x, y));
    p.set_pen(pen(LEGS, 1.8));
    p.draw_path(&polyline(&[(x - 4.5, y + 2.6), (x, y + 2.2), (x + 7.5, y + 2.6)])); // back + front toes
    p.draw_line(pt(x, y + 2.2), pt(x + 5.0, y + 3.2));
}

fn paint_wing(p: &mut Painter, raised: f64) {
    p.save();
    p.translate(6.0, -43.0); // shoulder
    p.rotate(105.0 * raised); // folded along the back -> raised (negative: down-stroke)
    let mut wing = Path::at(3.0, -3.0);
    wing.cubic_to(-10.0, -8.0, -30.0, -4.0, -42.0, 6.0); // leading edge to tip
    wing.quad_to(-36.0, 7.0, -33.0, 10.0); // long primaries...
    wing.quad_to(-29.0, 8.0, -25.0, 12.0);
    wing.quad_to(-20.0, 9.0, -15.0, 12.0);
    wing.cubic_to(-6.0, 11.0, 2.0, 5.0, 3.0, -3.0);
    p.no_pen();
    p.set_brush(WING);
    p.draw_path(&wing);
    p.set_pen(pen(FEATHER_LINE, 1.0));
    for ((x1, y1), (x2, y2)) in [((-12.0, 5.0), (-30.0, 4.0)), ((-9.0, 0.0), (-34.0, 2.0)), ((-16.0, 9.0), (-26.0, 9.0))] {
        p.draw_line(pt(x1, y1), pt(x2, y2));
    }
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&wing);
    p.restore();
}

fn paint_coin(p: &mut Painter, c: Pt, rx: f64, ry: f64, glint: f64) {
    p.set_pen(pen(GOLD_EDGE, 1.0));
    p.set_brush(GOLD);
    p.draw_ellipse(c, rx, ry);
    p.set_pen(pen(GOLD_LIGHT, 0.8));
    p.no_brush();
    p.draw_ellipse(c, rx * 0.55, ry * 0.55);
    if glint > 0.05 {
        // four-point sparkle
        let s = 2.0 + 6.0 * glint;
        let g = pt(c.x + rx * 0.4, c.y - ry * 0.7);
        let k = s * 0.22;
        p.no_pen();
        p.set_brush(WHITE);
        p.draw_polygon(&[
            pt(g.x, g.y - s),
            pt(g.x + k, g.y - k),
            pt(g.x + s, g.y),
            pt(g.x + k, g.y + k),
            pt(g.x, g.y + s),
            pt(g.x - k, g.y + k),
            pt(g.x - s, g.y),
            pt(g.x - k, g.y - k),
        ]);
    }
}

fn paint_head(p: &mut Painter, pose: &CrowPose, head: Pt, facing: f64) {
    p.save();
    p.translate_pt(head);
    p.rotate(pose.head_angle);

    p.save(); // lower mandible hinges open
    p.translate(7.0, 2.0);
    p.rotate(28.0 * pose.beak_open.min(1.0));
    let mut lower = polyline(&[(0.0, -0.4), (14.0, 0.6), (13.5, 1.8), (0.0, 3.4)]);
    lower.close();
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(BEAK);
    p.draw_path(&lower);
    p.restore();

    let mut upper = Path::at(6.0, -5.0);
    upper.cubic_to(13.0, -5.5, 19.0, -3.0, 23.0, 1.0); // heavy curved culmen
    upper.quad_to(21.0, 2.2, 18.0, 2.0);
    upper.line_to(7.0, 2.4);
    upper.quad_to(4.5, -1.0, 6.0, -5.0);
    upper.close();
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(BEAK);
    p.draw_path(&upper);
    p.set_pen(pen(BEAK_RIDGE, 0.9));
    p.draw_line(pt(9.0, -4.0), pt(19.0, -1.6));
    if pose.coin == Coin::Beak {
        paint_coin(p, pt(20.0, 3.4), 3.8, 3.2, pose.glint);
    }

    let eye = pt(1.5, -3.0);
    let (lx, ly) = (pose.look.0 * facing, pose.look.1);
    if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
        let wide = pose.eyes == Eyes::Wide;
        p.no_pen();
        p.set_brush(EYE_RING);
        p.draw_circle(eye, if wide { 3.8 } else { 3.0 });
        let (r, k) = if wide { (1.8, 1.0) } else { (2.1, 0.7) };
        let pupil = pt(eye.x + lx * k, eye.y + ly * k);
        p.set_brush(PUPIL);
        p.draw_circle(pupil, r);
        p.set_brush(WHITE);
        p.draw_circle(pt(pupil.x - 0.6, pupil.y - 0.7), 0.7);
    } else {
        let mut path = Path::new();
        let (x, y) = (eye.x, eye.y);
        match pose.eyes {
            Eyes::Closed => {
                path.move_to(x - 2.8, y);
                path.quad_to(x, y + 2.0, x + 2.8, y);
            }
            Eyes::Happy => {
                path.move_to(x - 2.8, y + 1.0);
                path.quad_to(x, y - 2.6, x + 2.8, y + 1.0);
            }
            _ => {
                path.move_to(x - 2.3, y - 2.3);
                path.line_to(x + 2.3, y + 2.3);
                path.move_to(x + 2.3, y - 2.3);
                path.line_to(x - 2.3, y + 2.3);
            }
        }
        p.no_brush();
        p.set_pen(pen(EYE_RING, 1.6)); // light strokes so they read on black feathers
        p.draw_path(&path);
    }
    p.restore();
}

pub static CHARACTER: Character = Character {
    name: "crow",
    half_width: HALF_WIDTH,
    height: HEIGHT,
    eye_y: EYE_Y,
    grip: (37.0, -52.0),
    icon_box: (22.0, -54.0, 40.0),
    size: Character::DEFAULT_SIZE,
    previews: &[
        ("peck", "peck", 3.0, 1.0),
        ("caw", "caw", CAW_TIME, 0.3),
        ("curious tilt", "tilt", 3.0, 0.5),
        ("shiny: pick up", "shiny", SHINY_TIME, 0.85),
        ("shiny: admire", "shiny", SHINY_TIME, 1.7),
        ("shiny: hide it", "shiny", SHINY_TIME, 3.95),
        ("fly", "fly", 1.0, 0.45),
    ],
    params: &Crow::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Crow, x, y, seed)),
};
