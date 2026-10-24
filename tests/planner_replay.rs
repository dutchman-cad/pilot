//! Replay complete chat-completions bodies, including their original tool offers.
#![allow(clippy::unwrap_used)]
use dutchman_pilot::{catalog::Catalog, planner, repair};
use serde::Deserialize;
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    request_file: String,
    expected: Option<Expected>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    rule: String,
    slots: Value,
    calls: Value,
}
fn load(path: &Path) -> Value {
    let mut bytes = Vec::new();
    File::open(path)
        .unwrap()
        .take(repair::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(
        bytes.len() <= repair::MAX_BYTES,
        "oversized replay: {}",
        path.display()
    );
    repair::parse_json(std::str::from_utf8(&bytes).unwrap()).unwrap()
}
#[test]
fn recorded_chat_completion_bodies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/planner-replay");
    let fixtures: Vec<Fixture> = serde_json::from_value(load(&root.join("cases.json"))).unwrap();
    assert!((3..=128).contains(&fixtures.len()));
    let catalog = Catalog::bundled().unwrap();
    for fixture in fixtures {
        assert!(!fixture.request_file.contains(['/', '\\']));
        let request = load(&root.join(&fixture.request_file));
        let actual = planner::plan(&request, &catalog).unwrap();
        match fixture.expected {
            None => assert!(actual.is_none(), "{}", fixture.request_file),
            Some(expected) => {
                let actual = actual.unwrap();
                assert_eq!(actual.rule, expected.rule, "{}", fixture.request_file);
                assert_eq!(actual.slots, expected.slots, "{}", fixture.request_file);
                assert_eq!(
                    serde_json::to_value(actual.calls).unwrap(),
                    expected.calls,
                    "{}",
                    fixture.request_file
                );
            }
        }
    }
}

#[test]
fn first_turn_regressions_have_long_context_and_one_user() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/planner-replay");
    for file in [
        "rectangle-plane.json",
        "circle-order.json",
        "relative-holes.json",
        "body-count.json",
    ] {
        let request = load(&root.join(file));
        let messages = request["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2, "{file}");
        assert_eq!(messages[0]["role"], "system");
        assert!(messages[0]["content"].as_str().unwrap().len() > 16_384);
        assert_eq!(messages[1]["role"], "user");
    }
}

#[test]
fn long_system_context_and_offered_tools_are_replayed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/planner-replay");
    let mut request = load(&root.join("box.json"));
    let system = request["messages"][0]["content"]
        .as_str()
        .unwrap()
        .repeat(16);
    assert!(system.len() > 16_384);
    request["messages"][0]["content"] = Value::String(system);
    let c = Catalog::bundled().unwrap();
    assert_eq!(planner::plan(&request, &c).unwrap().unwrap().rule, "box");
    request["tools"] = serde_json::json!([]);
    assert!(planner::plan(&request, &c).unwrap().is_none());
}

#[cfg(feature = "proxy")]
#[test]
fn offline_reports_replay_every_recorded_request() {
    use dutchman_pilot::report;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/planner-replay");
    let fixtures: Vec<Fixture> = serde_json::from_value(load(&root.join("cases.json"))).unwrap();
    for fixture in fixtures {
        let request = load(&root.join(&fixture.request_file));
        let turn = report::offline(&request).unwrap();
        let (_, text) = report::render(&turn, false).unwrap();
        assert!(!text.contains("Request text (explicitly"));
        match fixture.expected {
            Some(expected) => {
                assert_eq!(turn.shape["intent"], expected.rule);
                assert_eq!(turn.shape["slots"], expected.slots);
            }
            None => assert!(turn.failed),
        }
        assert!(text.contains("redacted:"));
    }
}
