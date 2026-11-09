//! Goose: waddles, grazes, honks, preens, charges at the cursor.
//! Port of `characters/goose/{brain,art,__init__}.py`.
//!
//! Body, tapered neck and head are unioned into one silhouette so the outline
//! flows around the whole bird; shading, wing and face are layered on top.

use crate::art::{bubble, paint_stars, pen, stroke_with, tapered};
use crate::engine::{bump, ease, lerp2, Bounds, Brain, Character, Core, Eyes, Params, Species};
use crate::gfx::{polyline, pt, Color, Painter, Path, Pt};
use crate::pose;

const REST_HEAD: (f64, f64) = (19.0, -78.0);
const CHASE_SPEED: f64 = 150.0;
pub const HONK_TIME: f64 = 1.1;

pose!(GoosePose {
    head: (f64, f64) = REST_HEAD, // head centre, relative to the feet (facing +x)
    head_angle: f64 = 0.0,        // degrees, positive tips the beak down
    neck_bend: f64 = 0.0,         // extra bulge of the neck curve
    beak_open: f64 = 0.0,         // 0..1
    wing: f64 = 0.0,              // 0 folded .. 1 raised
    leg_front: f64 = 4.0,         // leg angles, degrees, positive = forward
    leg_back: f64 = -4.0,
    tail: f64 = 0.0,              // wag, -1..1
});

pub struct Goose;

const LINES: &[&str] = &["HONK!", "Honk?", "HONK HONK!", "Hssss!"];

impl Species for Goose {
    type Pose = GoosePose;
    const PARAMS: Params = Params {
        steal_speed: 200.0,
        steal_lines: &["HONK!", "Mine now.", "HONK HONK!"],
        return_lines: &["Hmph. Take it.", "Honk."],
        walk_speed: 55.0,
        jump_vy: -620.0,
        lines: LINES,
        choices: &[
            (38.0, "walk", (3.0, 7.0)),
            (18.0, "graze", (2.5, 5.0)),
            (10.0, "honk", (HONK_TIME, HONK_TIME)),
            (9.0, "preen", (2.0, 3.5)),
            (8.0, "chase", (2.5, 4.5)),
            (5.0, "crouch", (0.16, 0.16)),
            (14.0, "idle", (1.5, 4.0)),
        ],
        ..Params::BASE
    };

