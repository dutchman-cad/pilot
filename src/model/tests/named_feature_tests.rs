//! Development examples authored for the public CLI; no benchmark tasks.
use super::context_tests::{drive, Session};
use super::*;

fn setup(s: &mut Session, c: &Value) -> String {
    s.execute(
        "solid.box",
        json!({"length":74,"width":56,"height":32,"body_name":"Carrier"}),
    );
    if c["multi"] == true {
        s.execute(
            "solid.box",
            json!({"length":19,"width":23,"height":17,"corner":[100,0,0],"body_name":"Bracket"}),
        );
    }
    let kind = c["kind"].as_str().unwrap();
    let name = if c["named"] == true {
        Some(match kind {
            "hole" => "Vent",
            "pad" => "Support",
            "extrude" | "offset" => "Stiffener",
            _ => "Well",
        })
    } else {
        None
    };
    let result = if matches!(kind, "hole" | "counter") {
        let mut p = json!({"position":[22,21,32],"diameter":8,"depth":12,"targets":["Carrier"]});
        if kind == "counter" {
            p["type"] = json!("counterbore");
            p["cb_depth"] = json!(5);
            p["cb_diameter"] = json!(13);
        }
        s.execute("solid.hole", p)
    } else if kind == "cylinder" {
        let mut p = json!({"base":[22,21,32],"radius":5,"height":8,"operation":"join","targets":["Carrier"]});
        if let Some(n) = name {
            p["name"] = json!(n);
            p.as_object_mut().unwrap().remove("radius");
            p["diameter"] = json!(10);
        }
        s.execute("solid.cylinder", p)
    } else if kind == "pad" {
        let mut p = json!({"length":16,"width":14,"height":8,"corner":[15,12,32],"operation":"join","targets":["Carrier"]});
        if let Some(n) = name {
            p["name"] = json!(n);
        }
        s.execute("solid.box", p)
    } else {
        let cut = matches!(kind, "pocket" | "under");
        s.execute(
            "sketch.create",
            json!({"plane":"XY","offset":if kind=="under"{0}else{32}}),
        );
        s.execute(
            "sketch.rectangle.two_point",
            json!({"p0":[15,12],"p1":[31,29]}),
        );
        s.execute("sketch.finish", json!({}));
        let mut p = json!({"distance":if cut{12}else{8},"direction":if kind=="pocket"{"negative"}else{"positive"},"operation":if cut{"cut"}else{"join"},"targets":["Carrier"]});
        if kind == "offset" {
            p["start_offset"] = json!(-2);
        }
        if let Some(n) = name {
            p["name"] = json!(n);
        }
        s.execute("solid.extrude", p)
    };
    let mut actual = result["name"].as_str().unwrap().to_string();
    if matches!(kind, "hole" | "counter") {
        if let Some(n) = name {
            s.execute("timeline.rename", json!({"feature":actual,"name":n}));
            actual = n.into();
        }
    }
    actual
}
fn model_form(wire: &Value) -> Value {
    let Some(user) = wire["messages"][1]["content"].as_str() else {
        return reply("unexpected ordinary wire");
    };
    let text = user.split("\nDesign context:").next().unwrap();
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_edits.json"
    ))
    .unwrap();
    let c = cases.iter().find(|c| c["prompt"] == text).unwrap();
    let names = wire["tools"][0]["function"]["parameters"]["properties"]["feature"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(names.len(), 1, "{wire}");
    assert!(
        wire["tools"][0]["function"]["parameters"]["properties"]["feature"]["description"]
            .as_str()
            .unwrap()
            .contains("Carrier")
    );
    call(
        "pilot_fill",
        json!({"feature":names[0],"parameter":c["parameter"],"mode":c["mode"],"value":c["value"]}),
    )
}
#[test]
fn named_feature_development_cli_replays_exact_and_forms() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_edits.json"
    ))
    .unwrap();
    assert!(cases.len() >= 40);
    for c in &cases {
        for exact in [true, false] {
            eprintln!("named edit exact={exact}: {}", c["prompt"]);
            let Some(mut s) = Session::new() else { return };
            let feature = setup(&mut s, c);
            let mut f = fill_fixture(vec![]);
            f.proxy.catalog = context_tests::public_catalog::catalog().unwrap();
            f.proxy.cfg.direct_recipes = exact;
            f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(model_form);
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":c["prompt"]}]);
            let reply = drive(&mut f, &mut s, &mut req);
            let text = reply["content"].as_str().unwrap();
            assert!(
                text.contains("Measured") && !text.contains("unverified"),
                "{c} exact={exact}: {reply}; {}",
                fs::read_to_string(f.dir.join("log.jsonl")).unwrap()
            );
            let d = s.execute("timeline.inspect", json!({"feature":feature}));
            let p = c["parameter"].as_str().unwrap();
            let v = match p {
                "depth" if c["kind"] == "counter" => d["definition"]["hole"]["cb_depth"].clone(),
                "diameter" if c["kind"] == "counter" => {
                    d["definition"]["hole"]["cb_diameter"].clone()
                }
                "diameter" if c["kind"] == "cylinder" => json!(
                    d["definition"]["radius"]
                        .as_f64()
                        .unwrap_or_else(|| d["definition"]["radius"]
                            .as_str()
                            .unwrap()
                            .parse()
                            .unwrap())
                        * 2.
                ),
                "x" => d["definition"]["position"][0].clone(),
                "y" => d["definition"]["position"][1].clone(),
                "height" | "depth" if d["definition"]["extent"].is_object() => {
                    d["definition"]["extent"]["distance"].clone()
                }
                "start_offset" => d["definition"]["extent"]["start_offset"].clone(),
                _ => d["definition"][p].clone(),
            };
            let actual = v
                .as_f64()
                .unwrap_or_else(|| v.as_str().unwrap().parse().unwrap());
            assert!(
                (actual - c["after"].as_f64().unwrap()).abs() < 1e-6,
                "{c}: {d}"
            );
            if exact && c["kind"] != "counter" {
                assert!(
                    f.proxy
                        .scripted
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .requests
                        .is_empty(),
                    "{c}"
                );
            }
            let faces = s.execute("model.faces", json!({"body":"Carrier"}));
            let prefix = format!("F{}:", d["id"].as_u64().unwrap());
            let owned: Vec<_> = faces["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["name"].as_str().unwrap().starts_with(&prefix))
                .collect();
            let after = c["after"].as_f64().unwrap();
            if p == "diameter" {
                assert!(
                    owned
                        .iter()
                        .filter_map(|v| v["cylinder"]["radius"].as_f64())
                        .any(|r| (r * 2. - after).abs() < 0.001),
                    "{c}: {faces}"
                );
            } else if matches!(p, "x" | "y") {
                let axis = usize::from(p == "y");
                assert!(
                    owned
                        .iter()
                        .filter_map(|v| v["cylinder"]["origin"][axis].as_f64())
                        .any(|v| (v - after).abs() < 0.001),
                    "{c}: {faces}"
                );
            } else {
                let (axis, expected) = match p {
                    "width" => (1, 12. + after),
                    "length" => (0, 15. + after),
                    "start_offset" => (2, 40. + after),
                    "height" => (2, 32. + after),
                    _ => (
                        2,
                        if c["kind"] == "under" {
                            after
                        } else {
                            32. - after
                        },
                    ),
                };
                assert!(
                    owned.iter().any(|v| v["plane_normal"][axis]
                        .as_f64()
                        .is_some_and(|n| n.abs() > 0.999)
                        && v["centroid"][axis]
                            .as_f64()
                            .is_some_and(|v| (v - expected).abs() < 0.001)),
                    "{c}: {faces}"
                );
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("named_feature"));
        }
    }
}

