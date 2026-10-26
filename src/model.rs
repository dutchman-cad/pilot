//! Blocking OpenAI-compatible proxy with explicit-call repair and optional recipes.
use crate::{
    audit::{now, Audit},
    catalog::Catalog,
    guidance, planner, prompting, repair,
};
use prompting::FALLBACK;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
mod direct;
mod fill;
mod history;
mod recovery;
mod shortcuts;
mod text_mode;
use recovery::Conversation;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_BODY: usize = repair::MAX_BYTES;
const MAX_HEADERS: usize = 16_384;
const MAX_CALLS: usize = 64;
const MAX_PROFILES: usize = 256;
pub const DEFAULT_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(300);
pub const MAX_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(3600);
pub const DEFAULT_TRIM_CAP: usize = 1500;
pub const DEFAULT_TRIM_KEEP: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPrompt {
    Auto,
    On,
    Off,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Explicit SolidCraft 98 dialect: no SolidCraft catalogue, planner, or local how.
    pub solidcraft98: bool,
    pub listen: SocketAddr,
    pub no_report_buffer: bool,
    pub upstream: String,
    pub upstream_timeout: Duration,
    pub fallback_prompt: FallbackPrompt,
    pub no_how: bool,
    pub no_hints: bool,
    pub no_shortcuts: bool,
    pub offer_recipe_tools: bool,
    pub no_fill: bool,
    pub no_trim: bool,
    pub trim_cap: usize,
    pub trim_keep: usize,
    pub recipes_dir: Option<PathBuf>,
    pub planner: bool,
    pub direct_recipes: bool,
    pub log: Option<PathBuf>,
    pub profile_dir: Option<PathBuf>,
    /// JSON object keyed by model name; omitted models keep their own settings.
    pub model_settings: Option<PathBuf>,
    pub model_settings_enabled: bool,
    pub seed: Option<i64>,
    pub prompt_help: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub temperature: Option<f64>,
    pub seed: Option<i64>,
    pub num_ctx: Option<u32>,
    #[serde(default)]
    pub json_retry: bool,
}
impl ModelSettings {
    fn validate(&self) -> Result<(), String> {
        if self.temperature.is_some_and(|n| !(0.0..=0.2).contains(&n)) {
            return Err("model temperature must be between 0 and 0.2".into());
        }
        if self.num_ctx.is_some_and(|n| !(8192..=131072).contains(&n)) {
            return Err("model num_ctx must be between 8192 and 131072".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Profile {
    pub upstream: String,
    pub model: String,
    pub turns: u64,
    pub direct_turns: u64,
    pub direct_recipe_calls: u64,
    pub direct_recipe_reply: u64,
    pub direct_recipe_fallback: u64,
    pub native: u64,
    pub text_written: u64,
    pub prose: u64,
    pub empty: u64,
    pub repairs: u64,
    pub hand_backs: u64,
    pub repairs_by_kind: BTreeMap<String, u64>,
    pub provider_errors: u64,
    pub ollama_500_recoveries: u64,
    pub provider_500_retries: u64,
    pub plans_used: u64,
    pub how_answers: u64,
    pub hints_added: u64,
    pub finds: u64,
    pub find_hits: u64,
    pub find_misses: u64,
    #[serde(default = "trim_default")]
    pub trim_enabled: bool,
    pub offer_recipe_tools: bool,
    #[serde(default = "default_trim_keep")]
    pub trim_keep: usize,
    pub upstream_requests: u64,
    pub request_chars_total: BTreeMap<String, u64>,
    pub request_chars_max: BTreeMap<String, u64>,
    pub capabilities_checked: bool,
    pub native_support: Option<bool>,
    /// Explicit provider rejection; independent of prose/text-call profiling.
    pub text_tool_mode: bool,
    pub ollama: bool,
    /// One entry per client turn, oldest first. Old profiles start with no evidence.
    pub recent_text_calls: Vec<bool>,
}
type Profiles = BTreeMap<String, Profile>;

fn trim_default() -> bool {
    true
}

fn default_trim_keep() -> usize {
    DEFAULT_TRIM_KEEP
}

pub(crate) fn data_dir(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(root).join(crate::identity::BINARY));
    }
    if cfg!(target_os = "windows") {
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(root).join(crate::identity::BINARY));
        }
    }
    let home = std::env::var_os("HOME").ok_or("cannot locate user data dir; use --profile-dir")?;
    let suffix = if cfg!(target_os = "macos") {
        "Library/Application Support"
    } else {
        ".local/share"
    };
    Ok(PathBuf::from(home)
        .join(suffix)
        .join(crate::identity::BINARY))
}
fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not read body within the time limit".to_string())?;
    if bytes.len() > limit {
        return Err(format!("body exceeds {limit} bytes"));
    }
    Ok(bytes)
}
fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match File::open(path) {
        Ok(file) => serde_json::from_slice(&read_bounded(file, 2 * MAX_BODY)?)
            .map_err(|e| format!("invalid local JSON {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}
pub(crate) fn load_profiles(
    dir: Option<&Path>,
    model: Option<&str>,
) -> Result<Vec<Profile>, String> {
    let profiles: Profiles = read_json(&data_dir(dir)?.join("profiles.json"))?;
    if profiles.len() > MAX_PROFILES {
        return Err("profile file exceeds 256 models".into());
    }
    if profiles.values().any(|p| {
        p.recent_text_calls.len() > 5
            || p.repairs_by_kind.len() > 256
            || p.request_chars_total.len() > 512
            || p.request_chars_max.len() > 512
    }) {
        return Err("profile history exceeds five turns or repair kinds exceed 256".into());
    }
    let mut records: Vec<_> = profiles
        .values()
        .filter(|p| model.is_none_or(|m| p.model == m))
        .cloned()
        .collect();
    records.sort_by(|a, b| (&a.model, &a.upstream).cmp(&(&b.model, &b.upstream)));
    Ok(records)
}
pub fn profiles(dir: Option<&Path>, model: Option<&str>) -> Result<String, String> {
    serde_json::to_string_pretty(&load_profiles(dir, model)?)
        .map_err(|e| format!("cannot format profiles: {e}"))
}
fn normalize(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}
fn json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    repair::check_value(value)?;
    let bytes = serde_json::to_vec(value).map_err(|e| format!("cannot encode JSON: {e}"))?;
    if bytes.len() > MAX_BODY {
        return Err("encoded JSON exceeds 1 MiB".into());
    }
    Ok(bytes)
}

/// Single-threaded by design: one bounded request at a time, no growing worker queue.
pub struct Server {
    listener: TcpListener,
    proxy: Proxy,
}
struct Proxy {
    cfg: Config,
    recipes: crate::recipe_library::Library,
    agent: ureq::Agent,
    catalog: Catalog,
    profiles: Profiles,
    profile_path: PathBuf,
    settings: BTreeMap<String, ModelSettings>,
    log: File,
    conversations: Vec<Conversation>,
    form_turns: std::cell::RefCell<fill::Turns>,
    turns: crate::capture::Ring,
    #[cfg(test)]
    scripted: Option<std::cell::RefCell<Scripted>>,
}
#[cfg(test)]
#[derive(Default)]
struct Scripted {
    model: Option<fn(&Value) -> Value>,
    reject_tools: bool,
    replies: std::collections::VecDeque<Upstream>,
    requests: Vec<Value>,
}
impl Server {
    pub fn bind(mut cfg: Config) -> Result<Self, String> {
        if !(256..=65536).contains(&cfg.trim_cap) {
            return Err("trim-cap must be between 256 and 65536 characters".into());
        }
        if cfg.trim_keep > 4096 {
            return Err("trim-keep must be between 0 and 4096 results".into());
        }
        if cfg.solidcraft98 {
            cfg.planner = false;
            cfg.direct_recipes = false;
            cfg.no_how = true;
            cfg.no_hints = true;
            cfg.no_shortcuts = true;
            cfg.prompt_help = false;
        }
        if cfg.upstream_timeout < Duration::from_secs(1)
            || cfg.upstream_timeout > MAX_UPSTREAM_TIMEOUT
        {
            return Err("upstream-timeout must be between 1 and 3600 seconds".into());
        }
        if cfg.upstream != "none" {
            let url = url::Url::parse(&cfg.upstream).map_err(|_| "invalid upstream URL")?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(
                    "upstream must be an HTTP(S) base URL without credentials, query or fragment"
                        .into(),
                );
            }
            cfg.upstream = url.as_str().trim_end_matches('/').to_string();
        }
        let dir = data_dir(cfg.profile_dir.as_deref())?;
        fs::create_dir_all(&dir).map_err(|e| format!("cannot create profile dir: {e}"))?;
        let profile_path = dir.join("profiles.json");
        let profiles: Profiles = read_json(&profile_path)?;
        if profiles.len() > MAX_PROFILES {
            return Err("profile file exceeds 256 models".into());
        }
        if profiles.values().any(|p| p.recent_text_calls.len() > 5) {
            return Err("profile history exceeds five turns".into());
        }
        let settings: BTreeMap<String, ModelSettings> = match &cfg.model_settings {
            Some(path) => {
                if !path.is_file() {
                    return Err(format!(
                        "model settings file does not exist: {}",
                        path.display()
                    ));
                }
                read_json(path)?
            }
            None => BTreeMap::new(),
        };
        if settings.len() > MAX_PROFILES {
            return Err("settings exceed 256 models".into());
        }
        for setting in settings.values() {
            setting.validate()?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(cfg.log.clone().unwrap_or(dir.join("rewrites.jsonl")))
            .map_err(|e| format!("cannot open rewrite log: {e}"))?;
        let listener = TcpListener::bind(cfg.listen).map_err(|e| format!("cannot listen: {e}"))?;
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .try_proxy_from_env(false)
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(cfg.upstream_timeout)
            .timeout_write(cfg.upstream_timeout)
            .timeout(cfg.upstream_timeout)
            .build();
        Ok(Self {
            listener,
            proxy: Proxy {
                recipes: crate::recipe_library::Library::load(cfg.recipes_dir.as_deref())?,
                agent,
                catalog: if cfg.solidcraft98 {
                    Catalog::from_json("[]")?
                } else {
                    Catalog::bundled()?
                },
                cfg,
                profiles,
                profile_path,
                settings,
                log,
                conversations: Vec::new(),
                form_turns: std::cell::RefCell::default(),
                turns: crate::capture::Ring::default(),
                #[cfg(test)]
                scripted: None,
            },
        })
    }
    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.listener
            .local_addr()
            .map_err(|e| format!("cannot get listen address: {e}"))
    }
    /// Handle one connection, including malformed-request responses; useful for embedding/tests.
    pub fn serve_one(&mut self) -> Result<(), String> {
        let (mut socket, peer) = self
            .listener
            .accept()
            .map_err(|e| format!("accept failed: {e}"))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("cannot set timeout: {e}"))?;
        let response = match read_request(&mut socket) {
            Ok(request) => self.proxy.handle_peer(request, peer),
            Err(message) => Err(message),
        }
        .unwrap_or_else(|message| Reply::error(400, "proxy_error", &message));
        // A disconnected client must not stop the proxy.
        let _ = response.write(&mut socket);
        Ok(())
    }
}
pub fn serve(cfg: Config) -> Result<(), String> {
    let mut server = Server::bind(cfg)?;
    loop {
        server.serve_one()?;
    }
}

struct Request {
    method: String,
    path: String,
    auth: Option<String>,
    body: Vec<u8>,
}
// A short HTTP/1.1 reader keeps header/body sizes and the entire ingress deadline bounded.
// Connections close after one request. Chunked bodies are accepted with capped chunk count.
fn read_request(socket: &mut TcpStream) -> Result<Request, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= MAX_HEADERS {
            return Err("headers exceed 16 KiB".into());
        }
        header.push(read_byte(socket, deadline)?);
    }
    let text = std::str::from_utf8(&header).map_err(|_| "invalid HTTP headers")?;
    let mut lines = text.split("\r\n");
    let mut first = lines
        .next()
        .ok_or("missing request line")?
        .split_whitespace();
    let method = first.next().ok_or("missing HTTP method")?.to_string();
    let path = first.next().ok_or("missing HTTP path")?.to_string();
    if first.next() != Some("HTTP/1.1") || first.next().is_some() {
        return Err("expected HTTP/1.1 request".into());
    }
    let mut auth = None;
    let mut length = None;
    let mut chunked = false;
    for line in lines.filter(|s| !s.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid HTTP header")?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "authorization" => {
                if auth.is_some() {
                    return Err("duplicate Authorization header".into());
                }
                auth = Some(value.to_string());
            }
            "content-length" => {
                if length.is_some() {
                    return Err("duplicate Content-Length header".into());
                }
                length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid Content-Length")?,
                );
            }
            "transfer-encoding" if value.eq_ignore_ascii_case("chunked") && !chunked => {
                chunked = true
            }
            "transfer-encoding" => return Err("unsupported Transfer-Encoding".into()),
            "expect" => return Err("Expect is unsupported; send the bounded body directly".into()),
            _ => {}
        }
    }
    if chunked && length.is_some() {
        return Err("ambiguous request framing".into());
    }
    let mut body = Vec::new();
    if chunked {
        let mut finished = false;
        for _ in 0..4096 {
            let mut line = Vec::new();
            while !line.ends_with(b"\r\n") {
                if line.len() >= 128 {
                    return Err("chunk header too long".into());
                }
                line.push(read_byte(socket, deadline)?);
            }
            let text = std::str::from_utf8(&line).map_err(|_| "invalid chunk header")?;
            let size = usize::from_str_radix(text.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| "invalid chunk size")?;
            if size == 0 {
                if read_byte(socket, deadline)? != b'\r' || read_byte(socket, deadline)? != b'\n' {
                    return Err("chunk trailers unsupported".into());
                }
                finished = true;
                break;
            }
            read_part(socket, deadline, &mut body, size)?;
            if read_byte(socket, deadline)? != b'\r' || read_byte(socket, deadline)? != b'\n' {
                return Err("invalid chunk terminator".into());
            }
        }
        if !finished {
            return Err("chunk count exceeds 4096".into());
        }
    } else {
        read_part(socket, deadline, &mut body, length.unwrap_or(0))?;
    }
    Ok(Request {
        method,
        path,
        auth,
        body,
    })
}
fn read_byte(socket: &mut TcpStream, deadline: Instant) -> Result<u8, String> {
    let mut byte = [0];
    read_deadline(socket, deadline, &mut byte)?;
    Ok(byte[0])
}
fn read_deadline(
    socket: &mut TcpStream,
    deadline: Instant,
    mut bytes: &mut [u8],
) -> Result<(), String> {
    while !bytes.is_empty() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or("request read timed out")?;
        socket
            .set_read_timeout(Some(remaining))
            .map_err(|_| "cannot set read timeout")?;
        let count = socket
            .read(bytes)
            .map_err(|_| "incomplete request or read timeout")?;
        if count == 0 {
            return Err("incomplete request body".into());
        }
        bytes = &mut bytes[count..];
    }
    Ok(())
}
fn read_part(
    socket: &mut TcpStream,
    deadline: Instant,
    body: &mut Vec<u8>,
    size: usize,
) -> Result<(), String> {
    if size > MAX_BODY.saturating_sub(body.len()) {
        return Err("request body exceeds 1 MiB".into());
    }
    let start = body.len();
    body.resize(start + size, 0);
    read_deadline(socket, deadline, &mut body[start..])
}

struct Reply {
    status: u16,
    content_type: String,
    body: Vec<u8>,
    rewrites: usize,
    retry_after: Option<String>,
}
impl Reply {
    fn error(status: u16, code: &str, message: &str) -> Self {
        Self {
            status,
            content_type: "application/json".into(),
            body: json!({"error":{"message":message,"code":code,"type":"dutchman_pilot_error"}})
                .to_string()
                .into_bytes(),
            rewrites: 0,
            retry_after: None,
        }
    }
    fn write(self, socket: &mut TcpStream) -> Result<(), String> {
        let retry = self
            .retry_after
            .map(|s| format!("Retry-After: {s}\r\n"))
            .unwrap_or_default();
        let header = format!("HTTP/1.1 {} Response\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}: rewrites={}\r\n{}\r\n",self.status,self.content_type,self.body.len(),crate::identity::RESPONSE_HEADER,self.rewrites,retry);
        let deadline = Instant::now() + Duration::from_secs(5);
        for mut bytes in [header.as_bytes(), self.body.as_slice()] {
            while !bytes.is_empty() {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .filter(|d| !d.is_zero())
                    .ok_or("response write timed out")?;
                socket
                    .set_write_timeout(Some(remaining))
                    .map_err(|_| "cannot set write timeout")?;
                let count = socket
                    .write(bytes)
                    .map_err(|_| "client disconnected or response write timed out")?;
                if count == 0 {
                    return Err("client disconnected".into());
                }
                bytes = &bytes[count..];
            }
        }
        Ok(())
    }
}

