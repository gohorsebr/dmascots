//! Horse: walks, trots, gallops, grazes, whinnies, rears, swishes its tail.
//! Port of `characters/horse/{brain,art,__init__}.py`.
//! Bay horse: brown coat, black mane, tail and lower legs, white star and stripe.

use crate::art::{pen, tapered_curve};
use crate::engine::{bump, ease, keyframes, Bounds, Brain, Character, Core, Params};
use crate::gfx::{pt, Color, Painter, Path, Pt, Xf};
use crate::quadruped::{
    self, breathe, circle, default_pose_walk, foot, head_to_ground, polygon, straight, FootKind, Gait, Geometry,
    Phase, Quad, QuadArt, QuadKind, QuadParams, QuadPose, Rig, Style, FF, FN,
};

const HORSE_RIG: Rig = Rig {
    shoulder: (24.0, -62.0),
    hip: (-24.0, -62.0),
    front_leg: (30.0, 33.0),
    hind_leg: (32.0, 32.0),
    head: (60.0, -100.0),
    head_angle: 40.0,
    neck_base: (4.0, -8.0),
    tail_root: (-14.0, -8.0),
    ..Rig::BASE
};
pub const NEIGH_TIME: f64 = 1.6;
pub const REAR_TIME: f64 = 2.0;
const TROT_SPEED: f64 = 140.0;
const LINES: &[&str] = &["Neigh!", "Hrrrm.", "*snort*", "Nicker…"];

pub struct Horse {
    phase: Phase,
}

impl QuadKind for Horse {
    const PARAMS: Params = Params {
        steal_speed: 300.0,
        steal_lines: &["Neigh!", "Giddy-up!"],
        return_lines: &["*snort*", "Hrrm. Here."],
        walk_speed: 55.0,
        jump_vy: -520.0,
        dizzy_impact: 1700.0,
        lines: LINES,
        choices: &[
            (24.0, "walk", (3.0, 7.0)),
            (10.0, "trot", (2.0, 4.0)),
            (6.0, "run", (1.5, 3.0)),
            (18.0, "graze", (3.0, 7.0)),
            (8.0, "neigh", (NEIGH_TIME, NEIGH_TIME)),
            (5.0, "rear", (REAR_TIME, REAR_TIME)),
            (10.0, "swish", (1.5, 3.0)),
            (2.0, "crouch", (0.16, 0.16)),
            (12.0, "idle", (1.5, 4.0)),
        ],
        ..Params::BASE
    };
    const QUAD: QuadParams =
        QuadParams { rig: HORSE_RIG, stride: 12.0, lift: 7.0, run_speed: 340.0, ..QuadParams::BASE };

    fn phase(&self) -> &Phase {
        &self.phase
    }

    fn poke(&mut self, c: &mut Core) {
        if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy" | "rear") {
            // two separate draws, as in the Python
            let rear = c.random() < 0.3;
            let long = c.random() < 0.3;
            c.go(if rear { "rear" } else { "neigh" }, if long { REAR_TIME } else { NEIGH_TIME });
        } else {
            c.chatter(LINES);
        }
    }

    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        breathe(c, p);
        let cl = c.clock;
        p.tail_wag = 10.0 * (cl * 1.1).sin();
        p.head = (0.0, 1.5 * (cl * 0.8).sin());
        p.head_angle = 3.0 * (cl * 0.6).sin();
    }

    fn pose_walk(&self, c: &Core, p: &mut QuadPose, t: f64) {
        default_pose_walk(self, c, p, t);
        let ph = self.phase.get();
        p.head = (0.0, 2.5 * (ph * 2.0).sin()); // head nods with each step
        p.tail_wag = 8.0 * ph.sin();
    }

    fn state(&mut self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) -> bool {
        match c.state {
            "trot" => self.trot(c, dt, b, p),
            "run" => {
                quadruped::run_state(self, c, dt, b, p);
                p.head = (8.0, 6.0); // neck stretched out at the gallop
                p.head_angle = 8.0;
                p.tail = 25.0;
            }
            "graze" => self.graze(c, p),
            "neigh" => self.neigh(c, p),
            "rear" => self.rear(c, p),
            "swish" => self.swish(c, p),
            _ => return false,
        }
        true
    }
}