#[test]
fn named_feature_nonroutes_preserve_first_request_bytes() {
    let cases: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_nonroutes.json"
    ))
    .unwrap();
    assert!(cases.len() >= 15);
    for text in cases {
        assert!(!planner::named_feature::candidate(&text), "{text}");
        let mut old = fill_fixture(vec![reply("Please clarify.")]);
        old.proxy
            .recipes
            .recipes
            .retain(|r| r.id != "named_feature");
        let mut new = fill_fixture(vec![reply("Please clarify.")]);
        for exact in [false, true] {
            old.proxy.cfg.direct_recipes = exact;
            new.proxy.cfg.direct_recipes = exact;
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":text}]);
            let a = old.chat(req.clone());
            let b = new.chat(req);
            assert_eq!(a.status, b.status, "{text}");
            let a = old.proxy.scripted.as_ref().unwrap().borrow();
            let b = new.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(
                serde_json::to_vec(&a.requests).unwrap(),
                serde_json::to_vec(&b.requests).unwrap(),
                "{text}"
            );
        }
    }
}

fn ambiguous_model(wire: &Value) -> Value {
    if wire["tools"][0]["function"]["name"] != "pilot_fill" {
        return reply("Which hole should change? Please give its feature name.");
    }
    let s = &wire["tools"][0]["function"]["parameters"];
    assert_eq!(
        s["properties"]["feature"]["enum"].as_array().unwrap().len(),
        2
    );
    call("pilot_fill", json!({"decline":true}))
}

