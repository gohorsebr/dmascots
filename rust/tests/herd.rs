//! The running app's command handling (list/add/remove/say/steal), port of tests/test_herd.py.
//! Runs on the headless backend: real herd and brains, no screen.

use serde_json::{json, Value};

use desktop_mascots::app::{Herd, Options};
use desktop_mascots::platform::Headless;

fn herd() -> Herd {
    Herd::new(
        Box::new(Headless::default()),
        &Options { scale: 1.0, tray: false, menu: false, click_through: false, quit_when_empty: false },
    )
}

fn cmd(h: &mut Herd, req: Value) -> Value {
    let reply = h.handle(&req);
    assert_eq!(reply["ok"], true, "{reply}");
    reply
}

fn ids(reply: &Value) -> Vec<u64> {
    reply["mascots"].as_array().unwrap().iter().map(|m| m["id"].as_u64().unwrap()).collect()
}

#[test]
fn add_and_list() {
    let mut h = herd();
    let added = cmd(&mut h, json!({"cmd": "spawn", "mascots": ["goose", "cat", "goose"]}));
    let names: Vec<&str> = added["mascots"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["goose", "cat", "goose"]);
    assert_eq!(ids(&added), [1, 2, 3]);
    let listed = cmd(&mut h, json!({"cmd": "list"}));
    let pairs: Vec<(u64, &str)> = listed["mascots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["id"].as_u64().unwrap(), m["name"].as_str().unwrap()))
        .collect();
    assert_eq!(pairs, [(1, "goose"), (2, "cat"), (3, "goose")]);
    for m in listed["mascots"].as_array().unwrap() {
        for key in ["state", "x", "y", "leaves_in", "holding_pointer"] {
            assert!(m.get(key).is_some(), "missing {key}");
        }
    }
}

#[test]
fn remove_by_id_name_and_all() {
    let mut h = herd();
    cmd(&mut h, json!({"cmd": "spawn", "mascots": ["goose", "cat", "goose", "horse"]}));
    assert_eq!(ids(&cmd(&mut h, json!({"cmd": "remove", "targets": ["2"]}))), [2]);
    assert_eq!(ids(&cmd(&mut h, json!({"cmd": "remove", "targets": ["goose"]}))), [1, 3]);
    assert_eq!(cmd(&mut h, json!({"cmd": "remove", "targets": ["7", "parrot"]}))["count"], 0);
    assert_eq!(ids(&cmd(&mut h, json!({"cmd": "remove", "targets": ["all"]}))), [4]);
    assert_eq!(cmd(&mut h, json!({"cmd": "list"}))["mascots"], json!([]));
    assert!(!h.quitting); // an empty herd keeps running
    assert_eq!(ids(&cmd(&mut h, json!({"cmd": "spawn", "mascots": ["crow"]}))), [5]); // ids never reused
}

#[test]
fn app_started_with_time_exits_when_empty() {
    let mut h = herd();
    h.quit_when_empty = true;
    cmd(&mut h, json!({"cmd": "spawn", "mascots": ["cat"]}));
    cmd(&mut h, json!({"cmd": "remove", "targets": ["all"]}));
    assert!(h.quitting);
}

#[test]
fn say_and_steal_target_one_mascot_by_id() {
    let mut h = herd();
    cmd(&mut h, json!({"cmd": "spawn", "mascots": ["goose", "goose"]}));
    assert_eq!(cmd(&mut h, json!({"cmd": "say", "text": "just you", "to": "2"}))["count"], 1);
    assert_eq!(h.windows[0].brain.core().heard, Vec::<String>::new());
    assert_eq!(h.windows[1].brain.core().heard, ["just you"]);
    let stolen = cmd(&mut h, json!({"cmd": "steal", "to": "1", "seconds": 3}));
    assert_eq!(ids(&stolen), [1]);
    // just spawned, so it is still dropping in: the theft starts once it lands
    let first = h.windows[0].brain.core();
    assert!(first.state == "steal" || first.pending_steal.is_some());
    assert_eq!(h.handle(&json!({"cmd": "steal", "to": "2"}))["ok"], false); // one thief at a time
}

#[test]
fn lifetime_shows_in_list() {
    let mut h = herd();
    cmd(&mut h, json!({"cmd": "spawn", "mascots": ["parrot"], "lifetime": 30}));
    let listed = cmd(&mut h, json!({"cmd": "list"}));
    let left = listed["mascots"][0]["leaves_in"].as_f64().unwrap();
    assert!((left - 30.0).abs() < 1.0, "{left}");
}

#[test]
fn click_through_applies_to_new_mascots() {
    let mut h = herd();
    h.set_click_through(true);
    cmd(&mut h, json!({"cmd": "spawn", "mascots": ["monkey"]}));
    assert!(h.windows[0].click_through);
}

#[test]
fn unknown_command_is_refused() {
    let mut h = herd();
    let reply = h.handle(&json!({"cmd": "dance"}));
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"], "unknown command: 'dance'");
}