impl Horse {
    fn trot(&self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) {
        if c.paused || c.done() {
            self.pose_rest(c, p);
            c.go_idle();
            return;
        }
        quadruped::move_(self, c, p, b, dt, TROT_SPEED, Gait::Trot, None, None);
        p.head = (2.0, -2.0);
        p.head_angle = -6.0;
        p.ears = 1.0;
        p.tail = 10.0;
    }

    fn graze(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.8) * ease((c.duration - c.t) / 0.8);
        head_to_ground(&HORSE_RIG, p, k, 10.0, 30.0);
        p.head = (p.head.0, p.head.1 - 20.0 * k); // muzzle (not the skull) in the grass
        p.mouth = 0.2 * (c.clock * 7.0).sin().max(0.0) * k;
        p.feet[FN] = foot(6.0 * k, 0.0, false);
        p.feet[FF] = foot(2.0 * k, 0.0, false);
        if c.done() {
            c.go_idle();
        }
    }

    fn neigh(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        if c.entered() {
            let line = c.choice(&["Neeeigh!", "Neigh-hey-hey!", "Hrrrrm!"]);
            c.say(line, 1.6, false);
        }
        let k = ease(c.t / 0.25) * ease((c.duration - c.t) / 0.3);
        p.head = (-6.0 * k, -10.0 * k);
        p.head_angle = -25.0 * k;
        p.mouth = if 0.2 < c.t && c.t < 1.25 { (0.55 + 0.3 * (c.clock * 34.0).sin()) * k } else { 0.0 };
        p.ears = 1.0;
        p.tail = 20.0 * k;
        if c.done() {
            c.go_idle();
        }
    }

    /// Up on the hind legs, front hooves paddling the air.
    fn rear(&self, c: &mut Core, p: &mut QuadPose) {
        breathe(c, p);
        if c.entered() {
            c.say("NEIGH!", 1.4, false);
        }
        let t = c.t;
        let [k] = keyframes(t, &[(0.0, [0.0]), (0.45, [1.0]), (1.4, [1.0]), (REAR_TIME, [0.0])]);
        p.shoulder = (-20.0 * k, -34.0 * k);
        p.hip = (6.0 * k, 4.0 * k);
        p.head = (-24.0 * k, -38.0 * k);
        p.head_angle = -30.0 * k;
        let paddle = (c.clock * 9.0).sin();
        if k > 0.15 {
            p.feet = [
                foot(6.0 + 6.0 * paddle, 18.0 * k, true),
                foot(8.0 - 6.0 * paddle, 16.0 * k, true),
                foot(14.0 * k, 0.0, false),
                foot(12.0 * k, 0.0, false),
            ];
        }
        p.mouth = 0.6 * bump(t, 0.7, 0.8);
        p.tail = 30.0 * k;
        p.ears = -0.3;
        if c.done() {
            c.go_idle();
        }
    }

    fn swish(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        p.tail_wag = 45.0 * (c.clock * 7.0).sin();
        let toss = bump(c.t.rem_euclid(1.5), 0.5, 0.5);
        p.head_angle = -18.0 * toss;
        p.head = (0.0, -4.0 * toss);
        if c.done() {
            c.go_idle();
        }
    }
}

// =================================================================== art

const MANE: Color = Color::hex("#1E1512");
const MANE_LINE: Color = Color::hex("#46352C");
const MUZZLE: Color = Color::hex("#5E3219");
const WHITE: Color = Color::hex("#F6F2EA");
const MOUTH: Color = Color::hex("#3A1A14");

const EARS: [[(f64, f64); 3]; 2] =
    [[(-6.0, -6.0), (-13.0, -19.0), (-1.5, -9.0)], [(-2.0, -8.0), (-7.5, -21.0), (3.0, -8.5)]];