fn inspected(s: &mut Session, text: &str, cat: &Catalog) -> Value {
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":text}]);
    for _ in 0..3 {
        let reads = planner::named_feature::reads(&req, cat).unwrap();
        for c in reads {
            let result = s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":result.to_string()}));
        }
    }
    req
}
#[test]
fn named_feature_verification_requires_fresh_changed_geometry() {
    let Some(mut s) = Session::new() else { return };
    setup(&mut s, &json!({"kind":"hole","multi":false,"named":false}));
    let cat = context_tests::public_catalog::catalog().unwrap();
    let mut req = inspected(&mut s, "deepen the hole by 3 mm", &cat);
    let plan = planner::named_feature::exact(&req, &cat).unwrap().unwrap();
    assert!(planner::named_feature::completion(&req, &plan).is_some());
    let calls = direct::calls(&req, &plan, 0);
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"assistant","tool_calls":calls}));
    for c in &plan.calls {
        let result = s.execute(
            c["arguments"]["command"].as_str().unwrap(),
            c["arguments"]["params"].clone(),
        );
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":result.to_string()}));
    }
    assert!(planner::named_feature::completion(&req, &plan).is_none());
    let faces = req["messages"].as_array().unwrap().len() - 3;
    let original = req["messages"][faces]["content"].clone();
    req["messages"][faces]["content"] = json!(json!({"body":"Carrier","faces":[]}).to_string());
    assert!(planner::named_feature::completion(&req, &plan).is_some());
    req["messages"][faces]["content"] = original;
    let mut data: Value =
        serde_json::from_str(req["messages"][faces]["content"].as_str().unwrap()).unwrap();
    for f in data["faces"].as_array_mut().unwrap() {
        f["exact"] = json!(false);
    }
    req["messages"][faces]["content"] = json!(data.to_string());
    assert!(planner::named_feature::completion(&req, &plan).is_some());
}
#[test]
fn named_feature_two_candidates_never_guesses() {
    let Some(mut s) = Session::new() else { return };
    setup(&mut s, &json!({"kind":"hole","multi":false,"named":false}));
    s.execute(
        "solid.hole",
        json!({"position":[48,21,32],"diameter":8,"depth":12,"targets":["Carrier"]}),
    );
    let mut f = fill_fixture(vec![]);
    f.proxy.cfg.direct_recipes = true;
    f.proxy.catalog = context_tests::public_catalog::catalog().unwrap();
    f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(ambiguous_model);
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":"deepen the hole by 3 mm"}]);
    // Drive the three inspection rounds, then check the dynamic form directly.
    for _ in 0..3 {
        let out = f.chat(req.clone());
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        let msg = out["choices"][0]["message"].clone();
        req["messages"].as_array_mut().unwrap().push(msg.clone());
        for c in msg["tool_calls"].as_array().unwrap() {
            let args: Value =
                serde_json::from_str(c["function"]["arguments"].as_str().unwrap()).unwrap();
            let result = s.execute(args["command"].as_str().unwrap(), args["params"].clone());
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","tool_call_id":c["id"],"content":result.to_string()}));
        }
    }
    assert!(planner::named_feature::exact(&req, &f.proxy.catalog)
        .unwrap()
        .is_none());
    let form = planner::named_feature::form(&req);
    assert_eq!(
        form["properties"]["feature"]["enum"],
        json!(["Hole2", "Hole1"])
    );
    let out = f.chat(req.clone());
    let out: Value = serde_json::from_slice(&out.body).unwrap();
    assert!(
        out["choices"][0]["message"]["tool_calls"].is_null(),
        "{out}"
    );
    assert!(fs::read_to_string(f.dir.join("log.jsonl"))
        .unwrap()
        .contains("fill_declined"));
    for name in ["Hole1", "Hole2"] {
        assert_eq!(
            s.execute("timeline.inspect", json!({"feature":name}))["definition"]["depth"],
            "12"
        );
    }
    let err = planner::named_feature::build(
        &json!({"feature":"Hole1","parameter":"depth","mode":"change","value":3}),
        &req,
        &f.proxy.catalog,
    )
    .unwrap_err();
    assert!(err.contains("ambiguous"));
    // A body or actual name separates the candidates, without a model.
    req["messages"][0]["content"] = json!("deepen the hole Hole1 by 3 mm");
    assert!(planner::named_feature::exact(&req, &f.proxy.catalog)
        .unwrap()
        .is_some());
}

