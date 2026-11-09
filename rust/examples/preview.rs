//! Render a contact sheet of every mascot's animations to a PNG -- no window needed.
//!
//!     cargo run --example preview -- [out.png] [mascot ...]
//!
//! Port of tools/preview.py, with the same scenes, so the two sheets can be compared.

use desktop_mascots::characters;
use desktop_mascots::engine::{Bounds, Character, Mascot};
use desktop_mascots::gfx::{Color, Painter};

const FLOOR: f64 = 1000.0;
const BOUNDS: Bounds = Bounds { left: 0.0, right: 2000.0, top: 0.0, floor: FLOOR };
const DT: f64 = 1.0 / 60.0;
const CURSOR: (f64, f64) = (250.0, -60.0); // up and to the right: idle mascots look there

fn brain(c: &Character) -> Box<dyn Mascot> {
    let mut b = c.brain(1000.0, FLOOR, Some(3));
    b.update(DT, &BOUNDS, None); // lands immediately
    b.core_mut().go("idle", 99.0);
    b
}

fn step(mut b: Box<dyn Mascot>, seconds: f64) -> Box<dyn Mascot> {
    for _ in 0..((seconds / DT).round() as usize).max(1) {
        b.update(DT, &BOUNDS, Some(CURSOR));
    }
    b
}

fn scene(c: &Character, state: &'static str, duration: f64, at: f64, facing: f64) -> Box<dyn Mascot> {
    let mut b = brain(c);
    b.core_mut().facing = facing;
    b.core_mut().go(state, duration);
    step(b, at)
}

fn dragged(c: &Character) -> Box<dyn Mascot> {
    let mut b = brain(c);
    b.core_mut().grab();
    for i in 0..20 {
        b.core_mut().drag_to(1000.0 + i as f64 * 15.0, FLOOR - 200.0);
        b.update(DT, &BOUNDS, None);
    }
    b
}

fn scenes(c: &'static Character) -> Vec<(String, Box<dyn Fn() -> Box<dyn Mascot>>)> {
    let mut v: Vec<(String, Box<dyn Fn() -> Box<dyn Mascot>>)> = vec![
        ("idle (eyes on cursor)".into(), Box::new(move || scene(c, "idle", 99.0, 0.5, 1.0))),
        ("walk".into(), Box::new(move || scene(c, "walk", 99.0, 0.17, 1.0))),
        ("walk (left)".into(), Box::new(move || scene(c, "walk", 99.0, 0.52, -1.0))),
    ];
    for &(label, state, duration, at) in c.previews {
        v.push((label.into(), Box::new(move || scene(c, state, duration, at, 1.0))));
    }
    v.push(("crouch".into(), Box::new(move || scene(c, "crouch", 0.14, 0.12, 1.0))));
    v.push(("jump".into(), Box::new(move || step(scene(c, "crouch", 0.14, 0.15, 1.0), 0.08))));
    v.push((
        "falling".into(),
        Box::new(move || {
            let mut b = brain(c);
            b.core_mut().y = FLOOR - 400.0;
            b.core_mut().throw(300.0, -100.0);
            step(b, 0.1)
        }),
    ));
    v.push(("dragged".into(), Box::new(move || dragged(c))));
    v.push(("land".into(), Box::new(move || scene(c, "land", 0.22, 0.03, 1.0))));
    v.push(("dizzy".into(), Box::new(move || scene(c, "dizzy", 2.6, 1.0, 1.0))));
    v.push((
        "poked".into(),
        Box::new(move || {
            let mut b = brain(c);
            b.core_mut().say(c.params.lines[0], 1.6, false);
            step(b, 0.1)
        }),
    ));
    v.push((
        "say (sent text)".into(),
        Box::new(move || {
            let mut b = brain(c);
            b.core_mut().say(
                "Your build finished in 3m12s with 0 errors and 2 warnings. Time for a banana break?",
                9.0,
                true,
            );
            step(b, 0.1)
        }),
    ));
    v
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| "preview.png".into());
    let chars: Vec<&'static Character> = if args.len() > 1 {
        args[1..].iter().map(|n| characters::get(n).unwrap_or_else(|| panic!("unknown mascot {n}"))).collect()
    } else {
        characters::all().to_vec()
    };
    let (cols, cw, ch, s) = (5usize, 250.0, 250.0, 1.4);
    let sections: Vec<_> = chars.iter().map(|&c| (c, scenes(c))).collect();
    let rows: usize = sections.iter().map(|(_, sc)| sc.len().div_ceil(cols)).sum();
    let title_h = 28.0;
    let h = rows as f64 * ch + sections.len() as f64 * title_h;
    let mut img = tiny_skia::Pixmap::new((cols as f64 * cw) as u32, h as u32).unwrap();
    {
        let mut p = Painter::new(&mut img);
        p.fill_rect_device(Color::hex("#DDE6EE"));
        let mut y0 = 0.0;
        for (c, items) in &sections {
            p.set_pen(Color::hex("#223322"));
            p.set_font_px(16.0);
            let name = c.name[..1].to_uppercase() + &c.name[1..];
            p.draw_text(10.0, y0 + 20.0, &name);
            y0 += title_h;
            for (i, (label, make)) in items.iter().enumerate() {
                let b = make();
                let x = (i % cols) as f64 * cw;
                let y = y0 + (i / cols) as f64 * ch;
                p.save();
                p.translate(x + cw / 2.0, y + ch - 30.0 - (FLOOR - b.core().y) * 0.15);
                p.scale(s, s);
                b.paint(&mut p, false);
                p.restore();
                p.set_pen(Color::hex("#333344"));
                p.set_font_px(12.0);
                let text = format!("{label}  [{}]", b.core().state);
                p.draw_text_centered(x, y + ch - 24.0, cw, 20.0, &[text]);
            }
            y0 += items.len().div_ceil(cols) as f64 * ch;
        }
    }
    img.save_png(&out).expect("write png");
    println!("wrote {out}");
}
