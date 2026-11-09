//! A small QPainter look-alike on top of tiny-skia, so the character art ports line by line.
//!
//! * `Path` keeps its own command list (like QPainterPath) so it can be transformed,
//!   flattened and run through boolean ops (`united`, `simplified`) with i_overlay.
//! * `Painter` has a save/restore stack of (transform, pen, brush, clip, font size).
//!   Angles are in degrees and y points down, exactly like Qt.

use std::f64::consts::PI;
use std::ops::{Add, Mul, Neg, Sub};
use std::sync::OnceLock;

use i_overlay::core::fill_rule::FillRule as OvFill;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use tiny_skia as sk;

// ------------------------------------------------------------------ points & colours

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

#[inline]
pub const fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        pt(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        pt(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, k: f64) -> Pt {
        pt(self.x * k, self.y * k)
    }
}
impl Neg for Pt {
    type Output = Pt;
    fn neg(self) -> Pt {
        pt(-self.x, -self.y)
    }
}
impl From<(f64, f64)> for Pt {
    fn from((x, y): (f64, f64)) -> Pt {
        pt(x, y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
    /// "#RRGGBB" or "#AARRGGBB" (Qt's order), usable in consts.
    pub const fn hex(s: &str) -> Color {
        const fn nib(c: u8) -> u8 {
            match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => panic!("bad hex digit"),
            }
        }
        const fn byte(b: &[u8], i: usize) -> u8 {
            nib(b[i]) * 16 + nib(b[i + 1])
        }
        let b = s.as_bytes();
        match b.len() {
            7 => Color::rgba(byte(b, 1), byte(b, 3), byte(b, 5), 255),
            9 => Color::rgba(byte(b, 3), byte(b, 5), byte(b, 7), byte(b, 1)),
            _ => panic!("colour must be #RRGGBB or #AARRGGBB"),
        }
    }
    pub fn with_alpha_f(self, a: f64) -> Color {
        Color { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..self }
    }
    /// Qt's QColor::lighter / darker (factor in percent), done in HSV like Qt.
    pub fn lighter(self, factor: f64) -> Color {
        let (h, s, v) = self.hsv();
        let mut v = v * factor / 100.0;
        let mut s = s;
        if v > 1.0 {
            s = (s - (v - 1.0)).max(0.0);
            v = 1.0;
        }
        Color::from_hsv(h, s, v, self.a)
    }
    pub fn darker(self, factor: f64) -> Color {
        let (h, s, v) = self.hsv();
        Color::from_hsv(h, s, v * 100.0 / factor, self.a)
    }
    fn hsv(self) -> (f64, f64, f64) {
        let (r, g, b) = (self.r as f64 / 255.0, self.g as f64 / 255.0, self.b as f64 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        (h, if max == 0.0 { 0.0 } else { d / max }, max)
    }
    fn from_hsv(h: f64, s: f64, v: f64, a: u8) -> Color {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to = |f: f64| ((f + m).clamp(0.0, 1.0) * 255.0).round() as u8;
        Color::rgba(to(r), to(g), to(b), a)
    }
    fn sk(self) -> sk::Color {
        sk::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

// ------------------------------------------------------------------ transforms

/// Affine transform with Qt's composition order: `t.translate(..).rotate(..)` applies
/// the rotation first, then the translation, to a point (i.e. post-multiplication).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub a: f64, // x' = a*x + c*y + e
    pub b: f64, // y' = b*x + d*y + f
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Xf {
    fn default() -> Self {
        Xf::IDENTITY
    }
}

impl Xf {
    pub const IDENTITY: Xf = Xf { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn new() -> Xf {
        Xf::IDENTITY
    }
    /// self ∘ m: m is applied to points first.
    pub fn then_local(&self, m: &Xf) -> Xf {
        Xf {
            a: self.a * m.a + self.c * m.b,
            b: self.b * m.a + self.d * m.b,
            c: self.a * m.c + self.c * m.d,
            d: self.b * m.c + self.d * m.d,
            e: self.a * m.e + self.c * m.f + self.e,
            f: self.b * m.e + self.d * m.f + self.f,
        }
    }
    pub fn translate(&self, x: f64, y: f64) -> Xf {
        self.then_local(&Xf { e: x, f: y, ..Xf::IDENTITY })
    }
    pub fn scale(&self, sx: f64, sy: f64) -> Xf {
        self.then_local(&Xf { a: sx, d: sy, ..Xf::IDENTITY })
    }
    pub fn rotate(&self, deg: f64) -> Xf {
        let (s, c) = (deg * PI / 180.0).sin_cos();
        self.then_local(&Xf { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 })
    }
    pub fn map(&self, p: Pt) -> Pt {
        pt(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }
    /// QTransform::map(QPainterPath)
    pub fn map_path(&self, path: &Path) -> Path {
        path.transformed(self)
    }
    fn sk(&self) -> sk::Transform {
        sk::Transform::from_row(
            self.a as f32, self.b as f32, self.c as f32, self.d as f32, self.e as f32, self.f as f32,
        )
    }
}

// ------------------------------------------------------------------ paths

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    Move(Pt),
    Line(Pt),
    Quad(Pt, Pt),
    Cubic(Pt, Pt, Pt),
    Close,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    #[default]
    OddEven, // Qt's default for QPainterPath and drawPolygon
    Winding,
}

#[derive(Clone, Debug, Default)]
pub struct Path {
    cmds: Vec<Cmd>,
    pub fill_rule: FillRule,
}

/// Flatten everything to this precision (in device pixels) before boolean ops.
const FLATTEN_TOL: f64 = 0.02;

impl Path {
    pub fn new() -> Path {
        Path::default()
    }
    /// QPainterPath(QPointF start)
    pub fn at(x: f64, y: f64) -> Path {
        let mut p = Path::new();
        p.move_to(x, y);
        p
    }
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
    fn ensure_start(&mut self) {
        // Qt: drawing without a moveTo starts at the end of the previous subpath (or the origin)
        match self.cmds.last() {
            None => self.cmds.push(Cmd::Move(pt(0.0, 0.0))),
            Some(Cmd::Close) => {
                let start = self.subpath_start();
                self.cmds.push(Cmd::Move(start));
            }
            _ => {}
        }
    }
    fn subpath_start(&self) -> Pt {
        self.cmds
            .iter()
            .rev()
            .find_map(|c| if let Cmd::Move(p) = *c { Some(p) } else { None })
            .unwrap_or_default()
    }
    pub fn move_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.cmds.push(Cmd::Move(pt(x, y)));
        self
    }
    pub fn line_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.ensure_start();
        self.cmds.push(Cmd::Line(pt(x, y)));
        self
    }
    pub fn quad_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) -> &mut Self {
        self.ensure_start();
        self.cmds.push(Cmd::Quad(pt(cx, cy), pt(x, y)));
        self
    }
    pub fn cubic_to(&mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) -> &mut Self {
        self.ensure_start();
        self.cmds.push(Cmd::Cubic(pt(c1x, c1y), pt(c2x, c2y), pt(x, y)));
        self
    }
    pub fn line_pt(&mut self, p: Pt) -> &mut Self {
        self.line_to(p.x, p.y)
    }
    pub fn quad_pt(&mut self, c: Pt, p: Pt) -> &mut Self {
        self.quad_to(c.x, c.y, p.x, p.y)
    }
    pub fn cubic_pt(&mut self, c1: Pt, c2: Pt, p: Pt) -> &mut Self {
        self.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y)
    }
    pub fn close(&mut self) -> &mut Self {
        if !self.cmds.is_empty() && !matches!(self.cmds.last(), Some(Cmd::Close)) {
            self.cmds.push(Cmd::Close);
        }
        self
    }
    /// QPainterPath::addEllipse: a closed subpath of 4 cubics, starting at angle 0.
    pub fn add_ellipse(&mut self, c: Pt, rx: f64, ry: f64) -> &mut Self {
        const K: f64 = 0.552_284_749_830_793_4;
        let (x, y) = (c.x, c.y);
        // Qt goes counter-clockwise on screen (0° -> 90° up), y-down coordinates
        self.move_to(x + rx, y);
        self.cubic_to(x + rx, y - ry * K, x + rx * K, y - ry, x, y - ry);
        self.cubic_to(x - rx * K, y - ry, x - rx, y - ry * K, x - rx, y);
        self.cubic_to(x - rx, y + ry * K, x - rx * K, y + ry, x, y + ry);
        self.cubic_to(x + rx * K, y + ry, x + rx, y + ry * K, x + rx, y);
        self.close()
    }
    pub fn add_polygon(&mut self, pts: &[Pt]) -> &mut Self {
        if let Some((first, rest)) = pts.split_first() {
            self.move_to(first.x, first.y);
            for p in rest {
                self.line_to(p.x, p.y);
            }
        }
        self
    }
    pub fn add_rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close()
    }
    pub fn add_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> &mut Self {
        let rx = rx.min(w / 2.0);
        let ry = ry.min(h / 2.0);
        const K: f64 = 0.552_284_749_830_793_4;
        let (r, b) = (x + w, y + h);
        self.move_to(r, y + ry);
        self.cubic_to(r, y + ry * (1.0 - K), r - rx * (1.0 - K), y, r - rx, y);
        self.line_to(x + rx, y);
        self.cubic_to(x + rx * (1.0 - K), y, x, y + ry * (1.0 - K), x, y + ry);
        self.line_to(x, b - ry);
        self.cubic_to(x, b - ry * (1.0 - K), x + rx * (1.0 - K), b, x + rx, b);
        self.line_to(r - rx, b);
        self.cubic_to(r - rx * (1.0 - K), b, r, b - ry * (1.0 - K), r, b - ry);
        self.close()
    }
    pub fn add_path(&mut self, other: &Path) -> &mut Self {
        self.cmds.extend_from_slice(&other.cmds);
        self
    }
    pub fn transformed(&self, t: &Xf) -> Path {
        let m = |p: Pt| t.map(p);
        Path {
            cmds: self
                .cmds
                .iter()
                .map(|c| match *c {
                    Cmd::Move(p) => Cmd::Move(m(p)),
                    Cmd::Line(p) => Cmd::Line(m(p)),
                    Cmd::Quad(a, p) => Cmd::Quad(m(a), m(p)),
                    Cmd::Cubic(a, b, p) => Cmd::Cubic(m(a), m(b), m(p)),
                    Cmd::Close => Cmd::Close,
                })
                .collect(),
            fill_rule: self.fill_rule,
        }
    }
    pub fn translated(&self, dx: f64, dy: f64) -> Path {
        self.transformed(&Xf::new().translate(dx, dy))
    }

