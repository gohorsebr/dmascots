//! Dogs: one shared brain, two breeds. Port of `characters/dog/{brain,art,__init__}.py`.
//!
//! Labrador: easy-going, pants, begs, naps a lot.
//! Border collie: high energy, more zoomies, and "herds" your cursor with the
//! classic low, staring stalk.

use crate::art::{pen, tapered, tapered_curve};
use crate::engine::{bump, ease, Bounds, Brain, Character, Core, Eyes, Params, Preview};
use crate::gfx::{pt, Color, Painter, Path, Pt, Xf};
use crate::quadruped::{
    self, bow, breathe, circle, default_paint_eye, ellipse, foot, polygon, sit, Geometry, Phase, Quad, QuadArt,
    QuadKind, QuadParams, QuadPose, Rig, Style, FF, FN,
};

const LAB_RIG: Rig = Rig {
    shoulder: (13.0, -29.0),
    hip: (-14.0, -30.0),
    front_leg: (14.0, 15.0),
    hind_leg: (15.0, 15.0),
    head: (27.0, -44.0),
    neck_base: (3.0, -6.0),
    tail_root: (-8.0, -4.0),
    foot_y: -2.3,
    ..Rig::BASE
};
const COLLIE_RIG: Rig = Rig {
    shoulder: (13.0, -27.0),
    hip: (-15.0, -28.0),
    front_leg: (13.0, 14.0),
    hind_leg: (14.0, 14.0),
    head: (26.0, -42.0),
    neck_base: (3.0, -6.0),
    tail_root: (-8.0, -4.0),
    foot_y: -2.2,
    ..Rig::BASE
};
pub const BARK_TIME: f64 = 1.2;

const DOG_PARAMS: Params = Params {
    walk_speed: 60.0,
    steal_speed: 260.0,
    steal_lines: &["Fetch!", "Woof! Mine!", "Catch me!"],
    return_lines: &["Okay, you can have it.", "Again? Again?"],
    ..Params::BASE
};
const DOG_QUAD: QuadParams = QuadParams { stride: 7.0, lift: 4.0, run_speed: 260.0, ..QuadParams::BASE };

// =================================================================== shared dog behaviour

/// Poke a dog and it barks back, then wags.
fn dog_poke(c: &mut Core) {
    if c.state == "nap" {
        c.say("…?", 1.6, false);
        c.go_idle();
    } else if !c.airborne() && !matches!(c.state, "crouch" | "land" | "dizzy") {
        c.go("bark", BARK_TIME);
    } else {
        c.chatter(c.params.lines);
    }
}

fn dog_rest(c: &Core, p: &mut QuadPose, pants: bool) {
    breathe(c, p);
    p.tail_wag = 12.0 * (c.clock * 3.0).sin();
    if pants {
        p.mouth = 0.3 + 0.08 * (c.clock * 9.0).sin();
        p.tongue = true;
    }
}

