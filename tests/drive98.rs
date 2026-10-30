#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::solidcraft98::{
    drive::{self, Call, Config},
    transport::Endpoint,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "drive98-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn fake() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let p = dir().join("remote98");
        assert!(Command::new("rustc")
            .args([
                "--edition=2021",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/support/fake_remote98.rs"
                ),
                "-o"
            ])
            .arg(&p)
            .status()
            .unwrap()
            .success());
        p
    })
    .clone()
}
struct Upstream {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Upstream {
    fn new(replies: Vec<Value>) -> Self {
        let s = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", s.server_addr());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let r = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let quit = stop.clone();
        let t = thread::spawn(move || {
            let mut replies = VecDeque::from(replies);
            while !quit.load(Ordering::Relaxed) {
                if let Some(mut req) = s.recv_timeout(Duration::from_millis(50)).unwrap() {
                    assert_eq!(req.url(), "/v1/chat/completions");
                    let mut text = String::new();
                    req.as_reader().read_to_string(&mut text).unwrap();
                    let v: Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(v["tools"].as_array().unwrap().len(), 12);
                    assert!(!v["tools"].to_string().contains("quit"));
                    r.lock().unwrap().push(v);
                    let message=replies.pop_front().unwrap_or(json!({"role":"assistant","content":"COMMAND: done no more scripted replies"}));
                    let body =
                        json!({"choices":[{"message":message}],"usage":{"total_tokens":100}});
                    let _ = req.respond(
                        tiny_http::Response::from_string(body.to_string()).with_header(
                            tiny_http::Header::from_bytes("Content-Type", "application/json")
                                .unwrap(),
                        ),
                    );
                }
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(t),
        }
    }
    fn cfg(&self, plain: bool) -> Config {
        Config {
            endpoint: Endpoint::Program {
                path: fake(),
                headless: true,
            },
            upstream: self.url.clone(),
            model: "scripted-fake".into(),
            task: "make a cube".into(),
            plain,
            log: Some(dir().join("log.jsonl")),
            budget: 40,
            wall: Duration::from_secs(30),
            tokens: 131072,
        }
    }
}
impl Drop for Upstream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn text(s: &str) -> Value {
    json!({"role":"assistant","content":s})
}
fn native(name: &str, args: &str) -> Value {
    json!({"role":"assistant","content":null,"tool_calls":[{"id":"call1","type":"function","function":{"name":name,"arguments":args}}]})
}
#[test]
fn strict_text_and_explicit_objects_only() {
    for plain in [true, false] {
        for s in [
            "I'll click New Part",
            "click New Part",
            "COMMAND: click New Part\nCOMMAND: yes",
            "COMMAND: quit",
            "COMMAND: yes\nprose",
            "{\"tool\":\"look\"} {\"tool\":\"yes\"}",
            "prose {\"tool\":\"look\"}",
            "COMMAND: look\nTHOUGHT: other",
        ] {
            assert!(drive::parse_text(s, plain).is_err(), "{s}");
        }
        assert_eq!(
            drive::parse_text("THOUGHT: I'll inspect.\nCOMMAND: look", plain).unwrap(),
            Call::Tool {
                name: "look".into(),
                args: json!({})
            }
        );
        assert_eq!(
            drive::parse_text("COMMAND: done finished", plain).unwrap(),
            Call::Done("finished".into())
        );
        for s in [
            r#"{"tool":"click","args":{"name":"New Part"}}"#,
            r#"{"name":"click","arguments":{"name":"New Part"}}"#,
            r#"{"function":{"name":"click","arguments":{"name":"New Part"}}}"#,
            r#"{"click":{"name":"New Part"}}"#,
        ] {
            assert!(drive::parse_text(s, plain).is_ok(), "{s}");
        }
    }
    let gemma = r#"fill_dialog{fields:[{name:<|"|>Corner radius (mm)<|"|>,value:<|"|>3<|"|>}]}"#;
    assert_eq!(
        drive::parse_text(gemma, false).unwrap(),
        Call::Tool {
            name: "fill_dialog".into(),
            args: json!({"fields":[{"name":"Corner radius (mm)","value":"3"}]})
        }
    );
    assert!(drive::parse_text(gemma, true).is_err());
    assert!(drive::parse_text("I will click{name:x}", false).is_err());
    assert_eq!(
        drive::parse_text("doubleclick Sketch1", false).unwrap_err(),
        "a command needs its prefix and did not run; reply exactly: COMMAND: doubleclick Sketch1"
    );
    assert!(drive::parse_text("doubleclick Sketch1", true)
        .unwrap_err()
        .starts_with("expected one COMMAND: line"));
    assert!(drive::parse_text("I'll click New Part", false)
        .unwrap_err()
        .starts_with("expected one COMMAND: line"));
    assert!(drive::parse_text("```json\n{tool:'look',args:{},}\n```", false).is_ok());
    assert!(drive::parse_text("```json\n{tool:'look',args:{},}\n```", true).is_err());
    assert_eq!(
        drive::parse_text("COMMAND: type  hello  ", false).unwrap(),
        Call::Tool {
            name: "type".into(),
            args: json!({"text":" hello  "})
        }
    );
    let multi = json!({"tool_calls":[{},{}]});
    assert!(drive::parse_message(&multi, false).is_err());
}
#[test]
fn double_and_right_clicks_use_the_verbs_both_builds_know() {
    let u = Upstream::new(vec![
        native("doubleclick", r#"{"point":{"x":200,"y":300}}"#),
        native("rightclick", r#"{"point":{"x":200,"y":300}}"#),
        text("COMMAND: done finished"),
    ]);
    let result = drive::run(u.cfg(false)).unwrap();
    assert_eq!(result["stop_reason"], "done");
    assert_eq!(result["actions"], 2);
    assert_eq!(result["errors"], 0, "{result}");
}
#[test]
fn loop_format_errors_native_done_and_transcript() {
    let u = Upstream::new(vec![
        text("I will click New Part."),
        native("click", r#"{"name":"New Part"}"#),
        text("COMMAND: done finished"),
    ]);
    let cfg = u.cfg(false);
    let path = cfg.log.clone().unwrap();
    let result = drive::run(cfg).unwrap();
    assert_eq!(result["stop_reason"], "done");
    assert_eq!(result["steps"], 3);
    assert_eq!(result["actions"], 1);
    assert_eq!(result["errors"], 1);
    assert_eq!(result["native"], 1);
    assert_eq!(result["done_message"], "finished");
    assert!(result["final_screen_text"]
        .as_str()
        .unwrap()
        .contains("SolidCraft 98"));
    let records = std::fs::read_to_string(path).unwrap();
    for kind in [
        "request",
        "response",
        "transcript",
        "summary",
        "prompt",
        "help",
        "hand_back",
    ] {
        assert!(records.contains(&format!("\"kind\":\"{kind}\"")), "{kind}");
    }
    let requests = u.requests.lock().unwrap();
    assert!(requests[1]["messages"].to_string().contains("formatError"));
    assert!(requests[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["role"] == "tool"));
}
#[test]
fn plain_disables_all_additions_and_every_default_rewrite_is_audited() {
    for plain in [true, false] {
        let u = Upstream::new(vec![
            text(r#"{"tool":"click","args":{"name":"new part"}}"#),
            native("dblclick", "{name:'New Part',}"),
            text("COMMAND: done finished"),
        ]);
        let cfg = u.cfg(plain);
        let log = cfg.log.clone().unwrap();
        let result = drive::run(cfg).unwrap();
        let records = std::fs::read_to_string(log).unwrap();
        let request = u.requests.lock().unwrap()[0].clone();
        let prompt = request["messages"][0]["content"].as_str().unwrap();
        assert!(prompt.starts_with(drive::SYSTEM));
        if plain {
            assert_eq!(prompt, drive::SYSTEM);
            assert_eq!(result["rewrites"], 0);
            assert!(!records.contains("\"kind\":\"name\""));
            assert!(!records.contains("pilotHelp"));
            assert!(!records.contains("\"kind\":\"syntax\""));
        } else {
            assert!(result["rewrites"].as_u64().unwrap() > 0);
            for kind in ["name", "alias", "syntax", "text_call"] {
                assert!(records.contains(&format!("\"kind\":\"{kind}\"")), "{kind}");
            }
        }
    }
}
#[test]
fn budgets_repeated_results_and_bounded_format_recovery() {
    let u = Upstream::new(vec![text("COMMAND: wait 0"); 4]);
    let mut c = u.cfg(true);
    c.budget = 2;
    assert_eq!(drive::run(c).unwrap()["stop_reason"], "step_budget");
    let u = Upstream::new(vec![text("COMMAND: wait 0"); 4]);
    assert_eq!(
        drive::run(u.cfg(false)).unwrap()["stop_reason"],
        "repeated_result"
    );
    let u = Upstream::new(vec![]);
    let mut c = u.cfg(false);
    c.tokens = 1;
    let r = drive::run(c).unwrap();
    assert_eq!(r["stop_reason"], "token_budget");
    assert_eq!(r["steps"], 0);
    let u = Upstream::new(vec![]);
    let mut c = u.cfg(false);
    c.wall = Duration::from_nanos(1);
    assert_eq!(drive::run(c).unwrap()["stop_reason"], "wall_budget");
    let u = Upstream::new(vec![text(""); 3]);
    let r = drive::run(u.cfg(false)).unwrap();
    assert_eq!(r["stop_reason"], "empty_reply");
    assert_eq!(r["retries"], 1);
    let u = Upstream::new(vec![text("bad"), text("different prose"), text("unknown")]);
    let r = drive::run(u.cfg(false)).unwrap();
    assert!(r["steps"].as_u64().unwrap() <= 3);
    assert!(r["hand_backs"].as_u64().unwrap() <= 2);
}
#[test]
#[ignore = "requires SC98_PROGRAM, uses only a scripted fake upstream"]
fn real_freebasic_task_one_through_drive98() {
    let commands = [
        "menu File > New",
        "menu Sketch > Rectangle...",
        "type 20",
        "key tab",
        "type 20",
        "key enter",
        "menu Features > Extrude...",
        "type 20",
        "key enter",
        "done made a 20 mm cube",
    ];
    let u = Upstream::new(
        commands
            .into_iter()
            .map(|c| text(&format!("COMMAND: {c}")))
            .collect(),
    );
    let mut cfg = u.cfg(false);
    cfg.endpoint = Endpoint::Program {
        path: std::env::var_os("SC98_PROGRAM")
            .expect("set SC98_PROGRAM")
            .into(),
        headless: true,
    };
    cfg.task = "Make a 20 mm cube.".into();
    let r = drive::run(cfg).unwrap();
    assert_eq!(r["stop_reason"], "done");
    assert_eq!(r["errors"], 0, "{r}");
    let screen = r["final_screen_text"].as_str().unwrap();
    assert!(
        screen.contains("20 x 20 mm") && screen.contains("Extrude1 | Extrude | Sketch1, 20 mm"),
        "{screen}"
    );
}

#[test]
fn replayed_completions_over_real_session_without_network() {
    for plain in [true, false] {
        let mut replies = VecDeque::from(vec![
            text("I will click."),
            native("click", r#"{"name":"new part"}"#),
            native("dblclick", "{name:'New Part',}"),
            text("COMMAND: done finished"),
        ]);
        let log = dir().join("audit.jsonl");
        let cfg = Config {
            endpoint: Endpoint::Program {
                path: fake(),
                headless: true,
            },
            upstream: "http://127.0.0.1:1/v1".into(),
            model: "scripted-fake".into(),
            task: "make a cube".into(),
            plain,
            log: Some(log.clone()),
            budget: 40,
            wall: Duration::from_secs(30),
            tokens: 131072,
        };
        let mut requests = Vec::new();
        let r=drive::run_with(cfg,|_,body,_| {requests.push(body.clone());Ok(json!({"choices":[{"message":replies.pop_front().unwrap()}],"usage":{"total_tokens":100}}))}).unwrap();
        assert_eq!(r["stop_reason"], "done", "{r}");
        assert_eq!(r["steps"], 4);
        assert_eq!(r["native"], if plain { 1 } else { 2 });
        assert!(r["final_screen_text"]
            .as_str()
            .unwrap()
            .contains("SolidCraft 98"));
        let records = std::fs::read_to_string(log).unwrap();
        if plain {
            assert_eq!(r["rewrites"], 0);
            assert_eq!(requests[0]["messages"][0]["content"], drive::SYSTEM);
            assert!(!records.contains("pilotHelp"));
        } else {
            for k in ["name", "alias", "syntax", "prompt", "help", "hand_back"] {
                assert!(records.contains(&format!("\"kind\":\"{k}\"")), "{k}");
            }
        }
    }
}
#[test]
fn replay_budget_and_repeat_guards_without_network() {
    for (budget, tokens, wall, reply, expected) in [
        (
            2,
            131072,
            Duration::from_secs(30),
            "COMMAND: wait 0",
            "step_budget",
        ),
        (
            40,
            131072,
            Duration::from_secs(30),
            "COMMAND: wait 0",
            "repeated_result",
        ),
        (
            40,
            1,
            Duration::from_secs(30),
            "COMMAND: wait 0",
            "token_budget",
        ),
        (
            40,
            131072,
            Duration::from_nanos(1),
            "COMMAND: wait 0",
            "wall_budget",
        ),
        (40, 131072, Duration::from_secs(30), "", "empty_reply"),
    ] {
        let cfg = Config {
            endpoint: Endpoint::Program {
                path: fake(),
                headless: true,
            },
            upstream: "http://127.0.0.1:1/v1".into(),
            model: "fake".into(),
            task: "cube".into(),
            plain: false,
            log: Some(dir().join("audit.jsonl")),
            budget,
            wall,
            tokens,
        };
        let r = drive::run_with(cfg, |_, _, _| {
            Ok(json!({"choices":[{"message":text(reply)}],"usage":{"total_tokens":100}}))
        })
        .unwrap();
        assert_eq!(r["stop_reason"], expected, "{r}");
    }
}

#[test]
fn cli_offline_help_and_driver_bounds() {
    let binary = env!("CARGO_BIN_EXE_dutchman-pilot");
    for command in ["drive98", "how98", "model"] {
        let out = Command::new(binary)
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .contains("SolidCraft 98"));
    }
    let out = Command::new(binary)
        .args(["how98", "move the box along X"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["examples"][0]["name"], "doubleclick");
    let out = Command::new(binary)
        .args(["how98", "round the four upright edges with a 3 mm radius"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["examples"][0]["arguments"]["name"], "Sketch1");
    assert!(v["limitation"].as_str().unwrap().contains("Corner radius"));
    for args in [
        vec![
            "drive98",
            "--upstream",
            "http://127.0.0.1:1/v1",
            "--model",
            "fake",
            "task",
        ],
        vec![
            "drive98",
            "--connect",
            "127.0.0.1:1",
            "--headless",
            "--upstream",
            "http://127.0.0.1:1/v1",
            "--model",
            "fake",
            "task",
        ],
        vec![
            "drive98",
            "--program",
            "unused",
            "--upstream",
            "http://127.0.0.1:1/v1",
            "--model",
            "fake",
            "--budget",
            "201",
            "task",
        ],
        vec!["model", "--dialect", "unknown"],
    ] {
        assert!(!Command::new(binary)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
}

// Public benchmark screens only. This peer checks the exact emitted wire sequence;
// it never starts or connects to the shared application.
fn replay_peer(
    initial: String,
    transitions: Vec<(String, String)>,
) -> (Endpoint, thread::JoinHandle<()>) {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = Endpoint::Tcp(listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        // Only a guard against a hung test: a loaded CI runner can take over ten seconds
        // between commands.
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut screen = initial;
        let mut transitions = VecDeque::from(transitions);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() != 0 {
            let command = line.trim_end();
            let answer = if command == "look" {
                screen.clone()
            } else {
                let (expected, next) = transitions.pop_front().expect("unexpected command");
                assert_eq!(command, expected);
                screen = next;
                "ok\n".into()
            };
            for l in answer.lines() {
                writeln!(stream, "{}{l}", if l.starts_with('.') { "." } else { "" }).unwrap();
            }
            writeln!(stream, ".").unwrap();
            stream.flush().unwrap();
            line.clear();
        }
        assert!(transitions.is_empty(), "missing commands: {transitions:?}");
    });
    (endpoint, handle)
}
fn public_fixture(name: &str) -> Value {
    serde_json::from_str(match name {
        "gemma" => include_str!("fixtures/sc98-public/gemma-cube.json"),
        "direct" => include_str!("fixtures/sc98-public/gemma-cube-direct.json"),
        "muse" => include_str!("fixtures/sc98-public/muse-cube.json"),
        _ => panic!("unknown fixture"),
    })
    .unwrap()
}
fn recorded_screen(fixture: &Value, step: usize) -> String {
    fixture["steps"][step]["screen"].as_str().unwrap().into()
}
#[test]
fn public_cube_dialog_batch_spends_two_actions_instead_of_eleven() {
    let fixture = public_fixture("gemma");
    let direct = public_fixture("direct");
    let dialog = recorded_screen(&fixture, 0);
    let sketch = recorded_screen(&fixture, 10);
    let extrude = recorded_screen(&direct, 6);
    let cube = recorded_screen(&direct, 7);
    let mut transitions = vec![("menu Sketch > Rectangle...".into(), dialog.clone())];
    for (name, value) in [("Width (mm)", "20"), ("Height (mm)", "20")] {
        transitions.extend([
            (format!("click {name}"), dialog.clone()),
            ("key tab shift+tab".into(), dialog.clone()),
            (format!("type {value}"), dialog.clone()),
        ]);
    }
    transitions.extend([
        ("click OK".into(), sketch),
        ("menu Features > Extrude".into(), extrude.clone()),
        ("click Depth (mm)".into(), extrude.clone()),
        ("key tab shift+tab".into(), extrude.clone()),
        ("type 20".into(), extrude),
        ("click OK".into(), cube.clone()),
    ]);
    let (endpoint, peer) = replay_peer(
        fixture["initial_screen"].as_str().unwrap().into(),
        transitions,
    );
    let u = Upstream::new(vec![
        fixture["steps"][0]["message"].clone(),
        native(
            "fill_dialog",
            r#"{"fields":[{"name":"Width (mm)","value":"20"},{"name":"Height (mm)","value":"20"}]}"#,
        ),
        direct["steps"][6]["message"].clone(),
        native(
            "fill_dialog",
            r#"{"fields":[{"name":"Depth (mm)","value":"20"}]}"#,
        ),
        text("COMMAND: done cube created"),
    ]);
    let mut cfg = u.cfg(false);
    cfg.endpoint = endpoint;
    cfg.budget = 5;
    let log = cfg.log.clone().unwrap();
    let r = drive::run(cfg).unwrap();
    peer.join().unwrap();
    assert_eq!(r["stop_reason"], "done");
    assert_eq!(r["actions"], 4); // two for the entire rectangle dialog, two for extrusion
    assert_eq!(r["errors"], 0);
    assert_eq!(r["final_screen_text"], cube);
    assert_eq!(fixture["summary"]["actions"], 11); // old run stopped before extrusion
    let records = std::fs::read_to_string(log).unwrap();
    assert_eq!(
        records
            .lines()
            .filter(|l| l.contains("\"kind\":\"prompt\""))
            .count(),
        1
    );
    let requests = u.requests.lock().unwrap();
    for req in requests.iter() {
        assert_eq!(
            req["messages"]
                .to_string()
                .matches("SolidCraft 98 dialogs:")
                .count(),
            1
        );
        assert_eq!(
            req["messages"]
                .to_string()
                .matches("SolidCraft 98, ")
                .count(),
            1
        );
    }
}
#[test]
fn public_muse_key_repair_and_exit_recovery_do_not_spend_model_actions() {
    let fixture = public_fixture("muse");
    let dialog = recorded_screen(&fixture, 2);
    let typed = recorded_screen(&fixture, 3);
    let height = recorded_screen(&fixture, 5);
    let initial = fixture["initial_screen"].as_str().unwrap().to_owned();
    let (endpoint, peer) = replay_peer(
        initial.clone(),
        vec![
            ("click Sketch Rectangle".into(), dialog),
            ("type 20".into(), typed),
            ("key tab".into(), height),
            ("key esc".into(), initial.clone()),
        ],
    );
    let u = Upstream::new(vec![
        fixture["steps"][2]["message"].clone(),
        fixture["steps"][3]["message"].clone(),
        fixture["steps"][4]["message"].clone(), // Tab was formerly rejected
        text("COMMAND: done stopping"),
    ]);
    let mut cfg = u.cfg(false);
    cfg.endpoint = endpoint;
    let r = drive::run(cfg).unwrap();
    peer.join().unwrap();
    assert_eq!(r["actions"], 3);
    assert_eq!(r["errors"], 0);
    assert_eq!(r["dialog_cancelled"], true);
    assert_eq!(r["final_screen_text"], initial);
    assert!(r["done_message"].as_str().unwrap().contains("not applied"));
}

#[test]
fn invalid_batch_validates_every_field_before_editing_and_recovers_before_handback() {
    for rejected in [
        native(
            "fill_dialog",
            r#"{"fields":[{"name":"Width (mm)","value":"20"},{"name":"Missing","value":"20"}]}"#,
        ),
        native("menu", r#"{"title":"Features","item":"Extrude"}"#),
        native("click", r#"{"name":"Height (mm)","point":{"x":0,"y":0}}"#),
    ] {
        let fixture = public_fixture("gemma");
        let initial = fixture["initial_screen"].as_str().unwrap().to_owned();
        let (endpoint, peer) = replay_peer(
            initial.clone(),
            vec![
                (
                    "menu Sketch > Rectangle...".into(),
                    recorded_screen(&fixture, 0),
                ),
                ("key esc".into(), initial.clone()),
            ],
        );
        let u = Upstream::new(vec![
            fixture["steps"][0]["message"].clone(),
            rejected,
            text("COMMAND: done stopped"),
        ]);
        let mut cfg = u.cfg(false);
        cfg.endpoint = endpoint;
        let r = drive::run(cfg).unwrap();
        peer.join().unwrap();
        assert_eq!(r["actions"], 2);
        assert_eq!(r["errors"], 1);
        let requests = u.requests.lock().unwrap();
        let last = requests.last().unwrap()["messages"].to_string();
        assert!(last.contains("Unfinished dialog cancelled"));
        assert!(!last.contains(
            "SCREEN:\\nSolidCraft 98, 800 x 600. Turbo off (the LED says 66 MHz).\\nDialog"
        ));
        assert_eq!(r["final_screen_text"], initial);
    }
}
#[test]
fn format_repair_requests_do_not_spend_the_model_action_budget() {
    let u = Upstream::new(vec![text("not a call"), text("COMMAND: wait 0")]);
    let mut cfg = u.cfg(false);
    cfg.budget = 1;
    let r = drive::run(cfg).unwrap();
    assert_eq!(r["steps"], 2);
    assert_eq!(r["actions"], 1);
    assert_eq!(r["hand_backs"], 1);
}
