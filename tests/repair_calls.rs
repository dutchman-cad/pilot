#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::Catalog,
    repair::{failed_sketch_context, repair_command},
};
use serde_json::{json, Value};
#[path = "support/callrepair_cases.rs"]
mod cases;

#[test]
fn callrepair_accepted_audited_idempotent() {
    let catalog = Catalog::bundled().unwrap();
    let cases = cases::accepted();
    assert!(cases.len() >= 40);
    for (before, expected) in cases {
        let mut actual = before.clone();
        let rewrites = repair_command(&mut actual, &catalog).unwrap();
        assert_eq!(actual, expected, "{before}");
        assert_eq!(rewrites.len(), 1, "{before}");
        assert_eq!(rewrites[0].before, before);
        assert_eq!(rewrites[0].after, actual);
        assert!(
            repair_command(&mut actual, &catalog).unwrap().is_empty(),
            "{actual}"
        );
    }
}
#[test]
fn callrepair_ambiguous_calls_stay_errors_transactionally() {
    let catalog = Catalog::bundled().unwrap();
    let cases = cases::refused();
    assert!(cases.len() >= 15);
    for before in cases {
        let mut actual = before.clone();
        let error = repair_command(&mut actual, &catalog).unwrap_err();
        assert_eq!(actual, before);
        assert!(error.contains("; use {\"command\":"), "{error}");
        assert!(error.len() < 2048, "{error}");
    }
}
#[test]
fn callrepair_preserves_native_forms_expressions_and_catalogue_commands() {
    let catalog = Catalog::bundled().unwrap();
    for mut call in [
        json!({"command":"solid.combine","params":{"target":"A","tools":["B"],"operation":"intersect"}}),
        json!({"command":"solid.extrude","params":{"distance":"width / 2","operation":"auto"}}),
        json!({"command":"sketch.create","params":{"plane":"XY","offset":"height + 2"}}),
        json!({"command":"sketch.create","params":{"plane":{"face":[1,2,3],"body":"A","index":2}}}),
        json!({"command":"solid.hole","params":{"position":["width / 2",3,"height"],"diameter":"bolt"}}),
        json!({"command":"solid.hole","params":{"sketch":"Locations","points":["p1"],"diameter":3}}),
        json!({"command":"solid.move","params":{"bodies":["A","B"],"translate":["distance",0,0]}}),
        json!({"command":"solid.fillet","params":{"edges":[[1,2,3],{"body":"A","index":0}],"radius":"r"}}),
    ] {
        let before = call.clone();
        assert!(repair_command(&mut call, &catalog).unwrap().is_empty());
        assert_eq!(call, before);
    }
    let catalog = Catalog::from_json(r#"[{"id":"solid.cut","label":"Native cut","params":"target, tool"},{"id":"solid.combine","label":"Combine","params":"target, tools"}]"#).unwrap();
    let mut native = json!({"command":"solid.cut","params":{"target":"Blank","tool":"Drill"}});
    assert!(repair_command(&mut native, &catalog).unwrap().is_empty());
}
#[test]
fn callrepair_failed_sketch_context_requires_matching_failure() {
    for command in ["sketch.circle.center", "sketch.rectangle.two_point"] {
        let args = json!({"commands":[{"command":"sketch.create","params":{"plane":"Z=oops"}},{"command":command,"params":{"center":[2,3],"radius":4}}]});
        let result =
            json!({"results":[{"error":"no plane Z=oops"},{"error":"no sketch is being edited"}]});
        let hint = failed_sketch_context(&args, &result).unwrap();
        assert!(hint.contains("earlier sketch.create at batch step 0 failed: no plane Z=oops"));
        let stopped =
            json!({"failed":{"index":0,"error":"no plane Z=oops"},"not_run":[1],"results":[]});
        assert!(failed_sketch_context(&args, &stopped)
            .unwrap()
            .contains("no plane Z=oops"));
        assert!(failed_sketch_context(
            &args,
            &json!({"results":[{"sketch":1},{"error":"no sketch is being edited"}]})
        )
        .is_none());
        assert!(failed_sketch_context(
            &args,
            &json!({"results":[{"error":"bad plane"},{"error":"radius must be positive"}]})
        )
        .is_none());
    }
    let args = json!({"commands":[{"command":"sketch.create"},{"command":"sketch.create"},{"command":"sketch.circle.center"}]});
    assert!(failed_sketch_context(
        &args,
        &json!({"results":[{"error":"bad plane"},{"sketch":2},{"error":"no sketch"}]})
    )
    .is_none());
}

fn native(
    cli: &std::ffi::OsStr,
    steps: &[Value],
    file: &std::path::Path,
) -> Result<Vec<Value>, String> {
    std::fs::write(file, serde_json::to_vec(steps).unwrap()).unwrap();
    let output = std::process::Command::new(cli)
        .arg("run")
        .arg(file)
        .args(["--results", "--quiet"])
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
fn setup(call: &Value) -> Vec<Value> {
    let mut steps = vec![
        json!({"command":"solid.box","params":{"length":20,"width":20,"height":10,"body_name":"Blank","operation":"new"}}),
    ];
    match call["command"].as_str().unwrap() {
        "solid.combine" => steps.push(json!({"command":"solid.box","params":{"length":10,"width":10,"height":10,"corner":[15,0,0],"body_name":"Drill","operation":"new"}})),
        "solid.extrude" => {
            let offset = if call["params"]["operation"]=="cut" { 0 } else { 10 };
            steps.extend([
                json!({"command":"sketch.create","params":{"plane":"XY","offset":offset}}),
                json!({"command":"sketch.rectangle.two_point","params":{"p0":[2,2],"p1":[6,7]}}),
                json!({"command":"sketch.finish","params":{}}),
            ]);
        }
        _ => (),
    }
    steps
}
#[test]
fn callrepair_public_cli_replays_measure_intent() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let output = std::process::Command::new(&cli)
        .arg("commands")
        .output()
        .unwrap();
    assert!(output.status.success());
    let catalog = Catalog::from_json(std::str::from_utf8(&output.stdout).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("pilot-callrepair-replay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("development.json");
    let mut baseline_errors = 0;
    for (before, expected) in cases::accepted() {
        let mut repaired = before.clone();
        repair_command(&mut repaired, &catalog).unwrap();
        assert_eq!(repaired, expected);
        let mut prefix = setup(&expected);
        let mut original = prefix.clone();
        original.push(before.clone());
        if native(&cli, &original, &file).is_err() {
            baseline_errors += 1;
        }
        prefix.push(repaired.clone());
        if expected["command"] == "sketch.create" {
            prefix.extend([
                json!({"command":"sketch.circle.center","params":{"center":[0,0],"radius":2}}),
                json!({"command":"sketch.finish","params":{}}),
                json!({"command":"solid.extrude","params":{"distance":3,"operation":"new","body_name":"Probe"}}),
            ]);
        }
        prefix.push(json!({"command":"document.inspect","params":{"measure":true}}));
        let results =
            native(&cli, &prefix, &file).unwrap_or_else(|e| panic!("{before} => {repaired}: {e}"));
        let doc = results.last().unwrap();
        let bodies = doc["bodies"].as_array().unwrap();
        let volume: f64 = bodies
            .iter()
            .map(|b| b["volume_mm3"].as_f64().unwrap())
            .sum();
        let intended = match expected["command"].as_str().unwrap() {
            "solid.combine" if expected["params"]["operation"] == "cut" => 3500.0,
            "solid.combine" => 4500.0,
            "solid.extrude" if expected["params"]["operation"] == "cut" => 3860.0,
            "solid.extrude" => 4140.0,
            "solid.hole" => 4000.0 - std::f64::consts::PI * 10.0,
            "solid.move" => 4000.0,
            "sketch.create" => 4000.0 + std::f64::consts::PI * 4.0 * 3.0,
            "solid.fillet" => {
                assert!(volume < 4000.0 && volume > 3990.0, "{doc}");
                volume
            }
            _ => panic!("missing intent measurement"),
        };
        assert!(
            (volume - intended).abs() < 0.02,
            "{before}: intended {intended}, got {doc}"
        );
        if expected["command"] == "solid.move" {
            let bbox = &bodies[0]["bbox"];
            for axis in 0..3 {
                let delta = expected["params"]["translate"][axis].as_f64().unwrap();
                assert!((bbox["min"][axis].as_f64().unwrap() - delta).abs() < 0.001);
                let size = [20.0, 20.0, 10.0][axis];
                assert!((bbox["max"][axis].as_f64().unwrap() - delta - size).abs() < 0.001);
            }
        }
        if expected["command"] == "sketch.create" {
            let probe = bodies.iter().find(|b| b["name"] == "Probe").unwrap();
            let plane = &expected["params"]["plane"];
            let (axis, start) = if let Some(plane) = plane.as_str() {
                let axis = match plane {
                    "XY" => 2,
                    "XZ" => 1,
                    "YZ" => 0,
                    _ => panic!("plane"),
                };
                (axis, expected["params"]["offset"].as_f64().unwrap())
            } else if plane.get("face").is_some() {
                (2, 10.0)
            } else {
                (2, plane["origin"][2].as_f64().unwrap())
            };
            assert!(
                (probe["bbox"]["min"][axis].as_f64().unwrap() - start).abs() < 0.001,
                "{before}: {probe}"
            );
            assert!(
                (probe["bbox"]["max"][axis].as_f64().unwrap() - start - 3.0).abs() < 0.001,
                "{before}: {probe}"
            );
        }
        if expected["command"] == "solid.extrude" {
            assert_eq!(
                bodies.len(),
                if expected["params"]["operation"] == "new" {
                    2
                } else {
                    1
                }
            );
        }
        // Compare the full measured result with the independently authored canonical call too.
        let mut canonical = setup(&expected);
        canonical.push(expected.clone());
        if expected["command"] != "sketch.create" {
            canonical.push(json!({"command":"document.inspect","params":{"measure":true}}));
            let canonical = native(&cli, &canonical, &file).unwrap();
            assert_eq!(
                doc["bodies"],
                canonical.last().unwrap()["bodies"],
                "{before}"
            );
        }
    }
    println!("CALLREPAIR CLI: {baseline_errors} original calls failed, 42 repaired calls built and measured");
    std::fs::remove_dir_all(&dir).unwrap();
}
#[test]
fn callrepair_caps_are_transactional() {
    let catalog = Catalog::bundled().unwrap();
    let mut huge = json!({"command":"solid.cut","params":{"target":"A","tools":vec!["B";dutchman_pilot::repair::MAX_NODES+1]}});
    let before = huge.clone();
    assert!(repair_command(&mut huge, &catalog).is_err());
    assert_eq!(huge, before);
    assert!(failed_sketch_context(&huge, &json!({})).is_none());
}