    /// Geese don't hop when poked. They honk back.
    fn poke(&mut self, c: &mut Core) {
        if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy") {
            c.go("honk", HONK_TIME);
        } else {
            c.chatter(LINES);
        }
    }

    // ----------------------------------------------------------- shared-state poses
    fn pose_rest(&self, c: &Core, p: &mut GoosePose) {
        let s = (c.clock * 1.8).sin();
        p.bob = s * 0.5;
        p.head = (REST_HEAD.0 + 0.8 * (c.clock * 0.9).sin(), REST_HEAD.1 + 0.8 * s);
        p.tail = (c.clock * 1.1).sin() * 0.5;
    }

    fn pose_walk(&self, _c: &Core, p: &mut GoosePose, t: f64) {
        let ph = t * 8.0;
        let s = ph.sin();
        p.leg_front = 22.0 * s;
        p.leg_back = -22.0 * s;
        p.lean = 2.5 * (ph * 2.0).sin(); // rocking waddle
        p.head = (21.0 + 2.5 * (ph * 2.0).cos(), -77.0 + 1.2 * (ph * 2.0).sin());
        p.tail = s;
    }

    fn pose_crouch(&self, c: &Core, p: &mut GoosePose, k: f64) {
        self.pose_rest(c, p);
        p.wing = 0.35 * k;
        p.head = lerp2(p.head, (22.0, -72.0), k);
    }

    fn pose_jump(&self, c: &Core, p: &mut GoosePose) {
        p.wing = 0.65 + 0.35 * (c.clock * 30.0).sin();
        p.head = (24.0, -80.0);
        p.beak_open = 0.4;
        p.leg_front = -20.0;
        p.leg_back = -35.0;
    }

    fn pose_fall(&self, c: &Core, p: &mut GoosePose) {
        let c = c.clock;
        p.wing = 0.6 + 0.4 * (c * 34.0).sin();
        p.head = (25.0, -75.0);
        p.beak_open = 0.7 + 0.3 * (c * 9.0).sin();
        p.leg_front = 12.0 * (c * 15.0).sin();
        p.leg_back = -12.0 * (c * 15.0).sin();
    }

    fn pose_land(&self, c: &Core, p: &mut GoosePose, bend: f64) {
        self.pose_rest(c, p);
        p.wing = 0.6 * bend;
        p.head = lerp2(p.head, (24.0, -70.0), bend);
    }

    fn pose_dizzy(&self, c: &Core, p: &mut GoosePose) {
        let c = c.clock;
        p.head = (REST_HEAD.0 + 3.0 * (c * 4.0).cos(), REST_HEAD.1 + 2.0 * (c * 4.0).sin());
        p.head_angle = 12.0 * (c * 4.0).sin();
        p.beak_open = 0.25;
        p.wing = 0.1;
    }

    fn pose_dragged(&self, c: &Core, p: &mut GoosePose) {
        let c = c.clock;
        p.wing = 0.5 + 0.5 * (c * 25.0).sin().abs();
        p.head = (16.0, -84.0);
        p.head_angle = -15.0;
        p.beak_open = (c * 6.0).sin().abs();
        p.leg_front = 25.0 * (c * 10.0).sin();
        p.leg_back = -25.0 * (c * 10.0).sin();
        p.tail = (c * 12.0).sin();
    }

    // ----------------------------------------------------------- goose states
    fn state(&mut self, c: &mut Core, dt: f64, b: &Bounds, p: &mut GoosePose) -> bool {
        match c.state {
            "graze" => self.graze(c, p),
            "honk" => self.honk(c, p),
            "preen" => self.preen(c, p),
            "chase" => self.chase(c, dt, b, p),
            _ => return false,
        }
        true
    }

    fn paint(&self, p: &mut Painter, pose: &GoosePose, facing: f64, head_only: bool) {
        paint(p, pose, facing, head_only);
    }
}

impl Goose {
    fn graze(&self, c: &mut Core, p: &mut GoosePose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.4) * ease((c.duration - c.t) / 0.4);
        let peck = (c.clock * 9.0).sin().max(0.0).powi(4);
        p.head = lerp2(p.head, (40.0, -12.0 + 5.0 * peck), k);
        p.head_angle = 75.0 * k;
        p.beak_open = 0.35 * peck * k;
        p.tail = (c.clock * 3.0).sin() * 0.6;
        if c.done() {
            c.go_idle();
        }
    }

    fn honk(&self, c: &mut Core, p: &mut GoosePose) {
        self.pose_rest(c, p);
        if c.entered() {
            let line = c.choice(&["HONK!", "HONK HONK!"]);
            c.say(line, 1.4, false);
        }
        let k = ease(c.t / 0.15) * ease((c.duration - c.t) / 0.2);
        p.head = lerp2(p.head, (32.0, -70.0), k);
        p.head_angle = -18.0 * k;
        p.neck_bend = -4.0 * k;
        p.beak_open = bump(c.t, 0.3, 0.35) + bump(c.t, 0.75, 0.35);
        p.wing = 0.25 * k;
        p.tail = (c.clock * 14.0).sin() * k;
        if c.done() {
            c.go_idle();
        }
    }

    fn preen(&self, c: &mut Core, p: &mut GoosePose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.35) * ease((c.duration - c.t) / 0.35);
        let cl = c.clock;
        p.head = lerp2(p.head, (-6.0 + 3.0 * (cl * 14.0).sin(), -56.0 + 2.0 * (cl * 14.0).cos()), k);
        p.head_angle = 150.0 * k;
        p.wing = k * (0.12 + 0.05 * (cl * 14.0).sin());
        if k > 0.8 {
            p.eyes = Eyes::Closed;
        }
        if c.done() {
            c.go_idle();
        }
    }

    fn chase(&self, c: &mut Core, dt: f64, b: &Bounds, p: &mut GoosePose) {
        let dx = c.cursor.map_or(0.0, |cur| cur.0);
        if c.paused || c.done() || c.cursor.is_none() {
            self.pose_rest(c, p);
            c.go_idle();
            return;
        }
        if dx.abs() < 45.0 {
            // caught up with the cursor: tell it off
            self.pose_rest(c, p);
            c.go("honk", HONK_TIME);
            return;
        }
        if c.entered() {
            c.say("Hssss!", 1.0, false);
        }
        c.facing = if dx > 0.0 { 1.0 } else { -1.0 };
        c.step_x(b, c.facing * CHASE_SPEED * dt);
        self.pose_walk(c, p, c.t * 1.9);
        let cl = c.clock;
        p.head = (31.0 + 1.5 * (cl * 30.0).sin(), -67.0); // neck stretched out low, menacing
        p.head_angle = 6.0;
        p.neck_bend = 3.0;
        p.beak_open = 0.35 + 0.25 * (cl * 20.0).sin();
        p.wing = 0.35 + 0.08 * (cl * 18.0).sin();
        p.lean = 6.0;
    }
}

