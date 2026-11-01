//! A fake SolidCraft client and model for edit-request development tests.
//!
//! The proxy runs with a fake upstream that always answers in prose. The client
//! side supplies model inspection history and replays suggestions through the
//! public `solidcraft-cli run` command on a design built from a start script.
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

/// `direct` is the default mode; false is model only.
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
                    let message = json!({"role":"assistant","content":"done"});
                    json!({"id":"t","object":"chat.completion","model":"m","created":1,"choices":[{"index":0,"message":message,"finish_reason":"stop"}]}).to_string()
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
        no_fill: true,
        no_trim: true,
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

impl Proxy {
    pub fn send(&self, messages: &[Value]) -> Value {
        let body = json!({"model":"m","messages":messages,"tools":tools()});
        let response = ureq::post(&format!("{}/v1/chat/completions", self.url))
            .timeout(Duration::from_secs(60))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .unwrap();
        serde_json::from_str(&response.into_string().unwrap()).unwrap()
    }
    pub fn last(&self) -> Value {
        self.requests.lock().unwrap().last().unwrap().clone()
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
