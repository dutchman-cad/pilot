//! Independently authored box/container wordings, using only the public CLI.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::Catalog,
    planner,
    recipe_library::{forms, Bm25, Library},
};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::Command;
use std::process::Stdio;

// Separate public MCP calls keep the session alive after a refused edge feature,
// so the following inspection measures the shell actually left by that failure.
fn replay_edges(cli: &std::ffi::OsStr, commands: &[Value]) -> Vec<Value> {
    assert!(commands.len() <= 16);
    let mut child = Command::new(cli)
        .args(["mcp", "--headless"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut results = Vec::new();
    for (n, command) in commands.iter().enumerate() {
        writeln!(input,"{}",json!({"jsonrpc":"2.0","id":n,"method":"tools/call","params":{"name":"execute","arguments":command}})).unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output
            .by_ref()
            .take(4 * 1024 * 1024)
            .read_line(&mut line)
            .unwrap();
        let wire: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(wire["id"], n, "{wire}");
        let result = &wire["result"];
        let payload = result.get("structuredContent").cloned().unwrap_or_else(|| {
            let text = result["content"][0]["text"].as_str().unwrap();
            if result["isError"] == true {
                json!({"error":text})
            } else {
                serde_json::from_str(text).unwrap_or_else(|e| panic!("{wire}: {e}"))
            }
        });
        results.push(payload);
    }
    child.kill().unwrap();
    child.wait().unwrap();
    results
}
fn request(text: &str) -> Value {
    json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
}
fn compare(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            assert!((a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-9)
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, v) in a {
                compare(v, &b[k]);
            }
        }
        _ => assert_eq!(a, b),
    }
}
fn cases() -> Vec<(String, Value, bool)> {
    let mut out = Vec::new();
    for (sizes, xyz, wall) in [
        ("68 × 46 × 32 mm", [68., 46., 32.], "2 mm"),
        ("6.8 x 4.6 x 3.2 cm", [68., 46., 32.], "0.2 cm"),
        ("3 by 2 by 1 inches", [76.2, 50.8, 25.4], "2 mm"),
        ("68mm x 46mm x 32mm", [68., 46., 32.], "2mm"),
    ] {
        for (style, choice, exact) in [
            ("box", "top", false),
            ("hollow container", "top", false),
            ("open top box", "top", true),
            ("closed container", "closed", true),
            ("lid", "bottom", true),
            ("box open bottom", "bottom", true),
            ("hollow open-top container", "top", true),
            ("closed hollow box", "closed", true),
        ] {
            let text = format!("Make a {sizes} {style} with {wall} walls.");
            out.push((
                text,
                json!({"length":xyz[0],"width":xyz[1],"height":xyz[2],"wall":2.,"open":choice}),
                exact,
            ));
        }
    }
    for (edge, size) in [
        ("fillet", "0.2 mm"),
        ("chamfer", "0.2mm"),
        ("fillet", "3mm"),
        ("round", "0.02cm"),
    ] {
        out.push((format!("Build an open top box 68 x 46 x 32mm with 2mm walls and {edge} all the edges {size}."),json!({"length":68.,"width":46.,"height":32.,"wall":2.,"open":"top"}),true));
    }
    for edge in [
        "0.2mm fillets on all edges",
        "0.02 cm chamfers on all the edges",
        "rounded all edges radius 0.2mm",
        "chamfered all edges distance 0.2mm",
    ] {
        out.push((
            format!("Make an outside 68x46x32mm open top box with 2mm walls and {edge}."),
            json!({"length":68.,"width":46.,"height":32.,"wall":2.,"open":"top"}),
            true,
        ));
    }
    out.push((
        "open top box 68x46x32mm walls 2mm fillet all edges radius 0.2mm".into(),
        json!({"length":68.,"width":46.,"height":32.,"wall":2.,"open":"top"}),
        true,
    ));
    out
}
const MISSES: &[&str] = &[
    "open top box 68 x 46mm with 2mm walls",
    "open top box 68x46x32mm with walls",
    "open top box 68x46x32mm with -2mm walls",
    "open top box 68x46x32mm with 25mm walls",
    "box 68x46x32mm walls 2mm open top or bottom",
    "box 68x46x32mm walls 2mm open top and open bottom",
    "not an open top box 68x46x32mm walls 2mm",
    "open top box 68x46x32mm walls 2mm and holes",
    "closed box 68x46x32mm walls 2mm open top",
    "open top box 68x46x32mm walls 2mm fillet all edges",
    "open top box 68x46x32mm walls 2mm fillet all edges 1mm or 2mm",
    "open top box 68x46x32mm walls 2mm chamfer top edges 1mm",
];
#[test]
fn hollow_box_development_wordings() {
    let cat = Catalog::bundled().unwrap();
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "enclosure")
        .unwrap()
        .schema;
    assert!(cases().len() >= 30);
    for (text, mut args, exact) in cases() {
        let req = request(&text);
        let found = lib.find(&text, &Bm25).unwrap();
        assert_eq!(found["recipes"][0]["id"], "enclosure", "{text}: {found}");
        assert!(
            found["recipes"][0]["score"].as_f64().unwrap() >= 4.5,
            "{text}: {found}"
        );
        assert_eq!(
            planner::plan_direct(&req, &cat).unwrap().is_some(),
            exact,
            "{text}"
        );
        args.as_object_mut().unwrap().remove("open");
        forms::request_defaults(&mut args, schema, &req).unwrap();
        let form = lib
            .build(&json!({"recipe":"enclosure","args":args}), &req, &cat)
            .unwrap();
        if exact {
            let plan = planner::plan_direct(&req, &cat).unwrap().unwrap();
            compare(&json!(plan.calls), &json!(form.calls));
        }
    }
    for text in MISSES {
        assert!(
            planner::plan_direct(&request(text), &cat)
                .unwrap()
                .is_none(),
            "claimed {text}"
        );
    }
    let original = "a 50 x 50 x 30 mm box with 2 mm walls and fillet all the edges";
    assert_eq!(lib.form_band(original).unwrap()["band"], "fill");
    assert_eq!(
        lib.form_band(original).unwrap()["recipes"][0]["id"],
        "enclosure"
    );
    assert!(planner::plan_direct(&request(original), &cat)
        .unwrap()
        .is_none());
    for text in [
        "box 68x46x32mm walls 2mm",
        "hollow box 68x46x32mm walls 2mm",
        "box 68x46x32mm walls 2mm not open top",
        "closed box 68x46x32mm walls 2mm open top",
        "box 68x46x32mm walls 2mm open top or bottom",
    ] {
        let mut args = json!({"length":68.,"width":46.,"height":32.,"wall":2.});
        assert!(
            forms::request_defaults(&mut args, schema, &request(text))
                .unwrap()
                .is_empty(),
            "{text}"
        );
        assert!(args.get("open").is_none());
    }
    let mut args = json!({"length":68.,"width":46.,"height":3.,"wall":2.});
    assert!(
        forms::request_defaults(&mut args, schema, &request("closed box"))
            .unwrap()
            .is_empty()
    );
    // A request never overrides a supplied opening, even when it says closed.
    let mut args = json!({"length":68.,"width":46.,"height":32.,"wall":2.,"open":"bottom"});
    assert!(
        forms::request_defaults(&mut args, schema, &request("closed box"))
            .unwrap()
            .is_empty()
    );
    for text in [
        "box 68x46x32mm walls 2mm do not fillet all edges radius 1mm",
        "box 68x46x32mm walls 2mm fillet all edges 1mm or 2mm",
        "box 68x46x32mm walls 2mm fillet all edges",
    ] {
        let form = lib
            .build(
                &json!({"recipe":"enclosure","args":args}),
                &request(text),
                &cat,
            )
            .unwrap();
        assert!(form.continuation.is_none());
    }
}
fn replay(cli: &std::ffi::OsStr, commands: &[Value], id: usize) -> Vec<Value> {
    assert!(commands.len() <= 16);
    let file = std::env::temp_dir().join(format!("pilot-hollow-{}-{id}.json", std::process::id()));
    std::fs::write(&file, serde_json::to_vec(commands).unwrap()).unwrap();
    let output = Command::new(cli)
        .arg("run")
        .arg(&file)
        .arg("--results")
        .arg("--allow-errors")
        .output()
        .unwrap();
    std::fs::remove_file(file).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 4 * 1024 * 1024);
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn hollow_box_public_cli_dozen() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = Library::embedded().unwrap();
    let all = cases();
    for id in [0, 2, 3, 4, 10, 11, 18, 19, 32, 33, 34, 35] {
        let (text, args, _) = &all[id];
        let mut req = request(text);
        let plan = lib
            .build(&json!({"recipe":"enclosure","args":args}), &req, &cat)
            .unwrap();
        let mut commands: Vec<_> = plan.calls.iter().map(|c| c["arguments"].clone()).collect();
        let mut results = replay(&cli, &commands, id);
        let mut rounded = false;
        if plan.continuation.is_some() {
            for (n, result) in results.iter().enumerate() {
                req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":format!("dev{n}"),"content":result.to_string()}));
            }
            let next = planner::resume(&req, &plan, &cat).unwrap().unwrap();
            assert!(matches!(
                next.calls[0]["arguments"]["command"].as_str(),
                Some("solid.fillet" | "solid.chamfer")
            ));
            commands.extend(next.calls.iter().map(|c| c["arguments"].clone()));
            results = replay_edges(&cli, &commands);
            let last = results.last().unwrap();
            rounded = last["timeline"].as_array().unwrap().iter().any(|f| {
                matches!(f["type"].as_str(), Some("FilletFeature" | "ChamferFeature"))
                    && f["error"].is_null()
            });
            if !rounded {
                assert!(
                    results[results.len() - 2].get("error").is_some(),
                    "missing edge refusal: {results:?}"
                );
            }
            eprintln!("edge replay {text}: built={rounded}");
        }
        let state = results.last().unwrap();
        assert_eq!(
            state["bodies"].as_array().unwrap().len(),
            1,
            "{text}: {state}"
        );
        let body = &state["bodies"][0];
        let x = args["length"].as_f64().unwrap();
        let y = args["width"].as_f64().unwrap();
        let z = args["height"].as_f64().unwrap();
        let volume =
            x * y * z - (x - 4.) * (y - 4.) * (z - if args["open"] == "closed" { 4. } else { 2. });
        let actual = body["volume_mm3"].as_f64().unwrap();
        if rounded {
            assert!(actual > 0. && actual < volume * 1.05, "{text}: {actual}");
        } else {
            assert!(
                (actual - volume).abs() < 0.01,
                "{text}: {actual} != {volume}"
            );
        }
        for (end, expected) in [("min", [0., 0., 0.]), ("max", [x, y, z])] {
            for (axis, v) in expected.into_iter().enumerate() {
                assert!(
                    (body["bbox"][end][axis].as_f64().unwrap() - v).abs() < 0.001,
                    "{text}: {body}"
                );
            }
        }
    }
}
