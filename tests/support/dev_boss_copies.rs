//! Clean-room development replays against public MCP and literal CLI references.
use super::*;
use dutchman_pilot::recipe_library::Library;

fn commands(s: &mut Session, steps: &Value) {
    for c in steps.as_array().unwrap() {
        s.execute(c["command"].as_str().unwrap(), c["params"].clone());
    }
}
fn geometry(s: &mut Session) -> Value {
    let doc = s.execute("document.inspect", json!({"measure":true}));
    let mut bodies = Vec::new();
    for b in doc["bodies"].as_array().unwrap() {
        let faces = s.execute("model.faces", json!({"body":b["name"]}));
        let mut cylinders: Vec<_> = faces["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f.get("cylinder"))
            .map(|c| {
                json!([
                    c["origin"][0].as_f64().unwrap(),
                    c["origin"][1].as_f64().unwrap(),
                    c["radius"].as_f64().unwrap()
                ]
                .map(|x| (x * 1000.).round() as i64))
            })
            .collect();
        cylinders.sort_by_key(Value::to_string);
        cylinders.dedup();
        bodies.push(json!({"volume":b["volume_mm3"],"bbox":b["bbox"],"cylinders":cylinders}));
    }
    bodies.sort_by_key(Value::to_string);
    json!(bodies)
}
fn compare(a: &Value, b: &Value) {
    match b {
        Value::Number(n) => assert!(
            (a.as_f64().unwrap() - n.as_f64().unwrap()).abs() < 0.001,
            "{a} != {b}"
        ),
        Value::Array(v) => {
            assert_eq!(a.as_array().unwrap().len(), v.len(), "{a} != {b}");
            for (x, y) in a.as_array().unwrap().iter().zip(v) {
                compare(x, y);
            }
        }
        Value::Object(o) => {
            for (k, v) in o {
                compare(&a[k], v);
            }
        }
        _ => assert_eq!(a, b),
    }
}
#[test]
fn dev_boss_copies_replay() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/dev_boss_copies.json")).unwrap();
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    assert!((8..=12).contains(&fixture["cases"].as_array().unwrap().len()));
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["prompt"].as_str().unwrap();
        let band = lib.form_band(text).unwrap();
        eprintln!(
            "{} routing: {} {} score {}",
            case["id"], band["band"], band["recipes"][0]["id"], band["recipes"][0]["score"]
        );
        assert_eq!(band["recipes"][0]["id"], "linear_pattern", "{text}");
        assert_ne!(band["band"], "low", "{text}");
        let mut s = Session::new(&path);
        if case["setup"] == "plate" {
            let p = lib
                .build(
                    &json!({"recipe":"plate","args":fixture["plate_recipe"]}),
                    &json!({"messages":[]}),
                    &cat,
                )
                .unwrap();
            for c in p
                .calls
                .iter()
                .filter(|c| c["arguments"]["command"] != "document.inspect")
            {
                s.execute(
                    c["arguments"]["command"].as_str().unwrap(),
                    c["arguments"]["params"].clone(),
                );
            }
        } else {
            commands(&mut s, &fixture["setups"][case["setup"].as_str().unwrap()]);
        }
        let mut req = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        let doc = s.execute("document.inspect", json!({"measure":true}));
        eprintln!("{} setup: {}", case["id"], doc);
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":doc.to_string()}));
        for f in doc["timeline"].as_array().unwrap() {
            let d = s.execute("timeline.inspect", json!({"feature":f["name"]}));
            eprintln!("{} feature: {}", case["id"], d);
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":d.to_string()}));
        }
        let mut slots = case["args"].clone();
        let mut complete = false;
        for _ in 0..6 {
            let plan = lib.build(&json!({"recipe":"linear_pattern","args":slots}), &req, &cat);
            let p = match plan {
                Ok(p) => p,
                Err(e) => {
                    if case["error"].as_str().is_some_and(|s| e.contains(s)) {
                        complete = true;
                    } else {
                        failures.push(format!("{}: {e}", case["id"]));
                    }
                    break;
                }
            };
            slots = p.slots.clone();
            eprintln!("{} slots: {slots}", case["id"]);
            for c in &p.calls {
                let result = s.execute(
                    c["arguments"]["command"].as_str().unwrap(),
                    c["arguments"]["params"].clone(),
                );
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","content":result.to_string()}));
            }
            if let Some(expected) = &p.expected {
                assert_eq!(planner::completion(&req, expected), None, "{}", case["id"]);
                complete = true;
                break;
            }
        }
        if !complete {
            failures.push(format!("{}: incomplete", case["id"]));
            continue;
        }
        if case.get("error").is_some() {
            continue;
        }
        let mut reference = Session::new(&path);
        commands(
            &mut reference,
            &fixture["setups"][case["setup"].as_str().unwrap()],
        );
        reference.execute(
            case["reference"]["command"].as_str().unwrap(),
            case["reference"]["params"].clone(),
        );
        compare(&geometry(&mut s), &geometry(&mut reference));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
