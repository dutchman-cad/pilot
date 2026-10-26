//! MCP tools face. Every dispatch is an explicit typed call.
use super::guidance::help;
use super::{
    protocol,
    screen::Screen,
    transport::{Endpoint, RemoteSession},
};
use crate::audit::Audit;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, BufRead, Read, Write},
    path::PathBuf,
};
#[derive(Debug)]
pub struct Config {
    pub endpoint: Endpoint,
    pub log: Option<PathBuf>,
}
pub const DIALOG_FACT: &str = "SolidCraft 98 dialogs: fill every field, then OK; typing affects only the focused field, so select each intended field first (fill_dialog fills named fields and presses OK, preserving other displayed defaults).";

pub fn tools() -> Vec<Value> {
    let string = json!({"type":"string","maxLength":256,"minLength":1});
    let integer = json!({"type":"integer","minimum":0,"maximum":16383});
    let point = json!({"type":"object","properties":{"x":integer,"y":integer},"required":["x","y"],"additionalProperties":false});
    [
        ("fill_dialog",json!({"fields":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","properties":{"name":string,"value":{"type":"string","maxLength":256}},"required":["name","value"],"additionalProperties":false}}}),vec!["fields"]),
        ("look",json!({}),vec![]), ("yes",json!({}),vec![]),
        ("click",json!({"name":string,"point":point}),vec![]),
        ("doubleclick",json!({"name":string,"point":point}),vec![]),
        ("rightclick",json!({"name":string,"point":point}),vec![]),
        ("drag",json!({"x1":integer,"y1":integer,"x2":integer,"y2":integer,"button":{"enum":["left","right"]}}),vec!["x1","y1","x2","y2","button"]),
        ("menu",json!({"title":string,"item":string}),vec!["title"]),
        ("key",json!({"keys":string}),vec!["keys"]),
        ("type",json!({"text":{"type":"string","maxLength":4000}}),vec!["text"]),
        ("wait",json!({"ms":{"type":"integer","minimum":0,"maximum":5000}}),vec!["ms"]),
        ("how",json!({"task":string}),vec!["task"]),
    ].into_iter().map(|(name,properties,required)| {
        let mut schema=json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
        if matches!(name,"click"|"doubleclick"|"rightclick") { schema["oneOf"]=json!([{"required":["name"]},{"required":["point"]}]); }
        json!({"name":name,"description":format!("SolidCraft 98 {name}. Explicit actions only; type preserves text (application trims outer whitespace). yes requires a current application offer."),"inputSchema":schema})
    }).collect()
}
pub(crate) fn validate(value: &Value, schema: &Value) -> Result<(), String> {
    if let Some(options) = schema["enum"].as_array() {
        if !options.contains(value) {
            return Err("value outside allowed enum".into());
        }
    }
    match schema["type"].as_str() {
        Some("object") => {
            let obj = value.as_object().ok_or("arguments must be an object")?;
            let properties = schema["properties"]
                .as_object()
                .ok_or("invalid tool schema")?;
            if let Some(required) = schema["required"].as_array() {
                for key in required {
                    if !obj.contains_key(key.as_str().ok_or("invalid required key")?) {
                        return Err(format!("missing required argument {key}"));
                    }
                }
            }
            for (key, v) in obj {
                validate(
                    v,
                    properties
                        .get(key)
                        .ok_or_else(|| format!("unknown argument {key}"))?,
                )?;
            }
            if schema.get("oneOf").is_some() && obj.len() != 1 {
                return Err("provide exactly one of name or point".into());
            }
        }
        Some("array") => {
            let items = value.as_array().ok_or("argument must be an array")?;
            if items.len() < schema["minItems"].as_u64().unwrap_or(0) as usize
                || items.len() > schema["maxItems"].as_u64().unwrap_or(16) as usize
            {
                return Err("array count outside tool limits".into());
            }
            for item in items {
                validate(item, &schema["items"])?;
            }
        }
        Some("string") => {
            let s = value.as_str().ok_or("argument must be a string")?;
            if s.len() > schema["maxLength"].as_u64().unwrap_or(4096) as usize
                || s.len() < schema["minLength"].as_u64().unwrap_or(0) as usize
            {
                return Err("string length outside tool limits".into());
            }
            protocol::encode(s)?;
        }
        Some("integer") => {
            let n = value
                .as_u64()
                .ok_or("argument must be a nonnegative integer")?;
            if n > schema["maximum"].as_u64().unwrap_or(0) {
                return Err("integer exceeds tool limit".into());
            }
        }
        _ => {}
    }
    Ok(())
}
pub struct Server {
    remote: RemoteSession,
    plain: bool,
    action_limit: usize,
    last_text: String,
    screen: Option<Screen>,
    audit: Audit,
    observation: u64,
    actions: usize,
    offer: Option<String>,
    preferred_offer: Option<String>,
    previous: Option<(String, usize)>,
    session: String,
    transport: &'static str,
    audit_failed: bool,
}
impl Server {
    pub fn new(cfg: Config) -> Result<Self, String> {
        let path = match cfg.log {
            Some(p) => p,
            None => {
                let d = crate::model::data_dir(None)?;
                std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                d.join("rewrites.jsonl")
            }
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("cannot open rewrite log: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let transport = if matches!(cfg.endpoint, Endpoint::Tcp(_)) {
            "tcp"
        } else {
            "process"
        };
        Ok(Self {
            remote: RemoteSession::new(cfg.endpoint),
            plain: false,
            action_limit: 40,
            last_text: String::new(),
            screen: None,
            audit: Audit {
                events: None,
                file,
                size,
                model: "mcp98".into(),
                count: 0,
                kinds: BTreeMap::new(),
            },
            observation: 0,
            actions: 0,
            offer: None,
            preferred_offer: None,
            previous: None,
            session: format!("{}-{}", std::process::id(), crate::audit::now()),
            transport,
            audit_failed: false,
        })
    }
    /// Configure the same validated session for a driver; framing remains identical.
    pub(crate) fn driver_options(
        &mut self,
        plain: bool,
        budget: usize,
        model: &str,
    ) -> Result<(), String> {
        if !(1..=200).contains(&budget) {
            return Err("step budget must be 1–200".into());
        }
        self.plain = plain;
        self.action_limit = budget;
        self.remote.command_limit = (budget * 4 + 2).clamp(160, 800);
        self.audit.model = model.to_owned();
        Ok(())
    }
    pub(crate) fn driver_deadline(&mut self, deadline: std::time::Instant) {
        self.remote.deadline = deadline;
    }
    pub(crate) fn final_look(&mut self) -> Result<String, String> {
        self.look()
    }
    pub(crate) fn screen_text(&self) -> &str {
        &self.last_text
    }
    pub(crate) fn stable_state(&self) -> Value {
        json!({"focus":self.screen.as_ref().and_then(|s|s.focus.clone()),"menu":self.screen.as_ref().and_then(|s|s.menu.clone()),"dialog":self.screen.as_ref().and_then(|s|s.dialog.clone()),"timeline":self.screen.as_ref().map(|s|s.timeline.clone())})
    }
    pub(crate) fn repair_count(&self) -> u64 {
        self.audit
            .kinds
            .iter()
            .filter(|(k, _)| matches!(k.as_str(), "alias" | "syntax" | "name"))
            .map(|(_, v)| v)
            .sum()
    }
    pub(crate) fn rewrite_count(&self) -> u64 {
        self.audit
            .kinds
            .iter()
            .filter(|(k, _)| {
                matches!(
                    k.as_str(),
                    "alias"
                        | "syntax"
                        | "name"
                        | "prompt"
                        | "hint"
                        | "help"
                        | "hand_back"
                        | "text_call"
                        | "result_guidance"
                        | "error_guidance"
                )
            })
            .map(|(_, v)| v)
            .sum()
    }
    pub(crate) fn record(
        &mut self,
        kind: &str,
        before: &Value,
        after: &Value,
    ) -> Result<(), String> {
        let result = self.audit.record_context(kind,before,after,&json!({"session":self.session,"observation":self.observation,"transport":self.transport}));
        if result.is_err() {
            self.audit_failed = true;
        }
        result
    }
    fn send(&mut self, command: &str) -> Result<String, String> {
        match self.remote.exchange(command) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.screen = None;
                self.last_text.clear();
                self.offer = None;
                self.preferred_offer = None;
                // Recover observation only. The command that may have run is never resent.
                if self.remote.is_tcp() {
                    if let Ok(text) = self.remote.exchange("look") {
                        self.observation += 1;
                        if let Ok(screen) = Screen::parse(&text, self.observation) {
                            if screen.bounds.is_some() {
                                self.offer = screen.offer.clone();
                                self.last_text = text;
                                self.screen = Some(screen);
                            }
                        }
                    }
                }
                Err(e)
            }
        }
    }
    fn look(&mut self) -> Result<String, String> {
        self.screen = None;
        self.offer = None;
        let text = self.send("look")?;
        self.observation += 1;
        self.last_text = text.clone();
        let s = match Screen::parse(&text, self.observation) {
            Ok(s) => s,
            Err(e) => {
                self.preferred_offer = None;
                return Err(e);
            }
        };
        if s.bounds.is_none() {
            self.screen = None;
            self.preferred_offer = None;
            return Err("look did not report a complete SolidCraft 98 screen".into());
        }
        self.offer = self.preferred_offer.clone().or_else(|| s.offer.clone());
        self.screen = Some(s);
        Ok(text)
    }
    fn point(&self, v: &Value) -> Result<String, String> {
        let x = v["x"].as_u64().ok_or("missing x")?;
        let y = v["y"].as_u64().ok_or("missing y")?;
        let (w, h) = self
            .screen
            .as_ref()
            .and_then(|s| s.bounds)
            .ok_or("screen bounds unavailable")?;
        if x >= u64::from(w) || y >= u64::from(h) {
            return Err("point is outside current screen bounds".into());
        }
        Ok(format!("{x} {y}"))
    }
    fn command(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "look" => Ok("look".into()),
            "yes" => {
                if self.offer.is_none() {
                    return Err("no current application offer; yes cannot run".into());
                }
                Ok("yes".into())
            }
            "click" | "doubleclick" | "rightclick" => {
                let target = if let Some(n) = args["name"].as_str() {
                    let target = if self.plain {
                        n.to_owned()
                    } else {
                        self.screen
                            .as_ref()
                            .ok_or("screen unavailable")?
                            .resolve(n)?
                    };
                    if target != n {
                        self.record("name", &json!(n), &json!(target))?;
                    }
                    target
                } else {
                    self.point(&args["point"])?
                };
                Ok(format!("{name} {target}"))
            }
            "drag" => Ok(format!(
                "drag {} {}{}",
                self.point(&json!({"x":args["x1"],"y":args["y1"]}))?,
                self.point(&json!({"x":args["x2"],"y":args["y2"]}))?,
                if args["button"] == "right" {
                    " right"
                } else {
                    ""
                }
            )),
            "menu" => {
                if self.screen.as_ref().is_some_and(|s| s.dialog.is_some()) {
                    return Err("a dialog is open; close it before using menus".into());
                }
                let title = args["title"].as_str().ok_or("missing title")?;
                let item = args["item"].as_str();
                if title.contains('>') || item.is_some_and(|s| s.contains('>')) {
                    return Err("menu components cannot contain >".into());
                }
                Ok(format!(
                    "menu {title}{}",
                    item.map(|i| format!(" > {i}")).unwrap_or_default()
                ))
            }
            "key" => {
                let keys = args["keys"].as_str().ok_or("missing keys")?;
                let chords: Vec<_> = keys.split_whitespace().collect();
                if chords.is_empty() || chords.len() > 16 {
                    return Err("key requires 1–16 supported chords".into());
                }
                for chord in chords {
                    let parts: Vec<_> = chord.split('+').collect();
                    let key = parts.last().copied().unwrap_or("");
                    if parts[..parts.len().saturating_sub(1)]
                        .iter()
                        .any(|p| !matches!(*p, "ctrl" | "shift" | "alt"))
                        || !(key.len() == 1 && key.is_ascii()
                            || matches!(
                                key,
                                "enter"
                                    | "esc"
                                    | "tab"
                                    | "space"
                                    | "backspace"
                                    | "delete"
                                    | "up"
                                    | "down"
                                    | "left"
                                    | "right"
                                    | "home"
                                    | "end"
                                    | "f1"
                                    | "f2"
                                    | "f3"
                                    | "f4"
                                    | "f5"
                                    | "f6"
                                    | "f7"
                                    | "f8"
                                    | "f9"
                                    | "f10"
                                    | "f11"
                                    | "f12"
                            ))
                    {
                        return Err(format!("unsupported key chord {chord}"));
                    }
                }
                Ok(format!("key {keys}"))
            }
            "type" => Ok(format!(
                "type {}",
                args["text"].as_str().ok_or("missing text")?
            )),
            "wait" => Ok(format!("wait {}", args["ms"])),
            "how" => Ok(format!(
                "how {}",
                args["task"].as_str().ok_or("missing task")?
            )),
            _ => Err("unknown SolidCraft 98 tool; prose is never executed".into()),
        }
    }
    pub fn call(&mut self, name: &str, args: &Value) -> Value {
        let start = self.audit.count;
        let mut repaired = args.clone();
        let canonical = match name {
            _ if self.plain => name,
            "dblclick" | "double-click" | "double_click" => "doubleclick",
            "rclick" | "right-click" | "right_click" => "rightclick",
            _ => name,
        };
        let preparation = (|| {
            if self.audit_failed {
                return Err(
                    "audit failed; start a new session with a writable log before further actions"
                        .into(),
                );
            }
            if canonical != "look" && self.actions >= self.action_limit {
                return Err(format!(
                    "{} action turns exhausted; start an explicit new session",
                    self.action_limit
                ));
            }
            if canonical != "look" {
                self.actions += 1;
            }
            if canonical != name {
                self.record("alias", &json!(name), &json!(canonical))?;
            }
            if let Some(tool) = tools()
                .into_iter()
                .find(|t| !self.plain && t["name"] == canonical)
            {
                let changes = crate::repair::repair_arguments(
                    canonical,
                    &mut repaired,
                    &tool["inputSchema"],
                )?;
                for change in changes {
                    self.record("syntax", &change.before, &change.after)?;
                }
            }
            if !self.plain && canonical == "key" {
                if let Some(keys) = repaired["keys"].as_str() {
                    let normalized = keys.to_ascii_lowercase();
                    if normalized != keys {
                        self.record("syntax", &json!(keys), &json!(normalized))?;
                        repaired["keys"] = json!(normalized);
                    }
                }
            }
            self.record(
                "native",
                &json!({"name":name,"arguments":args}),
                &json!({"name":canonical,"arguments":repaired}),
            )?;
            Ok::<(), String>(())
        })();
        match preparation.and_then(|()| self.execute(canonical, &repaired, start)) {
            Ok(v) => v,
            Err(mut error) => {
                if !self.plain {
                    match self.recover_dialog() {
                        Ok(true) => {
                            error.push_str(" Unfinished dialog cancelled; edits were not applied.")
                        }
                        Ok(false) => {}
                        Err(e) => error.push_str(&format!(" Dialog recovery failed: {e}")),
                    }
                }
                let nearest =
                    if !self.plain && matches!(canonical, "click" | "doubleclick" | "rightclick") {
                        self.screen
                            .as_ref()
                            .map(|s| s.nearest(repaired["name"].as_str().unwrap_or("")))
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                let suggested: Vec<_> = nearest
                    .into_iter()
                    .map(|n| json!({"name":canonical,"arguments":{"name":n},"execute":false}))
                    .collect();
                let mut result = json!({"content":[{"type":"text","text":error}],"isError":true,"structuredContent":{"error":error,"screen":self.screen,"suggested":suggested,"rewrites":self.audit.count-start+1+usize::from(!self.plain && !suggested.is_empty())}});
                if !self.plain && !suggested.is_empty() {
                    if let Err(e) = self.record("error_guidance", &json!(error), &result) {
                        return json!({"isError":true,"content":[{"type":"text","text":format!("cannot audit suggestions: {e}")}]});
                    }
                }
                if self.plain {
                    if let Some(obj) = result["structuredContent"].as_object_mut() {
                        obj.remove("suggested");
                        obj.remove("rewrites");
                    }
                }
                if let Err(audit_error) = self.record("error", &json!(error), &result) {
                    return json!({"content":[{"type":"text","text":format!("{error}; cannot audit the error: {audit_error}")}],"isError":true});
                }
                result
            }
        }
    }
    /// Cancel uncommitted edits, never guess missing values or approve them on exit.
    pub(crate) fn recover_dialog(&mut self) -> Result<bool, String> {
        if self.plain || self.screen.as_ref().is_none_or(|s| s.dialog.is_none()) {
            return Ok(false);
        }
        self.record("dialog_recovery", &json!(self.screen), &json!("key esc"))?;
        let answer = self.send("key esc")?;
        self.look()?;
        if answer.to_lowercase().starts_with("error:")
            || self.screen.as_ref().is_none_or(|s| s.dialog.is_some())
        {
            return Err("could not cancel unfinished dialog; inspect before continuing".into());
        }
        Ok(true)
    }
    fn fill_dialog(&mut self, args: &Value, before_count: usize) -> Result<Value, String> {
        let screen = self.screen.as_ref().ok_or("screen unavailable")?;
        let dialog = screen
            .dialog
            .clone()
            .ok_or("fill_dialog requires an open dialog")?;
        let fields = args["fields"].as_array().ok_or("fields must be an array")?;
        let mut commands = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        // Validate every target and encoded command before sending the first edit.
        for field in fields {
            let requested = field["name"].as_str().ok_or("missing field name")?;
            let name = if self.plain {
                requested.to_owned()
            } else {
                screen.resolve(requested)?
            };
            if !self
                .last_text
                .lines()
                .any(|l| l.trim_start().starts_with(&format!("field \"{name}\":")))
            {
                return Err(format!("{name} is not a displayed dialog field"));
            }
            if !seen.insert(name.clone()) {
                return Err(format!("duplicate field {name}"));
            }
            let value = field["value"].as_str().ok_or("missing field value")?;
            commands.push(format!("click {name}"));
            // Moving away and back selects all, including a previously edited field.
            commands.push("key tab shift+tab".into());
            commands.push(format!("type {value}"));
        }
        commands.push(format!("click {}", screen.resolve("OK")?));
        for command in &commands {
            protocol::encode(command)?;
        }
        self.record("dialog_fill", args, &json!(commands))?;
        for command in commands {
            if self.screen.as_ref().and_then(|s| s.dialog.as_ref()) != Some(&dialog) {
                return Err("dialog changed during fill; remaining edits were not sent".into());
            }
            let answer = self.send(&command)?;
            self.look()?;
            if answer.to_lowercase().starts_with("error:") {
                return Err(format!("dialog fill stopped: {answer}"));
            }
        }
        if self.screen.as_ref().is_some_and(|s| s.dialog.is_some()) {
            return Err("OK left a dialog open; inspect its validation error".into());
        }
        let result = json!({"content":[{"type":"text","text":format!("Dialog filled and OK pressed.\n{}",self.last_text)}],"isError":false,"structuredContent":{"screen":self.screen,"rewrites":self.audit.count-before_count}});
        self.record("result", &Value::Null, &result)?;
        Ok(result)
    }
    fn execute(&mut self, name: &str, args: &Value, before_count: usize) -> Result<Value, String> {
        let schema = tools()
            .into_iter()
            .find(|t| t["name"] == name)
            .ok_or("unknown SolidCraft 98 tool; prose is never executed")?;
        validate(args, &schema["inputSchema"])?;
        if name != "look" {
            self.look()?;
        }
        if name == "fill_dialog" {
            return self.fill_dialog(args, before_count);
        }
        let command = self.command(name, args)?;
        let answer = if name == "look" {
            self.look()?
        } else {
            if name != "how" {
                self.offer = None;
                self.preferred_offer = None;
            }
            self.send(&command)?
        };
        let application_offer = answer
            .split_once("Say \"yes\" to do: ")
            .map(|(_, s)| s.trim().to_owned());
        let is_error = answer.to_lowercase().starts_with("error:");
        let mut text = answer.clone();
        if name != "look" {
            let screen = self.look()?;
            text.push('\n');
            text.push_str(&screen);
        }
        if application_offer.is_some() {
            self.preferred_offer = application_offer.clone();
            self.offer = application_offer;
        }
        let hint = if self.plain {
            None
        } else {
            self.screen
                .as_ref()
                .and_then(|s| s.tutor.clone())
                .map(|s| format!("SolidCraft 98 Tutor: {s}"))
        };
        let help = if !self.plain && name == "how" {
            Some(help(args["task"].as_str().unwrap_or("")))
        } else {
            None
        };
        let suggested = if !self.plain && answer.contains("Did you mean") {
            self.offer
                .as_ref()
                .map(|s| json!({"applicationOffer":s,"execute":false}))
        } else {
            None
        };
        let state = json!({"focus":self.screen.as_ref().and_then(|s|s.focus.clone()),"dialog":self.screen.as_ref().and_then(|s|s.dialog.clone()),"menu":self.screen.as_ref().and_then(|s|s.menu.clone()),"timeline":self.screen.as_ref().map(|s|s.timeline.clone())});
        let triple =
            json!([command, if is_error { Some(&answer) } else { None }, state]).to_string();
        let repeats = match &self.previous {
            Some((old, n)) if old == &triple => n + 1,
            _ => 1,
        };
        self.previous = Some((triple, repeats));
        if name != "look" && repeats >= 3 {
            self.actions = self.action_limit;
            return Err("three identical action/error/state results; stopped. Inspect the state and start an explicit new session.".into());
        }
        let structured = json!({"screen":self.screen,"hint":hint,"suggested":suggested,"pilotHelp":help,"offer":self.offer,"error":if is_error {Some(&answer)}else{None},"rewrites":self.audit.count-before_count+1+usize::from(hint.is_some() || help.is_some() || suggested.is_some())});
        let structured = if self.plain {
            json!({"screen":self.screen,"error":if is_error {Some(&answer)}else{None}})
        } else {
            structured
        };
        if !self.plain && (hint.is_some() || help.is_some() || suggested.is_some()) {
            self.record("result_guidance", &json!(answer), &structured)?;
        }
        self.record("result", &json!(answer), &structured).map_err(|e|format!("Remote command completed, but result audit failed: {e}. The command must not be repeated without inspecting the design."))?;
        Ok(
            json!({"content":[{"type":"text","text":text}],"isError":is_error,"structuredContent":structured}),
        )
    }
}
pub fn serve(cfg: Config) -> Result<(), String> {
    let mut server = Server::new(cfg)?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut bytes = Vec::new();
        let n = (&mut input)
            .take(65537)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        if bytes.len() > 65536 {
            return Err("MCP message exceeds 64 KiB".into());
        }
        let request: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("invalid MCP JSON: {e}"))?;
        let Some(id) = request.get("id") else {
            continue;
        };
        let result = match request["method"].as_str() {
            Some("initialize") => {
                json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"dutchman-pilot-solidcraft98","version":env!("CARGO_PKG_VERSION")}})
            }
            Some("ping") => json!({}),
            Some("tools/list") => json!({"tools":tools()}),
            Some("tools/call") => server.call(
                request["params"]["name"].as_str().unwrap_or(""),
                &request["params"]["arguments"],
            ),
            _ => {
                let response = json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"unknown MCP method"}});
                writeln!(output, "{response}").map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };
        writeln!(
            output,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"result":result})
        )
        .map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
}
