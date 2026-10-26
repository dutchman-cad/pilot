#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
struct Session {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Session {
    fn start(cli: &std::path::Path, args: &[&str]) -> Self {
        Self::with_flags(cli, args, &[])
    }
    fn with_flags(cli: &std::path::Path, args: &[&str], flags: &[&str]) -> Self {
        let log = cli.parent().unwrap().join("rewrites.jsonl");
        let mut child = Command::new(env!("CARGO_BIN_EXE_dutchman-pilot"))
            .args(["mcp", "--solidcraft-cli"])
            .arg(cli)
            .arg("--log")
            .arg(log)
            .args(flags)
            .arg("--")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }
    fn send(&mut self, v: Value) {
        writeln!(self.input, "{v}").unwrap();
        self.input.flush().unwrap();
    }
    fn read(&mut self) -> String {
        let mut raw = String::new();
        assert!(self.output.read_line(&mut raw).unwrap() > 0);
        raw
    }
    fn call(&mut self, id: u64, name: &str, args: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));
        let v: Value = serde_json::from_str(&self.read()).unwrap();
        assert_eq!(v["id"], id);
        v
    }
    fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}));
        assert_eq!(
            serde_json::from_str::<Value>(&self.read()).unwrap()["id"],
            0
        );
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
}
fn fake() -> PathBuf {
    // Reuse the crate's already-built serde_json, keeping the fake server Rust-only.
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = std::fs::read_dir(&deps)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libserde_json-")
                && p.extension().is_some_and(|s| s == "rlib")
        })
        .unwrap();
    static NEXT_FAKE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_FAKE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "dutchman-pilot-mcp-test-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("fake-child{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new("rustc")
        .args(["--edition=2021", "--crate-name", "fake_child"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/fake_child.rs"
        ))
        .arg("--extern")
        .arg(format!("serde_json={}", library.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("-o")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    path
}

#[test]
fn sketch_error_and_existing_hints_are_audited_once_and_opt_out() {
    let path = fake();
    for disabled in [false, true] {
        let flags = if disabled { vec!["--no-hints"] } else { vec![] };
        let mut session = Session::with_flags(&path, &["--test-arg"], &flags);
        session.initialize();
        for (i, args, needle) in [
            (
                1,
                json!({"command":"sketch.dimension","params":{"entities":["l1","l2","l3"]}}),
                "One line's length",
            ),
            (
                2,
                json!({"command":"sketch.constraint.coincident","params":{}}),
                "move on to dimensions",
            ),
        ] {
            let first = texts(&session.call(i, "execute", args.clone()));
            assert_eq!(first.contains(needle), !disabled);
            let repeated = texts(&session.call(i + 10, "execute", args));
            assert!(!repeated.contains(needle));
        }
        session.call(
            30,
            "execute",
            json!({"command":"sketch.create","params":{}}),
        );
        let next = texts(&session.call(
            31,
            "execute",
            json!({"command":"sketch.dimension","params":{"entities":"l1"}}),
        ));
        assert_eq!(next.contains("One line's length"), !disabled);
    }
    let log = std::fs::read_to_string(path.parent().unwrap().join("rewrites.jsonl")).unwrap();
    assert!(log
        .lines()
        .any(|line| line.contains("next_step_hint") && line.contains("One line's length")));
}
fn texts(v: &Value) -> String {
    v["result"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn fake_child_end_to_end() {
    let path = fake();
    let mut session = Session::start(&path, &["--test-arg"]);
    session.initialize();
    let raw = "{ \"jsonrpc\" : \"2.0\", \"id\" : 1, \"method\" : \"raw\" }";
    writeln!(session.input, "{raw}").unwrap();
    session.input.flush().unwrap();
    assert_eq!(
        session.read(),
        format!(
            "{{  \"jsonrpc\": \"2.0\", \"id\": 1, \"result\": {}  }}\n",
            json!(raw)
        )
    );
    session.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let tools: Value = serde_json::from_str(&session.read()).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 5);
    assert_eq!(tools["result"]["tools"][3]["name"], "how");
    assert_eq!(tools["result"]["tools"][4]["name"], "plan");
    let plan: Value = serde_json::from_str(&texts(&session.call(
        20,
        "plan",
        json!({"task":"make a 20 mm cube"}),
    )))
    .unwrap();
    assert_eq!(
        plan["calls"],
        json!([{"tool":"execute","arguments":{"command":"solid.box","params":{"length":20.0,"width":20.0,"height":20.0,"operation":"new"}}}])
    );
    let failure = session.call(
        21,
        "execute",
        json!({"command":"make a 20 mm cube","params":{}}),
    );
    assert_eq!(failure["result"]["isError"], true);
    let text = texts(&failure);
    let suggestion: Value = serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("suggested: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(suggestion, plan["calls"][0]);
    let facts: Value = serde_json::from_str(&texts(&session.call(
        22,
        "plan",
        json!({"task":"how many bodies"}),
    )))
    .unwrap();
    assert_eq!(facts["facts"]["body_count"], 1);
    assert!(
        texts(&session.call(23, "plan", json!({"task":"make it fly"})))
            .contains("No deterministic plan")
    );
    let invalid = session.call(24, "plan", json!({"task":false}));
    assert_eq!(invalid["error"]["code"], -32602);
    let oversized = session.call(25, "plan", json!({"task":"x".repeat(4097)}));
    assert_eq!(oversized["error"]["code"], -32602);
    let rpc_error = session.call(26, "failed_task", json!({"task":"make a 20 mm cube"}));
    assert_eq!(rpc_error["error"]["code"], -32602);
    assert!(rpc_error["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("request failed\nsuggested: "));
    let suggestion: Value = serde_json::from_str(
        rpc_error["error"]["message"]
            .as_str()
            .unwrap()
            .split_once("suggested: ")
            .unwrap()
            .1,
    )
    .unwrap();
    assert_eq!(suggestion, plan["calls"][0]);
    let how = texts(&session.call(3, "how", json!({"task":"round edges 3 mm"})));
    let example: Value = serde_json::from_str(how.lines().next().unwrap()).unwrap();
    assert_eq!(example["arguments"]["command"], "solid.fillet");
    assert!(how.contains("Key parameters:"));
    let unknown = texts(&session.call(
        4,
        "execute",
        json!({"command":"solid.bxo","params":{"length":10,"width":10,"height":10}}),
    ));
    assert!(unknown.contains("suggested:"));
    assert!(unknown.contains("Did you mean"));
    assert!(unknown.contains("Say \"yes\" to do:"));
    assert!(unknown.contains("solid.box"));
    let correction = texts(&session.call(
        5,
        "execute",
        json!({"command":"solid.box","params":{"length":10,"width":10,"Height":10}}),
    ));
    assert!(correction.contains("suggested:"));
    assert!(correction.contains("\"height\":10"));
    assert!(texts(&session.call(6, "count", json!({}))).contains("\"executions\":0"));
    let change = texts(&session.call(7, "execute", json!({"command":"solid.box","params":{}})));
    assert!(change.contains("design didn't change"));
    let page = session.call(8, "pages", json!({}));
    assert_eq!(
        page["result"]["content"][0]["text"],
        "shown 2 of 9 (partial)"
    );
    assert!(texts(&session.call(9, "unknown_total", json!({})))
        .starts_with("shown 0 of unknown total (partial)"));
    let batch = texts(&session.call(
        10,
        "batch",
        json!({"commands":[{"command":"solid.box","params":{}}]}),
    ));
    assert!(batch.contains("design didn't change"));
    // Server-initiated requests and client responses retain their original bytes.
    session.send(json!({"jsonrpc":"2.0","id":77,"method":"reverse"}));
    assert_eq!(session.read(), "{ \"jsonrpc\":\"2.0\",\"id\":\"reverse-id\",\"method\":\"sampling/createMessage\",\"params\":{} }\n");
    session.send(json!({"jsonrpc":"2.0","id":"reverse-id","result":"client answer"}));
    assert!(session.read().contains("reverse completed"));
    drop(session);
    let mut fallback = Session::start(&path, &["--test-arg", "--fallback"]);
    fallback.initialize();
    assert!(texts(&fallback.call(1, "how", json!({"task":"box"}))).contains("solid.box"));
    drop(fallback);
    let mut disabled = Session::with_flags(
        &path,
        &["--test-arg"],
        &["--no-plan", "--no-how", "--no-hints"],
    );
    disabled.initialize();
    disabled.send(json!({"jsonrpc":"2.0","id":30,"method":"tools/list"}));
    let tools: Value = serde_json::from_str(&disabled.read()).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 3);
    assert_eq!(
        texts(&disabled.call(31, "plan", json!({"task":"make a 20 mm cube"}))),
        "{\"ok\":true}"
    );
    assert_eq!(
        texts(&disabled.call(32, "how", json!({"task":"box"}))),
        "{\"ok\":true}"
    );
    let failed = texts(&disabled.call(
        33,
        "execute",
        json!({"command":"make a 20 mm cube","params":{}}),
    ));
    assert!(!failed.contains("suggested:"));
    assert!(
        !texts(&disabled.call(34, "execute", json!({"command":"solid.box","params":{}})))
            .contains("design didn't change")
    );
    assert!(texts(&disabled.call(35, "count", json!({}))).contains("\"inspections\":0"));
    assert!(texts(&disabled.call(36, "pages", json!({}))).starts_with("shown 2 of 9 (partial)"));
    drop(disabled);
    // Each switch affects its own feature, leaving the others enabled.
    for flag in ["--no-plan", "--no-how", "--no-hints"] {
        let mut session = Session::with_flags(&path, &["--test-arg"], &[flag]);
        session.initialize();
        session.send(json!({"jsonrpc":"2.0","id":40,"method":"tools/list"}));
        let reply: Value = serde_json::from_str(&session.read()).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(
            tools
                .iter()
                .filter(|t| t["description"]
                    .as_str()
                    .is_some_and(|s| s.contains(dutchman_pilot::prompting::PLACEMENT_FACT)))
                .count(),
            usize::from(flag != "--no-hints")
        );
        assert_eq!(
            tools.iter().any(|t| t["name"] == "plan"),
            flag != "--no-plan"
        );
        assert_eq!(
            tools
                .iter()
                .filter(|t| t["description"]
                    .as_str()
                    .is_some_and(|s| s.contains(dutchman_pilot::prompting::SKETCH_FACT)))
                .count(),
            usize::from(flag != "--no-hints")
        );
        assert_eq!(tools.iter().any(|t| t["name"] == "how"), flag != "--no-how");
        let change =
            texts(&session.call(41, "execute", json!({"command":"solid.box","params":{}})));
        assert_eq!(
            change.contains("design didn't change"),
            flag != "--no-hints"
        );
        if flag != "--no-plan" {
            assert!(
                texts(&session.call(42, "plan", json!({"task":"make a 20 mm cube"})))
                    .contains("solid.box")
            );
        }
    }
    let log = path.parent().unwrap().join("rewrites.jsonl");
    let records: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for record in &records {
        assert_eq!(record["model"], "mcp");
        assert!(record["time"].is_u64());
        assert!(record.get("before").is_some());
        assert!(record.get("after").is_some());
    }
    for kind in [
        "plan",
        "how_answer",
        "tools_list",
        "placement_fact",
        "next_step_hint",
        "paging",
        "suggested_call",
        "command_hint",
    ] {
        assert!(records.iter().any(|r| r["kind"] == kind), "missing {kind}");
    }
    let summary: Value =
        serde_json::from_str(&dutchman_pilot::profile::log_summary(&log, true).unwrap()).unwrap();
    assert_eq!(summary["total"], records.len());
    assert_eq!(summary["by_model"]["mcp"]["total"], records.len());
    // A full log never ends the session: rewrites are withheld, local answers say why.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&log)
        .unwrap()
        .set_len(64 * 1_048_576)
        .unwrap();
    let mut full = Session::start(&path, &["--test-arg"]);
    full.initialize();
    full.send(json!({"jsonrpc":"2.0","id":50,"method":"tools/list"}));
    let tools: Value = serde_json::from_str(&full.read()).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 3);
    let how = full.call(51, "how", json!({"task":"box"}));
    assert!(how["error"]["message"].as_str().unwrap().contains("64 MiB"));
    let unknown = texts(&full.call(52, "execute", json!({"command":"solid.bxo","params":{}})));
    assert!(!unknown.contains("suggested:"));
    assert!(texts(&full.call(53, "count", json!({}))).contains("executions"));
    drop(full);
    remove_dir_when_released(path.parent().unwrap());
}

/// Killing the proxy leaves its fake child exiting on its own; Windows keeps a
/// running executable's directory locked until then.
fn remove_dir_when_released(dir: &std::path::Path) {
    for _ in 0..50 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            Err(e) => panic!("cannot remove {}: {e}", dir.display()),
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
#[ignore = "set SOLIDCRAFT_CLI and run cargo test --test mcp -- --ignored"]
fn real_solidcraft_smoke() {
    let Some(path) = std::env::var_os("SOLIDCRAFT_CLI") else {
        return;
    };
    let mut session = Session::start(&PathBuf::from(path), &[]);
    session.initialize();
    let how = texts(&session.call(1, "how", json!({"task":"make a box"})));
    assert!(how.starts_with('{'));
    assert!(how.contains("solid.box"));
    let result = session.call(
        2,
        "execute",
        json!({"command":"solid.box","params":{"length":10,"width":20,"height":30}}),
    );
    assert_ne!(result["result"]["isError"], true);
    assert!(texts(&result).contains("created_bodies"));
}

#[test]
fn upstream_command_hints_are_not_duplicated() -> Result<(), Box<dyn std::error::Error>> {
    let path = fake();
    for args in [vec!["--test-arg"], vec!["--test-arg", "--jsonrpc-errors"]] {
        let rpc_error = args.contains(&"--jsonrpc-errors");
        let mut session = Session::with_flags(&path, &args, &["--no-plan", "--no-hints"]);
        session.initialize();
        for (index, suffix) in [
            "; closest: `solid.box`",
            "; CLOSEST IDS: solid.box",
            ". Did you mean `solid.box`?",
            ". DID \tYOU\nMEAN: solid.box?",
            ". Did-you-mean: solid.box?",
        ]
        .iter()
        .enumerate()
        {
            let error = format!("unknown command solid.bxo{suffix}");
            let reply = session.call(
                index as u64 + 1,
                "execute",
                json!({"command":"solid.bxo","params":{"error":error}}),
            );
            let expected_call = json!({"tool":"execute","arguments":{
                "command":"solid.box","params":{"error":error}
            }});
            let suggestion =
                format!("suggested: {expected_call}\nSay \"yes\" to do: {expected_call}");
            if rpc_error {
                assert_eq!(reply["error"]["message"], format!("{error}\n{suggestion}"));
            } else {
                assert_eq!(reply["result"]["isError"], true);
                assert_eq!(
                    reply["result"]["content"],
                    json!([
                        {"type":"text","text":error},
                        {"type":"text","text":suggestion}
                    ])
                );
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_commands_without_upstream_hints_keep_pilot_guidance(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = fake();
    let mut session = Session::with_flags(&path, &["--test-arg"], &["--no-plan", "--no-hints"]);
    session.initialize();
    for (index, suffix) in [
        "",
        "; see closest_match in the docs",
        "; did you check the spelling?",
    ]
    .iter()
    .enumerate()
    {
        let error = format!("unknown command solid.bxo{suffix}");
        let reply = session.call(
            index as u64 + 1,
            "execute",
            json!({"command":"solid.bxo","params":{"error":error}}),
        );
        let content = reply["result"]["content"]
            .as_array()
            .ok_or("missing content")?;
        assert_eq!(content.len(), 3);
        assert_eq!(content[0]["text"], error);
        let hint = content[1]["text"].as_str().ok_or("missing hint")?;
        assert!(hint.contains("closest: solid.box"), "{hint}");
        let suggestion = content[2]["text"].as_str().ok_or("missing suggestion")?;
        assert!(suggestion.starts_with("suggested: "));
        let call: Value = serde_json::from_str(
            suggestion
                .lines()
                .next()
                .and_then(|line| line.strip_prefix("suggested: "))
                .ok_or("missing call")?,
        )?;
        assert_eq!(call["arguments"]["command"], "solid.box");
        assert_eq!(call["arguments"]["params"]["error"], error);
    }
    Ok(())
}

#[test]
fn box_placement_hints_on_both_creation_tools_and_opt_out() {
    let path = fake();
    let args = json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}});
    for flags in [vec![], vec!["--no-hints"]] {
        let mut session = Session::with_flags(&path, &["--test-arg"], &flags);
        session.initialize();
        for (name, args) in [
            ("execute", args.clone()),
            ("batch", json!({"commands":[args.clone()]})),
        ] {
            let reply = texts(&session.call(1, name, args));
            assert!(reply.contains("created_bodies"));
            assert_eq!(reply.contains("Body1 spans X 0..20, Y 0..20, Z 0..20 mm; its centre is (10, 10, 10) and its top face centre is (10, 10, 20)."), flags.is_empty(), "{reply}");
        }
    }
    let audit = std::fs::read_to_string(path.parent().unwrap().join("rewrites.jsonl")).unwrap();
    assert!(audit.contains("next_step_hint"));
}

#[test]
fn plane_guidance_is_shared_audited_and_optional() {
    let path = fake();
    for flags in [vec![], vec!["--no-hints"]] {
        let mut s = Session::with_flags(&path, &["--test-arg"], &flags);
        s.initialize();
        s.call(
            1,
            "execute",
            json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}}),
        );
        let plane = texts(&s.call(
            2,
            "execute",
            json!({"command":"sketch.create","params":{"plane":"XZ"}}),
        ));
        assert_eq!(
            plane.contains(dutchman_pilot::prompting::SKETCH_AXES),
            flags.is_empty(),
            "{plane}"
        );
        let circle = texts(&s.call(
            3,
            "execute",
            json!({"command":"sketch.circle.center","params":{"center":[10,10],"radius":4}}),
        ));
        assert_eq!(
            circle.contains("XZ sketch v is -world Z"),
            flags.is_empty(),
            "{circle}"
        );
        let repeated = texts(&s.call(
            4,
            "execute",
            json!({"command":"sketch.circle.center","params":{"center":[10,12],"radius":2}}),
        ));
        assert!(!repeated.contains("XZ sketch v is -world Z"));
        let cleanup=texts(&s.call(5,"execute",json!({"command":"construct.plane.offset","params":{"base":"XY","offset":10,"name":"Unused"}})));
        assert_eq!(
            cleanup.contains("Before finishing, delete construction planes"),
            flags.is_empty(),
            "{cleanup}"
        );
    }
    let audit = std::fs::read_to_string(path.parent().unwrap().join("rewrites.jsonl")).unwrap();
    assert!(audit.contains("sketch_axes_fact"));
    assert!(audit.contains("plane_hint"));
}
