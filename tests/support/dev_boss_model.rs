//! Full HTTP proxy replay with deterministic extraction and the public CLI.
use super::*;
use dutchman_pilot::{catalog::Catalog, recipe_library::Library};
use std::process::Command;

fn native(path: &std::ffi::OsStr, steps: &[Value], file: &std::path::Path) -> Vec<Value> {
    fs::write(file, serde_json::to_vec(steps).unwrap()).unwrap();
    let output = Command::new(path)
        .arg("run")
        .arg(file)
        .args(["--results", "--quiet"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn shape(path: &std::ffi::OsStr, steps: &[Value], file: &std::path::Path) -> Value {
    let mut steps = steps.to_vec();
    steps.push(json!({"command":"document.inspect","params":{"measure":true}}));
    let doc = native(path, &steps, file).pop().unwrap();
    let inspected = doc["bodies"].as_array().unwrap();
    let offset = steps.len();
    for b in inspected {
        steps.push(json!({"command":"model.faces","params":{"body":b["name"]}}));
    }
    let results = native(path, &steps, file);
    let mut bodies = Vec::new();
    for (b, faces) in inspected.iter().zip(&results[offset..]) {
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
fn boss_copies_end_to_end_model_proxy() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/dev_boss_copies.json")).unwrap();
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        eprintln!("boss-copy HTTP replay {}", case["id"]);
        let missing = case.get("error").is_some();
        let band = lib.form_band(case["prompt"].as_str().unwrap()).unwrap();
        let mut replies = Vec::new();
        if band["band"] == "route" {
            replies.push(ok(call("pilot_route", json!({"recipe":"linear_pattern"}))));
        }
        for _ in 0..if missing { 3 } else { 1 } {
            replies.push(ok(call("pilot_fill", case["args"].clone())));
        }
        if missing {
            replies.push(ok(prose(
                "What centre spacing in millimetres should I use?",
            )));
        }
        let h = Harness::with_how(
            replies,
            12,
            FallbackPrompt::Off,
            None,
            json!({"capabilities":["tools"]}),
            true,
        );
        let file = h.dir.join("boss-script.json");
        let mut steps = if case["setup"] == "plate" {
            lib.build(
                &json!({"recipe":"plate","args":fixture["plate_recipe"]}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap()
            .calls
            .into_iter()
            .filter(|c| c["arguments"]["command"] != "document.inspect")
            .map(|c| c["arguments"].clone())
            .collect::<Vec<_>>()
        } else {
            fixture["setups"][case["setup"].as_str().unwrap()]
                .as_array()
                .unwrap()
                .clone()
        };
        let initial = steps.len();
        let mut req = request(case["prompt"].as_str().unwrap());
        // Keep inspection reads as individual public execute calls.
        req["tools"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["function"]["name"] != "batch");
        let mut rounds = 0;
        let mut done = false;
        let mut patterns = 0;
        for _ in 0..10 {
            let (status, _, body) = h.chat(&req);
            rounds += 1;
            assert_eq!(status, 200, "{}: {body}", case["id"]);
            let response: Value = serde_json::from_str(&body).unwrap();
            let message = &response["choices"][0]["message"];
            let Some(calls) = message["tool_calls"].as_array() else {
                assert!(
                    message["content"].as_str().unwrap().contains(if missing {
                        "spacing"
                    } else {
                        // The scripted model fills the form, so the build is not an exact match.
                        "Built from the model's form:"
                    }),
                    "{}: {message}",
                    case["id"]
                );
                done = true;
                break;
            };
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(message.clone());
            for c in calls {
                let a: Value =
                    serde_json::from_str(c["function"]["arguments"].as_str().unwrap()).unwrap();
                assert_eq!(c["function"]["name"], "execute", "{c}");
                if a["command"] == "solid.pattern.rectangular" {
                    patterns += 1;
                    assert!(!missing);
                    assert_eq!(
                        a["params"].as_object().unwrap().len(),
                        case["reference"]["params"].as_object().unwrap().len()
                    );
                    compare(&a["params"], &case["reference"]["params"]);
                } else {
                    assert!(
                        matches!(
                            a["command"].as_str(),
                            Some("document.inspect" | "timeline.inspect" | "parameters.list")
                        ),
                        "unexpected {a}"
                    );
                }
                steps.push(a);
                let result = native(&path, &steps, &file).pop().unwrap();
                req["messages"].as_array_mut().unwrap().push(
                    json!({"role":"tool","tool_call_id":c["id"],"content":result.to_string()}),
                );
            }
        }
        assert!(done, "{} exceeded ten proxy rounds", case["id"]);
        assert_eq!(patterns, usize::from(!missing));
        if !missing {
            let mut reference = fixture["setups"][case["setup"].as_str().unwrap()]
                .as_array()
                .unwrap()
                .clone();
            reference.push(case["reference"].clone());
            compare(
                &shape(&path, &steps, &file),
                &shape(&path, &reference, &file),
            );
        } else {
            assert!(steps[initial..]
                .iter()
                .all(|c| c["command"].as_str().unwrap().ends_with("inspect")));
        }
        let audit = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
        assert!(audit.contains("fill_offered"), "{}: {audit}", case["id"]);
        for wire in h
            .chats()
            .iter()
            .filter(|wire| wire["tools"][0]["function"]["name"] == "pilot_fill")
        {
            let user = wire["messages"][1]["content"].as_str().unwrap();
            let (_, context) = user.split_once("\nDesign context: ").unwrap();
            let context: Value = serde_json::from_str(context).unwrap();
            assert!(context["bodies"].as_array().is_some_and(|b| !b.is_empty()));
            assert_eq!(context["last_feature"]["type"], "CylinderFeature");
            assert!(context["last_feature"]["definition"].is_object());
        }
        if !missing {
            assert!(audit.contains("fill_accepted"), "{}: {audit}", case["id"]);
        }
        // Finish the fixed-length HTTP harness after the variable inspection chain.
        for _ in rounds..12 {
            assert!(matches!(
                ureq::get(&format!("{}/boss-replay-done", h.url))
                    .timeout(Duration::from_secs(8))
                    .call(),
                Err(ureq::Error::Status(404, _))
            ));
        }
        fs::remove_file(file).unwrap();
        h.finish();
    }
}
