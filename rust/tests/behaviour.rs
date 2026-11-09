//! Headless behaviour tests for every mascot (port of tests/test_mascots.py).

use std::collections::HashSet;

use desktop_mascots::characters;
use desktop_mascots::engine::{Bounds, Character, Mascot, Pointer};

const DT: f64 = 1.0 / 60.0;
const B: Bounds = Bounds { left: 50.0, right: 950.0, top: 150.0, floor: 800.0 };

fn step(b: &mut dyn Mascot, seconds: f64, bounds: &Bounds, cursor: (f64, f64)) -> HashSet<&'static str> {
    let mut seen = HashSet::new();
    for _ in 0..(seconds / DT).round() as usize {
        b.update(DT, bounds, Some(cursor));
        seen.insert(b.core().state);
    }
    seen
}

fn each(check: impl Fn(&'static Character)) {
    for c in characters::all() {
        eprintln!("mascot: {}", c.name);
        check(c);
    }
}

#[test]
fn wanders_inside_bounds_and_uses_every_behaviour() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor - 10.0, Some(7));
        let mut expected: HashSet<&str> = c.params.choices.iter().map(|ch| ch.1).collect();
        expected.insert("air");
        expected.insert("land");
        let mut seen = HashSet::new();
        for i in 0..(900.0 / DT) as usize {
            // sweep the cursor around so cursor-driven behaviours get exercised
            let cx = if (i / 300) % 2 == 1 { 300.0 } else { -300.0 };
            b.update(DT, &B, Some((cx, -40.0)));
            seen.insert(b.core().state);
            if !b.core().airborne() {
                assert_eq!(b.core().y, B.floor, "{} {}", c.name, b.core().state);
                assert!(B.left <= b.core().x && b.core().x <= B.right, "{} x={}", c.name, b.core().x);
            }
            if expected.is_subset(&seen) {
                break;
            }
        }
        let missing: Vec<_> = expected.difference(&seen).collect();
        assert!(missing.is_empty(), "{} never entered: {missing:?}", c.name);
    });
}

#[test]
fn drag_and_throw() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor, Some(1));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().grab();
        for i in 0..30 {
            b.core_mut().drag_to(500.0 + i as f64 * 25.0, 300.0);
            b.update(DT, &B, None);
        }
        assert_eq!(b.core().state, "dragged");
        assert_ne!(b.pose().swing, 0.0);
        b.core_mut().release();
        assert_eq!(b.core().state, "air");
        assert!(b.core().vx > 0.0);
        step(b.as_mut(), 3.0, &B, (30.0, -40.0));
        assert!(!b.core().airborne(), "{}", c.name);
        assert_eq!(b.core().y, B.floor);
    });
}

#[test]
fn hard_drop_makes_it_dizzy() {
    let tall = Bounds { left: 50.0, right: 950.0, top: 0.0, floor: 1200.0 };
    each(|c| {
        let mut b = c.brain(500.0, 0.0, Some(2));
        assert!(step(b.as_mut(), 1.5, &tall, (30.0, -40.0)).contains("dizzy"), "{}", c.name);
    });
}

#[test]
fn paused_stays_put() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor, Some(4));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().paused = true;
        step(b.as_mut(), 0.1, &B, (30.0, -40.0));
        let x = b.core().x;
        step(b.as_mut(), 30.0, &B, (400.0, 0.0));
        assert!((b.core().x - x).abs() < 1e-6, "{} moved {} -> {}", c.name, x, b.core().x);
    });
}

#[test]
fn poke_reacts_and_talks() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor, Some(5));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().go("idle", 99.0);
        b.poke();
        assert_ne!(b.core().state, "idle", "{}", c.name);
        step(b.as_mut(), 0.1, &B, (30.0, -40.0));
        assert!(b.pose().bubble.is_some(), "{}", c.name);
    });
}

