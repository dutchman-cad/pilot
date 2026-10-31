//! Development coverage for exact hole-pattern plates and form equivalence.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::{Catalog, MAX_CATALOG_BYTES},
    planner,
    recipe_library::{forms, Library},
};
use serde_json::{json, Value};
#[path = "support/dev_exactgrid.rs"]
mod fixtures;
#[path = "support/public_catalog.rs"]
mod public_catalog;

fn request(text: &str) -> Value {
    json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
}

#[test]
fn development_exact_patterns_match_form_calls() {
    let cat = public_catalog::catalog().unwrap();
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let mut claimed = [0; 4];
    for (n, (text, mut args)) in fixtures::cases().into_iter().enumerate() {
        let req = request(&text);
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        let form = lib
            .build(&json!({"recipe":"plate","args":args}), &req, &cat)
            .unwrap();
        match planner::plan_direct(&req, &cat).unwrap() {
            Some(exact) => {
                claimed[(n / 12).min(3)] += 1;
                assert_eq!(exact.calls, form.calls, "case {n}: {text}");
            }
            None => panic!("case {n} declined: {text}"),
        }
    }
    eprintln!("claims by group (numbers first, cm/chamfer, inch circle, legacy): {claimed:?}");
    for text in fixtures::near_misses() {
        assert!(
            planner::plan_direct(&request(&text), &cat)
                .unwrap()
                .is_none(),
            "near miss: {text}"
        );
    }
}

#[test]
fn explicit_units_planes_and_placement_match_form_calls() {
    let cat = public_catalog::catalog().unwrap();
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let base = json!({"length":180,"width":120,"thickness":10,"grid_columns":4,"grid_rows":3,"grid_pitch_x":30,"grid_pitch_y":30,"grid_hole_diameter":8});
    for (modifier, plane, centred) in [
        ("on front plane", "XZ", false),
        ("in XZ plane", "XZ", false),
        ("on YZ plane", "YZ", false),
        ("front face on XY plane", "XY", false),
        ("centred at origin", "XY", true),
        ("centered on origin in XZ plane", "XZ", true),
        ("one corner at origin", "XY", false),
    ] {
        let text = format!(
            "plate 18 cm x 120 mm x 10 mm, {modifier}, 4 x 3 grid of 8 mm holes 3 cm apart"
        );
        let mut args = base.clone();
        args["plane"] = json!(plane);
        args["centred"] = json!(centred);
        let req = request(&text);
        let exact = planner::plan_direct(&req, &cat).unwrap().unwrap();
        let form = lib
            .build(&json!({"recipe":"plate","args":args}), &req, &cat)
            .unwrap();
        assert_eq!(exact.calls, form.calls, "{text}");
    }
    let text = "plate 8in x 5in x 1/4in, 4 x 3 grid of 1/4in holes 1in apart";
    let mut args = json!({"length":"8 in","width":"5 in","thickness":"1/4 in","grid_columns":4,"grid_rows":3,"grid_pitch_x":"1 in","grid_pitch_y":"1 in","grid_hole_diameter":"1/4 in"});
    forms::coerce(&mut args, &forms::flat(schema)).unwrap();
    let req = request(text);
    assert_eq!(
        planner::plan_direct(&req, &cat).unwrap().unwrap().calls,
        lib.build(&json!({"recipe":"plate","args":args}), &req, &cat)
            .unwrap()
            .calls
    );
    for tail in [
        "centred at origin and corner at origin",
        "on XY plane and XZ plane",
        "1 mm chamfer edges and 2 mm chamfer edges",
    ] {
        let text = format!("plate 180 x 120 x 10, {tail}, 4 x 3 grid of 8 mm holes 30 mm apart");
        assert!(
            planner::plan_direct(&request(&text), &cat)
                .unwrap()
                .is_none(),
            "{text}"
        );
    }
    for text in [
        "plate 180 x 120 x 10, 9 x 8 grid of 8 mm holes 30 mm apart",
        "plate 180 x 120 x 10, 4 x 3 grid of 8 mm holes 2 mm apart",
        "plate 180 x 120 x 10, 4 x 3 grid of 8 mm holes 100 mm apart",
        "plate 1000001 x 120 x 10, 4 x 3 grid of 8 mm holes 30 mm apart",
        "plate 18 cm x 120 x 10 mm, 4 x 3 grid of 8 mm holes 30 mm apart",
        "18 x 12 cm plate 8 thick, 4 x 3 grid of 8 mm holes 30 mm apart",
        "4in square plate 1/4 thick, 5 holes 1/4in on a 3in bolt circle",
        "plate 180 x 120 x 10, 0 x 3 grid of 8 mm holes 30 mm apart",
        "plate 180 x 120 x 10, 4 x 3 grid of 8 mm holes -30 mm apart",
        "plate 180 x 120 x 10, 4 x 3 grid of 8 mm holes",
        "4in square plate 1/4in thick, 65 holes 1/4in on a 3in bolt circle",
    ] {
        assert!(
            planner::plan_direct(&request(text), &cat)
                .unwrap()
                .is_none(),
            "{text}"
        );
    }
}

#[test]
fn native_cli_patterns_have_requested_bounds_and_volume() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = public_catalog::catalog().unwrap();
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let cases = fixtures::cases();
    for n in [0, 1, 12, 13, 24, 29] {
        let (text, args) = &cases[n];
        let mut args = args.clone();
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        let plan = planner::plan_direct(&request(text), &cat).unwrap().unwrap();
        let mut commands: Vec<_> = plan.calls.iter().map(|c| c["arguments"].clone()).collect();
        commands.retain(|c| c["command"] != "document.inspect");
        commands.push(json!({"command":"document.inspect","params":{"measure":true}}));
        assert!(commands.len() <= 64);
        let file =
            std::env::temp_dir().join(format!("pilot-exactgrid-{}-{n}.json", std::process::id()));
        std::fs::write(&file, serde_json::to_vec(&commands).unwrap()).unwrap();
        let output = std::process::Command::new(&cli)
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
        assert!(output.stdout.len() <= 4 * 1024 * 1024);
        let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(results.len(), commands.len());
        let state = results.last().unwrap();
        assert!(
            state["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["error"].is_null()),
            "{text}: {state}"
        );
        let bodies = state["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), 1, "{text}: {state}");
        let body = &bodies[0];
        let size = [
            args["length"].as_f64().unwrap(),
            args["width"].as_f64().unwrap(),
            args["thickness"].as_f64().unwrap(),
        ];
        for (axis, extent) in size.iter().enumerate() {
            assert!(
                body["bbox"]["min"][axis].as_f64().unwrap().abs() < 0.001,
                "{text}: {body}"
            );
            assert!(
                (body["bbox"]["max"][axis].as_f64().unwrap() - extent).abs() < 0.001,
                "{text}: {body}"
            );
        }
        let count = args["bolt_hole_count"].as_f64().unwrap_or_else(|| {
            args["grid_columns"].as_f64().unwrap() * args["grid_rows"].as_f64().unwrap()
        });
        let diameter = args["bolt_hole_diameter"]
            .as_f64()
            .unwrap_or_else(|| args["grid_hole_diameter"].as_f64().unwrap());
        let chamfer = args["corner_size"].as_f64().unwrap_or(0.);
        let volume = size[2]
            * (size[0] * size[1]
                - count * std::f64::consts::PI * (diameter / 2.).powi(2)
                - 2. * chamfer.powi(2));
        assert!(
            (body["volume_mm3"].as_f64().unwrap() - volume).abs() < (volume * 0.001).max(0.01),
            "{text}: expected {volume}: {body}"
        );
    }
}
