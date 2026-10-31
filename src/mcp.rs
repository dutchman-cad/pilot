//! Bounded, newline-delimited stdio MCP proxy with local discovery and planning.
use crate::audit::Audit;
use crate::catalog::{Catalog, MAX_CATALOG_BYTES};
use crate::guidance::{how_answer, how_tool, paging, payload, state_hint, unique_close};
use crate::{model, planner};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

const MAX_MESSAGE: usize = 8 * 1_048_576;
const MAX_PENDING: usize = 64;
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub solidcraft_cli: PathBuf,
    pub args: Vec<String>,
    pub no_plan: bool,
    pub no_hints: bool,
    pub no_how: bool,
    pub log: Option<PathBuf>,
}
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
enum Event {
    Client(Result<Option<Vec<u8>>, String>),
    Child(Result<Option<Vec<u8>>, String>),
}
fn frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    let n = reader
        .take((MAX_MESSAGE + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| format!("cannot read MCP message: {e}"))?;
    if n > MAX_MESSAGE {
        return Err("MCP message exceeds 8 MiB".into());
    }
    if n == 0 {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}
fn reader<R: Read + Send + 'static>(input: R, tx: SyncSender<Event>, child: bool) {
    thread::spawn(move || {
        let mut input = BufReader::new(input);
        loop {
            let result = frame(&mut input);
            let done = !matches!(&result, Ok(Some(_)));
            let event = if child {
                Event::Child(result)
            } else {
                Event::Client(result)
            };
            if tx.send(event).is_err() || done {
                break;
            }
        }
    });
}
fn write_raw(out: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|e| format!("cannot write MCP message: {e}"))
}
fn write_json(out: &mut impl Write, value: &Value) -> Result<(), String> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|e| format!("cannot encode MCP message: {e}"))?;
    if bytes.len() >= MAX_MESSAGE {
        return Err("processed MCP message exceeds 8 MiB".into());
    }
    bytes.push(b'\n');
    write_raw(out, &bytes)
}
struct Proxy {
    input: ChildStdin,
    output: io::Stdout,
    events: Receiver<Event>,
    queued: VecDeque<Vec<u8>>,
    eof: bool,
    serial: u64,
    retired: BTreeSet<String>,
    catalog: Option<Catalog>,
    inspect: String,
    cfg: Config,
    audit: Audit,
    sketch_seen: [bool; 2],
    plane_guidance: crate::guidance::PlaneGuidance,
    plane_seen: [bool; 2],
    axes_seen: bool,
    hole_faces: crate::guidance::holes::Faces,
}
impl Proxy {
    // While a call is outstanding, relay reverse requests and client replies immediately.
    // New client requests wait in a bounded queue, keeping before/after state unambiguous.
    fn receive(&mut self, id: &Value, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = match self.events.recv_timeout(remaining) {
                Ok(e) => e,
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(None),
                Err(e) => return Err(format!("MCP reader stopped: {e}")),
            };
            match event {
                Event::Child(Ok(Some(raw))) => {
                    let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                    if v.get("method").is_none() && v.get("id") == Some(id) {
                        return Ok(Some(raw));
                    }
                    if !self.is_retired(&v) {
                        write_raw(&mut self.output, &raw)?;
                    }
                }
                Event::Child(Ok(None)) => return Err("SolidCraft MCP child closed stdout".into()),
                Event::Client(Ok(Some(raw))) => {
                    let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                    if v.get("method").is_none() || v.get("id").is_none() {
                        write_raw(&mut self.input, &raw)?;
                    } else {
                        if self.queued.len() >= MAX_PENDING {
                            return Err("more than 64 queued MCP requests".into());
                        }
                        self.queued.push_back(raw);
                    }
                }
                Event::Client(Ok(None)) => self.eof = true,
                Event::Child(Err(e)) | Event::Client(Err(e)) => return Err(e),
            }
        }
    }
    fn is_retired(&self, v: &Value) -> bool {
        v.get("method").is_none()
            && v.get("id")
                .and_then(Value::as_str)
                .is_some_and(|s| self.retired.contains(s))
    }
    fn probe(&mut self, name: &str, args: Value) -> Result<Option<Value>, String> {
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("internal MCP id limit reached")?;
        // No client request is outstanding while probes run; IDs can safely be reused
        // by clients after each probe. Timed-out IDs remain reserved until their reply.
        let id = format!("{}{}", crate::identity::PROBE_PREFIX, self.serial);
        if self.retired.len() >= MAX_PENDING {
            return Err("too many timed-out MCP probes".into());
        }
        write_json(
            &mut self.input,
            &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}),
        )?;
        let reply = self.receive(&json!(id), PROBE_TIMEOUT)?;
        if let Some(raw) = reply {
            let value: Value = serde_json::from_slice(&raw)
                .map_err(|e| format!("invalid MCP probe reply: {e}"))?;
            if failed(&value) {
                return Ok(None);
            }
            let pages = result_pages(&value);
            if name == "list_commands" && !pages.is_empty() {
                return Ok(None);
            }
            let mut data = payload(&value);
            if !pages.is_empty() && data.is_object() {
                data["partial"] = json!(true);
            }
            Ok(Some(data))
        } else {
            self.retired.insert(id);
            eprintln!(
                "{}: read-only {name} probe timed out",
                crate::identity::BINARY
            );
            Ok(None)
        }
    }
    fn load_catalog(&mut self) -> Result<(), String> {
        if self.catalog.is_some() {
            return Ok(());
        }
        if let Some(value) = self.probe("list_commands", json!({}))? {
            if paging(&value).is_empty() {
                if let Ok(catalog) = Catalog::from_json(&value.to_string()) {
                    if !catalog.commands().is_empty() {
                        self.catalog = Some(catalog);
                    }
                }
            }
        }
        if self.catalog.is_none() {
            eprintln!(
                "{}: live catalogue unavailable; trying solidcraft-cli commands",
                crate::identity::BINARY
            );
            self.catalog = Some(fallback(&self.cfg)?);
        }
        Ok(())
    }
    fn call(&mut self, raw: &[u8], v: &Value) -> Result<(), String> {
        self.audit.count = 0;
        self.audit.kinds.clear();
        let method = v.get("method").and_then(Value::as_str).unwrap_or("");
        let name = v
            .pointer("/params/name")
            .and_then(Value::as_str)
            .unwrap_or("");
        let id = &v["id"];
        // Drain an outstanding timed-out probe before forwarding a client that
        // happens to reuse its ID. Never confuse an internal reply with theirs.
        if v.get("method").is_some() && id.as_str().is_some_and(|id| self.retired.contains(id)) {
            self.receive(id, PROBE_TIMEOUT)?
                .ok_or("client ID conflicts with an outstanding timed-out probe")?;
            if let Some(id) = id.as_str() {
                self.retired.remove(id);
            }
        }
        if method == "tools/call"
            && name == "batch"
            && v.pointer("/params/arguments/commands")
                .and_then(Value::as_array)
                .is_some_and(|a| a.len() > 256)
        {
            if v.get("id").is_some() {
                write_json(
                    &mut self.output,
                    &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"batch exceeds 256 commands"}}),
                )?;
            }
            return Ok(());
        }
        if method == "tools/call"
            && ((name == "how" && !self.cfg.no_how) || (name == "plan" && !self.cfg.no_plan))
        {
            self.load_catalog()?;
            let state = if name == "plan" {
                self.probe(&self.inspect.clone(), json!({}))?
            } else {
                None
            };
            let result = self
                .catalog
                .as_ref()
                .ok_or_else(|| "catalogue unavailable".to_string())
                .and_then(|catalog| {
                    if name == "how" {
                        how_answer(catalog, &v["params"]["arguments"])
                    } else {
                        let task = v.pointer("/params/arguments/task").and_then(Value::as_str)
                            .ok_or("plan requires a string task")?;
                        if task.trim().is_empty() || task.len() > 4096 {
                            return Err("plan task must contain 1–4096 bytes".into());
                        }
                        Ok(plan_for(catalog, task, state.as_ref())?
                            .map(|plan| json!({"rule":plan.rule,"calls":plan.calls,"facts":plan.facts}).to_string())
                            .unwrap_or_else(|| "No deterministic plan for that request; try how for command discovery.".into()))
                    }
                });
            if !id.is_null() {
                let reply = match result {
                    Ok(text) => {
                        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}})
                    }
                    Err(message) => {
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":message}})
                    }
                };
                let kind = if name == "how" { "how_answer" } else { "plan" };
                // Every local answer is logged; when the log cannot take it, say so instead.
                let reply = match self.audit.record(kind, v, &reply) {
                    Ok(()) => reply,
                    Err(message) => {
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":message}})
                    }
                };
                eprintln!("{}: answered {name} locally", crate::identity::BINARY);
                write_json(&mut self.output, &reply)?;
            }
            return Ok(());
        }
        if method == "tools/list" || method == "tools/call" {
            self.load_catalog()?;
        }
        let change = method == "tools/call" && matches!(name, "execute" | "batch");
        let before = if change && !self.cfg.no_hints {
            self.probe(&self.inspect.clone(), json!({}))?
        } else {
            None
        };
        write_raw(&mut self.input, raw)?;
        if method == "notifications/initialized" {
            self.load_catalog()?;
        }
        if v.get("id").is_none() || v.get("method").is_none() {
            return Ok(());
        }
        let response = self
            .receive(id, CALL_TIMEOUT)?
            .ok_or("SolidCraft MCP call timed out after 120 seconds")?;
        let mut reply: Value = match serde_json::from_slice(&response) {
            Ok(v) => v,
            Err(_) => return write_raw(&mut self.output, &response),
        };
        let mut modified = false;
        if method == "tools/list" {
            if let Some(tools) = reply
                .pointer_mut("/result/tools")
                .and_then(Value::as_array_mut)
            {
                if tools.len() >= 4096 {
                    return Err("tools/list exceeds 4096 tools".into());
                }
                if tools.iter().any(|t| t["name"] == "inspect")
                    && !tools.iter().any(|t| t["name"] == "inspect_design")
                {
                    self.inspect = "inspect".into();
                }
                // Avoid advertising a colliding upstream tool with different semantics.
                if !self.cfg.no_how {
                    tools.retain(|t| t["name"] != "how");
                    tools.push(how_tool());
                    modified = true;
                }
                if !self.cfg.no_plan {
                    tools.retain(|t| t["name"] != "plan");
                    tools.push(plan_tool());
                    modified = true;
                }
                if tools.len() > 4096 {
                    return Err("augmented tools/list exceeds 4096 tools".into());
                }
                if !self.cfg.no_hints && crate::prompting::describe_holes(tools) {
                    self.audit.record(
                        "hole_tool_hint",
                        &Value::Null,
                        &json!(crate::guidance::holes::HINT),
                    )?;
                    modified = true;
                }
                if !self.cfg.no_hints && crate::prompting::describe_placement(tools) {
                    if let Err(message) = self.audit.record(
                        "placement_fact",
                        &Value::Null,
                        &json!(crate::prompting::PLACEMENT_FACT),
                    ) {
                        eprintln!(
                            "{}: {message}; passing tools/list through unchanged",
                            crate::identity::BINARY
                        );
                        return write_raw(&mut self.output, &response);
                    }
                    modified = true;
                }
                if !self.cfg.no_hints && crate::prompting::describe_sketch(tools) {
                    if let Err(message) = self.audit.record(
                        "sketch_fact",
                        &Value::Null,
                        &json!(crate::prompting::SKETCH_FACT),
                    ) {
                        eprintln!(
                            "{}: {message}; passing tools/list through unchanged",
                            crate::identity::BINARY
                        );
                        return write_raw(&mut self.output, &response);
                    }
                    modified = true;
                }
            }
        }
        if method == "tools/call" {
            if name == "batch" {
                if let Some(reason) =
                    crate::repair::failed_sketch_context(&v["params"]["arguments"], &reply)
                {
                    modified |=
                        self.append(&mut reply, &reason, false, "sketch_dependency_error")?;
                }
            }
            let mut data = payload(&reply);
            let page = result_pages(&reply);
            if !page.is_empty() && data.is_object() {
                data["partial"] = json!(true);
            }
            if !page.is_empty() {
                modified |= self.append(&mut reply, &page.join("\n"), true, "paging")?;
            }
            if !self.cfg.no_hints {
                let args = &v["params"]["arguments"];
                let hints = self.hole_faces.visit(name, args, &reply);
                if !hints.is_empty() {
                    modified |=
                        self.append(&mut reply, &hints.join("\n"), false, "hole_position_hint")?;
                }
                self.plane_guidance.visit(name, args, &reply);
                // Sketch setup changes no body, so plane guidance does not wait for a change.
                let mut context = json!({"tools":[{"function":{"name":"execute"}}],"messages":[{"role":"user","content":args.to_string()},{"role":"assistant","tool_calls":[{"function":{"name":name,"arguments":args.to_string()}}]}]});
                if !self.axes_seen && crate::prompting::append_sketch_axes(&mut context)? {
                    modified |= self.append(
                        &mut reply,
                        crate::prompting::SKETCH_AXES,
                        false,
                        "sketch_axes_fact",
                    )?;
                    self.axes_seen = true;
                }
                for (i, hint) in self.plane_guidance.hints().into_iter().enumerate() {
                    if let Some(hint) = hint.filter(|_| !self.plane_seen[i]) {
                        modified |= self.append(&mut reply, hint, false, "plane_hint")?;
                        self.plane_seen[i] = true;
                    }
                }
            }
            if !self.cfg.no_hints && change {
                let args = &v["params"]["arguments"];
                // MCP has no turn history; a new sketch starts a new hint scope.
                if name == "execute" && args["command"] == "sketch.create" {
                    self.sketch_seen = [false; 2];
                }
                let sketch =
                    crate::guidance::sketch_hints(name, args, &reply, &mut self.sketch_seen);
                if !sketch.is_empty() {
                    modified |=
                        self.append(&mut reply, &sketch.join("\n"), false, "next_step_hint")?;
                }
                let hints =
                    crate::guidance::placement_hints(name, &v["params"]["arguments"], &data);
                if !hints.is_empty() {
                    modified |=
                        self.append(&mut reply, &hints.join("\n"), false, "next_step_hint")?;
                }
            }
            if failed(&reply) {
                let (hint, suggested) = self.correction(v, &reply)?;
                if let Some(text) = hint {
                    modified |= self.append(&mut reply, &text, false, "command_hint")?;
                }
                // MCP has no user-message history: only explicit task text (or a
                // command field containing the task itself) can drive a plan.
                // Never suggest replaying a batch that may have partly succeeded.
                let planned = if !self.cfg.no_plan && name != "batch" {
                    let task = v
                        .pointer("/params/arguments/task")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            v.pointer("/params/arguments/command")
                                .and_then(Value::as_str)
                        });
                    if let Some(task) = task.filter(|s| s.len() <= 4096) {
                        let state = self.probe(&self.inspect.clone(), json!({}))?;
                        self.catalog
                            .as_ref()
                            .and_then(|c| plan_for(c, task, state.as_ref()).ok().flatten())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(plan) = planned {
                    let mut text = plan
                        .calls
                        .iter()
                        .map(|call| format!("suggested: {call}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Some(facts) = plan.facts {
                        text.push_str(&format!("\nComputed facts: {facts}"));
                    }
                    modified |= self.append(&mut reply, &text, false, "plan")?;
                } else if let Some(call) = suggested {
                    let text = format!("suggested: {call}\nSay \"yes\" to do: {call}");
                    modified |= self.append(&mut reply, &text, false, "suggested_call")?;
                }
            } else if !self.cfg.no_hints && (change || matches!(name, "inspect" | "inspect_design"))
            {
                let after = if change {
                    self.probe(&self.inspect.clone(), json!({}))?
                } else {
                    Some(data)
                };
                if let Some(after) = after {
                    if let Some(hint) =
                        state_hint(before.as_ref(), &after, change, self.catalog.as_ref())
                    {
                        modified |= self.append(&mut reply, &hint, false, "next_step_hint")?;
                    }
                }
            }
        }
        if modified && method == "tools/list" {
            let original: Value = serde_json::from_slice(&response)
                .map_err(|e| format!("invalid tools/list reply: {e}"))?;
            if let Err(message) = self.audit.record("tools_list", &original, &reply) {
                eprintln!(
                    "{}: {message}; passing tools/list through unchanged",
                    crate::identity::BINARY
                );
                modified = false;
            }
        }
        if modified {
            eprintln!(
                "{}: augmented {method} {name} reply",
                crate::identity::BINARY
            );
            write_json(&mut self.output, &reply)
        } else {
            write_raw(&mut self.output, &response)
        }
    }
    fn append(
        &mut self,
        reply: &mut Value,
        text: &str,
        first: bool,
        kind: &str,
    ) -> Result<bool, String> {
        let before = reply.clone();
        if !add_text(reply, text, first) {
            return Ok(false);
        }
        // An unlogged rewrite is never sent: a full log leaves the reply as it was.
        if let Err(message) = self.audit.record(kind, &before, reply) {
            eprintln!("{}: {message}; {kind} not added", crate::identity::BINARY);
            *reply = before;
            return Ok(false);
        }
        Ok(true)
    }
    fn correction(
        &mut self,
        request: &Value,
        reply: &Value,
    ) -> Result<(Option<String>, Option<Value>), String> {
        let text = reply.to_string().to_ascii_lowercase();
        let args = &request["params"]["arguments"];
        // A failed batch can have completed steps with rollback disabled: never suggest
        // replaying the batch. Only suggest its explicitly identified failing step.
        let data = payload(reply);
        let args = if request["params"]["name"] == "batch" {
            let Some(index) = data
                .get("index")
                .or_else(|| data.get("failed_index"))
                .or_else(|| data.pointer("/error/index"))
                .or_else(|| data.pointer("/failed/index"))
                .and_then(Value::as_u64)
            else {
                return Ok((None, None));
            };
            let Some(step) = args
                .get("commands")
                .and_then(Value::as_array)
                .and_then(|a| a.get(index as usize))
            else {
                return Ok((None, None));
            };
            step
        } else if request["params"]["name"] == "execute" {
            args
        } else {
            return Ok((None, None));
        };
        let Some(command) = args.get("command").and_then(Value::as_str) else {
            return Ok((None, None));
        };
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok((None, None));
        };
        let mut fixed = args.clone();
        match crate::repair::repair_command(&mut fixed, catalog) {
            Ok(changes) if !changes.is_empty() => {
                return Ok((None, Some(json!({"tool":"execute","arguments":fixed}))));
            }
            Err(reason) => return Ok((Some(reason), None)),
            _ => (),
        }
        if text.contains("unknown command") {
            let task = request
                .pointer("/params/arguments/task")
                .and_then(Value::as_str)
                .unwrap_or("");
            let (hint, best) = if command.len() > crate::catalog::MAX_QUERY_BYTES {
                (
                    "unknown command id exceeds 256 bytes; use plan or how".into(),
                    None,
                )
            } else {
                (
                    format!(
                        "{}. Did you mean one of these?",
                        catalog.unknown_hint(command, task)?
                    ),
                    unique_close(catalog, command)?,
                )
            };
            let suggested = best.map(|id| {
                let mut fixed = args.clone();
                fixed["command"] = json!(id);
                json!({"tool":"execute","arguments":fixed})
            });
            return Ok(((!upstream_command_hint(reply)).then_some(hint), suggested));
        }
        if !["parameter", "param", "required", "missing", "unknown field"]
            .iter()
            .any(|s| text.contains(s))
        {
            return Ok((None, None));
        }
        let docs = catalog.get(command).map(|c| c.params.clone());
        let mut description = self
            .probe("describe_command", json!({"id":command}))?
            .unwrap_or(Value::Null);
        if !description.get("input").is_some_and(Value::is_object) {
            description = doc_contract(docs.as_deref().unwrap_or(""));
        }
        let Some(params) = args.get("params").and_then(Value::as_object) else {
            return Ok((None, None));
        };
        let fixed = reconcile(params, &description);
        Ok((
            None,
            fixed.map(|params| {
                let mut fixed = args.clone();
                fixed["params"] = json!(params);
                json!({"tool":"execute","arguments":fixed})
            }),
        ))
    }
    fn run(&mut self) -> Result<(), String> {
        loop {
            if let Some(raw) = self.queued.pop_front() {
                let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                self.call(&raw, &v)?;
                continue;
            }
            if self.eof {
                return Ok(());
            }
            match self
                .events
                .recv()
                .map_err(|e| format!("MCP readers stopped: {e}"))?
            {
                Event::Client(Ok(Some(raw))) => {
                    let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                    self.call(&raw, &v)?;
                }
                Event::Client(Ok(None)) => return Ok(()),
                Event::Child(Ok(Some(raw))) => {
                    let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                    if !self.is_retired(&v) {
                        write_raw(&mut self.output, &raw)?;
                    }
                }
                Event::Child(Ok(None)) => return Err("SolidCraft MCP child exited".into()),
                Event::Client(Err(e)) | Event::Child(Err(e)) => return Err(e),
            }
        }
    }
}
pub fn serve(cfg: Config) -> Result<(), String> {
    let path = match &cfg.log {
        Some(path) => path.clone(),
        None => {
            let dir = model::data_dir(None)?;
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("cannot create log directory: {e}"))?;
            dir.join("rewrites.jsonl")
        }
    };
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("cannot open rewrite log: {e}"))?;
    let size = file
        .metadata()
        .map_err(|e| format!("cannot inspect rewrite log: {e}"))?
        .len();
    let audit = Audit {
        events: None,
        file,
        size,
        model: "mcp".into(),
        count: 0,
        kinds: BTreeMap::new(),
    };
    let mut child = Process(
        Command::new(&cfg.solidcraft_cli)
            .arg("mcp")
            .args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start SolidCraft MCP child: {e}"))?,
    );
    let input = child.0.stdin.take().ok_or("child stdin unavailable")?;
    let output = child.0.stdout.take().ok_or("child stdout unavailable")?;
    let (tx, events) = mpsc::sync_channel(16);
    reader(output, tx.clone(), true);
    reader(io::stdin(), tx, false);
    Proxy {
        input,
        output: io::stdout(),
        events,
        queued: VecDeque::new(),
        eof: false,
        serial: 0,
        retired: BTreeSet::new(),
        catalog: None,
        inspect: "inspect_design".into(),
        cfg,
        audit,
        sketch_seen: [false; 2],
        plane_guidance: crate::guidance::PlaneGuidance::default(),
        hole_faces: crate::guidance::holes::Faces::default(),
        plane_seen: [false; 2],
        axes_seen: false,
    }
    .run()
}
fn plan_tool() -> Value {
    json!({"name":"plan","description":"Compute exact catalogue-checked calls for a common task using current design state; never executes them.","inputSchema":{"type":"object","properties":{"task":{"type":"string","maxLength":4096}},"required":["task"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false}})
}
fn plan_for(
    catalog: &Catalog,
    task: &str,
    state: Option<&Value>,
) -> Result<Option<planner::Plan>, String> {
    let mut messages = Vec::new();
    if let Some(state) = state {
        messages.push(json!({"role":"tool","content":state}));
    }
    messages.push(json!({"role":"user","content":task}));
    planner::plan(
        &json!({"messages":messages,"tools":[{"type":"function","function":{"name":"execute"}}]}),
        catalog,
    )
}
fn fallback(cfg: &Config) -> Result<Catalog, String> {
    let mut child = Process(
        Command::new(&cfg.solidcraft_cli)
            .arg("commands")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot fetch fallback catalogue: {e}"))?,
    );
    let output = child
        .0
        .stdout
        .take()
        .ok_or("catalogue stdout unavailable")?;
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output
            .take((MAX_CATALOG_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| format!("cannot read catalogue: {e}"));
        let _ = tx.send(result);
    });
    let bytes = rx
        .recv_timeout(PROBE_TIMEOUT)
        .map_err(|e| format!("fallback catalogue unavailable: {e}"))??;
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|e| format!("cannot wait for catalogue: {e}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("fallback catalogue process timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        return Err(format!("fallback catalogue process failed: {status}"));
    }
    Catalog::from_json(
        std::str::from_utf8(&bytes).map_err(|e| format!("catalogue is not UTF-8: {e}"))?,
    )
}
fn upstream_command_hint(reply: &Value) -> bool {
    fn contains_hint(text: &str) -> bool {
        let words = text
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|word| !word.is_empty());
        let mut previous = ["", ""];
        for word in words {
            if word.eq_ignore_ascii_case("closest")
                || (previous[0].eq_ignore_ascii_case("did")
                    && previous[1].eq_ignore_ascii_case("you")
                    && word.eq_ignore_ascii_case("mean"))
            {
                return true;
            }
            previous = [previous[1], word];
        }
        false
    }
    fn error_hint(data: &Value) -> bool {
        data.get("error")
            .and_then(Value::as_str)
            .into_iter()
            .chain(data.pointer("/error/message").and_then(Value::as_str))
            .chain(data.pointer("/failed/error").and_then(Value::as_str))
            .any(contains_hint)
    }
    error_hint(reply)
        || reply
            .pointer("/result/structuredContent")
            .is_some_and(error_hint)
        || reply
            .pointer("/result/content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .any(|text| match serde_json::from_str::<Value>(text) {
                        Ok(data) => error_hint(&data),
                        Err(_) => contains_hint(text),
                    })
            })
}
fn failed(v: &Value) -> bool {
    v.get("error").is_some()
        || v.pointer("/result/isError") == Some(&json!(true))
        || payload(v).get("error").is_some()
}
fn add_text(reply: &mut Value, text: &str, first: bool) -> bool {
    let block = json!({"type":"text","text":text});
    if reply.get("result").is_some_and(Value::is_object)
        && reply.pointer("/result/content").is_none()
    {
        reply["result"]["content"] = json!([]);
    }
    if let Some(content) = reply
        .pointer_mut("/result/content")
        .and_then(Value::as_array_mut)
    {
        if first {
            content.insert(0, block);
        } else {
            content.push(block);
        }
        true
    } else if let Some(error) = reply.get_mut("error").and_then(Value::as_object_mut) {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("");
        error.insert("message".into(), json!(format!("{message}\n{text}")));
        true
    } else {
        false
    }
}
fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn type_matches(value: &Value, schema: &Value, root: &Value, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|s| s.strip_prefix('#'))
    {
        return root
            .pointer(reference)
            .is_some_and(|s| type_matches(value, s, root, depth + 1));
    }
    if let Some(choices) = schema.get("anyOf").and_then(Value::as_array) {
        return choices
            .iter()
            .take(32)
            .any(|s| type_matches(value, s, root, depth + 1));
    }
    let check = |t: &str| match t {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    schema.get("type").is_some_and(|t| {
        t.as_str().is_some_and(check)
            || t.as_array()
                .is_some_and(|a| a.iter().filter_map(Value::as_str).any(check))
    })
}
fn reconcile(
    params: &serde_json::Map<String, Value>,
    description: &Value,
) -> Option<serde_json::Map<String, Value>> {
    let schema = description.get("input")?;
    let properties = schema.get("properties")?.as_object()?;
    if properties.len() > 256 || params.len() > 256 {
        return None;
    }
    let unknown: Vec<_> = params
        .keys()
        .filter(|k| !properties.contains_key(*k))
        .collect();
    // Exactly one unknown spelling and one equivalent target; types alone
    // cannot identify the intended parameter.
    if unknown.len() != 1 {
        return None;
    }
    let source = unknown[0];
    let missing: Vec<_> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|k| !params.contains_key(*k))
        .collect();
    let candidates: Vec<_> = properties
        .iter()
        .filter(|(k, s)| {
            !params.contains_key(*k)
                && type_matches(&params[source], s, schema, 0)
                && norm(source) == norm(k)
        })
        .map(|(k, _)| k.as_str())
        .collect();
    if candidates.len() != 1 {
        return None;
    }
    if missing.iter().any(|k| *k != candidates[0]) {
        return None;
    }
    let mut fixed = params.clone();
    let value = fixed.remove(source)?;
    fixed.insert(candidates[0].into(), value);
    Some(fixed)
}

