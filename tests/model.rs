#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::model::{Config, FallbackPrompt, Server};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Harness {
    url: String,
    dir: PathBuf,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    remaining: Arc<Mutex<VecDeque<(u16, String, String)>>>,
    stop: Arc<AtomicBool>,
    upstream: Option<thread::JoinHandle<()>>,
    proxy: Option<thread::JoinHandle<()>>,
}
impl Harness {
    fn new(
        replies: Vec<(u16, String, String)>,
        count: usize,
        fallback: FallbackPrompt,
        settings: Option<Value>,
        capabilities: Value,
    ) -> Self {
        Self::with_how(replies, count, fallback, settings, capabilities, false)
    }
    fn with_how(
        replies: Vec<(u16, String, String)>,
        count: usize,
        fallback: FallbackPrompt,
        settings: Option<Value>,
        capabilities: Value,
        how: bool,
    ) -> Self {
        Self::with_direct(replies, count, fallback, settings, capabilities, how, false)
    }
    fn with_direct(
        replies: Vec<(u16, String, String)>,
        count: usize,
        fallback: FallbackPrompt,
        settings: Option<Value>,
        capabilities: Value,
        how: bool,
        direct: bool,
    ) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dutchman-pilot-model-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let settings_path = settings.map(|v| {
            let p = dir.join("settings.json");
            fs::write(&p, v.to_string()).unwrap();
            p
        });
        let upstream = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let upstream_url = format!("http://{}/v1", upstream.server_addr());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let remaining = Arc::new(Mutex::new(VecDeque::from(replies)));
        let stop = Arc::new(AtomicBool::new(false));
        let (record, queue, quit) = (requests.clone(), remaining.clone(), stop.clone());
        let upstream_thread = thread::spawn(move || {
            while !quit.load(Ordering::Relaxed) {
                if let Some(mut req) = upstream.recv_timeout(Duration::from_millis(100)).unwrap() {
                    assert!(
                        !direct,
                        "direct turn called upstream, including capability probe"
                    );
                    assert_eq!(
                        req.headers()
                            .iter()
                            .find(|h| h.field.equiv("Authorization"))
                            .map(|h| h.value.as_str()),
                        Some("Bearer test-secret")
                    );
                    let path = req.url().to_string();
                    let mut text = String::new();
                    req.as_reader().read_to_string(&mut text).unwrap();
                    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
                    record.lock().unwrap().push((path.clone(), body));
                    let (status, text, content_type) = if path == "/api/show" {
                        (200, capabilities.to_string(), "application/json".into())
                    } else {
                        queue
                            .lock()
                            .unwrap()
                            .pop_front()
                            .expect("unexpected upstream request")
                    };
                    let response = tiny_http::Response::from_string(text)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes("Content-Type", content_type).unwrap(),
                        )
                        .with_header(tiny_http::Header::from_bytes("Retry-After", "7").unwrap());
                    req.respond(response).unwrap();
                }
            }
        });
        let mut proxy = Server::bind(Config {
            direct_recipes: direct,
            no_report_buffer: false,
            listen: "127.0.0.1:0".parse().unwrap(),
            upstream: upstream_url,
            upstream_timeout: Duration::from_secs(300),
            fallback_prompt: fallback,
            no_how: !how,
            no_hints: false,
            planner: true,
            log: Some(dir.join("audit.jsonl")),
            profile_dir: Some(dir.clone()),
            model_settings: settings_path,
            model_settings_enabled: false,
            seed: None,
            prompt_help: false,
        })
        .unwrap();
        let url = format!("http://{}", proxy.local_addr().unwrap());
        let proxy_thread = thread::spawn(move || {
            for _ in 0..count {
                proxy.serve_one().unwrap();
            }
        });
        Self {
            url,
            dir,
            requests,
            remaining,
            stop,
            upstream: Some(upstream_thread),
            proxy: Some(proxy_thread),
        }
    }
    fn chat(&self, body: &Value) -> (u16, String, String) {
        let result = ureq::post(&format!("{}/v1/chat/completions", self.url))
            .timeout(Duration::from_secs(8))
            .set("Authorization", "Bearer test-secret")
            .set("Content-Type", "application/json")
            .send_string(&body.to_string());
        let response = match result {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => panic!("{e}"),
        };
        let status = response.status();
        let header = response.header("x-dutchman-pilot").unwrap().to_string();
        (status, header, response.into_string().unwrap())
    }
    fn chats(&self) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p == "/v1/chat/completions")
            .map(|(_, v)| v.clone())
            .collect()
    }
    fn finish(mut self) {
        self.proxy.take().unwrap().join().unwrap();
        self.stop.store(true, Ordering::Relaxed);
        self.upstream.take().unwrap().join().unwrap();
        assert!(self.remaining.lock().unwrap().is_empty());
        for file in ["profiles.json", "audit.jsonl"] {
            if self.dir.join(file).exists() {
                assert!(!fs::read_to_string(self.dir.join(file))
                    .unwrap()
                    .contains("test-secret"));
            }
        }
        fs::remove_dir_all(&self.dir).unwrap();
    }
}
fn tools() -> Value {
    json!([
        {"type":"function","function":{"name":"execute","parameters":{"type":"object","properties":{"command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}},
        {"type":"function","function":{"name":"batch","parameters":{"type":"object","properties":{"commands":{"type":"array"}},"required":["commands"]}}},
        {"type":"function","function":{"name":"inspect","parameters":{"type":"object","properties":{}}}}
    ])
}
fn request(text: &str) -> Value {
    json!({"model":"test-model","messages":[{"role":"user","content":text}],"tools":tools()})
}
fn reply(message: Value) -> Value {
    json!({"id":"turn-123","object":"chat.completion","model":"test-model","created":123,"choices":[{"index":0,"message":message,"finish_reason":if message["tool_calls"].is_array() {"tool_calls"} else {"stop"}}],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}})
}
fn prose(text: &str) -> Value {
    reply(json!({"role":"assistant","content":text}))
}
fn call(name: &str, args: Value) -> Value {
    reply(
        json!({"role":"assistant","content":null,"tool_calls":[{"id":"native-id","type":"function","function":{"name":name,"arguments":args.to_string()}}]}),
    )
}
fn ok(value: Value) -> (u16, String, String) {
    (200, value.to_string(), "application/json".into())
}
fn args(response: &str) -> Value {
    let response: Value = serde_json::from_str(response).unwrap();
    serde_json::from_str(
        response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn parse_error(raw: &str) -> (u16, String, String) {
    (500, json!({"error":{"message":format!("error parsing tool call: raw='{raw}', err=invalid tool JSON")}}).to_string(), "application/json".into())
}

#[test]
fn ollama_500_recovers_repairs_and_streams() {
    for stream in [false, true] {
        let raw = r#"{tool:'execute',args:{command:'solid.box',params:'{length:20,width:20,height:20,}'}}"#;
        let h = Harness::new(
            vec![parse_error(raw)],
            1,
            FallbackPrompt::Off,
            None,
            json!({}),
        );
        let mut req = request("make a box");
        req["stream"] = json!(stream);
        let (status, _, text) = h.chat(&req);
        assert_eq!(status, 200, "{text}");
        if stream {
            assert!(text.contains("tool_calls"));
            assert!(text.contains("[DONE]"));
        } else {
            assert_eq!(
                args(&text),
                json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}})
            );
        }
        assert_eq!(h.chats().len(), 1);
        assert!(fs::read_to_string(h.dir.join("audit.jsonl"))
            .unwrap()
            .contains("ollama 500 recovered"));
        h.finish();
    }
}

#[test]
fn ollama_500_retries_once_with_identical_messages() {
    for raw in [
        "no call",
        "{tool:'inspect',args:{}} {tool:'inspect',args:{}}",
        "[{tool:'inspect',args:{}}]",
        "{tool:'execute',args:{command:'solid.box',params:{}}}",
        "{tool:'execute',args:{command:'solid.box',params:'broken'}}",
    ] {
        let h = Harness::new(
            vec![parse_error(raw), parse_error(raw)],
            1,
            FallbackPrompt::Off,
            None,
            json!({}),
        );
        let (status, _, text) = h.chat(&request("make a box"));
        assert_eq!(status, 500, "{raw}: {text}");
        assert!(text.contains("The provider could not complete the request (HTTP 500)."));
        assert!(!text.contains(raw));
        let chats = h.chats();
        assert_eq!(chats.len(), 2);
        assert_eq!(chats[0], chats[1]);
        let log = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
        assert_eq!(log.matches("ollama 500 retried").count(), 1);
        h.finish();
    }
    let h = Harness::new(
        vec![parse_error("bad"), ok(call("inspect", json!({})))],
        1,
        FallbackPrompt::Off,
        None,
        json!({}),
    );
    assert_eq!(h.chat(&request("inspect")).0, 200);
    h.finish();
}

#[test]
fn conversation_hand_backs_stop_at_four_for_the_same_command() {
    let mut replies = Vec::new();
    for _ in 0..4 {
        replies.extend([
            ok(call(
                "execute",
                json!({"command":"solid.box","params":{"length":20}}),
            )),
            ok(prose("Cannot do it.")),
        ]);
    }
    replies.push(ok(call(
        "execute",
        json!({"command":"solid.box","params":{}}),
    )));
    let h = Harness::new(replies, 5, FallbackPrompt::Off, None, json!({}));
    let mut req = request("make a box");
    for turn in 0..5 {
        let (status, _, text) = h.chat(&req);
        assert_eq!(status, 200);
        let result: Value = serde_json::from_str(&text).unwrap();
        req["messages"].as_array_mut().unwrap().extend([
            result["choices"][0]["message"].clone(),
            json!({"role":"user","content":format!("try again {turn}")}),
        ]);
    }
    assert_eq!(h.chats().len(), 9);
    let profile: Value =
        serde_json::from_str(&dutchman_pilot::model::profiles(Some(&h.dir), None).unwrap())
            .unwrap();
    assert_eq!(profile[0]["hand_backs"], 4);
    h.finish();
}

#[test]
fn successful_previous_tool_turn_suppresses_hand_backs() {
    for success in [true, false] {
        let mut replies = vec![ok(prose("Use solid.box to make it."))];
        if !success {
            replies.push(ok(prose("Cannot do it.")));
        }
        let h = Harness::new(replies, 1, FallbackPrompt::Off, None, json!({}));
        let mut req = request("make a box");
        req["messages"].as_array_mut().unwrap().extend([
            call("execute",json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}}))["choices"][0]["message"].clone(),
            json!({"role":"tool","tool_call_id":"native-id","content":json!({"isError":!success,"result":{"bodies":[{}]}}).to_string()}),
        ]);
        assert_eq!(h.chat(&req).0, 200);
        assert_eq!(h.chats().len(), if success { 1 } else { 2 });
        h.finish();
    }
}

