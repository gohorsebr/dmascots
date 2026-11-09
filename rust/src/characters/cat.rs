//! Cat: walks tail-up, sits, grooms, loafs, stretches, and stalks + pounces on the cursor.
//! Port of `characters/cat/{brain,art,__init__}.py`. Orange tabby.

use std::cell::Cell;

use crate::art::{pen, tapered_curve};
use crate::engine::{bump, clamp, ease, Bounds, Brain, Character, Core, Eyes, Params};
use crate::gfx::{pt, Color, Painter, Path, Pen, Pt, Xf};
use crate::quadruped::{
    self, bow, breathe, circle, default_pose_dragged, default_pose_walk, foot, polygon, sit, Geometry, Phase,
    Quad, QuadArt, QuadKind, QuadParams, QuadPose, Rig, Style, FN,
};

const CAT_RIG: Rig = Rig {
    shoulder: (11.0, -21.0),
    hip: (-11.0, -22.0),
    front_leg: (10.5, 11.0),
    hind_leg: (11.5, 11.5),
    head: (21.0, -33.0),
    neck_base: (2.0, -5.0),
    tail_root: (-6.0, -3.0),
    foot_y: -1.6,
    ..Rig::BASE
};
pub const MEOW_TIME: f64 = 0.9;
const LINES: &[&str] = &["Meow.", "Mrrp?", "Mew!", "Prrr…"];

pub struct Cat {
    phase: Phase,
    wiggle_at: Cell<Option<f64>>,
}

impl QuadKind for Cat {
    const PARAMS: Params = Params {
        steal_speed: 250.0,
        steal_lines: &["Mine.", "Mrrp!", "*knocks it over*"],
        return_lines: &["Bored now.", "…fine."],
        walk_speed: 45.0,
        jump_vy: -560.0,
        lines: LINES,
        choices: &[
            (22.0, "walk", (2.0, 5.0)),
            (14.0, "sit", (3.0, 6.0)),
            (10.0, "groom", (3.0, 5.0)),
            (10.0, "nap", (6.0, 12.0)),
            (6.0, "stretch", (2.0, 2.0)),
            (9.0, "pounce", (6.0, 6.0)),
            (5.0, "run", (1.2, 2.2)),
            (4.0, "meow", (MEOW_TIME, MEOW_TIME)),
            (4.0, "crouch", (0.14, 0.14)),
            (12.0, "idle", (1.5, 4.0)),
        ],
        ..Params::BASE
    };
    const QUAD: QuadParams = QuadParams { rig: CAT_RIG, stride: 6.0, lift: 3.5, run_speed: 240.0, ..QuadParams::BASE };

    fn phase(&self) -> &Phase {
        &self.phase
    }

    fn poke(&mut self, c: &mut Core) {
        if c.state == "nap" {
            c.say("…?", 1.6, false);
            c.go_idle();
        } else if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy" | "pounce") {
            c.go("meow", MEOW_TIME);
        } else {
            c.chatter(LINES);
        }
    }

    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        breathe(c, p);
        let cl = c.clock;
        p.tail = 15.0 + 10.0 * (cl * 0.9).sin();
        p.tail_curl = 0.5 + 0.3 * (cl * 1.3).sin();
        p.tail_wag = 6.0 * (cl * 1.7).sin();
        if (cl * 0.7).sin() > 0.97 {
            p.ears = -0.8; // the occasional ear flick
        }
    }

    fn pose_walk(&self, c: &Core, p: &mut QuadPose, t: f64) {
        default_pose_walk(self, c, p, t);
        p.tail = 60.0; // tail up: a happy cat
        p.tail_curl = 0.7;
        p.tail_wag = 4.0 * (t * 3.0).sin();
    }

    fn pose_dragged(&self, c: &Core, p: &mut QuadPose) {
        default_pose_dragged(c, p);
        p.tail_curl = 0.2;
    }

    fn state(&mut self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) -> bool {
        match c.state {
            "sit" => {
                quadruped::sit_state(self, c, p);
                let k = ease(c.t / 0.4);
                p.tail = -8.0 * k + p.tail * (1.0 - k); // tail along the floor, tip hooked up
                p.tail_curl = k;
                let ph = c.t.rem_euclid(3.0);
                if 0.4 < ph && ph < 0.75 {
                    p.eyes = Eyes::Happy; // slow blink: cat for "I like you"
                }
            }
            "nap" => {
                quadruped::nap_state(self, c, p);
                p.tail = -5.0;
                p.tail_curl = 1.0;
            }
            "groom" => self.groom(c, p),
            "stretch" => self.stretch(c, p),
            "meow" => self.meow(c, p),
            "pounce" => self.pounce(c, dt, b, p),
            _ => return false,
        }
        true
    }
}

