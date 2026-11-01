//! Clean-room exact/form parity and public-binary development replays.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "support/dev_exact_enclosure.rs"]
mod fixtures;
use dutchman_pilot::{catalog::Catalog, planner, recipe_library::Library};
use serde_json::{json, Value};
use std::process::Command;

fn request(text: &str) -> Value {
    json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
}
fn compare(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-9,
            "{a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{a:?} != {b:?}");
            for (k, v) in a {
                compare(v, &b[k]);
            }
        }
        _ => assert_eq!(a, b),
    }
}
#[test]
fn exact_enclosures_match_form_calls() {
    let cat = Catalog::bundled().unwrap();
    let lib = Library::embedded().unwrap();
    let cases = fixtures::cases();
    assert_eq!(cases.len(), 42);
    for (text, args) in cases {
        let req = request(&text);
        let exact = planner::plan_direct(&req, &cat)
            .unwrap()
            .unwrap_or_else(|| panic!("declined: {text}"));
        let form = lib
            .build(&json!({"recipe":"enclosure","args":args}), &req, &cat)
            .unwrap();
        eprintln!("parity: {text}");
        compare(&json!(exact.calls), &json!(form.calls));
    }
}
#[test]
fn unclear_enclosures_decline_without_errors() {
    let cat = Catalog::bundled().unwrap();
    for text in fixtures::DECLINES {
        assert!(
            planner::plan_direct(&request(text), &cat)
                .unwrap()
                .is_none(),
            "claimed: {text}"
        );
    }
}
#[test]
fn exact_enclosures_public_cli_bounds_and_volume() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    // Two examples of each kind, including cm and independently stated mixed units.
    for index in [0, 1, 2, 27, 28, 29] {
        let (text, args) = &fixtures::cases()[index];
        let plan = planner::plan_direct(&request(text), &cat).unwrap().unwrap();
        let commands: Vec<_> = plan.calls.iter().map(|c| c["arguments"].clone()).collect();
        assert!(commands.len() <= 12);
        let file = std::env::temp_dir().join(format!(
            "pilot-exactenc-{}-{index}.json",
            std::process::id()
        ));
        std::fs::write(&file, serde_json::to_vec(&commands).unwrap()).unwrap();
        let output = Command::new(&cli)
            .arg("run")
            .arg(&file)
            .arg("--results")
            .output()
            .unwrap();
        std::fs::remove_file(file).unwrap();
        assert!(
            output.status.success(),
            "{text}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.len() < 4 * 1024 * 1024);
        let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(results.len(), commands.len());
        let state = results.last().unwrap();
        assert_eq!(
            state["bodies"].as_array().unwrap().len(),
            1,
            "{text}: {state}"
        );
        assert!(
            state["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["error"].is_null()),
            "{state}"
        );
        let body = &state["bodies"][0];
        let x = args["length"].as_f64().unwrap();
        let y = args["width"].as_f64().unwrap();
        let z = args["height"].as_f64().unwrap();
        let t = 2.;
        let floor = args["floor"].as_f64().unwrap_or(t);
        let lip = args.get("lip").is_some();
        let volume = x * y * z - (x - 2. * t) * (y - 2. * t) * (z - floor)
            + if lip {
                (x * y - (x - 2.) * (y - 2.)) * 3.
            } else {
                0.
            };
        assert!(
            (body["volume_mm3"].as_f64().unwrap() - volume).abs() < 0.01,
            "{text}: {body}"
        );
        for (end, expected) in [
            ("min", [0., 0., if lip { -3. } else { 0. }]),
            ("max", [x, y, z]),
        ] {
            for (axis, v) in expected.into_iter().enumerate() {
                assert!(
                    (body["bbox"][end][axis].as_f64().unwrap() - v).abs() < 0.001,
                    "{text}: {body}"
                );
            }
        }
    }
}
