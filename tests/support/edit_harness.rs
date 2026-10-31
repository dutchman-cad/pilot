//! A fake SolidCraft client and model for edit-request development tests.
//!
//! The proxy runs with a fake upstream that always answers in prose. The client
//! side executes every tool call Pilot emits through the public `solidcraft-cli
//! run` command on a design built from a start script, as the Assistant would.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::model::{Config, FallbackPrompt, Server};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub fn cli() -> Option<PathBuf> {
    std::env::var_os("PILOT_SOLIDCRAFT_CLI").map(PathBuf::from)
}

/// Run a script on the public CLI and return one result per command.
pub fn run_script(cli: &PathBuf, commands: &[Value]) -> Vec<Value> {
    let path = std::env::temp_dir().join(format!(
        "dutchman-edit-{}-{}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, json!({"commands":commands}).to_string()).unwrap();
    let out = Command::new(cli)
        .args(["run"])
        .arg(&path)
        .args(["--results", "--quiet"])
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let _ = fs::remove_file(&path);
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "cli results are not JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

pub struct Proxy {
    pub url: String,
    pub dir: PathBuf,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// `direct` is the default mode; false is `--no-direct-recipes` (model only).
pub fn proxy(direct: bool, no_hints: bool) -> Proxy {
    let dir = std::env::temp_dir().join(format!(
        "dutchman-edit-proxy-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let upstream = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let upstream_url = format!("http://{}/v1", upstream.server_addr());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (record, quit) = (requests.clone(), stop.clone());
    thread::spawn(move || {
        while !quit.load(Ordering::Relaxed) {
            if let Some(mut req) = upstream.recv_timeout(Duration::from_millis(100)).unwrap() {
                let path = req.url().to_string();
                let mut text = String::new();
                req.as_reader().read_to_string(&mut text).unwrap();
                let body = if path == "/api/show" {
                    json!({"capabilities":["tools"]}).to_string()
                } else {
                    record
                        .lock()
                        .unwrap()
                        .push(serde_json::from_str(&text).unwrap_or(Value::Null));
                    json!({"id":"t","object":"chat.completion","model":"m","created":1,"choices":[{"index":0,"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]}).to_string()
                };
                let response = tiny_http::Response::from_string(body).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                );
                req.respond(response).unwrap();
            }
        }
    });
    let mut server = Server::bind(Config {
        solidcraft98: false,
        direct_recipes: direct,
        no_report_buffer: false,
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream_url,
        upstream_timeout: Duration::from_secs(60),
        fallback_prompt: FallbackPrompt::Off,
        no_how: false,
        no_hints,
        offer_recipe_tools: false,
        no_shortcuts: false,
        no_fill: false,
        no_trim: false,
        trim_cap: 1500,
        trim_keep: 2,
        recipes_dir: None,
        planner: false,
        log: Some(dir.join("audit.jsonl")),
        profile_dir: Some(dir.clone()),
        model_settings: None,
        model_settings_enabled: false,
        seed: None,
        prompt_help: false,
    })
    .unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    thread::spawn(move || loop {
        if server.serve_one().is_err() {
            break;
        }
    });
    Proxy {
        url,
        dir,
        requests,
        stop,
    }
}

pub fn tools() -> Value {
    json!([
        {"type":"function","function":{"name":"execute","parameters":{"type":"object","properties":{"command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}},
        {"type":"function","function":{"name":"batch","parameters":{"type":"object","properties":{"commands":{"type":"array"}},"required":["commands"]}}},
        {"type":"function","function":{"name":"inspect","parameters":{"type":"object","properties":{}}}}
    ])
}

pub struct Turn {
    /// The last chat request that reached the model.
    pub upstream: Value,
    /// Commands Pilot asked the client to run before the model was called.
    pub pilot_reads: Vec<String>,
    /// The first reply, when Pilot answered without the model.
    pub direct: Option<Value>,
}

impl Proxy {
    /// Send one user message the way a client does, running Pilot's own tool calls.
    pub fn ask(&self, cli: &PathBuf, start: &[Value], text: &str) -> Turn {
        let before = self.requests.lock().unwrap().len();
        let mut messages = vec![json!({"role":"user","content":text})];
        let mut reads = Vec::new();
        for round in 0..8 {
            let body = json!({"model":"m","messages":messages,"tools":tools()});
            let response = ureq::post(&format!("{}/v1/chat/completions", self.url))
                .timeout(Duration::from_secs(60))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string());
            let response = match response {
                Ok(r) | Err(ureq::Error::Status(_, r)) => r,
                Err(e) => panic!("{e}"),
            };
            let value: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
            let message = value["choices"][0]["message"].clone();
            let Some(calls) = message["tool_calls"].as_array().filter(|c| !c.is_empty()) else {
                let seen = self.requests.lock().unwrap().len();
                let upstream = if seen > before {
                    self.requests.lock().unwrap()[seen - 1].clone()
                } else {
                    Value::Null
                };
                return Turn {
                    upstream,
                    pilot_reads: reads,
                    direct: (seen == before).then_some(message),
                };
            };
            assert!(round < 7, "too many tool rounds");
            messages.push(message.clone());
            for call in calls {
                let args: Value =
                    serde_json::from_str(call["function"]["arguments"].as_str().unwrap()).unwrap();
                let commands: Vec<Value> = if call["function"]["name"] == "batch" {
                    args["commands"].as_array().unwrap().clone()
                } else {
                    vec![json!({"command":args["command"],"params":args["params"]})]
                };
                for c in &commands {
                    reads.push(format!("{} {}", c["command"], c["params"]));
                }
                let mut script = start.to_vec();
                script.extend(commands.iter().cloned());
                let results = run_script(cli, &script);
                let last = results.last().unwrap().to_string();
                messages.push(json!({"role":"tool","tool_call_id":call["id"],"content":last}));
            }
        }
        panic!("unreachable");
    }
}

/// The system messages added in front of or after the client history.
pub fn systems(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "system")
        .map(|m| text_of(&m["content"]))
        .collect()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