// =================================================================== art

const FEATHER: Color = Color::hex("#FCFCF8");
const SHADE: Color = Color::hex("#E4E4DB");
const WING: Color = Color::hex("#F1F1EA");
const WING_SHADE: Color = Color::hex("#DADAD0");
const WING_LINE: Color = Color::hex("#A8A89D");
const OUTLINE: Color = Color::hex("#343434");
const ORANGE: Color = Color::hex("#F59B2A");
const ORANGE_DARK: Color = Color::hex("#CF7417");
const NAIL: Color = Color::hex("#A85B12");
const EYE: Color = Color::hex("#151515");
const WHITE: Color = Color::hex("#FFFFFF");

const LINE_W: f64 = 1.8;
const HEAD_R: f64 = 9.8;
const NECK_BASE: Pt = pt(12.0, -42.0);
const NECK_BASE_W: f64 = 24.0; // thick where it flows into the breast...
const NECK_TOP_W: f64 = 12.0; // ...slender under the head
const PIVOT: Pt = pt(0.0, -54.0); // grab point when dangling from the cursor
const HIP_BACK: Pt = pt(-7.0, -22.0);
const HIP_FRONT: Pt = pt(1.0, -22.0);
const LEG: f64 = 19.0;

// Extents used by the window for layout / screen-edge collisions
const HALF_WIDTH: f64 = 50.0;
const HEIGHT: f64 = 92.0;
const EYE_Y: f64 = 81.0;

pub fn paint(p: &mut Painter, pose: &GoosePose, facing: f64, head_only: bool) {
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
    if pose.grounded && !head_only {
        let drop = pose.leg_front.to_radians().cos().max(pose.leg_back.to_radians().cos()) * LEG;
        p.translate(0.0, LEG - drop);
    }

    let head = Pt::from(pose.head);
    let body = if head_only { None } else { Some(body_path(pose.tail)) };
    let mut skull = Path::new();
    skull.add_ellipse(head, HEAD_R, HEAD_R);
    // boolean union (not add_path): the pieces wind in different directions
    let mut sil = neck_path(head, pose.neck_bend).united(&skull);
    if let Some(body) = &body {
        sil = sil.united(body);
    }

    if !head_only {
        for (hip, angle, color) in [(HIP_BACK, pose.leg_back, ORANGE_DARK), (HIP_FRONT, pose.leg_front, ORANGE)] {
            let a = angle.to_radians();
            let foot = pt(hip.x + LEG * a.sin(), hip.y + LEG * a.cos());
            stroke_with(p, &polyline(&[(hip.x, hip.y), (foot.x, foot.y)]), color, 3.6, OUTLINE, 1.1);
            paint_foot(p, foot, color);
        }
    }

    p.no_pen();
    p.set_brush(FEATHER);
    p.draw_path(&sil);
    if let Some(body) = &body {
        // soft shading under the belly and tail
        p.save();
        p.set_clip_path(body);
        p.set_brush(SHADE);
        p.draw_ellipse(pt(-8.0, -16.0), 34.0, 10.0);
        p.draw_ellipse(pt(-38.0, -36.0), 10.0, 9.0);
        p.restore();
    }
    p.set_pen(pen(OUTLINE, LINE_W));
    p.no_brush();
    p.draw_path(&sil);

    if !head_only {
        paint_wing(p, pose.wing);
    }
    paint_head(p, pose, head, facing);
    if let Some(stars) = pose.stars {
        paint_stars(p, head.x, head.y - 16.0, stars, 17.0, 4.0);
    }
    p.restore();

    if let (Some(text), false) = (&pose.bubble, head_only) {
        bubble(p, text, facing * pose.head.0, pose.head.1 - 13.0 + pose.bob, facing, OUTLINE);
    }
}

