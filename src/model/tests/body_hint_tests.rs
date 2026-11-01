//! Independently authored development geometry, replayed through the public binary only.
use super::*;
use std::process::Command;

fn native(steps: &[Value], dir: &Path) -> Vec<Value> {
    let cli = std::env::var_os("PILOT_SOLIDCRAFT_CLI").unwrap();
    let file = dir.join("body-hint-script.json");
    fs::write(&file, serde_json::to_vec(steps).unwrap()).unwrap();
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
    serde_json::from_slice(&output.stdout).unwrap()
}
fn execute(command: &str, params: Value) -> Value {
    json!({"command":command,"params":params})
}
fn request(prompt: &str) -> Value {
    json!({"messages":[{"role":"user","content":prompt}],"tools":[{"type":"function",
        "function":{"name":"execute","parameters":{"type":"object","properties":{
            "command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}}]})
}
fn result(req: &mut Value, args: Value, data: Value, id: usize) {
    let c = call("execute", args);
    let mut m = c["choices"][0]["message"].clone();
    m["tool_calls"][0]["id"] = json!(format!("development_{id}"));
    req["messages"].as_array_mut().unwrap().extend([
        m, json!({"role":"tool","tool_call_id":format!("development_{id}"),"content":data.to_string()})]);
}
fn facts(wire: &Value) -> Vec<String> {
    wire["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .filter_map(|m| m["content"].as_str())
        .flat_map(str::lines)
        .filter(|l| l.contains("Body check:"))
        .map(str::to_owned)
        .collect()
}
fn fixture(default_mode: bool, replies: Vec<Value>) -> Fixture {
    let mut f = Fixture::new(replies);
    f.proxy.cfg.planner = false;
    f.proxy.cfg.direct_recipes = default_mode;
    f
}
fn plate() -> Value {
    execute(
        "solid.box",
        json!({"length":40,"width":30,"height":5,"body_name":"Base plate"}),
    )
}

#[test]
fn body_hint_twenty_public_cli_join_examples() {
    if std::env::var_os("PILOT_SOLIDCRAFT_CLI").is_none() {
        return;
    }
    // Different faces, axes, contact depths, names and solid types; all are one-part requests.
    let cases = [
        (
            "Build a plate with an upright boss",
            "solid.cylinder",
            json!({"radius":4,"height":8,"base":[10,10,5]}),
        ),
        (
            "Make a base carrying an embedded boss",
            "solid.cylinder",
            json!({"radius":3,"height":7,"base":[15,10,3]}),
        ),
        (
            "Create a bracket with a side knuckle",
            "solid.cylinder",
            json!({"radius":2,"height":8,"axis":"X","base":[40,15,2.5]}),
        ),
        (
            "Model a plate with a front mounting lug",
            "solid.cylinder",
            json!({"radius":2,"height":6,"axis":"Y","base":[20,30,2.5]}),
        ),
        (
            "Build a plate with a downward foot",
            "solid.cylinder",
            json!({"radius":3,"height":6,"base":[10,10,-6]}),
        ),
        (
            "Make a base with a tapered boss",
            "solid.cone",
            json!({"radius":4,"top_radius":2,"height":9,"base":[20,15,5]}),
        ),
        (
            "Create a bracket with a tapered pad",
            "solid.cone",
            json!({"radius":3,"top_radius":1,"height":6,"base":[15,15,4]}),
        ),
        (
            "Build a plate with a reinforcing block",
            "solid.box",
            json!({"length":8,"width":6,"height":7,"corner":[12,12,5]}),
        ),
        (
            "Make a base with a short rib",
            "solid.box",
            json!({"length":20,"width":2,"height":5,"corner":[10,10,4]}),
        ),
        (
            "Create a bracket with a right flange",
            "solid.box",
            json!({"length":8,"width":10,"height":5,"corner":[40,10,0]}),
        ),
        (
            "Build a plate with a left flange",
            "solid.box",
            json!({"length":8,"width":10,"height":5,"corner":[-8,10,0]}),
        ),
        (
            "Make a bracket with a rear flange",
            "solid.box",
            json!({"length":10,"width":8,"height":5,"corner":[10,30,0]}),
        ),
        (
            "Create a bracket with a front flange",
            "solid.box",
            json!({"length":10,"width":8,"height":5,"corner":[10,-8,0]}),
        ),
        (
            "Build a base with a bottom pad",
            "solid.box",
            json!({"length":10,"width":8,"height":3,"corner":[10,10,-3]}),
        ),
        (
            "Make a plate with a raised rim",
            "solid.box",
            json!({"length":40,"width":2,"height":8,"corner":[0,0,4]}),
        ),
        (
            "Create a plate with a rounded bump",
            "solid.sphere",
            json!({"radius":4,"center":[20,15,5]}),
        ),
        (
            "Make a plate with a round reinforcing bump",
            "solid.sphere",
            json!({"radius":3,"center":[20,15,4]}),
        ),
        (
            "Make a base containing a reinforcement",
            "solid.box",
            json!({"length":4,"width":4,"height":2,"corner":[10,10,1]}),
        ),
        (
            "Create a base with a buried pin",
            "solid.cylinder",
            json!({"radius":1,"height":2,"base":[10,10,1]}),
        ),
        (
            "Build a plate with a wide top pad",
            "solid.box",
            json!({"length":30,"width":20,"height":4,"corner":[5,5,5]}),
        ),
    ];
    for (i, (prompt, command, mut params)) in cases.into_iter().enumerate() {
        let mut f = fixture(
            i % 2 == 0,
            vec![reply("Done."), reply("I will explain the remaining work.")],
        );
        let feature = match i {
            0 => "Boss \"A\"",
            1 => "Böss Ω",
            2 => "Side\\knuckle",
            _ => "Added feature",
        };
        params["body_name"] = json!(feature);
        let mut steps = vec![
            plate(),
            execute(command, params),
            execute("document.inspect", json!({"measure":true})),
            execute(
                "inspect.interference",
                json!({"bodies":["Base plate",feature],"budget_s":3}),
            ),
            execute(
                "inspect.measure",
                json!({"items":[{"type":"body","name":"Base plate"},{"type":"body","name":feature}]}),
            ),
        ];
        let results = native(&steps, &f.dir);
        let mut req = request(prompt);
        for (id, (args, data)) in steps.iter().zip(results).enumerate() {
            result(&mut req, args.clone(), data, id);
        }
        let out = f.chat(req);
        assert_eq!(out.status, 200, "case {i}");
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(
            script.requests.len(),
            2,
            "case {i}: final hand-back; {}",
            String::from_utf8_lossy(&out.body)
        );
        let fs = facts(&script.requests[0]);
        assert_eq!(fs.len(), 1, "case {i}: {}", script.requests[0]);
        let suggested: Value =
            serde_json::from_str(fs[0].split_once("join it: ").unwrap().1).unwrap();
        assert_eq!(suggested["tool"], "execute");
        assert_eq!(suggested["arguments"]["command"], "solid.combine");
        assert_eq!(suggested["arguments"]["params"]["target"], "Base plate");
        assert_eq!(suggested["arguments"]["params"]["tools"], json!([feature]));
        assert!(script.requests[1]["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["content"]
            .as_str()
            .unwrap()
            .contains(&fs[0]));
        drop(script);
        steps.push(suggested["arguments"].clone());
        steps.push(execute("document.inspect", json!({"measure":true})));
        let final_doc = native(&steps, &f.dir).pop().unwrap();
        assert_eq!(final_doc["bodies"].as_array().unwrap().len(), 1, "case {i}");
        assert_eq!(final_doc["bodies"][0]["closed"], true, "case {i}");
        assert_eq!(final_doc["bodies"][0]["kernel"]["shells"], 1, "case {i}");
    }
}

fn synthetic(prompt: &str, touching: bool) -> Value {
    let mut req = request(prompt);
    result(
        &mut req,
        execute("solid.box", json!({})),
        json!({"created_bodies":["Boss"],"bodies":["Plate","Boss"]}),
        0,
    );
    result(
        &mut req,
        execute("document.inspect", json!({"measure":true})),
        json!({"bodies":[
        {"name":"Plate","bbox":{"min":[0,0,0],"max":[10,10,2]}},
        {"name":"Boss","bbox":{"min":[0,0,2],"max":[10,10,4]}}]}),
        1,
    );
    for (id, read) in [
        execute(
            "inspect.interference",
            json!({"bodies":["Boss","Plate"],"budget_s":3}),
        ),
        execute(
            "inspect.measure",
            json!({"items":[{"type":"body","name":"Boss"},{"type":"body","name":"Plate"}]}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let data = if id == 0 {
            json!({"interferences":[]})
        } else {
            json!({"items":[{"name":"Boss"},{"name":"Plate"}],"distance_mm":if touching {0.0} else {1.0}})
        };
        result(&mut req, read, data, id + 2);
    }
    req
}
#[test]
fn body_hint_fifteen_separation_requests_are_silent() {
    let prompts = [
        "Make two separate parts, a plate and a boss",
        "Create two bodies touching each other",
        "Build an assembly of a base and a knuckle",
        "Make a plate and a separate boss",
        "Create copies as bodies",
        "Design several parts with mounting holes",
        "Model an assembly with a plate and standoffs",
        "Build independent components",
        "Create three disconnected bodies",
        "Make two parts spaced 50 mm apart",
        "Build an enclosure and its separate lid",
        "Create a mechanism with a rotating pin",
        "Make copies of this base as bodies",
        "Build a plate and a boss separately",
        "Create an assembly with non-touching components",
    ];
    for (i, prompt) in prompts.into_iter().enumerate() {
        let mut f = fixture(i % 2 == 0, vec![reply("Done.")]);
        let out = f.chat(synthetic(prompt, i % 2 == 0));
        assert_eq!(out.status, 200, "{prompt}");
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 1, "{prompt}");
        assert!(facts(&script.requests[0]).is_empty(), "{prompt}");
        assert!(!fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .contains("body_hint"));
    }
}

#[test]
fn body_hint_once_per_turn_survives_client_continuations_and_resets() {
    let mut f = fixture(false, vec![reply("Done."); 5]);
    let mut req = synthetic("Build a plate with a reinforcing boss", true);
    assert_eq!(f.chat(req.clone()).status, 200);
    // The client does not receive Pilot's internal hand-back; continuing it must not repeat it.
    req["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","tool_calls":[{"id":"client_read","function":{"name":"execute","arguments":"{\"command\":\"document.inspect\",\"params\":{}}"}}]}),
        json!({"role":"tool","tool_call_id":"client_read","content":"{}"})]);
    assert_eq!(f.chat(req.clone()).status, 200);
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        3
    );
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"Make this one part with the boss"}));
    // A retained complete inspection and contact evidence are still valid on the next user turn.
    assert_eq!(f.chat(req).status, 200);
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        5
    );
}

#[test]
fn body_hint_plain_style_and_valid_model_calls_pass_through() {
    for disabled in [false, true] {
        let valid = call(
            "execute",
            execute(
                "solid.cylinder",
                json!({"radius":2,"height":3,"body_name":"Another"}),
            ),
        );
        let mut f = fixture(false, vec![valid.clone()]);
        f.proxy.cfg.no_hints = disabled;
        let out = f.chat(synthetic("Build a plate with a boss", true));
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(out["choices"][0]["message"], valid["choices"][0]["message"]);
        assert_eq!(
            facts(&f.proxy.scripted.as_ref().unwrap().borrow().requests[0]).len(),
            usize::from(!disabled)
        );
    }
}

#[test]
fn body_hint_extra_reads_are_bounded_and_reuse_existing_document() {
    if std::env::var_os("PILOT_SOLIDCRAFT_CLI").is_none() {
        return;
    }
    let mut f = fixture(false, vec![reply("Done."), reply("Still done.")]);
    let mut steps = vec![
        plate(),
        execute(
            "solid.cylinder",
            json!({"radius":3,"height":5,"base":[10,10,5],"body_name":"Boss"}),
        ),
    ];
    let results = native(&steps, &f.dir);
    let mut req = request("Build a plate with a boss");
    for (i, (a, d)) in steps.iter().zip(results).enumerate() {
        result(&mut req, a.clone(), d, i);
    }
    let mut reads = Vec::new();
    let mut finished = false;
    for _ in 0..6 {
        let out = f.chat(req.clone());
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        let m = &out["choices"][0]["message"];
        let Some(cs) = m["tool_calls"].as_array() else {
            finished = true;
            break;
        };
        assert_eq!(cs.len(), 1);
        let args: Value =
            serde_json::from_str(cs[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert!(matches!(
            args["command"].as_str(),
            Some("document.inspect" | "inspect.interference" | "inspect.measure")
        ));
        reads.push(args["command"].as_str().unwrap().to_owned());
        steps.push(args);
        let d = native(&steps, &f.dir).pop().unwrap();
        req["messages"].as_array_mut().unwrap().extend([
            m.clone(),
            json!({"role":"tool","tool_call_id":cs[0]["id"],"content":d.to_string()}),
        ]);
    }
    assert!(finished);
    assert_eq!(
        reads,
        vec![
            "document.inspect",
            "inspect.interference",
            "inspect.measure"
        ]
    );
    assert_eq!(
        f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
        2
    );
}

#[test]
fn body_hint_bounds_failures_partial_snapshots_and_stale_evidence() {
    let f = fixture(false, vec![]);
    let mut audit = f.audit();
    let cat = &f.proxy.catalog;
    let mut state = body_hint::State::default();
    state.reset(0, true);
    let mut req = synthetic("Build a plate with a boss", true);
    // A later successful change invalidates both inspection and contact evidence.
    result(
        &mut req,
        execute("solid.move", json!({})),
        json!({"changed_bodies":["Boss"],"bodies":["Plate","Boss"]}),
        4,
    );
    assert!(body_hint::completion(&req, cat).is_none());
    let read = body_hint::prepare(&mut req, &mut state, cat, &mut audit, false)
        .unwrap()
        .unwrap();
    assert!(read["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap()
        .contains("document.inspect"));
    let id = read["tool_calls"][0]["id"].clone();
    req["messages"].as_array_mut().unwrap().extend([
        read,
        json!({"role":"tool","tool_call_id":id,"content":"{\"isError\":true}"}),
    ]);
    assert!(
        body_hint::prepare(&mut req, &mut state, cat, &mut audit, false)
            .unwrap()
            .is_none()
    );
    // A partial snapshot cannot revive a join based on stale measurements.
    result(
        &mut req,
        execute("document.inspect", json!({})),
        json!({"bodies":[{"name":"Plate"},{"name":"Boss"}],"truncated":true}),
        5,
    );
    assert!(body_hint::completion(&req, cat).is_none());

    // Bounding-box overlap is only a candidate, never evidence of contact.
    let mut clear = synthetic("Build a plate with a boss", false);
    assert!(body_hint::completion(&clear, cat)
        .unwrap()
        .contains("verify contact"));
    assert!(!body_hint::completion(&clear, cat)
        .unwrap()
        .contains("solid.combine"));
    body_hint::prepare(&mut clear, &mut state, cat, &mut audit, false).unwrap();
    assert!(facts(&clear).is_empty());
}

#[test]
fn body_hint_twelve_read_cap_and_four_document_cap() {
    let f = fixture(false, vec![]);
    let mut audit = f.audit();
    let mut state = body_hint::State::default();
    state.reset(0, true);
    let mut req = request("Build a base with reinforcing features");
    let mut total = 0;
    let mut documents = 0;
    // Every changed state offers fresh candidates, but cannot reset this turn's budget.
    for round in 0..20 {
        result(
            &mut req,
            execute("solid.box", json!({})),
            json!({"created_bodies":["B"],"bodies":["A","B"]}),
            round * 10,
        );
        for probe in 0..5 {
            let Some(read) =
                body_hint::prepare(&mut req, &mut state, &f.proxy.catalog, &mut audit, false)
                    .unwrap()
            else {
                break;
            };
            total += 1;
            let a: Value = serde_json::from_str(
                read["tool_calls"][0]["function"]["arguments"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            let d = if a["command"] == "document.inspect" {
                documents += 1;
                json!({"bodies":[{"name":"A","bbox":{"min":[0,0,0],"max":[10,10,10]}},
                    {"name":"B","bbox":{"min":[5,5,5],"max":[15,15,15]}}]})
            } else {
                json!({"isError":true})
            };
            let id = read["tool_calls"][0]["id"].clone();
            req["messages"].as_array_mut().unwrap().extend([
                read,
                json!({"role":"tool","tool_call_id":id,"content":d.to_string()}),
            ]);
            assert!(probe < 3, "unchanged failed reads must not repeat");
        }
    }
    assert_eq!(total, 12);
    assert_eq!(documents, 4);
}

#[test]
fn body_hint_mcp_envelopes_batches_and_streaming_keep_the_fact() {
    for (wrapper, stream) in [(0, false), (1, false), (2, true), (3, true)] {
        let mut f = fixture(false, vec![reply("Done."), reply("Done.")]);
        let mut req = synthetic("Build a hinge leaf with a raised knuckle", true);
        for m in req["messages"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|m| m["role"] == "tool")
        {
            let d: Value = serde_json::from_str(m["content"].as_str().unwrap()).unwrap();
            let d = match wrapper {
                0 => json!({"result":{"content":[{"type":"text","text":d.to_string()}]}}),
                1 => json!({"structuredContent":d}),
                2 => json!({"results":[{"result":d}]}),
                _ => json!({"after":d}),
            };
            m["content"] = json!(d.to_string());
        }
        req["stream"] = json!(stream);
        let out = f.chat(req);
        assert_eq!(out.status, 200);
        if stream {
            assert_eq!(out.content_type, "text/event-stream");
            assert_eq!(
                collect_sse(&out.body).unwrap()["choices"][0]["message"]["content"],
                "Done."
            );
        }
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        assert_eq!(facts(&script.requests[0]).len(), 1);
    }
}

#[test]
fn body_hint_disconnected_one_part_gets_one_count_handback_without_join() {
    let mut f = fixture(false, vec![reply("Done."), reply("Done.")]);
    let req = synthetic("Build a plate with a boss", false);
    let out = f.chat(req);
    assert_eq!(out.status, 200);
    let script = f.proxy.scripted.as_ref().unwrap().borrow();
    assert_eq!(script.requests.len(), 2);
    assert!(facts(&script.requests[0]).is_empty());
    let reason = script.requests[1]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(reason.contains("2 separate bodies"));
    assert!(!reason.contains("solid.combine"));
}
