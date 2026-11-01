//! Independently authored builds; only the public SolidCraft CLI executes geometry.
use super::*;
use std::process::Command;

fn request(prompt: &str) -> Value {
    json!({"messages":[{"role":"user","content":prompt}],"tools":[{"type":"function",
        "function":{"name":"execute","parameters":{"type":"object","properties":{
        "command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}}]})
}
fn result(req: &mut Value, args: Value, data: Value, id: usize) {
    let mut message = call("execute", args)["choices"][0]["message"].clone();
    message["tool_calls"][0]["id"] = json!(format!("size_development_{id}"));
    req["messages"].as_array_mut().unwrap().extend([message,
        json!({"role":"tool","tool_call_id":format!("size_development_{id}"),"content":data.to_string()})]);
}
fn fixture(default_mode: bool, replies: usize) -> Fixture {
    let mut f = Fixture::new(vec![reply("Finished."); replies]);
    f.proxy.cfg.planner = false;
    f.proxy.cfg.direct_recipes = default_mode;
    f
}
fn build(f: &Fixture, prompt: &str, command: &str, params: Value) -> Value {
    let cli = std::env::var_os("PILOT_SOLIDCRAFT_CLI").unwrap();
    let steps = json!([
        {"command":command,"params":params},
        {"command":"document.inspect","params":{"measure":true}}]);
    let file = f.dir.join("size-development.json");
    fs::write(&file, steps.to_string()).unwrap();
    let output = Command::new(cli)
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
    let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(results.len(), 2);
    let mut req = request(prompt);
    for (i, (args, data)) in steps.as_array().unwrap().iter().zip(results).enumerate() {
        result(&mut req, args.clone(), data, i);
    }
    req
}

#[test]
fn sizecheck_twenty_five_public_cli_mismatches() {
    if std::env::var_os("PILOT_SOLIDCRAFT_CLI").is_none() {
        return;
    }
    let mut cases = Vec::new();
    // Axis mix-ups, doubled extrusion lengths, wrong thickness, and unit mistakes.
    for i in 0..20 {
        let length = 113 + i * 7;
        let width = 19 + i;
        let height = 4 + i % 5;
        let actual = match i % 4 {
            0 => [length, width + height, height],
            1 => [length * 2, width, height],
            2 => [length, width, height * 2],
            _ => [length / 2, width, height],
        };
        cases.push((
            format!("Build a {length} x {width} x {height} mm bar"),
            "solid.box",
            json!({"length":actual[0],"width":actual[1],"height":actual[2]}),
        ));
    }
    cases.extend([
        (
            "Create a rod, 83 mm overall length".into(),
            "solid.cylinder",
            json!({"radius":5,"height":166}),
        ),
        (
            "Make a part with 21 mm overall diameter".into(),
            "solid.cylinder",
            json!({"radius":21,"height":71}),
        ),
        (
            "Build a 6 x 2 x 0.5 cm block".into(),
            "solid.box",
            json!({"length":60,"width":20,"height":10}),
        ),
        (
            "Make a 3 in by 1 in by 1/4 in bar".into(),
            "solid.box",
            json!({"length":152.4,"width":25.4,"height":6.35}),
        ),
        (
            "Create a part, 1 1/2 in overall length".into(),
            "solid.cylinder",
            json!({"radius":4,"height":76.2}),
        ),
    ]);
    assert_eq!(cases.len(), 25);
    for (i, (prompt, command, params)) in cases.into_iter().enumerate() {
        let mut f = fixture(i % 2 == 0, 2);
        let req = build(&f, &prompt, command, params);
        let reason = sizecheck::completion(&req).unwrap_or_else(|| panic!("case {i}: {prompt}"));
        assert!(reason.contains("measured body bounds") && reason.contains("stated overall size"));
        assert_eq!(f.chat(req).status, 200, "{prompt}");
        let scripted = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(scripted.requests.len(), 2, "{prompt}");
        assert!(scripted.requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["content"]
                .as_str()
                .is_some_and(|s| s.contains("Overall size check:"))));
        assert!(!fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .contains("size_check_read"));
    }
}

#[test]
fn sizecheck_fifteen_public_cli_silent_builds() {
    if std::env::var_os("PILOT_SOLIDCRAFT_CLI").is_none() {
        return;
    }
    let prompts = [
        "Build a 51 x 23 x 7 mm bar",
        "Create a 7 x 51 x 23 mm block",
        "I need a bar, 23 x 7 x 51 mm",
        "Build a 5.1 x 2.3 x 0.7 cm bar",
        "Make a bar, 51 mm overall length",
        "Make a bar with a 12 mm hole",
        "Build a bracket with a 15 x 9 x 2 mm boss",
        "Make a plate with a 13 mm slot",
        "Create two bars 91 x 24 x 5 mm",
        "Build a 91 x 24 x 5 mm bar and a block",
        "Create separate parts 91 x 24 x 5 mm",
        "Make a roughly 91 x 24 x 5 mm bar",
        "Build a bar 91 by 24 by 5",
        "Make a bar 91 or 51 mm overall length",
        "Build a box 91 x 24 x 5 mm inside",
    ];
    for (i, prompt) in prompts.into_iter().enumerate() {
        let mut f = fixture(i % 2 == 0, 1);
        let req = build(
            &f,
            prompt,
            "solid.box",
            json!({"length":51,"width":23,"height":7}),
        );
        assert!(sizecheck::completion(&req).is_none(), "{prompt}");
        assert_eq!(f.chat(req).status, 200, "{prompt}");
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
            1,
            "{prompt}"
        );
        assert!(!fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .contains("size_check"));
    }
}

fn synthetic() -> Value {
    let mut req = request("Build a 101 x 27 x 6 mm bar");
    result(
        &mut req,
        json!({"command":"solid.box","params":{"length":202,"width":27,"height":6}}),
        json!({"created_bodies":["Bar"],"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,27,6]}}]}),
        0,
    );
    req
}

#[test]
fn sizecheck_once_per_turn_and_next_turn_reset() {
    let mut f = fixture(false, 5);
    let mut req = synthetic();
    assert_eq!(f.chat(req.clone()).status, 200);
    result(
        &mut req,
        json!({"command":"document.inspect","params":{"measure":true}}),
        json!({"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,27,6]}}]}),
        1,
    );
    assert_eq!(f.chat(req.clone()).status, 200);
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        3
    );
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"Build a 103 x 27 x 6 mm bar"}));
    result(
        &mut req,
        json!({"command":"solid.box","params":{}}),
        json!({"changed_bodies":["Bar"],"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,27,6]}}]}),
        2,
    );
    assert_eq!(f.chat(req).status, 200);
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        5
    );
}

#[test]
fn sizecheck_one_read_failures_stale_partial_and_invalid_bounds() {
    let f = fixture(false, 1);
    let mut req = synthetic();
    // Remove bounds: the completion check can request one measurement, even if it fails.
    req["messages"][2]["content"] = json!("{\"created_bodies\":[\"Bar\"]}");
    let mut state = sizecheck::State::default();
    let mut audit = f.audit();
    assert!(
        sizecheck::prepare(&req, &mut state, &f.proxy.catalog, &mut audit)
            .unwrap()
            .is_some()
    );
    result(
        &mut req,
        json!({"command":"document.inspect","params":{"measure":true}}),
        json!({"ok":false,"error":"measurement failed"}),
        1,
    );
    assert!(
        sizecheck::prepare(&req, &mut state, &f.proxy.catalog, &mut audit)
            .unwrap()
            .is_none()
    );
    assert!(sizecheck::completion(&req).is_none());
    for data in [
        json!({"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,27,6]}}],"body_count":2}),
        json!({"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[-1,27,6]}}]}),
        json!({"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,null,6]}}]}),
        json!({"bodies":[{"name":"Bar","closed":false,"bbox":{"min":[0,0,0],"max":[202,27,6]}}]}),
    ] {
        let mut req = synthetic();
        req["messages"][2]["content"] = json!(data.to_string());
        assert!(sizecheck::completion(&req).is_none());
    }
    let mut req = synthetic();
    result(
        &mut req,
        json!({"command":"timeline.edit","params":{}}),
        json!({"ok":true}),
        1,
    );
    assert!(sizecheck::completion(&req).is_none());
}

#[test]
fn sizecheck_disabled_recipe_and_valid_calls_pass_through() {
    for mode in 0..3 {
        let mut f = fixture(false, 1);
        let mut req = synthetic();
        if mode == 0 {
            f.proxy.cfg.no_hints = true;
        }
        if mode == 1 {
            req["messages"][1]["tool_calls"][0]["id"] = json!("pilot_direct_0");
        }
        if mode == 2 {
            f.proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow_mut()
                .replies
                .clear();
            f.proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow_mut()
                .replies
                .push_back(Upstream {
                    status: 200,
                    body: call("execute", json!({"command":"document.inspect","params":{}}))
                        .to_string()
                        .into_bytes(),
                    content_type: "application/json".into(),
                    retry_after: None,
                });
        }
        assert_eq!(f.chat(req).status, 200);
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
            1
        );
        assert!(!fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .contains("size_check"));
    }
}

#[test]
fn sizecheck_final_read_uses_native_client_path_and_survives_continuation() {
    let mut f = fixture(false, 4);
    let mut req = synthetic();
    // A noun-only request avoids the separate-body check's action-intent trigger.
    req["messages"][0]["content"] = json!("a 101 x 27 x 6 mm bar");
    req["messages"][2]["content"] = json!("{\"created_bodies\":[\"Bar\"]}");
    let out = f.chat(req.clone());
    assert_eq!(out.status, 200);
    let response: Value = serde_json::from_slice(&out.body).unwrap();
    let m = &response["choices"][0]["message"];
    assert!(m["tool_calls"][0]["id"]
        .as_str()
        .unwrap()
        .starts_with("pilot_sizecheck_"));
    let args: Value = serde_json::from_str(
        m["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        args,
        json!({"command":"document.inspect","params":{"measure":true}})
    );
    req["messages"].as_array_mut().unwrap().extend([m.clone(), json!({"role":"tool",
        "tool_call_id":m["tool_calls"][0]["id"],"content":json!({"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":[202,27,6]}}]}).to_string()})]);
    assert_eq!(f.chat(req.clone()).status, 200);
    result(
        &mut req,
        json!({"command":"timeline.edit","params":{}}),
        json!({"ok":true}),
        9,
    );
    assert_eq!(f.chat(req).status, 200);
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        4
    );
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert_eq!(log.matches("\"kind\":\"size_check_read\"").count(), 1);
    assert_eq!(log.matches("\"kind\":\"size_check\"").count(), 1);
}

#[test]
fn sizecheck_all_axis_orders_tolerance_units_and_punctuation() {
    for max in [
        [101., 27., 6.],
        [101., 6., 27.],
        [27., 101., 6.],
        [27., 6., 101.],
        [6., 101., 27.],
        [6., 27., 101.],
        [101.05, 27., 6.],
    ] {
        let mut req = synthetic();
        req["messages"][2]["content"] = json!(json!({"created_bodies":["Bar"],"bodies":[{"name":"Bar","bbox":{"min":[0,0,0],"max":max}}]}).to_string());
        assert!(sizecheck::completion(&req).is_none());
    }
    for prompt in [
        "a 101 x 27 x 6 mm bar.",
        "Make a bar 101 mm by 27 mm by 6 mm.",
        "Make a 10.1 × 2.7 × 0.6 cm bar!",
        "Make a bar, 101 mm overall length.",
        "Make a bar with overall length 101 mm.",
        "Make a bar 101 x 27 x 6 mm with 4 mm holes.",
        "Make a 101 x 27 x 6 mm bar centered at the origin.",
        "Make a plate with overall dimensions 101 x 27 x 6 mm with 4 mm holes.",
    ] {
        let mut req = synthetic();
        req["messages"][0]["content"] = json!(prompt);
        assert!(sizecheck::eligible(&req), "{prompt}");
        assert!(sizecheck::completion(&req).is_some(), "{prompt}");
    }
    for prompt in [
        "Make a 1/2/3 x 2 x 3 in bar",
        "Make a -101 x 27 x 6 mm bar",
        "Make a 101 x 27 x 6 mm bar at position 0,0,0",
        "Make a plate with an overall length of 14 mm slot",
        "Build a block; create a bar 101 x 27 x 6 mm",
        "Make a bar, 101 mm overall length, and a disc",
        "Make an up to 101 x 27 x 6 mm bar",
        "Make a bar with 101 mm maximum overall length",
        "Make a plate with a 97 mm overall length slot",
        "Make a plate with overall length 97 mm slot",
        "Do not build a 101 x 27 x 6 mm bar",
        "Make a bar to fit within overall dimensions 101 x 27 x 6 mm",
        "Make a plate with 97 mm overall length of a slot",
        "Make a plate with hole overall dimensions 101 x 27 x 6 mm",
        "Make a 101 x 27 x 6 mm bar slot in a plate",
    ] {
        assert!(!sizecheck::eligible(&request(prompt)), "{prompt}");
    }
}

#[test]
fn sizecheck_recipe_suppression_is_limited_to_its_user_turn() {
    let mut req = synthetic();
    req["messages"][1]["tool_calls"][0]["id"] = json!("pilot_direct_0");
    assert!(!sizecheck::eligible(&req));
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"a 101 x 27 x 6 mm bar"}));
    assert!(sizecheck::eligible(&req));
}

#[test]
fn sizecheck_public_result_envelopes_and_streaming() {
    for (wrapper, stream) in [(0, false), (1, false), (2, true), (3, true)] {
        let mut f = fixture(false, 2);
        let mut req = synthetic();
        let d: Value =
            serde_json::from_str(req["messages"][2]["content"].as_str().unwrap()).unwrap();
        let d = match wrapper {
            0 => json!({"result":{"content":[{"type":"text","text":d.to_string()}]}}),
            1 => json!({"structuredContent":d}),
            2 => json!({"results":[{"result":d}]}),
            _ => json!({"after":d}),
        };
        req["messages"][2]["content"] = json!(d.to_string());
        req["stream"] = json!(stream);
        let out = f.chat(req);
        assert_eq!(out.status, 200);
        if stream {
            assert_eq!(out.content_type, "text/event-stream");
            assert_eq!(
                collect_sse(&out.body).unwrap()["choices"][0]["message"]["content"],
                "Finished."
            );
        }
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
            2
        );
    }
}

#[test]
fn sizecheck_does_not_duplicate_another_pilot_measurement() {
    let f = fixture(false, 0);
    let mut audit = f.audit();
    let mut req = synthetic();
    req["messages"][2]["content"] = json!("{\"created_bodies\":[\"Bar\"]}");
    result(
        &mut req,
        json!({"command":"document.inspect","params":{"measure":true}}),
        json!({"ok":false,"error":"failed"}),
        1,
    );
    req["messages"][3]["tool_calls"][0]["id"] = json!("pilot_bodyhint_3_1");
    let mut state = sizecheck::State::default();
    assert!(
        sizecheck::prepare(&req, &mut state, &f.proxy.catalog, &mut audit)
            .unwrap()
            .is_none()
    );
}