/// The neck: a cubic spine from the breast to the head, swept with a tapering width.
fn neck_path(head: Pt, bend: f64) -> Path {
    let p0 = (NECK_BASE.x, NECK_BASE.y);
    tapered(
        p0,
        (p0.0 + 6.0 + bend, p0.1 - 16.0),
        (head.x - 4.0 + bend * 0.5, head.y + 14.0),
        (head.x, head.y),
        NECK_BASE_W,
        NECK_TOP_W,
    )
}

/// Pear-shaped body: deep breast up front, belly, upturned tail at the back.
fn body_path(tail: f64) -> Path {
    let mut path = Path::at(18.0, -50.0);
    path.cubic_to(29.0, -42.0, 26.0, -22.0, 6.0, -18.0); // breast
    path.cubic_to(-10.0, -15.0, -27.0, -21.0, -33.0, -32.0); // belly
    path.line_to(-44.0, -50.0 + 3.0 * tail); // tail tip
    path.cubic_to(-35.0, -51.0, -26.0, -55.0, -12.0, -55.0); // back
    path.cubic_to(0.0, -55.0, 10.0, -55.0, 18.0, -50.0);
    path.close();
    path
}

fn paint_foot(p: &mut Painter, ankle: Pt, color: Color) {
    let (x, y) = (ankle.x, ankle.y);
    let mut web = Path::at(x - 3.0, y - 1.4);
    web.line_to(x + 4.0, y - 2.4);
    web.quad_to(x + 10.0, y - 1.8, x + 13.5, y + 0.2); // outer toe
    web.quad_to(x + 11.5, y + 1.0, x + 12.5, y + 2.2); // webbing scallop
    web.line_to(x - 3.5, y + 2.2);
    web.close();
    p.set_pen(pen(OUTLINE, 1.1));
    p.set_brush(color);
    p.draw_path(&web);
    p.set_pen(pen(NAIL, 0.8));
    p.draw_line(pt(x + 1.0, y - 0.2), pt(x + 10.0, y + 0.6));
}

fn paint_wing(p: &mut Painter, raised: f64) {
    p.save();
    p.translate(2.0, -43.0); // shoulder
    p.rotate(105.0 * raised); // folded along the back -> raised up and back
    let mut wing = Path::at(4.0, -2.0);
    wing.cubic_to(-8.0, -10.0, -26.0, -9.0, -38.0, 2.0); // leading edge to tip
    wing.quad_to(-33.0, 3.0, -31.0, 7.0); // primaries...
    wing.quad_to(-27.0, 5.0, -24.0, 10.0);
    wing.quad_to(-19.0, 7.0, -15.0, 11.0);
    wing.cubic_to(-6.0, 12.0, 2.0, 6.0, 4.0, -2.0);
    p.no_pen();
    p.set_brush(WING);
    p.draw_path(&wing);
    p.save();
    p.set_clip_path(&wing);
    p.set_brush(WING_SHADE);
    p.draw_ellipse(pt(-22.0, 12.0), 20.0, 6.0);
    p.restore();
    p.set_pen(pen(WING_LINE, 1.1));
    p.no_brush();
    for ((x1, y1), (x2, y2)) in [((-12.0, 6.0), (-27.0, 4.0)), ((-9.0, 1.0), (-30.0, 0.0)), ((-16.0, 9.0), (-23.0, 8.0))] {
        p.draw_line(pt(x1, y1), pt(x2, y2));
    }
    p.set_pen(pen(OUTLINE, LINE_W));
    p.draw_path(&wing);
    p.restore();
}