#[test]
fn local_how_hints_and_clean_tools_capability_profile() {
    let h = Harness::with_how(
        vec![
            ok(call("how", json!({"task":"round edges"}))),
            ok(call("inspect", json!({}))),
            ok(prose("{tool:'how',args:{task:'round edges'}}")),
            ok(call("inspect", json!({}))),
            ok(prose("Ready.")),
        ],
        3,
        FallbackPrompt::Auto,
        None,
        json!({"capabilities":["completion","tools"]}),
        true,
    );
    for _ in 0..2 {
        let (status, _, text) = h.chat(&request("find a command"));
        assert_eq!(status, 200);
        let response: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            response["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "inspect"
        );
        assert!(!text.contains("how"));
    }
    let state = json!({"bodies":[],"sketches":[]}).to_string();
    let mut req = request("inspect");
    req["messages"].as_array_mut().unwrap().extend([
        call("inspect", json!({}))["choices"][0]["message"].clone(),
        json!({"role":"tool","tool_call_id":"native-id","content":state}),
    ]);
    assert_eq!(h.chat(&req).0, 200);
    let chats = h.chats();
    assert_eq!(chats.len(), 5);
    for chat in &chats {
        assert!(chat["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["role"] != "system"));
        assert!(chat["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == "how"));
    }
    for index in [1, 3] {
        let messages = chats[index]["messages"].as_array().unwrap();
        assert_eq!(messages[1]["tool_calls"][0]["function"]["name"], "how");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(
            messages[2]["tool_call_id"],
            messages[1]["tool_calls"][0]["id"]
        );
        assert!(messages[2]["content"].as_str().unwrap().contains("fillet"));
    }
    assert_eq!(
        chats[4]["messages"][2]["content"],
        format!("{state}\n[dutchman-pilot hint] Empty design: start with a sketch or a primitive.")
    );
    let log = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
    assert!(!log.contains("fallback_prompt"));
    assert!(log.contains("how_answer"));
    assert!(log.contains("next_step_hint"));
    h.finish();
}

#[test]
fn end_to_end_required_paths_and_models_pass_through() {
    let native = call("inspect", json!({}));
    let batch = call(
        "batch",
        json!({"commands":"```json\n[{command:'solid.box',params:'{length:20,width:20,height:20}'}]\n```"}),
    );
    let models = json!({"object":"list","data":[{"id":"test-model"}]});
    let h = Harness::new(vec![
        ok(native.clone()),
        ok(prose("```json\n{\"tool\":\"execute\",\"args\":{\"command\":\"solid.box\",\"params\":{\"length\":20,\"width\":20,\"height\":20}}}\n```")),
        ok(batch),
        ok(prose("I will use solid.box to make that.")), ok(prose("I cannot modify a read-only design.")),
        ok(prose("")), ok(prose("Ready.")),
        ok(prose("{\"inspect\":{}}")),
        (429,json!({"error":{"code":"rate_limit_exceeded","message":"raw secret from provider"}}).to_string(),"application/json".into()),
        ok(models.clone()),
    ],8,FallbackPrompt::Auto,None,json!({"capabilities":["completion","tools"]}));
    let req = request("make a box");
    let (status, header, text) = h.chat(&req);
    assert_eq!(status, 200);
    assert_eq!(header, "rewrites=0");
    assert_eq!(text, native.to_string());
    assert_eq!(h.chats()[0], req);
    let (_, header, text) = h.chat(&req);
    assert_ne!(header, "rewrites=0");
    assert_eq!(args(&text)["command"], "solid.box");
    let (_, _, text) = h.chat(&req);
    assert_eq!(args(&text)["commands"][0]["params"]["length"], 20);
    let (_, _, text) = h.chat(&req);
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["choices"][0]["message"]["content"],
        "I cannot modify a read-only design."
    );
    let chats = h.chats();
    assert!(
        chats[4]["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("you described solid.box; run it, or say why not")
    );
    let (_, _, text) = h.chat(&request("hello"));
    assert!(text.contains("Ready."));
    let mut stream_req = request("inspect the design");
    stream_req["stream"] = json!(true);
    stream_req["stream_options"] = json!({"include_usage":true});
    let (_, header, text) = h.chat(&stream_req);
    assert_ne!(header, "rewrites=0");
    assert!(text.ends_with("data: [DONE]\n\n"));
    let chunks: Vec<Value> = text
        .split("\n\n")
        .filter_map(|s| s.strip_prefix("data: "))
        .filter(|s| *s != "[DONE]")
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");
    assert_eq!(
        chunks[1]["choices"][0]["delta"]["tool_calls"][0]["index"],
        0
    );
    assert_eq!(chunks[2]["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(chunks[3]["usage"]["total_tokens"], 14);
    let (status, _, text) = h.chat(&req);
    assert_eq!(status, 429);
    let error: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(error["error"]["code"], "rate_limited");
    assert!(error["error"]["message"].as_str().unwrap().contains('7'));
    assert!(!text.contains("raw secret"));
    let response = ureq::get(&format!("{}/v1/models", h.url))
        .set("Authorization", "Bearer test-secret")
        .call()
        .unwrap();
    assert_eq!(response.into_string().unwrap(), models.to_string());
    let chats = h.chats();
    assert!(!chats[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["role"] == "system"));
    assert_eq!(chats[7]["stream"], false);
    assert!(chats[7].get("stream_options").is_none());
    let profiles: Value = serde_json::from_str(
        &dutchman_pilot::model::profiles(Some(&h.dir), Some("test-model")).unwrap(),
    )
    .unwrap();
    assert_eq!(profiles[0]["turns"], 7);
    assert_eq!(profiles[0]["hand_backs"], 2);
    assert_eq!(profiles[0]["empty"], 1);
    assert_eq!(profiles[0]["text_written"], 2);
    assert_eq!(profiles[0]["native"], 2);
    let log = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
    for line in log.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        for key in ["time", "model", "kind", "before", "after"] {
            assert!(v.get(key).is_some());
        }
    }
    assert!(log.contains("argument_repair"));
    assert!(log.contains("hand_back"));
    h.finish();
}

#[test]
fn guards_reconcile_and_stop_after_two_hand_backs() {
    let h = Harness::new(
        vec![
            ok(call(
                "execute",
                json!({"Command":"solid.bxo {length:20,width:20,height:20}","Params":{}}),
            )),
            ok(call(
                "execute",
                json!({"command":"solid.box","params":{"length":20}}),
            )),
            ok(call(
                "execute",
                json!({"command":"solid.nothing","params":{}}),
            )),
            ok(prose("I cannot run that command.")),
            ok(prose("")),
            ok(prose("")),
        ],
        3,
        FallbackPrompt::Off,
        None,
        json!({}),
    );
    let (_, _, text) = h.chat(&request("make it"));
    assert_eq!(
        args(&text),
        json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}})
    );
    let (_, _, text) = h.chat(&request("make it"));
    assert!(text.contains("cannot run"));
    let chats = h.chats();
    assert!(
        chats[2]["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("needs height")
    );
    // "make it" says nothing about the task, so only the id's own namespace can rank suggestions.
    let nudge = chats[3]["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        nudge.starts_with("unknown solid.nothing; ")
            && (nudge.contains("closest: solid.") || nudge.contains("no close command")),
        "{nudge}"
    );
    let (_, _, text) = h.chat(&request("hello"));
    assert!(text.contains("\"content\":\"\""));
    assert_eq!(h.chats().len(), 6);
    h.finish();
}

#[test]
fn opt_in_settings_capability_fallback_and_json_only_on_decided_retry() {
    let h = Harness::new(
        vec![
            ok(call(
                "execute",
                json!({"command":"solid.box","params":{"length":20}}),
            )),
            ok(prose("{\"tool\":\"inspect\",\"args\":{}}")),
            ok(prose("I don't know.")),
        ],
        2,
        FallbackPrompt::Auto,
        Some(json!({"test-model":{"temperature":0.1,"seed":42,"num_ctx":8192,"json_retry":true}})),
        json!({"capabilities":["completion"]}),
    );
    h.chat(&request("make a box"));
    let chats = h.chats();
    assert_eq!(chats[0]["temperature"], 0.1);
    assert_eq!(chats[0]["seed"], 42);
    assert_eq!(chats[0]["options"]["num_ctx"], 8192);
    assert!(chats[0].get("response_format").is_none());
    assert!(fs::read_to_string(h.dir.join("audit.jsonl"))
        .unwrap()
        .contains("capabilities_lack_tools"));
    assert_eq!(chats[1]["response_format"]["type"], "json_object");
    assert_eq!(
        chats[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "system")
            .count(),
        // Text tool mode joins Pilot's system block instead of adding another.
        1
    );
    h.chat(&request("what can you do?"));
    assert_eq!(h.chats()[2]["temperature"], 0.1);
    h.finish();
}

#[test]
fn streamed_upstream_fragments_are_buffered_and_repaired() {
    let events = [
        json!({"id":"s","model":"test-model","created":1,"choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"fragment-id","type":"function","function":{"name":"bat","arguments":"{\"commands\": "}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"ch","arguments":"\"[{command:'sketch.finish',params:'{}'}]\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let mut stream = events
        .iter()
        .map(|e| format!("data: {e}\n\n"))
        .collect::<String>();
    stream.push_str("data: [DONE]\n\n");
    let h = Harness::new(
        vec![(200, stream, "text/event-stream".into())],
        1,
        FallbackPrompt::Off,
        None,
        json!({}),
    );
    let mut req = request("finish the sketch");
    req["stream"] = json!(true);
    let (_, _, text) = h.chat(&req);
    let chunks: Vec<Value> = text
        .lines()
        .filter_map(|s| s.strip_prefix("data: "))
        .filter(|s| *s != "[DONE]")
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let call = &chunks[1]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(call["id"], "fragment-id");
    assert_eq!(call["function"]["name"], "batch");
    let args: Value =
        serde_json::from_str(call["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(args["commands"][0]["params"], json!({}));
    h.finish();
}

#[test]
fn narrow_lookup_guard_and_prose_is_never_executed() {
    let h = Harness::new(
        vec![
            ok(prose("It is 40 mm³.")),
            ok(call("inspect", json!({}))),
            ok(prose("Use solid.box for boxes.")),
        ],
        2,
        FallbackPrompt::Off,
        None,
        json!({}),
    );
    let (_, _, text) = h.chat(&request("what is the volume of \"block\"?"));
    assert_eq!(args(&text), json!({}));
    assert!(
        h.chats()[1]["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("look it up first")
    );
    let (_, header, text) = h.chat(&request("what command makes a box?"));
    assert_eq!(header, "rewrites=0");
    assert!(text.contains("Use solid.box"));
    h.finish();
}

#[test]
fn malformed_or_oversized_requests_are_rejected_before_upstream() {
    let h = Harness::new(vec![], 2, FallbackPrompt::Off, None, json!({}));
    let (status, _, _) = h.chat(&json!({"model":"test-model","messages":[],"tools":[]}));
    assert_eq!(status, 400);
    let mut socket = TcpStream::connect(h.url.trim_start_matches("http://")).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    socket.write_all(b"POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1048577\r\n\r\n").unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"));
    assert!(h.requests.lock().unwrap().is_empty());
    h.finish();
}

#[test]
fn deterministic_planner_adds_one_system_message_and_logs_rule() {
    let h = Harness::new(
        vec![ok(call(
            "execute",
            json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}}),
        ))],
        1,
        FallbackPrompt::Off,
        None,
        json!({"capabilities":["tools"]}),
    );
    let (status, _, _) = h.chat(&request("make a 20 mm cube"));
    assert_eq!(status, 200);
    let chats = h.chats();
    let systems: Vec<_> = chats[0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "system")
        .collect();
    assert_eq!(systems.len(), 1);
    let text = systems[0]["content"].as_str().unwrap();
    let calls = text
        .strip_prefix("For this request, run exactly this plan: ")
        .unwrap()
        .split_once(". Don't change it; then reply using the tool results.")
        .unwrap()
        .0;
    let calls: Value = serde_json::from_str(calls).unwrap();
    assert_eq!(
        calls,
        json!([
            {"tool":"execute","arguments":{"command":"solid.box","params":{"length":20.0,"width":20.0,"height":20.0,"operation":"new"}}},
            {"tool":"execute","arguments":{"command":"document.inspect","params":{"measure":true}}}
        ])
    );
    let log = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
    let entries: Vec<Value> = log
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let plans: Vec<_> = entries.iter().filter(|v| v["kind"] == "plan").collect();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0]["after"]["rule"], "box");
    assert_eq!(
        plans[0]["after"]["slots"]["Box"]["size"],
        json!([20.0, 20.0, 20.0])
    );
    h.finish();
}

#[test]
fn unsupported_tools_retry_persists_and_flattens_client_history() {
    let h = Harness::new(
        vec![
            (
                400,
                json!({"error":"gemma3 DOES NOT SUPPORT TOOLS"}).to_string(),
                "application/json".into(),
            ),
            ok(prose(
                "Here is the call:\n```json\n{\"tool\":\"inspect\",\"args\":{}}\n```",
            )),
            ok(prose("{\"tool\":\"inspect\",\"args\":{}}")),
        ],
        2,
        FallbackPrompt::Off,
        None,
        json!({}),
    );
    let mut req = request("check status");
    req["tool_choice"] = json!("auto");
    req["parallel_tool_calls"] = json!(true);
    assert_eq!(h.chat(&req).0, 200);
    req["messages"] = json!([
        {"role":"user","content":"check status"},
        {"role":"assistant","content":null,"tool_calls":[{"id":"i1","type":"function","function":{"name":"inspect","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"i1","content":"volume = 123"},
        {"role":"user","content":"check again"}
    ]);
    let (status, _, response) = h.chat(&req);
    assert_eq!(status, 200);
    assert_eq!(args(&response), json!({}));
    let chats = h.chats();
    assert!(chats[0].get("tools").is_some());
    for chat in &chats[1..] {
        for key in ["tools", "tool_choice", "parallel_tool_calls"] {
            assert!(chat.get(key).is_none());
        }
        assert!(chat["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("command:string"));
    }
    let history = chats[2]["messages"].as_array().unwrap();
    assert!(history
        .iter()
        .all(|m| m["role"] != "tool" && m.get("tool_calls").is_none()));
    assert!(history.iter().any(|m| m["content"]
        .as_str()
        .is_some_and(|s| s.contains("Tool inspect returned: volume = 123"))));
    let profile = dutchman_pilot::model::profiles(Some(&h.dir), None).unwrap();
    assert!(profile.contains("\"text_tool_mode\": true"));
    let log = fs::read_to_string(h.dir.join("audit.jsonl")).unwrap();
    for kind in [
        "text_tool_mode",
        "tools_stripped",
        "tool_history_flattened",
        "text_call_converted",
    ] {
        assert!(log.contains(&format!("\"kind\":\"{kind}\"")), "{kind}");
    }
    h.finish();
}

#[test]
fn capabilities_select_text_mode_and_recipes_still_fire() {
    let h = Harness::with_how(
        vec![ok(prose("I will make a 20 mm cube"))],
        1,
        FallbackPrompt::Off,
        None,
        json!({"capabilities":["completion"]}),
        true,
    );
    let (status, _, response) = h.chat(&request("make a 20 mm cube"));
    assert_eq!(status, 200, "{response}");
    assert_eq!(args(&response)["command"], "solid.box");
    assert!(h.chats()[0].get("tools").is_none());
    let prompt = h.chats()[0]["messages"][0]["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(prompt.contains("how:") && !prompt.contains("solid.box:"));
    assert!(fs::read_to_string(h.dir.join("audit.jsonl"))
        .unwrap()
        .contains("recipe_prose"));
    h.finish();
}

#[test]
fn text_mode_refuses_multiple_objects_and_keeps_plain_text() {
    for text in [
        "Hello.",
        "{\"tool\":\"inspect\",\"args\":{}} {\"tool\":\"inspect\",\"args\":{}}",
    ] {
        let h = Harness::new(
            vec![ok(prose(text))],
            1,
            FallbackPrompt::Auto,
            None,
            json!({"capabilities":["completion"]}),
        );
        let (status, _, response) = h.chat(&request("hello"));
        assert_eq!(status, 200);
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["choices"][0]["message"]["content"], text);
        h.finish();
    }
}

#[test]
fn direct_http_turn_has_no_upstream_requests() {
    for stream in [false, true] {
        let h = Harness::with_direct(
            vec![],
            2,
            FallbackPrompt::Auto,
            None,
            json!({}),
            false,
            true,
        );
        let mut req = request("create a cube 1/4 inch");
        req["stream"] = json!(stream);
        let (status, _, text) = h.chat(&req);
        assert_eq!(status, 200, "{text}");
        let value: Value = if stream {
            let chunks: Vec<Value> = text
                .lines()
                .filter_map(|s| s.strip_prefix("data: "))
                .filter(|s| *s != "[DONE]")
                .map(|s| serde_json::from_str(s).unwrap())
                .collect();
            assert!(text.ends_with("data: [DONE]\n\n"));
            json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":chunks[1]["choices"][0]["delta"]["tool_calls"]}}]})
        } else {
            serde_json::from_str(&text).unwrap()
        };
        let mut message = value["choices"][0]["message"].clone();
        for call in message["tool_calls"].as_array_mut().unwrap() {
            call.as_object_mut().unwrap().remove("index");
        }
        let calls = message["tool_calls"].as_array().unwrap().clone();
        req["messages"].as_array_mut().unwrap().push(message);
        for c in calls {
            req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":c["id"],"content":json!({"bodies":[{"name":"Cube","volume_mm3":256.047875}]}).to_string()}));
        }
        let (status, _, text) = h.chat(&req);
        assert_eq!(status, 200, "{text}");
        assert!(text.contains("Exact match (no model):") && text.contains("256.047875"));
        assert!(h.requests.lock().unwrap().is_empty());
        h.finish();
    }
}

#[test]
fn report_capture_endpoint_and_disabled_buffer() {
    for disabled in [false, true] {
        let dir = std::env::temp_dir().join(format!(
            "pilot-report-capture-{}-{disabled}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut server = Server::bind(Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            upstream: "none".into(),
            upstream_timeout: Duration::from_secs(2),
            fallback_prompt: FallbackPrompt::Off,
            no_report_buffer: disabled,
            no_how: true,
            no_hints: true,
            planner: true,
            direct_recipes: true,
            log: Some(dir.join("audit.jsonl")),
            profile_dir: Some(dir.clone()),
            model_settings: None,
            model_settings_enabled: false,
            seed: None,
            prompt_help: false,
        })
        .unwrap();
        let address = server.local_addr().unwrap();
        let thread = thread::spawn(move || {
            for _ in 0..9 {
                server.serve_one().unwrap();
            }
        });
        for i in 0..7 {
            let body = json!({"model":"unit","messages":[{"role":"user","content":format!("create a cube {}mm",i+1)}],"tools":[{"type":"function","function":{"name":"execute","parameters":{"type":"object"}}}]});
            ureq::post(&format!("http://{address}/v1/chat/completions"))
                .send_string(&body.to_string())
                .unwrap();
        }
        let mut response = String::new();
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .write_all(b"GET /pilot/turns HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 "));
        let turns: Value =
            serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            turns.as_array().unwrap().len(),
            if disabled { 0 } else { 5 }
        );
        if !disabled {
            assert_eq!(turns[0]["id"], 3);
            assert_eq!(turns[4]["id"], 7);
            assert!(turns[4]["events"].to_string().contains("finish_reason"));
            assert!(!turns.to_string().contains("parameters"));
        }
        let models: Value = serde_json::from_str(
            &ureq::get(&format!("http://{address}/v1/models"))
                .call()
                .unwrap()
                .into_string()
                .unwrap(),
        )
        .unwrap();
        assert!(!models.to_string().contains("turns"));
        thread.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn report_endpoint_rejects_non_loopback_peer() {
    // Discover the host's local interface without sending a UDP packet.
    let route = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
    route.connect("192.0.2.1:9").unwrap();
    let ip = route.local_addr().unwrap().ip();
    assert!(!ip.is_loopback());
    let dir = std::env::temp_dir().join(format!("pilot-report-peer-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut server = Server::bind(Config {
        listen: "0.0.0.0:0".parse().unwrap(),
        upstream: "none".into(),
        upstream_timeout: Duration::from_secs(2),
        fallback_prompt: FallbackPrompt::Off,
        no_report_buffer: false,
        no_how: true,
        no_hints: true,
        planner: true,
        direct_recipes: true,
        log: Some(dir.join("audit.jsonl")),
        profile_dir: Some(dir.clone()),
        model_settings: None,
        model_settings_enabled: false,
        seed: None,
        prompt_help: false,
    })
    .unwrap();
    let address = std::net::SocketAddr::new(ip, server.local_addr().unwrap().port());
    let thread = thread::spawn(move || server.serve_one().unwrap());
    let mut socket = TcpStream::connect(address).unwrap();
    socket
        .write_all(b"GET /pilot/turns HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 403 "), "{response}");
    thread.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}