fn bark(c: &mut Core, p: &mut QuadPose, barks: &[&'static str]) {
    breathe(c, p);
    if c.entered() {
        let line = c.choice(barks);
        c.say(line, 1.3, false);
    }
    let woof = bump(c.t, 0.25, 0.3) + bump(c.t, 0.7, 0.3);
    p.mouth = 0.9 * woof;
    p.head = (2.0, -3.0 - 2.0 * woof);
    p.head_angle = -10.0 * woof;
    p.shoulder = (p.shoulder.0, p.shoulder.1 - 2.0 * woof);
    p.ears = 1.0;
    p.tail = 15.0;
    p.tail_wag = 25.0 * (c.clock * 18.0).sin();
    if c.done() {
        c.go("wag", 1.5);
    }
}

fn wag(c: &mut Core, p: &mut QuadPose) {
    breathe(c, p);
    let fast = (c.clock * 22.0).sin();
    p.tail = 20.0;
    p.tail_wag = 35.0 * fast;
    p.hip = (p.hip.0 + 1.2 * fast, p.hip.1); // the whole back end wags along
    p.mouth = 0.45;
    p.tongue = true;
    if (c.clock * 1.5) as i64 % 3 == 0 {
        p.eyes = Eyes::Happy;
    }
    if c.done() {
        c.go_idle();
    }
}

fn bow_state(c: &mut Core, p: &mut QuadPose, rig: &Rig) {
    breathe(c, p);
    let k = ease(c.t / 0.3) * ease((c.duration - c.t) / 0.3);
    bow(rig, p, k);
    p.tail_wag = 30.0 * (c.clock * 20.0).sin();
    p.ears = 1.0;
    p.mouth = 0.35;
    p.tongue = true;
    if c.entered() && c.random() < 0.5 {
        c.say("Play?", 1.2, false);
    }
    if c.done() {
        // play bows usually end in zoomies
        if c.random() < 0.6 && !c.paused {
            c.facing = c.coin();
            let d = c.uniform(1.5, 3.0);
            c.go("run", d);
        } else {
            c.go_idle();
        }
    }
}

/// Shared dog states; true if handled.
fn dog_state(c: &mut Core, p: &mut QuadPose, rig: &Rig, barks: &[&'static str]) -> bool {
    match c.state {
        "bark" => bark(c, p, barks),
        "wag" => wag(c, p),
        "bow" => bow_state(c, p, rig),
        _ => return false,
    }
    true
}

// =================================================================== shared dog art

const NOSE: Color = Color::hex("#2E2220");
const MOUTH: Color = Color::hex("#4A1E1E");
const TONGUE: Color = Color::hex("#E8707A");

const DOG_STYLE: Style = Style {
    neck_w: (17.0, 13.0),
    front_w: (8.0, 5.0, 4.6),
    hind_w: (10.0, 5.5, 4.6),
    ..Style::BASE
};

/// The DogArt class attributes.
struct Look {
    muzzle: (f64, f64, f64, f64), // local centre x, y, rx, ry
    nose_at: Pt,
    tail_len: f64,
    tail_w: (f64, f64),
    tail_base: f64, // degrees above horizontal, pointing back
}

trait DogArt: QuadArt {
    fn look(&self) -> &Look;
    fn ear_silhouette(&self, _pose: &QuadPose) -> Path {
        Path::new()
    }
    fn paint_ear(&self, _p: &mut Painter, _pose: &QuadPose) {}
    fn muzzle_color(&self) -> Color {
        self.style().coat
    }

    fn dog_head_shape(&self, pose: &QuadPose, g: &Geometry) -> Path {
        let (mx, my, rx, ry) = self.look().muzzle;
        let local = circle(pt(0.0, 0.0), 9.5).united(&ellipse(pt(mx, my), rx, ry));
        self.head_transform(g).map_path(&local.united(&self.ear_silhouette(pose)))
    }

    fn tail_path(&self, pose: &QuadPose, g: &Geometry) -> (Path, Pt) {
        let l = self.look();
        let a = (l.tail_base + pose.tail + 0.3 * pose.tail_wag).to_radians();
        let length = l.tail_len * (1.0 - 0.3 * pose.tail_wag.to_radians().sin().abs()); // wagging foreshortens
        let r = g.tail_root;
        let (dx, dy) = (-a.cos(), -a.sin());
        let tip = (r.x + dx * length, r.y + dy * length);
        let p1 = (r.x + dx * length * 0.35, r.y + dy * length * 0.35);
        let p2 = (r.x + dx * length * 0.75, r.y + dy * length * 0.75 - length * 0.08);
        (tapered((r.x, r.y), p1, p2, tip, l.tail_w.0, l.tail_w.1), Pt::from(tip))
    }

    fn dog_paint_tail(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry) {
        let (path, _) = self.tail_path(pose, g);
        p.set_pen(pen(self.style().outline, self.style().line_w));
        p.set_brush(self.style().coat);
        p.draw_path(&path);
    }

    fn dog_paint_head(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry, facing: f64) {
        p.save();
        p.combine_transform(&self.head_transform(g));
        self.paint_mouth(p, pose);
        self.paint_ear(p, pose);
        self.paint_eye(p, pose, pt(3.5, -3.2), facing, 2.0, None, false, None);
        let n = self.look().nose_at;
        p.no_pen();
        p.set_brush(NOSE);
        p.draw_ellipse(n, 2.8, 2.3);
        p.set_brush(Color::rgba(255, 255, 255, 120));
        p.draw_ellipse(pt(n.x - 0.8, n.y - 0.9), 0.9, 0.6);
        p.restore();
    }

    fn paint_mouth(&self, p: &mut Painter, pose: &QuadPose) {
        let outline = self.style().outline;
        let tip = self.look().nose_at.x - 0.5;
        if pose.mouth > 0.05 {
            let hinge = pt(7.0, 5.0);
            let mut jaw = Path::at(0.0, 0.0);
            jaw.line_to(tip - 8.0, 0.4);
            jaw.quad_to(tip - 7.0, 3.2, tip - 9.5, 3.6);
            jaw.line_to(0.0, 3.4);
            jaw.close();
            let t = Xf::new().translate(hinge.x, hinge.y).rotate(26.0 * pose.mouth);
            let jaw = t.map_path(&jaw);
            p.no_pen();
            p.set_brush(MOUTH);
            p.draw_polygon(&[hinge, pt(tip - 1.0, 4.6), t.map(pt(tip - 8.0, 0.4))]);
            p.set_pen(pen(outline, 1.1));
            p.set_brush(self.muzzle_color());
            p.draw_path(&jaw);
            if pose.tongue {
                let tongue = tapered_curve(
                    (11.5, 6.0),
                    (12.5, 8.5),
                    (13.5, 11.0 + 3.0 * pose.mouth),
                    (13.0, 13.0 + 4.0 * pose.mouth),
                    4.0,
                    3.4,
                    8,
                    1.6,
                );
                p.set_pen(pen(Color::hex("#B04652"), 0.9));
                p.set_brush(TONGUE);
                p.draw_path(&tongue);
            }
        } else {
            let mut mouth = Path::at(tip - 1.0, 5.0);
            mouth.quad_to(tip - 6.0, 6.6, 9.0, 5.4);
            mouth.quad_to(8.0, 5.0, 8.2, 4.2);
            p.set_pen(pen(outline, 1.1));
            p.no_brush();
            p.draw_path(&mouth);
        }
    }
}

// =================================================================== labrador

pub struct Labrador {
    phase: Phase,
}

const LAB_STYLE: Style = Style {
    coat: Color::hex("#E6BE77"),
    shade: Color::hex("#C99E57"),
    outline: Color::hex("#5B3E1D"),
    chest_r: 12.0,
    hip_r: 11.0,
    ..DOG_STYLE
};
const LAB_EAR: Color = Color::hex("#CFA05A");
const LAB_LOOK: Look =
    Look { muzzle: (10.0, 3.0, 9.5, 6.2), nose_at: pt(19.0, 0.8), tail_len: 20.0, tail_w: (6.0, 2.5), tail_base: 10.0 };

impl QuadKind for Labrador {
    const PARAMS: Params = Params {
        lines: &["Woof!", "Woof woof!", "*pant pant*", "Treat?"],
        choices: &[
            (24.0, "walk", (2.5, 6.0)),
            (7.0, "run", (1.8, 3.0)),
            (11.0, "sit", (2.5, 5.0)),
            (9.0, "nap", (6.0, 12.0)),
            (11.0, "sniff", (2.0, 4.5)),
            (6.0, "bark", (BARK_TIME, BARK_TIME)),
            (8.0, "wag", (1.5, 3.0)),
            (5.0, "bow", (1.2, 2.0)),
            (7.0, "beg", (2.0, 3.5)),
            (4.0, "crouch", (0.14, 0.14)),
            (8.0, "idle", (1.5, 3.5)),
        ],
        ..DOG_PARAMS
    };
    const QUAD: QuadParams = QuadParams { rig: LAB_RIG, ..DOG_QUAD };

    fn phase(&self) -> &Phase {
        &self.phase
    }
    fn poke(&mut self, c: &mut Core) {
        dog_poke(c);
    }
    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        dog_rest(c, p, true);
    }
    fn state(&mut self, c: &mut Core, _dt: f64, _b: &Bounds, p: &mut QuadPose) -> bool {
        if c.state == "beg" {
            self.beg(c, p);
            return true;
        }
        dog_state(c, p, &LAB_RIG, &["WOOF!", "Woof woof!"])
    }
}

impl Labrador {
    /// Sit up with both front paws in the air.
    fn beg(&self, c: &mut Core, p: &mut QuadPose) {
        self.pose_rest(c, p);
        let k = ease(c.t / 0.4) * ease((c.duration - c.t) / 0.4);
        sit(&LAB_RIG, p, k);
        p.shoulder = (p.shoulder.0 - 5.0 * k, p.shoulder.1 - 7.0 * k);
        let wave = (c.clock * 7.0).sin();
        if k > 0.5 {
            p.feet[FN] = foot(4.0 + 2.0 * wave, 11.0, true);
            p.feet[FF] = foot(5.0 - 2.0 * wave, 10.0, true);
        }
        p.head = (p.head.0 - 3.0 * k, p.head.1 - 5.0 * k);
        p.head_angle = -12.0 * k;
        p.ears = 0.5;
        p.tail_wag = 25.0 * (c.clock * 16.0).sin();
        if c.entered() {
            c.say("Treat? Treat?", 1.6, false);
        }
        if c.done() {
            c.go_idle();
        }
    }
}

impl QuadArt for Labrador {
    fn style(&self) -> &Style {
        &LAB_STYLE
    }
    fn rig(&self) -> &Rig {
        &LAB_RIG
    }
    fn head_shape(&self, pose: &QuadPose, g: &Geometry) -> Path {
        self.dog_head_shape(pose, g)
    }
    fn paint_tail(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry) {
        self.dog_paint_tail(p, pose, g);
    }
    fn paint_head(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry, facing: f64) {
        self.dog_paint_head(p, pose, g, facing);
    }
    fn paint_markings(&self, p: &mut Painter, _pose: &QuadPose, g: &Geometry) {
        p.no_pen();
        p.set_brush(Color::hex("#F0D29A")); // lighter chest and belly
        p.draw_ellipse(pt(g.shoulder.x + 3.0, g.shoulder.y + 7.0), 9.0, 6.0);
    }
}

impl DogArt for Labrador {
    fn look(&self) -> &Look {
        &LAB_LOOK
    }
    /// Floppy ear hanging over the side of the head.
    fn paint_ear(&self, p: &mut Painter, pose: &QuadPose) {
        p.save();
        p.translate(-2.0, -6.0);
        p.rotate(-14.0 * pose.ears);
        let mut ear = Path::at(-1.0, -1.0);
        ear.cubic_to(-7.0, 1.0, -8.0, 12.0, -4.0, 15.0);
        ear.quad_to(0.0, 16.0, 3.0, 11.0);
        ear.cubic_to(3.0, 5.0, 2.0, 0.0, -1.0, -1.0);
        p.set_pen(pen(LAB_STYLE.outline, 1.2));
        p.set_brush(LAB_EAR);
        p.draw_path(&ear);
        p.restore();
    }
}

// =================================================================== border collie

pub struct Collie {
    phase: Phase,
}

const COLLIE_WHITE: Color = Color::hex("#F3F1EC");
const COLLIE_STYLE: Style = Style {
    coat: Color::hex("#232126"),
    shade: Color::hex("#151417"),
    outline: Color::hex("#0B0A0C"),
    chest_r: 10.5,
    hip_r: 9.5,
    neck_w: (17.0, 12.0),
    ..DOG_STYLE
};
const COLLIE_LOOK: Look = Look {
    muzzle: (11.0, 3.2, 10.5, 5.2),
    nose_at: pt(21.0, 1.2),
    tail_len: 22.0,
    tail_w: (7.0, 6.0),  // bushy
    tail_base: -25.0,    // carried low
};

impl QuadKind for Collie {
    const PARAMS: Params = Params {
        lines: &["Arf!", "Arf arf!", "Woof!", "Ball?"],
        choices: &[
            (22.0, "walk", (2.0, 5.0)),
            (13.0, "run", (2.0, 3.5)),
            (8.0, "sit", (2.0, 4.0)),
            (4.0, "nap", (4.0, 8.0)),
            (9.0, "sniff", (1.5, 3.5)),
            (7.0, "bark", (BARK_TIME, BARK_TIME)),
            (6.0, "wag", (1.2, 2.5)),
            (7.0, "bow", (1.0, 1.6)),
            (11.0, "herd", (4.0, 7.0)),
            (5.0, "crouch", (0.14, 0.14)),
            (8.0, "idle", (1.2, 3.0)),
        ],
        ..DOG_PARAMS
    };
    const QUAD: QuadParams = QuadParams { rig: COLLIE_RIG, run_speed: 330.0, ..DOG_QUAD };

    fn phase(&self) -> &Phase {
        &self.phase
    }
    fn poke(&mut self, c: &mut Core) {
        dog_poke(c);
    }
    fn pose_rest(&self, c: &Core, p: &mut QuadPose) {
        dog_rest(c, p, false);
    }
    fn state(&mut self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) -> bool {
        if c.state == "herd" {
            self.herd(c, dt, b, p);
            return true;
        }
        dog_state(c, p, &COLLIE_RIG, &["ARF!", "Arf arf!"])
    }
}

impl Collie {
    /// The border-collie eye: creep low toward the cursor, then dash.
    fn herd(&self, c: &mut Core, dt: f64, b: &Bounds, p: &mut QuadPose) {
        if c.paused || c.cursor.is_none() {
            self.pose_rest(c, p);
            c.go_idle();
            return;
        }
        let left = quadruped::stalk(self, c, p, b, dt, 22.0);
        p.head = (6.0, p.head.1 + 4.0); // head lower than the back, staring
        p.tail = -25.0;
        if left < 70.0 || c.done() {
            c.go("run", 1.4);
        }
    }
}

impl QuadArt for Collie {
    fn style(&self) -> &Style {
        &COLLIE_STYLE
    }
    fn rig(&self) -> &Rig {
        &COLLIE_RIG
    }
    fn head_shape(&self, pose: &QuadPose, g: &Geometry) -> Path {
        self.dog_head_shape(pose, g)
    }
    fn paint_head(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry, facing: f64) {
        self.dog_paint_head(p, pose, g, facing);
    }
    fn paint_tail(&self, p: &mut Painter, pose: &QuadPose, g: &Geometry) {
        let (path, tip) = self.tail_path(pose, g);
        p.no_pen();
        p.set_brush(COLLIE_STYLE.coat);
        p.draw_path(&path);
        p.save();
        p.set_clip_path(&path);
        p.set_brush(COLLIE_WHITE);
        p.draw_circle(tip, 6.0); // white tail tip
        p.restore();
        p.set_pen(pen(COLLIE_STYLE.outline, COLLIE_STYLE.line_w));
        p.no_brush();
        p.draw_path(&path);
    }
    fn paint_markings(&self, p: &mut Painter, _pose: &QuadPose, g: &Geometry) {
        p.no_pen();
        p.set_brush(COLLIE_WHITE);
        let nb = g.neck_base;
        p.draw_ellipse(pt(nb.x + 5.0, nb.y + 10.0), 9.0, 11.0); // ruff and chest
        p.draw_rect(-200.0, -8.5, 400.0, 20.0); // socks
        p.save();
        p.combine_transform(&self.head_transform(g));
        p.draw_ellipse(pt(12.0, 5.0), 11.0, 4.2); // white muzzle
        p.draw_polygon(&[pt(-1.0, -10.0), pt(2.5, -10.0), pt(9.0, -1.0), pt(4.0, 1.0)]); // blaze
        p.restore();
    }
    fn paint_far_markings(&self, p: &mut Painter, _pose: &QuadPose, _g: &Geometry) {
        p.no_pen();
        p.set_brush(Color::hex("#CFCCC6"));
        p.draw_rect(-200.0, -8.5, 400.0, 20.0);
    }
    fn paint_eye(
        &self,
        p: &mut Painter,
        pose: &QuadPose,
        at: Pt,
        facing: f64,
        r: f64,
        _iris: Option<Color>,
        slit: bool,
        _light: Option<Color>,
    ) {
        default_paint_eye(
            COLLIE_STYLE.outline,
            p,
            pose,
            at,
            facing,
            r,
            Some(Color::hex("#7A4A22")),
            slit,
            Some(Color::hex("#E8E4DC")),
        );
    }
}

impl DogArt for Collie {
    fn look(&self) -> &Look {
        &COLLIE_LOOK
    }
    /// Semi-pricked ear, tip folded forward.
    fn ear_silhouette(&self, pose: &QuadPose) -> Path {
        let ear = polygon(&[pt(-6.0, -5.0), pt(-6.5, -14.0), pt(-3.5, -19.0), pt(0.5, -16.5), pt(2.0, -6.0)]);
        Xf::new().translate(-2.0, -5.0).rotate(-20.0 * pose.ears.min(0.0)).translate(2.0, 5.0).map_path(&ear)
    }
    fn muzzle_color(&self) -> Color {
        COLLIE_WHITE
    }
}

// =================================================================== characters

const COMMON: [Preview; 7] = [
    ("sit", "sit", 4.0, 1.0),
    ("nap", "nap", 8.0, 2.0),
    ("sniff", "sniff", 3.0, 1.0),
    ("bark", "bark", BARK_TIME, 0.25),
    ("wag", "wag", 3.0, 0.5),
    ("play bow", "bow", 2.0, 0.8),
    ("zoomies", "run", 3.0, 0.8),
];

const fn with(extra: Preview) -> [Preview; 8] {
    let mut out = [extra; 8];
    let mut i = 0;
    while i < 7 {
        out[i] = COMMON[i];
        i += 1;
    }
    out
}

static LAB_PREVIEWS: [Preview; 8] = with(("beg", "beg", 3.0, 1.2));
static COLLIE_PREVIEWS: [Preview; 8] = with(("herd the cursor", "herd", 6.0, 0.8));

pub static LABRADOR: Character = Character {
    name: "labrador",
    half_width: 50.0,
    height: 62.0,
    eye_y: 47.0,
    icon_box: (30.0, -45.0, 40.0),
    grip: (44.0, -38.0),
    size: Character::DEFAULT_SIZE,
    previews: &LAB_PREVIEWS,
    params: &Labrador::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Quad(Labrador { phase: Phase::default() }), x, y, seed)),
};

pub static COLLIE: Character = Character {
    name: "collie",
    half_width: 50.0,
    height: 62.0,
    eye_y: 47.0,
    icon_box: (30.0, -45.0, 40.0),
    grip: (44.0, -38.0),
    size: Character::DEFAULT_SIZE,
    previews: &COLLIE_PREVIEWS,
    params: &Collie::PARAMS,
    make: |x, y, seed| Box::new(Brain::new(Quad(Collie { phase: Phase::default() }), x, y, seed)),
};
