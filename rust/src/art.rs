//! Painting helpers shared by every character (port of `desktop_mascots/art.py`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;

use crate::engine::Character;
use crate::gfx::{polyline, wrap_lines, Color, FillRule, Font, Painter, Path, Pen, Pt, Wrap, pt};

pub const OUTLINE: Color = Color::hex("#3A2314");
pub const WHITE: Color = Color::hex("#FFFFFF");
pub const STAR: Color = Color::hex("#FFD84A");
pub const STAR_EDGE: Color = Color::hex("#A9800F");
pub const LINE_W: f64 = 1.6;

/// Round-capped, round-joined pen.
pub fn pen(color: Color, width: f64) -> Pen {
    Pen::round(color, width)
}

/// A thick stroke with the default outline (limbs, tails).
pub fn stroke(p: &mut Painter, path: &Path, color: Color, width: f64) {
    stroke_with(p, path, color, width, OUTLINE, LINE_W);
}

/// A thick stroke with an outline (limbs, tails, necks).
pub fn stroke_with(p: &mut Painter, path: &Path, color: Color, width: f64, outline: Color, outline_w: f64) {
    p.no_brush();
    p.set_pen(pen(outline, width + 2.0 * outline_w));
    p.draw_path(path);
    p.set_pen(pen(color, width));
    p.draw_path(path);
}

/// A filled ellipse with the default outline.
pub fn blob(p: &mut Painter, c: Pt, rx: f64, ry: f64, fill: Color) {
    blob_with(p, c, rx, ry, fill, OUTLINE, LINE_W);
}

pub fn blob_with(p: &mut Painter, c: Pt, rx: f64, ry: f64, fill: Color, outline: Color, outline_w: f64) {
    p.set_pen(pen(outline, outline_w));
    p.set_brush(fill);
    p.draw_ellipse(c, rx, ry);
}

/// A filled shape: a cubic Bézier spine swept with a width tapering from w0 to w1 (necks).
#[allow(clippy::too_many_arguments)]
pub fn tapered_curve(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    w0: f64,
    w1: f64,
    steps: usize,
    power: f64,
) -> Path {
    let mut left = Vec::with_capacity(steps + 1);
    let mut right = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let u = 1.0 - t;
        let x = u.powi(3) * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t.powi(3) * p3.0;
        let y = u.powi(3) * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t.powi(3) * p3.1;
        let dx = 3.0 * u * u * (p1.0 - p0.0) + 6.0 * u * t * (p2.0 - p1.0) + 3.0 * t * t * (p3.0 - p2.0);
        let dy = 3.0 * u * u * (p1.1 - p0.1) + 6.0 * u * t * (p2.1 - p1.1) + 3.0 * t * t * (p3.1 - p2.1);
        let d = dx.hypot(dy);
        let d = if d == 0.0 { 1.0 } else { d };
        let (nx, ny) = (-dy / d, dx / d);
        let w = (w1 + (w0 - w1) * u.powf(power)) / 2.0;
        left.push(pt(x + nx * w, y + ny * w));
        right.push(pt(x - nx * w, y - ny * w));
    }
    let mut path = Path::new();
    path.fill_rule = FillRule::Winding; // tight bends may fold the outline over itself
    left.extend(right.into_iter().rev());
    path.add_polygon(&left);
    path.close();
    path.simplified()
}

/// `tapered_curve` with the Python defaults (steps=16, power=1.6).
pub fn tapered(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), w0: f64, w1: f64) -> Path {
    tapered_curve(p0, p1, p2, p3, w0, w1, 16, 1.6)
}

pub fn paint_stars(p: &mut Painter, cx: f64, cy: f64, angle: f64, rx: f64, ry: f64) {
    for i in 0..3 {
        let a = angle + i as f64 * 2.0 * PI / 3.0;
        let c = pt(cx + rx * a.cos(), cy + ry * a.sin());
        let pts: Vec<Pt> = (0..10)
            .map(|k| {
                let r = if k % 2 == 0 { 4.6 } else { 2.0 };
                let t = -PI / 2.0 + k as f64 * PI / 5.0 + a * 2.0;
                pt(c.x + r * t.cos(), c.y + r * t.sin())
            })
            .collect();
        p.set_pen(pen(STAR_EDGE, 1.0));
        p.set_brush(STAR);
        p.draw_polygon(&pts);
    }
}

pub const BUBBLE_TEXT_W: f64 = 196.0; // wrap width
pub const BUBBLE_LINES: usize = 5; // longer messages are cut with an ellipsis
const BUBBLE_PX: f64 = 11.0;

#[derive(Clone)]
struct BubbleLayout {
    lines: Vec<String>,
    width: f64,
    height: f64,
}

thread_local! {
    static BUBBLES: RefCell<HashMap<String, BubbleLayout>> = RefCell::new(HashMap::new());
}