#[test]
fn sent_message_is_not_overwritten_by_chatter() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor, Some(6));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().say("build finished", 3.0, true);
        b.poke(); // pokes, barks and honks would normally talk
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        assert_eq!(b.pose().bubble.as_deref(), Some("build finished"), "{}", c.name);
        step(b.as_mut(), 2.5, &B, (30.0, -40.0));
        assert_ne!(b.pose().bubble.as_deref(), Some("build finished"), "{}", c.name);
    });
}

#[test]
fn parrot_repeats_what_it_was_told() {
    let Some(parrot) = characters::get("parrot") else { return }; // not ported yet
    let mut b = parrot.brain(500.0, B.floor, Some(8));
    step(b.as_mut(), 1.0, &B, (30.0, -40.0));
    b.core_mut().say("deploy is green", 0.5, true);
    step(b.as_mut(), 1.0, &B, (30.0, -40.0));
    let mut said = HashSet::new();
    for _ in 0..20 {
        b.core_mut().go("mimic", 2.0);
        step(b.as_mut(), 0.1, &B, (30.0, -40.0));
        said.insert(b.pose().bubble.clone());
    }
    assert!(said.contains(&Some("deploy is green".to_string())));
}

#[test]
fn steals_the_pointer_and_gives_it_back() {
    each(|c| {
        let mut b = c.brain(300.0, B.floor, Some(9));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().go("idle", 99.0);
        let mut cursor = (800.0, 300.0); // absolute pointer position, far away
        let mut phases: Vec<&str> = Vec::new();
        let mut held = 0.0;
        assert!(b.core_mut().steal(2.0));
        for _ in 0..(12.0 / DT).round() as usize {
            let (x, y) = (b.core().x, b.core().y);
            b.update(DT, &B, Some((cursor.0 - x, cursor.1 - (y - c.eye_y))));
            let core = b.core();
            if let Some(ptr) = core.pointer {
                // the window would now move the real pointer
                cursor = (core.x + c.grip.0 * core.facing, core.y + c.grip.1);
                if ptr == Pointer::Hold {
                    held += DT;
                }
            }
            assert!(B.left <= core.x && core.x <= B.right);
            let phase = match core.pointer {
                None => "free",
                Some(Pointer::Pull(_)) => "pull",
                Some(Pointer::Hold) => "hold",
            };
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
            if core.state != "steal" {
                break;
            }
        }
        assert_eq!(phases, ["free", "pull", "hold", "free"], "{}", c.name); // chase, snatch, carry, give back
        assert!((held - 2.0f64).abs() < 0.1, "{} held {held}", c.name);
        assert!(b.core().pointer.is_none());
        let bubble = b.pose().bubble.clone().unwrap_or_default();
        assert!(c.params.return_lines.contains(&bubble.as_str()), "{} said {bubble:?}", c.name);
    });
}

#[test]
fn clicking_the_thief_gets_the_pointer_back() {
    each(|c| {
        let mut b = c.brain(500.0, B.floor, Some(10));
        step(b.as_mut(), 1.0, &B, (30.0, -40.0));
        b.core_mut().steal(30.0);
        step(b.as_mut(), 2.0, &B, (0.0, 0.0)); // pointer is right there: snatched and carried
        assert_eq!(b.core().pointer, Some(Pointer::Hold), "{}", c.name);
        b.poke();
        step(b.as_mut(), 0.05, &B, (0.0, 0.0));
        assert!(b.core().pointer.is_none(), "{}", c.name);
        assert_ne!(b.core().state, "steal");
    });
}

#[test]
fn steal_requested_mid_air_waits_for_landing() {
    let goose = characters::get("goose").unwrap();
    let mut b = goose.brain(500.0, B.floor - 300.0, Some(11));
    b.core_mut().steal(1.0);
    assert_eq!(b.core().state, "air");
    let seen = step(b.as_mut(), 2.0, &B, (0.0, 0.0));
    assert!(seen.contains("steal"));
}
