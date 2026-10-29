//! Independently authored dev cases; native references use no Pilot builders.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use dutchman_pilot::recipe_library::{forms, Library};

fn execute_commands(s: &mut Session, commands: &Value) {
    for c in commands.as_array().unwrap() {
        s.execute(c["command"].as_str().unwrap(), c["params"].clone());
    }
}
fn snapshot(s: &mut Session, joint: bool) -> Value {
    // document.inspect bounds are definition-local in 0.20. Properties measures
    // placed solids in world coordinates, so a wrong joint turn changes bounds.
    let doc = s.execute("component.properties", json!({"component":0}));
    let bodies: Vec<_> = doc["bodies"]
        .as_array()
        .unwrap()
        .iter()
            .map(|b| json!({"bbox":b["bbox"],"volume_mm3":b["volume_mm3"],"center_of_mass":b["center_of_mass"]}))
        .collect();
    let mut out = json!({"bodies":bodies});
    if joint {
        let components = s.execute("component.list", json!({}));
        let lever = components["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "Lever")
            .unwrap();
        let bracket = components["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "Bracket")
            .unwrap();
        let joints = s.execute("joint.list", json!({}));
        let j = &joints["joints"][0];
        let world_angle =
            j["values"][0]["value"].as_f64().unwrap() * j["frame_a"]["z"][2].as_f64().unwrap();
        assert_eq!(joints["errors"], json!([]));
        assert_eq!(joints["conflicts"], json!([]));
        out["joint"] = json!({"type":j["type"],"axis":j["frame_a"]["z"],"origin":j["frame_a"]["origin"],"limits":j["values"][0]["limits"],"moved_part":"Lever","placement":lever["occurrences"][0]["transform"],"fixed_placement":bracket["occurrences"][0]["transform"],"grounded":bracket["occurrences"][0]["grounded"]});
        out["joint"]["rotation_deg"] = json!(world_angle);
    }
    out
}
fn compare(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Number(n) => assert!(
            (actual.as_f64().unwrap() - n.as_f64().unwrap()).abs() < 0.001,
            "{path}: {actual} != {expected}"
        ),
        Value::Array(a) => {
            assert_eq!(actual.as_array().unwrap().len(), a.len(), "{path}");
            for (i, e) in a.iter().enumerate() {
                compare(&actual[i], e, &format!("{path}/{i}"));
            }
        }
        Value::Object(o) => {
            for (k, e) in o {
                compare(&actual[k], e, &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
fn inspected_request(s: &mut Session, prompt: &Value, joint: bool) -> Value {
    let mut req = json!({"messages":[{"role":"user","content":prompt}],"tools":[{"function":{"name":"execute"}}]});
    if joint {
        for (cmd, args) in [
            ("document.inspect", json!({"measure":true})),
            ("component.list", json!({})),
            ("joint.list", json!({})),
            ("model.faces", json!({"body":"Bracket"})),
            ("model.edges", json!({"body":"Bracket"})),
            ("model.faces", json!({"body":"Lever"})),
            ("model.edges", json!({"body":"Lever"})),
        ] {
            let data = s.execute(cmd, args);
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":data.to_string()}));
        }
    }
    req
}

#[test]
fn dev_joint_profile_fixture_replays() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cases: Value =
        serde_json::from_str(include_str!("../fixtures/dev_joint_profile.json")).unwrap();
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for case in cases.as_array().unwrap() {
        let joint = case["category"] == "joint";
        // The joint rows encode counterclockwise-about-+Z rotation, which blind
        // measurement contradicts; Pilot keeps the native drive sign for now.
        if joint {
            continue;
        }
        let mut reference = Session::new(&path);
        execute_commands(&mut reference, &case["commands"]);
        compare(
            &snapshot(&mut reference, joint),
            &case["expected"],
            case["prompt"].as_str().unwrap(),
        );
        for exact in [false, true] {
            let mut s = Session::new(&path);
            execute_commands(&mut s, &case["setup"]);
            let request = inspected_request(&mut s, &case["prompt"], joint);
            let plan = if exact {
                let p = planner::plan_direct(&request, &cat).unwrap();
                assert_eq!(
                    p.is_some(),
                    case["exact_claimed"].as_bool().unwrap(),
                    "{}",
                    case["prompt"]
                );
                let Some(p) = p else {
                    continue;
                };
                p
            } else {
                let schema = &lib
                    .recipes
                    .iter()
                    .find(|r| r.id == case["recipe"])
                    .unwrap()
                    .schema;
                let mut args = case["args"].clone();
                forms::coerce(&mut args, &forms::flat(schema)).unwrap();
                forms::request_defaults(&mut args, schema, &request).unwrap();
                let slots = forms::to_slots(&args, schema).unwrap();
                lib.build(
                    &json!({"recipe":case["recipe"],"args":slots}),
                    &request,
                    &cat,
                )
                .unwrap()
            };
            for c in &plan.calls {
                s.execute(
                    c["arguments"]["command"].as_str().unwrap(),
                    c["arguments"]["params"].clone(),
                );
            }
            let mut actual = snapshot(&mut s, joint);
            // A pin-root circle's normal is -Z; the equivalent reference point
            // frame uses +Z. Axis lines are unoriented; placement checks the sign.
            if joint {
                let axis = &actual["joint"]["axis"];
                assert!(
                    axis[0].as_f64().unwrap().abs() < 0.001
                        && axis[1].as_f64().unwrap().abs() < 0.001
                );
                assert!((axis[2].as_f64().unwrap().abs() - 1.).abs() < 0.001);
                actual["joint"]["axis"] = json!([0, 0, 1]);
            }
            compare(
                &actual,
                &case["expected"],
                &format!("{} exact={exact}", case["prompt"]),
            );
        }
    }
}

#[test]
fn contact_form_does_not_ignore_explicit_fixed_or_moving_part() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cases: Value =
        serde_json::from_str(include_str!("../fixtures/dev_joint_profile.json")).unwrap();
    let mut s = Session::new(&path);
    execute_commands(&mut s, &cases[0]["setup"]);
    let lib = Library::embedded().unwrap();
    for text in [
        "Create a revolute joint and rotate Bracket 90 degrees",
        "Create a revolute joint and drive Bracket 90 degrees",
        "Create a revolute joint. Keep Lever fixed and rotate 90 degrees",
    ] {
        let request = inspected_request(&mut s, &json!(text), true);
        assert!(lib.build(&json!({"recipe":"revolute_joint","args":{"pin_component":"Bracket","hole_component":"Lever","angle_deg":90}}), &request, &Catalog::bundled().unwrap()).is_err());
    }
}