    /// Polygons approximating each subpath (every subpath implicitly closed, as for filling).
    pub fn to_polygons(&self, tol: f64) -> Vec<Vec<Pt>> {
        let mut out: Vec<Vec<Pt>> = Vec::new();
        let mut cur: Vec<Pt> = Vec::new();
        let mut last = pt(0.0, 0.0);
        let flush = |cur: &mut Vec<Pt>, out: &mut Vec<Vec<Pt>>| {
            if cur.len() >= 3 {
                out.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
        };
        for c in &self.cmds {
            match *c {
                Cmd::Move(p) => {
                    flush(&mut cur, &mut out);
                    cur.push(p);
                    last = p;
                }
                Cmd::Line(p) => {
                    cur.push(p);
                    last = p;
                }
                Cmd::Quad(a, p) => {
                    let n = segments((last - a * 2.0 + p).len() * 2.0, tol);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        cur.push(last * (u * u) + a * (2.0 * u * t) + p * (t * t));
                    }
                    last = p;
                }
                Cmd::Cubic(a, b, p) => {
                    let d2 = (last - a * 2.0 + b).len().max((a - b * 2.0 + p).len()) * 6.0;
                    let n = segments(d2, tol);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        cur.push(
                            last * (u * u * u) + a * (3.0 * u * u * t) + b * (3.0 * u * t * t) + p * (t * t * t),
                        );
                    }
                    last = p;
                }
                Cmd::Close => {
                    let start = cur.first().copied().unwrap_or(last);
                    flush(&mut cur, &mut out);
                    last = start;
                }
            }
        }
        flush(&mut cur, &mut out);
        out
    }

    fn contours(&self) -> Vec<Vec<[f64; 2]>> {
        self.to_polygons(FLATTEN_TOL)
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| [p.x, p.y]).collect())
            .collect()
    }

    fn ov_fill(&self) -> OvFill {
        match self.fill_rule {
            FillRule::OddEven => OvFill::EvenOdd,
            FillRule::Winding => OvFill::NonZero,
        }
    }

    fn from_shapes(shapes: Vec<Vec<Vec<[f64; 2]>>>) -> Path {
        let mut p = Path { cmds: Vec::new(), fill_rule: FillRule::Winding };
        for shape in shapes {
            for contour in shape {
                if contour.len() < 3 {
                    continue;
                }
                p.move_to(contour[0][0], contour[0][1]);
                for q in &contour[1..] {
                    p.line_to(q[0], q[1]);
                }
                p.close();
            }
        }
        p
    }

    /// QPainterPath::simplified: the filled area as non-overlapping contours.
    pub fn simplified(&self) -> Path {
        Path::from_shapes(self.contours().simplify_shape(self.ov_fill(), 0.0))
    }

    /// QPainterPath::united: boolean union, each operand filled with its own fill rule.
    pub fn united(&self, other: &Path) -> Path {
        let a = self.contours().simplify_shape(self.ov_fill(), 0.0);
        let b = other.contours().simplify_shape(other.ov_fill(), 0.0);
        let a: Vec<Vec<[f64; 2]>> = a.into_iter().flatten().collect();
        let b: Vec<Vec<[f64; 2]>> = b.into_iter().flatten().collect();
        if a.is_empty() {
            return Path::from_shapes(vec![b]);
        }
        if b.is_empty() {
            return Path::from_shapes(vec![a]);
        }
        Path::from_shapes(a.overlay(&b, OverlayRule::Union, OvFill::NonZero))
    }

    pub fn bounding_rect(&self) -> (f64, f64, f64, f64) {
        let polys = self.to_polygons(0.1);
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in polys.iter().flatten() {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        if x0 > x1 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (x0, y0, x1 - x0, y1 - y0)
    }

    fn to_sk(&self) -> Option<sk::Path> {
        let mut b = sk::PathBuilder::with_capacity(self.cmds.len(), self.cmds.len() * 3);
        for c in &self.cmds {
            match *c {
                Cmd::Move(p) => b.move_to(p.x as f32, p.y as f32),
                Cmd::Line(p) => b.line_to(p.x as f32, p.y as f32),
                Cmd::Quad(a, p) => b.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
                Cmd::Cubic(a, c2, p) => {
                    b.cubic_to(a.x as f32, a.y as f32, c2.x as f32, c2.y as f32, p.x as f32, p.y as f32)
                }
                Cmd::Close => b.close(),
            }
        }
        b.finish()
    }
}

