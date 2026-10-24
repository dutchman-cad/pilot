#![cfg(feature = "core")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::solidcraft98::{protocol, screen::Screen};
#[test]
fn framing_encoding_and_caps() {
    assert_eq!(
        protocol::read(&mut &b"a\r\n..b\r\n.\r\n"[..]).unwrap(),
        "a\n.b\n"
    );
    assert_eq!(protocol::decode(&[0x80, 0xe9]), "€é");
    assert_eq!(protocol::encode("€é").unwrap(), vec![0x80, 0xe9]);
    for s in ["a\nb", "a\rb", "a\0b", "🙂"] {
        assert!(protocol::encode(s).is_err());
    }
    assert!(protocol::encode(&"x".repeat(4097)).is_err());
    for bytes in [
        b"no terminator".to_vec(),
        vec![b'x'; 8193],
        b"a\n".repeat(4097),
        b"x\n".repeat(200000),
        [vec![b'x'; 8190], b"\n".to_vec()].concat().repeat(34),
    ] {
        assert!(protocol::read(&mut bytes.as_slice()).is_err());
    }
}
#[test]
fn active_context_duplicates_and_partial_state() {
    let text="SolidCraft 98, 800 x 600.\nMenu bar: File\nDialog \"Numbers\", on top\n  field \"Width (mm)\": [20] (has the focus)\n  button [OK]\n  button [OK]\nDialog \"Under\", under the one above\n  button [Hidden]\nUnderneath: main\nToolbar buttons: Extrude\nTutor: Say \"yes\" to do: key enter\n";
    let s = Screen::parse(text, 5).unwrap();
    assert_eq!(s.observation, 5);
    assert_eq!(s.resolve("width (mm):").unwrap(), "Width (mm)");
    assert!(s.resolve("OK").is_err());
    assert!(s.resolve("File").is_err());
    assert!(s.resolve("Extrude").is_err());
    assert!(s.resolve("Hidden").is_err());
    assert_eq!(s.offer.as_deref(), Some("key enter"));
    assert_eq!(s.nearest("widt (mm)"), ["Width (mm)"]);
    assert!(Screen::parse("SolidCraft 98, 20000 x 600.\n", 0).is_err());
    let s = Screen::parse(
        "SolidCraft 98, 800 x 600.\nToolbar buttons: &Save Part...\n",
        0,
    )
    .unwrap();
    assert_eq!(s.resolve(" save   part…:").unwrap(), "&Save Part...");
}
#[cfg(feature = "proxy")]
mod host {
    use dutchman_pilot::solidcraft98::{
        mcp::{Config, Server},
        transport::{Endpoint, RemoteSession},
    };
    use serde_json::{json, Value};
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        path::PathBuf,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicU64, Ordering},
            OnceLock,
        },
        thread,
        time::Duration,
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "pilot98-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn fake() -> PathBuf {
        static BIN: OnceLock<PathBuf> = OnceLock::new();
        BIN.get_or_init(|| {
            let p = dir().join("fake98");
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
    fn config(path: PathBuf) -> Config {
        Config {
            endpoint: Endpoint::Program {
                path,
                headless: true,
            },
            log: Some(dir().join("audit.jsonl")),
        }
    }
    #[test]
    fn fake_child_typed_calls_offers_errors_repair_and_audit() {
        let cfg = config(fake());
        let log = cfg.log.clone().unwrap();
        let mut s = Server::new(cfg).unwrap();
        assert_eq!(s.call("look", &json!({}))["isError"], false);
        assert_eq!(
            s.call("click", &json!({"name":"new part"}))["isError"],
            false
        );
        for (name, args) in [
            ("click", json!({"name":"New Part","point":{"x":1,"y":2}})),
            ("click", json!({"point":{"x":800,"y":0}})),
            ("menu", json!({"title":"File > New"})),
            ("wait", json!({"ms":5001})),
            ("key", json!({"keys":"kaboom"})),
            ("type", json!({"text":"hi\nquit"})),
            ("quit", json!({})),
            ("I'll extrude it", json!({})),
            ("look", json!({"other":true})),
        ] {
            assert_eq!(s.call(name, &args)["isError"], true, "{name}");
        }
        let v = s.call("click", &json!({"name":"New Prt"}));
        assert_eq!(v["isError"], true);
        assert_eq!(
            v["structuredContent"]["suggested"][0]["arguments"]["name"],
            "New Part"
        );
        let v = s.call("menu", &json!({"title":"File","item":"Bad"}));
        assert_eq!(v["isError"], true);
        assert!(v["structuredContent"]["suggested"].is_object());
        assert_eq!(s.call("yes", &json!({}))["isError"], false);
        let v = s.call("how", &json!({"task":"rename body"}));
        assert!(v.to_string().contains("cannot rename"));
        assert!(v.to_string().contains(".dot"));
        assert_eq!(
            s.call("dblclick", &json!("```json\n{name:'New Part',}\n```"))["isError"],
            false
        );
        let records = std::fs::read_to_string(log).unwrap();
        assert!(records.contains("\"kind\":\"name\""));
        assert!(records.contains("\"kind\":\"syntax\""));
        assert!(records.contains("\"kind\":\"alias\""));
        assert!(records.contains("observation"));
    }
    #[test]
    fn tcp_fragmented_unknown_outcome_reconnect_never_replays() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = thread::spawn(move || {
            let (a, _) = listener.accept().unwrap();
            let mut r = BufReader::new(a);
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            assert_eq!(line, "type 20\n");
            r.get_mut().write_all(b"half\n").unwrap();
            drop(r);
            let (a, _) = listener.accept().unwrap();
            let mut r = BufReader::new(a);
            line.clear();
            r.read_line(&mut line).unwrap();
            assert_eq!(line, "look\n");
            for b in b"SolidCraft 98, 800 x 600.\r\n..dot\r\n.\r\n" {
                r.get_mut().write_all(&[*b]).unwrap();
            }
        });
        let mut session = RemoteSession::new(Endpoint::Tcp(address));
        assert!(session
            .exchange("type 20")
            .unwrap_err()
            .contains("Outcome unknown"));
        assert!(session.exchange("look").unwrap().contains(".dot"));
        peer.join().unwrap();
    }
    #[test]
    fn pipe_eof_no_respawn_and_command_budget() {
        let path = dir().join("fake-eof");
        std::fs::copy(fake(), &path).unwrap();
        let mut session = RemoteSession::new(Endpoint::Program {
            path,
            headless: true,
        });
        assert!(session.exchange("look").is_err());
        assert!(session
            .exchange("look")
            .unwrap_err()
            .contains("explicit new session"));
        let mut session = RemoteSession::new(Endpoint::Program {
            path: fake(),
            headless: true,
        });
        for _ in 0..160 {
            session.exchange("look").unwrap();
        }
        assert!(session.exchange("look").unwrap_err().contains("budget"));
    }
    #[test]
    fn audit_failure_prevents_name_rewrite_dispatch() {
        let mut cfg = config(fake());
        cfg.log = Some(dir().join("large-log"));
        let file = std::fs::File::create(cfg.log.as_ref().unwrap()).unwrap();
        file.set_len(64 * 1048576).unwrap();
        let mut s = Server::new(cfg).unwrap();
        let v = s.call("click", &json!({"name":"new part"}));
        assert_eq!(v["isError"], true);
        assert!(v.to_string().contains("64 MiB"));
    }
    #[test]
    fn offers_stale_bounds_repeated_state_and_turn_budget() {
        let mut s = Server::new(config(fake())).unwrap();
        assert_eq!(s.call("look", &json!({}))["isError"], false);
        assert_eq!(s.call("yes", &json!({}))["isError"], false);
        assert_eq!(s.call("yes", &json!({}))["isError"], true);
        for _ in 0..2 {
            assert_eq!(s.call("wait", &json!({"ms":0}))["isError"], false);
        }
        assert!(s
            .call("wait", &json!({"ms":0}))
            .to_string()
            .contains("three identical"));
        assert!(s.call("look", &json!({})).to_string().contains("40 action"));
        let path = dir().join("stale");
        std::fs::copy(fake(), &path).unwrap();
        let mut s = Server::new(config(path)).unwrap();
        s.call("look", &json!({}));
        assert!(s
            .call("click", &json!({"point":{"x":200,"y":1}}))
            .to_string()
            .contains("outside current"));
        let mut s = Server::new(config(fake())).unwrap();
        for _ in 0..40 {
            assert_eq!(s.call("unknown", &json!({}))["isError"], true);
        }
        assert!(s.call("look", &json!({})).to_string().contains("40 action"));
    }
    #[test]
    fn slow_pipe_and_blocked_pipe_deadlines() {
        for mode in ["hang", "blocked"] {
            let path = dir().join(mode);
            std::fs::copy(fake(), &path).unwrap();
            let start = std::time::Instant::now();
            let mut session = RemoteSession::new(Endpoint::Program {
                path,
                headless: true,
            });
            assert!(session.exchange("look").is_err());
            assert!(start.elapsed() < Duration::from_secs(19));
        }
    }
    struct Mcp {
        child: std::process::Child,
        input: Option<std::process::ChildStdin>,
        output: BufReader<std::process::ChildStdout>,
        id: u64,
    }
    impl Mcp {
        fn start(path: PathBuf) -> Self {
            let mut child = Command::new(env!("CARGO_BIN_EXE_dutchman-pilot"))
                .args(["mcp98", "--program"])
                .arg(path)
                .args(["--headless", "--log"])
                .arg(dir().join("mcp.jsonl"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            Self {
                input: child.stdin.take(),
                output: BufReader::new(child.stdout.take().unwrap()),
                child,
                id: 0,
            }
        }
        fn request(&mut self, method: &str, params: Value) -> Value {
            self.id += 1;
            let input = self.input.as_mut().unwrap();
            writeln!(
                input,
                "{}",
                json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
            )
            .unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            self.output.read_line(&mut line).unwrap();
            let v: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(v["id"], self.id);
            v["result"].clone()
        }
        fn call(&mut self, name: &str, args: Value) -> Value {
            self.request("tools/call", json!({"name":name,"arguments":args}))
        }
        fn ok(&mut self, name: &str, args: Value) -> Value {
            let v = self.call(name, args);
            assert_eq!(v["isError"], false, "{v}");
            v
        }
    }
    impl Drop for Mcp {
        fn drop(&mut self) {
            self.input.take();
            let end = std::time::Instant::now() + Duration::from_secs(4);
            while std::time::Instant::now() < end {
                if self.child.try_wait().unwrap().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            self.child.kill().unwrap();
            self.child.wait().unwrap();
        }
    }
    #[test]
    fn mcp_stdio_lifecycle_and_schemas() {
        let mut m = Mcp::start(fake());
        assert!(m.request("initialize", json!({}))["capabilities"]["tools"].is_object());
        let tools = m.request("tools/list", json!({}));
        assert_eq!(tools["tools"].as_array().unwrap().len(), 11);
        assert!(!tools.to_string().contains("quit"));
        m.ok("look", json!({}));
    }
    #[test]
    fn cli_requires_exactly_one_transport_and_program_for_headless() {
        let binary = env!("CARGO_BIN_EXE_dutchman-pilot");
        for args in [
            vec!["mcp98"],
            vec![
                "mcp98",
                "--connect",
                "127.0.0.1:9898",
                "--program",
                "somewhere",
            ],
            vec!["mcp98", "--connect", "127.0.0.1:9898", "--headless"],
        ] {
            assert!(!Command::new(binary)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        }
        let help = Command::new(binary)
            .args(["mcp98", "--help"])
            .output()
            .unwrap();
        assert!(help.status.success());
        assert!(String::from_utf8(help.stdout)
            .unwrap()
            .contains("SolidCraft 98"));
    }
    #[test]
    #[ignore = "requires SC98_PROGRAM pointing to a verified FreeBASIC --remote build"]
    fn real_freebasic_task_one_through_mcp98() {
        let mut m = Mcp::start(PathBuf::from(
            std::env::var_os("SC98_PROGRAM").expect("set SC98_PROGRAM"),
        ));
        m.ok("look", json!({}));
        m.ok("menu", json!({"title":"File","item":"New"}));
        m.ok("menu", json!({"title":"Sketch","item":"Rectangle..."}));
        m.ok("type", json!({"text":"20"}));
        m.ok("key", json!({"keys":"tab"}));
        m.ok("type", json!({"text":"20"}));
        m.ok("key", json!({"keys":"enter"}));
        m.ok("menu", json!({"title":"Features","item":"Extrude..."}));
        m.ok("type", json!({"text":"20"}));
        m.ok("key", json!({"keys":"enter"}));
        let v = m.ok("look", json!({}));
        let timeline = v["structuredContent"]["screen"]["timeline"].to_string();
        assert!(timeline.contains("20 x 20 mm"), "{v}");
        assert!(
            timeline.contains("Extrude1") && timeline.contains("20 mm"),
            "{v}"
        );
    }
}