/// Wrap (and if needed truncate) text into at most BUBBLE_LINES lines.
fn bubble_layout(text: &str) -> BubbleLayout {
    if let Some(l) = BUBBLES.with(|c| c.borrow().get(text).cloned()) {
        return l;
    }
    let font = Font::new(BUBBLE_PX);
    let measure = |t: &str, wrap: Wrap| -> (Vec<String>, f64) {
        let lines = wrap_lines(&font, t, BUBBLE_TEXT_W, wrap);
        let w = lines.iter().map(|l| font.width(l)).fold(0.0, f64::max);
        (lines, w)
    };
    let mut wrap = Wrap::Word;
    if measure(text, wrap).1 > BUBBLE_TEXT_W {
        wrap = Wrap::Anywhere; // a word too long to wrap (URL, path…)
    }
    let mut shown = text.to_string();
    if measure(text, wrap).0.len() > BUBBLE_LINES {
        let (mut parts, joiner): (Vec<String>, &str) = match wrap {
            Wrap::Anywhere => (text.chars().map(String::from).collect(), ""),
            Wrap::Word => (text.split(' ').map(String::from).collect(), " "),
        };
        while !parts.is_empty() && measure(&(parts.join(joiner) + "…"), wrap).0.len() > BUBBLE_LINES {
            parts.pop();
        }
        shown = parts.join(joiner).trim_end().to_string() + "…";
    }
    let (lines, w) = measure(&shown, wrap);
    let layout = BubbleLayout {
        height: font.line_spacing() * lines.len() as f64,
        width: w.min(BUBBLE_TEXT_W),
        lines,
    };
    BUBBLES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert(text.to_string(), layout.clone());
    });
    layout
}

/// Glyphs (sleepy z's, music notes) drifting up and away from (x, y); drawn unmirrored.
pub fn paint_zzz(p: &mut Painter, x: f64, y: f64, phase: f64, facing: f64, glyphs: &str, color: Color) {
    p.save();
    p.scale(facing, 1.0); // undo the caller's mirroring so the letters read correctly
    let x = x * facing;
    let n = glyphs.chars().count() as f64;
    for (i, glyph) in glyphs.chars().enumerate() {
        let k = (phase * 0.6 + i as f64 / n).rem_euclid(1.0);
        p.set_font_px((7.0 + 7.0 * k).round());
        p.set_pen(color.with_alpha_f((1.4 - 1.4 * k).clamp(0.0, 1.0)));
        p.draw_text(x + facing * (4.0 + 14.0 * k), y - 4.0 - 22.0 * k, &glyph.to_string());
    }
    p.restore();
}

/// Speech bubble whose tail points at (ax, ay); drawn unmirrored. Wraps long text.
pub fn paint_bubble(p: &mut Painter, text: &str, ax: f64, ay: f64, facing: f64, outline: Color, limit: f64) {
    p.save();
    p.set_font_px(BUBBLE_PX);
    let l = bubble_layout(text);
    let w = l.width + 16.0;
    let h = l.height + 9.0;
    let x = if facing > 0.0 { ax - 8.0 } else { ax + 8.0 - w };
    let x = x.min(limit - w).max(-limit);
    let (ry, rh) = (ay - 8.0 - h, h);
    let bottom = ry + rh;
    let mut shape = Path::new();
    shape.add_rounded_rect(x, ry, w, rh, 8.0, 8.0);
    let mut tail = polyline(&[(ax - 4.0, bottom - 1.0), (ax, ay), (ax + 4.0, bottom - 1.0)]);
    tail.close();
    p.set_pen(pen(outline, 1.4));
    p.set_brush(WHITE);
    p.draw_path(&shape.united(&tail));
    p.set_pen(outline);
    p.draw_text_centered(x, ry, w, rh, &l.lines);
    p.restore();
}

/// The bubble with the Python defaults (limit=117).
pub fn bubble(p: &mut Painter, text: &str, ax: f64, ay: f64, facing: f64, outline: Color) {
    paint_bubble(p, text, ax, ay, facing, outline, 117.0);
}

/// The character's head, framed by its icon_box, as straight (unpremultiplied) RGBA.
pub fn render_icon(character: &Character, size: u32) -> tiny_skia::Pixmap {
    let (cx, cy, extent) = character.icon_box;
    let mut pm = tiny_skia::Pixmap::new(size, size).expect("icon pixmap");
    {
        let mut p = Painter::new(&mut pm);
        let s = size as f64;
        p.translate(s / 2.0, s / 2.0);
        p.scale(s / extent, s / extent);
        p.translate(-cx, -cy);
        let brain = character.brain(0.0, 0.0, Some(0));
        brain.paint_icon(&mut p);
    }
    pm
}