impl Pt {
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
}

fn segments(second_derivative: f64, tol: f64) -> usize {
    // uniform subdivision error ≈ |B''| / (8 n²)
    ((second_derivative / (8.0 * tol)).sqrt().ceil() as usize).clamp(1, 200)
}

/// A polyline through the points (QPainterPath with lineTo).
pub fn polyline(pts: &[(f64, f64)]) -> Path {
    let mut p = Path::at(pts[0].0, pts[0].1);
    for &(x, y) in &pts[1..] {
        p.line_to(x, y);
    }
    p
}

// ------------------------------------------------------------------ pens & text

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Flat,
    Square,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pen {
    pub color: Color,
    pub width: f64,
    pub cap: Cap,
    pub join: Join,
    /// Dash pattern in units of the pen width, and its offset (Qt semantics).
    pub dash: Option<(Vec<f64>, f64)>,
}

impl Pen {
    /// QPen(color, width): square caps, bevel joins.
    pub fn new(color: Color, width: f64) -> Pen {
        Pen { color, width, cap: Cap::Square, join: Join::Bevel, dash: None }
    }
    /// The round-capped, round-joined pen every character uses.
    pub fn round(color: Color, width: f64) -> Pen {
        Pen { cap: Cap::Round, join: Join::Round, ..Pen::new(color, width) }
    }
}