const HORSE_STYLE: Style = Style {
    coat: Color::hex("#8B4A22"),
    shade: Color::hex("#6C3618"),
    outline: Color::hex("#2A140A"),
    chest_r: 17.0,
    hip_r: 17.5,
    neck_w: (34.0, 18.0),
    front_w: (14.0, 7.0, 6.5),
    hind_w: (17.0, 7.5, 6.5),
    foot: FootKind::Hoof,
    hoof_color: Color::hex("#2B2522"),
    line_w: 1.8,
    ..Style::BASE
};
const HEAD_SCALE: f64 = 1.18;

impl QuadArt for Horse {
    fn style(&self) -> &Style {
        &HORSE_STYLE
    }
    fn rig(&self) -> &Rig {
        &HORSE_RIG
    }

    fn head_transform(&self, g: &Geometry) -> Xf {
        Xf::new().translate(g.head.x, g.head.y).rotate(g.head_angle).scale(HEAD_SCALE, HEAD_SCALE)
    }

    fn head_shape(&self, pose: &QuadPose, g: &Geometry) -> Path {
        let mut head = circle(pt(0.0, 0.0), 10.5).united(&circle(pt(2.0, 5.0), 9.5));
        head = head.united(&straight(pt(4.0, 2.0), pt(25.0, 3.5), 18.0, 12.5));
        head = head.united(&circle(pt(25.0, 4.0), 6.6));
        for ear in EARS {
            let path = polygon(&ear.map(|(x, y)| pt(x, y)));
            let back = -25.0 * (-pose.ears).max(0.0) + 8.0 * pose.ears.max(0.0);
            let t = Xf::new().translate(ear[0].0, ear[0].1).rotate(back).translate(-ear[0].0, -ear[0].1);
            head = head.united(&t.map_path(&path));
        }
        self.head_transform(g).map_path(&head)
    }

    fn paint_tail(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry) {
        let r = g.tail_root;
        let lift = pose.tail.to_radians();
        let swish = 12.0 * (pose.tail_wag.to_radians() * 2.0).sin();
        let up = 22.0 * lift.sin();
        let p0 = (r.x, r.y);
        let p1 = (r.x - 13.0, r.y - 2.0 - up * 0.6);
        let p2 = (r.x - 22.0 + swish * 0.5, r.y + 18.0 - up);
        let tip = (r.x - 16.0 + swish, r.y + 46.0 - up * 1.4);
        let tail = tapered_curve(p0, p1, p2, tip, 9.0, 13.0, 20, 1.0);
        p.set_pen(pen(HORSE_STYLE.outline, HORSE_STYLE.line_w));
        p.set_brush(MANE);
        p.draw_path(&tail);
        p.set_pen(pen(MANE_LINE, 1.0));
        p.no_brush();
        for off in [-3.0, 0.0, 3.0] {
            // a few strands of hair
            let mut strand = Path::at(p1.0, p1.1 + 4.0);
            strand.cubic_to(p1.0 - 4.0 + off, p1.1 + 10.0, p2.0 + off, p2.1 + 6.0, tip.0 + off * 1.5, tip.1 - 6.0);
            p.draw_path(&strand);
        }
    }

    fn paint_markings(&self, p: &mut Painter, _pose: &QuadPose, g: &Geometry) {
        p.no_pen();
        p.set_brush(MANE);
        p.draw_rect(-300.0, -21.0, 600.0, 30.0); // black "points" on the lower legs
        p.save();
        p.combine_transform(&self.head_transform(g));
        p.set_brush(MUZZLE);
        p.draw_ellipse(pt(25.0, 4.5), 8.0, 7.5);
        p.set_brush(WHITE);
        p.draw_ellipse(pt(2.0, -6.0), 3.4, 2.6); // star
        p.draw_path(&straight(pt(6.0, -5.5), pt(21.0, -1.5), 3.2, 2.0)); // stripe
        p.restore();
    }