struct Upstream {
    status: u16,
    body: Vec<u8>,
    content_type: String,
    retry_after: Option<String>,
}
impl Proxy {
    fn fetch(
        &self,
        method: &str,
        path: &str,
        auth: Option<&str>,
        body: &[u8],
        deadline: Instant,
    ) -> Result<Upstream, String> {
        #[cfg(test)]
        if let Some(script) = &self.scripted {
            let mut script = script.borrow_mut();
            script
                .requests
                .push(serde_json::from_slice(body).unwrap_or(Value::Null));
            if script.reject_tools
                && script
                    .requests
                    .last()
                    .is_some_and(|r| r.get("tools").is_some())
            {
                return Ok(Upstream {
                    status: 400,
                    body: br#"{"error":"model DOES NOT SUPPORT TOOLS"}"#.to_vec(),
                    content_type: "application/json".into(),
                    retry_after: None,
                });
            }
            if let Some(model) = script.model {
                let request =
                    serde_json::from_slice(body).map_err(|e| format!("test request: {e}"))?;
                return Ok(Upstream {
                    status: 200,
                    body: model(&request).to_string().into_bytes(),
                    content_type: "application/json".into(),
                    retry_after: None,
                });
            }
            return script
                .replies
                .pop_front()
                .ok_or_else(|| "scripted replies exhausted".into());
        }
        let time = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or("turn time limit reached")?
            .min(self.cfg.upstream_timeout);
        let mut request = self
            .agent
            .request(method, &format!("{}{path}", self.cfg.upstream))
            .timeout(time);
        if let Some(auth) = auth {
            request = request.set("Authorization", auth);
        }
        let result = if method == "GET" {
            request.call()
        } else {
            request
                .set("Content-Type", "application/json")
                .send_bytes(body)
        };
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(_) => return Err("upstream connection failed or timed out".into()),
        };
        let status = response.status();
        let content_type = response
            .header("Content-Type")
            .unwrap_or("application/json")
            .to_string();
        let retry_after = response
            .header("Retry-After")
            .filter(|s| s.len() <= 128 && s.chars().all(|c| c.is_ascii() && !c.is_control()))
            .map(str::to_string);
        let body = read_bounded(response.into_reader(), MAX_BODY)?;
        Ok(Upstream {
            status,
            body,
            content_type,
            retry_after,
        })
    }
    fn handle_peer(&mut self, request: Request, peer: SocketAddr) -> Result<Reply, String> {
        if request.path == "/pilot/turns" && !peer.ip().is_loopback() {
            return Ok(Reply::error(
                403,
                "forbidden",
                "turn capture is loopback only",
            ));
        }
        self.handle(request)
    }
    fn handle(&mut self, request: Request) -> Result<Reply, String> {
        // Two shared hand-backs, one parse-error retry, and the capability probe.
        let deadline = Instant::now() + self.cfg.upstream_timeout * 5 + Duration::from_secs(3);
        if request.path == "/pilot/turns" && request.method == "GET" {
            let body = serde_json::to_vec(self.turns.turns())
                .map_err(|e| format!("cannot encode turns: {e}"))?;
            if body.len() > 8 * 1_048_576 {
                return Err("turn response exceeds 8 MiB".into());
            }
            return Ok(Reply {
                status: 200,
                content_type: "application/json".into(),
                body,
                rewrites: 0,
                retry_after: None,
            });
        }
        if request.path == "/v1/models" && request.method == "GET" {
            if self.cfg.upstream == "none" {
                return Ok(Reply {
                    status: 200,
                    content_type: "application/json".into(),
                    body: json_bytes(
                        &json!({"object":"list","data":[{"id":"pilot-recipes","object":"model","owned_by":"pilot"}]}),
                    )?,
                    rewrites: 0,
                    retry_after: None,
                });
            }
            let response = self.fetch("GET", "/models", request.auth.as_deref(), &[], deadline)?;
            return Ok(Reply {
                status: response.status,
                body: response.body,
                content_type: response.content_type,
                rewrites: 0,
                retry_after: response.retry_after,
            });
        }
        if request.path != "/v1/chat/completions" || request.method != "POST" {
            return Ok(Reply::error(
                404,
                "not_found",
                "Use POST /v1/chat/completions or GET /v1/models.",
            ));
        }
        let body: Value =
            serde_json::from_slice(&request.body).map_err(|_| "invalid request JSON")?;
        validate_request(&body)?;
        let model = body["model"]
            .as_str()
            .ok_or("model must be a string")?
            .to_string();
        let key = serde_json::to_string(&(&self.cfg.upstream, &model))
            .map_err(|_| "cannot encode profile key")?;
        let mut audit = Audit {
            events: None,
            file: self
                .log
                .try_clone()
                .map_err(|e| format!("cannot open log: {e}"))?,
            size: self
                .log
                .metadata()
                .map_err(|e| format!("cannot inspect log: {e}"))?
                .len(),
            model,
            count: 0,
            kinds: BTreeMap::new(),
        };
        if let Some(note) = crate::report::chat_command(last_user(&body)) {
            let text = match crate::report::chat_reply(
                self.turns.turns(),
                self.cfg.no_report_buffer,
                note,
                self.cfg.profile_dir.as_deref(),
            ) {
                Ok(text) => text,
                Err(error) => format!("Pilot report: Could not save a report: {error}. Try `dutchman-pilot report` in the CLI."),
            };
            audit.record(
                "pilot_report_reply",
                &Value::Null,
                &json!({"from_model":false,"text":text}),
            )?;
            return direct::reply(&body, text, audit.count);
        }
        if !self.profiles.contains_key(&key) && self.profiles.len() >= MAX_PROFILES {
            return Err("profile limit of 256 models reached".into());
        }
        let mut profile = self.profiles.remove(&key).unwrap_or_else(|| Profile {
            upstream: self.cfg.upstream.clone(),
            model: audit.model.clone(),
            ..Profile::default()
        });
        profile.trim_enabled = !self.cfg.no_trim;
        profile.offer_recipe_tools = self.cfg.offer_recipe_tools;
        profile.trim_keep = self.cfg.trim_keep;
        let mut captured = if self.cfg.no_report_buffer {
            None
        } else {
            audit.events = Some(Vec::new());
            Some(crate::capture::Turn::from_request(
                &body,
                &self.catalog,
                json!({
                    "planner":self.cfg.planner,"direct_recipes":self.cfg.direct_recipes,
                    "fallback_prompt":format!("{:?}",self.cfg.fallback_prompt),"prompt_help":self.cfg.prompt_help,
                    "offer_recipe_tools":self.cfg.offer_recipe_tools,"no_shortcuts":self.cfg.no_shortcuts,"no_fill":self.cfg.no_fill,"no_how":self.cfg.no_how,"no_hints":self.cfg.no_hints,"no_report_buffer":self.cfg.no_report_buffer,
                    "trim_enabled":!self.cfg.no_trim,"trim_cap":self.cfg.trim_cap,"trim_keep":self.cfg.trim_keep,
                    "model_settings_enabled":self.cfg.model_settings_enabled || self.cfg.model_settings.is_some(),
                    "seed":self.cfg.seed,"upstream_timeout":self.cfg.upstream_timeout.as_secs(),
                    "upstream":crate::capture::bounded(&self.cfg.upstream),"listen":self.cfg.listen.to_string(),
                    "log":self.cfg.log.as_ref().map(|p|crate::capture::bounded(&p.to_string_lossy())),
                    "profile_dir":self.cfg.profile_dir.as_ref().map(|p|crate::capture::bounded(&p.to_string_lossy()))
                }),
            )?)
        };
        let text_before = profile.text_written;
        let history = recovery::history(&body)?;
        let mut conversation = self
            .conversations
            .iter()
            .rposition(|c| c.model == profile.model && history.starts_with(&c.history))
            .map(|i| self.conversations.remove(i))
            .unwrap_or_default();
        conversation.model.clone_from(&profile.model);
        conversation.history = history;
        let result = self.turn(
            &request,
            body,
            &mut profile,
            &mut audit,
            deadline,
            &mut conversation,
        );
        crate::profile::record_events(&mut profile, &audit.kinds);
        if self.conversations.len() >= 256 {
            self.conversations.remove(0);
        }
        self.conversations.push(conversation);
        if audit.kinds.contains_key("direct_recipe_calls")
            || audit.kinds.contains_key("direct_recipe_reply")
        {
            profile.direct_turns = profile.direct_turns.saturating_add(1);
        } else if self.cfg.upstream != "none" {
            profile.turns = profile.turns.saturating_add(1);
            profile
                .recent_text_calls
                .push(profile.text_written > text_before);
        }
        if profile.recent_text_calls.len() > 5 {
            profile.recent_text_calls.remove(0);
        }
        self.profiles.insert(key, profile);
        let mut reply = match self.save_profiles() {
            Ok(()) => result
                .unwrap_or_else(|message| Reply::error(502, "upstream_protocol_error", &message)),
            Err(message) => Reply::error(500, "profile_error", &message),
        };
        reply.rewrites = audit.count;
        if let Some(turn) = &mut captured {
            for event in audit.events.take().unwrap_or_default() {
                turn.event(&event.kind, event.facts);
            }
            let response = if reply.content_type.contains("text/event-stream") {
                collect_sse(&reply.body).unwrap_or(Value::Null)
            } else {
                serde_json::from_slice(&reply.body).unwrap_or(Value::Null)
            };
            turn.finish(&response, reply.status);
        }
        if let Some(turn) = captured {
            self.turns.push(turn);
        }
        Ok(reply)
    }
    fn save_profiles(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.profiles)
            .map_err(|e| format!("cannot encode profiles: {e}"))?;
        if bytes.len() > 2 * MAX_BODY {
            return Err("profile file exceeds 2 MiB".into());
        }
        let temp = self.profile_path.with_extension("json.tmp");
        fs::write(&temp, bytes)
            .and_then(|()| fs::rename(&temp, &self.profile_path))
            .map_err(|e| format!("cannot save profiles: {e}"))
    }
    fn probe(&self, profile: &mut Profile, auth: Option<&str>, deadline: Instant) {
        if profile.capabilities_checked {
            return;
        }
        profile.capabilities_checked = true;
        // The same configured origin only; unsupported /api/show is harmless.
        let Some(root) = self.cfg.upstream.strip_suffix("/v1") else {
            return;
        };
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(3));
        if remaining.is_zero() {
            return;
        }
        let mut request = self
            .agent
            .post(&format!("{root}/api/show"))
            .timeout(remaining)
            .set("Content-Type", "application/json");
        if let Some(auth) = auth {
            request = request.set("Authorization", auth);
        }
        let Ok(response) = request.send_string(&json!({"model":profile.model}).to_string()) else {
            return;
        };
        let Ok(bytes) = read_bounded(response.into_reader(), MAX_BODY) else {
            return;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return;
        };
        if let Some(capabilities) = value["capabilities"].as_array() {
            profile.ollama = true;
            profile.native_support = Some(capabilities.iter().any(|v| v == "tools"));
        }
    }
    fn turn(
        &self,
        request: &Request,
        mut body: Value,
        profile: &mut Profile,
        audit: &mut Audit,
        deadline: Instant,
        conversation: &mut Conversation,
    ) -> Result<Reply, String> {
        let form_allowed = self.form_turns.borrow_mut().claim(&body)?;
        let before_strip = body.clone();
        shortcuts::strip(&mut body);
        let stripped = body != before_strip;
        if stripped {
            audit.record("shortcut_history_stripped", &before_strip, &body)?;
        }
        let original = body.clone();
        let client: Value = serde_json::from_slice(&request.body)
            .map_err(|_| "invalid request JSON for size audit")?;
        if self.cfg.solidcraft98 {
            set_logged(
                &mut body,
                "tools",
                json!(crate::solidcraft98::drive::chat_tools()),
                "solidcraft98_tools",
                audit,
            )?;
        }
        let user_index = body["messages"]
            .as_array()
            .and_then(|m| m.iter().rposition(|m| m["role"] == "user"))
            .unwrap_or(0);
        let pending_form = conversation
            .form_pending
            .take()
            .filter(|(i, _, _, _)| *i == user_index);
        if conversation.sketch_hint_index != user_index {
            conversation.sketch_hint_index = user_index;
            conversation.pattern_inspections = 0;
        }
        if conversation
            .recipe
            .as_ref()
            .is_some_and(|(index, _)| *index != user_index)
        {
            conversation.recipe = None;
            conversation.completion_checked = false;
            conversation.pending_recipe = None;
            conversation.direct = None;
            conversation.direct_failed = false;
        }
        let mut recipe = if pending_form.is_some() {
            None
        } else if self.cfg.direct_recipes {
            planner::plan_direct(&body, &self.catalog)?
        } else if self.cfg.planner {
            planner::plan(&body, &self.catalog)?
        } else {
            None
        };
        let shortcut_active = conversation
            .direct
            .as_ref()
            .is_some_and(|(i, _, _)| *i == user_index);
        if let Some((i, args)) = &conversation.shortcut_args {
            if *i == user_index
                && conversation.direct.as_ref().is_some_and(|(_, p, _)| {
                    matches!(p.rule, "resolve_joint" | "resolve_pattern" | "resolve_body")
                })
            {
                recipe = if args.get("recipe").is_some() {
                    self.recipes.build(args, &body, &self.catalog).ok()
                } else {
                    planner::shortcut(args, &body, &self.catalog).ok()
                };
            }
        }
        let mut resumed = false;
        if (self.cfg.planner || self.cfg.direct_recipes || shortcut_active) && recipe.is_none() {
            if let Some(pending) = &conversation.pending_recipe {
                recipe = planner::resume(&body, pending, &self.catalog)?;
                resumed = recipe.is_some();
            }
        }
        if self.cfg.direct_recipes
            && pending_form.is_none()
            && recipe.is_none()
            && !conversation.direct_failed
        {
            recipe = planner::inspect_for_edit(&body, &self.catalog)?;
        }
        if self.cfg.direct_recipes || shortcut_active {
            if let Some(reply) =
                self.direct_turn(&original, &recipe, user_index, conversation, audit)?
            {
                return Ok(reply);
            }
        }
        if conversation.direct_failed {
            recipe = None;
        }
        if let Some(reason) = conversation.direct_handback.take() {
            body["messages"]
                .as_array_mut()
                .ok_or("messages must be an array")?
                .push(json!({"role":"system","content":reason}));
            profile.hand_backs = profile.hand_backs.saturating_add(1);
        }
        if self.cfg.planner
            && self.cfg.direct_recipes
            && recipe.is_none()
            && !conversation.direct_failed
        {
            if let Some((plan, rest)) = planner::partial_plan(&original, &self.catalog)? {
                let message = format!("Nothing has been done yet. You must submit tool calls for the whole request and verify their results before answering. For the matched leading part, you may submit these computed calls: {}. You must also handle this remaining request: {}. All calls require client approval and tool results.", plan.message(), rest);
                body["messages"]
                    .as_array_mut()
                    .ok_or("messages must be an array")?
                    .push(json!({"role":"system","content":message}));
                audit.record("direct_recipe_fallback", &Value::Null, &json!({"reason":"multiple intents","offered":plan,"remaining":rest,"from_model":false}))?;
            }
        }
        if self.cfg.upstream == "none" {
            return direct::reply(&original, direct::unavailable(), audit.count);
        }
        if let Some(plan) = &recipe {
            conversation.pending_recipe = plan.continuation.as_ref().map(|_| plan.clone());
            if let Some(expected) = &plan.expected {
                conversation.recipe = Some((user_index, expected.clone()));
                conversation.completion_checked = false;
            }
            let message = plan.message();
            body["messages"]
                .as_array_mut()
                .ok_or("messages must be an array")?
                .push(json!({"role":"system","content":message}));
            audit.record(
                    if resumed { "recipe_continue" } else { "plan" },
                    &Value::Null,
                    &json!({"request":last_user(&body),"rule":plan.rule,"slots":plan.slots,"expected":plan.expected,"calls":plan.calls,"prompt":message}),
                )?;
        }
        let accepted = recovery::previous_accepted(&body);
        if !self.cfg.no_how {
            let schema = guidance::how_tool();
            let mut tools = body
                .get("tools")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            tools.retain(|t| t["function"]["name"] != "how");
            if tools.len() >= 128 {
                return Err("cannot inject how: tools would exceed 128 entries".into());
            }
            tools.push(json!({"type":"function","function":{"name":schema["name"],"description":schema["description"],"parameters":schema["inputSchema"]}}));
            set_logged(&mut body, "tools", json!(tools), "how_tool", audit)?;
        }
        let shortcuts_enabled = !self.cfg.no_shortcuts
            && !conversation.direct_failed
            && !self.cfg.solidcraft98
            && shortcuts::supported(&body);
        let mut shortcuts_offered = self.cfg.offer_recipe_tools
            && !self.cfg.no_shortcuts
            && !conversation.direct_failed
            && !self.cfg.solidcraft98
            && shortcuts::offer(&mut body)?;
        if shortcuts_offered {
            audit.record(
                "shortcut_tool",
                &Value::Null,
                &crate::recipe_library::build_tool(),
            )?;
        }
        let tools = body
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let defaults = ModelSettings::default();
        let settings = self
            .settings
            .get(&profile.model)
            .or_else(|| self.cfg.model_settings_enabled.then_some(&defaults));
        if !tools.is_empty() || settings.is_some() {
            self.probe(profile, request.auth.as_deref(), deadline);
        }
        let settings = settings.filter(|_| profile.ollama);
        let stream = body["stream"].as_bool().unwrap_or(false);
        let include_usage = body["stream_options"]["include_usage"] == true;
        if stream {
            body["stream"] = json!(false);
            if let Some(object) = body.as_object_mut() {
                object.remove("stream_options");
            }
            audit.record("buffer_stream", &json!(true), &json!(false))?;
        }
        let fallback_rule = match self.cfg.fallback_prompt {
            FallbackPrompt::On => Some("forced_on"),
            FallbackPrompt::Off => None,
            FallbackPrompt::Auto if profile.ollama && profile.native_support == Some(false) => {
                Some("capabilities_lack_tools")
            }
            FallbackPrompt::Auto
                if profile
                    .recent_text_calls
                    .iter()
                    .rev()
                    .take(5)
                    .filter(|v| **v)
                    .count()
                    >= 2 =>
            {
                Some("text_calls_two_of_last_five")
            }
            FallbackPrompt::Auto => None,
        };
        let fallback_rule = fallback_rule.filter(|_| !tools.is_empty());
        let already = body["messages"].as_array().is_some_and(|messages| {
            messages.iter().any(|m| {
                m["role"] == "system" && m["content"].as_str().is_some_and(|s| s.contains(FALLBACK))
            })
        });
        if let Some(block) = prompting::system_block(
            &body,
            &self.catalog,
            self.cfg.prompt_help,
            fallback_rule.is_some() && !already,
        )? {
            body["messages"]
                .as_array_mut()
                .ok_or("messages must be an array")?
                .insert(0, json!({"role":"system","content":block}));
            if self.cfg.prompt_help {
                audit.record("prompt_help", &Value::Null, &json!(block))?;
            }
            if let Some(rule) = fallback_rule.filter(|_| !already) {
                audit.record(
                    "fallback_prompt",
                    &Value::Null,
                    &json!({"rule":rule,"prompt":FALLBACK}),
                )?;
            }
        }
        for (key, value) in
            prompting::settings_additions(&body, profile.ollama, settings, self.cfg.seed)?
        {
            let before = body.get(key).cloned().unwrap_or(Value::Null);
            if before != value {
                audit.record(
                    "model_settings",
                    &json!({"field":key,"value":before}),
                    &json!({"field":key,"value":value}),
                )?;
                body[key] = value;
            }
        }
        if !self.cfg.no_hints {
            if !conversation.facts_seen[0] && prompting::append_placement_fact(&mut body)? {
                audit.record(
                    "placement_fact",
                    &Value::Null,
                    &json!(prompting::PLACEMENT_FACT),
                )?;
            }
            if !conversation.facts_seen[1] && prompting::append_sketch_fact(&mut body)? {
                audit.record("sketch_fact", &Value::Null, &json!(prompting::SKETCH_FACT))?;
            }
            if !conversation.facts_seen[2] && prompting::append_sketch_axes(&mut body)? {
                audit.record(
                    "sketch_axes_fact",
                    &Value::Null,
                    &json!(prompting::SKETCH_AXES),
                )?;
            }
            append_plane_hints(&mut body, &mut conversation.plane_hints_seen, audit)?;
            append_edge_hints(&mut body, &mut conversation.edge_hint_seen, audit)?;
            append_hints(
                &mut body,
                &self.catalog,
                audit,
                &mut conversation.sketch_hints_seen,
                &mut conversation.next_hints_seen,
            )?;
            if !conversation.pattern_hint_seen {
                if let Some(hint) = planner::repetition_hint(&body) {
                    conversation.pattern_hint_seen = true;
                    let present = body["messages"].as_array().is_some_and(|messages| {
                        messages
                            .iter()
                            .any(|m| prompting::message_text(&m["content"]).contains(hint))
                    });
                    if !present {
                        if let Some(messages) = body["messages"].as_array_mut() {
                            messages.push(json!({"role":"system","content":format!("{} {hint}",crate::identity::HINT)}));
                            audit.record("pattern_hint", &Value::Null, &json!(hint))?;
                        }
                    }
                }
            }
            if !conversation.joint_hint_seen {
                if let Some(hint) = planner::joint_hint(&body) {
                    conversation.joint_hint_seen = true;
                    let present = body["messages"].as_array().is_some_and(|messages| {
                        messages
                            .iter()
                            .any(|m| prompting::message_text(&m["content"]).contains(hint))
                    });
                    if !present {
                        if let Some(messages) = body["messages"].as_array_mut() {
                            messages.push(json!({"role":"system","content":format!("{} {hint}",crate::identity::HINT)}));
                            audit.record("joint_contact_hint", &Value::Null, &json!(hint))?;
                        }
                    }
                }
            }
            for (index, fact) in [
                prompting::PLACEMENT_FACT,
                prompting::SKETCH_FACT,
                prompting::SKETCH_AXES,
            ]
            .iter()
            .enumerate()
            {
                // Match decoded message text as JSON escapes quotes inside facts.
                if body["messages"].as_array().is_some_and(|messages| {
                    messages
                        .iter()
                        .any(|m| prompting::message_text(&m["content"]).contains(fact))
                }) {
                    conversation.facts_seen[index] = true;
                }
            }
        }
        let mut text_mode = profile.text_tool_mode || profile.native_support == Some(false);
        let (continued_band, context_round) = if let Some((_, band, emitted, rounds)) = pending_form
        {
            if !fill::inspection_results(&original, &emitted) {
                return direct::reply(&original, "Form error: design inspection failed or its results are incomplete. Inspect the target again before requesting a form.".into(), audit.count);
            }
            (Some(band), rounds)
        } else {
            (None, 0)
        };
        let band = if let Some(band) = continued_band {
            band
        } else if shortcuts_enabled && !self.cfg.no_fill && form_allowed && recipe.is_none() {
            self.recipes.form_band(last_user(&original))?
        } else {
            json!({"band":"low"})
        };
        let all_candidates = band["recipes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let candidates =
            &all_candidates[..all_candidates
                .len()
                .min(if band["band"] == "fill" { 1 } else { 4 })];
        if matches!(band["band"].as_str(), Some("fill" | "route"))
            && candidates.iter().any(fill::needs_names)
        {
            let joint = candidates
                .iter()
                .take(4)
                .any(|r| r["id"] == "revolute_joint");
            let reads = planner::form_context::reads(&original, joint, &self.catalog)?;
            if !reads.is_empty() {
                if context_round >= 3 {
                    return direct::reply(&original, "Form error: design inspection limit reached; select a target and inspect it again.".into(), audit.count);
                }
                let calls: Vec<_> = reads.into_iter().enumerate().map(|(n,c)| json!({"id":format!("pilot_context_{user_index}_{context_round}_{n}"),"type":"function","function":{"name":c["tool"],"arguments":c["arguments"].to_string()}})).collect();
                let message = json!({"role":"assistant","content":null,"tool_calls":calls});
                conversation.form_pending =
                    Some((user_index, band, message.clone(), context_round + 1));
                audit.record("form_inspect", &Value::Null, &message)?;
                return direct::native(&original, message, audit.count);
            }
        }
        let mut fill_recipe = (band["band"] == "fill").then(|| band["recipes"][0].clone());
        let mut route_candidates = if band["band"] == "route" {
            band["recipes"].as_array().cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        if !route_candidates.is_empty() {
            let choices: Vec<_> = route_candidates
                .iter()
                .map(|r| json!({"id":r["id"],"score":r["score"]}))
                .collect();
            audit.record("route_offered", &Value::Null, &json!({"recipes":choices}))?;
        }
        if let Some(found) = &fill_recipe {
            audit.record(
                "fill_offered",
                &Value::Null,
                &json!({"recipe":found["id"],"score":found["score"]}),
            )?;
        }
        if text_mode && fill_recipe.is_none() && route_candidates.is_empty() {
            text_mode::activate(&mut body, &tools, "profile_or_capabilities", audit)?;
        }
        let mut fill_error: Option<String> = None;
        let mut form_reply_retried = false;
        let mut fill_attempts = 0;
        let mut route_attempts = 0;
        let mut form_attempts = 0;
        let mut empty_retried = false;
        let mut parse_retried = false;
        let mut hand_backs = 0;
        let mut guard_backs = 0;
        let mut how_answers = 0;
        let mut last_reason = String::new();
        let mut mode_retry = 0;
        let mut shortcut_retries = 0;
        let mut find_count = 0;
        // Freeze the current answer's result block before retry nudges append
        // assistant/user messages; those nudges must not make its evidence old.
        let answering = history::answering_start(&body);
        for wire_attempt in 0..=7 {
            if form_attempts >= 4 && (fill_recipe.is_some() || !route_candidates.is_empty()) {
                fill_recipe = None;
                route_candidates.clear();
                audit.record(
                    "fill_fallback",
                    &Value::Null,
                    &json!({"reason":"four upstream form calls exhausted"}),
                )?;
                if text_mode {
                    text_mode::activate(&mut body, &tools, "form_cap", audit)?;
                }
            }
            let attempt = wire_attempt - mode_retry - fill_attempts - route_attempts;
            if attempt > 3 {
                break;
            }
            // All deterministic readers above and below retain the full body. Conservatively
            // keep the wire full too while any recipe continuation/verification is pending.
            let protected = conversation.pending_recipe.is_some()
                || conversation.recipe.is_some()
                || conversation.direct.is_some()
                || fill_recipe.is_some()
                || !route_candidates.is_empty();
            let mut outgoing = body.clone();
            let changed = if self.cfg.no_trim {
                0
            } else {
                history::trim(
                    &mut outgoing,
                    self.cfg.trim_cap,
                    self.cfg.trim_keep,
                    protected,
                    answering,
                )
            };
            let trimmed_chars = history::chars(&body).saturating_sub(history::chars(&outgoing));
            if !route_candidates.is_empty() {
                outgoing = client.clone();
                fill::offer_route(&mut outgoing, &route_candidates)?;
                if let Some(error) = &fill_error {
                    fill::retry_note(&mut outgoing, error);
                }
            } else if let Some(found) = &fill_recipe {
                outgoing = client.clone();
                fill::offer(&mut outgoing, found, fill_error.as_deref())?;
            } else if text_mode {
                outgoing = text_mode::wire(&outgoing, audit)?;
            }
            history::audit_request(
                &client,
                if fill_recipe.is_some() || !route_candidates.is_empty() {
                    &outgoing
                } else {
                    &body
                },
                &outgoing,
                !self.cfg.no_trim,
                (changed, trimmed_chars),
                profile,
                audit,
            )?;
            let wire = json_bytes(&outgoing)?;
            if fill_recipe.is_some() || !route_candidates.is_empty() {
                audit.record("form_request", &Value::Null, &json!({"bytes":wire.len(),"characters":history::chars(&outgoing),"tool":outgoing["tools"][0]["function"]["name"]}))?;
                form_attempts += 1;
            }
            let mut upstream = match self.fetch(
                "POST",
                "/chat/completions",
                request.auth.as_deref(),
                &wire,
                deadline,
            ) {
                Ok(response) => response,
                Err(message) => {
                    let reply = Reply::error(502, "upstream_unavailable", &message);
                    audit.record(
                        "provider_error",
                        &json!({"status":502}),
                        &serde_json::from_slice::<Value>(&reply.body)
                            .map_err(|_| "invalid internal error")?,
                    )?;
                    return Ok(reply);
                }
            };
            if (!text_mode || fill_recipe.is_some() || !route_candidates.is_empty())
                && !tools.is_empty()
                && text_mode::rejected(upstream.status, &upstream.body)
            {
                if fill_recipe.take().is_some() {
                    audit.record(
                        "fill_fallback",
                        &Value::Null,
                        &json!({"reason":"provider rejected tools"}),
                    )?;
                }
                if !route_candidates.is_empty() {
                    route_candidates.clear();
                    audit.record(
                        "route_none",
                        &Value::Null,
                        &json!({"reason":"provider rejected tools"}),
                    )?;
                }
                profile.text_tool_mode = true;
                profile.native_support = Some(false);
                text_mode = true;
                mode_retry = 1;
                text_mode::activate(&mut body, &tools, "provider_400_unsupported_tools", audit)?;
                continue;
            }
            let mut recovered = false;
            if let Some(raw) = recovery::ollama_raw(&upstream).filter(|_| !self.cfg.solidcraft98) {
                if let Some(response) =
                    recovery::recover(&raw, &tools, &self.catalog, audit, profile)?
                {
                    audit.record("ollama 500 recovered", &json!(raw), &response)?;
                    upstream.status = 200;
                    upstream.content_type = "application/json".into();
                    upstream.body = json_bytes(&response)?;
                    recovered = true;
                } else if !parse_retried && attempt < 3 {
                    parse_retried = true;
                    audit.record(
                        "ollama 500 retried",
                        &json!({"status":500}),
                        &json!({"retry":1}),
                    )?;
                    continue;
                }
            }
            if upstream.status == 500 && !self.cfg.solidcraft98 && !parse_retried && attempt < 3 {
                parse_retried = true;
                audit.record(
                    "provider 500 retried",
                    &json!({"status":500,"body":String::from_utf8_lossy(&upstream.body).chars().take(300).collect::<String>()}),
                    &json!({"retry":1}),
                )?;
                continue;
            }
            if !(200..300).contains(&upstream.status) {
                let reply = provider_error(&upstream);
                let mut before = json!({"status":upstream.status});
                if upstream.status == 500 {
                    before["body"] = json!(String::from_utf8_lossy(&upstream.body)
                        .chars()
                        .take(300)
                        .collect::<String>());
                }
                audit.record(
                    "provider_error",
                    &before,
                    &serde_json::from_slice::<Value>(&reply.body)
                        .map_err(|_| "invalid internal error")?,
                )?;
                return Ok(reply);
            }
            let mut response = if upstream.content_type.starts_with("text/event-stream") {
                collect_sse(&upstream.body)?
            } else {
                serde_json::from_slice::<Value>(&upstream.body)
                    .map_err(|_| "upstream did not return completion JSON")?
            };
            repair::check_value(&response)?;
            let offered = if !route_candidates.is_empty() {
                Some("pilot_route")
            } else if fill_recipe.is_some() {
                Some("pilot_fill")
            } else {
                None
            };
            if let Some(offered) = offered {
                let off_form = fill::off_form(&response, offered);
                let wrong_shape = response["choices"]
                    .as_array()
                    .is_none_or(|cs| cs.len() != 1)
                    || response["choices"][0]["message"]["tool_calls"]
                        .as_array()
                        .is_none_or(|cs| cs.len() != 1 || cs[0]["function"]["name"] != offered);
                if !off_form.is_empty() || wrong_shape {
                    let problem = if off_form.is_empty() {
                        format!("Reply contained text or multiple calls; call exactly one {offered} tool.")
                    } else {
                        format!(
                            "Called unoffered tool(s) {}; call exactly one {offered} tool.",
                            off_form
                                .iter()
                                .take(8)
                                .copied()
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    for tool in off_form {
                        audit.record(
                            "fill_offform",
                            &Value::Null,
                            &json!({"tool":tool,"offered":offered}),
                        )?;
                    }
                    if offered == "pilot_route" {
                        route_attempts += 1;
                    } else {
                        fill_attempts += 1;
                    }
                    if !form_reply_retried
                        && form_attempts < 4
                        && (offered == "pilot_route" || fill_attempts < 3)
                    {
                        form_reply_retried = true;
                        fill_error = Some(problem);
                        audit.record("fill_retry", &Value::Null, &json!({"reason":fill_error}))?;
                        continue;
                    }
                    fill_recipe = None;
                    route_candidates.clear();
                    audit.record(
                        "fill_fallback",
                        &Value::Null,
                        &json!({"reason":"off-form or text reply after retry"}),
                    )?;
                    if text_mode {
                        text_mode::activate(&mut body, &tools, "fill_offform", audit)?;
                    }
                    continue;
                }
            }
            if !route_candidates.is_empty() {
                route_attempts += 1;
                fill_recipe = fill::picked(&response, &route_candidates);
                route_candidates.clear();
                fill_error = None;
                form_reply_retried = false;
                if let Some(found) = &fill_recipe {
                    audit.record("route_picked", &response, &json!({"recipe":found["id"]}))?;
                    audit.record(
                        "fill_offered",
                        &Value::Null,
                        &json!({"recipe":found["id"],"score":found["score"]}),
                    )?;
                } else {
                    audit.record(
                        "route_none",
                        &response,
                        &json!({"reason":"none or invalid selection"}),
                    )?;
                    if text_mode {
                        text_mode::activate(&mut body, &tools, "route_none", audit)?;
                    }
                }
                continue;
            }
            if let Some(choices) = response["choices"].as_array() {
                for choice in choices.iter().take(8) {
                    audit.capture(
                        "upstream finish",
                        &json!({"finish_reason":choice["finish_reason"]}),
                    );
                }
            }
            if let Some(found) = &fill_recipe {
                fill_attempts += 1;
                let response = match fill::coerced(&response, found) {
                    Ok((r, changes)) => {
                        for change in changes {
                            audit.record(
                                if change["defaulted"] == true {
                                    "fill_defaulted"
                                } else {
                                    "fill_coerced"
                                },
                                &change["before"],
                                &json!({"field":change["field"],"value":change["after"]}),
                            )?;
                        }
                        r
                    }
                    Err(_) => response.clone(),
                };
                match fill::accept(&response, found, &original, &self.recipes, &self.catalog) {
                    fill::Outcome::Build(args, plan) => {
                        let raw = &response["choices"][0]["message"]["tool_calls"][0]["function"]
                            ["arguments"];
                        let supplied: Value = raw
                            .as_str()
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or_else(|| raw.clone());
                        if supplied.get("body").is_none() && args["args"].get("body").is_some() {
                            audit.record("fill_defaulted", &Value::Null, &json!({"field":"body","value":args["args"]["body"],"rule":"selection, last created body, only body"}))?;
                        }
                        audit.record(
                            "fill_accepted",
                            &args,
                            &json!({"recipe":found["id"],"rule":plan.rule,"attempt":fill_attempts,"max_attempts":3}),
                        )?;
                        audit.record("shortcut", &args, &json!({"args":args,"rule":plan.rule}))?;
                        conversation.shortcut_args = Some((user_index, args));
                        if let Some(reply) = self.direct_turn(
                            &original,
                            &Some(*plan),
                            user_index,
                            conversation,
                            audit,
                        )? {
                            return Ok(reply);
                        }
                        return Err("pilot_fill: recipe could not emit calls".into());
                    }
                    fill::Outcome::Invalid(reason) => {
                        audit.record(
                            "fill_invalid",
                            &response,
                            &json!({"recipe":found["id"],"reason":reason,"attempt":fill_attempts,"max_attempts":3}),
                        )?;
                        if form_attempts < 4 && fill_attempts < 3 {
                            fill_error = Some(reason);
                            continue;
                        }
                    }
                    fill::Outcome::Decline => {
                        audit.record(
                            "fill_declined",
                            &Value::Null,
                            &json!({"recipe":found["id"]}),
                        )?;
                    }
                    fill::Outcome::Fallback => {}
                }
                audit.record("fill_fallback", &Value::Null, &json!({"recipe":found["id"],"attempts":fill_attempts,"max_attempts":3,"reason":"form declined, invalid three times, or not called"}))?;
                fill_recipe = None;
                if text_mode {
                    text_mode::activate(&mut body, &tools, "fill_fallback", audit)?;
                }
                continue;
            }
            // Text-mode local calls need interception before ordinary call guards.
            if let Some(choices) = response["choices"].as_array_mut() {
                for choice in choices.iter_mut().take(8) {
                    let message = &mut choice["message"];
                    if message["tool_calls"]
                        .as_array()
                        .is_none_or(|cs| cs.is_empty())
                        && message["content"]
                            .as_str()
                            .is_some_and(|s| s.contains("pilot_build"))
                    {
                        let mut converted = message.clone();
                        if convert_text(
                            &mut converted,
                            &[
                                crate::recipe_library::build_tool(),
                                crate::recipe_library::find_tool(),
                            ],
                            &self.catalog,
                            audit,
                        )? && converted["tool_calls"].as_array().is_some_and(|cs| {
                            cs.iter().any(|c| {
                                matches!(
                                    c["function"]["name"].as_str(),
                                    Some("pilot_build" | "pilot_find")
                                )
                            })
                        }) {
                            *message = converted;
                        }
                    }
                }
            }
            if shortcuts::contains_find(&response) {
                if !shortcuts_offered || find_count >= 2 {
                    return direct::reply(&original, "pilot_find unavailable or search limit reached; ask one question or describe supported recipes.".into(), audit.count);
                }
                find_count += 1;
                if let Err(reason) = shortcuts::find(&mut body, &response, &self.recipes, audit) {
                    shortcuts::reject(&mut body, &response, &reason)?;
                }
                continue;
            }
            if shortcuts::contains(&response) {
                let result = shortcuts::expand(
                    &response,
                    &original,
                    &self.catalog,
                    &self.recipes,
                    shortcuts_offered,
                );
                match result {
                    Ok((args, plan)) => {
                        audit.record("shortcut", &args, &json!({"args":args,"rule":plan.rule}))?;
                        conversation.shortcut_args = Some((user_index, args));
                        if let Some(reply) = self.direct_turn(
                            &original,
                            &Some(plan),
                            user_index,
                            conversation,
                            audit,
                        )? {
                            return Ok(reply);
                        }
                        return Err("shortcut: recipe could not emit calls".into());
                    }
                    Err(reason) => {
                        audit.record("shortcut_rejected", &response, &json!({"reason":reason}))?;
                        if attempt >= 3 || !shortcuts_offered {
                            return direct::reply(&original, format!("pilot_build rejected: {reason}. Shortcut retry limit reached; no changes were sent."), audit.count);
                        }
                        if shortcut_retries >= 2 {
                            shortcuts_offered = false;
                            shortcuts::strip(&mut body);
                            body["messages"].as_array_mut().ok_or("messages must be an array")?.push(json!({"role":"system","content":format!("pilot_build unavailable for this turn after two retries: {reason}. Use the real tools or explain the problem.")}));
                            continue;
                        }
                        shortcut_retries += 1;
                        shortcuts::reject(&mut body, &response, &reason)?;
                        continue;
                    }
                }
            }
            let before = response.clone();
            let choices = response["choices"]
                .as_array_mut()
                .filter(|c| !c.is_empty() && c.len() <= 8)
                .ok_or("upstream choices must contain 1 to 8 entries")?;
            let mut nudge = None;
            let single_choice = choices.len() == 1;
            for choice in choices.iter_mut() {
                let message = choice
                    .get_mut("message")
                    .filter(|m| m.is_object())
                    .ok_or("upstream choice is missing an assistant message")?;
                let raw_message = message.clone();
                let had_native = message["tool_calls"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty());
                if !recovered {
                    if had_native {
                        profile.native = profile.native.saturating_add(1);
                        profile.native_support = Some(true);
                    } else if if self.cfg.solidcraft98 {
                        convert98(message, audit)?
                    } else {
                        (!text_mode
                            || repair::single_call_text(message["content"].as_str().unwrap_or("")))
                            && convert_text(message, &tools, &self.catalog, audit)?
                    } {
                        if text_mode {
                            audit.record("text_call_converted", &raw_message, message)?;
                        }
                        profile.text_written = profile.text_written.saturating_add(1);
                    } else if is_empty(message) {
                        profile.empty = profile.empty.saturating_add(1);
                    } else {
                        profile.prose = profile.prose.saturating_add(1);
                        if single_choice {
                            if let Some(plan) = recipe.as_ref().filter(|p| p.expected.is_some()) {
                                if planner::matches_prose(&original, &self.catalog, plan, message)?
                                {
                                    let mut planned = plan.calls.clone();
                                    let has_batch =
                                        tools.iter().any(|t| t["function"]["name"] == "batch");
                                    let inspect = planned.last().is_some_and(|c| {
                                        c["arguments"]["command"] == "document.inspect"
                                    });
                                    if has_batch && planned.len() > 1 + usize::from(inspect) {
                                        let verification =
                                            if inspect { planned.pop() } else { None };
                                        let commands = planned
                                            .iter()
                                            .map(|c| c["arguments"].clone())
                                            .collect::<Vec<_>>();
                                        planned = vec![
                                            json!({"tool":"batch","arguments":{"commands":commands}}),
                                        ];
                                        if let Some(verification) = verification {
                                            planned.push(verification);
                                        }
                                    }
                                    let calls = planned.iter().enumerate().map(|(i,c)| json!({"id":format!("pilot_recipe_{user_index}_{i}"),"type":"function","function":{"name":c["tool"],"arguments":c["arguments"].to_string()}})).collect::<Vec<_>>();
                                    let before = message.clone();
                                    message["content"] = Value::Null;
                                    message["tool_calls"] = json!(calls);
                                    audit.record("recipe_prose", &before, &json!({"rule":plan.rule,"slots":plan.slots,"message":message}))?;
                                }
                            }
                        }
                    }
                }
                if self.cfg.solidcraft98 {
                    aliases98(message, audit)?;
                }
                let guard = repair_calls(
                    message,
                    &tools,
                    &self.catalog,
                    last_user(&body),
                    audit,
                    profile,
                )?;
                if message["tool_calls"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty())
                    && choice["finish_reason"] != "tool_calls"
                {
                    audit.record(
                        "finish_reason",
                        &choice["finish_reason"],
                        &json!("tool_calls"),
                    )?;
                    choice["finish_reason"] = json!("tool_calls");
                }
                // Multiple alternatives are rewritten independently; no ambiguous retry history.
                if before["choices"].as_array().is_some_and(|c| c.len() == 1) {
                    nudge = guard;
                }
            }
            let message = &response["choices"][0]["message"];
            if !self.cfg.no_how
                && response["choices"]
                    .as_array()
                    .is_some_and(|choices| choices.iter().any(|c| has_how(&c["message"])))
            {
                if response["choices"].as_array().is_none_or(|c| c.len() != 1) {
                    return Err("local how requires a single completion choice".into());
                }
                let calls: Vec<_> = message["tool_calls"]
                    .as_array()
                    .ok_or("how call list missing")?
                    .iter()
                    .filter(|c| c["function"]["name"] == "how")
                    .cloned()
                    .collect();
                // `how` is a lookup the model asked for, not a Pilot hand-back
                // correcting it, so an accepted previous call does not block it.
                if how_answers + calls.len() > 2
                    || hand_backs + calls.len() > 2
                    || attempt == 3
                    || !conversation.allow("how", calls.len())
                {
                    return Err(
                        "local how hand-back limit reached (two per request, four per conversation)".into(),
                    );
                }
                let messages = body["messages"]
                    .as_array_mut()
                    .ok_or("messages must be an array")?;
                // Only local calls enter this history. Other calls in a mixed reply have
                // not run; the model must propose them again after reading the answer.
                let history = json!({"role":"assistant","content":null,"tool_calls":calls});
                audit.record("how_hand_back", message, &history)?;
                messages.push(history);
                for call in &calls {
                    let id = call["id"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or("how call needs an id")?;
                    let result = call["function"]["arguments"]
                        .as_str()
                        .ok_or_else(|| "how arguments must be JSON".to_string())
                        .and_then(|s| {
                            serde_json::from_str::<Value>(s)
                                .map_err(|e| format!("invalid how arguments: {e}"))
                        })
                        .and_then(|args| guidance::how_answer(&self.catalog, &args))
                        .unwrap_or_else(|e| format!("how error: {e}"));
                    audit.record("how_answer", call, &json!(result))?;
                    messages.push(json!({"role":"tool","tool_call_id":id,"content":result}));
                }
                how_answers += calls.len();
                hand_backs += calls.len();
                profile.hand_backs = profile.hand_backs.saturating_add(calls.len() as u64);
                continue;
            }
            let mut completion_nudge = false;
            if nudge.is_none()
                && self.cfg.planner
                && !conversation.completion_checked
                && response["choices"].as_array().is_some_and(|c| c.len() == 1)
                && message["tool_calls"]
                    .as_array()
                    .is_none_or(|c| c.is_empty())
            {
                if let Some((_, expected)) = &conversation.recipe {
                    let difference = if conversation.pending_recipe.is_some() {
                        Some("Recipe completion is unverified: the edge topology lookup has not completed. Run the planned lookup, finish the edge operation, and verify; do not repeat accepted changes.".into())
                    } else {
                        planner::completion(&original, expected)
                    };
                    if let Some(reason) = difference {
                        nudge = Some((reason, false));
                        completion_nudge = true;
                    }
                    conversation.completion_checked = true;
                }
            }
            if nudge.is_none() && response["choices"].as_array().is_some_and(|c| c.len() == 1) {
                if is_empty(message) && !empty_retried {
                    empty_retried = true;
                    nudge = Some(("your reply was empty; answer or call a tool".into(), false));
                } else if !is_empty(message) && !self.cfg.solidcraft98 {
                    nudge = prose_guard(&body, message, &self.catalog);
                }
            }
            if let Some((reason, json_decided)) = nudge {
                let command = recovery::failing_command(message, &reason);
                if (!accepted || completion_nudge)
                    && guard_backs < 2
                    && hand_backs < 2
                    && reason != last_reason
                    && attempt < 3
                    && conversation.allow(&command, 1)
                {
                    hand_backs += 1;
                    guard_backs += 1;
                    profile.hand_backs = profile.hand_backs.saturating_add(1);
                    last_reason.clone_from(&reason);
                    // A rejected tool call must not leave orphan tool_call ids in history.
                    let history = if message["tool_calls"]
                        .as_array()
                        .is_some_and(|c| !c.is_empty())
                    {
                        json!({"role":"assistant","content":format!("Proposed tool calls: {}", message["tool_calls"])})
                    } else {
                        message.clone()
                    };
                    let messages = body["messages"]
                        .as_array_mut()
                        .ok_or("messages must be an array")?;
                    messages.push(history);
                    messages.push(json!({"role":"user","content":reason}));
                    audit.record("hand_back", message, &json!(reason))?;
                    if json_decided && settings.is_some_and(|s| s.json_retry) {
                        set_logged(
                            &mut body,
                            "response_format",
                            json!({"type":"json_object"}),
                            "json_retry",
                            audit,
                        )?;
                    }
                    continue;
                }
            }
            let bytes = if stream {
                emit_sse(&response, include_usage)?
            } else if response == before && !upstream.content_type.starts_with("text/event-stream")
            {
                upstream.body
            } else {
                json_bytes(&response)?
            };
            return Ok(Reply {
                status: 200,
                content_type: if stream {
                    "text/event-stream"
                } else {
                    "application/json"
                }
                .into(),
                body: bytes,
                rewrites: audit.count,
                retry_after: None,
            });
        }
        Err("turn retry limit reached".into())
    }
}
fn aliases98(message: &mut Value, audit: &mut Audit) -> Result<(), String> {
    if let Some(calls) = message["tool_calls"].as_array_mut() {
        for call in calls {
            let old = call["function"]["name"].clone();
            let name = match old.as_str() {
                Some("dblclick" | "double-click" | "double_click") => Some("doubleclick"),
                Some("rclick" | "right-click" | "right_click") => Some("rightclick"),
                _ => None,
            };
            if let Some(name) = name {
                audit.record("solidcraft98_alias", &old, &json!(name))?;
                call["function"]["name"] = json!(name);
            }
        }
    }
    Ok(())
}
fn convert98(message: &mut Value, audit: &mut Audit) -> Result<bool, String> {
    // The model proxy converts explicit JSON only; COMMAND: belongs to drive98.
    let text = message["content"].as_str().unwrap_or("");
    if !repair::strip_fence(text).starts_with('{') {
        return Ok(false);
    }
    let Ok(crate::solidcraft98::drive::Call::Tool { name, args }) =
        crate::solidcraft98::drive::parse_text(text, false)
    else {
        return Ok(false);
    };
    let after = json!({"role":"assistant","content":null,"tool_calls":[{"id":"pilot98_1","type":"function","function":{"name":name,"arguments":args.to_string()}}]});
    audit.record("solidcraft98_text_call", message, &after)?;
    *message = after;
    Ok(true)
}
fn has_how(message: &Value) -> bool {
    message["tool_calls"]
        .as_array()
        .is_some_and(|calls| calls.iter().any(|c| c["function"]["name"] == "how"))
}

pub(crate) fn tool_data(message: &Value) -> Option<Value> {
    let content = message.get("content")?;
    let value = if let Some(text) = content.as_str() {
        serde_json::from_str::<Value>(text).ok()?
    } else if content.is_array() {
        json!({"content":content})
    } else {
        content.clone()
    };
    if value.get("error").is_some_and(|e| !e.is_null())
        || value["isError"] == true
        || value["result"]["isError"] == true
    {
        return None;
    }
    Some(guidance::payload(&value))
}

fn append_edge_hints(body: &mut Value, seen: &mut bool, audit: &mut Audit) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .map_or(0, |n| n + 1);
    *seen |= messages
        .iter()
        .any(|m| m["content"].to_string().contains(guidance::EDGE_HINT));
    for n in start..messages.len() {
        if messages[n]["role"] != "tool" {
            continue;
        }
        let id = messages[n]["tool_call_id"].as_str();
        let call = messages[..n]
            .iter()
            .rev()
            .filter_map(|m| m["tool_calls"].as_array())
            .flatten()
            .find(|c| c["id"].as_str() == id);
        if let Some(call) = call {
            let args = &call["function"]["arguments"];
            let args = args
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| args.clone());
            let result = messages[n]["content"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| json!({"error":messages[n]["content"]}));
            let hints = guidance::edge_hints(
                call["function"]["name"].as_str().unwrap_or(""),
                &args,
                &result,
                seen,
            );
            if !hints.is_empty() {
                let content = messages[n]["content"].as_str().unwrap_or("");
                messages[n]["content"] = json!(format!("{content}\n{}", hints.join("\n")));
                audit.record("edge_hint", &Value::Null, &json!(guidance::EDGE_HINT))?;
            }
        }
    }
    Ok(())
}

fn append_plane_hints(
    body: &mut Value,
    seen: &mut [bool; 2],
    audit: &mut Audit,
) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let turn = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    for (index, hint) in [guidance::XZ_SIGN_HINT, guidance::PLANE_CLEANUP_HINT]
        .iter()
        .enumerate()
    {
        seen[index] |= messages
            .iter()
            .any(|m| prompting::message_text(&m["content"]).contains(hint));
    }
    let mut state = guidance::PlaneGuidance::default();
    let mut calls = BTreeMap::new();
    for (i, m) in messages.iter().enumerate().take(4096) {
        if i == turn {
            state.new_turn();
        }
        if let Some(cs) = m["tool_calls"].as_array() {
            for c in cs.iter().take(128) {
                if let Some(id) = c["id"].as_str() {
                    calls.insert(id, c);
                }
            }
        }
        if m["role"] != "tool" {
            continue;
        }
        let raw = m["content"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or_else(|| m["content"].clone());
        if let Some(c) = m["tool_call_id"].as_str().and_then(|id| calls.get(id)) {
            let args = &c["function"]["arguments"];
            let parsed = args
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok());
            state.visit(
                c["function"]["name"].as_str().unwrap_or(""),
                parsed.as_ref().unwrap_or(args),
                &raw,
            );
        } else {
            state.visit("inspect", &Value::Null, &raw);
        }
    }
    for (index, hint) in state.hints().into_iter().enumerate() {
        if let Some(hint) = hint {
            if !seen[index]
                && !messages[turn..]
                    .iter()
                    .any(|m| m["content"].as_str().is_some_and(|s| s.contains(hint)))
            {
                messages.push(
                    json!({"role":"system","content":format!("{} {hint}",crate::identity::HINT)}),
                );
                seen[index] = true;
                audit.record("plane_hint", &Value::Null, &json!(hint))?;
            }
        }
    }
    Ok(())
}

fn append_hints(
    body: &mut Value,
    catalog: &Catalog,
    audit: &mut Audit,
    sketch_seen: &mut [bool; 2],
    next_seen: &mut std::collections::BTreeSet<u64>,
) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let start = messages
        .iter()
        .rposition(|m| m["role"] != "tool")
        .map_or(0, |i| i + 1);
    let mut previous = None;
    let mut prior_hints: Vec<String> = messages
        .iter()
        .map(|m| prompting::message_text(&m["content"]))
        .collect();
    let mut placement_count = 0;
    for message in messages.iter() {
        let text = message["content"].to_string();
        sketch_seen[0] |= text.contains(&guidance::SKETCH_ENTITY_HINT.replace('"', "\\\""));
        sketch_seen[1] |= text.contains(guidance::SKETCH_EXISTING_HINT);
    }
    for i in 0..messages.len() {
        if messages[i]["role"] != "tool" {
            continue;
        }
        let id = messages[i]["tool_call_id"].as_str();
        let call = id.and_then(|id| {
            messages[..i]
                .iter()
                .rev()
                .filter_map(|m| m["tool_calls"].as_array())
                .flat_map(|calls| calls.iter())
                .find(|call| call["id"].as_str() == Some(id))
        });
        let name = call
            .and_then(|call| call["function"]["name"].as_str())
            .or_else(|| messages[i]["name"].as_str())
            .unwrap_or("");
        let change = matches!(name, "execute" | "batch");
        if !name.is_empty() && !change && !matches!(name, "inspect" | "inspect_design") {
            continue;
        }
        let raw = &messages[i]["content"];
        let raw = raw
            .as_str()
            .map(|s| serde_json::from_str::<Value>(s).unwrap_or_else(|_| json!({"error":s})))
            .unwrap_or_else(|| {
                if raw.is_array() {
                    json!({"content":raw})
                } else {
                    raw.clone()
                }
            });
        let data = tool_data(&messages[i]).unwrap_or(Value::Null);
        let after = data.get("after").unwrap_or(&data);
        let before = data.get("before").or(previous.as_ref());
        let mut hint = (i >= start)
            .then(|| guidance::state_hint(before, after, change, Some(catalog)))
            .flatten()
            .map(|h| h.split_whitespace().collect::<Vec<_>>().join(" "));
        if i >= start {
            if let Some(args) = call.and_then(|call| call["function"].get("arguments")) {
                let parsed = args
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                for next in
                    guidance::sketch_hints(name, parsed.as_ref().unwrap_or(args), &raw, sketch_seen)
                {
                    hint = Some(hint.map_or_else(|| next.to_string(), |h| format!("{h}\n{next}")));
                }
            }
        }
        if i >= start && placement_count < 4 {
            if let Some(args) = call.and_then(|call| call["function"].get("arguments")) {
                let parsed = args
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                let placements =
                    guidance::placement_hints(name, parsed.as_ref().unwrap_or(args), &data);
                for placement in placements.into_iter().take(4 - placement_count) {
                    placement_count += 1;
                    hint = Some(
                        hint.map_or_else(|| placement.clone(), |h| format!("{h}\n{placement}")),
                    );
                }
            }
        }
        if after.get("bodies").is_some()
            || after.get("sketches").is_some()
            || after.get("active_sketch").is_some()
        {
            previous = Some(after.clone());
        }
        if let Some(hint) = hint {
            let line = format!(
                "{} {}",
                crate::identity::HINT,
                hint.lines()
                    .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            if prior_hints.iter().any(|text| text.contains(&line)) {
                continue;
            }
            // Bound retained hint identities without retaining tool-result text.
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            line.hash(&mut hash);
            if next_seen.len() >= 256 || !next_seen.insert(hash.finish()) {
                continue;
            }
            prior_hints.push(line.clone());
            let content = &mut messages[i]["content"];
            let before = content.clone();
            if let Some(text) = content.as_str() {
                *content = json!(format!("{text}\n{line}"));
            } else if let Some(blocks) = content.as_array_mut() {
                blocks.push(json!({"type":"text","text":line}));
            }
            if *content != before {
                audit.record("next_step_hint", &before, content)?;
            }
        }
    }
    Ok(())
}

fn set_logged(
    body: &mut Value,
    key: &str,
    after: Value,
    kind: &str,
    audit: &mut Audit,
) -> Result<(), String> {
    let before = body.get(key).cloned().unwrap_or(Value::Null);
    if before != after {
        audit.record(kind, &before, &after)?;
        body[key] = after;
    }
    Ok(())
}
fn validate_request(body: &Value) -> Result<(), String> {
    repair::check_value(body)?;
    if !body.is_object() {
        return Err("request must be an object".into());
    }
    let model = body["model"].as_str().ok_or("model must be a string")?;
    if model.is_empty() || model.len() > 256 {
        return Err("model name must contain 1 to 256 bytes".into());
    }
    let messages = body["messages"]
        .as_array()
        .ok_or("messages must be an array")?;
    if messages.is_empty() || messages.len() > 256 {
        return Err("messages must contain 1 to 256 entries".into());
    }
    if body.get("stream").is_some_and(|v| !v.is_boolean()) {
        return Err("stream must be a boolean".into());
    }
    if body
        .get("n")
        .is_some_and(|v| v.as_u64().is_none_or(|n| !(1..=8).contains(&n)))
    {
        return Err("n must be between 1 and 8".into());
    }
    if let Some(tools) = body.get("tools") {
        let tools = tools.as_array().ok_or("tools must be an array")?;
        if tools.len() > 128 {
            return Err("tools exceed 128 entries".into());
        }
        let mut names = std::collections::BTreeSet::new();
        for tool in tools {
            let name = tool["function"]["name"]
                .as_str()
                .filter(|n| !n.is_empty() && n.len() <= 128)
                .ok_or("tool name must contain 1 to 128 bytes")?;
            if !names.insert(name) {
                return Err("duplicate tool name".into());
            }
        }
    }
    Ok(())
}
fn is_empty(message: &Value) -> bool {
    message["tool_calls"]
        .as_array()
        .is_none_or(|c| c.is_empty())
        && message["content"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
        && message["refusal"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
}
fn last_user(body: &Value) -> &str {
    body["messages"]
        .as_array()
        .and_then(|ms| ms.iter().rev().find(|m| m["role"] == "user"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
}
fn action_turn(body: &Value) -> bool {
    let text = last_user(body).trim().to_ascii_lowercase();
    let text = text
        .strip_prefix("can you ")
        .or_else(|| text.strip_prefix("could you "))
        .or_else(|| text.strip_prefix("would you "))
        .unwrap_or(&text);
    let text = text.strip_prefix("please ").unwrap_or(text);
    let verb = text.split_whitespace().next().unwrap_or("");
    matches!(
        verb,
        "make"
            | "create"
            | "draw"
            | "add"
            | "remove"
            | "delete"
            | "move"
            | "translate"
            | "rotate"
            | "scale"
            | "rename"
            | "extrude"
            | "fillet"
            | "chamfer"
            | "round"
            | "bevel"
            | "shell"
            | "hollow"
            | "finish"
            | "close"
            | "open"
            | "inspect"
            | "measure"
            | "show"
            | "set"
            | "change"
            | "build"
            | "cut"
            | "union"
            | "fuse"
            | "split"
            | "mirror"
            | "pattern"
            | "run"
            | "execute"
    )
}
fn prose_guard(body: &Value, message: &Value, catalog: &Catalog) -> Option<(String, bool)> {
    if message["tool_calls"]
        .as_array()
        .is_some_and(|c| !c.is_empty())
        || message["refusal"].as_str().is_some_and(|s| !s.is_empty())
    {
        return None;
    }
    let text = message["content"].as_str()?;
    let available = body["tools"].as_array()?;
    if available.is_empty() {
        return None;
    }
    if let Ok(Some(object)) = repair::extract_balanced_json(text) {
        let function = object.get("function").unwrap_or(&object);
        if let Some(name) = function
            .get("tool")
            .or_else(|| function.get("name"))
            .and_then(Value::as_str)
        {
            let (id, _) = split_command(name);
            if id.len() <= 256
                && tool_schema(available, name).is_none()
                && catalog.get(id).is_none()
            {
                return Some((catalog.unknown_hint(id, last_user(body)).ok()?, true));
            }
        }
    }
    if action_turn(body) {
        let tokens: std::collections::BTreeSet<_> = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
            .map(|s| s.trim_matches('.'))
            .collect();
        if let Some(command) = catalog
            .commands()
            .iter()
            .find(|c| tokens.contains(c.id.as_str()))
        {
            return Some((
                format!("you described {}; run it, or say why not", command.id),
                false,
            ));
        }
    }
    let user = last_user(body).to_ascii_lowercase();
    let specific = user.contains('"') || user.contains('`');
    let property_question = [
        "what is the volume of ",
        "what is the mass of ",
        "how many faces does ",
    ]
    .iter()
    .any(|s| user.starts_with(s));
    let looked_up = body["messages"].as_array().is_some_and(|ms| {
        let start = ms.iter().rposition(|m| m["role"] == "user").unwrap_or(0);
        ms[start..].iter().any(|m| m["role"] == "tool")
    });
    if specific
        && property_question
        && !looked_up
        && available.iter().any(|t| t["function"]["name"] == "inspect")
    {
        return Some((
            "look it up first: use inspect for the specific object before answering".into(),
            false,
        ));
    }
    None
}
fn provider_error(response: &Upstream) -> Reply {
    let code = serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|v| v["error"]["code"].as_str().map(str::to_string))
        .unwrap_or_default();
    let (machine, sentence) = match (response.status, code.as_str()) {
        (_, "insufficient_quota" | "quota_exceeded") => (
            "out_of_quota",
            "The provider account is out of quota; add credit or change accounts.".into(),
        ),
        (429, _) => (
            "rate_limited",
            response
                .retry_after
                .as_ref()
                .map(|s| format!("The provider is rate limiting requests; retry after {s}."))
                .unwrap_or_else(|| "The provider is rate limiting requests; retry later.".into()),
        ),
        (401 | 403, _) => (
            "bad_key",
            "The provider rejected the API key; check the client's Authorization setting.".into(),
        ),
        (404, _) | (_, "model_not_found") => (
            "missing_model",
            "The requested model is unavailable; check its name and provider.".into(),
        ),
        _ => (
            "provider_error",
            format!(
                "The provider could not complete the request (HTTP {}).",
                response.status
            ),
        ),
    };
    let mut reply = Reply::error(response.status, machine, &sentence);
    reply.retry_after.clone_from(&response.retry_after);
    reply
}

fn tool_schema<'a>(tools: &'a [Value], name: &str) -> Option<&'a Value> {
    tools
        .iter()
        .find(|t| t["function"]["name"] == name)
        .map(|t| &t["function"]["parameters"])
}
fn split_command(name: &str) -> (&str, &str) {
    let split = name
        .find(|c: char| c.is_whitespace() || matches!(c, '{' | '('))
        .unwrap_or(name.len());
    (&name[..split], name[split..].trim())
}
fn packed_params(text: &str) -> Option<Value> {
    let text = if let Some(inner) = text.strip_prefix('(') {
        format!("{{{}}}", inner.strip_suffix(')').unwrap_or(inner))
    } else {
        text.to_string()
    };
    repair::parse_json(&text).ok().filter(Value::is_object)
}
fn convert_text(
    message: &mut Value,
    tools: &[Value],
    catalog: &Catalog,
    audit: &mut Audit,
) -> Result<bool, String> {
    let text = message["content"].as_str().unwrap_or("");
    let Ok(Some(object)) = repair::extract_balanced_json(text) else {
        return Ok(false);
    };
    let function = object.get("function").unwrap_or(&object);
    let (name, mut args) = if let Some(name) = function
        .get("tool")
        .or_else(|| function.get("name"))
        .and_then(Value::as_str)
    {
        (
            name.to_string(),
            function
                .get("args")
                .or_else(|| function.get("arguments"))
                .or_else(|| function.get("parameters"))
                .or_else(|| function.get("params"))
                .or_else(|| function.get("input"))
                .cloned()
                .unwrap_or(json!({})),
        )
    } else if let Some(object) = object.as_object().filter(|o| o.len() == 1) {
        let Some((name, args)) = object.iter().next() else {
            return Ok(false);
        };
        (name.to_string(), args.clone())
    } else {
        return Ok(false);
    };
    let native_name = if tool_schema(tools, &name).is_some() {
        name.clone()
    } else {
        let (id, rest) = split_command(&name);
        if id.len() > 256 || tool_schema(tools, "execute").is_none() {
            return Ok(false);
        }
        let resolved = if catalog.get(id).is_some() {
            id.to_string()
        } else {
            let Some(close) = guidance::unique_close(catalog, id)? else {
                return Ok(false);
            };
            close
        };
        if !rest.is_empty() {
            let Some(params) = packed_params(rest) else {
                return Ok(false);
            };
            if args.as_object().is_none_or(|a| !a.is_empty()) {
                return Ok(false);
            }
            args = params;
        }
        args = json!({"command":resolved,"params":args});
        "execute".into()
    };
    let arguments = if let Some(s) = args.as_str() {
        s.to_string()
    } else {
        String::from_utf8(json_bytes(&args)?).map_err(|_| "cannot encode arguments")?
    };
    let before = message.clone();
    message["content"] = Value::Null;
    message["tool_calls"] = json!([{"id":format!("call_dutchman_pilot_{}", now()),"type":"function","function":{"name":native_name,"arguments":arguments}}]);
    audit.record("text_tool_call", &before, message)?;
    Ok(true)
}

fn repair_calls(
    message: &mut Value,
    tools: &[Value],
    catalog: &Catalog,
    task: &str,
    audit: &mut Audit,
    profile: &mut Profile,
) -> Result<Option<(String, bool)>, String> {
    let Some(calls) = message.get_mut("tool_calls") else {
        return Ok(None);
    };
    let calls = calls
        .as_array_mut()
        .ok_or("upstream tool_calls must be an array")?;
    if calls.len() > MAX_CALLS {
        return Err("upstream exceeds 64 tool calls".into());
    }
    let mut guard = None;
    for call in calls {
        let name = call["function"]["name"]
            .as_str()
            .filter(|s| s.len() <= 128)
            .ok_or("invalid upstream tool name")?
            .to_string();
        let Some(schema) = tool_schema(tools, &name) else {
            guard.get_or_insert((format!("unknown tool {name}; use an advertised tool"), true));
            continue;
        };
        let before = call["function"]["arguments"].clone();
        let mut args = if let Some(text) = before.as_str() {
            match repair::parse_json(text) {
                Ok(args) => args,
                Err(_) => continue, // Preserve unparseable arguments for SolidCraft's validator.
            }
        } else {
            before.clone()
        };
        let mut root_schema = schema.clone();
        if root_schema.is_object() && root_schema.get("type").is_none() {
            root_schema["type"] = json!("object");
        }
        let changes = repair::repair_arguments(&name, &mut args, &root_schema)?;
        for change in &changes {
            audit.record(
                &format!("argument_repair:{}", change.path),
                &change.before,
                &change.after,
            )?;
            profile.repairs = profile.repairs.saturating_add(1);
        }
        let arg_before_guard = args.clone();
        if let Some(reason) = reconcile(&mut args, schema) {
            guard.get_or_insert((format!("{name} {reason}"), true));
        }
        let steps: Vec<&mut Value> = if name == "execute" {
            vec![&mut args]
        } else if name == "batch" {
            args.get_mut("commands")
                .and_then(Value::as_array_mut)
                .map(|s| s.iter_mut().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for step in steps {
            if let Some(reason) = guard_command(step, catalog, task)? {
                guard.get_or_insert((reason, true));
            }
        }
        if args != arg_before_guard {
            audit.record("argument_reconcile", &arg_before_guard, &args)?;
            profile.repairs = profile.repairs.saturating_add(1);
        }
        let strict_same = before
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .is_some_and(|v| v == args);
        if !strict_same {
            let after = Value::String(
                String::from_utf8(json_bytes(&args)?).map_err(|_| "cannot encode arguments")?,
            );
            // Also retain the complete wire argument, including relaxed-syntax changes.
            audit.record("argument_json", &before, &after)?;
            if changes.is_empty() && arg_before_guard == args {
                profile.repairs = profile.repairs.saturating_add(1);
            }
            call["function"]["arguments"] = after;
        }
    }
    Ok(guard)
}
fn schema_type_matches(value: &Value, schema: &Value) -> bool {
    match schema["type"].as_str() {
        Some("array") => value.is_array(),
        Some("object") => value.is_object(),
        Some("string") => value.is_string(),
        Some("number") => value.is_number(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("boolean") => value.is_boolean(),
        _ => false,
    }
}
fn reconcile(args: &mut Value, schema: &Value) -> Option<String> {
    let object = args.as_object_mut()?;
    let properties = schema["properties"].as_object()?;
    let required: Vec<_> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let extras: Vec<_> = object
        .keys()
        .filter(|k| !properties.contains_key(*k))
        .cloned()
        .collect();
    for extra in extras {
        let spelling = normalize(&extra);
        if spelling.is_empty()
            || object
                .keys()
                .filter(|key| normalize(key) == spelling)
                .take(2)
                .count()
                != 1
        {
            continue;
        }
        let matches: Vec<_> = properties
            .keys()
            .filter(|k| !object.contains_key(*k) && normalize(k) == spelling)
            .cloned()
            .collect();
        if matches.len() == 1 {
            if let Some(value) = object.remove(&extra) {
                object.insert(matches[0].clone(), value);
            }
        }
    }
    let missing: Vec<_> = required
        .iter()
        .filter(|k| !object.contains_key(**k))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Some(format!("needs {}", missing.join(", ")));
    }
    for group in schema["allOf"].as_array().into_iter().flatten() {
        let names: Vec<_> = group["anyOf"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|choice| choice["required"][0].as_str())
            .collect();
        if !names.is_empty() && !names.iter().any(|name| object.contains_key(*name)) {
            return Some(format!("needs one of {}", names.join(" | ")));
        }
    }
    for (key, value) in object.iter() {
        if let Some(allowed) = properties.get(key).and_then(|s| s["enum"].as_array()) {
            if !allowed.contains(value) {
                return Some(format!("{key} must be one of {}", json!(allowed)));
            }
        }
        if let Some(count) = properties.get(key).and_then(|s| s["maxItems"].as_u64()) {
            if value.as_array().is_none_or(|a| a.len() != count as usize) {
                return Some(format!(
                    "{key} needs an array of exactly {count} coordinates"
                ));
            }
        }
    }
    if schema["additionalProperties"] == false {
        if let Some(key) = object.keys().find(|k| !properties.contains_key(*k)) {
            return Some(format!("has unknown argument {key}"));
        }
    }
    None
}
fn guard_command(
    step: &mut Value,
    catalog: &Catalog,
    task: &str,
) -> Result<Option<String>, String> {
    let Some(command) = step.get("command").and_then(Value::as_str) else {
        return Ok(None);
    };
    if command.len() > 256 {
        return Err("command id exceeds 256 bytes".into());
    }
    let (id, packed) = split_command(command);
    let mut id = id.to_string();
    if !packed.is_empty() {
        if let Some(params) = packed_params(packed) {
            if step
                .get("params")
                .is_none_or(|p| p.as_object().is_some_and(|o| o.is_empty()))
            {
                step["params"] = params;
            } else {
                return Ok(Some(format!(
                    "{id} has both packed and explicit params; send one params object"
                )));
            }
        } else {
            return Ok(Some(format!("{id} needs a valid params object")));
        }
    }
    if catalog.get(&id).is_none() {
        if let Some(close) = guidance::unique_close(catalog, &id)? {
            id = close;
        } else {
            return Ok(Some(catalog.unknown_hint(&id, task)?));
        }
    }
    if step["command"] != id {
        step["command"] = json!(id);
    }
    let schema = catalog.parameter_schema(&id);
    if let Some(params) = step.get_mut("params") {
        repair::repair_arguments("execute", params, &schema)?;
        if id == "sketch.line" && schema["properties"].get("points").is_some() {
            if let Some(object) = params.as_object_mut() {
                let point = |v: &Value| {
                    v.as_array().is_some_and(|a| {
                        a.len() == 2 && a.iter().all(|v| v.is_number() || v.is_string())
                    })
                };
                if object.len() == 2
                    && object.get("p0").is_some_and(point)
                    && object.get("p1").is_some_and(point)
                {
                    let points = json!([object["p0"], object["p1"]]);
                    object.clear();
                    object.insert("points".into(), points);
                } else if !object.contains_key("points") {
                    return Ok(Some(
                        "sketch.line needs points: [[x,y], [x,y], …] in sketch coordinates".into(),
                    ));
                }
                if object
                    .get("points")
                    .and_then(Value::as_array)
                    .is_none_or(|points| {
                        points.len() < 2
                            || points
                                .iter()
                                .any(|p| p.as_array().is_some_and(|a| a.len() != 2))
                    })
                {
                    return Ok(Some(
                        "sketch.line needs 2D points: [[x,y], [x,y], …]; use sketch coordinates"
                            .into(),
                    ));
                }
            } else {
                return Ok(Some(
                    "sketch.line needs params with points: [[x,y], [x,y], …]".into(),
                ));
            }
        }
        if let Some(reason) = reconcile(params, &schema) {
            return Ok(Some(format!("{id} {reason}")));
        }
    } else if schema["required"].as_array().is_some_and(|r| !r.is_empty()) {
        return Ok(Some(format!("{id} needs params {}", schema["required"])));
    }
    Ok(None)
}

fn collect_sse(bytes: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "upstream stream is not UTF-8")?
        .replace("\r\n", "\n");
    let mut response = json!({"id":crate::identity::BINARY, "object":"chat.completion","created":now(),"model":"","choices":[]});
    let mut done = false;
    for (count, block) in text.split("\n\n").enumerate() {
        if count > 4096 {
            return Err("upstream stream exceeds 4096 events".into());
        }
        let data = block
            .lines()
            .filter_map(|s| s.strip_prefix("data:").map(str::trim_start))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            done = true;
            break;
        }
        let chunk: Value = serde_json::from_str(&data).map_err(|_| "invalid upstream SSE JSON")?;
        repair::check_value(&chunk)?;
        if chunk.get("error").is_some() {
            return Err("provider returned an error inside its stream".into());
        }
        for key in ["id", "model", "created", "system_fingerprint", "usage"] {
            if let Some(value) = chunk.get(key) {
                response[key] = value.clone();
            }
        }
        if let Some(choices) = chunk["choices"].as_array() {
            for choice in choices {
                let index = choice["index"]
                    .as_u64()
                    .filter(|i| *i < 8)
                    .ok_or("invalid streamed choice index")? as usize;
                let out = response["choices"]
                    .as_array_mut()
                    .ok_or("missing choices")?;
                while out.len() <= index {
                    out.push(json!({"index":out.len(),"message":{"role":"assistant","content":null},"finish_reason":null}));
                }
                let out = &mut out[index];
                for key in ["content", "refusal"] {
                    if let Some(fragment) = choice["delta"][key].as_str() {
                        let mut value = out["message"][key].as_str().unwrap_or("").to_string();
                        if value.len().saturating_add(fragment.len()) > MAX_BODY {
                            return Err("streamed text exceeds limit".into());
                        }
                        value.push_str(fragment);
                        out["message"][key] = json!(value);
                    }
                }
                if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
                    for call in calls {
                        let i = call["index"]
                            .as_u64()
                            .filter(|i| *i < MAX_CALLS as u64)
                            .ok_or("invalid streamed tool index")?
                            as usize;
                        if out["message"].get("tool_calls").is_none() {
                            out["message"]["tool_calls"] = json!([]);
                        }
                        let target = out["message"]["tool_calls"]
                            .as_array_mut()
                            .ok_or("invalid streamed tools")?;
                        while target.len() <= i {
                            target.push(json!({"id":"","type":"function","function":{"name":"","arguments":""}}));
                        }
                        for key in ["id", "type"] {
                            if let Some(v) = call.get(key) {
                                target[i][key] = v.clone();
                            }
                        }
                        for key in ["name", "arguments"] {
                            if let Some(fragment) = call["function"][key].as_str() {
                                let mut value = target[i]["function"][key]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                value.push_str(fragment);
                                target[i]["function"][key] = json!(value);
                            }
                        }
                    }
                }
                if !choice["finish_reason"].is_null() {
                    out["finish_reason"] = choice["finish_reason"].clone();
                }
            }
        }
    }
    if !done {
        return Err("upstream stream ended without [DONE]".into());
    }
    repair::check_value(&response)?;
    Ok(response)
}
fn emit_sse(response: &Value, include_usage: bool) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut push = |choices: Value, usage: Option<&Value>| -> Result<(), String> {
        let mut chunk = json!({"id":response["id"],"object":"chat.completion.chunk","created":response["created"],"model":response["model"],"choices":choices});
        if let Some(value) = response.get("system_fingerprint") {
            chunk["system_fingerprint"] = value.clone();
        }
        if include_usage {
            chunk["usage"] = usage.cloned().unwrap_or(Value::Null);
        }
        let bytes = json_bytes(&chunk)?;
        if output.len().saturating_add(bytes.len() + 8) > 2 * MAX_BODY {
            return Err("output stream exceeds 2 MiB".into());
        }
        output.extend_from_slice(b"data: ");
        output.extend(bytes);
        output.extend_from_slice(b"\n\n");
        Ok(())
    };
    for (i, choice) in response["choices"]
        .as_array()
        .ok_or("missing choices")?
        .iter()
        .enumerate()
    {
        let index = choice.get("index").cloned().unwrap_or(json!(i));
        push(
            json!([{"index":index,"delta":{"role":"assistant"},"finish_reason":null}]),
            None,
        )?;
        let mut delta = choice["message"].clone();
        if let Some(object) = delta.as_object_mut() {
            object.remove("role");
        }
        if let Some(calls) = delta["tool_calls"].as_array_mut() {
            for (i, call) in calls.iter_mut().enumerate() {
                call["index"] = json!(i);
            }
        }
        push(
            json!([{"index":index,"delta":delta,"finish_reason":null}]),
            None,
        )?;
        push(
            json!([{"index":index,"delta":{},"finish_reason":choice["finish_reason"]}]),
            None,
        )?;
    }
    if include_usage {
        push(json!([]), response.get("usage"))?;
    }
    output.extend_from_slice(b"data: [DONE]\n\n");
    Ok(output)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    mod context_tests;
    use super::*;
    use std::{
        cell::RefCell,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        proxy: Proxy,
        dir: PathBuf,
    }
    impl Fixture {
        fn new(replies: Vec<Value>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "dutchman-pilot-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            let log = OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("log.jsonl"))
                .unwrap();
            Self {
                proxy: Proxy {
                    recipes: crate::recipe_library::Library::embedded().unwrap(),
                    cfg: Config {
                        solidcraft98: false,
                        no_report_buffer: false,
                        listen: "127.0.0.1:0".parse().unwrap(),
                        upstream: "http://localhost:11434/v1".into(),
                        upstream_timeout: DEFAULT_UPSTREAM_TIMEOUT,
                        fallback_prompt: FallbackPrompt::Off,
                        no_how: true,
                        no_hints: false,
                        no_shortcuts: true,
                        offer_recipe_tools: true,
                        no_fill: true,
                        no_trim: false,
                        trim_cap: DEFAULT_TRIM_CAP,
                        trim_keep: DEFAULT_TRIM_KEEP,
                        recipes_dir: None,
                        planner: true,
                        direct_recipes: false,
                        log: None,
                        profile_dir: Some(dir.clone()),
                        model_settings: None,
                        model_settings_enabled: false,
                        seed: None,
                        prompt_help: false,
                    },
                    agent: ureq::AgentBuilder::new().build(),
                    catalog: Catalog::bundled().unwrap(),
                    profiles: BTreeMap::new(),
                    profile_path: dir.join("profiles.json"),
                    settings: BTreeMap::new(),
                    log,
                    conversations: Vec::new(),
                    form_turns: std::cell::RefCell::default(),
                    turns: crate::capture::Ring::default(),
                    scripted: Some(RefCell::new(Scripted {
                        model: None,
                        reject_tools: false,
                        replies: replies
                            .into_iter()
                            .map(|v| Upstream {
                                status: 200,
                                body: v.to_string().into_bytes(),
                                content_type: "application/json".into(),
                                retry_after: None,
                            })
                            .collect(),
                        requests: Vec::new(),
                    })),
                },
                dir,
            }
        }
        fn with_shortcuts(replies: Vec<Value>) -> Self {
            let mut fixture = Self::new(replies);
            fixture.proxy.cfg.no_shortcuts = false;
            fixture
        }
        fn chat(&mut self, mut body: Value) -> Reply {
            body["model"] = json!("unit");
            self.proxy
                .handle(Request {
                    method: "POST".into(),
                    path: "/v1/chat/completions".into(),
                    auth: Some("Bearer never-stored".into()),
                    body: body.to_string().into_bytes(),
                })
                .unwrap()
        }
        fn audit(&self) -> Audit {
            Audit {
                events: None,
                file: self.proxy.log.try_clone().unwrap(),
                size: 0,
                model: "unit".into(),
                count: 0,
                kinds: BTreeMap::new(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    #[test]
    fn retries_keep_the_current_parallel_result_block_even_with_zero_window() {
        let mut f = Fixture::new(vec![reply(""), reply("Ready.")]);
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_hints = true;
        f.proxy.cfg.trim_keep = 0;
        let mut req = request();
        let results = [
            json!({"role":"tool","tool_call_id":"a","content":"a".repeat(5000)}),
            json!({"role":"tool","tool_call_id":"b","content":"b".repeat(5000)}),
        ];
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","tool_calls":[
                {"id":"a","function":{"name":"inspect","arguments":"{}"}},
                {"id":"b","function":{"name":"inspect","arguments":"{}"}}
            ]}));
        req["messages"]
            .as_array_mut()
            .unwrap()
            .extend(results.clone());
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        for wire in &script.requests {
            for result in &results {
                assert!(wire["messages"].as_array().unwrap().contains(result));
            }
        }
    }

    #[test]
    fn normal_recipe_tools_are_opt_in_and_no_shortcuts_wins() {
        for (offer, disabled) in [(false, false), (true, false), (true, true)] {
            let mut f = Fixture::new(vec![reply("Ready.")]);
            f.proxy.cfg.offer_recipe_tools = offer;
            f.proxy.cfg.no_shortcuts = disabled;
            f.proxy.cfg.planner = false;
            f.proxy.cfg.no_fill = false;
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":"hello"}]);
            f.chat(req);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            for name in ["pilot_build", "pilot_find"] {
                assert_eq!(
                    script.requests[0]["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == name),
                    offer && !disabled
                );
            }
            assert!(!script.requests[0]["messages"]
                .to_string()
                .contains("call pilot_find first"));
            assert_eq!(
                f.proxy.profiles.values().next().unwrap().offer_recipe_tools,
                offer
            );
        }
    }

    #[test]
    fn facts_are_sent_once_when_only_client_history_returns() {
        let mut f = Fixture::new(vec![reply("Ready."); 3]);
        f.proxy.cfg.planner = false;
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":"sketch a constrained plate profile with a hole through the middle"}]);
        for _ in 0..3 {
            f.chat(req.clone());
            req["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":"Ready."}),
                json!({"role":"user","content":"sketch a constrained plate profile with a hole through the middle"}),
            ]);
        }
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        for fact in [prompting::PLACEMENT_FACT, prompting::SKETCH_FACT] {
            let occurrences = script
                .requests
                .iter()
                .flat_map(|r| r["messages"].as_array().unwrap())
                .filter(|m| prompting::message_text(&m["content"]).contains(fact))
                .count();
            assert_eq!(occurrences, 1, "{fact}");
        }
    }

    #[test]
    fn facts_and_how_are_sent_even_without_size_savings() {
        let mut f = Fixture::new(vec![reply("Ready."); 3]);
        f.proxy.cfg.planner = false;
        f.proxy.cfg.offer_recipe_tools = false;
        f.proxy.cfg.no_how = false;
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":"sketch a constrained plate profile with a hole through the middle on XZ"}]);
        for _ in 0..3 {
            f.chat(req.clone());
            req["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":"Ready."}),
                json!({"role":"user","content":"sketch a constrained plate profile with a hole through the middle on XZ"}),
            ]);
        }
        assert_eq!(f.proxy.conversations[0].facts_seen, [true; 3]);
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(!log.contains("optional_help_deferred"));
        for kind in ["placement_fact", "sketch_fact", "sketch_axes_fact"] {
            assert_eq!(log.matches(&format!("\"kind\":\"{kind}\"")).count(), 1);
        }
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        for (index, wire) in script.requests.iter().enumerate() {
            assert!(wire["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == "how"));
            for fact in [
                prompting::PLACEMENT_FACT,
                prompting::SKETCH_FACT,
                prompting::SKETCH_AXES,
            ] {
                assert_eq!(
                    wire["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|m| prompting::message_text(&m["content"]).contains(fact)),
                    index == 0
                );
            }
        }
    }

    #[test]
    fn normal_public_long_and_short_requests_fit_client_plus_added_guidance() {
        let requests: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/v2-03-conversation.json"))
                .unwrap();
        assert_eq!(requests.len(), 24);
        // Keep the first three real requests, including their original histories.
        let short: Vec<_> = requests.iter().take(3).cloned().collect();
        for requests in [&requests, &short] {
            // --no-how isolates recipe-schema savings and the once-only guidance.
            // A separate regression above keeps how available even without savings.
            let mut f = Fixture::new(vec![reply("Ready."); requests.len()]);
            f.proxy.cfg.planner = false;
            f.proxy.cfg.offer_recipe_tools = false;
            f.proxy.cfg.no_shortcuts = false;
            let mut untrimmed = Fixture::new(vec![reply("Ready."); requests.len()]);
            untrimmed.proxy.cfg = f.proxy.cfg.clone();
            untrimmed.proxy.cfg.no_trim = true;
            let mut client_total = 0;
            let mut upstream_total = 0;
            let mut guidance_total = 0;
            for (index, req) in requests.iter().enumerate() {
                assert_eq!(
                    f.proxy
                        .handle(Request {
                            method: "POST".into(),
                            path: "/v1/chat/completions".into(),
                            auth: None,
                            body: req.to_string().into_bytes(),
                        })
                        .unwrap()
                        .status,
                    200
                );
                assert_eq!(untrimmed.chat(req.clone()).status, 200);
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                let full_script = untrimmed.proxy.scripted.as_ref().unwrap().borrow();
                let wire = &script.requests[index];
                let client = history::chars(req);
                let upstream = history::chars(wire);
                // With trimming disabled, only facts/hints change this request.
                // Include their actual JSON escaping, separators and message wrappers.
                let mut full = full_script.requests[index].clone();
                full["model"] = req["model"].clone();
                assert_eq!(full["tools"], req["tools"]);
                let added = history::chars(&full).checked_sub(client).unwrap();
                assert!(
                    upstream <= client + added,
                    "request {index}: {client} + {added} facts/hints -> {upstream}"
                );
                eprintln!("public {}-request replay #{index}: client {client}, upstream {upstream}, facts/hints added {added}", requests.len());
                assert!(!wire["tools"].to_string().contains("pilot_build"));
                assert!(!wire["tools"].to_string().contains("pilot_find"));
                client_total += client;
                upstream_total += upstream;
                guidance_total += added;
            }
            eprintln!("normal public {}-request replay (--no-how): client {client_total} -> upstream {upstream_total}; facts/hints added {guidance_total}", requests.len());
        }
        let schema = guidance::how_tool();
        let how = json!({"type":"function","function":{"name":schema["name"],"description":schema["description"],"parameters":schema["inputSchema"]}});
        assert_eq!(history::chars(&how), 302);
        let mut old_how = how.clone();
        old_how["function"]["description"] = json!("Find a SolidCraft command, a typed template, and a concrete example when possible. Templates must be filled from the request and current model; never executes anything.");
        let client = requests[0].clone();
        let mut before = client.clone();
        shortcuts::offer(&mut before).unwrap();
        before["tools"].as_array_mut().unwrap().push(old_how);
        before["messages"].as_array_mut().unwrap().insert(0, json!({"role":"system","content":"For plates, profiles, enclosures, patterns, honeycombs, cross-drilled boxes and joints, call pilot_find first."}));
        eprintln!("per-request fixed overhead before: {}; after: 303 with how, 0 with --no-how (excluding once-only facts/hints)", history::chars(&before) - history::chars(&client));
        eprintln!(
            "fixed schemas: build {}, find {}, how {} (was 366); default fixed overhead 303",
            history::chars(&crate::recipe_library::build_tool()),
            history::chars(&crate::recipe_library::find_tool()),
            history::chars(&how)
        );
    }

    #[test]
    fn public_single_turn_conversation_replay_trims_all_24_requests() {
        let requests: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/v2-03-conversation.json"))
                .unwrap();
        assert_eq!(requests.len(), 24);
        let mut measurements = Vec::new();
        for no_trim in [true, false] {
            let mut f = Fixture::new(vec![reply("Ready."); requests.len()]);
            f.proxy.cfg.planner = false;
            f.proxy.cfg.direct_recipes = false;
            f.proxy.cfg.no_hints = true;
            f.proxy.cfg.no_trim = no_trim;
            let mut total = 0;
            let mut last = 0;
            for request in &requests {
                let response = f
                    .proxy
                    .handle(Request {
                        method: "POST".into(),
                        path: "/v1/chat/completions".into(),
                        auth: None,
                        body: request.to_string().into_bytes(),
                    })
                    .unwrap();
                assert_eq!(response.status, 200);
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                let wire = script.requests.last().unwrap();
                last = history::chars(wire);
                total += last;
                if no_trim {
                    assert_eq!(wire, request);
                }
                let recent: Vec<_> = request["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m["role"] == "tool")
                    .rev()
                    .take(2)
                    .collect();
                for (index, message) in recent {
                    assert_eq!(&wire["messages"][index], message);
                }
            }
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                24
            );
            let profile = f.proxy.profiles.values().next().unwrap();
            assert_eq!(profile.trim_keep, 2);
            assert_eq!(profile.request_chars_total["upstream.total"], total as u64);
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            let sizes: Vec<Value> = log
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .filter(|v: &Value| v["kind"] == "request_size")
                .collect();
            assert_eq!(sizes.len(), 24);
            let saved: u64 = sizes
                .iter()
                .map(|v| v["dutchman-pilot"]["trimmed_chars"].as_u64().unwrap())
                .sum();
            if no_trim {
                assert_eq!(saved, 0);
            } else {
                assert!(saved > 0);
            }
            measurements.push((total, last));
        }
        let reduction = 100.0 * (1.0 - measurements[1].0 as f64 / measurements[0].0 as f64);
        eprintln!(
            "public 24-request replay: {:?} -> {:?} (total, last); {reduction:.2}% reduction",
            measurements[0], measurements[1]
        );
        assert!(reduction >= 55.0);
        assert!(measurements[1].1 < 40_000);
    }

    #[test]
    fn long_history_fixture_replay_measures_wire_reduction_and_opt_out() {
        let requests = history::replay_fixture();
        let mut totals = Vec::new();
        for no_trim in [true, false] {
            let mut f = Fixture::new(vec![reply("Ready."); requests.len()]);
            f.proxy.cfg.planner = false;
            f.proxy.cfg.no_hints = true;
            f.proxy.cfg.no_trim = no_trim;
            // Preserve the original regression's one-result window; the public
            // single-turn replay above exercises the new default of two.
            f.proxy.cfg.trim_keep = 1;
            let mut total = 0;
            for (index, request) in requests.iter().enumerate() {
                let original = request.clone();
                assert_eq!(f.chat(request.clone()).status, 200);
                assert_eq!(*request, original);
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                let wire = &script.requests[index];
                total += wire.to_string().chars().count();
                let latest = request["messages"].as_array().unwrap().len() - 1;
                assert_eq!(wire["messages"][latest], request["messages"][latest]);
                if no_trim {
                    assert_eq!(wire, request);
                }
            }
            let profile = f.proxy.profiles.values().next().unwrap();
            assert_eq!(profile.trim_enabled, !no_trim);
            assert_eq!(profile.upstream_requests, requests.len() as u64);
            assert_eq!(profile.request_chars_total["upstream.total"], total as u64);
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            let events: Vec<Value> = log
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .filter(|v: &Value| v["kind"] == "request_size")
                .collect();
            assert_eq!(events.len(), requests.len());
            assert!(
                events[0]["dutchman-pilot"]["client"]["tool_results.list_commands"]
                    .as_u64()
                    .unwrap()
                    > 1500
            );
            let report = crate::profile::profiles(Some(&f.dir), Some("unit"), false).unwrap();
            assert!(report.contains("mean / max per upstream request (8 requests)"));
            totals.push(total);
        }
        let percent = 100.0 * (1.0 - totals[1] as f64 / totals[0] as f64);
        eprintln!(
            "history replay: {} -> {} upstream characters; {percent:.2}% reduction",
            totals[0], totals[1]
        );
        assert!(percent > 65.0);
    }

    #[test]
    fn request_size_audit_counts_client_and_pilot_parts_and_retries() {
        let mut f = Fixture::with_shortcuts(vec![reply(""), reply("Ready.")]);
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_how = false;
        let mut req = request();
        req["messages"] = json!([{"role":"system","content":"éπ"},{"role":"user","content":"sketch a constrained plate profile with a centered hole"}]);
        assert_eq!(f.chat(req).status, 200);
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        let events: Vec<Value> = log
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .filter(|v: &Value| v["kind"] == "request_size")
            .collect();
        assert_eq!(events.len(), 2);
        let sizes = &events[0]["dutchman-pilot"];
        assert_eq!(sizes["client"]["system"], 2);
        assert_eq!(
            sizes["pilot"]["sketch_fact"],
            prompting::SKETCH_FACT.chars().count()
        );
        assert_eq!(
            sizes["pilot"]["placement_fact"],
            prompting::PLACEMENT_FACT.chars().count()
        );
        for name in ["how_schema", "pilot_build_schema", "pilot_find_schema"] {
            assert!(sizes["pilot"][name].as_u64().unwrap() > 0);
        }
        let profile = f.proxy.profiles.values().next().unwrap();
        assert_eq!(profile.upstream_requests, 2);
        assert_eq!(profile.request_chars_total["client.system"], 4);
        assert_eq!(profile.request_chars_max["client.system"], 2);
    }

    #[test]
    fn resent_facts_and_hints_are_present_once_at_latest_system() {
        let mut f = Fixture::new(vec![reply("Ready."); 2]);
        f.proxy.cfg.planner = false;
        let mut req = request();
        req["messages"] = json!([{"role":"system","content":"old rules"},{"role":"user","content":"previous turn"},{"role":"system","content":"latest rules"},{"role":"user","content":"sketch a constrained plate profile"}]);
        f.chat(req);
        let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[0].clone();
        assert_eq!(sent["messages"][0]["content"], "old rules");
        assert!(sent["messages"][2]["content"]
            .as_str()
            .unwrap()
            .contains(prompting::SKETCH_FACT));
        f.chat(sent);
        let repeated = f.proxy.scripted.as_ref().unwrap().borrow().requests[1].clone();
        assert_eq!(
            repeated["messages"]
                .to_string()
                .matches(prompting::SKETCH_FACT.split('"').next().unwrap())
                .count(),
            1
        );
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert_eq!(
            log.lines()
                .filter(|line| line.contains("\"kind\":\"sketch_fact\""))
                .count(),
            1
        );
    }

    #[test]
    fn solidcraft98_history_trimming_and_opt_out_use_the_same_wire_pass() {
        for no_trim in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready.")]);
            f.proxy.cfg.solidcraft98 = true;
            f.proxy.cfg.trim_keep = 0;
            f.proxy.cfg.planner = false;
            f.proxy.cfg.no_hints = true;
            f.proxy.cfg.no_trim = no_trim;
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":"read screen"},
                {"role":"assistant","tool_calls":[{"id":"screen1","type":"function","function":{"name":"screen","arguments":"{}"}}]},
                {"role":"tool","tool_call_id":"screen1","content":"screen text ".repeat(500)},
                {"role":"user","content":"hello"}]);
            f.chat(req.clone());
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[0].clone();
            assert_eq!(sent["messages"][2] == req["messages"][2], no_trim);
            assert_eq!(
                f.proxy.profiles.values().next().unwrap().trim_enabled,
                !no_trim
            );
        }
    }
    #[test]
    fn chat_reports_are_local_redacted_and_never_captured_or_profiled() {
        for dialect98 in [false, true] {
            for stream in [false, true] {
                let mut f = Fixture::new(vec![]);
                f.proxy.cfg.solidcraft98 = dialect98;
                if stream {
                    f.proxy.cfg.upstream = "none".into();
                }
                let mut failed = crate::report::offline(&json!({"model":"test-model", "messages":[{"role":"user","content":"PrivateRequestWord"}]})).unwrap();
                failed.failed = true;
                failed.names.push(("Hidden Gear".into(), "Body".into()));
                failed.event(
                    "provider_error",
                    json!({"error":"failed Hidden Gear /private/design.step PrivateRequestWord"}),
                );
                f.proxy.turns.push(failed.clone());
                let mut good = failed;
                good.failed = false;
                good.model = "other-model".into();
                f.proxy.turns.push(good);
                // Also exclude report requests retained by an older capture producer.
                let old = crate::report::offline(
                    &json!({"messages":[{"role":"user","content":"/report"}]}),
                )
                .unwrap();
                f.proxy.turns.push(old);
                let note = "/REPORT failed Hidden Gear /private/note.step jane@example.com https://private.example 192.0.2.4 sk-private-secret-token";
                let response = f.chat(json!({"stream":stream,"stream_options":{"include_usage":true},"messages":[{"role":"user","content":note}]}));
                assert_eq!(response.status, 200);
                assert_eq!(response.rewrites, 1);
                let value = if stream {
                    assert_eq!(response.content_type, "text/event-stream");
                    assert!(response.body.ends_with(b"data: [DONE]\n\n"));
                    collect_sse(&response.body).unwrap()
                } else {
                    assert_eq!(response.content_type, "application/json");
                    serde_json::from_slice(&response.body).unwrap()
                };
                assert_eq!(value["choices"][0]["message"]["role"], "assistant");
                assert_eq!(value["choices"][0]["finish_reason"], "stop");
                assert!(value["choices"][0]["message"].get("tool_calls").is_none());
                let text = value["choices"][0]["message"]["content"].as_str().unwrap();
                assert!(text.starts_with("Pilot report: Saved a redacted report to "));
                assert!(text.ends_with("Nothing was sent. Open the link to review and submit."));
                let block = text
                    .split("```text\n")
                    .nth(1)
                    .unwrap()
                    .split("\n```")
                    .next()
                    .unwrap();
                assert_eq!(block.lines().count(), 5);
                for prefix in [
                    "Pilot version:",
                    "Model: test-model",
                    "Request shape:",
                    "Outcome:",
                    "Issue link:",
                ] {
                    assert!(block.contains(prefix), "{block}");
                }
                let files: Vec<_> = fs::read_dir(f.dir.join("reports")).unwrap().collect();
                assert_eq!(files.len(), 1);
                let report = fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
                assert!(report.contains("Turn: 1"));
                assert!(report.contains("Tester note (redacted)"));
                assert!(report.contains("Body1"));
                assert!(report.contains("[path]"));
                for private in [
                    "Hidden Gear",
                    "/private/",
                    "jane@example.com",
                    "private.example",
                    "192.0.2.4",
                    "sk-private-secret-token",
                    "PrivateRequestWord",
                ] {
                    assert!(!report.contains(private), "leaked {private}: {report}");
                    assert!(!block.contains(private));
                }
                assert!(!report.contains("Request text (explicitly included"));
                assert_eq!(f.proxy.turns.turns().len(), 3);
                assert!(f.proxy.profiles.is_empty());
                assert!(f.proxy.conversations.is_empty());
                assert!(f
                    .proxy
                    .scripted
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .requests
                    .is_empty());
                assert!(fs::read_to_string(f.dir.join("log.jsonl"))
                    .unwrap()
                    .contains("pilot_report_reply"));
            }
        }
    }
    #[test]
    fn chat_report_empty_disabled_and_real_requests() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.no_report_buffer = disabled;
            let response = f.chat(json!({"messages":[{"role":"user","content":"report this"}]}));
            let value: Value = serde_json::from_slice(&response.body).unwrap();
            let text = value["choices"][0]["message"]["content"].as_str().unwrap();
            assert_eq!(text.lines().count(), 1);
            assert!(text.contains(if disabled { "disabled" } else { "empty" }));
            assert!(text.contains("dutchman-pilot report"));
            assert!(f
                .proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow()
                .requests
                .is_empty());
            assert!(f.proxy.turns.turns().is_empty());
        }
        for request in [
            "/reporting",
            "please report the volume",
            "report the body count",
        ] {
            let mut f = Fixture::new(vec![reply("ordinary model answer")]);
            f.proxy.cfg.planner = false;
            let response = f.chat(json!({"messages":[{"role":"user","content":"/report"},{"role":"user","content":request}]}));
            assert_eq!(response.status, 200);
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                1
            );
        }
    }
    #[test]
    fn report_ring_and_peer_policy_without_sockets() {
        for disabled in [false, true] {
            let mut fixture = Fixture::new(Vec::new());
            fixture.proxy.cfg.upstream = "none".into();
            fixture.proxy.cfg.direct_recipes = true;
            fixture.proxy.cfg.planner = false;
            fixture.proxy.cfg.no_report_buffer = disabled;
            for i in 1..=7 {
                assert_eq!(
                    fixture
                        .chat(planner::cases::request(&format!("create a cube {i}mm")))
                        .status,
                    200
                );
            }
            for (peer, status) in [
                ("127.0.0.1:1", 200),
                ("[::1]:1", 200),
                ("192.0.2.1:1", 403),
                ("[2001:db8::1]:1", 403),
            ] {
                let reply = fixture
                    .proxy
                    .handle_peer(
                        Request {
                            method: "GET".into(),
                            path: "/pilot/turns".into(),
                            auth: None,
                            body: Vec::new(),
                        },
                        peer.parse().unwrap(),
                    )
                    .unwrap();
                assert_eq!(reply.status, status);
                if status == 200 {
                    let turns: Value = serde_json::from_slice(&reply.body).unwrap();
                    assert_eq!(
                        turns.as_array().unwrap().len(),
                        if disabled { 0 } else { 5 }
                    );
                    if !disabled {
                        assert_eq!(turns[0]["id"], 3);
                        assert_eq!(turns[4]["id"], 7);
                        assert_eq!(turns[4]["flags"]["planner"], false);
                        assert_eq!(turns[4]["flags"]["direct_recipes"], true);
                        assert_eq!(turns[4]["shape"]["intent"], "box");
                        assert_eq!(turns[4]["failed"], false);
                        assert!(turns[4]["events"].to_string().contains("finish_reason"));
                    }
                }
            }
        }
    }
    fn tools() -> Value {
        json!([
            {"type":"function","function":{"name":"execute","parameters":{"type":"object","properties":{"command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}},
            {"type":"function","function":{"name":"batch","parameters":{"type":"object","properties":{"commands":{"type":"array"}},"required":["commands"]}}},
            {"type":"function","function":{"name":"inspect","parameters":{"type":"object","properties":{}}}}
        ])
    }
    fn request() -> Value {
        json!({"model":"unit","messages":[{"role":"user","content":"make a box"}],"tools":tools()})
    }
    fn failed_parse(raw: &str) -> Value {
        json!({"error":format!("error parsing tool call: raw='{raw}', err=invalid JSON")})
    }
    fn mark_errors(f: &Fixture) {
        let mut script = f.proxy.scripted.as_ref().unwrap().borrow_mut();
        for response in &mut script.replies {
            if serde_json::from_slice::<Value>(&response.body)
                .unwrap()
                .get("error")
                .is_some()
            {
                response.status = 500;
            }
        }
    }
    #[test]
    fn scripted_opaque_500_retries_once_and_profiles() {
        let mut f = Fixture::new(vec![Value::Null, call("inspect", json!({}))]);
        {
            let mut script = f.proxy.scripted.as_ref().unwrap().borrow_mut();
            script.replies[0].status = 500;
            script.replies[0].body = b"opaque provider failure".to_vec();
        }
        let response = f.chat(request());
        assert_eq!(response.status, 200);
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(
            body["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "inspect"
        );
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        assert_eq!(script.requests[0], script.requests[1]);
        let events: Vec<Value> = fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let retries: Vec<_> = events
            .iter()
            .filter(|e| e["kind"] == "provider 500 retried")
            .collect();
        assert_eq!(retries.len(), 1);
        assert_eq!(
            retries[0]["before"],
            json!({"status":500,"body":"opaque provider failure"})
        );
        assert_eq!(retries[0]["after"], json!({"retry":1}));
        let profiles = load_profiles(Some(&f.dir), Some("unit")).unwrap();
        assert_eq!(profiles[0].provider_500_retries, 1);
        assert_eq!(profiles[0].provider_errors, 0);
    }

    #[test]
    fn scripted_500_excerpts_are_lossy_and_bounded_with_one_shared_retry() {
        for first_parse in [false, true] {
            let mut f = Fixture::new(vec![failed_parse("bad"), Value::Null]);
            let mut bytes = vec![0xff];
            bytes.extend("é".repeat(350).as_bytes());
            let excerpt = format!("�{}", "é".repeat(299));
            {
                let mut script = f.proxy.scripted.as_ref().unwrap().borrow_mut();
                for response in &mut script.replies {
                    response.status = 500;
                }
                if !first_parse {
                    script.replies[0].body = bytes.clone();
                }
                script.replies[1].body = bytes;
            }
            let response = f.chat(request());
            assert_eq!(response.status, 500);
            let body: Value = serde_json::from_slice(&response.body).unwrap();
            assert_eq!(body["error"]["code"], "provider_error");
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                2
            );
            let events: Vec<Value> = fs::read_to_string(f.dir.join("log.jsonl"))
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            let retries: Vec<_> = events
                .iter()
                .filter(|e| e["kind"] == "provider 500 retried")
                .collect();
            assert_eq!(retries.len(), usize::from(!first_parse));
            if !first_parse {
                assert_eq!(retries[0]["before"], json!({"status":500,"body":excerpt}));
            }
            let errors: Vec<_> = events
                .iter()
                .filter(|e| e["kind"] == "provider_error")
                .collect();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0]["before"], json!({"status":500,"body":excerpt}));
        }
    }

    #[test]
    fn scripted_provider_retry_excludes_other_statuses_and_solidcraft98() {
        for (status, dialect98) in [(400, false), (429, false), (503, false), (500, true)] {
            let mut f = Fixture::new(vec![Value::Null, call("inspect", json!({}))]);
            f.proxy.cfg.solidcraft98 = dialect98;
            {
                let mut script = f.proxy.scripted.as_ref().unwrap().borrow_mut();
                script.replies[0].status = status;
                script.replies[0].body = b"opaque provider failure".to_vec();
            }
            assert_eq!(f.chat(request()).status, status);
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                1
            );
            assert!(!fs::read_to_string(f.dir.join("log.jsonl"))
                .unwrap()
                .contains("500 retried"));
        }
    }

    #[test]
    fn scripted_parse_error_recovery_retry_and_refusals() {
        for stream in [false, true] {
            let mut f = Fixture::new(vec![failed_parse("{tool:'execute',args:{command:'solid.box',params:'{length:20,width:20,height:20}'}}")]);
            mark_errors(&f);
            let mut req = request();
            req["stream"] = json!(stream);
            let response = f.chat(req);
            assert_eq!(response.status, 200);
            if stream {
                assert!(String::from_utf8(response.body).unwrap().contains("[DONE]"));
            } else {
                assert_eq!(
                    args(&response)["params"],
                    json!({"length":20,"width":20,"height":20})
                );
            }
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                1
            );
            assert!(fs::read_to_string(f.dir.join("log.jsonl"))
                .unwrap()
                .contains("ollama 500 recovered"));
            assert_eq!(
                load_profiles(Some(&f.dir), Some("unit")).unwrap()[0].ollama_500_recoveries,
                1
            );
        }
        for raw in [
            "",
            "no call",
            "'",
            "{tool:'inspect',args:{}} {tool:'inspect',args:{}}",
            "[{tool:'inspect',args:{}}]",
            "{tool:'execute',args:{command:'solid.box',params:{}}}",
            "{tool:'execute',args:{command:'solid.box',params:'broken'}}",
            "{tool:'inspect',args:'broken'}",
        ] {
            let mut f = Fixture::new(vec![failed_parse(raw), failed_parse(raw)]);
            mark_errors(&f);
            let response = f.chat(request());
            assert_eq!(response.status, 500, "{raw}");
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 2);
            assert_eq!(script.requests[0], script.requests[1]);
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.matches("ollama 500 retried").count(), 1);
            assert!(!log.contains("provider 500 retried"));
        }
        let mut f = Fixture::new(vec![failed_parse("bad"), call("inspect", json!({}))]);
        mark_errors(&f);
        assert_eq!(f.chat(request()).status, 200);
    }

    #[test]
    fn scripted_conversation_limits_and_previous_success() {
        let mut replies = Vec::new();
        for _ in 0..4 {
            replies.extend([
                call(
                    "execute",
                    json!({"command":"solid.box","params":{"length":20}}),
                ),
                reply("Cannot do it."),
            ]);
        }
        replies.push(call("execute", json!({"command":"solid.box","params":{}})));
        let mut f = Fixture::new(replies);
        let mut req = request();
        for turn in 0..5 {
            let result = f.chat(req.clone());
            assert_eq!(result.status, 200);
            let response: Value = serde_json::from_slice(&result.body).unwrap();
            req["messages"].as_array_mut().unwrap().extend([
                response["choices"][0]["message"].clone(),
                json!({"role":"user","content":format!("try again {turn}")}),
            ]);
        }
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
            9
        );
        assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 4);
        for success in [false, true] {
            let mut replies = vec![reply("Use solid.box to make it.")];
            if !success {
                replies.push(reply("Cannot do it."));
            }
            let mut f = Fixture::new(replies);
            let mut req = request();
            req["messages"].as_array_mut().unwrap().extend([
                call("execute", json!({"command":"solid.box","params":{}}))["choices"][0]["message"].clone(),
                json!({"role":"tool","tool_call_id":"native","content":json!({"isError":!success,"result":{}}).to_string()}),
            ]);
            assert_eq!(f.chat(req).status, 200);
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                if success { 1 } else { 2 }
            );
        }
    }
    #[test]
    fn scripted_how_lookup_allowed_after_accepted_call() {
        let how = call("how", json!({"task":"box"}));
        let mut f = Fixture::new(vec![how, reply("Ready.")]);
        f.proxy.cfg.no_how = false;
        let mut req = request();
        req["messages"].as_array_mut().unwrap().extend([
            call("execute", json!({"command":"solid.box","params":{}}))["choices"][0]["message"].clone(),
            json!({"role":"tool","tool_call_id":"native","content":json!({"result":{}}).to_string()}),
        ]);
        assert_eq!(f.chat(req).status, 200);
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
            2
        );
    }
    #[test]
    fn planner_is_optional_and_only_guides_the_upstream() {
        for enabled in [false, true] {
            let mut f = Fixture::new(vec![call(
                "execute",
                json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}}),
            )]);
            f.proxy.cfg.planner = enabled;
            let mut req = request();
            req["messages"][0]["content"] = json!("make a 20 mm cube");
            assert_eq!(f.chat(req.clone()).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 1);
            if enabled {
                assert_eq!(script.requests[0]["messages"].as_array().unwrap().len(), 2);
                assert_eq!(script.requests[0]["messages"][1]["role"], "system");
                assert!(script.requests[0]["messages"][1]["content"]
                    .as_str()
                    .unwrap()
                    .contains("solid.box"));
            } else {
                assert_eq!(script.requests[0], req);
            }
        }
    }
    #[test]
    fn replay_every_recipe_through_prose_model_face() {
        for (rule, phrases) in planner::cases::CASES {
            for phrase in *phrases {
                let req = planner::cases::request(phrase);
                let expected = planner::plan(&req, &Catalog::bundled().unwrap())
                    .unwrap()
                    .unwrap();
                let mut f = Fixture::new(vec![reply(&format!("I will {phrase}"))]);
                let result = f.chat(req);
                assert_eq!(
                    result.status,
                    200,
                    "{rule}: {phrase}: {}",
                    String::from_utf8_lossy(&result.body)
                );
                let response: Value = serde_json::from_slice(&result.body).unwrap();
                let message = &response["choices"][0]["message"];
                if matches!(*rule, "volume" | "body_count") {
                    assert!(message.get("tool_calls").is_none());
                } else {
                    let calls = message["tool_calls"]
                        .as_array()
                        .unwrap_or_else(|| panic!("no calls: {rule}: {phrase}: {response}"));
                    assert_eq!(calls.len(), expected.calls.len());
                    for (actual, planned) in calls.iter().zip(expected.calls) {
                        assert_eq!(actual["function"]["name"], planned["tool"]);
                        let args: Value =
                            serde_json::from_str(actual["function"]["arguments"].as_str().unwrap())
                                .unwrap();
                        assert_eq!(args, planned["arguments"], "{phrase}");
                    }
                    assert!(f.proxy.scripted.as_ref().unwrap().borrow().requests.len() == 1);
                }
            }
        }
    }
    #[test]
    fn recipe_completion_checks_once_after_accepted_calls() {
        for (actual, retries) in [(1256.047875, 0), (1000.0, 1)] {
            let first = call(
                "execute",
                json!({"command":"solid.box","params":{"length":6.35,"width":6.35,"height":6.35,"operation":"new"}}),
            );
            let mut replies = vec![first, reply("Done.")];
            if retries == 1 {
                replies.push(reply("The measured volume differs; I cannot finish."));
            }
            let mut f = Fixture::new(replies);
            let mut req = planner::cases::request("create a cube 1/4 inch");
            let response = f.chat(req.clone());
            assert_eq!(response.status, 200);
            let response: Value = serde_json::from_slice(&response.body).unwrap();
            req["messages"].as_array_mut().unwrap().extend([
                response["choices"][0]["message"].clone(),
                json!({"role":"tool","tool_call_id":"native","content":json!({"bodies":[{"name":"Blank","volume_mm3":1000},{"name":"New","volume_mm3":actual-1000.0}]}).to_string()}),
            ]);
            assert_eq!(f.chat(req).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 2 + retries);
            if retries == 1 {
                assert!(script.requests[2]["messages"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .contains("expected volume"));
            }
        }
    }
    #[test]
    fn prose_conversion_requires_plan_mode_and_equal_user_slots() {
        for (enabled, prose) in [
            (false, "I will create a cube 1/4 inch"),
            (true, "I will create a sphere radius 3mm"),
            (true, "I will create a cube 1/2 inch"),
            (true, "A cube could be created here."),
        ] {
            let mut f = Fixture::new(vec![reply(prose), reply("I cannot finish this request.")]);
            f.proxy.cfg.planner = enabled;
            let response = f.chat(planner::cases::request("create a cube 1/4 inch"));
            assert_eq!(response.status, 200);
            let response: Value = serde_json::from_slice(&response.body).unwrap();
            assert!(response["choices"][0]["message"]
                .get("tool_calls")
                .is_none());
        }
    }
    #[test]
    fn recipe_stream_and_slot_audit_are_visible() {
        let mut f = Fixture::new(vec![reply("I will create a cube 1/4 inch")]);
        let mut req = planner::cases::request("create a cube 1/4 inch");
        req["stream"] = json!(true);
        let result = f.chat(req);
        assert_eq!(result.status, 200);
        assert_eq!(result.content_type, "text/event-stream");
        let response = collect_sse(&result.body).unwrap();
        assert_eq!(response["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            response["choices"][0]["message"]["tool_calls"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let records: Vec<Value> = fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        for kind in ["plan", "recipe_prose"] {
            let record = records.iter().find(|r| r["kind"] == kind).unwrap();
            assert_eq!(
                record["after"]["slots"]["Box"]["size"],
                json!([6.35, 6.35, 6.35])
            );
        }
    }
    #[test]
    fn compound_prose_prefers_advertised_atomic_batch() {
        let text = "build a plate 72x48x6mm with a centred through hole diameter 8mm";
        let mut f = Fixture::new(vec![reply(&format!("I will {text}"))]);
        let mut req = planner::cases::request(text);
        req["tools"] = tools();
        let result = f.chat(req);
        assert_eq!(result.status, 200);
        let response: Value = serde_json::from_slice(&result.body).unwrap();
        let calls = response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["function"]["name"], "batch");
        let args: Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args["commands"].as_array().unwrap().len(), 2);
        assert_eq!(
            args["commands"][1]["params"]["targets"],
            json!(["PilotRecipe1"])
        );
        assert_eq!(calls[1]["function"]["name"], "execute");
    }
    fn reply(text: &str) -> Value {
        json!({"id":"a","created":1,"model":"unit","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":"stop"}]})
    }
    fn call(name: &str, args: Value) -> Value {
        let mut r = reply("");
        r["choices"][0]["message"]["tool_calls"] = json!([{"id":"native","type":"function","function":{"name":name,"arguments":args.to_string()}}]);
        r["choices"][0]["finish_reason"] = json!("tool_calls");
        r
    }
    fn args(reply: &Reply) -> Value {
        let v: Value = serde_json::from_slice(&reply.body).unwrap();
        serde_json::from_str(
            v["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn unknown_hints_use_latest_task_without_executing_suggestions() {
        for bad_reply in [
            call("execute", json!({"command":"Edge.Polygon","params":{}})),
            call(
                "batch",
                json!({"commands":[{"command":"Edge.Polygon","params":{}}]}),
            ),
            reply("{name:'Edge.Polygon',parameters:{}}"),
        ] {
            let mut f = Fixture::new(vec![bad_reply, reply("Which radius?")]);
            let mut req = request();
            req["messages"] = json!([
                {"role":"user","content":"open a document"},
                {"role":"assistant","content":"Done."},
                {"role":"user","content":"round all the edges"}
            ]);
            f.chat(req);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 2);
            let hint = script.requests[1]["messages"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["content"]
                .as_str()
                .unwrap();
            assert!(
                hint.starts_with("unknown Edge.Polygon; closest: solid.fillet"),
                "{hint}"
            );
            for junk in ["assembly.bom", "doc.open", "edit.redo"] {
                assert!(!hint.contains(junk), "{hint}");
            }
        }
        let catalog = Catalog::bundled().unwrap();
        assert_eq!(
            catalog.unknown_hint("Edge.Polygon", "make it fly").unwrap(),
            "unknown Edge.Polygon; no close command; use the how tool or list_commands"
        );
        assert_eq!(
            catalog.unknown_hint("solid.nothing", "make it").unwrap(),
            "unknown solid.nothing; no close command; use the how tool or list_commands"
        );
        assert!(catalog
            .suggestions("solid.filet", "")
            .unwrap()
            .contains(&"solid.fillet".to_string()));
    }

    #[test]
    fn text_argument_aliases_and_stray_bracket_preserve_arguments() {
        for key in ["arguments", "parameters", "params", "args", "input"] {
            for nested in [false, true] {
                let function = format!("\"name\":\"execute\",\"{key}\":{{\"command\":\"solid.box\",\"params\":{{\"length\":20,\"width\":30,\"height\":40}}}}]");
                let text = if nested {
                    format!("{{\"function\":{{{function}}}}}")
                } else {
                    format!("{{{function}}}")
                };
                let mut f = Fixture::new(vec![reply(&text)]);
                let response = f.chat(request());
                assert_eq!(
                    args(&response),
                    json!({"command":"solid.box","params":{"length":20,"width":30,"height":40}}),
                    "{text}"
                );
                assert!(response.rewrites > 0);
            }
        }
    }

    #[test]
    fn full_turn_native_text_batch_and_profile_persistence() {
        let native = call("inspect", json!({}));
        let mut f = Fixture::new(vec![
            native.clone(),
            reply(
                "{tool:'execute',args:{command:'solid.box',params:{length:20,width:20,height:20}}}",
            ),
            call(
                "batch",
                json!({"commands":"[{command:'sketch.finish',params:'{}'}]"}),
            ),
        ]);
        let response = f.chat(request());
        assert_eq!(response.body, native.to_string().into_bytes());
        assert_eq!(response.rewrites, 0);
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests[0],
            request()
        );
        let response = f.chat(request());
        assert_eq!(args(&response)["command"], "solid.box");
        assert!(response.rewrites > 0);
        let response = f.chat(request());
        assert_eq!(args(&response)["commands"][0]["params"], json!({}));
        let text = profiles(Some(&f.dir), Some("unit")).unwrap();
        assert!(!text.contains("never-stored"));
        let p: Vec<Profile> = serde_json::from_str(&text).unwrap();
        assert_eq!((p[0].turns, p[0].native, p[0].text_written), (3, 2, 1));
        assert_eq!(p[0].repairs, 2);
        assert_eq!(p[0].repairs_by_kind.get("argument_repair"), Some(&2));
        assert_eq!(p[0].repairs_by_kind.get("argument_json"), Some(&1));
        let restored: Profiles = read_json(&f.dir.join("profiles.json")).unwrap();
        assert_eq!(restored.len(), 1);
    }
    #[test]
    fn all_explicit_shapes_packed_ids_and_first_object_refusal() {
        let f = Fixture::new(vec![]);
        let mut audit = f.audit();
        let tools = tools().as_array().unwrap().clone();
        for text in [
            "{tool:'inspect',args:{}}",
            "{name:'inspect',arguments:{}}",
            "{function:{name:'inspect',arguments:{}}}",
            "{inspect:{}}",
            "{tool:'solid.box {length:20,width:20,height:20}'}",
            "{tool:'solid.box(length:20,width:20,height:20)'}",
            "{'solid.box':{length:20,width:20,height:20}}",
            "{tool:'solid.bxo',args:{length:20,width:20,height:20}}",
        ] {
            let mut message = json!({"role":"assistant","content":format!("```json\n{text}\n``` \n {{inspect: {{}}}}")});
            assert!(
                convert_text(&mut message, &tools, &f.proxy.catalog, &mut audit).unwrap(),
                "{text}"
            );
            assert!(message["content"].is_null());
            assert_eq!(message["tool_calls"].as_array().unwrap().len(), 1);
        }
        for text in [
            "I'll use solid.box",
            "{bad} then {inspect:{}}",
            "{tool:'unknown',args:{}}",
            "{irrelevant:1} then {inspect:{}}",
        ] {
            let mut message = json!({"role":"assistant","content":text});
            let original = message.clone();
            assert!(!convert_text(&mut message, &tools, &f.proxy.catalog, &mut audit).unwrap());
            assert_eq!(message, original);
        }
    }
    #[test]
    fn retries_empty_once_prose_once_and_maximum_two() {
        for replies in [
            vec![reply(""), reply("")],
            vec![
                reply("I will use solid.box."),
                reply("I will use solid.box."),
            ],
        ] {
            let mut f = Fixture::new(replies);
            let response = f.chat(request());
            assert!(response.rewrites > 0);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 2);
            assert!(script.replies.is_empty());
        }
        let mut f = Fixture::new(vec![
            call(
                "execute",
                json!({"command":"solid.box","params":{"length":20}}),
            ),
            call("execute", json!({"command":"solid.nothing","params":{}})),
            reply("I'll use solid.box."),
        ]);
        let response = f.chat(request());
        assert_eq!(response.status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 3);
        assert!(script.requests[1]["messages"][2]["content"]
            .as_str()
            .unwrap()
            .contains("needs height"));
        assert!(script.requests[2]["messages"][4]["content"]
            .as_str()
            .unwrap()
            .contains("unknown solid.nothing; closest:"));
        assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 2);
    }
    #[test]
    fn documented_optional_and_alternative_params_do_not_hand_back() {
        let catalog = Catalog::bundled().unwrap();
        for params in [json!({"distance":10}), json!({"through_all":true})] {
            let mut step = json!({"command":"solid.extrude","params":params});
            assert_eq!(guard_command(&mut step, &catalog, "extrude").unwrap(), None);
            for tool in ["execute", "batch"] {
                let arguments = if tool == "batch" {
                    json!({"commands":[step]})
                } else {
                    step.clone()
                };
                let mut f = Fixture::new(vec![call(tool, arguments)]);
                assert_eq!(f.chat(request()).status, 200);
                assert_eq!(
                    f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                    1
                );
                assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 0);
            }
        }
        let mut sketch = json!({"command":"sketch.create","params":{"plane":"XY"}});
        assert_eq!(
            guard_command(&mut sketch, &catalog, "sketch").unwrap(),
            None
        );
        sketch["params"] = json!({});
        assert_eq!(
            guard_command(&mut sketch, &catalog, "sketch").unwrap(),
            Some("sketch.create needs plane".into())
        );
        let mut extrude = json!({"command":"solid.extrude"});
        assert_eq!(
            guard_command(&mut extrude, &catalog, "extrude").unwrap(),
            None
        );
        let mut fillet = json!({"command":"solid.fillet","params":{"edges":[[0,0,0]]}});
        assert_eq!(
            guard_command(&mut fillet, &catalog, "fillet").unwrap(),
            Some("solid.fillet needs radius".into())
        );
    }
    #[test]
    fn recorded_radius_batches_preserve_every_argument() {
        // Unmodified audit `before` arguments from runs-pilot-c:
        // v1-C/gpt-oss_20b/10-two-holes, v2-C/gpt-oss_20b/05-hole-grid,
        // and v1-C/qwen3_14b/10-two-holes (pilot-rewrites.jsonl).
        let fixture = Fixture::new(vec![]);
        let tools = tools().as_array().unwrap().clone();
        for (text, circles) in [
            (include_str!("../tests/fixtures/gpt-oss-r5.json"), 2),
            (include_str!("../tests/fixtures/gpt-oss-r4.json"), 12),
            (include_str!("../tests/fixtures/qwen3-r5.json"), 2),
        ] {
            let original: Value = serde_json::from_str(text).unwrap();
            assert_eq!(
                original["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|step| step["command"] == "sketch.circle.center")
                    .count(),
                circles
            );
            let mut message = call("batch", original.clone())["choices"][0]["message"].clone();
            let before = message.clone();
            repair_calls(
                &mut message,
                &tools,
                &fixture.proxy.catalog,
                "",
                &mut fixture.audit(),
                &mut Profile::default(),
            )
            .unwrap();
            assert_eq!(message, before);
            for step in original["commands"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|step| step["command"] == "sketch.circle.center")
            {
                for name in ["radius", "diameter"] {
                    let mut step = step.clone();
                    let value = step["params"]
                        .as_object_mut()
                        .unwrap()
                        .remove("radius")
                        .unwrap();
                    step["params"][name] = value;
                    let before = step.clone();
                    assert_eq!(
                        guard_command(&mut step, &fixture.proxy.catalog, "").unwrap(),
                        None
                    );
                    assert_eq!(step, before);
                }
            }
        }
    }

    #[test]
    fn parameter_names_never_substitute_semantics() {
        for (source, target) in [
            ("radius", "diameter"),
            ("diameter", "radius"),
            ("distance", "radius"),
            ("width", "height"),
            ("amount", "radius"),
            ("start_end", "end_start"),
        ] {
            let schema = json!({"properties":{target:{"type":"number"}},"required":[target]});
            let mut params = json!({source:5});
            let before = params.clone();
            assert!(reconcile(&mut params, &schema).is_some());
            assert_eq!(params, before);
        }
        let schema = json!({"properties":{"body_name":{"type":"string"}},"required":["body_name"]});
        for source in ["Body_Name", "body-name", "bodyName"] {
            let mut params = json!({source:"Plate"});
            assert_eq!(reconcile(&mut params, &schema), None);
            assert_eq!(params, json!({"body_name":"Plate"}));
        }
        let mut ambiguous = json!({"Body_Name":"First", "body-name":"Second"});
        let before = ambiguous.clone();
        assert!(reconcile(&mut ambiguous, &schema).is_some());
        assert_eq!(ambiguous, before);
    }

    #[test]
    fn shapes_and_enums_are_repaired_without_guessing_and_logged() {
        let fixture = Fixture::new(vec![]);
        let tools = tools().as_array().unwrap().clone();
        let mut message = call(
            "execute",
            json!({"command":"sketch.line",
            "params":{"p0":[0,0], "p1":["15 mm",25]}}),
        )["choices"][0]["message"]
            .clone();
        assert_eq!(
            repair_calls(
                &mut message,
                &tools,
                &fixture.proxy.catalog,
                "",
                &mut fixture.audit(),
                &mut Profile::default()
            )
            .unwrap(),
            None
        );
        let args: Value = serde_json::from_str(
            message["tool_calls"][0]["function"]["arguments"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(args["params"], json!({"points":[[0,0],["15 mm",25]]}));
        assert!(fs::read_to_string(fixture.dir.join("log.jsonl"))
            .unwrap()
            .contains("argument_reconcile"));
        for params in [
            json!({"p0":[0,0,0],"p1":[15,25,0]}),
            json!({"p0":[0,0],"p1":[15,25],"closed":true}),
            json!({"p0":[0,0]}),
            json!({"points":[[0,0,0],[15,25,0]]}),
        ] {
            let mut step = json!({"command":"sketch.line","params":params});
            let before = step.clone();
            assert!(guard_command(&mut step, &fixture.proxy.catalog, "")
                .unwrap()
                .unwrap()
                .contains("points"));
            assert_eq!(step, before);
        }
        let mut circle =
            json!({"command":"sketch.circle.center","params":{"center":"[15,25]","radius":5}});
        assert_eq!(
            guard_command(&mut circle, &fixture.proxy.catalog, "").unwrap(),
            None
        );
        assert_eq!(circle["params"], json!({"center":[15,25],"radius":5}));
        let mut step =
            json!({"command":"solid.extrude","params":{"distance":"2 in","operation":"create"}});
        let before = step.clone();
        let reason = guard_command(&mut step, &fixture.proxy.catalog, "")
            .unwrap()
            .unwrap();
        for allowed in ["new", "join", "cut", "intersect", "auto"] {
            assert!(reason.contains(allowed));
        }
        assert_eq!(step, before);
    }

    #[test]
    fn command_and_parameter_reconciliation_is_conservative() {
        let mut box_call =
            json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}});
        assert_eq!(
            guard_command(&mut box_call, &Catalog::bundled().unwrap(), "box").unwrap(),
            None
        );
        let mut f = Fixture::new(vec![call(
            "execute",
            json!({"Command":"solid.bxo {length:20,width:20,height:20}","Params":{}}),
        )]);
        let response = f.chat(request());
        assert_eq!(
            args(&response),
            json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}})
        );
        let schema = json!({"properties":{"target":{"type":"object"},"radius":{"type":"number"}},"required":["target","radius"]});
        let mut args = json!({"target":{},"amount":3});
        assert!(reconcile(&mut args, &schema).unwrap().contains("radius"));
        assert_eq!(args["amount"], 3);
        assert!(args.get("radius").is_none());
        let mut args = json!({"target":{},"amount":"3"});
        assert!(reconcile(&mut args, &schema).unwrap().contains("radius"));
        assert!(args.get("amount").is_some());
        let mut invalid = call("execute", json!({}));
        invalid["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] =
            json!("not JSON");
        let mut f = Fixture::new(vec![invalid.clone()]);
        assert_eq!(f.chat(request()).body, invalid.to_string().into_bytes());
    }
    #[test]
    fn streaming_round_trip_and_missing_done_refusal() {
        let mut f = Fixture::new(vec![reply("{inspect:{}}")]);
        let mut req = request();
        req["stream"] = json!(true);
        let response = f.chat(req);
        assert_eq!(response.content_type, "text/event-stream");
        let collected = collect_sse(&response.body).unwrap();
        assert_eq!(
            collected["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "inspect"
        );
        assert_eq!(collected["choices"][0]["finish_reason"], "tool_calls");
        assert!(collect_sse(b"data: {}\n\n").is_err());
        assert!(collect_sse(b"data: {\"choices\":[{\"index\":999}]}\n\ndata: [DONE]\n\n").is_err());
    }
    #[test]
    fn prompt_help_and_default_settings_are_opt_in_and_logged() {
        for ollama in [false, true] {
            let mut f = Fixture::new(vec![call("inspect", json!({})); 2]);
            let key = serde_json::to_string(&(f.proxy.cfg.upstream.as_str(), "unit")).unwrap();
            f.proxy.profiles.insert(
                key,
                Profile {
                    model: "unit".into(),
                    upstream: f.proxy.cfg.upstream.clone(),
                    capabilities_checked: true,
                    ollama,
                    ..Profile::default()
                },
            );
            f.chat(request());
            f.proxy.cfg.prompt_help = true;
            f.proxy.cfg.model_settings_enabled = true;
            f.proxy.cfg.seed = Some(42);
            f.proxy.cfg.fallback_prompt = FallbackPrompt::On;
            f.chat(request());
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests[0], request());
            let req = &script.requests[1];
            let systems: Vec<_> = req["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["role"] == "system")
                .collect();
            assert_eq!(systems.len(), 1);
            let block = systems[0]["content"].as_str().unwrap();
            assert!(block.len() < 1500);
            assert!(block.contains(FALLBACK));
            assert!(block.contains("solid.box:"));
            if ollama {
                assert_eq!(req["temperature"], 0.1);
                assert_eq!(req["seed"], 42);
                assert_eq!(req["options"]["num_ctx"], 8192);
            } else {
                assert!(req.get("temperature").is_none());
                assert!(req.get("seed").is_none());
                assert!(req.get("options").is_none());
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("prompt_help"));
            assert_eq!(log.contains("model_settings"), ollama);
        }
    }

    #[test]
    fn edge_hints_persist_per_conversation_and_respect_no_hints() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready."); 3]);
            f.proxy.cfg.no_hints = disabled;
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":"round it"}]);
            for step in 0..3 {
                let messages = req["messages"].as_array_mut().unwrap();
                if step == 2 {
                    messages.push(json!({"role":"user","content":"round another body"}));
                }
                let id = format!("edge_{step}");
                messages.push(json!({"role":"assistant","tool_calls":[{"id":id,"function":{"name":"execute","arguments":json!({"command":"solid.fillet","params":{"edges":[[0,0,0]],"radius":2}}).to_string()}}]}));
                messages.push(json!({"role":"tool","tool_call_id":id,"content":"{\"error\":\"no edge at [0,0,0]\"}"}));
                f.chat(req.clone());
                let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[step].clone();
                let last = sent["messages"].as_array().unwrap().last().unwrap()["content"]
                    .as_str()
                    .unwrap();
                assert_eq!(last.contains(guidance::EDGE_HINT), !disabled && step == 0);
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("\"kind\":\"edge_hint\""), !disabled);
        }
    }

    #[test]
    fn sketch_hints_remember_unmodified_client_history_across_user_turns() {
        let mut f = Fixture::new(vec![reply("Ready."); 3]);
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":"dimension it"}]);
        for step in 0..3 {
            let messages = req["messages"].as_array_mut().unwrap();
            if step == 2 {
                messages.push(json!({"role":"user","content":"dimension another profile"}));
            }
            let id = format!("dimension_{step}");
            messages.push(json!({"role":"assistant","tool_calls":[{"id":id,"function":{"name":"execute","arguments":json!({"command":"sketch.dimension","params":{"entities":"l1"}}).to_string()}}]}));
            messages.push(json!({"role":"tool","tool_call_id":id,"content":"{\"error\":\"entities must be a list\"}"}));
            f.chat(req.clone());
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[step].clone();
            let last = sent["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap();
            assert_eq!(last.contains(guidance::SKETCH_ENTITY_HINT), step == 0);
        }
    }

    #[test]
    fn next_step_hints_send_once_across_client_history_without_size_savings() {
        let mut f = Fixture::new(vec![reply("Ready."); 3]);
        f.proxy.cfg.offer_recipe_tools = false;
        f.proxy.cfg.planner = false;
        let mut req = request();
        req["messages"] = json!([{"role":"user","content":"inspect"}]);
        for step in 0..3 {
            let id = format!("inspect_{step}");
            let messages = req["messages"].as_array_mut().unwrap();
            messages.extend([
                json!({"role":"assistant","tool_calls":[{"id":id,"type":"function","function":{"name":"inspect","arguments":"{}"}}]}),
                json!({"role":"tool","tool_call_id":id,"content":"{\"bodies\":[],\"sketches\":[]}"}),
            ]);
            f.chat(req.clone());
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            let wire = &script.requests[step];
            assert_eq!(
                wire["messages"]
                    .to_string()
                    .contains("Empty design: start with a sketch or a primitive."),
                step == 0
            );
            if step == 0 {
                assert!(history::chars(wire) > history::chars(&req));
            }
            drop(script);
            req["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":"Ready."}),
                json!({"role":"user","content":"inspect again"}),
            ]);
        }
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert_eq!(log.matches("\"kind\":\"next_step_hint\"").count(), 1);
    }

    #[test]
    fn sketch_fact_is_upfront_and_combines_with_placement_unless_disabled() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready.")]);
            f.proxy.cfg.no_hints = disabled;
            let mut req = request();
            req["messages"] = json!([{"role":"system","content":"Rules"},{"role":"user","content":"sketch a constrained plate profile with a centered hole"}]);
            f.chat(req);
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[0].clone();
            assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
            let system = sent["messages"][0]["content"].as_str().unwrap();
            assert_eq!(system.contains(prompting::SKETCH_FACT), !disabled);
            assert_eq!(system.contains(prompting::PLACEMENT_FACT), !disabled);
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("\"kind\":\"sketch_fact\""), !disabled);
        }
    }

    #[test]
    fn plane_facts_sign_cleanup_and_conversation_dedup_are_audited_and_optional() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready."); 3]);
            f.proxy.cfg.no_hints = disabled;
            let mut req = request();
            req["messages"] = json!([{"role":"user","content":"Continue the sketch on XZ"}]);
            for step in 0..3 {
                if step == 2 {
                    req["messages"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"role":"user","content":"Another sketch on XZ"}));
                }
                if step != 1 {
                    for (n, (command, params, result)) in [
                        (
                            "solid.box",
                            json!({"length":20,"width":20,"height":20}),
                            json!({"created_bodies":["Body1"]}),
                        ),
                        (
                            "construct.plane.offset",
                            json!({"base":"XY","offset":10,"name":"Unused"}),
                            json!({"name":"Unused"}),
                        ),
                        ("sketch.create", json!({"plane":"XZ"}), json!({})),
                        (
                            "sketch.circle.center",
                            json!({"center":[10,10],"radius":4}),
                            json!({}),
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let id = format!("plane_{step}_{n}");
                        req["messages"].as_array_mut().unwrap().push(json!({"role":"assistant","tool_calls":[{"id":id,"function":{"name":"execute","arguments":json!({"command":command,"params":params}).to_string()}}]}));
                        req["messages"].as_array_mut().unwrap().push(
                            json!({"role":"tool","tool_call_id":id,"content":result.to_string()}),
                        );
                    }
                }
                f.chat(req.clone());
                let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[step].clone();
                let text = sent.to_string();
                assert_eq!(
                    text.contains(prompting::SKETCH_AXES),
                    !disabled && step == 0
                );
                assert_eq!(
                    text.contains(guidance::XZ_SIGN_HINT),
                    !disabled && step == 0
                );
                assert_eq!(
                    text.contains(guidance::PLANE_CLEANUP_HINT),
                    !disabled && step == 0
                );
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("\"kind\":\"sketch_axes_fact\""), !disabled);
            assert_eq!(log.contains("\"kind\":\"plane_hint\""), !disabled);
        }
    }

    #[test]
    fn sketch_facts_errors_and_existing_hints_are_audited_and_deduplicated() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready."); 3]);
            f.proxy.cfg.no_hints = disabled;
            let mut req = request();
            req["messages"] = json!([
                {"role":"user","content":"sketch a constrained profile"},
                {"role":"assistant","tool_calls":[{"id":"d1","function":{"name":"execute","arguments":json!({"command":"sketch.dimension","params":{"entities":["l1","l2","l3"],"value":"width"}}).to_string()}}]},
                {"role":"tool","tool_call_id":"d1","content":json!({"error":"entities must list one or two sketch entities"}).to_string()},
                {"role":"assistant","tool_calls":[{"id":"c1","function":{"name":"execute","arguments":json!({"command":"sketch.constraint.coincident","params":{}}).to_string()}}]},
                {"role":"tool","tool_call_id":"c1","content":"{\"existing\":true}"},
                {"role":"tool","tool_call_id":"c1","content":"{\"existing\":true}"}
            ]);
            // Only trailing results are annotated, with earlier hints remembered.
            req["messages"][2]["content"] = json!(format!(
                "{{\"error\":\"entities invalid\"}}\n{} {}",
                crate::identity::HINT,
                guidance::SKETCH_ENTITY_HINT
            ));
            f.chat(req.clone());
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[0].clone();
            assert_eq!(sent["messages"][0]["role"] == "system", !disabled);
            let all = sent["messages"].to_string();
            assert_eq!(
                all.matches(guidance::SKETCH_EXISTING_HINT).count(),
                usize::from(!disabled)
            );
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("\"kind\":\"sketch_fact\""), !disabled);
            assert_eq!(log.contains("\"kind\":\"next_step_hint\""), !disabled);

            // A fresh dimension error must also reach the upstream, including plain text errors.
            req["messages"] = json!([
                {"role":"user","content":"dimension it"},
                {"role":"assistant","tool_calls":[{"id":"d2","function":{"name":"execute","arguments":json!({"command":"sketch.dimension","params":{"entities":"l1","value":35}}).to_string()}}]},
                {"role":"tool","tool_call_id":"d2","content":"Error: entities must be a list"}
            ]);
            f.chat(req);
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[1].clone();
            let last = sent["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap();
            assert_eq!(last.contains(guidance::SKETCH_ENTITY_HINT), !disabled);
            let mut req = request();
            req["messages"] = sent["messages"].clone();
            f.chat(req);
            let sent = f.proxy.scripted.as_ref().unwrap().borrow().requests[2].clone();
            let last = sent["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap();
            assert_eq!(
                last.matches(guidance::SKETCH_ENTITY_HINT).count(),
                usize::from(!disabled)
            );
        }
    }

    #[test]
    fn field_prompt_gets_fact_before_first_call_unless_hints_disabled() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![call(
                "execute",
                json!({"command":"solid.box","params":{"width":20,"length":20,"height":20}}),
            )]);
            f.proxy.cfg.no_hints = disabled;
            f.proxy.cfg.planner = false;
            let mut req = request();
            req["messages"] = json!([
                {"role":"system","content":"Existing rules"},
                {"role":"user","content":"draw a 20mm cube with a 5mm hole through the center"}]);
            f.chat(req.clone());
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            let sent = &script.requests[0];
            assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
            if disabled {
                assert_eq!(sent["messages"], req["messages"]);
            } else {
                assert_eq!(
                    sent["messages"][0]["content"],
                    format!("Existing rules\n{}", prompting::PLACEMENT_FACT)
                );
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("\"kind\":\"placement_fact\""), !disabled);
        }
    }

    #[test]
    fn profile_fallback_and_opt_in_settings_only() {
        let mut f = Fixture::new(vec![
            reply("{inspect:{}}"),
            reply("Ready."),
            reply("{inspect:{}}"),
        ]);
        f.proxy.cfg.fallback_prompt = FallbackPrompt::Auto;
        // Capabilities are checked already, simulating a provider without /api/show.
        let key = serde_json::to_string(&(f.proxy.cfg.upstream.as_str(), "unit")).unwrap();
        f.proxy.profiles.insert(
            key,
            Profile {
                model: "unit".into(),
                upstream: f.proxy.cfg.upstream.clone(),
                capabilities_checked: true,
                ..Profile::default()
            },
        );
        f.chat(request());
        f.chat(request());
        assert_eq!(
            f.proxy.scripted.as_ref().unwrap().borrow().requests[1]["messages"][0]["content"],
            request()["messages"][0]["content"]
        );
        f.proxy.settings.insert(
            "unit".into(),
            ModelSettings {
                temperature: Some(0.1),
                seed: Some(42),
                num_ctx: Some(8192),
                json_retry: true,
            },
        );
        let response = f.chat(request());
        assert!(response.rewrites > 0);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert!(script.requests[2].get("temperature").is_none());
        assert!(script.requests[2].get("seed").is_none());
        assert!(script.requests[2].get("options").is_none());
        assert!(script.requests[2].get("response_format").is_none());
        assert!(ModelSettings {
            temperature: Some(0.3),
            ..ModelSettings::default()
        }
        .validate()
        .is_err());
        assert!(ModelSettings {
            num_ctx: Some(1024),
            ..ModelSettings::default()
        }
        .validate()
        .is_err());
    }
    #[test]
    fn local_how_native_text_stream_and_budget() {
        for first in [
            call("how", json!({"task":"round edges"})),
            reply("{tool:'how',args:{task:'round edges'}}"),
        ] {
            for stream in [false, true] {
                let mut f = Fixture::new(vec![first.clone(), call("inspect", json!({}))]);
                f.proxy.cfg.no_how = false;
                let mut req = request();
                req["stream"] = json!(stream);
                let result = f.chat(req);
                assert_eq!(result.status, 200);
                let persisted = load_profiles(Some(&f.dir), Some("unit")).unwrap();
                assert_eq!(persisted[0].how_answers, 1);
                let output = String::from_utf8(result.body).unwrap();
                assert!(!output.contains("how"));
                assert!(output.contains("inspect"));
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                assert_eq!(script.requests.len(), 2);
                let history = script.requests[1]["messages"].as_array().unwrap();
                assert_eq!(history[1]["tool_calls"][0]["function"]["name"], "how");
                assert_eq!(history[2]["role"], "tool");
                assert_eq!(
                    history[2]["tool_call_id"],
                    history[1]["tool_calls"][0]["id"]
                );
                assert!(history[2]["content"].as_str().unwrap().contains("fillet"));
                let advertised = script.requests[0]["tools"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap();
                assert_eq!(
                    advertised["function"]["parameters"],
                    guidance::how_tool()["inputSchema"]
                );
            }
        }
        let how = call("how", json!({"task":"box"}));
        let mut f = Fixture::new(vec![how.clone(), how.clone(), reply("Ready.")]);
        f.proxy.cfg.no_how = false;
        assert_eq!(f.chat(request()).status, 200);
        assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 2);
        let mut f = Fixture::new(vec![how.clone(), how.clone(), how.clone(), how]);
        f.proxy.cfg.no_how = false;
        let result = f.chat(request());
        assert_eq!(result.status, 502);
        assert!(!String::from_utf8(result.body)
            .unwrap()
            .contains("tool_calls"));
        assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 2);
    }

    #[test]
    fn how_mixed_calls_errors_and_shared_guard_budget() {
        let mut mixed = call("how", json!({"task":42}));
        mixed["choices"][0]["message"]["tool_calls"]
            .as_array_mut()
            .unwrap()
            .push(call("inspect", json!({}))["choices"][0]["message"]["tool_calls"][0].clone());
        let mut f = Fixture::new(vec![mixed, call("inspect", json!({}))]);
        f.proxy.cfg.no_how = false;
        assert_eq!(f.chat(request()).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(
            script.requests[1]["messages"][1]["tool_calls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(script.requests[1]["messages"][2]["content"]
            .as_str()
            .unwrap()
            .contains("how error:"));
        drop(script);
        let how = call("how", json!({"task":"box"}));
        let mut f = Fixture::new(vec![reply(""), how.clone(), how.clone(), how]);
        f.proxy.cfg.no_how = false;
        assert_eq!(f.chat(request()).status, 502);
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert_eq!(
            log.lines()
                .filter(|s| s.contains("\"kind\":\"how_answer\""))
                .count(),
            1
        );
        assert_eq!(f.proxy.profiles.values().next().unwrap().hand_backs, 2);
    }

    #[test]
    fn field_report_box_creation_gets_placement_hint_by_default() {
        let state = json!({"bodies":["Body1"],"changed_bodies":[],"created_bodies":["Body1"],"feature":1,"name":"Box1","removed_bodies":[],"warning":null}).to_string();
        let mut req = request();
        req["messages"] = json!([
            {"role":"user","content":"draw a 20mm cube with a 5mm hole through the center"},
            call("execute", json!({"command":"solid.box","params":{"length":20,"width":20,"height":20}}))["choices"][0]["message"],
            {"role":"tool","tool_call_id":"native","content":state}
        ]);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.proxy.cfg.planner = false;
        assert!(!f.proxy.cfg.planner);
        assert!(!f.proxy.cfg.direct_recipes);
        assert_eq!(f.chat(req.clone()).status, 200);
        assert_eq!(load_profiles(Some(&f.dir), None).unwrap()[0].hints_added, 1);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests[0]["messages"][3]["content"], format!("{state}\n[dutchman-pilot hint] Body1 spans X 0..20, Y 0..20, Z 0..20 mm; its centre is (10, 10, 10) and its top face centre is (10, 10, 20)."));
        drop(script);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_hints = true;
        f.chat(req.clone());
        assert_eq!(f.proxy.scripted.as_ref().unwrap().borrow().requests[0], req);
    }

    #[test]
    fn placement_hints_cap_bodies_across_trailing_tool_results() {
        let args = json!({"commands":[{"command":"solid.box","params":{"length":20,"width":20,"height":20}}]});
        let mut calls = Vec::new();
        let mut messages = vec![json!({"role":"user","content":"make eight boxes"})];
        for i in 0..8 {
            calls.push(json!({"id":format!("box{i}"),"type":"function","function":{"name":"batch","arguments":args.to_string()}}));
        }
        messages.push(json!({"role":"assistant","tool_calls":calls}));
        for i in 0..8 {
            messages.push(json!({"role":"tool","tool_call_id":format!("box{i}"),"content":json!({"ok":true,"results":[{"created_bodies":[format!("Body{i}")]}]}).to_string()}));
        }
        let mut req = request();
        req["messages"] = json!(messages);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.proxy.cfg.planner = false;
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        let sent = script.requests[0]["messages"].as_array().unwrap();
        let hints = sent
            .iter()
            .filter(|m| m["role"] == "tool" && m["content"].as_str().unwrap().contains("spans X"))
            .count();
        assert_eq!(hints, 4);
    }

    #[test]
    fn model_hints_preserve_results_and_only_augment_trailing_tools() {
        let state = json!({"bodies":[],"sketches":[]}).to_string();
        let mut req = request();
        req["messages"] = json!([
            {"role":"user","content":"inspect"},
            call("inspect", json!({}))["choices"][0]["message"],
            {"role":"tool","tool_call_id":"native","content":state}
        ]);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        assert_eq!(f.chat(req.clone()).status, 200);
        assert_eq!(load_profiles(Some(&f.dir), None).unwrap()[0].hints_added, 1);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(
            script.requests[0]["messages"][2]["content"],
            format!(
                "{state}\n[dutchman-pilot hint] Empty design: start with a sketch or a primitive."
            )
        );
        drop(script);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.proxy.cfg.no_hints = true;
        f.chat(req.clone());
        assert_eq!(f.proxy.scripted.as_ref().unwrap().borrow().requests[0], req);
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"thanks"}));
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.chat(req.clone());
        assert_eq!(f.proxy.scripted.as_ref().unwrap().borrow().requests[0], req);
        let mut req = request();
        let state =
            json!({"bodies":[{"volume":20}],"active_sketch":"s","sketches":[{"name":"s","dof":4}]})
                .to_string();
        req["messages"] = json!([
            call("inspect", json!({}))["choices"][0]["message"],
            {"role":"tool","tool_call_id":"native","content":state},
            call("execute", json!({"command":"sketch.finish","params":{}}))["choices"][0]["message"],
            {"role":"tool","tool_call_id":"native","content":state}
        ]);
        let mut f = Fixture::new(vec![reply("Ready.")]);
        f.chat(req);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        let content = script.requests[0]["messages"][3]["content"]
            .as_str()
            .unwrap();
        assert!(content.starts_with(&state));
        assert!(content.contains("didn't change"));
        assert!(content.contains("open sketch"));
        assert!(content.contains("4 free degrees"));
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn auto_fallback_uses_recent_client_turns_and_logs_rule() {
        let mut f = Fixture::new(vec![
            reply("{inspect:{}}"),
            reply("Ready."),
            reply("{inspect:{}}"),
            reply("Ready."),
            reply("Ready."),
            reply("Ready."),
            reply("Ready."),
            reply("Ready."),
        ]);
        f.proxy.cfg.fallback_prompt = FallbackPrompt::Auto;
        let key = serde_json::to_string(&(f.proxy.cfg.upstream.as_str(), "unit")).unwrap();
        f.proxy.profiles.insert(
            key,
            Profile {
                model: "unit".into(),
                capabilities_checked: true,
                native_support: Some(true),
                text_written: 100,
                ..Profile::default()
            },
        );
        for _ in 0..8 {
            assert_eq!(f.chat(request()).status, 200);
        }
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        let fallback: Vec<_> = script
            .requests
            .iter()
            .map(|req| req["messages"][0]["content"] == FALLBACK)
            .collect();
        assert_eq!(
            fallback,
            [false, false, false, true, true, true, false, false]
        );
        assert_eq!(
            f.proxy
                .profiles
                .values()
                .next()
                .unwrap()
                .recent_text_calls
                .len(),
            5
        );
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("text_calls_two_of_last_five"));
        assert!(!log.contains("capabilities_lack_tools"));
    }
    #[test]
    fn capability_fallback_requires_observed_capabilities_and_history_survives_reload() {
        for ollama in [false, true] {
            let mut f = Fixture::new(vec![reply("Ready.")]);
            f.proxy.cfg.fallback_prompt = FallbackPrompt::Auto;
            let key = serde_json::to_string(&(f.proxy.cfg.upstream.as_str(), "unit")).unwrap();
            f.proxy.profiles.insert(
                key.clone(),
                Profile {
                    model: "unit".into(),
                    capabilities_checked: true,
                    native_support: Some(false),
                    ollama,
                    text_written: 100,
                    ..Profile::default()
                },
            );
            assert_eq!(f.chat(request()).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(
                script.requests[0]["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["content"].as_str().is_some_and(|c| c.contains(FALLBACK))),
                ollama
            );
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.contains("capabilities_lack_tools"), ollama);
            let restored: Profiles = read_json(&f.dir.join("profiles.json")).unwrap();
            assert_eq!(restored[&key].recent_text_calls, [false]);
        }
        let old: Profile = serde_json::from_value(json!({"text_written":100})).unwrap();
        assert!(old.recent_text_calls.is_empty());
    }
    #[test]
    fn text_mode_bounds_and_narrow_rejection_patterns() {
        assert!(text_mode::rejected(400, b"model DOES NOT SUPPORT TOOLS"));
        for (status, body) in [
            (500, "does not support tools"),
            (400, "invalid tools schema"),
            (404, "missing model"),
        ] {
            assert!(!text_mode::rejected(status, body.as_bytes()));
        }
        let f = Fixture::new(vec![]);
        let mut body = request();
        body["messages"] = json!([
            {"role":"assistant","content":[{"type":"text","text":"context"}],"tool_calls":[]},
            {"role":"tool","name":"inspect","content":"é".repeat(20000)}
        ]);
        let wire = text_mode::wire(&body, &mut f.audit()).unwrap();
        assert!(wire["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("context"));
        let text = wire["messages"][1]["content"].as_str().unwrap();
        assert!(text.len() < 16410 && text.ends_with("[truncated]"));
        let tools: Vec<_> = (0..128).map(|i| json!({"function":{"name":format!("tool{i}"), "description":"purpose ".repeat(1000), "parameters":{"type":"object","properties":{"command":{"type":"string"}}}}})).collect();
        let mut body = request();
        text_mode::activate(&mut body, &tools, "test", &mut f.audit()).unwrap();
        assert!(body["messages"][0]["content"].as_str().unwrap().len() <= 32768);
        let tools = vec![
            json!({"function":{"name":"tool", "parameters":{"properties":(0..64).map(|i| (format!("{i}{}", "x".repeat(128)), json!({"type":"string"}))).collect::<serde_json::Map<_, _>>()}}});
            128
        ];
        assert!(text_mode::activate(&mut body, &tools, "test", &mut f.audit()).is_err());
    }

    #[test]
    fn text_mode_rejection_reload_history_and_repairs() {
        let mut f = Fixture::new(vec![
            reply("Here is the call:\n```json\n{\"tool\":\"inspect\",\"args\":{}}\n```"),
            reply("{\"tool\":\"inspect\",\"args\":{}}"),
        ]);
        f.proxy.scripted.as_ref().unwrap().borrow_mut().reject_tools = true;
        let mut req = request();
        req["tool_choice"] = json!("auto");
        req["parallel_tool_calls"] = json!(true);
        let result = f.chat(req.clone());
        assert_eq!(result.status, 200);
        assert_eq!(args(&result), json!({}));
        let restored: Profiles = read_json(&f.dir.join("profiles.json")).unwrap();
        assert!(restored.values().next().unwrap().text_tool_mode);
        f.proxy.profiles = restored;
        req["messages"] = json!([
            {"role":"user","content":"hello"},
            {"role":"assistant","tool_calls":[{"id":"a","function":{"name":"inspect","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"a","content":"volume = 42"}
        ]);
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 3);
        for wire in &script.requests[1..] {
            for key in ["tools", "tool_choice", "parallel_tool_calls"] {
                assert!(wire.get(key).is_none());
            }
            assert!(wire["messages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["role"] != "tool" && m.get("tool_calls").is_none()));
        }
        assert!(script.requests[2]
            .to_string()
            .contains("Tool inspect returned: volume = 42"));
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        for kind in [
            "text_tool_mode",
            "tools_stripped",
            "tool_history_flattened",
            "text_call_converted",
        ] {
            assert!(log.contains(kind));
        }
    }

    #[test]
    fn text_mode_preserves_recipes_guards_how_and_refusals() {
        for text in [
            "Hello.",
            "{\"tool\":\"inspect\",\"args\":{}} {\"tool\":\"inspect\",\"args\":{}}",
        ] {
            let mut f = Fixture::new(vec![reply(text)]);
            f.proxy.scripted.as_ref().unwrap().borrow_mut().reject_tools = true;
            let result = f.chat(request());
            assert_eq!(result.status, 200);
            let value: Value = serde_json::from_slice(&result.body).unwrap();
            assert_eq!(value["choices"][0]["message"]["content"], text);
        }
        let mut f = Fixture::new(vec![reply("I will make a 20 mm cube")]);
        f.proxy.scripted.as_ref().unwrap().borrow_mut().reject_tools = true;
        let mut req = request();
        req["messages"][0]["content"] = json!("make a 20 mm cube");
        assert_eq!(args(&f.chat(req))["command"], "solid.box");
        let mut f = Fixture::new(vec![
            reply("I'll use solid.box."),
            reply("{\"tool\":\"inspect\",\"args\":{}}"),
        ]);
        f.proxy.scripted.as_ref().unwrap().borrow_mut().reject_tools = true;
        let mut req = request();
        req["messages"][0]["content"] = json!("make a box");
        assert_eq!(f.chat(req).status, 200);
        assert!(fs::read_to_string(f.dir.join("log.jsonl"))
            .unwrap()
            .contains("hand_back"));
        let mut f = Fixture::new(vec![
            reply("{\"tool\":\"how\",\"args\":{\"task\":\"make a box\"}}"),
            reply("Ready."),
        ]);
        f.proxy.cfg.no_how = false;
        f.proxy.scripted.as_ref().unwrap().borrow_mut().reject_tools = true;
        assert_eq!(f.chat(request()).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert!(script.requests[2]
            .to_string()
            .contains("Tool how returned:"));
        assert!(script.requests[2]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["role"] != "tool"));
    }

    #[test]
    fn direct_all_recipe_phrasings_and_nonrecipe_passthrough() {
        for enabled in [false, true] {
            for (_, phrases) in planner::cases::CASES {
                for phrase in *phrases {
                    let mut f = Fixture::new(vec![]);
                    f.proxy.cfg.direct_recipes = true;
                    f.proxy.cfg.planner = enabled;
                    let result = f.chat(planner::cases::request(phrase));
                    assert_eq!(
                        result.status,
                        200,
                        "{phrase}: {}",
                        String::from_utf8_lossy(&result.body)
                    );
                    assert!(
                        f.proxy
                            .scripted
                            .as_ref()
                            .unwrap()
                            .borrow()
                            .requests
                            .is_empty(),
                        "{phrase}"
                    );
                }
            }
        }
        let mut f = Fixture::new(vec![reply("Hello.")]);
        f.proxy.cfg.direct_recipes = true;
        let mut req = request();
        req["messages"][0]["content"] = json!("hello");
        let result = f.chat(req.clone());
        let value: Value = serde_json::from_slice(&result.body).unwrap();
        assert_eq!(value["choices"][0]["message"]["content"], "Hello.");
        assert_eq!(f.proxy.scripted.as_ref().unwrap().borrow().requests[0], req);
        assert_eq!(f.proxy.profiles.values().next().unwrap().turns, 1);
    }
    #[test]
    fn no_model_models_endpoint_and_missing_results_are_explicit() {
        let mut f = Fixture::new(vec![]);
        f.proxy.cfg.direct_recipes = true;
        f.proxy.cfg.upstream = "none".into();
        let models = f
            .proxy
            .handle(Request {
                method: "GET".into(),
                path: "/v1/models".into(),
                auth: None,
                body: vec![],
            })
            .unwrap();
        let value: Value = serde_json::from_slice(&models.body).unwrap();
        assert_eq!(value["data"][0]["id"], "pilot-recipes");
        let mut req = planner::cases::request("create a cube 1/4 inch");
        let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(first["choices"][0]["message"].clone());
        let result = f.chat(req);
        assert_eq!(result.status, 200);
        assert!(String::from_utf8(result.body)
            .unwrap()
            .contains("missing or invalid"));
    }

    fn direct_results(req: &mut Value, message: Value, error: bool, volume: f64) {
        let calls = message["tool_calls"].as_array().unwrap().clone();
        req["messages"].as_array_mut().unwrap().push(message);
        for (i, call) in calls.iter().enumerate() {
            let content = if error && i == 0 {
                json!({"isError":true,"error":"tool failed"})
            } else {
                json!({"bodies":[{"name":"Blank","volume_mm3":1000},{"name":"New","volume_mm3":volume,"bbox":{"min":[0,0,0],"max":[6.35,6.35,6.35]}}]})
            };
            req["messages"].as_array_mut().unwrap().push(
                json!({"role":"tool","tool_call_id":call["id"],"content":content.to_string()}),
            );
        }
    }
    #[test]
    fn direct_polite_creation_questions_skip_upstream() {
        for (text, command) in [
            (
                "Would you make a solid sphere measuring 26 mm in diameter?",
                "solid.sphere",
            ),
            (
                "Could you build a box that measures 36 by 24 by 8 mm?",
                "solid.box",
            ),
            (
                "Can you create a solid cylinder with a size of diameter 12 and height 25 mm?",
                "solid.cylinder",
            ),
            (
                "Would you make a solid plate sized 48x26x5 mm?",
                "solid.box",
            ),
        ] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            let first = f.chat(planner::cases::request(text));
            assert_eq!(first.status, 200, "{text}");
            let value: Value = serde_json::from_slice(&first.body).unwrap();
            let call = &value["choices"][0]["message"]["tool_calls"][0];
            assert_eq!(call["function"]["name"], "execute", "{text}: {value}");
            let args: Value =
                serde_json::from_str(call["function"]["arguments"].as_str().unwrap()).unwrap();
            assert_eq!(args["command"], command);
            assert!(f
                .proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow()
                .requests
                .is_empty());
        }
    }

    #[test]
    fn direct_query_recorded_results_and_units() {
        let fixtures: Vec<Value> = [
            include_str!("../tests/fixtures/inspect-volume-granite-0.20.json"),
            include_str!("../tests/fixtures/inspect-volume-qwen-0.20.json"),
            include_str!("../tests/fixtures/measure-volume-0.20.json"),
        ]
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
        for payload in fixtures {
            for stream in [false, true] {
                for (question, expected) in [
                    ("Report the volume of the current solid in cubic millimeters; leave it unchanged.", "Exact match (no model): volume 9520 mm³"),
                    ("What volume does this design have in cm³?", "Exact match (no model): volume 9.52 cm³ (converted from 9520 mm³; 1 cm³ = 1000 mm³)"),
                    ("Tell me the volume in cubic inches", "1 in³ = 16387.064 mm³"),
                    ("How many bodies are in this document?", "Exact match (no model): 1 bodies"),
                ] {
                    let mut f = Fixture::new(vec![]);
                    f.proxy.cfg.direct_recipes = true;
                    let mut req = planner::cases::request(question);
                    // Query starts without a measured baseline.
                    req["messages"].as_array_mut().unwrap().remove(0);
                    req["stream"] = json!(stream);
                    let decode = |reply: Reply| -> Value {
                        assert_eq!(reply.status, 200, "{}", String::from_utf8_lossy(&reply.body));
                        if stream { collect_sse(&reply.body).unwrap() } else { serde_json::from_slice(&reply.body).unwrap() }
                    };
                    let first = decode(f.chat(req.clone()));
                    let message = first["choices"][0]["message"].clone();
                    let call = &message["tool_calls"][0];
                    assert_eq!(call["function"]["name"], "execute");
                    assert_eq!(serde_json::from_str::<Value>(call["function"]["arguments"].as_str().unwrap()).unwrap(), json!({"command":"document.inspect","params":{"measure":true}}));
                    let tool = json!({"role":"tool","tool_call_id":call["id"],"content":payload.to_string()});
                    req["messages"].as_array_mut().unwrap().extend([message, tool]);
                    let second = decode(f.chat(req));
                    let text = second["choices"][0]["message"]["content"].as_str().unwrap();
                    assert!(text.contains(expected), "{question}: {text}");
                    assert!(!text.contains("completed"));
                    assert!(f.proxy.scripted.as_ref().unwrap().borrow().requests.is_empty());
                }
            }
        }
    }

    #[test]
    fn direct_query_missing_or_ambiguous_facts_hand_back() {
        for (question, payload) in [
            ("Tell me the volume", json!({"bodies":[{"name":"A"}]})),
            (
                "Tell me the volume",
                json!({"bodies":[{"name":"A","volume_mm3":-1}]}),
            ),
            (
                "Tell me the volume",
                json!({"bodies":[{"name":"A","volume_mm3":10}],"total":{"volume_mm3":20}}),
            ),
            (
                "Tell me the volume of the current solid",
                json!({"bodies":[{"name":"A","volume_mm3":10},{"name":"B","volume_mm3":20}]}),
            ),
            (
                "Tell me the volume of Missing",
                json!({"bodies":[{"name":"A","volume_mm3":10}]}),
            ),
            ("Count the bodies", json!({"bodies":["A"]})),
            (
                "Count the bodies",
                json!({"bodies":[{"name":"A"}],"body_count":2}),
            ),
            (
                "Count the bodies",
                json!({"bodies":[{"name":"A"}],"truncated":true}),
            ),
            (
                "Count the bodies",
                json!({"isError":true,"error":"cannot inspect"}),
            ),
        ] {
            let mut f = Fixture::new(vec![reply("I need a complete measurement.")]);
            f.proxy.cfg.direct_recipes = true;
            let mut req = planner::cases::request(question);
            req["messages"].as_array_mut().unwrap().remove(0);
            let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            let message = first["choices"][0]["message"].clone();
            let id = message["tool_calls"][0]["id"].clone();
            req["messages"].as_array_mut().unwrap().extend([
                message,
                json!({"role":"tool","tool_call_id":id,"content":payload.to_string()}),
            ]);
            let second = f.chat(req);
            assert_eq!(second.status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 1, "{question}: {payload}");
            assert!(script.requests[0].to_string().contains("Recipe completion"));
        }
    }
    #[test]
    fn direct_calls_and_measured_replies_never_use_upstream() {
        for stream in [false, true] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            f.proxy.cfg.planner = false;
            let mut req = planner::cases::request("create a cube 1/4 inch");
            req["stream"] = json!(stream);
            let first = f.chat(req.clone());
            assert_eq!(first.status, 200);
            let value: Value = if stream {
                collect_sse(&first.body).unwrap()
            } else {
                serde_json::from_slice(&first.body).unwrap()
            };
            assert_eq!(value["choices"][0]["finish_reason"], "tool_calls");
            assert_eq!(value["model"], "pilot-recipes");
            let result_start = req["messages"].as_array().unwrap().len();
            direct_results(
                &mut req,
                value["choices"][0]["message"].clone(),
                false,
                256.047875,
            );
            // Completion must still read full measured evidence above the trim threshold.
            for message in &mut req["messages"].as_array_mut().unwrap()[result_start..] {
                if message["role"] == "tool" {
                    let mut data: Value =
                        serde_json::from_str(message["content"].as_str().unwrap()).unwrap();
                    data["history_padding"] = json!((0..200).collect::<Vec<_>>());
                    message["content"] = json!(data.to_string());
                }
            }
            f.proxy.cfg.trim_cap = 256;
            let second = f.chat(req);
            assert_eq!(second.status, 200);
            let value: Value = if stream {
                collect_sse(&second.body).unwrap()
            } else {
                serde_json::from_slice(&second.body).unwrap()
            };
            let text = value["choices"][0]["message"]["content"].as_str().unwrap();
            assert!(text.starts_with("Exact match (no model):"));
            assert!(text.contains("1256.047875 mm³") && text.contains("6.35 × 6.35 × 6.35 mm"));
            assert!(f
                .proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow()
                .requests
                .is_empty());
            let p = f.proxy.profiles.values().next().unwrap();
            assert_eq!((p.direct_turns, p.turns, p.native, p.prose), (2, 0, 0, 0));
            assert!(!p.capabilities_checked && p.recent_text_calls.is_empty());
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(
                log.contains("direct_recipe_calls")
                    && log.contains("direct_recipe_reply")
                    && log.contains("from_model")
            );
        }
    }

    #[test]
    fn real_assistant_dimensioned_profile_is_direct() {
        let request: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/constrained-profile-assistant-0.20.json"
        ))
        .unwrap();
        let mut f = Fixture::new(vec![]);
        f.proxy.cfg.direct_recipes = true;
        f.proxy.cfg.planner = false;
        let response: Value = serde_json::from_slice(&f.chat(request).body).unwrap();
        let calls = response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        let args: Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        let commands = args["commands"].as_array().unwrap();
        assert_eq!(
            commands[0],
            json!({"command":"sketch.create","params":{"name":"L profile","plane":"XZ"}})
        );
        assert!(commands
            .iter()
            .any(|c| c["command"] == "solid.extrude" && c["params"]["distance"] == 30.));
        assert!(commands.iter().any(|c| c["command"] == "sketch.dimension"));
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
    fn real_assistant_joint_prerequisites_and_truncation() {
        let requests: Vec<Value> = serde_json::from_str(include_str!(
            "../tests/fixtures/revolute-assistant-0.20.json"
        ))
        .unwrap();
        let mut f = Fixture::new(vec![]);
        f.proxy.cfg.direct_recipes = true;
        let first: Value = serde_json::from_slice(&f.chat(requests[0].clone()).body).unwrap();
        assert_eq!(first["choices"][0]["message"], requests[1]["messages"][2]);
        let next: Value = serde_json::from_slice(&f.chat(requests[1].clone()).body).unwrap();
        let topology = next["choices"][0]["message"].clone();
        let calls = topology["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 4);
        assert!(calls.iter().all(|c| c["function"]["name"] == "execute"));
        assert_ne!(
            calls[0]["id"],
            requests[1]["messages"][2]["tool_calls"][0]["id"]
        );

        // Preserve the real, invalid request as evidence; never salvage partial
        // topology in production. The new calls avoid its per-message truncation.
        assert!(tool_data(&requests[2]["messages"][5]).is_none());
        let mut truncated = requests[1].clone();
        truncated["messages"]
            .as_array_mut()
            .unwrap()
            .push(topology.clone());
        for c in calls {
            truncated["messages"].as_array_mut().unwrap().push(json!({
                "role":"tool", "tool_call_id":c["id"],
                "content":requests[2]["messages"][5]["content"]
            }));
        }
        f.proxy.cfg.upstream = "none".into();
        let failed = f.chat(truncated);
        assert!(String::from_utf8(failed.body)
            .unwrap()
            .contains("missing or invalid"));

        // The first three complete topology entries really reached Pilot. The
        // last was cut mid-entry; use the existing public CLI recording for it.
        let text = requests[2]["messages"][5]["content"].as_str().unwrap();
        let mut tail = text.split_once("\"results\": [").unwrap().1;
        let mut data = Vec::new();
        for _ in 0..3 {
            let mut stream = serde_json::Deserializer::from_str(tail).into_iter::<Value>();
            data.push(stream.next().unwrap().unwrap());
            tail = tail[stream.byte_offset()..]
                .trim_start()
                .strip_prefix(',')
                .unwrap();
        }
        let last: Value = include_str!("../tests/fixtures/revolute-public-0.20.jsonl")
            .lines()
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .find(|v| v["id"] == 14)
            .unwrap();
        data.push(guidance::payload(&last));
        for (wrapped, as_json) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            f.chat(requests[0].clone());
            f.chat(requests[1].clone());
            let mut req = requests[1].clone();
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(topology.clone());
            // Tool messages may arrive in reverse order; match by their ids.
            for (c, d) in calls.iter().zip(&data).rev() {
                let content = if wrapped {
                    json!({"content":[{"type":"text","text":d.to_string()}],"isError":false})
                } else {
                    d.clone()
                };
                let text = serde_json::to_string_pretty(&content).unwrap();
                assert!(text.len() < 20_000);
                let content = if as_json { content } else { json!(text) };
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","tool_call_id":c["id"],"content":content}));
            }
            let response: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
            let batch: Value = serde_json::from_str(
                response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            let joint = batch["commands"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["command"] == "joint.create")
                .unwrap();
            assert_eq!(joint["params"]["flip"], true);
            assert!(joint["params"]["a"]["circle"].is_array());
            assert!(joint["params"]["b"]["circle"].is_array());
            assert!((joint["params"]["a"]["circle"][2].as_f64().unwrap() - 10.).abs() < 0.001);
            assert!(f
                .proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow()
                .requests
                .is_empty());
        }
        let mut f = Fixture::new(vec![]);
        f.proxy.cfg.direct_recipes = true;
        f.proxy.cfg.upstream = "none".into();
        f.chat(requests[0].clone());
        let mut req = requests[1].clone();
        let mut data = tool_data(&req["messages"][3]).unwrap();
        data["ok"] = json!(false);
        req["messages"][3]["content"] = json!(data.to_string());
        let response = f.chat(req);
        assert!(String::from_utf8(response.body)
            .unwrap()
            .contains("missing or invalid"));
    }

    #[test]
    fn direct_revolute_inspects_then_verifies_without_upstream() {
        let recorded: Vec<Value> = include_str!("../tests/fixtures/revolute-public-0.20.jsonl")
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        let result = |c: &Value, final_stage: bool| -> Value {
            let id = match c["command"].as_str().unwrap() {
                "document.inspect" => 9,
                "component.list" => {
                    if final_stage {
                        17
                    } else {
                        10
                    }
                }
                "joint.list" if !final_stage => {
                    return json!({"joints":[],"origins":[],"errors":[],"conflicts":[]})
                }
                "joint.list" => 18,
                "model.faces" => {
                    if c["params"]["body"] == "Base" {
                        11
                    } else {
                        13
                    }
                }
                "model.edges" => {
                    if c["params"]["body"] == "Base" {
                        12
                    } else {
                        14
                    }
                }
                "occurrence.ground" => return json!({}),
                "joint.create" => 15,
                "joint.drive" => 16,
                other => panic!("unexpected {other}"),
            };
            let mut d: Value = serde_json::from_str(
                recorded.iter().find(|v| v["id"] == id).unwrap()["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            if id == 17 {
                d["components"][1]["occurrences"][0]["grounded"] = json!(true);
            }
            d
        };
        for (batch, shortcut) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut f = Fixture::new(if shortcut {
                vec![call(
                    "pilot_build",
                    json!({"shape":"revolute_joint","pin_component":"Base","hole_component":"Arm","angle_deg":45}),
                )]
            } else {
                vec![]
            });
            f.proxy.cfg.direct_recipes = !shortcut;
            f.proxy.cfg.planner = !shortcut;
            f.proxy.cfg.no_shortcuts = !shortcut;
            let mut req = planner::cases::revolute_request(
                "Connect Base and Arm with a revolute joint then drive 45 degrees.",
            );
            req["messages"].as_array_mut().unwrap().truncate(1);
            if shortcut {
                req["messages"][0]["content"] = json!("Fit these components as specified.");
                req["tools"][0] = tools()[0].clone();
            }
            if batch {
                req["tools"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"function":{"name":"batch"}}));
            }
            for stage in 0..3 {
                let response: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
                let m = response["choices"][0]["message"].clone();
                let cs = m["tool_calls"].as_array().unwrap();
                req["messages"].as_array_mut().unwrap().push(m.clone());
                for c in cs {
                    let args: Value =
                        serde_json::from_str(c["function"]["arguments"].as_str().unwrap()).unwrap();
                    let mut data = if c["function"]["name"] == "batch" {
                        json!({"ok":true,"results":args["commands"].as_array().unwrap().iter().map(|a|result(a,stage==2)).collect::<Vec<_>>()})
                    } else {
                        result(&args, stage == 2)
                    };
                    // Joint face/edge inspection and final verification must remain whole.
                    data["history_padding"] = json!((0..500).collect::<Vec<_>>());
                    req["messages"].as_array_mut().unwrap().push(
                        json!({"role":"tool","tool_call_id":c["id"],"content":data.to_string()}),
                    );
                }
            }
            let response: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
            assert!(response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .starts_with("Exact match (no model):"));
            assert!(!response.to_string().contains("failed"), "{response}");
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                usize::from(shortcut)
            );
        }
    }

    #[test]
    fn selected_pattern_inspection_continues_and_verifies_without_model_replay() {
        for feature in [false, true] {
            let mut f = Fixture::with_shortcuts(vec![call(
                "pilot_build",
                json!({"recipe":"linear_pattern","args":{"target":if feature {"last_feature"}else{"body"},"count":3,"spacing":10,"direction":"X"}}),
            )]);
            f.proxy.cfg.direct_recipes = false;
            f.proxy.cfg.no_fill = true; // Exercise explicitly selected pilot_build without a forced form.
            f.proxy.cfg.planner = false;
            let mut req = planner::cases::request("Repeat the seed with my selected recipe");
            req["messages"] =
                json!([{"role":"user","content":"Repeat the seed with my selected recipe"}]);
            req["tools"][0] = tools()[0].clone();
            let before = json!({"bodies":[{"name":"Seed","volume_mm3":24,"bbox":{"min":[0,0,0],"max":[2,3,4]}}],"timeline":[{"id":1,"name":"Box1","type":"BoxFeature","error":null}]});
            let mut after = before.clone();
            after["bodies"] = json!([
                before["bodies"][0].clone(),
                before["bodies"][0].clone(),
                before["bodies"][0].clone()
            ]);
            after["timeline"].as_array_mut().unwrap().push(json!({"name":"RectangularPattern1","type":"RectangularPatternFeature","error":null}));
            let mut patterns = 0;
            for stage in 0..5 {
                let response: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
                let message = response["choices"][0]["message"].clone();
                let Some(calls) = message["tool_calls"].as_array() else {
                    assert!(stage >= 2, "{message}");
                    assert!(
                        message["content"]
                            .as_str()
                            .unwrap()
                            .contains("Exact match (no model):"),
                        "{message}"
                    );
                    assert_eq!(patterns, 1);
                    break;
                };
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(message.clone());
                for call in calls {
                    let args: Value =
                        serde_json::from_str(call["function"]["arguments"].as_str().unwrap())
                            .unwrap();
                    let data = match args["command"].as_str().unwrap() {
                        "document.inspect" => {
                            if patterns == 0 {
                                before.clone()
                            } else {
                                after.clone()
                            }
                        }
                        "timeline.inspect" => {
                            json!({"id":1,"name":"Box1","type":"BoxFeature","error":null,"definition":{"body_names":["Seed"],"corner":[0,0,0],"length":"2","width":"3","height":"4","operation":"new_body"}})
                        }
                        "solid.pattern.rectangular" => {
                            patterns += 1;
                            json!({"created_bodies":["Seed (2)","Seed (3)"]})
                        }
                        other => panic!("unexpected {other}"),
                    };
                    req["messages"].as_array_mut().unwrap().push(
                        json!({"role":"tool","tool_call_id":call["id"],"content":data.to_string()}),
                    );
                }
            }
            assert_eq!(patterns, 1);
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                1
            );
        }
    }
    #[test]
    fn pattern_hint_is_once_per_conversation_and_respects_opt_out() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![reply("Checking."), reply("Checking.")]);
            f.proxy.cfg.direct_recipes = false;
            f.proxy.cfg.planner = false;
            f.proxy.cfg.no_hints = disabled;
            let calls:Vec<_>=(0..3).map(|i| json!({"id":format!("box{i}"),"type":"function","function":{"name":"execute","arguments":json!({"command":"solid.box","params":{"length":2,"width":3,"height":4,"corner":[i*10,0,0]}}).to_string()}})).collect();
            let mut req = planner::cases::request("Build these repeated blocks");
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"assistant","tool_calls":calls}));
            for i in 0..3 {
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","tool_call_id":format!("box{i}"),"content":"{}"}));
            }
            f.chat(req.clone());
            f.chat(req);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            let counts: Vec<_> = script
                .requests
                .iter()
                .map(|r| {
                    r.to_string()
                        .matches("Use solid.pattern.rectangular with recipe linear_pattern")
                        .count()
                })
                .collect();
            assert_eq!(counts, if disabled { vec![0, 0] } else { vec![1, 0] });
        }
    }
    #[test]
    fn joint_contact_hint_is_once_per_conversation_and_respects_opt_out() {
        for disabled in [false, true] {
            let mut f = Fixture::new(vec![
                reply("I will inspect the joint."),
                reply("I will inspect the joint."),
            ]);
            f.proxy.cfg.direct_recipes = false;
            f.proxy.cfg.planner = false;
            f.proxy.cfg.no_hints = disabled;
            let req = planner::cases::revolute_finished(
                planner::cases::revolute_request("Connect Base and Arm with a revolute joint."),
                1,
            );
            f.chat(req.clone());
            f.chat(req);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            let counts: Vec<_> = script
                .requests
                .iter()
                .map(|r| r.to_string().matches("Pick the contact circles").count())
                .collect();
            assert_eq!(counts, if disabled { vec![0, 0] } else { vec![1, 0] });
        }
    }
    #[test]
    fn direct_failures_hand_back_once_and_disable_direct_for_the_turn() {
        for (error, volume) in [(true, 256.047875), (false, 100.0)] {
            for no_model in [false, true] {
                let mut f = Fixture::new(vec![reply("Recovery needed.")]);
                f.proxy.cfg.direct_recipes = true;
                if no_model {
                    f.proxy.cfg.upstream = "none".into();
                }
                let mut req = planner::cases::request("create a cube 1/4 inch");
                let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
                direct_results(
                    &mut req,
                    first["choices"][0]["message"].clone(),
                    error,
                    volume,
                );
                let second = f.chat(req);
                assert_eq!(second.status, 200);
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                if no_model {
                    assert!(script.requests.is_empty());
                    assert!(String::from_utf8(second.body)
                        .unwrap()
                        .contains("No model is configured to recover"));
                } else {
                    assert_eq!(script.requests.len(), 1);
                    assert!(script.requests[0].to_string().contains("Recipe completion"));
                }
                assert_eq!(
                    f.proxy
                        .profiles
                        .values()
                        .next()
                        .unwrap()
                        .direct_recipe_fallback,
                    1
                );
            }
        }
    }
    #[test]
    fn direct_no_model_nonrecipe_batch_and_multiple_intents() {
        let mut f = Fixture::new(vec![]);
        f.proxy.cfg.direct_recipes = true;
        f.proxy.cfg.upstream = "none".into();
        let result = f.chat(request());
        let text = String::from_utf8(result.body).unwrap();
        assert!(text.contains("No model is configured") && text.contains("sketch_extrude"));
        let mut req = planner::cases::request(
            "build a plate 72x48x6mm with a centred through hole diameter 8mm",
        );
        req["tools"] = tools();
        let value: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
        let calls = value["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls[0]["function"]["name"], "batch");
        assert!(
            serde_json::from_str::<Value>(calls[0]["function"]["arguments"].as_str().unwrap())
                .unwrap()["commands"]
                .is_array()
        );
        for enabled in [false, true] {
            let mut f = Fixture::new(vec![reply("I'll handle the rest.")]);
            f.proxy.cfg.direct_recipes = true;
            f.proxy.cfg.planner = enabled;
            assert_eq!(
                f.chat(planner::cases::request(
                    "make a 20mm cube and explain how to export it"
                ))
                .status,
                200
            );
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 1);
            let sent = script.requests[0].to_string();
            assert_eq!(sent.contains("Nothing has been done yet."), enabled);
            assert_eq!(
                sent.contains("must submit tool calls for the whole request"),
                enabled
            );
            assert_eq!(
                sent.contains("For this request, run exactly this plan:"),
                enabled
            );
            for claim in [
                "completed",
                "already executed",
                "worked out in code and offered",
                "Handle only the rest",
            ] {
                assert!(!sent.contains(claim), "{claim}: {sent}");
            }
        }
    }

    #[test]
    fn provider_failures_are_counted_and_persisted() {
        let mut f = Fixture::new(vec![]);
        assert_eq!(f.chat(request()).status, 502);
        let records = load_profiles(Some(&f.dir), Some("unit")).unwrap();
        assert_eq!(records[0].provider_errors, 1);
        assert_eq!(records[0].turns, 1);
    }

    #[test]
    fn provider_sentences_and_guard_scope() {
        for (status, code, expected) in [
            (429, "rate_limit_exceeded", "rate_limited"),
            (429, "insufficient_quota", "out_of_quota"),
            (401, "", "bad_key"),
            (404, "", "missing_model"),
            (500, "", "provider_error"),
        ] {
            let response = provider_error(&Upstream {
                status,
                body: json!({"error":{"code":code,"message":"never expose provider secrets"}})
                    .to_string()
                    .into_bytes(),
                content_type: "application/json".into(),
                retry_after: Some("7".into()),
            });
            let v: Value = serde_json::from_slice(&response.body).unwrap();
            assert_eq!(v["error"]["code"], expected);
            assert!(!String::from_utf8(response.body)
                .unwrap()
                .contains("secrets"));
        }
        let catalog = Catalog::bundled().unwrap();
        let mut req = request();
        req["messages"][0]["content"] = json!("what command makes a box?");
        assert!(prose_guard(&req, &json!({"content":"Use solid.box."}), &catalog).is_none());
        req["messages"][0]["content"] = json!("explain solid.box");
        assert!(prose_guard(&req, &json!({"content":"Use solid.box."}), &catalog).is_none());
        req["messages"][0]["content"] = json!("what is the volume of `block`?");
        assert!(prose_guard(&req, &json!({"content":"20"}), &catalog)
            .unwrap()
            .0
            .contains("look it up first"));
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":"20"}));
        assert!(prose_guard(&req, &json!({"content":"20"}), &catalog).is_none());
    }
    #[test]
    fn scripted_solidcraft98_dialect_repair_and_catalogue_isolation() {
        let mut f = Fixture::new(vec![
            json!({"choices":[{"message":{"role":"assistant","content":"{tool:'dblclick',args:{name:'Sketch1'}}"},"finish_reason":"stop"}]}),
        ]);
        f.proxy.cfg.solidcraft98 = true;
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_hints = true;
        f.proxy.cfg.no_how = true;
        f.proxy.catalog = Catalog::from_json("[]").unwrap();
        let r = f.chat(request());
        let response: Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            response["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "doubleclick"
        );
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        let body = &script.requests[0];
        assert_eq!(body["tools"].as_array().unwrap().len(), 11);
        assert!(!body["tools"].to_string().contains("execute"));
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("solidcraft98_alias"));
        assert!(!log.contains("\"kind\":\"plan\""));
        drop(script);
        let mut f = Fixture::new(vec![
            json!({"choices":[{"message":{"role":"assistant","content":"{\"tool\":\"solid.cube\",\"args\":{\"side\":20}}"},"finish_reason":"stop"}]}),
        ]);
        f.proxy.cfg.solidcraft98 = true;
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_hints = true;
        f.proxy.cfg.no_how = true;
        f.proxy.catalog = Catalog::from_json("[]").unwrap();
        let r = f.chat(request());
        let response: Value = serde_json::from_slice(&r.body).unwrap();
        assert!(response["choices"][0]["message"]["tool_calls"].is_null());
    }
    #[test]
    fn direct_compound_plate_reads_assistant_edge_results() {
        for plain in [false, true] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            let mut req = planner::cases::request("Make a plate 120x100x10mm with its corner at the origin with a centred bore radius 5mm through then round all edges radius 1mm");
            req["tools"]
                .as_array_mut()
                .unwrap()
                .push(json!({"function":{"name":"batch"}}));
            let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            let message = first["choices"][0]["message"].clone();
            let calls = message["tool_calls"].as_array().unwrap();
            // Creation prefix followed by a separate topology lookup, as the
            // Assistant's plain results would appear in a real request.
            assert_eq!(calls.len(), 3);
            let args: Value =
                serde_json::from_str(calls[2]["function"]["arguments"].as_str().unwrap()).unwrap();
            assert_eq!(args["command"], "model.edges");
            let body = args["params"]["body"].clone();
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(message.clone());
            let mids = [
                [0., 0., 5.],
                [120., 100., 5.],
                [60., 0., 0.],
                [60., 0., 10.],
                [65., 50., 0.],
                [65., 50., 10.],
            ];
            for (i, c) in calls.iter().enumerate() {
                let mut data = if i == 2 {
                    json!({"body":body,"edges":mids.iter().enumerate().map(|(index, mid)|json!({"index":index,"mid":mid})).collect::<Vec<_>>()})
                } else {
                    json!({"created_bodies":[body],"error":null})
                };
                // The middle edges are required, and would be lost by compaction.
                data["history_padding"] = json!((0..500).collect::<Vec<_>>());
                let content = if plain {
                    data
                } else {
                    json!({"content":[{"type":"text","text":data.to_string()}]})
                };
                req["messages"].as_array_mut().unwrap().push(
                    json!({"role":"tool","tool_call_id":c["id"],"content":content.to_string()}),
                );
            }
            let next: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
            let args: Value = serde_json::from_str(
                next["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(args["command"], "solid.fillet");
            assert_eq!(args["params"]["body"], body);
            assert_eq!(args["params"]["edges"], json!(mids));
            assert!(f
                .proxy
                .scripted
                .as_ref()
                .unwrap()
                .borrow()
                .requests
                .is_empty());
        }
    }

    #[test]
    fn direct_natural_first_turn_edits_with_recorded_inspection() {
        let inspect: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/inspect-single-body-0.20.json"
        ))
        .unwrap();
        let topology: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/edges-single-body-0.20.json"
        ))
        .unwrap();
        let edges: Vec<Value> = (0..12)
            .map(|index| json!({"body":"Body1","index":index}))
            .collect();
        for (text, expected, needs_edges) in [
            (
                "Fillet all edges with a radius of 1 mm.",
                json!({"command":"solid.fillet","params":{"body":"Body1","edges":edges,"radius":1.0}}),
                true,
            ),
            (
                "Chamfer all edges by 1 mm.",
                json!({"command":"solid.chamfer","params":{"body":"Body1","edges":edges,"distance":1.0}}),
                true,
            ),
            (
                "Move this body 9 mm along -Y.",
                json!({"command":"solid.move","params":{"bodies":["Body1"],"translate":[0.0,-9.0,0.0]}}),
                false,
            ),
            (
                "Call this body Mount.",
                json!({"command":"body.rename","params":{"body":"Body1","name":"Mount"}}),
                false,
            ),
            (
                "Mirror this body across the X-Z plane keeping the original.",
                json!({"command":"solid.mirror","params":{"bodies":["Body1"],"plane":"XZ"}}),
                false,
            ),
            (
                "Double the size with the origin as the centre.",
                json!({"command":"solid.scale","params":{"bodies":["Body1"],"factor":2.0,"origin":[0,0,0]}}),
                false,
            ),
        ] {
            for (ambiguous, plain) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut f = Fixture::new(vec![reply("Which body should I edit?")]);
                f.proxy.cfg.direct_recipes = true;
                f.proxy.cfg.planner = false;
                let mut req = planner::cases::request(text);
                req["messages"] = json!([{"role":"system","content":"Use the offered tools for CAD edits."},{"role":"user","content":text}]);
                let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
                let message = first["choices"][0]["message"].clone();
                let calls = message["tool_calls"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{text}: {first}"));
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0]["function"]["name"], "execute");
                let args: Value =
                    serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap())
                        .unwrap();
                assert_eq!(
                    args,
                    json!({"command":"document.inspect","params":{"measure":true}})
                );
                assert!(f
                    .proxy
                    .scripted
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .requests
                    .is_empty());
                let mut result = inspect.clone();
                if ambiguous {
                    let mut data: Value =
                        serde_json::from_str(result["content"][0]["text"].as_str().unwrap())
                            .unwrap();
                    let mut second = data["bodies"][0].clone();
                    second["name"] = json!("Body2");
                    data["bodies"].as_array_mut().unwrap().push(second);
                    result["content"][0]["text"] = json!(data.to_string());
                }
                if plain {
                    result = guidance::payload(&result);
                }
                req["messages"].as_array_mut().unwrap().extend([
                    message.clone(),
                    json!({"role":"tool","tool_call_id":calls[0]["id"],"content":result.to_string()}),
                ]);
                let mut next: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
                if ambiguous {
                    assert_eq!(
                        next["choices"][0]["message"]["content"], "Which body should I edit?",
                        "{text}: {next}"
                    );
                    assert!(next["choices"][0]["message"]["tool_calls"].is_null());
                    let scripted = f.proxy.scripted.as_ref().unwrap().borrow();
                    assert_eq!(scripted.requests.len(), 1, "{text}");
                    assert!(scripted.requests[0]
                        .to_string()
                        .contains("Recipe target is unresolved"));
                    continue;
                }
                if needs_edges {
                    let message = next["choices"][0]["message"].clone();
                    let calls = message["tool_calls"].as_array().unwrap();
                    assert_eq!(calls.len(), 1);
                    let args: Value =
                        serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap())
                            .unwrap();
                    assert_eq!(
                        args,
                        json!({"command":"model.edges","params":{"body":"Body1"}})
                    );
                    req["messages"].as_array_mut().unwrap().extend([
                        message.clone(),
                        json!({"role":"tool","tool_call_id":calls[0]["id"],"content":if plain {guidance::payload(&topology).to_string()} else {topology.to_string()}}),
                    ]);
                    next = serde_json::from_slice(&f.chat(req).body).unwrap();
                }
                let calls = next["choices"][0]["message"]["tool_calls"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{text}: {next}"));
                assert_eq!(calls.len(), 2, "{text}: {next}");
                for (call, expected) in calls.iter().zip([
                    expected.clone(),
                    json!({"command":"document.inspect","params":{"measure":true}}),
                ]) {
                    assert_eq!(call["function"]["name"], "execute");
                    let args: Value =
                        serde_json::from_str(call["function"]["arguments"].as_str().unwrap())
                            .unwrap();
                    assert_eq!(args, expected, "{text}");
                }
                assert!(f
                    .proxy
                    .scripted
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .requests
                    .is_empty());
            }
        }
    }

    #[test]
    fn direct_first_turn_body_lookup_then_edit_and_verification() {
        for text in [
            "move this box 8mm along X",
            "rename to Spacer",
            "mirror it across XZ",
            "double it about origin",
        ] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            f.proxy.cfg.planner = false;
            let mut req = planner::cases::request(text);
            req["messages"].as_array_mut().unwrap().remove(0);
            let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            let assistant = first["choices"][0]["message"].clone();
            let calls = assistant["tool_calls"].as_array().unwrap();
            assert_eq!(calls.len(), 1, "{text}: {first}");
            let args: Value =
                serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
            assert_eq!(
                args,
                json!({"command":"document.inspect","params":{"measure":true}})
            );
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(assistant.clone());
            req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":calls[0]["id"],"content":planner::cases::design().to_string()}));
            let second: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            let assistant = second["choices"][0]["message"].clone();
            let calls = assistant["tool_calls"]
                .as_array()
                .unwrap_or_else(|| panic!("{text}: {second}"));
            assert_eq!(calls.len(), 2, "{text}: {second}");
            assert!(!calls[0]["id"].as_str().unwrap().contains("resolve_body"));
            let mut after = planner::cases::design();
            if text.starts_with("rename") {
                after["bodies"][0]["name"] = json!("Spacer");
            }
            if text.starts_with("double") {
                after["bodies"][0]["volume_mm3"] = json!(8000);
            }
            if text.starts_with("mirror") {
                after["bodies"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"name":"Mirrored","volume_mm3":1000}));
            }
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(assistant.clone());
            for call in calls {
                req["messages"].as_array_mut().unwrap().push(
                    json!({"role":"tool","tool_call_id":call["id"],"content":after.to_string()}),
                );
            }
            let final_reply: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
            assert!(
                final_reply["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap()
                    .contains("completed"),
                "{text}: {final_reply}"
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
    }

    #[test]
    fn direct_body_lookup_refuses_ambiguous_failed_and_missing_results() {
        for result in [
            Some(json!({"bodies":[]})),
            Some(json!({"bodies":[{"name":"A"},{"name":"B"}]})),
            Some(json!({"isError":true})),
            Some(json!({"bodies":[{"name":"A"}],"truncated":true})),
            None,
        ] {
            let mut f = Fixture::new(vec![]);
            f.proxy.cfg.direct_recipes = true;
            f.proxy.cfg.upstream = "none".into();
            let mut req = planner::cases::request("move this box 8mm along X");
            req["messages"].as_array_mut().unwrap().remove(0);
            let first: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            let assistant = first["choices"][0]["message"].clone();
            let id = assistant["tool_calls"][0]["id"].clone();
            req["messages"].as_array_mut().unwrap().push(assistant);
            if let Some(result) = result {
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","tool_call_id":id,"content":result.to_string()}));
            }
            let next: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
            assert!(
                next["choices"][0]["message"]["tool_calls"].is_null(),
                "{next}"
            );
            let again: Value = serde_json::from_slice(&f.chat(req).body).unwrap();
            assert!(
                again["choices"][0]["message"]["tool_calls"].is_null(),
                "{again}"
            );
        }
    }
    #[test]
    fn shortcut_turn_rewrites_real_calls_and_verifies_even_without_direct_recipes() {
        for stream in [false, true] {
            let a = json!({"shape":"profile","kind":"L","plane":"XY","width":40,"height":30,"thickness":3,"depth":12});
            let mut f = Fixture::with_shortcuts(vec![call("pilot_build", a.clone())]);
            let mut req = request();
            req["stream"] = json!(stream);
            let expected = planner::shortcut(&a, &req, &f.proxy.catalog).unwrap();
            let result = f.chat(req.clone());
            let value = if stream {
                collect_sse(&result.body).unwrap()
            } else {
                serde_json::from_slice(&result.body).unwrap()
            };
            let message = value["choices"][0]["message"].clone();
            assert_eq!(message["tool_calls"], direct::calls(&req, &expected, 0));
            assert!(!message.to_string().contains("pilot_build"));
            {
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                assert_eq!(
                    script.requests[0]["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|t| t["function"]["name"] == "pilot_build")
                        .count(),
                    1
                );
            }
            req["stream"] = json!(false);
            let calls = message["tool_calls"].as_array().unwrap().clone();
            req["messages"].as_array_mut().unwrap().push(message);
            for call in calls {
                req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":call["id"],"content":json!({"bodies":[{"name":"New","volume_mm3":2412.0}]}).to_string()}));
            }
            let final_reply = f.chat(req);
            let result: Value = serde_json::from_slice(&final_reply.body).unwrap();
            assert!(result["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .contains("Exact match"));
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("\"shortcut\"") && log.contains("l_profile"));
        }
    }
    #[test]
    fn shortcut_rejection_retries_twice_then_removes_tool() {
        let invalid = json!({"shape":"profile","kind":"L","plane":"XY","width":40,"height":30,"thickness":40,"depth":12});
        let mut valid = invalid.clone();
        valid["thickness"] = json!(3);
        let mut f = Fixture::with_shortcuts(vec![
            call("pilot_build", invalid.clone()),
            call("pilot_build", valid),
        ]);
        let out = f.chat(request());
        assert_eq!(out.status, 200);
        let sent = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(sent.requests.len(), 2);
        let error = sent.requests[1]["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap();
        assert_eq!(error["role"], "tool");
        assert!(error["content"].as_str().unwrap().contains("web_thickness"));
        drop(sent);
        let mut f = Fixture::with_shortcuts(vec![
            call("pilot_build", invalid.clone()),
            call("pilot_build", invalid.clone()),
            call("pilot_build", invalid),
            reply("Please revise the wall thickness."),
        ]);
        let out = f.chat(request());
        assert!(!String::from_utf8_lossy(&out.body).contains("tool_calls"));
        let sent = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(sent.requests.len(), 4);
        assert!(!sent.requests[3]["tools"]
            .to_string()
            .contains("pilot_build"));
        assert!(!sent.requests[3]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "pilot_build"));
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert_eq!(log.matches("\"kind\":\"shortcut_rejected\"").count(), 3);
    }
    #[test]
    fn shortcut_history_disabled_dialect_and_exact_match_guards() {
        let mut untouched = json!({"messages":[{"role":"assistant","content":"","tool_calls":[]}]});
        let before = untouched.clone();
        shortcuts::strip(&mut untouched);
        assert_eq!(untouched, before);
        let mut req = request();
        req["messages"] = json!([
            {"role":"user","content":"old"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"local","function":{"name":"pilot_build","arguments":"{}"}},{"id":"real","function":{"name":"inspect","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"local","content":"not run"},
            {"role":"tool","tool_call_id":"real","content":"{}"},
            {"role":"user","content":"make something"}]);
        let mut f = Fixture::with_shortcuts(vec![reply("Need dimensions.")]);
        f.chat(req);
        let wire = &f.proxy.scripted.as_ref().unwrap().borrow().requests[0]["messages"];
        assert!(!wire.to_string().contains("pilot_build"));
        assert!(wire.to_string().contains("real"));
        for (disabled, dialect) in [(true, false), (false, true)] {
            let mut f = Fixture::with_shortcuts(vec![reply("Need dimensions.")]);
            f.proxy.cfg.no_shortcuts = disabled;
            f.proxy.cfg.solidcraft98 = dialect;
            f.chat(request());
            let wire = &f.proxy.scripted.as_ref().unwrap().borrow().requests[0];
            assert!(!wire["tools"].to_string().contains("pilot_build"));
        }
        let mut f = Fixture::with_shortcuts(vec![]);
        f.proxy.cfg.direct_recipes = true;
        let mut req = request();
        req["messages"][0]["content"] =
            json!("Make an L profile width 40 height 30 thickness 3 extrude 12");
        assert_eq!(f.chat(req).status, 200);
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
    fn shortcut_text_and_mixed_calls_never_escape() {
        let a = json!({"shape":"enclosure","length":80,"width":50,"height":30,"wall":2,"open":"top","centred":false});
        let mut f = Fixture::with_shortcuts(vec![reply(
            &json!({"tool":"pilot_build","args":a}).to_string(),
        )]);
        let out = f.chat(request());
        assert!(!String::from_utf8_lossy(&out.body).contains("pilot_build"));
        let mut mixed = call("pilot_build", json!({}));
        mixed["choices"][0]["message"]["tool_calls"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"other","function":{"name":"execute","arguments":"{}"}}));
        let mut f = Fixture::with_shortcuts(vec![mixed, reply("Need dimensions.")]);
        let out = f.chat(request());
        assert!(!String::from_utf8_lossy(&out.body).contains("tool_calls"));
    }

    #[test]
    fn find_then_build_keeps_local_history_and_audits_profile() {
        let a = json!({"recipe":"box","args":{"length":41,"width":23,"height":17}});
        let mut f = Fixture::with_shortcuts(vec![
            call("pilot_find", json!({"request":"a rectangular block"})),
            call("pilot_build", a.clone()),
        ]);
        f.proxy.cfg.planner = false;
        let req = request();
        let expected = f.proxy.recipes.build(&a, &req, &f.proxy.catalog).unwrap();
        let out = f.chat(req.clone());
        assert_eq!(out.status, 200);
        let value: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(
            value["choices"][0]["message"]["tool_calls"],
            direct::calls(&req, &expected, 0)
        );
        assert!(!value.to_string().contains("pilot_find"));
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        let messages = script.requests[1]["messages"].as_array().unwrap();
        let result: Value =
            serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap()).unwrap();
        assert_eq!(result["hit"], true);
        assert_eq!(result["recipes"][0]["id"], "box");
        let p = f.proxy.profiles.values().next().unwrap();
        assert_eq!((p.finds, p.find_hits, p.find_misses), (1, 1, 0));
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("\"kind\":\"find\""));
    }

    #[test]
    fn find_miss_and_disabled_tools() {
        let mut f = Fixture::with_shortcuts(vec![
            call("pilot_find", json!({"request":"write a sonnet"})),
            reply("Pilot builds boxes, tubes and plates. What part do you need?"),
        ]);
        let out = f.chat(request());
        assert_eq!(out.status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        let messages = script.requests[1]["messages"].as_array().unwrap();
        let result: Value =
            serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap()).unwrap();
        assert_eq!(result["hit"], false);
        let p = f.proxy.profiles.values().next().unwrap();
        assert_eq!((p.finds, p.find_hits, p.find_misses), (1, 0, 1));
        drop(script);
        let mut disabled = Fixture::with_shortcuts(vec![reply("Need dimensions.")]);
        disabled.proxy.cfg.no_shortcuts = true;
        disabled.chat(request());
        let script = disabled.proxy.scripted.as_ref().unwrap().borrow();
        let tools = script.requests[0]["tools"].to_string();
        assert!(!tools.contains("pilot_find") && !tools.contains("pilot_build"));
    }
    fn fill_request() -> Value {
        let mut req = request();
        req["messages"][0]["content"] =
            json!("Extrude an L profile on XZ 47 wide 33 tall legs 5 thick depth 22");
        req
    }
    fn fill_fixture(replies: Vec<Value>) -> Fixture {
        let mut f = Fixture::with_shortcuts(replies);
        f.proxy.cfg.offer_recipe_tools = false;
        f.proxy.cfg.planner = false;
        f.proxy.cfg.no_fill = false;
        f
    }
    fn filled() -> Value {
        call(
            "pilot_fill",
            json!({"kind":"L","plane":"XZ","width":47,"height":33,"thickness":5,"depth":22,"decline":false}),
        )
    }

    fn route_request() -> Value {
        let mut req = fill_request();
        req["messages"] = json!([{"role":"user","content":"hinge pivot two parts"}]);
        req
    }

    // Extraction-only fixtures acknowledge the new public read prerequisite with
    // an empty document. Geometry behavior is exercised against the real binary.
    fn chat_forms(f: &mut Fixture, mut req: Value) -> Reply {
        for _ in 0..4 {
            let out = f.chat(req.clone());
            let parsed = if out.content_type == "text/event-stream" {
                collect_sse(&out.body)
            } else {
                serde_json::from_slice::<Value>(&out.body).map_err(|e| e.to_string())
            };
            let Ok(value) = parsed else {
                return out;
            };
            let message = &value["choices"][0]["message"];
            let Some(calls) = message["tool_calls"].as_array().filter(|cs| {
                !cs.is_empty()
                    && cs.iter().all(|c| {
                        c["id"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("pilot_context_"))
                    })
            }) else {
                return out;
            };
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(message.clone());
            for call in calls {
                req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":call["id"],"content":json!({"bodies":[],"timeline":[],"components":[],"joints":[],"origins":[]}).to_string()}));
            }
        }
        panic!("form inspection did not finish");
    }

    fn public_l1() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/public-constraints-l1-fill-guard.json"
        ))
        .unwrap()
    }

    #[test]
    fn public_l1_offform_is_discarded_before_normal_tool_retry() {
        let data = public_l1();
        for route in [false, true] {
            for mixed in [false, true] {
                let mut req = data["requests"][0].clone();
                if route {
                    req["messages"][1]["content"] =
                        route_request()["messages"][0]["content"].clone();
                }
                let mut off = data["offform"].clone();
                if mixed {
                    // An offered call must not hide an unoffered call, even in
                    // another choice (providers need not obey n=1 either).
                    off["choices"].as_array_mut().unwrap().insert(
                        0,
                        call(
                            if route { "pilot_route" } else { "pilot_fill" },
                            json!({"decline":true}),
                        )["choices"][0]
                            .clone(),
                    );
                }
                let mut f = fill_fixture(vec![off.clone(), off, reply("Please clarify.")]);
                let out = chat_forms(&mut f, req);
                assert_eq!(out.status, 200);
                let out: Value = serde_json::from_slice(&out.body).unwrap();
                assert!(out["choices"][0]["message"]["tool_calls"].is_null());
                let script = f.proxy.scripted.as_ref().unwrap().borrow();
                assert_eq!(script.requests.len(), 3);
                assert_eq!(
                    script.requests[0]["messages"][1],
                    script.requests[1]["messages"][1]
                );
                assert!(script.requests[1]["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("Previous reply problem"));
                assert_eq!(
                    script.requests[0]["tools"][0]["function"]["name"],
                    if route { "pilot_route" } else { "pilot_fill" }
                );
                assert!(!script.requests[2]["tools"]
                    .to_string()
                    .contains("pilot_fill"));
                assert!(!script.requests[2]["tools"]
                    .to_string()
                    .contains("pilot_route"));
                assert_eq!(script.requests[0]["messages"].as_array().unwrap().len(), 2);
                assert_eq!(script.requests[2]["messages"][0]["role"], "system");
                let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
                assert!(log.contains("fill_offform") && log.contains("\"tool\":\"batch\""));
            }
        }
    }

    #[test]
    fn public_l1_sequence_and_resends_never_reoffer_forms() {
        let data = public_l1();
        let mut f = fill_fixture(vec![
            data["offform"].clone(),
            data["offform"].clone(),
            reply("Clarify."),
            reply("Ready."),
            reply("Ready."),
            reply("Ready."),
            reply("Ready."),
        ]);
        assert_eq!(f.chat(data["requests"][0].clone()).status, 200);
        // Replay the real follow-up histories despite deliberately losing all
        // prefix-based conversation state between requests.
        for req in data["requests"].as_array().unwrap().iter().skip(1) {
            f.proxy.conversations.clear();
            assert_eq!(f.chat(req.clone()).status, 200);
        }
        let mut resend = data["requests"][0].clone();
        resend["messages"].as_array_mut().unwrap().remove(0);
        f.proxy.conversations.clear();
        assert_eq!(f.chat(resend).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 7);
        assert!(script
            .requests
            .iter()
            .skip(2)
            .all(|r| !r["tools"].to_string().contains("pilot_fill")
                && !r["tools"].to_string().contains("pilot_route")));
    }

    #[test]
    fn public_l1_accepted_fill_and_prior_calls_block_later_forms() {
        let data = public_l1();
        let mut f = fill_fixture(vec![
            call(
                "pilot_fill",
                json!({"kind":"L","plane":"XZ","width":50,"height":40,"thickness":8,"depth":30}),
            ),
            reply("Ready."),
            reply("Ready."),
        ]);
        let mut req = data["requests"][0].clone();
        let out = f.chat(req.clone());
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        let message = out["choices"][0]["message"].clone();
        assert!(message["tool_calls"]
            .as_array()
            .is_some_and(|cs| !cs.is_empty()));
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(message.clone());
        for call in message["tool_calls"].as_array().unwrap() {
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","tool_call_id":call["id"],"content":"{\"ok\":true}"}));
        }
        f.proxy.conversations.clear();
        assert_eq!(f.chat(req).status, 200);
        // A re-send with the tool history removed still cannot re-offer.
        f.proxy.conversations.clear();
        assert_eq!(f.chat(data["requests"][0].clone()).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 3);
        assert!(script
            .requests
            .iter()
            .skip(1)
            .all(|r| !r["tools"].to_string().contains("pilot_fill")));
        drop(script);
        // A cold proxy must also reject each already-acted public history.
        for req in data["requests"].as_array().unwrap().iter().skip(1) {
            let mut cold = fill_fixture(vec![reply("Ready.")]);
            assert_eq!(cold.chat(req.clone()).status, 200);
            assert!(
                !cold.proxy.scripted.as_ref().unwrap().borrow().requests[0]["tools"]
                    .to_string()
                    .contains("pilot_fill")
            );
        }
    }

    #[test]
    fn public_l3_code_array_remains_text_and_builds_nothing() {
        let data: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/public-constraints-l3-fill-guard.json"
        ))
        .unwrap();
        let mut f = fill_fixture(vec![
            data["response"].clone(),
            data["response"].clone(),
            data["response"].clone(),
        ]);
        let out = f.chat(data["request"].clone());
        assert_eq!(out.status, 200);
        let out: Value = serde_json::from_slice(&out.body).unwrap();
        assert!(out["choices"][0]["message"]["tool_calls"].is_null());
        assert!(out["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .contains("line_id_1"));
    }

    #[test]
    fn mid_score_fills_directly_and_decline_restores_normal_tools() {
        let data: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/dev-route-tune.json")).unwrap();
        let library = crate::recipe_library::Library::embedded().unwrap();
        let row = data
            .iter()
            .find(|r| {
                let found = library.form_band(r["text"].as_str().unwrap()).unwrap();
                let score = found["recipes"][0]["score"].as_f64().unwrap_or(0.0);
                matches!(
                    r["recipe"].as_str(),
                    Some("box" | "cylinder" | "profile" | "plate")
                ) && found["recipes"][0]["id"] == r["recipe"]
                    && (4.5..8.0).contains(&score)
            })
            .unwrap();
        for decline in [false, true] {
            let mut req = fill_request();
            req["messages"] = json!([{"role":"user","content":row["text"]}]);
            let replies = if decline {
                vec![
                    call("pilot_fill", json!({"decline":true})),
                    reply("Use ordinary tools."),
                ]
            } else {
                vec![call("pilot_fill", row["args"].clone())]
            };
            let mut f = fill_fixture(replies);
            assert_eq!(f.chat(req.clone()).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), if decline { 2 } else { 1 });
            assert_eq!(script.requests[0]["tools"].as_array().unwrap().len(), 1);
            assert_eq!(
                script.requests[0]["tool_choice"]["function"]["name"],
                "pilot_fill"
            );
            if decline {
                assert_eq!(script.requests[1]["tools"], req["tools"]);
                assert!(!script.requests[1]["tools"]
                    .to_string()
                    .contains("pilot_route"));
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(!log.contains("route_offered"));
            assert!(log.contains(if decline {
                "fill_declined"
            } else {
                "fill_accepted"
            }));
        }
    }

    #[test]
    fn route_pick_valid_fill_emits_client_calls() {
        let data: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/dev-route-tune.json")).unwrap();
        let library = crate::recipe_library::Library::embedded().unwrap();
        let row = data
            .iter()
            .find(|r| {
                r["recipe"] == "box"
                    && library.form_band(r["text"].as_str().unwrap()).unwrap()["band"] == "route"
            })
            .unwrap();
        let mut req = fill_request();
        req["messages"] = json!([{"role":"user","content":row["text"]}]);
        let mut f = fill_fixture(vec![
            call("pilot_route", json!({"recipe":"box"})),
            call("pilot_fill", row["args"].clone()),
        ]);
        let out = chat_forms(&mut f, req);
        assert_eq!(out.status, 200);
        let response: Value = serde_json::from_slice(&out.body).unwrap();
        assert!(response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .is_some_and(|c| !c.is_empty()));
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        assert_eq!(
            script.requests[1]["tools"][0]["function"]["name"],
            "pilot_fill"
        );
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        assert!(log.contains("route_picked") && log.contains("fill_accepted"));
    }

    #[test]
    fn route_pick_then_fill_and_four_call_cap() {
        for streaming in [false, true] {
            let mut replies = vec![call("pilot_route", json!({"recipe":"revolute_joint"}))];
            // Joint forms require real component evidence, so invalid fields exercise
            // all three validation retries without executing a fabricated joint.
            replies.extend((0..3).map(|_| call("pilot_fill", json!({"bogus":1}))));
            replies.push(reply("Which parts should be joined?"));
            let mut f = fill_fixture(replies);
            let mut req = route_request();
            if streaming {
                req["stream"] = json!(true);
            }
            let out = chat_forms(&mut f, req);
            assert_eq!(out.status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 5);
            assert_eq!(script.requests[0]["tools"].as_array().unwrap().len(), 1);
            let tool = &script.requests[0]["tools"][0]["function"];
            assert_eq!(tool["name"], "pilot_route");
            assert_eq!(tool["parameters"]["required"], json!(["recipe"]));
            assert_eq!(tool["parameters"]["additionalProperties"], false);
            let ids = tool["parameters"]["properties"]["recipe"]["enum"]
                .as_array()
                .unwrap();
            assert_eq!(ids.len(), 5);
            assert_eq!(ids[4], "none");
            assert!(ids.contains(&json!("revolute_joint")));
            assert_eq!(
                script.requests[0]["tool_choice"]["function"]["name"],
                "pilot_route"
            );
            assert!(script.requests[0]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .starts_with("You select one form"));
            for wire in &script.requests[1..4] {
                assert_eq!(wire["tools"][0]["function"]["name"], "pilot_fill");
            }
            assert!(!script.requests[4]["tools"]
                .to_string()
                .contains("pilot_route"));
            assert!(!script.requests[4]["tools"]
                .to_string()
                .contains("pilot_fill"));
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(
                log.contains("route_offered")
                    && log.contains("route_picked")
                    && log.contains("fill_offered")
            );
        }
    }

    #[test]
    fn route_provider_retry_consumes_form_cap() {
        let mut f = fill_fixture(vec![
            json!({"error":"temporary failure"}),
            call("pilot_route", json!({"recipe":"revolute_joint"})),
            call("pilot_fill", json!({"bogus":1})),
            call("pilot_fill", json!({"bogus":1})),
            reply("Which parts?"),
        ]);
        mark_errors(&f);
        assert_eq!(chat_forms(&mut f, route_request()).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 5);
        assert_eq!(
            script.requests[0]["tools"][0]["function"]["name"],
            "pilot_route"
        );
        assert_eq!(
            script.requests[1]["tools"][0]["function"]["name"],
            "pilot_route"
        );
        for wire in &script.requests[2..4] {
            assert_eq!(wire["tools"][0]["function"]["name"], "pilot_fill");
        }
        assert!(!script.requests[4]["tools"]
            .to_string()
            .contains("pilot_fill"));
        assert!(!script.requests[4]["tools"]
            .to_string()
            .contains("pilot_route"));
    }

    #[test]
    fn route_none_or_invalid_restores_normal_tools() {
        for selection in [
            call("pilot_route", json!({"recipe":"none"})),
            call("pilot_route", json!({"recipe":"invented"})),
            call(
                "pilot_route",
                json!({"recipe":"revolute_joint","extra":true}),
            ),
        ] {
            let mut f = fill_fixture(vec![selection, reply("Which parts?")]);
            assert_eq!(chat_forms(&mut f, route_request()).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 2);
            assert_eq!(
                script.requests[0]["tools"][0]["function"]["name"],
                "pilot_route"
            );
            assert!(!script.requests[1]["tools"]
                .to_string()
                .contains("pilot_route"));
            assert!(!script.requests[1]["tools"]
                .to_string()
                .contains("pilot_fill"));
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("route_none"));
        }
    }

    #[test]
    #[ignore = "live development routing only: at most six Ollama requests, no CAD execution"]
    fn live_gemma_route_tune() {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout(Duration::from_secs(45))
            .build();
        let library = crate::recipe_library::Library::embedded().unwrap();
        let data: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/dev-route-tune.json")).unwrap();
        let mut sent = 0;
        for row in data {
            let band = library.form_band(row["text"].as_str().unwrap()).unwrap();
            if band["band"] != "route" {
                continue;
            }
            let mut req = json!({"model":"gemma4:latest","messages":[{"role":"user","content":row["text"]}],"stream":false,"temperature":0,"max_tokens":768});
            let candidates = band["recipes"].as_array().unwrap();
            fill::offer_route(&mut req, candidates).unwrap();
            sent += 1;
            match agent
                .post("http://127.0.0.1:11435/v1/chat/completions")
                .set("Content-Type", "application/json")
                .send_string(&req.to_string())
            {
                Ok(r) => {
                    let mut bytes = Vec::new();
                    r.into_reader()
                        .take(1048577)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(bytes.len() <= 1048576);
                    let response: Value = serde_json::from_slice(&bytes).unwrap();
                    let pick = fill::picked(&response, candidates);
                    eprintln!(
                        "LIVE {sent} text={} expected={} picked={} response={}",
                        row["text"],
                        row["recipe"],
                        pick.map(|r| r["id"].clone()).unwrap_or(json!("none")),
                        response["choices"][0]["message"]
                    );
                }
                Err(e) => {
                    eprintln!("LIVE unavailable: {e}");
                    break;
                }
            }
            if sent >= 6 {
                break;
            }
        }
    }
    #[test]
    fn every_fill_coercion_is_audited_before_validation() {
        let mut f = fill_fixture(vec![call(
            "pilot_fill",
            json!({"kind":"l","plane":"xz","width":"47 mm","height":"33","thickness":"5","depth":"22","decline":"no"}),
        )]);
        let out = f.chat(fill_request());
        assert_eq!(out.status, 200);
        let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
        let events: Vec<Value> = log
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            events
                .iter()
                .filter(|e| e["kind"] == "fill_coerced")
                .count(),
            7
        );
        assert!(events.iter().any(|e| e["kind"] == "fill_accepted"));
        assert!(!events.iter().any(|e| e["kind"] == "fill_invalid"));
    }
    #[test]
    fn fill_corner_defaults_are_audited_and_plate_can_recover_on_third_attempt() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/fill-audit4.json")).unwrap();
        let mut corrected = cases[2]["args"].clone();
        corrected["corner_edges"] = json!("vertical");
        for args in [
            vec![cases[1]["args"].clone()],
            vec![json!({}), cases[2]["args"].clone(), corrected],
        ] {
            let fill_count = args.len();
            let req: Value = serde_json::from_str(include_str!(
                "../tests/fixtures/fill-v2-01-bolt-circle-bracket.json"
            ))
            .unwrap();
            let library = crate::recipe_library::Library::embedded().unwrap();
            let routed = library.form_band(last_user(&req)).unwrap()["band"] == "route";
            let mut replies = Vec::new();
            if routed {
                replies.push(call("pilot_route", json!({"recipe":"plate"})));
            }
            replies.extend(args.into_iter().map(|a| call("pilot_fill", a)));
            let count = replies.len();
            let mut f = fill_fixture(replies);
            let out = f.chat(req);
            assert_eq!(out.status, 200);
            assert_eq!(
                f.proxy.scripted.as_ref().unwrap().borrow().requests.len(),
                count
            );
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("fill_accepted"));
            if fill_count == 1 {
                assert_eq!(log.matches("\"kind\":\"fill_defaulted\"").count(), 2);
            } else {
                assert!(log.contains("four corner edges through the thickness"));
            }
        }
    }
    #[test]
    fn clean_fill_excludes_older_history_on_every_attempt() {
        let mut f = fill_fixture(vec![call("pilot_fill", json!({"kind":"L"})), filled()]);
        f.proxy.cfg.trim_keep = 0;
        let mut req = fill_request();
        let old =
            json!({"role":"tool","tool_call_id":"old","content":"original geometry ".repeat(2000)});
        req["messages"].as_array_mut().unwrap().splice(0..0, [
            json!({"role":"user","content":"previous task"}),
            json!({"role":"assistant","tool_calls":[{"id":"old","function":{"name":"inspect_design","arguments":"{}"}}]}),
            old.clone(),
        ]);
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 2);
        for wire in &script.requests {
            assert_eq!(wire["messages"].as_array().unwrap().len(), 2);
            assert!(!wire.to_string().contains("original geometry"));
        }
    }

    #[test]
    fn forced_fill_accepts_real_calls_and_retries_one_field_error() {
        for invalid_count in 0..=2 {
            let mut replies = Vec::new();
            for _ in 0..invalid_count {
                replies.push(call("pilot_fill", json!({"kind":"L"})));
            }
            replies.push(filled());
            let mut f = fill_fixture(replies);
            let out = f.chat(fill_request());
            assert_eq!(out.status, 200);
            let body: Value = serde_json::from_slice(&out.body).unwrap();
            assert!(!body.to_string().contains("pilot_fill"));
            assert!(body["choices"][0]["message"]["tool_calls"].is_array());
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 1 + invalid_count);
            for wire in &script.requests {
                assert_eq!(wire["tools"].as_array().unwrap().len(), 1);
                assert_eq!(wire["tools"][0]["function"]["name"], "pilot_fill");
                assert_eq!(
                    wire["tool_choice"],
                    json!({"type":"function","function":{"name":"pilot_fill"}})
                );
                let line = wire["messages"][0]["content"].as_str().unwrap();
                assert!(
                    line.contains("You fill in one form")
                        && line.contains("inches (25.4)")
                        && line.contains("Leave optional fields out")
                );
            }
            if invalid_count > 0 {
                assert!(script.requests[1]["messages"]
                    .to_string()
                    .contains("field error"));
            }
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert!(log.contains("fill_offered") && log.contains("fill_accepted"));
        }
    }
    #[test]
    fn forced_fill_falls_back_on_decline_text_other_tool_and_third_invalid() {
        for rejected in [
            vec![call("pilot_fill", json!({"decline":true}))],
            vec![reply("I need more details."), reply("I need more details.")],
            vec![
                call("execute", json!({"command":"solid.box","params":{}})),
                call("execute", json!({"command":"solid.box","params":{}})),
            ],
            vec![
                call("pilot_fill", json!({})),
                call("pilot_fill", json!({})),
                call("pilot_fill", json!({})),
            ],
        ] {
            let count = rejected.len();
            let mut replies = rejected;
            replies.push(reply("What dimensions?"));
            let mut f = fill_fixture(replies);
            let out = f.chat(fill_request());
            assert_eq!(out.status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), count + 1);
            let normal = &script.requests[count];
            assert!(!normal["tools"].to_string().contains("pilot_find"));
            assert!(!normal["tools"].to_string().contains("pilot_build"));
            assert!(!normal.to_string().contains("pilot_fill"));
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            assert_eq!(log.matches("\"kind\":\"fill_offered\"").count(), 1);
            assert!(log.contains("fill_fallback"));
        }
    }
    #[test]
    fn forced_fill_turn_dialect_flags_and_exact_guards() {
        for mode in [
            "continuation",
            "no-fill",
            "no-shortcuts",
            "sc98",
            "exact",
            "no-native-tools",
        ] {
            let mut answer = reply("Ready.");
            answer["choices"][0]["message"]["tool_calls"] = json!([]);
            let mut f = fill_fixture(vec![answer]);
            let mut req = fill_request();
            match mode {
                "continuation" => req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"assistant","content":"Earlier answer"})),
                "no-fill" => f.proxy.cfg.no_fill = true,
                "no-shortcuts" => f.proxy.cfg.no_shortcuts = true,
                "sc98" => f.proxy.cfg.solidcraft98 = true,
                "no-native-tools" => req["tools"] = json!([]),
                "exact" => {
                    f.proxy.cfg.direct_recipes = true;
                    req["messages"][0]["content"] = json!("make a 20 mm cube");
                }
                _ => panic!("test mode"),
            }
            let out = f.chat(req);
            assert_eq!(
                out.status,
                200,
                "{mode}: {}",
                String::from_utf8_lossy(&out.body)
            );
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert!(script
                .requests
                .iter()
                .all(|r| !r["tools"].to_string().contains("pilot_fill")));
            if mode == "no-fill" {
                assert!(!script.requests[0]["tools"]
                    .to_string()
                    .contains("pilot_find"));
            }
        }
    }
    #[test]
    fn fill_is_offered_once_and_normal_fact_respects_flags() {
        let mut f = fill_fixture(vec![
            reply("no form"),
            reply("Ready."),
            reply("Ready again."),
            reply("Ready after resend."),
        ]);
        let req = fill_request();
        assert_eq!(f.chat(req.clone()).status, 200);
        assert_eq!(f.chat(req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 4);
        assert_eq!(
            script
                .requests
                .iter()
                .filter(|r| r["tools"][0]["function"]["name"] == "pilot_fill")
                .count(),
            2
        );
        assert!(!script.requests[2]["messages"]
            .to_string()
            .contains("call pilot_find first."));
        drop(script);
        let mut disabled = fill_fixture(vec![reply("Ready.")]);
        disabled.proxy.cfg.no_shortcuts = true;
        disabled.chat(fill_request());
        assert!(
            !disabled.proxy.scripted.as_ref().unwrap().borrow().requests[0]["messages"]
                .to_string()
                .contains("call pilot_find first.")
        );
    }
    #[test]
    fn public_probes_offer_the_right_fill_form() {
        for (text, id) in [
            (
                include_str!("../tests/fixtures/fill-constraints-L-1.json"),
                "profile",
            ),
            (
                include_str!("../tests/fixtures/fill-v2-01-bolt-circle-bracket.json"),
                "plate",
            ),
            (
                include_str!("../tests/fixtures/fill-v2-03-shell-lip.json"),
                "enclosure",
            ),
        ] {
            let req: Value = serde_json::from_str(text).unwrap();
            let library = crate::recipe_library::Library::embedded().unwrap();
            let routed = library.form_band(last_user(&req)).unwrap()["band"] == "route";
            let mut replies = Vec::new();
            if routed {
                replies.push(call("pilot_route", json!({"recipe":id})));
            }
            replies.extend([call("pilot_fill", json!({"decline":true})), reply("Ready.")]);
            let mut f = fill_fixture(replies);
            assert_eq!(f.chat(req).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests[0]["tools"].as_array().unwrap().len(), 1);
            assert!(
                script.requests[usize::from(routed)]["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .starts_with("You fill in one form")
            );
            assert!(
                script.requests[0]["tools"][0]["function"]["parameters"]["properties"].is_object()
            );
        }
    }
    #[test]
    #[ignore = "live clean extraction: exactly five public L prompts, no retries or CAD"]
    fn live_gemma_clean_fill_public_l() {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout(Duration::from_secs(45))
            .build();
        let library = crate::recipe_library::Library::embedded().unwrap();
        let tasks: Value =
            serde_json::from_str(include_str!("../bench/sets/constraints/tasks.json")).unwrap();
        let mut valid = 0;
        for task in tasks["tasks"].as_array().unwrap().iter().take(5) {
            let mut req = json!({"model":"gemma4:latest","temperature":0,"max_tokens":768,
                "messages":[{"role":"user","content":task["prompt"]}]});
            let found = fill::select(&library, &req).unwrap().unwrap();
            fill::offer(&mut req, &found, None).unwrap();
            let result = agent
                .post("http://127.0.0.1:11435/v1/chat/completions")
                .set("Content-Type", "application/json")
                .send_string(&req.to_string());
            match result {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    response
                        .into_reader()
                        .take(1048577)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(bytes.len() <= 1048576);
                    let response: Value = serde_json::from_slice(&bytes).unwrap();
                    let accepted = matches!(
                        fill::accept(
                            &response,
                            &found,
                            &req,
                            &library,
                            &Catalog::bundled().unwrap()
                        ),
                        fill::Outcome::Build(_, _)
                    );
                    valid += usize::from(accepted);
                    eprintln!(
                        "{} valid={accepted}: {}",
                        task["id"], response["choices"][0]["message"]
                    );
                }
                Err(error) => eprintln!("{} unavailable: {error}", task["id"]),
            }
        }
        eprintln!("Clean public L fills: {valid}/5 called pilot_fill with valid arguments");
    }

    #[test]
    fn clean_route_and_fill_each_retry_once_within_shared_cap() {
        let data: Vec<Value> =
            serde_json::from_str(include_str!("../tests/fixtures/dev-route-tune.json")).unwrap();
        let library = crate::recipe_library::Library::embedded().unwrap();
        let row = data
            .iter()
            .find(|r| {
                r["recipe"] == "box"
                    && library.form_band(r["text"].as_str().unwrap()).unwrap()["band"] == "route"
            })
            .unwrap();
        let mut req = fill_request();
        req["messages"] = json!([{"role":"user","content":row["text"]}]);
        let mut f = fill_fixture(vec![
            call("batch", json!({})),
            call("pilot_route", json!({"recipe":"box"})),
            reply("I can build it."),
            call("pilot_fill", row["args"].clone()),
        ]);
        assert_eq!(chat_forms(&mut f, req).status, 200);
        let script = f.proxy.scripted.as_ref().unwrap().borrow();
        assert_eq!(script.requests.len(), 4);
        for (i, tool) in ["pilot_route", "pilot_route", "pilot_fill", "pilot_fill"]
            .iter()
            .enumerate()
        {
            assert_eq!(script.requests[i]["tools"][0]["function"]["name"], *tool);
            assert_eq!(script.requests[i]["messages"].as_array().unwrap().len(), 2);
        }
        assert!(script.requests[1]["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("unoffered tool(s) batch"));
        assert!(script.requests[3]["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Reply contained text"));
        assert!(!script.requests[2]["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Previous reply problem"));
    }

    #[test]
    fn clean_form_sizes_count_every_retry_and_fallback() {
        for route in [false, true] {
            let mut req = if route {
                route_request()
            } else {
                fill_request()
            };
            req["temperature"] = json!(0.67);
            req["seed"] = json!(123);
            req["messages"].as_array_mut().unwrap().insert(
                0,
                json!({"role":"system","content":"call batch; ".repeat(1000)}),
            );
            let mut f = fill_fixture(vec![reply("text"), reply("text again"), reply("Clarify.")]);
            f.proxy.cfg.seed = Some(999);
            assert_eq!(chat_forms(&mut f, req.clone()).status, 200);
            let script = f.proxy.scripted.as_ref().unwrap().borrow();
            assert_eq!(script.requests.len(), 3);
            for wire in &script.requests[..2] {
                assert_eq!(wire["temperature"], req["temperature"]);
                assert_eq!(wire["seed"], req["seed"]);
                assert_eq!(wire["messages"].as_array().unwrap().len(), 2);
                assert!(!wire.to_string().contains("call batch"));
            }
            assert_eq!(script.requests[2]["tools"], req["tools"]);
            let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
            let entries: Vec<Value> = log
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .collect();
            let sizes: Vec<_> = entries
                .iter()
                .filter(|e| e["kind"] == "request_size")
                .collect();
            assert_eq!(sizes.len(), 3);
            for (entry, wire) in sizes.iter().zip(&script.requests) {
                assert_eq!(
                    entry["after"]["upstream"]["total"],
                    history::chars(wire) as u64
                );
            }
            let forms: Vec<_> = entries
                .iter()
                .filter(|e| e["kind"] == "form_request")
                .collect();
            assert_eq!(forms.len(), 2);
            for (entry, wire) in forms.iter().zip(&script.requests) {
                assert_eq!(entry["after"]["bytes"], json_bytes(wire).unwrap().len());
            }
        }
    }

    #[test]
    #[ignore = "optional live Ollama probe: at most three requests, no CAD execution"]
    fn live_gemma_fill_public_probes() {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout(Duration::from_secs(30))
            .build();
        let library = crate::recipe_library::Library::embedded().unwrap();
        for text in [
            include_str!("../tests/fixtures/fill-constraints-L-1.json"),
            include_str!("../tests/fixtures/fill-v2-01-bolt-circle-bracket.json"),
            include_str!("../tests/fixtures/fill-v2-03-shell-lip.json"),
        ] {
            let mut req: Value = serde_json::from_str(text).unwrap();
            // This legacy probe measures filling only; routing is measured separately.
            let found = library
                .find(last_user(&req), &crate::recipe_library::Bm25)
                .unwrap()["recipes"][0]
                .clone();
            req["model"] = json!("gemma4:latest");
            req["stream"] = json!(false);
            req["max_tokens"] = json!(768);
            req["temperature"] = json!(0);
            fill::offer(&mut req, &found, None).unwrap();
            match agent
                .post("http://127.0.0.1:11435/v1/chat/completions")
                .set("Content-Type", "application/json")
                .send_string(&req.to_string())
            {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    response
                        .into_reader()
                        .take(1048577)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(bytes.len() <= 1048576);
                    let response: Value = serde_json::from_slice(&bytes).unwrap();
                    let outcome = match fill::accept(
                        &response,
                        &found,
                        &req,
                        &library,
                        &Catalog::bundled().unwrap(),
                    ) {
                        fill::Outcome::Build(_, _) => "accepted".into(),
                        fill::Outcome::Decline => "declined".into(),
                        fill::Outcome::Fallback => "fallback".into(),
                        fill::Outcome::Invalid(e) => format!("invalid: {e}"),
                    };
                    eprintln!(
                        "gemma4:latest {}: {outcome}; {}",
                        found["id"], response["choices"][0]["message"]
                    );
                }
                Err(e) => {
                    eprintln!("gemma4:latest {}: unavailable: {e}", found["id"]);
                    break;
                }
            }
        }
    }
}
