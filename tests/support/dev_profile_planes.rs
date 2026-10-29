//! Literal public CLI references, independent of Pilot's constrained builders.
use super::*;
use dutchman_pilot::recipe_library::{forms, Library};

#[test]
fn dev_profile_planes_replay() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let mut cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_profile_planes.json")).unwrap();
    cases.extend(
        serde_json::from_str::<Vec<Value>>(include_str!("../fixtures/dev_profile_openings.json"))
            .unwrap(),
    );
    let lib = Library::embedded().unwrap();
    let cat = public_catalog::catalog().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let mut failures = Vec::new();
    for case in cases {
        let mut reference = Session::new(&path);
        for c in case["commands"].as_array().unwrap() {
            reference.execute(c["command"].as_str().unwrap(), c["params"].clone());
        }
        let expected = &case["expected"];
        check_shortcut_body(
            &mut reference,
            expected["volume"].as_f64().unwrap(),
            std::array::from_fn(|i| expected["bbox"]["min"][i].as_f64().unwrap()),
            std::array::from_fn(|i| expected["bbox"]["max"][i].as_f64().unwrap()),
            false,
        );
        let measured = reference.execute("component.properties", json!({"component":0}));
        let body = &measured["bodies"][0];
        assert!(body["center_of_mass"].is_array(), "{measured}");
        for exact in [false, true] {
            let request = json!({"messages":[{"role":"user","content":case["prompt"]}],"tools":[{"function":{"name":"execute"}}]});
            let plan = if exact {
                planner::plan_direct(&request, &cat).unwrap()
            } else {
                let mut args = case["args"].clone();
                forms::coerce(&mut args, &forms::flat(schema)).unwrap();
                forms::request_defaults(&mut args, schema, &request).unwrap();
                let slots = forms::to_slots(&args, schema).unwrap();
                Some(
                    lib.build(&json!({"recipe":"profile","args":slots}), &request, &cat)
                        .unwrap(),
                )
            };
            let Some(plan) = plan else {
                failures.push(format!("{} exact={exact}: no plan", case["prompt"]));
                continue;
            };
            let mut session = Session::new(&path);
            session.run(&plan);
            let state = session.execute("component.properties", json!({"component":0}));
            assert_eq!(
                state["bodies"].as_array().unwrap().len(),
                1,
                "{}",
                case["prompt"]
            );
            let actual = &state["bodies"][0];
            // Bounds and volume alone cannot distinguish opposite openings or
            // a misplaced offset stem. Compare the measured mass centre too.
            for field in ["bbox", "volume_mm3", "center_of_mass"] {
                let same = near(&actual[field], &body[field]);
                if !same {
                    failures.push(format!(
                        "{} exact={exact} {field}: {} != {}",
                        case["prompt"], actual[field], body[field]
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn near(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 0.001
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| near(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| near(a, b)))
        }
        _ => a == b,
    }
}