    fn paint_far_markings(&self, p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {
        p.no_pen();
        p.set_brush(MANE);
        p.draw_rect(-300.0, -21.0, 600.0, 30.0);
    }

    /// Mane along the crest of the neck (following its curve), plus a forelock.
    fn paint_over(&self, p: &mut Painter, _pose: &QuadPose, g: &Geometry) {
        let spine = self.neck_spine(g);
        let ((x0, y0), (x3, y3)) = (spine[0], spine[3]);
        let n = (x3 - x0).hypot(y3 - y0);
        let n = if n == 0.0 { 1.0 } else { n };
        let (mut ux, mut uy) = ((y3 - y0) / n, -(x3 - x0) / n); // perpendicular to the neck...
        if ux > 0.0 {
            ux = -ux; // ...on the crest (back/top) side
            uy = -uy;
        }
        let nw = HORSE_STYLE.neck_w;
        let offs = [nw.0 * 0.42, nw.0 * 0.34, nw.1 * 0.45, nw.1 * 0.35];
        let mut ctrl = [(0.0, 0.0); 4];
        for i in 0..4 {
            ctrl[i] = (spine[i].0 + ux * offs[i], spine[i].1 + uy * offs[i]);
        }
        ctrl[3] = (ctrl[3].0 - 2.0, ctrl[3].1 - 3.0);
        let mane = tapered_curve(ctrl[0], ctrl[1], ctrl[2], ctrl[3], 10.0, 6.0, 16, 1.0);
        p.set_pen(pen(HORSE_STYLE.outline, 1.2));
        p.set_brush(MANE);
        p.draw_path(&mane);
        p.set_pen(pen(MANE_LINE, 0.9));
        for f in [0.2, 0.4, 0.6, 0.8] {
            let u: f64 = 1.0 - f;
            let bez = |i: usize| -> f64 {
                let c = |k: usize| if i == 0 { ctrl[k].0 } else { ctrl[k].1 };
                u.powi(3) * c(0) + 3.0 * u * u * f * c(1) + 3.0 * u * f * f * c(2) + f.powi(3) * c(3)
            };
            let (x, y) = (bez(0), bez(1));
            p.draw_line(pt(x + ux * 3.0, y + uy * 3.0), pt(x - ux * 3.0, y - uy * 3.0));
        }
    }

    fn paint_head(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry, facing: f64) {
        let outline = HORSE_STYLE.outline;
        p.save();
        p.combine_transform(&self.head_transform(g));
        p.set_pen(pen(outline, 1.0));
        p.set_brush(MANE);
        p.draw_polygon(&[pt(-7.0, -9.0), pt(-1.0, -10.0), pt(3.0, -4.0), pt(-1.0, -2.5)]); // forelock
        p.no_pen();
        p.set_brush(Color::hex("#1A0E09"));
        p.draw_ellipse(pt(28.0, 0.8), 1.5, 2.3); // nostril
        if pose.mouth > 0.05 {
            let lip = Xf::new().translate(19.0, 8.0).rotate(22.0 * pose.mouth);
            p.set_brush(MOUTH);
            p.draw_polygon(&[pt(19.0, 8.0), pt(30.5, 7.0), lip.map(pt(10.0, 1.0))]);
            p.set_pen(pen(outline, 1.1));
            p.set_brush(MUZZLE);
            p.draw_polygon(&[pt(0.0, 0.0), pt(10.0, 1.0), pt(9.5, 3.5), pt(0.0, 3.5)].map(|q: Pt| lip.map(q)));
        } else {
            p.set_pen(pen(outline, 1.1));
            p.draw_line(pt(30.0, 7.5), pt(22.0, 8.5));
        }
        self.paint_eye(p, pose, pt(3.0, -3.0), facing, 2.4, None, false, Some(WHITE));
        p.restore();
    }
}

pub static CHARACTER: Character = Character {
    name: "horse",
    half_width: 88.0,
    height: 130.0,
    eye_y: 102.0,
    grip: (82.0, -68.0),
    icon_box: (70.0, -95.0, 72.0),
    size: (300, 280),
    previews: &[
        ("trot", "trot", 4.0, 0.3),
        ("gallop", "run", 3.0, 0.8),
        ("graze", "graze", 5.0, 2.0),
        ("neigh", "neigh", NEIGH_TIME, 0.6),
        ("rear", "rear", REAR_TIME, 0.9),
        ("tail swish", "swish", 3.0, 0.5),
    ],
    params: &Horse::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Quad(Horse { phase: Phase::default() }), x, y, seed)),
};