impl Cat {
    /// Sit, lick a paw, wipe the face with it.
    fn groom(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.4) * ease((c.duration - c.t) / 0.4);
        sit(&CAT_RIG, p, k);
        p.tail = -8.0 * k;
        p.tail_curl = k;
        let lick = (c.clock * 9.0).sin();
        let wipe = c.t.rem_euclid(2.0) > 1.2; // alternate: lick the paw, then wipe the face
        if k > 0.5 {
            p.feet[FN] = foot(9.0, 21.0 + if !wipe { 3.0 * lick } else { 6.0 + 4.0 * lick }, true);
        }
        p.head = (p.head.0 - 4.0 * k, p.head.1 + if !wipe { 4.0 } else { 1.0 } * k);
        p.head_angle = (if !wipe { 30.0 } else { 10.0 + 8.0 * lick }) * k;
        p.mouth = if !wipe { 0.3 * lick.max(0.0) * k } else { 0.0 };
        p.tongue = !wipe && k > 0.5;
        p.eyes = if k > 0.6 { Eyes::Closed } else { Eyes::Open };
        if c.done() {
            c.go_idle();
        }
    }

    fn stretch(&self, c: &mut Core, p: &mut QuadPose) {
        breathe(c, p);
        let k = ease(c.t / 0.6) * ease((c.duration - c.t) / 0.5);
        bow(&CAT_RIG, p, k);
        let yawn = bump(c.t, 1.0, 0.8);
        p.mouth = yawn;
        p.eyes = if yawn > 0.3 { Eyes::Closed } else { Eyes::Open };
        p.ears = -0.5 * yawn;
        p.tail = 50.0 * k;
        p.tail_curl = 0.3;
        if c.done() {
            c.go_idle();
        }
    }

    fn meow(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        if c.entered() {
            let line = c.choice(LINES);
            c.say(line, 1.2, false);
        }
        let m = bump(c.t, 0.35, 0.5);
        p.mouth = 0.8 * m;
        p.head = (1.0, -2.0 * m);
        p.head_angle = -15.0 * m;
        p.tail = 45.0;
        if c.done() {
            c.go_idle();
        }
    }

    /// Stalk the cursor low and slow, wiggle the hips, leap at it.
    fn pounce(&self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) {
        let Some((dx, _)) = c.cursor.filter(|_| !c.paused) else {
            self.pose_rest(c, p);
            c.go_idle();
            return;
        };
        if c.entered() {
            self.wiggle_at.set(None);
        }
        let Some(wiggle_at) = self.wiggle_at.get() else {
            let left = quadruped::stalk(self, c, p, b, dt, 28.0);
            p.tail = -5.0;
            p.tail_curl = 0.2;
            p.tail_wag = 10.0 * (c.clock * 8.0).sin(); // twitchy tail tip
            if left < 110.0 || c.t > 3.5 {
                self.wiggle_at.set(Some(c.t));
            }
            return;
        };
        // hips wiggle, then launch
        let w = c.t - wiggle_at;
        let r = &CAT_RIG;
        p.shoulder = (0.0, 0.4 * -r.shoulder.1);
        p.hip = (2.0 * (c.clock * 30.0).sin(), 0.3 * -r.hip.1 - 2.0);
        p.head = (4.0, 0.3 * -r.shoulder.1);
        p.ears = 1.0;
        p.eyes = Eyes::Wide;
        p.tail = -5.0;
        p.tail_wag = 14.0 * (c.clock * 30.0).sin();
        if w > 0.6 {
            c.facing = if dx >= 0.0 { 1.0 } else { -1.0 };
            let vx = clamp(dx * 1.6, -520.0, 520.0);
            c.say("!", 0.6, false);
            c.launch(vx, Self::PARAMS.jump_vy);
        }
    }
}

// =================================================================== art

const STRIPE: Color = Color::hex("#B9622A");
const CREAM: Color = Color::hex("#F7D9AE");
const PINK: Color = Color::hex("#EE9AA2");
const MOUTH: Color = Color::hex("#5A2226");
const IRIS: Color = Color::hex("#8CC63F");
const WHISKER: Color = Color::hex("#FFF6E8");

type Tri = [(f64, f64); 3];
const EAR_BACK: Tri = [(-6.0, -4.0), (-4.5, -15.0), (1.0, -7.0)];
const EAR_FRONT: Tri = [(-1.0, -7.0), (3.0, -16.0), (6.5, -5.0)];

