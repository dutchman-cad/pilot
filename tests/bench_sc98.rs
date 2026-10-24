#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "../bench/sc98/check.rs"]
mod check;
use serde_json::{json, Value};
fn fixture(n: usize) -> (Value, Value) {
    let tasks: Value = serde_json::from_str(include_str!("../bench/sets/sc98/tasks.json")).unwrap();
    let task = &tasks["tasks"][n - 1];
    let c = &task["check"];
    let size = c["size"].as_array().unwrap();
    let x = &c["origin"][0];
    let y = &c["origin"][1];
    let rectangle = c["shape"] == "rectangle";
    let sketch = if rectangle {
        json!({"Width (mm)":size[0],"Height (mm)":size[1],"Corner X (mm)":x,"Corner Y (mm)":y,"Corner radius (mm)":0,"On feature # (0 = ground)":0,"Height above that (mm)":0})
    } else {
        json!({"Radius (mm)":size[0].as_f64().unwrap()/2.0,"Centre X (mm)":x,"Centre Y (mm)":y,"On feature # (0 = ground)":0,"Height above that (mm)":0})
    };
    let mut state = json!({"screen":"","fields":{"Sketch1":sketch,"Extrude1":{"Depth (mm)":size.last().unwrap()}}});
    let mut screen = format!("SolidCraft 98, 800 x 600.\n1 | Sketch1 | Sketch ({}) | dimensions | OK\n2 | Extrude1 | Extrude | Sketch1 | OK   <- selected\n",if rectangle {"Rectangle"} else {"Circle"});
    for (i, h) in c["holes"].as_array().unwrap().iter().enumerate() {
        let name = format!("Hole{}", i + 1);
        screen.push_str(&format!(
            "{} | {name} | Hole | on Extrude1 top face | OK\n",
            i + 3
        ));
        state["fields"][name] = json!({"Diameter (mm)":h[0],"Depth (mm, 0 = through all)":h[1],"Centre X (mm)":h[2],"Centre Y (mm)":h[3]});
    }
    state["screen"] = json!(screen);
    (task.clone(), state)
}
fn passes(task: &Value, state: &Value, answer: &str) -> bool {
    check::score(
        &task["check"],
        &json!({"done_message":answer}),
        state,
        &Value::Null,
    )
    .unwrap()["pass"]
        == true
}
#[test]
fn all_reference_predicates_and_empty_states() {
    for n in 1..=10 {
        let (task, state) = fixture(n);
        let answer = task["reference_reply"].as_str().unwrap();
        assert!(passes(&task, &state, answer), "task {n}");
        assert!(!passes(&task, &json!({"screen":"","fields":{}}), answer));
    }
}
#[test]
fn dimensions_plane_bodies_and_hole_positions() {
    let (task, state) = fixture(1);
    for (field, value) in [
        ("Width (mm)", 21),
        ("Corner X (mm)", 1),
        ("Corner radius (mm)", 1),
        ("On feature # (0 = ground)", 1),
    ] {
        let mut bad = state.clone();
        bad["fields"]["Sketch1"][field] = json!(value);
        assert!(!passes(&task, &bad, "Done"));
    }
    let (task, mut state) = fixture(10);
    state["fields"]["Hole2"]["Centre X (mm)"] = json!(15);
    assert!(!passes(&task, &state, "Done"));
    let (task, mut state) = fixture(5);
    state["fields"]["Hole1"]["Depth (mm, 0 = through all)"] = json!(5);
    assert!(!passes(&task, &state, "Done"));
    let (task, mut state) = fixture(1);
    state["screen"] = json!(format!(
        "{}\n3 | Extrude2 | Extrude | Sketch1 | OK",
        state["screen"].as_str().unwrap()
    ));
    assert!(!passes(&task, &state, "Done"));
}
#[test]
fn unsupported_claims_and_calculated_volume() {
    for n in [6, 8, 9] {
        let (task, state) = fixture(n);
        assert!(!passes(&task, &state, "Done, succeeded."));
    }
    let (task, state) = fixture(9);
    for answer in [
        "Calculated 600 mm3.",
        "Measured 6000 mm3.",
        "Calculated 600 mm3, not 6000 mm3.",
        "Calculated 6000 mm3 or 5000 mm3.",
    ] {
        assert!(!passes(&task, &state, answer));
    }
    assert!(passes(&task, &state, "Calculated volume is 6 cm3."));
    let (task, mut state) = fixture(6);
    state["fields"]["Sketch1"]["Corner radius (mm)"] = json!(3);
    assert!(passes(
        &task,
        &state,
        "Cannot round solid edges; instead made 3 mm sketch corners."
    ));
    assert!(!passes(&task, &state, "Cannot round edges."));
}
#[test]
fn arbitrary_change_ignores_view_selection_names_and_clock() {
    let (_, before) = fixture(9);
    let mut after = before.clone();
    after["screen"] = json!(
        after["screen"].as_str().unwrap().replace("<- selected", "")
            + "\nView yaw 90\nClock: later"
    );
    after["screen"] = json!(after["screen"]
        .as_str()
        .unwrap()
        .replace("Sketch1", "Other"));
    let sketch = after["fields"]
        .as_object_mut()
        .unwrap()
        .remove("Sketch1")
        .unwrap();
    after["fields"]["Other"] = sketch;
    let c = json!({"changed_geometry":true});
    assert_eq!(
        check::score(&c, &json!({}), &after, &before).unwrap()["pass"],
        false
    );
    after["fields"]["Other"]["Width (mm)"] = json!(31);
    assert_eq!(
        check::score(&c, &json!({}), &after, &before).unwrap()["pass"],
        true
    );
}
#[test]
fn fields_status_dialogs_and_caps() {
    let f = check::fields("  field \"Width (mm)\": [20] (has the focus)\nfield \"bad\": [NaN]");
    assert_eq!(f, json!({"Width (mm)":20.0}));
    let (task, mut state) = fixture(1);
    state["screen"] =
        json!(state["screen"].as_str().unwrap().to_owned() + "\nDialog \"Edit Sketch1\"");
    assert!(!passes(&task, &state, "Done"));
    assert!(check::rows(&"x".repeat(262145)).is_err());
    assert!(check::rows(&"1 | X | Hole | D 10 | OK\n".repeat(65)).is_err());
}