#[test]
fn named_feature_bounds_and_no_model_path() {
    let Some(mut s) = Session::new() else { return };
    setup(&mut s, &json!({"kind":"pocket","multi":true,"named":true}));
    let cat = context_tests::public_catalog::catalog().unwrap();
    let mut req = inspected(&mut s, "deepen the pocket by 3 mm", &cat);
    for args in [
        json!({"feature":"Well","parameter":"depth","mode":"change","value":-20}),
        json!({"feature":"Well","parameter":"depth","mode":"set","value":1000001}),
        json!({"feature":"Other","parameter":"depth","mode":"set","value":4}),
        json!({"feature":"Well","parameter":"unsupported","mode":"set","value":4}),
        json!({"feature":"Well","parameter":"depth","mode":"change","value":3,"set":{"depth":99}}),
    ] {
        assert!(
            planner::named_feature::build(&args, &req, &cat).is_err(),
            "{args}"
        );
    }
    req["messages"][0]["content"] = json!("deepen the pocket in body Missing by 3 mm");
    assert!(planner::named_feature::exact(&req, &cat).unwrap().is_none());
    let mut f = fill_fixture(vec![]);
    f.proxy.catalog = cat;
    f.proxy.cfg.direct_recipes = true;
    f.proxy.cfg.upstream = "none".into();
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":"raise the pocket in body Carrier by 2 mm"}]);
    let out = drive(&mut f, &mut s, &mut req);
    assert!(
        out["content"].as_str().unwrap().contains("Measured"),
        "{out}"
    );
    assert!(f
        .proxy
        .scripted
        .as_ref()
        .unwrap()
        .borrow()
        .requests
        .is_empty());
}

