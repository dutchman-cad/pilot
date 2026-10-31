//! Independently authored development inputs; public recipe forms and CLI only.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::Catalog,
    planner,
    recipe_library::{forms, Library},
};
use serde_json::{json, Value};
use std::process::Command;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/dev_repeated_boss.json")).unwrap()
}
fn request(text: &str) -> Value {
    json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
}
#[test]
fn development_exact_repeated_features_match_forms_call_for_call() {
    let fixture = fixtures();
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let flat = forms::flat(schema);
    assert!(fixture["cases"].as_array().unwrap().len() >= 40);
    for case in fixture["cases"].as_array().unwrap() {
        let req = request(case["prompt"].as_str().unwrap());
        let band = lib.form_band(case["prompt"].as_str().unwrap()).unwrap();
        eprintln!(
            "{} form route: {} {} score {}; hand fill: {}",
            case["id"],
            band["band"],
            band["recipes"][0]["id"],
            band["recipes"][0]["score"],
            case["args"]
        );
        let mut fill = json!({});
        for (key, field) in flat["properties"].as_object().unwrap() {
            if field.get("x-scale").is_some() {
                continue;
            }
            if let Some(value) = field["x-slot"]
                .as_str()
                .and_then(|p| case["args"].pointer(p))
            {
                fill[key] = value.clone();
            }
        }
        if let Some(overrides) = case["fill"].as_object() {
            for (key, value) in overrides {
                fill[key] = value.clone();
            }
        }
        forms::coerce(&mut fill, &flat).unwrap();
        forms::request_defaults(&mut fill, schema, &req).unwrap();
        let slots = forms::to_slots(&fill, schema).unwrap();
        let form = lib
            .build(&json!({"recipe":"plate","args":slots}), &req, &cat)
            .unwrap();
        let exact = planner::plan_direct(&req, &cat)
            .unwrap()
            .unwrap_or_else(|| panic!("no exact match: {}", case["prompt"]));
        assert_eq!(exact.calls, form.calls, "{}", case["id"]);
        assert_eq!(
            serde_json::to_value(exact.expected).unwrap(),
            serde_json::to_value(form.expected).unwrap(),
            "{}",
            case["id"]
        );
    }
    assert!(fixture["declines"].as_array().unwrap().len() >= 12);
    for text in fixture["declines"].as_array().unwrap() {
        assert!(
            planner::plan_direct(&request(text.as_str().unwrap()), &cat)
                .unwrap()
                .is_none(),
            "must decline: {text}"
        );
    }
}

#[test]
fn development_repeated_features_public_cli_replays() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let fixture = fixtures();
    // Every layout plus both row sides and the tall-grid orientation.
    for id in [
        "edge-1", "edge-4", "circle-0", "grid-1", "grid-4", "copies-1",
    ] {
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let plan = planner::plan_direct(&request(case["prompt"].as_str().unwrap()), &cat)
            .unwrap()
            .unwrap();
        let commands: Vec<_> = plan.calls.iter().map(|c| c["arguments"].clone()).collect();
        assert!(commands.len() <= 66);
        let file =
            std::env::temp_dir().join(format!("pilot-repeat-{}-{id}.json", std::process::id()));
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
            "{id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.len() < 4 * 1024 * 1024);
        let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        let state = results.last().unwrap();
        let bodies = state["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), 1, "{id}: {state}");
        assert!(
            state["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["error"].is_null()),
            "{id}: {state}"
        );
        let a = &case["args"];
        let round = a.get("diameter").is_some();
        let x = a[if round { "diameter" } else { "length" }]
            .as_f64()
            .unwrap();
        let y = if round {
            x
        } else {
            a["width"].as_f64().unwrap()
        };
        let t = a["thickness"].as_f64().unwrap();
        let h = a["boss_height"].as_f64().unwrap();
        let r = a["boss_diameter"].as_f64().unwrap() / 2.;
        let n = if let Some(row) = a.get("row") {
            row["count"].as_f64().unwrap()
        } else if let Some(grid) = a.get("grid") {
            grid["rows"].as_f64().unwrap() * grid["columns"].as_f64().unwrap()
        } else {
            a["bolt_circle"]["count"].as_f64().unwrap()
        };
        let volume = if round {
            std::f64::consts::PI * (x / 2.).powi(2) * t
        } else {
            x * y * t
        } + n * std::f64::consts::PI * r * r * h;
        assert!(
            (bodies[0]["volume_mm3"].as_f64().unwrap() - volume).abs()
                < 0.01_f64.max(volume * 0.001),
            "{id}: {state}"
        );
        for (end, expected) in [("min", [0., 0., 0.]), ("max", [x, y, t + h])] {
            for (axis, value) in expected.into_iter().enumerate() {
                assert!(
                    (bodies[0]["bbox"][end][axis].as_f64().unwrap() - value).abs() < 0.001,
                    "{id}: {state}"
                );
            }
        }
    }
}