fn tri(pts: &Tri, inset: f64) -> [Pt; 3] {
    let cx = pts.iter().map(|p| p.0).sum::<f64>() / 3.0;
    let cy = pts.iter().map(|p| p.1).sum::<f64>() / 3.0;
    pts.map(|(x, y)| pt(x + (cx - x) * inset, y + (cy - y) * inset))
}

const CAT_STYLE: Style = Style {
    coat: Color::hex("#E8964A"),
    shade: Color::hex("#C97A36"),
    outline: Color::hex("#4A2A14"),
    chest_r: 8.0,
    hip_r: 8.5,
    neck_w: (11.0, 9.0),
    front_w: (5.4, 3.6, 3.4),
    hind_w: (7.0, 3.8, 3.4),
    ..Style::BASE
};
const TAIL_LEN: f64 = 30.0;

impl Cat {
    fn ear_transform(&self, pose: &QuadPose, base: (f64, f64)) -> Xf {
        let back = -35.0 * (-pose.ears).max(0.0); // flattened when annoyed / scared
        Xf::new().translate(base.0, base.1).rotate(back).translate(-base.0, -base.1)
    }

    fn tail_path(&self, pose: &QuadPose, g: &Geometry) -> (Path, [(f64, f64); 4]) {
        let a = (pose.tail + 0.3 * pose.tail_wag).to_radians();
        let l = TAIL_LEN;
        let r = g.tail_root;
        let (dx, dy) = (-a.cos(), -a.sin()); // back, and up by the tail angle
        let (ux, uy) = (dy, -dx); // perpendicular, toward "up" from the tail
        let curl = pose.tail_curl;
        let p1 = (r.x + dx * l * 0.4, r.y + dy * l * 0.4);
        let p2 = (r.x + dx * l * 0.85 + ux * l * 0.15 * curl, r.y + dy * l * 0.85 + uy * l * 0.15 * curl);
        let tip = (r.x + dx * l * 0.85 + ux * l * 0.5 * curl, r.y + dy * l * 0.85 + uy * l * 0.5 * curl);
        let p0 = (r.x, r.y);
        (tapered_curve(p0, p1, p2, tip, 5.5, 4.2, 20, 1.0), [p0, p1, p2, tip])
    }
}

impl QuadArt for Cat {
    fn style(&self) -> &Style {
        &CAT_STYLE
    }
    fn rig(&self) -> &Rig {
        &CAT_RIG
    }

    fn head_shape(&self, pose: &QuadPose, g: &Geometry) -> Path {
        let mut head = Path::new();
        head.add_ellipse(pt(0.0, 0.0), 9.0, 8.0);
        let mut head = head.united(&circle(pt(7.0, 3.0), 4.4));
        for ear in [EAR_BACK, EAR_FRONT] {
            let path = polygon(&tri(&ear, 0.0));
            head = head.united(&self.ear_transform(pose, ear[0]).map_path(&path));
        }
        self.head_transform(g).map_path(&head)
    }

    fn paint_tail(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry) {
        let (path, ctrl) = self.tail_path(pose, g);
        p.no_pen();
        p.set_brush(CAT_STYLE.coat);
        p.draw_path(&path);
        p.save(); // tabby rings: a dashed stroke along the spine
        p.set_clip_path(&path);
        let mut spine = Path::at(ctrl[0].0, ctrl[0].1);
        spine.cubic_to(ctrl[1].0, ctrl[1].1, ctrl[2].0, ctrl[2].1, ctrl[3].0, ctrl[3].1);
        let mut rings = Pen::new(STRIPE, 8.0);
        rings.dash = Some((vec![0.45, 0.55], -0.6));
        p.set_pen(rings);
        p.no_brush();
        p.draw_path(&spine);
        p.restore();
        p.set_pen(pen(CAT_STYLE.outline, CAT_STYLE.line_w));
        p.no_brush();
        p.draw_path(&path);
    }

    fn paint_markings(&self, p: &mut Painter, _pose: &QuadPose, g: &Geometry) {
        let (h, s) = (g.hip, g.shoulder);
        p.no_pen();
        p.set_brush(CREAM);
        p.draw_ellipse(pt(s.x + 4.0, s.y + 6.0), 6.0, 5.0); // chest
        p.set_pen(pen(STRIPE, 2.3));
        let (dx, dy) = (s.x - h.x, s.y - h.y);
        let n = dx.hypot(dy);
        let n = if n == 0.0 { 1.0 } else { n };
        let (nx, ny) = (dy / n, -dx / n); // body normal, toward the back
        for f in [0.05, 0.28, 0.5, 0.72] {
            let (cx, cy) = (h.x + dx * f, h.y + dy * f);
            let r = CAT_STYLE.hip_r + (CAT_STYLE.chest_r - CAT_STYLE.hip_r) * f;
            let mut stripe = Path::at(cx + nx * (r + 1.0), cy + ny * (r + 1.0));
            stripe.quad_to(cx + nx * r * 0.3 + 2.5, cy + ny * r * 0.3, cx - nx * r * 0.2, cy - ny * r * 0.2);
            p.draw_path(&stripe);
        }
        p.save();
        p.combine_transform(&self.head_transform(g));
        p.no_pen();
        p.set_brush(CREAM);
        p.draw_ellipse(pt(8.0, 4.2), 5.0, 3.4); // muzzle
        p.set_pen(pen(STRIPE, 1.6));
        for x in [-3.0, 0.0, 3.0] {
            p.draw_line(pt(x, -7.5), pt(x + 0.8, -4.5)); // forehead "M"
        }
        p.restore();
    }