// Freeform docs are a fallback only. Infer types solely from explicit type words,
// never from parameter names; an undocumented parameter cannot carry a suggestion.
fn doc_contract(doc: &str) -> Value {
    json!({"input":crate::catalog::parameter_schema(doc)})
}
fn result_pages(reply: &Value) -> Vec<String> {
    let result = reply.get("result").unwrap_or(reply);
    let mut pages = paging(&payload(reply));
    if let Some(blocks) = result.get("content").and_then(Value::as_array) {
        for text in blocks
            .iter()
            .take(32)
            .filter_map(|b| b.get("text").and_then(Value::as_str))
        {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                pages.extend(paging(&value));
            }
        }
    }
    // Paging metadata can also accompany structuredContent or text content.
    let mut outer = result.clone();
    if let Some(obj) = outer.as_object_mut() {
        obj.remove("content");
        obj.remove("structuredContent");
        if let Some(data) = payload(reply).as_object() {
            for (key, value) in data {
                if value.is_array() {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }
        pages.extend(paging(&outer));
    }
    let mut seen = BTreeSet::new();
    pages.retain(|p| seen.insert(p.clone()));
    pages.truncate(16);
    pages
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    #[test]
    fn upstream_command_hints_cover_structured_errors() {
        for error in [
            "unknown command solid.bxo; closest: solid.box",
            "unknown command solid.bxo. DID\tYOU\nMEAN: solid.box?",
        ] {
            for data in [json!({"error":error}), json!({"failed":{"error":error}})] {
                assert!(upstream_command_hint(&json!({
                    "result":{"structuredContent":data}
                })));
                assert!(upstream_command_hint(&json!({
                    "result":{"content":[{"type":"text","text":data.to_string()}]}
                })));
            }
        }
        let data = json!({"error":"unknown command solid.bxo",
            "params":{"label":"closest: solid.box"}});
        for result in [
            json!({"structuredContent":data}),
            json!({"content":[{"type":"text","text":data.to_string()}]}),
        ] {
            assert!(!upstream_command_hint(&json!({"result":result})));
        }
    }
    #[test]
    fn hint_rules_require_evidence() {
        let catalog = Catalog::bundled().unwrap();
        let empty = json!({"bodies":[],"sketches":[],"active_sketch":null});
        assert!(state_hint(None, &empty, false, Some(&catalog))
            .unwrap()
            .contains("Empty design"));
        let sketch = json!({"bodies":[],"active_sketch":1,"sketches":[{"name":"S","dof":4}]});
        let hint = state_hint(None, &sketch, false, Some(&catalog)).unwrap();
        assert!(hint.contains("4 free degrees"));
        assert!(hint.contains("Say \"yes\" to do:"));
        let a = json!({"bodies":[{"volume_mm3":5}]});
        let b = json!({"bodies":[{"volume_mm3":6}]});
        assert!(state_hint(Some(&a), &a, true, None)
            .unwrap()
            .contains("design didn't change"));
        assert!(state_hint(Some(&a), &b, true, None).is_none());
        assert!(state_hint(None, &a, true, None).is_none());
        assert!(state_hint(Some(&a), &json!({"bodies":[{}]}), true, None).is_none());
        assert!(state_hint(
            Some(&empty),
            &json!({"bodies":[],"sketches":[],"truncated":true}),
            true,
            None
        )
        .is_none());
    }
    #[test]
    fn fallback_contract_matches_catalogue_for_every_command() {
        let catalog = Catalog::bundled().unwrap();
        for command in catalog.commands() {
            assert_eq!(
                doc_contract(&command.params)["input"],
                catalog.parameter_schema(&command.id),
                "{}",
                command.id
            );
        }
        let extrude = doc_contract(&catalog.get("solid.extrude").unwrap().params);
        assert_eq!(extrude["input"]["required"], json!([]));
        assert!(reconcile(json!({"through_all":true}).as_object().unwrap(), &extrude).is_none());
    }
    #[test]
    fn corrections_are_unique_and_never_invent_values() {
        for (source, target) in [
            ("radius", "diameter"),
            ("diameter", "radius"),
            ("distance", "radius"),
            ("width", "height"),
            ("start_end", "end_start"),
        ] {
            let description =
                json!({"input":{"properties":{target:{"type":"number"}},"required":[target]}});
            assert!(reconcile(json!({source:5}).as_object().unwrap(), &description).is_none());
        }
        let catalog = Catalog::from_json(r#"[{"id":"solid.box","label":"Box","params":""},{"id":"solid.bow","label":"Bow","params":""}]"#).unwrap();
        assert!(unique_close(&catalog, "solid.boz").unwrap().is_none());
        assert_eq!(
            unique_close(&catalog, "SOLID_BOX").unwrap().as_deref(),
            Some("solid.box")
        );
        assert!(unique_close(&catalog, "unrelated").unwrap().is_none());
        let description = doc_contract("length, width, height: expr; edges?: [[x,y,z]]");
        let p = json!({"length":1,"width":2,"Height":3});
        let fixed = reconcile(p.as_object().unwrap(), &description).unwrap();
        assert_eq!(fixed["height"], 3);
        assert!(!fixed.contains_key("Height"));
        assert!(reconcile(json!({"length":1}).as_object().unwrap(), &description).is_none());
        assert!(reconcile(
            json!({"length":1,"width":2,"foo":[]}).as_object().unwrap(),
            &description
        )
        .is_none());
        assert!(reconcile(
            json!({"length":1,"width":2,"height":3,"foo":4})
                .as_object()
                .unwrap(),
            &description
        )
        .is_none());
        let malformed = json!({"input":{"required":["absent"],"properties":{}}});
        assert!(reconcile(json!({"foo":3}).as_object().unwrap(), &malformed).is_none());
    }
    #[test]
    fn paging_preserves_unknowns_and_zeroes() {
        assert_eq!(
            paging(&json!({"items":[],"total":12,"truncated":true})),
            ["shown 0 of 12 (partial)"]
        );
        assert_eq!(
            paging(&json!({"items":[1],"nextCursor":"next"})),
            ["shown 1 of unknown total (partial)"]
        );
        assert!(paging(&json!({"items":[],"total":0})).is_empty());
        assert!(paging(&json!({"items":[1,2]})).is_empty());
        let reply = json!({"result":{"total":10,"truncated":true,"content":[{"type":"text","text":"{\"items\":[1,2]}"}]}});
        assert_eq!(result_pages(&reply), ["shown 2 of 10 (partial)"]);
        let structured = json!({"result":{"structuredContent":{"items":[],"total":5}}});
        assert_eq!(result_pages(&structured), ["shown 0 of 5 (partial)"]);
    }
    #[test]
    fn frame_limits_and_exact_bytes() {
        let mut input = std::io::Cursor::new(b" {\"id\":1} \r\nlast");
        assert_eq!(frame(&mut input).unwrap().unwrap(), b" {\"id\":1} \r\n");
        assert_eq!(frame(&mut input).unwrap().unwrap(), b"last");
        assert!(frame(&mut input).unwrap().is_none());
        assert!(frame(&mut std::io::Cursor::new(vec![b'x'; MAX_MESSAGE + 1])).is_err());
    }
}
