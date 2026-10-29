//! Independent public-command references for enclosure and grid form semantics.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::Catalog,
    recipe_library::{forms, Library},
};
use serde_json::{json, Value};
use std::process::Command;

fn measure(path: &std::ffi::OsStr, commands: &Value, id: &str) -> Value {
    let file =
        std::env::temp_dir().join(format!("pilot-enc-grid-{}-{id}.json", std::process::id()));
    std::fs::write(&file, serde_json::to_vec(commands).unwrap()).unwrap();
    let output = Command::new(path).arg("eval").arg(&file).output().unwrap();
    std::fs::remove_file(file).unwrap();
    assert!(
        output.status.success(),
        "{id}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn development_enclosure_grid_forms_match_literal_references() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/dev_enclosure_grid.json")).unwrap();
    let lib = Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let mut differences = Vec::new();
    // Replay both careful-reader slots and optional-field/default probes.
    let variants = fixtures.into_iter().flat_map(|f| {
        let mut literal = f.clone();
        literal["id"] = json!(format!("{}-literal", f["id"].as_str().unwrap()));
        literal["args"] = f["literal_args"].clone();
        [literal, f]
    });
    for f in variants {
        let id = f["id"].as_str().unwrap();
        let reference = measure(&path, &f["commands"], &format!("{id}-reference"));
        let schema = &lib
            .recipes
            .iter()
            .find(|r| r.id == f["recipe"])
            .unwrap()
            .schema;
        let mut args = f["args"].clone();
        let request = json!({"messages":[{"role":"user","content":f["prompt"]}]});
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        forms::request_defaults(&mut args, schema, &request).unwrap();
        let plan = match lib.build(
            &json!({"recipe":f["recipe"],"args":args}),
            &request,
            &catalog,
        ) {
            Ok(p) => p,
            Err(e) => {
                differences.push(format!("{id}: {e}"));
                continue;
            }
        };
        let commands: Vec<_> = plan
            .calls
            .iter()
            .filter(|c| c["arguments"]["command"] != "document.inspect")
            .map(|c| c["arguments"].clone())
            .collect();
        // A plain shell can open at either end with identical bounds and volume.
        if f["recipe"] == "enclosure" {
            let shell = commands
                .iter()
                .find(|c| c["command"] == "solid.shell")
                .unwrap();
            let box_params = &f["commands"][0]["params"];
            let z = box_params["corner"][2].as_f64().unwrap()
                + if f["literal_args"]["open"] == "bottom" {
                    0.
                } else {
                    box_params["height"].as_f64().unwrap()
                };
            if (shell["params"]["faces"][0][2].as_f64().unwrap() - z).abs() > 0.001 {
                differences.push(format!("{id}: shell removes the wrong end face"));
            }
        }
        // Equal volume and bounds cannot distinguish swapped grid axes.
        let holes = |steps: &[Value]| {
            let mut positions: Vec<_> = steps
                .iter()
                .filter(|c| c["command"] == "solid.hole")
                .map(|c| c["params"]["position"].clone())
                .collect();
            positions.sort_by_key(Value::to_string);
            positions
        };
        let reference_holes = holes(f["commands"].as_array().unwrap());
        let actual_holes = holes(&commands);
        if actual_holes.len() != reference_holes.len()
            || actual_holes.iter().zip(&reference_holes).any(|(a, b)| {
                (0..3).any(|axis| {
                    (a[axis].as_f64().unwrap() - b[axis].as_f64().unwrap()).abs() > 0.001
                })
            })
        {
            differences.push(format!(
                "{id}: hole centres {actual_holes:?} != {reference_holes:?}"
            ));
        }
        let actual = measure(&path, &json!(commands), id);
        for (label, state) in [("reference", reference), ("form", actual)] {
            let bodies = state["bodies"]
                .as_array()
                .unwrap_or_else(|| panic!("{id}: {state}"));
            assert_eq!(bodies.len(), 1, "{id} {label}: {state}");
            let body = &bodies[0];
            let volume = body["volume_mm3"].as_f64().unwrap();
            if (volume - f["expected"]["volume"].as_f64().unwrap()).abs() > 0.01 {
                differences.push(format!(
                    "{id} {label}: volume {volume} != {}",
                    f["expected"]["volume"]
                ));
            }
            for end in ["min", "max"] {
                for axis in 0..3 {
                    if (body["bbox"][end][axis].as_f64().unwrap()
                        - f["expected"]["bbox"][end][axis].as_f64().unwrap())
                    .abs()
                        > 0.001
                    {
                        differences.push(format!(
                            "{id} {label}: bbox {} != {}",
                            body["bbox"], f["expected"]["bbox"]
                        ));
                        break;
                    }
                }
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn opening_and_plane_defaults_preserve_explicit_choices() {
    let lib = Library::embedded().unwrap();
    for (id, text, supplied, expected) in [
        ("enclosure", "a cover", json!({}), json!({"open":"bottom"})),
        (
            "enclosure",
            "a lid open top",
            json!({}),
            json!({"open":"top"}),
        ),
        (
            "enclosure",
            "an open-top enclosure",
            json!({}),
            json!({"open":"top"}),
        ),
        (
            "enclosure",
            "a cover open top",
            json!({"open":"bottom"}),
            json!({"open":"bottom"}),
        ),
        ("enclosure", "enclosure", json!({}), json!({})),
        (
            "enclosure",
            "enclosure with inner lip for a lid",
            json!({}),
            json!({}),
        ),
        ("enclosure", "box without a cover", json!({}), json!({})),
        (
            "enclosure",
            "housing with outer lip for a lid",
            json!({}),
            json!({}),
        ),
        ("enclosure", "without a lid", json!({}), json!({})),
        (
            "enclosure",
            "open top and open bottom",
            json!({}),
            json!({}),
        ),
        (
            "plate",
            "plate on front plane",
            json!({}),
            json!({"plane":"XZ"}),
        ),
        (
            "plate",
            "front-plane plate",
            json!({}),
            json!({"plane":"XZ"}),
        ),
        ("plate", "holes on the front face", json!({}), json!({})),
        ("plate", "front edge holes", json!({}), json!({})),
        (
            "plate",
            "plate on front plane",
            json!({"plane":"XY"}),
            json!({"plane":"XY"}),
        ),
    ] {
        let schema = &lib.recipes.iter().find(|r| r.id == id).unwrap().schema;
        let mut args = supplied.clone();
        let request = json!({"messages":[{"role":"user","content":text}]});
        let changes = forms::request_defaults(&mut args, schema, &request).unwrap();
        assert_eq!(args, expected, "{id}: {text}");
        assert_eq!(changes.len(), usize::from(supplied != expected));
        assert!(changes.iter().all(|c| c["defaulted"] == true));
        assert!(forms::request_defaults(&mut args, schema, &request)
            .unwrap()
            .is_empty());
    }
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    for (alias, plane) in [("front plane", "XZ"), ("top", "XY"), ("right", "YZ")] {
        let mut args = json!({"plane":alias});
        let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        assert_eq!(args["plane"], plane);
        assert_eq!(changes.len(), 1);
    }
}