/// These oracles use constructive solid arithmetic, never planner slots or parameter echoes.
#[test]
fn named_feature_varied_public_cli_volume_and_bounds() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_varied.json"
    ))
    .unwrap();
    assert!(cases.len() >= 60);
    let cat = context_tests::public_catalog::catalog().unwrap();
    for c in cases {
        eprintln!("varied: {}", c["prompt"]);
        let Some(mut s) = Session::new() else { return };
        for step in c["setup"].as_array().unwrap() {
            s.execute(step["command"].as_str().unwrap(), step["params"].clone());
        }
        let text = c["prompt"].as_str().unwrap();
        assert!(planner::named_feature::candidate(text), "{text}");
        let mut req = inspected(&mut s, text, &cat);
        let schema = planner::named_feature::form(&req);
        let args = json!({"feature":c["feature"],"parameter":c["parameter"],"mode":c["mode"],"value":c["value"]});
        let exact = planner::named_feature::exact(&req, &cat).unwrap();
        let result = planner::named_feature::build(&args, &req, &cat);
        if c["reject"] == true {
            assert!(exact.is_none(), "{c}: {exact:?}");
            assert!(result.is_err(), "{c}: {result:?}; {schema}");
        } else {
            assert!(
                schema["properties"]["parameter"]["enum"]
                    .as_array()
                    .unwrap()
                    .contains(&c["parameter"]),
                "{c}: {schema}"
            );
            let plan =
                exact.unwrap_or_else(|| result.unwrap_or_else(|e| panic!("{c}: {e}; {schema}")));
            let emitted = direct::calls(&req, &plan, 0);
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"assistant","tool_calls":emitted}));
            for call in &plan.calls {
                let result = s.execute(
                    call["arguments"]["command"].as_str().unwrap(),
                    call["arguments"]["params"].clone(),
                );
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","content":result.to_string()}));
            }
            assert_eq!(
                planner::named_feature::completion(&req, &plan).is_some(),
                c["report"] == true,
                "{c}: {plan:?}; {}",
                req["messages"].as_array().unwrap().last().unwrap()
            );
        }
        let doc = s.execute("document.inspect", json!({"measure":true}));
        let b = doc["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == c["body"])
            .unwrap();
        let expected = c["volume"].as_f64().unwrap();
        assert!(
            (b["volume_mm3"].as_f64().unwrap() - expected).abs() < expected * 0.0002,
            "{c}: {b}"
        );
        for (j, key) in ["min", "max"].iter().enumerate() {
            for a in 0..3 {
                assert!(
                    (b["bbox"][key][a].as_f64().unwrap() - c["bounds"][j][a].as_f64().unwrap())
                        .abs()
                        < 0.001,
                    "{c}: {b}"
                );
            }
        }
    }
}

fn varied_form(wire: &Value) -> Value {
    let text = wire["messages"][1]["content"]
        .as_str()
        .unwrap()
        .split("\nDesign context:")
        .next()
        .unwrap();
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_varied.json"
    ))
    .unwrap();
    let c = cases.iter().find(|c| c["prompt"] == text).unwrap();
    let schema = &wire["tools"][0]["function"]["parameters"];
    assert!(
        schema["properties"]["feature"]["enum"]
            .as_array()
            .unwrap()
            .contains(&c["feature"]),
        "{wire}"
    );
    call(
        "pilot_fill",
        json!({"feature":c["feature"],"parameter":c["parameter"],"mode":c["mode"],"value":c["value"]}),
    )
}

#[test]
fn named_feature_new_wordings_reach_forms_before_creation_and_failures_stay_reports() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dev_named_feature_varied.json"
    ))
    .unwrap();
    for index in [0, 9, 14, 23, 37, 43, 60, 66] {
        let c = &cases[index];
        let Some(mut s) = Session::new() else { return };
        for step in c["setup"].as_array().unwrap() {
            s.execute(step["command"].as_str().unwrap(), step["params"].clone());
        }
        let mut f = fill_fixture(vec![]);
        f.proxy.catalog = context_tests::public_catalog::catalog().unwrap();
        f.proxy.cfg.direct_recipes = true;
        f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(varied_form);
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":c["prompt"]}]);
        let result = drive(&mut f, &mut s, &mut req);
        let text = result["content"].as_str().unwrap();
        if c["report"] == true {
            assert!(text.contains("unverified"), "{c}: {result}");
        } else {
            assert!(
                text.contains("Measured") && !text.contains("unverified"),
                "{c}: {result}"
            );
        }
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("named_feature"), "{log}");
    }
}