impl From<Color> for Pen {
    /// QPainter::setPen(QColor): 1px square-capped pen.
    fn from(c: Color) -> Pen {
        Pen::new(c, 1.0)
    }
}

static FONT: &[u8] = include_bytes!("../assets/NotoSans-Bold.ttf");
/// Fallback for symbols Noto Sans lacks (♪ ♫ ★ arrows, ...), like Qt's font fallback.
static FALLBACK: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");

fn faces() -> &'static [ttf_parser::Face<'static>; 2] {
    static FACES: OnceLock<[ttf_parser::Face<'static>; 2]> = OnceLock::new();
    FACES.get_or_init(|| {
        [
            ttf_parser::Face::parse(FONT, 0).expect("embedded font"),
            ttf_parser::Face::parse(FALLBACK, 0).expect("embedded fallback font"),
        ]
    })
}

fn face() -> &'static ttf_parser::Face<'static> {
    &faces()[0]
}

/// The face that has a glyph for `ch`, the glyph, and that face's px-per-unit factor.
fn glyph_for(ch: char, px: f64) -> (&'static ttf_parser::Face<'static>, ttf_parser::GlyphId, f64) {
    for f in faces() {
        if let Some(g) = f.glyph_index(ch) {
            return (f, g, px / f.units_per_em() as f64);
        }
    }
    let f = face();
    (f, f.glyph_index('?').unwrap_or_default(), px / f.units_per_em() as f64)
}

/// Text metrics for the embedded bold font at a pixel size.
pub struct Font {
    pub px: f64,
}

impl Font {
    pub fn new(px: f64) -> Font {
        Font { px }
    }
    fn k(&self) -> f64 {
        self.px / face().units_per_em() as f64
    }
    pub fn ascent(&self) -> f64 {
        face().ascender() as f64 * self.k()
    }
    pub fn descent(&self) -> f64 {
        -(face().descender() as f64) * self.k()
    }
    pub fn line_spacing(&self) -> f64 {
        (face().ascender() as f64 - face().descender() as f64 + face().line_gap() as f64) * self.k()
    }
    pub fn height(&self) -> f64 {
        self.ascent() + self.descent()
    }
    pub fn width(&self, text: &str) -> f64 {
        text.chars()
            .map(|ch| {
                let (f, g, k) = glyph_for(ch, self.px);
                f.glyph_hor_advance(g).unwrap_or(0) as f64 * k
            })
            .sum::<f64>()
    }
    /// Outline of `text` with its baseline starting at (x, y).
    pub fn text_path(&self, x: f64, y: f64, text: &str) -> Path {
        struct Builder<'a> {
            path: &'a mut Path,
            k: f64,
            ox: f64,
            oy: f64,
        }
        impl Builder<'_> {
            fn p(&self, x: f32, y: f32) -> (f64, f64) {
                (self.ox + x as f64 * self.k, self.oy - y as f64 * self.k)
            }
        }
        impl ttf_parser::OutlineBuilder for Builder<'_> {
            fn move_to(&mut self, x: f32, y: f32) {
                let (x, y) = self.p(x, y);
                self.path.move_to(x, y);
            }
            fn line_to(&mut self, x: f32, y: f32) {
                let (x, y) = self.p(x, y);
                self.path.line_to(x, y);
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                let (a, b) = self.p(x1, y1);
                let (x, y) = self.p(x, y);
                self.path.quad_to(a, b, x, y);
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                let (a, b) = self.p(x1, y1);
                let (c, d) = self.p(x2, y2);
                let (x, y) = self.p(x, y);
                self.path.cubic_to(a, b, c, d, x, y);
            }
            fn close(&mut self) {
                self.path.close();
            }
        }
        let mut path = Path { cmds: Vec::new(), fill_rule: FillRule::Winding };
        let mut pen_x = x;
        for ch in text.chars() {
            let (f, g, k) = glyph_for(ch, self.px);
            let mut b = Builder { path: &mut path, k, ox: pen_x, oy: y };
            f.outline_glyph(g, &mut b);
            pen_x += f.glyph_hor_advance(g).unwrap_or(0) as f64 * k;
        }
        path
    }
}