fn paint_head(p: &mut Painter, pose: &GoosePose, head: Pt, facing: f64) {
    p.save();
    p.translate_pt(head);
    p.rotate(pose.head_angle);

    p.save(); // lower mandible hinges open
    p.translate(5.5, 1.8);
    p.rotate(30.0 * pose.beak_open.min(1.0));
    let mut lower = Path::at(0.0, -0.6);
    lower.quad_to(7.0, 1.2, 13.5, 0.4);
    lower.line_to(13.0, 1.8);
    lower.quad_to(6.0, 3.8, 0.0, 3.2);
    lower.close();
    p.set_pen(pen(OUTLINE, 1.2));
    p.set_brush(ORANGE_DARK);
    p.draw_path(&lower);
    p.restore();

    p.set_pen(pen(OUTLINE, 1.2));
    p.set_brush(ORANGE);
    p.draw_ellipse(pt(5.6, -5.4), 2.7, 2.2); // knob
    let mut upper = Path::at(4.5, -5.6);
    upper.cubic_to(10.0, -5.0, 16.0, -2.6, 20.0, 1.2); // ridge curving to the tip
    upper.quad_to(19.6, 2.5, 17.5, 2.3);
    upper.cubic_to(12.0, 2.2, 8.0, 2.5, 4.5, 2.7);
    upper.quad_to(3.0, -1.5, 4.5, -5.6);
    upper.close();
    p.draw_path(&upper);
    p.no_pen();
    p.set_brush(NAIL);
    p.draw_ellipse(pt(18.8, 1.3), 1.5, 1.1); // nail at the tip
    p.draw_ellipse(pt(10.5, -2.3), 1.0, 0.6); // nostril

    let eye = pt(1.2, -3.2);
    let (lx, ly) = (pose.look.0 * facing, pose.look.1);
    if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
        let (r, k) = if pose.eyes == Eyes::Wide {
            p.set_pen(pen(OUTLINE, 1.0));
            p.set_brush(WHITE);
            p.draw_circle(eye, 3.4);
            (1.7, 0.9)
        } else {
            (2.0, 0.7)
        };
        let pupil = pt(eye.x + lx * k, eye.y + ly * k);
        p.no_pen();
        p.set_brush(EYE);
        p.draw_circle(pupil, r);
        p.set_brush(WHITE);
        p.draw_circle(pt(pupil.x - 0.6, pupil.y - 0.7), 0.7);
    } else {
        let mut path = Path::new();
        let (x, y) = (eye.x, eye.y);
        match pose.eyes {
            Eyes::Closed => {
                path.move_to(x - 2.6, y);
                path.quad_to(x, y + 2.0, x + 2.6, y);
            }
            Eyes::Happy => {
                path.move_to(x - 2.6, y + 1.0);
                path.quad_to(x, y - 2.6, x + 2.6, y + 1.0);
            }
            _ => {
                // dizzy
                path.move_to(x - 2.2, y - 2.2);
                path.line_to(x + 2.2, y + 2.2);
                path.move_to(x + 2.2, y - 2.2);
                path.line_to(x - 2.2, y + 2.2);
            }
        }
        p.no_brush();
        p.set_pen(pen(EYE, 1.5));
        p.draw_path(&path);
    }
    p.restore();
}

pub static CHARACTER: Character = Character {
    name: "goose",
    half_width: HALF_WIDTH,
    height: HEIGHT,
    eye_y: EYE_Y,
    grip: (36.0, -76.0),
    icon_box: (24.0, -76.0, 42.0),
    size: Character::DEFAULT_SIZE,
    previews: &[
        ("graze", "graze", 4.0, 1.0),
        ("honk", "honk", HONK_TIME, 0.3),
        ("preen", "preen", 3.0, 1.2),
        ("chase the cursor", "chase", 4.0, 0.3),
    ],
    params: &Goose::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Goose, x, y, seed)),
};
