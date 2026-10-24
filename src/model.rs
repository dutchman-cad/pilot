//! Blocking OpenAI-compatible proxy. Only explicit JSON calls become actions.
use crate::{
    audit::{now, Audit},
    catalog::Catalog,
    guidance, planner, prompting, repair,
};
use prompting::FALLBACK;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
mod recovery;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPrompt {
    Auto,
    On,
    Off,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Explicit SolidCraft 98 dialect: no SolidCraft catalogue, planner or local how.
    pub solidcraft98: bool,
    pub listen: SocketAddr,
    pub upstream: String,
    pub upstream_timeout: Duration,
    pub fallback_prompt: FallbackPrompt,
    pub no_how: bool,
    pub no_hints: bool,
    pub planner: bool,
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
    pub native: u64,
    pub text_written: u64,
    pub prose: u64,
    pub empty: u64,
    pub repairs: u64,
    pub hand_backs: u64,
    pub repairs_by_kind: BTreeMap<String, u64>,
    pub provider_errors: u64,
    pub ollama_500_recoveries: u64,
    pub plans_used: u64,
    pub how_answers: u64,
    pub hints_added: u64,
    pub capabilities_checked: bool,
    pub native_support: Option<bool>,
    pub ollama: bool,
    /// One entry per client turn, oldest first. Old profiles start with no evidence.
    pub recent_text_calls: Vec<bool>,
}
type Profiles = BTreeMap<String, Profile>;

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
    if profiles
        .values()
        .any(|p| p.recent_text_calls.len() > 5 || p.repairs_by_kind.len() > 256)
    {
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
    agent: ureq::Agent,
    catalog: Catalog,
    profiles: Profiles,
    profile_path: PathBuf,
    settings: BTreeMap<String, ModelSettings>,
    log: File,
    conversations: Vec<Conversation>,
    #[cfg(test)]
    scripted: Option<std::cell::RefCell<Scripted>>,
}
#[cfg(test)]
#[derive(Default)]
struct Scripted {
    replies: std::collections::VecDeque<Upstream>,
    requests: Vec<Value>,
}
impl Server {
    pub fn bind(mut cfg: Config) -> Result<Self, String> {
        if cfg.solidcraft98 {
            cfg.planner = false;
            cfg.no_how = true;
            cfg.no_hints = true;
            cfg.prompt_help = false;
        }
        if cfg.upstream_timeout < Duration::from_secs(1)
            || cfg.upstream_timeout > MAX_UPSTREAM_TIMEOUT
        {
            return Err("upstream-timeout must be between 1 and 3600 seconds".into());
        }
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
        let (mut socket, _) = self
            .listener
            .accept()
            .map_err(|e| format!("accept failed: {e}"))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("cannot set timeout: {e}"))?;
        let response = match read_request(&mut socket) {
            Ok(request) => self.proxy.handle(request),
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
    fn handle(&mut self, request: Request) -> Result<Reply, String> {
        // Two shared hand-backs, one parse-error retry, and the capability probe.
        let deadline = Instant::now() + self.cfg.upstream_timeout * 4 + Duration::from_secs(3);
        if request.path == "/v1/models" && request.method == "GET" {
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
        if !self.profiles.contains_key(&key) && self.profiles.len() >= MAX_PROFILES {
            return Err("profile limit of 256 models reached".into());
        }
        let mut profile = self.profiles.remove(&key).unwrap_or_else(|| Profile {
            upstream: self.cfg.upstream.clone(),
            model: model.clone(),
            ..Profile::default()
        });
        let mut audit = Audit {
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
        profile.turns = profile.turns.saturating_add(1);
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
        profile
            .recent_text_calls
            .push(profile.text_written > text_before);
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
        let original = body.clone();
        if self.cfg.solidcraft98 {
            set_logged(
                &mut body,
                "tools",
                json!(crate::solidcraft98::drive::chat_tools()),
                "solidcraft98_tools",
                audit,
            )?;
        }
        if self.cfg.planner {
            if let Some(plan) = planner::plan(&body, &self.catalog)? {
                let message = plan.message();
                body["messages"]
                    .as_array_mut()
                    .ok_or("messages must be an array")?
                    .push(json!({"role":"system","content":message}));
                audit.record(
                    "plan",
                    &Value::Null,
                    &json!({"rule":plan.rule,"calls":plan.calls,"prompt":message}),
                )?;
            }
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
        if (!tools.is_empty() && self.cfg.fallback_prompt == FallbackPrompt::Auto)
            || settings.is_some()
        {
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
            append_hints(&mut body, &self.catalog, audit)?;
        }
        let mut empty_retried = false;
        let mut parse_retried = false;
        let mut hand_backs = 0;
        let mut guard_backs = 0;
        let mut how_answers = 0;
        let mut last_reason = String::new();
        for attempt in 0..=3 {
            let wire = if attempt == 0 && body == original {
                request.body.clone()
            } else {
                json_bytes(&body)?
            };
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
            if !(200..300).contains(&upstream.status) {
                let reply = provider_error(&upstream);
                audit.record(
                    "provider_error",
                    &json!({"status":upstream.status}),
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
            let before = response.clone();
            let choices = response["choices"]
                .as_array_mut()
                .filter(|c| !c.is_empty() && c.len() <= 8)
                .ok_or("upstream choices must contain 1 to 8 entries")?;
            let mut nudge = None;
            for choice in choices.iter_mut() {
                let message = choice
                    .get_mut("message")
                    .filter(|m| m.is_object())
                    .ok_or("upstream choice is missing an assistant message")?;
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
                        convert_text(message, &tools, &self.catalog, audit)?
                    } {
                        profile.text_written = profile.text_written.saturating_add(1);
                    } else if is_empty(message) {
                        profile.empty = profile.empty.saturating_add(1);
                    } else {
                        profile.prose = profile.prose.saturating_add(1);
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
                if !accepted
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
    if value.get("error").is_some()
        || value["isError"] == true
        || value["result"]["isError"] == true
    {
        return None;
    }
    Some(guidance::payload(&value))
}

fn append_hints(body: &mut Value, catalog: &Catalog, audit: &mut Audit) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let start = messages
        .iter()
        .rposition(|m| m["role"] != "tool")
        .map_or(0, |i| i + 1);
    let mut previous = None;
    for i in 0..messages.len() {
        if messages[i]["role"] != "tool" {
            continue;
        }
        let id = messages[i]["tool_call_id"].as_str();
        let name = id
            .and_then(|id| {
                messages[..i]
                    .iter()
                    .rev()
                    .filter_map(|m| m["tool_calls"].as_array())
                    .flat_map(|calls| calls.iter())
                    .find(|call| call["id"].as_str() == Some(id))
                    .and_then(|call| call["function"]["name"].as_str())
            })
            .or_else(|| messages[i]["name"].as_str())
            .unwrap_or("");
        let change = matches!(name, "execute" | "batch");
        if !name.is_empty() && !change && !matches!(name, "inspect" | "inspect_design") {
            continue;
        }
        let Some(data) = tool_data(&messages[i]) else {
            continue;
        };
        let after = data.get("after").unwrap_or(&data);
        let before = data.get("before").or(previous.as_ref());
        let hint = (i >= start)
            .then(|| guidance::state_hint(before, after, change, Some(catalog)))
            .flatten();
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
                hint.split_whitespace().collect::<Vec<_>>().join(" ")
            );
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
        // An unspecified or union type cannot justify moving an unclaimed argument.
        _ => false,
    }
}
fn key_words(key: &str) -> Vec<String> {
    let mut words: Vec<_> = key
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    words.sort();
    words
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
        let matches: Vec<_> = properties
            .keys()
            .filter(|k| {
                !object.contains_key(*k)
                    && (normalize(k) == normalize(&extra) || key_words(k) == key_words(&extra))
            })
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
    let extras: Vec<_> = object
        .keys()
        .filter(|k| !properties.contains_key(*k))
        .cloned()
        .collect();
    if missing.len() == 1
        && extras.len() == 1
        && schema_type_matches(&object[&extras[0]], &properties[missing[0]])
    {
        if let Some(value) = object.remove(&extras[0]) {
            object.insert(missing[0].to_string(), value);
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
                    cfg: Config {
                        solidcraft98: false,
                        listen: "127.0.0.1:0".parse().unwrap(),
                        upstream: "http://localhost:11434/v1".into(),
                        upstream_timeout: DEFAULT_UPSTREAM_TIMEOUT,
                        fallback_prompt: FallbackPrompt::Off,
                        no_how: true,
                        no_hints: false,
                        planner: true,
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
                    scripted: Some(RefCell::new(Scripted {
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
            let mut f = Fixture::new(vec![reply("Ready.")]);
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
    fn command_and_parameter_reconciliation_is_conservative() {
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
        assert!(reconcile(&mut args, &schema).is_none());
        assert_eq!(args["radius"], 3);
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
                script.requests[0]["messages"][0]["content"] == FALLBACK,
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
}