/// How text wraps inside a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wrap {
    Word,
    Anywhere,
}

/// Break `text` into lines no wider than `width` (Qt's TextWordWrap / TextWrapAnywhere).
pub fn wrap_lines(font: &Font, text: &str, width: f64, wrap: Wrap) -> Vec<String> {
    let mut lines = Vec::new();
    match wrap {
        Wrap::Word => {
            let mut line = String::new();
            for word in text.split(' ') {
                let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if !line.is_empty() && font.width(&candidate) > width {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        Wrap::Anywhere => {
            let mut line = String::new();
            for ch in text.chars() {
                let mut candidate = line.clone();
                candidate.push(ch);
                if !line.is_empty() && font.width(&candidate) > width {
                    lines.push(std::mem::take(&mut line).trim_end().to_string());
                    if ch != ' ' {
                        line.push(ch);
                    }
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
    }
    lines
}

// ------------------------------------------------------------------ painter

#[derive(Clone)]
struct State {
    xf: Xf,
    pen: Option<Pen>,
    brush: Option<Color>,
    clip: Option<std::rc::Rc<sk::Mask>>,
    font_px: f64,
}

pub struct Painter<'a> {
    pixmap: sk::PixmapMut<'a>,
    st: State,
    stack: Vec<State>,
}

impl<'a> Painter<'a> {
    pub fn new(pixmap: &'a mut sk::Pixmap) -> Painter<'a> {
        Painter {
            pixmap: pixmap.as_mut(),
            st: State { xf: Xf::IDENTITY, pen: None, brush: None, clip: None, font_px: 11.0 },
            stack: Vec::new(),
        }
    }

    pub fn save(&mut self) {
        self.stack.push(self.st.clone());
    }
    pub fn restore(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.st = s;
        }
    }
    pub fn translate(&mut self, x: f64, y: f64) {
        self.st.xf = self.st.xf.translate(x, y);
    }
    pub fn translate_pt(&mut self, p: Pt) {
        self.translate(p.x, p.y);
    }
    pub fn scale(&mut self, sx: f64, sy: f64) {
        self.st.xf = self.st.xf.scale(sx, sy);
    }
    pub fn rotate(&mut self, deg: f64) {
        self.st.xf = self.st.xf.rotate(deg);
    }
    pub fn transform(&self) -> Xf {
        self.st.xf
    }
    /// QPainter::setTransform(t, combine=false)
    pub fn set_transform(&mut self, t: Xf) {
        self.st.xf = t;
    }
    /// QPainter::setTransform(t, combine=true): t applies first, then the current transform
    pub fn combine_transform(&mut self, t: &Xf) {
        self.st.xf = self.st.xf.then_local(t);
    }

    pub fn set_pen(&mut self, pen: impl Into<Pen>) {
        self.st.pen = Some(pen.into());
    }
    pub fn no_pen(&mut self) {
        self.st.pen = None;
    }
    pub fn set_brush(&mut self, c: Color) {
        self.st.brush = Some(c);
    }
    pub fn no_brush(&mut self) {
        self.st.brush = None;
    }
    pub fn set_font_px(&mut self, px: f64) {
        self.st.font_px = px;
    }

    /// QPainter::setClipPath (ReplaceClip): later drawing only lands inside `path`.
    pub fn set_clip_path(&mut self, path: &Path) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let mut mask = sk::Mask::new(w, h).expect("mask");
        if let Some(sp) = path.to_sk() {
            let rule = match path.fill_rule {
                FillRule::OddEven => sk::FillRule::EvenOdd,
                FillRule::Winding => sk::FillRule::Winding,
            };
            mask.fill_path(&sp, rule, true, self.st.xf.sk());
        }
        self.st.clip = Some(std::rc::Rc::new(mask));
    }

    pub fn fill_rect_device(&mut self, c: Color) {
        self.pixmap.fill(c.sk());
    }

    pub fn draw_path(&mut self, path: &Path) {
        let Some(sp) = path.to_sk() else { return };
        let xf = self.st.xf.sk();
        let clip = self.st.clip.clone();
        if let Some(c) = self.st.brush {
            let mut paint = sk::Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            let rule = match path.fill_rule {
                FillRule::OddEven => sk::FillRule::EvenOdd,
                FillRule::Winding => sk::FillRule::Winding,
            };
            self.pixmap.fill_path(&sp, &paint, rule, xf, clip.as_deref());
        }
        if let Some(pen) = self.st.pen.clone() {
            self.stroke_sk(&sp, &pen);
        }
    }

    fn stroke_sk(&mut self, sp: &sk::Path, pen: &Pen) {
        let mut paint = sk::Paint::default();
        paint.set_color(pen.color.sk());
        paint.anti_alias = true;
        let mut stroke = sk::Stroke {
            width: pen.width as f32,
            line_cap: match pen.cap {
                Cap::Flat => sk::LineCap::Butt,
                Cap::Square => sk::LineCap::Square,
                Cap::Round => sk::LineCap::Round,
            },
            line_join: match pen.join {
                Join::Miter => sk::LineJoin::Miter,
                Join::Bevel => sk::LineJoin::Bevel,
                Join::Round => sk::LineJoin::Round,
            },
            ..Default::default()
        };
        if let Some((pattern, offset)) = &pen.dash {
            let w = pen.width.max(1.0);
            let mut v: Vec<f32> = pattern.iter().map(|d| (d * w) as f32).collect();
            if v.len() % 2 == 1 {
                v.extend_from_within(..);
            }
            stroke.dash = sk::StrokeDash::new(v, (offset * w) as f32);
        }
        let xf = self.st.xf.sk();
        let clip = self.st.clip.clone();
        self.pixmap.stroke_path(sp, &paint, &stroke, xf, clip.as_deref());
    }

    pub fn draw_ellipse(&mut self, c: Pt, rx: f64, ry: f64) {
        let mut p = Path::new();
        p.add_ellipse(c, rx, ry);
        self.draw_path(&p);
    }
    pub fn draw_circle(&mut self, c: Pt, r: f64) {
        self.draw_ellipse(c, r, r);
    }
    pub fn draw_line(&mut self, a: Pt, b: Pt) {
        let Some(pen) = self.st.pen.clone() else { return };
        let mut p = Path::at(a.x, a.y);
        p.line_to(b.x, b.y);
        if let Some(sp) = p.to_sk() {
            self.stroke_sk(&sp, &pen);
        }
    }
    pub fn draw_polygon(&mut self, pts: &[Pt]) {
        let mut p = Path::new();
        p.add_polygon(pts);
        p.close();
        self.draw_path(&p);
    }
    pub fn draw_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let mut p = Path::new();
        p.add_rect(x, y, w, h);
        self.draw_path(&p);
    }

    /// QPainter::drawText(QPointF, text): baseline at (x, y), filled with the pen colour.
    pub fn draw_text(&mut self, x: f64, y: f64, text: &str) {
        let Some(pen) = self.st.pen.clone() else { return };
        let path = Font::new(self.st.font_px).text_path(x, y, text);
        let brush = self.st.brush;
        self.st.pen = None;
        self.st.brush = Some(pen.color);
        self.draw_path(&path);
        self.st.pen = Some(pen);
        self.st.brush = brush;
    }

    /// Lines of text centred horizontally and vertically in the rectangle.
    pub fn draw_text_centered(&mut self, x: f64, y: f64, w: f64, h: f64, lines: &[String]) {
        let font = Font::new(self.st.font_px);
        let ls = font.line_spacing();
        let total = ls * lines.len() as f64;
        let mut base = y + (h - total) / 2.0 + font.ascent() + (ls - font.height()) / 2.0;
        for line in lines {
            let lw = font.width(line);
            self.draw_text(x + (w - lw) / 2.0, base, line);
            base += ls;
        }
    }
}