    fn paint_head(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry, facing: f64) {
        let outline = CAT_STYLE.outline;
        p.save();
        p.combine_transform(&self.head_transform(g));
        p.no_pen();
        p.set_brush(PINK);
        for ear in [EAR_BACK, EAR_FRONT] {
            p.save();
            p.combine_transform(&self.ear_transform(pose, ear[0]));
            p.draw_polygon(&tri(&ear, 0.38));
            p.restore();
        }
        if pose.mouth > 0.05 {
            let hinge = pt(6.0, 5.0);
            let t = Xf::new().translate(hinge.x, hinge.y).rotate(30.0 * pose.mouth);
            let jaw = [pt(0.0, 0.0), pt(5.5, 0.3), pt(5.0, 2.4), pt(0.0, 2.6)].map(|q| t.map(q));
            p.set_brush(MOUTH);
            p.draw_polygon(&[hinge, pt(11.5, 4.6), t.map(pt(5.5, 0.3))]);
            p.set_pen(pen(outline, 1.0));
            p.set_brush(CREAM);
            p.draw_polygon(&jaw);
            if pose.tongue {
                p.no_pen();
                p.set_brush(PINK);
                p.draw_ellipse(pt(11.0, 6.2), 2.0, 1.4);
            }
        } else {
            let mut mouth = Path::at(11.0, 3.2);
            mouth.quad_to(10.5, 5.6, 8.6, 5.4);
            mouth.move_to(11.0, 3.2);
            mouth.quad_to(11.6, 5.8, 13.0, 5.0);
            p.set_pen(pen(outline, 1.0));
            p.no_brush();
            p.draw_path(&mouth);
        }
        p.set_pen(pen(outline, 0.8));
        p.set_brush(PINK);
        p.draw_polygon(&[pt(10.0, 1.0), pt(12.0, 1.0), pt(11.2, 2.8)]); // nose
        self.paint_eye(p, pose, pt(4.0, -2.2), facing, 2.3, Some(IRIS), pose.eyes != Eyes::Wide, Some(outline));
        p.set_pen(pen(WHISKER, 0.7));
        for (y0, y1) in [(3.6, 1.6), (4.4, 4.2), (5.2, 6.8)] {
            p.draw_line(pt(10.0, y0), pt(19.5, y1));
        }
        p.restore();
    }

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
        quadruped::default_paint_eye(CAT_STYLE.outline, p, pose, at, facing, r, iris, slit, light);
        if matches!(pose.eyes, Eyes::Open | Eyes::Wide) {
            // almond shape: a lid line over the eye
            p.set_pen(pen(CAT_STYLE.outline, 1.1));
            p.no_brush();
            let mut lid = Path::at(at.x - r * 1.4, at.y + 0.3);
            lid.quad_to(at.x, at.y - r * 1.5, at.x + r * 1.4, at.y - 0.2);
            p.draw_path(&lid);
        }
    }
}

pub static CHARACTER: Character = Character {
    name: "cat",
    half_width: 46.0,
    height: 52.0,
    eye_y: 36.0,
    grip: (32.0, -28.0),
    icon_box: (24.0, -37.0, 34.0),
    size: Character::DEFAULT_SIZE,
    previews: &[
        ("sit (slow blink)", "sit", 5.0, 1.0),
        ("groom: lick paw", "groom", 4.0, 0.9),
        ("groom: wipe face", "groom", 4.0, 1.5),
        ("nap", "nap", 8.0, 2.0),
        ("stretch + yawn", "stretch", 2.0, 1.0),
        ("meow", "meow", MEOW_TIME, 0.35),
        ("stalk the cursor", "pounce", 6.0, 0.8),
        ("zoomies", "run", 2.0, 0.6),
    ],
    params: &Cat::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Quad(Cat { phase: Phase::default(), wiggle_at: Cell::new(None) }), x, y, seed)),
};
