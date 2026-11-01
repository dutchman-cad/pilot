// Independently authored public setups; the scripted extractor sees only its wire request.
use super::*;
use crate::catalog::MAX_CATALOG_BYTES;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
#[path = "../../../tests/support/public_catalog.rs"]
mod public_catalog;

struct Session {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: usize,
}
impl Session {
    fn new() -> Option<Self> {
        let path = std::env::var_os("PILOT_SOLIDCRAFT_CLI")?;
        let mut child = Command::new(path)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Some(Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            id: 0,
        })
    }
    fn execute(&mut self, command: &str, params: Value) -> Value {
        self.tool("execute", json!({"command":command,"params":params}))
    }
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.id += 1;
        writeln!(self.input,"{}",json!({"jsonrpc":"2.0","id":self.id,"method":"tools/call","params":{"name":name,"arguments":arguments}})).unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert!(
            response.get("error").is_none(),
            "{name} {arguments}: {response}"
        );
        assert_ne!(
            response["result"]["isError"], true,
            "{name} {arguments}: {response}"
        );
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn extractor(wire: &Value) -> Value {
    let user = wire["messages"][1]["content"].as_str().unwrap_or("");
    let text = user.split("\nDesign context: ").next().unwrap_or("");
    if wire["tools"][0]["function"]["name"] == "pilot_route" {
        let id = if text.contains("revolute") {
            "revolute_joint"
        } else if text.contains("fillet") {
            "fillet"
        } else if text.contains("chamfer") {
            "chamfer"
        } else if text.contains("move") {
            "move"
        } else if text.contains("mirror") {
            "mirror"
        } else if text.contains("shell") {
            "shell"
        } else if text.contains("grid") {
            "grid_pattern"
        } else if text.contains("circle") {
            "circular_pattern"
        } else {
            "linear_pattern"
        };
        return call("pilot_route", json!({"recipe":id}));
    }
    if wire["tools"][0]["function"]["name"] != "pilot_fill" {
        return reply("Please identify the target.");
    }
    let Some((_, summary)) = user.split_once("\nDesign context: ") else {
        return call("pilot_fill", json!({"decline":true}));
    };
    let context: Value = serde_json::from_str(summary).unwrap();
    assert!(context["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["bbox"].is_object() && b["centre"].is_array()));
    if text.contains("revolute") {
        assert!(context["components"].as_array().unwrap().len() >= 2);
        assert!(!context["pins_and_holes"].as_array().unwrap().is_empty());
        assert!(context["pins_and_holes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["kind"] == "pin"));
        assert!(context["pins_and_holes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["kind"] == "hole"));
        return call(
            "pilot_fill",
            json!({"pin_component":"Base","hole_component":"Arm","angle_deg":30}),
        );
    }
    let mut args = if text.contains("fillet") || text.contains("chamfer") {
        json!({"amount":0.5})
    } else if text.contains("move") {
        json!({"delta":[3,0,0]})
    } else if text.contains("mirror") {
        json!({"plane":"YZ","keep_original":true})
    } else if text.contains("shell") {
        json!({"wall":1})
    } else if text.contains("grid") {
        json!({"target":"body","rows":2,"columns":3,"spacing1":12,"spacing2":12,"plane":"XY"})
    } else if text.contains("circle") {
        json!({"target":"body","count":6,"axis":"Z","angle_deg":360})
    } else {
        json!({"target":"body","count":6,"spacing":12,"direction":"X"})
    };
    args["body"] = context["default_body"].clone();
    if text.contains("last feature")
        || text.contains("boss")
        || text.contains("pegs")
        || text.contains("hole")
        || text.contains("rib")
    {
        args["target"] = json!("last_feature");
        let definition = &context["last_feature"]["definition"];
        args["body"] = definition["participants"][0].clone();
        if args["body"].is_null() {
            args["body"] = definition["body_names"][0].clone();
        }
    }
    if text.contains("the peg") || text.contains("Peg") {
        args["body"] = json!("Peg");
    }
    call("pilot_fill", args)
}

#[test]
fn public_clean_pattern_context_end_to_end() {
    for text in [
        "make 6 of these in a row 12 mm apart",
        "make these in a grid 2 rows 3 columns 12 mm apart",
        "make 6 of these around a circle about Z",
    ] {
        let Some(mut s) = Session::new() else {
            return;
        };
        s.execute(
            "solid.box",
            json!({"length":80,"width":40,"height":4,"body_name":"Plate"}),
        );
        s.execute(
            "solid.cylinder",
            json!({"radius":2,"height":6,"base":[20,10,4],"body_name":"Peg"}),
        );
        let mut f = fill_fixture(vec![]);
        f.proxy.catalog = public_catalog::catalog().unwrap();
        f.proxy.cfg.direct_recipes = false;
        f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(extractor);
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":text}]);
        let mut patterns = 0;
        let mut done = false;
        for _ in 0..8 {
            let response = f.chat(req.clone());
            assert_eq!(
                response.status,
                200,
                "{}",
                String::from_utf8_lossy(&response.body)
            );
            let response: Value = serde_json::from_slice(&response.body).unwrap();
            let message = response["choices"][0]["message"].clone();
            let Some(calls) = message["tool_calls"].as_array() else {
                assert!(
                    message["content"]
                        .as_str()
                        .unwrap()
                        .starts_with("Built from the model's form:"),
                    "{message}"
                );
                assert!(!message.to_string().contains("unverified"), "{message}");
                done = true;
                break;
            };
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(message.clone());
            for c in calls {
                let args: Value =
                    serde_json::from_str(c["function"]["arguments"].as_str().unwrap()).unwrap();
                patterns += args.to_string().matches("solid.pattern.").count();
                let result = s.tool(c["function"]["name"].as_str().unwrap(), args);
                req["messages"].as_array_mut().unwrap().push(
                    json!({"role":"tool","tool_call_id":c["id"],"content":result.to_string()}),
                );
            }
        }
        assert!(done);
        assert_eq!(patterns, 1);
        let state = s.execute("document.inspect", json!({"measure":true}));
        assert_eq!(state["bodies"].as_array().unwrap().len(), 7, "{state}");
    }
}

fn drive(f: &mut Fixture, s: &mut Session, req: &mut Value) -> Value {
    for _ in 0..8 {
        let response = f.chat(req.clone());
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        let response: Value = serde_json::from_slice(&response.body).unwrap();
        let message = response["choices"][0]["message"].clone();
        let Some(calls) = message["tool_calls"].as_array() else {
            return message;
        };
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(message.clone());
        for c in calls {
            let args: Value =
                serde_json::from_str(c["function"]["arguments"].as_str().unwrap()).unwrap();
            let result = s.tool(c["function"]["name"].as_str().unwrap(), args);
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","tool_call_id":c["id"],"content":result.to_string()}));
        }
    }
    panic!("bounded public drive did not finish");
}

#[test]
fn public_existing_forms_primitives_and_features() {
    for (text, setup) in [
        ("fillet all edges of this body to 0.5 mm", "box"),
        ("chamfer all edges of this body by 0.5 mm", "box"),
        ("move it 3 mm along X", "box"),
        ("mirror this body across YZ keeping the original", "box"),
        ("shell this box with 1 mm walls", "box"),
        ("make 6 of this boss in a row 12 mm apart", "boss"),
        ("Add a grid of pegs to the plate", "boss"),
        (
            "Please add a grid of pegs to this existing plate, thanks",
            "boss",
        ),
        (
            "Could you add a grid of round pegs to this plate please?",
            "boss",
        ),
        (
            "repeat the hole 6 times in a row 12 mm apart along X",
            "hole",
        ),
        (
            "repeat the last feature in a row 6 times 12 mm apart along X",
            "rib",
        ),
        (
            "make 6 copies of the peg in a row 12 mm apart along X",
            "peg",
        ),
        ("make 6 copies of Peg in a row 12 mm apart along X", "peg"),
        ("linear pattern 6 instances 12 mm apart along X", "single"),
    ] {
        let Some(mut s) = Session::new() else {
            return;
        };
        if matches!(setup, "boss" | "hole" | "rib") {
            s.execute(
                "solid.box",
                json!({"length":120,"width":60,"height":5,"body_name":"Plate"}),
            );
            s.execute("solid.cylinder",json!({"base":[15,15,5],"radius":3,"height":5,"operation":"join","targets":["Plate"]}));
            if setup == "hole" {
                s.execute(
                    "solid.hole",
                    json!({"position":[15,15,10],"diameter":2,"targets":["Plate"]}),
                );
            }
            if setup == "rib" {
                s.execute("solid.box",json!({"length":2,"width":8,"height":5,"corner":[15,15,5],"operation":"join","targets":["Plate"]}));
            }
        } else {
            s.execute(
                "solid.box",
                json!({"length":8,"width":6,"height":4,"corner":[10,0,0],"body_name":"Plate"}),
            );
            if setup != "single" {
                s.execute(
                    "solid.box",
                    json!({"length":4,"width":4,"height":4,"corner":[30,0,0],"body_name":"Peg"}),
                );
            }
        }
        let mut f = fill_fixture(vec![]);
        f.proxy.catalog = public_catalog::catalog().unwrap();
        f.proxy.cfg.direct_recipes = false;
        f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(extractor);
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":text}]);
        let message = drive(&mut f, &mut s, &mut req);
        assert!(
            message["content"]
                .as_str()
                .unwrap()
                .starts_with("Built from the model's form:"),
            "{text}: {message}; {}",
            fs::read_to_string(f.dir.join("log.jsonl")).unwrap()
        );
        assert!(
            !message.to_string().contains("unverified"),
            "{text}: {message}"
        );
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("fill_accepted"), "{text}: {log}");
    }
}

#[test]
fn public_joint_form_sees_components_and_contact_geometry() {
    let Some(mut s) = Session::new() else {
        return;
    };
    s.execute("component.create", json!({"name":"Base"}));
    s.execute(
        "solid.box",
        json!({"length":30,"width":24,"height":5,"body_name":"Base"}),
    );
    s.execute(
        "solid.cylinder",
        json!({"base":[15,12,5],"radius":3,"height":8,"operation":"join","targets":["Base"]}),
    );
    s.execute("component.activate", json!({"component":"root"}));
    s.execute("component.create", json!({"name":"Arm"}));
    s.execute(
        "solid.box",
        json!({"length":22,"width":12,"height":4,"corner":[9,6,5],"body_name":"Arm"}),
    );
    s.execute(
        "solid.hole",
        json!({"position":[15,12,9],"diameter":6.4,"targets":["Arm"]}),
    );
    s.execute("component.activate", json!({"component":"root"}));
    let mut f = fill_fixture(vec![]);
    f.proxy.catalog = public_catalog::catalog().unwrap();
    f.proxy.cfg.direct_recipes = false;
    f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(extractor);
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":"Connect Base and Arm with a revolute joint and swing Arm 30 degrees, keeping Base fixed and Arm upright"}]);
    let message = drive(&mut f, &mut s, &mut req);
    assert!(
        message["content"]
            .as_str()
            .unwrap()
            .starts_with("Built from the model's form:"),
        "{message}"
    );
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert!(log.contains("fill_accepted"), "{log}");
}

#[test]
fn selection_wins_and_failed_inspection_never_offers_a_form() {
    let mut f = fill_fixture(vec![]);
    f.proxy.cfg.direct_recipes = false;
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":"make 6 of these in a row 12 mm apart"}]);
    let out = f.chat(req.clone());
    let out: Value = serde_json::from_slice(&out.body).unwrap();
    let message = out["choices"][0]["message"].clone();
    let c = &message["tool_calls"][0];
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(message.clone());
    req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":c["id"],"content":"{\"isError\":true,\"error\":\"inspection denied\"}"}));
    let out = f.chat(req);
    let out: Value = serde_json::from_slice(&out.body).unwrap();
    assert!(out["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .contains("Form error"));
    assert!(f
        .proxy
        .scripted
        .as_ref()
        .unwrap()
        .borrow()
        .requests
        .is_empty());

    let Some(mut s) = Session::new() else {
        return;
    };
    s.execute(
        "solid.box",
        json!({"length":2,"width":3,"height":4,"body_name":"Selected"}),
    );
    s.execute(
        "solid.box",
        json!({"length":2,"width":3,"height":4,"corner":[100,0,0],"body_name":"Newest"}),
    );
    let mut f = fill_fixture(vec![]);
    f.proxy.catalog = public_catalog::catalog().unwrap();
    f.proxy.cfg.direct_recipes = false;
    f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(extractor);
    let mut req = request();
    req["selection"] = json!(["Selected"]);
    req["messages"] = json!([{"role":"user","content":"make 6 of these in a row 12 mm apart"}]);
    let out = drive(&mut f, &mut s, &mut req);
    assert!(
        out["content"]
            .as_str()
            .unwrap()
            .starts_with("Built from the model's form:"),
        "{out}"
    );
    let state = s.execute("document.inspect", json!({"measure":true}));
    assert!(
        state["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["name"] == "Selected (6)"),
        "{state}"
    );
    let script = f.proxy.scripted.as_ref().unwrap().borrow();
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    let events: Vec<Value> = log
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let forms: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "form_request")
        .collect();
    assert_eq!(forms.len(), script.requests.len());
    for (event, wire) in forms.iter().zip(&script.requests) {
        assert_eq!(event["after"]["bytes"], wire.to_string().len());
        assert!(wire["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("\"selection\":[\"Selected\"]"));
    }
    assert!(events.iter().any(|e| e["kind"] == "request_size"));
}

// Stand in for a provisional exact recognizer that accepts syntax before it
// knows target names, then declines it. This keeps the regression independent
// of future additions to the English parser.
fn provisional_context(f: &mut Fixture, mut req: Value, rule: &'static str) -> Value {
    let results = req["messages"].as_array_mut().unwrap().split_off(1);
    let commands = [
        "document.inspect",
        "component.list",
        "model.faces",
        "model.edges",
        "model.faces",
        "model.edges",
        "joint.list",
    ];
    let plan = planner::Plan {
        rule,
        calls: results
            .iter()
            .enumerate()
            .map(|(n, _)| json!({"tool":"execute","arguments":{"command":commands[n],"params":{}}}))
            .collect(),
        facts: None,
        slots: json!({}),
        expected: None,
        continuation: None,
    };
    let mut conversation = Conversation::default();
    conversation.model = "unit".into();
    conversation.history = recovery::history(&req).unwrap();
    let direct::Outcome::Reply(out) = f
        .proxy
        .direct_turn(&req, &Some(plan), 0, &mut conversation, &mut f.audit())
        .unwrap()
    else {
        panic!("provisional inspection did not emit");
    };
    let out: Value = serde_json::from_slice(&out.body).unwrap();
    let message = out["choices"][0]["message"].clone();
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(message.clone());
    for (call, result) in message["tool_calls"]
        .as_array()
        .unwrap()
        .iter()
        .zip(results)
    {
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","tool_call_id":call["id"],"content":result["content"]}));
    }
    f.proxy.conversations.push(conversation);
    req
}

#[test]
fn declined_provisional_claims_fill_from_existing_inspection() {
    for (rule, text, id, args) in [
        (
            "resolve_joint",
            "Hinge Base and Arm with a revolute joint and swivel Arm to 90 degrees",
            "revolute_joint",
            json!({"pin_component":"Base","hole_component":"Arm","angle_deg":90}),
        ),
        (
            "resolve_pattern",
            "Please arrange the Peg in a linear pattern, six in total, spaced 12 mm along X",
            "linear_pattern",
            json!({"target":"body","body":"Peg","count":6,"spacing":12,"direction":"X"}),
        ),
        (
            "resolve_body",
            "Please move the Peg three millimeters along X",
            "move",
            json!({"body":"Peg","delta":[3,0,0]}),
        ),
    ] {
        let mut req = if rule == "resolve_joint" {
            let mut req = planner::cases::revolute_request(text);
            let mut doc = tool_data(&req["messages"][1]).unwrap();
            doc["timeline"] = json!([]);
            req["messages"][1]["content"] = json!(doc.to_string());
            req
        } else {
            json!({"messages":[{"role":"user","content":text},{"role":"tool","content":json!({"bodies":[{"name":"Peg","bbox":{"min":[0,0,0],"max":[4,4,6]},"center_of_mass":[2,2,3],"volume_mm3":96}],"timeline":[]}).to_string()}],"tools":tools()})
        };
        req["tools"] = tools();
        let mut f = fill_fixture(vec![
            call("pilot_route", json!({"recipe":id})),
            call("pilot_fill", args.clone()),
        ]);
        f.proxy.cfg.direct_recipes = true;
        let band = f.proxy.recipes.form_band(text).unwrap();
        if band["band"] == "fill" {
            f.proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow_mut()
                .replies
                .remove(0);
        }
        assert_ne!(band["band"], "low", "{text}: {band}");
        let req = provisional_context(&mut f, req, rule);
        assert!(
            planner::plan_direct(&req, &f.proxy.catalog)
                .unwrap()
                .is_none(),
            "{text}: should decline exact parsing"
        );
        let hand_filled = planner::recipe_expand(id, &args, &req, &f.proxy.catalog).unwrap();
        let expected = direct::calls(&req, &hand_filled, 0);
        let out = f.chat(req.clone());
        assert_eq!(out.status, 200, "{}", String::from_utf8_lossy(&out.body));
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(
            out["choices"][0]["message"]["tool_calls"], expected,
            "{text}: {out}"
        );
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(
            script.requests.last().unwrap()["tool_choice"]["function"]["name"],
            "pilot_fill"
        );
        for wire in &script.requests {
            let user = wire["messages"][1]["content"].as_str().unwrap();
            assert!(user.starts_with(text));
            assert!(user.contains("Design context:"));
            assert!(user.contains(if rule == "resolve_joint" {
                "pins_and_holes"
            } else {
                "Peg"
            }));
        }
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("direct_recipe_form_handback"));
        assert!(log.contains("fill_accepted"));
        assert!(
            !log.contains("form_inspect"),
            "inspection must not be resent"
        );
    }
}

#[test]
fn declined_provisional_joint_without_model_reports_no_plan() {
    let mut f = fill_fixture(vec![]);
    f.proxy.cfg.direct_recipes = true;
    f.proxy.cfg.upstream = "none".into();
    let req = provisional_context(
        &mut f,
        planner::cases::revolute_request("Hinge Base and Arm and swivel Arm to 90 degrees"),
        "resolve_joint",
    );
    let out = f.chat(req);
    let out: Value = serde_json::from_slice(&out.body).unwrap();
    let text = out["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(text.contains("No joint plan was built"), "{text}");
    assert!(f
        .proxy
        .scripted
        .as_ref()
        .unwrap()
        .borrow()
        .requests
        .is_empty());
}

#[test]
fn provisional_handback_respects_opt_outs_and_failed_inspections() {
    for mode in ["no_fill", "no_shortcuts", "failed", "partial"] {
        let mut f = fill_fixture(vec![reply("Inspect the design and clarify the target.")]);
        f.proxy.cfg.direct_recipes = true;
        f.proxy.cfg.no_fill = mode == "no_fill";
        f.proxy.cfg.no_shortcuts = mode == "no_shortcuts";
        let mut req = provisional_context(
            &mut f,
            planner::cases::revolute_request("Hinge Base and Arm and swivel Arm to 90 degrees"),
            "resolve_joint",
        );
        if matches!(mode, "failed" | "partial") {
            let mut doc = tool_data(&req["messages"][2]).unwrap();
            if mode == "failed" {
                doc["error"] = json!("inspection failed");
            } else {
                doc["truncated"] = json!(true);
            }
            req["messages"][2]["content"] = json!(doc.to_string());
        }
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 1, "{mode}");
        assert!(
            !script.requests[0]["tools"]
                .to_string()
                .contains("pilot_fill"),
            "{mode}"
        );
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(!log.contains("direct_recipe_form_handback"), "{mode}");
    }
}
