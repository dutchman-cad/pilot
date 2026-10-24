//! Bounded OpenAI-compatible driver over the MCP face's validated session.
use super::{guidance, mcp, transport::Endpoint};
use crate::repair;
use serde_json::{json, Value};
use std::{
    io::Read,
    path::PathBuf,
    time::{Duration, Instant},
};

pub const SYSTEM: &str = "You are using SolidCraft 98 through a screen reader. Execute one explicit call per turn. Reply with exactly one COMMAND: line, optionally preceded by THOUGHT: commentary, or one tool JSON object {\"tool\":\"name\",\"args\":{}}. Native tool calls are also accepted, exactly one per turn. Commands: look; click <displayed name or x y>; doubleclick <name or x y>; rightclick <name or x y>; menu <title> > <item>; key <keys>; type <literal text>; drag <x1> <y1> <x2> <y2> [right]; wait <0-5000 ms>; how <words>; yes. COMMAND: done <message> finishes locally. Read the screen after each action. Prose is never executed; several calls and unknown verbs are format errors.";
#[derive(Debug)]
pub struct Config {
    pub endpoint: Endpoint,
    pub upstream: String,
    pub model: String,
    pub task: String,
    pub plain: bool,
    pub log: Option<PathBuf>,
    pub budget: usize,
    pub wall: Duration,
    pub tokens: u64,
}
#[derive(Debug, PartialEq)]
pub enum Call {
    Tool { name: String, args: Value },
    Done(String),
}

pub fn chat_tools() -> Vec<Value> {
    mcp::tools().into_iter().map(|t| json!({"type":"function","function":{"name":t["name"],"description":t["description"],"parameters":t["inputSchema"]}})).collect()
}
fn canonical(name: &str, plain: bool) -> &str {
    match name {
        "dblclick" | "double-click" | "double_click" if !plain => "doubleclick",
        "rclick" | "right-click" | "right_click" if !plain => "rightclick",
        _ => name,
    }
}
/// Full-input parsing: never extracts an object or a command out of surrounding prose.
pub fn parse_text(text: &str, plain: bool) -> Result<Call, String> {
    if text.len() > 65536 {
        return Err("reply exceeds 64 KiB".into());
    }
    let text = text.trim_start();
    let object_text = if !plain {
        repair::strip_fence(text)
    } else {
        text.trim()
    };
    if object_text.starts_with('{') {
        let v: Value = if plain {
            serde_json::from_str(object_text).map_err(|e| e.to_string())?
        } else {
            repair::parse_json(object_text)?
        };
        repair::check_value(&v)?;
        let f = v.get("function").unwrap_or(&v);
        let (name, args) = if let Some(name) = f
            .get("tool")
            .or_else(|| f.get("name"))
            .and_then(Value::as_str)
        {
            let obj = f.as_object().ok_or("tool call must be an object")?;
            if obj.contains_key("tool") && obj.contains_key("name")
                || obj.contains_key("args") && obj.contains_key("arguments")
            {
                return Err("ambiguous tool name or argument fields".into());
            }
            if obj
                .keys()
                .any(|k| !matches!(k.as_str(), "tool" | "name" | "args" | "arguments"))
            {
                return Err("unexpected tool call field".into());
            }
            (
                name.to_owned(),
                f.get("args")
                    .or_else(|| f.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({})),
            )
        } else {
            let obj = v
                .as_object()
                .filter(|o| o.len() == 1)
                .ok_or("one tool object required")?;
            let (name, args) = obj.iter().next().ok_or("empty tool object")?;
            (name.clone(), args.clone())
        };
        if v.get("function").is_some() && v.as_object().is_none_or(|o| o.len() != 1) {
            return Err("one explicit tool object required".into());
        }
        return checked(name, args, plain);
    }
    let mut command = None;
    let mut thought = false;
    for line in text
        .lines()
        .map(str::trim_start)
        .filter(|s| !s.trim().is_empty())
    {
        if let Some(rest) = line.strip_prefix("COMMAND:") {
            if command.replace(rest.trim_start()).is_some() {
                return Err("exactly one COMMAND: line required".into());
            }
        } else if line.starts_with("THOUGHT:") && command.is_none() && !thought {
            thought = true;
        } else {
            return Err(
                "expected one COMMAND: line or one tool JSON object; prose is not a call".into(),
            );
        }
    }
    wire(command.ok_or("missing COMMAND: line")?, plain)
}
fn checked(name: String, args: Value, plain: bool) -> Result<Call, String> {
    let canonical = canonical(&name, plain);
    if !mcp::tools().iter().any(|t| t["name"] == canonical) {
        return Err(format!("unknown SolidCraft 98 verb {name}"));
    }
    Ok(Call::Tool { name, args })
}
fn wire(line: &str, plain: bool) -> Result<Call, String> {
    let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
    if name == "type" {
        return checked(name.into(), json!({"text":rest}), plain);
    }
    let rest = rest.trim();
    if name == "done" {
        if rest.is_empty() {
            return Err("done requires an answer message".into());
        }
        return Ok(Call::Done(rest.into()));
    }
    let args = match canonical(name, plain) {
        "look" | "yes" if rest.is_empty() => json!({}),
        "click" | "doubleclick" | "rightclick" if !rest.is_empty() => {
            let numbers: Vec<_> = rest.split_whitespace().collect();
            if numbers.len() == 2 && numbers.iter().all(|s| s.parse::<u64>().is_ok()) {
                json!({"point":{"x":numbers[0].parse::<u64>().map_err(|e|e.to_string())?,"y":numbers[1].parse::<u64>().map_err(|e|e.to_string())?}})
            } else {
                json!({"name":rest})
            }
        }
        "menu" if !rest.is_empty() => match rest.split_once('>') {
            Some((title, item)) => json!({"title":title.trim(),"item":item.trim()}),
            None => json!({"title":rest}),
        },
        "key" => json!({"keys":rest}),
        "how" => json!({"task":rest}),
        "wait" => {
            json!({"ms":rest.parse::<u64>().map_err(|_|"wait requires integer milliseconds")?})
        }
        "drag" => {
            let v: Vec<_> = rest.split_whitespace().collect();
            if !(v.len() == 4 || v.len() == 5 && v[4] == "right") {
                return Err("drag requires four coordinates and optional right".into());
            }
            let n: Vec<u64> = v[..4]
                .iter()
                .map(|s| s.parse().map_err(|_| "invalid drag coordinate"))
                .collect::<Result<_, _>>()?;
            json!({"x1":n[0],"y1":n[1],"x2":n[2],"y2":n[3],"button":if v.len()==5 {"right"}else{"left"}})
        }
        _ => return Err(format!("unknown verb or invalid arguments: {name}")),
    };
    checked(name.into(), args, plain)
}
/// Parse one native call or one complete explicit text call.
pub fn parse_message(message: &Value, plain: bool) -> Result<Call, String> {
    if let Some(calls) = message.get("tool_calls").filter(|v| !v.is_null()) {
        let calls = calls.as_array().ok_or("tool_calls must be an array")?;
        if !calls.is_empty() {
            if calls.len() != 1 {
                return Err("exactly one native tool call required".into());
            }
            if message["content"]
                .as_str()
                .is_some_and(|s| s.contains("COMMAND:") || repair::strip_fence(s).starts_with('{'))
            {
                return Err("do not mix native and explicit text calls".into());
            }
            let f = &calls[0]["function"];
            let args = match f["arguments"].as_str() {
                Some(text) if plain => serde_json::from_str(text)
                    .map_err(|e| format!("invalid tool arguments: {e}"))?,
                Some(text) => repair::parse_json(text)?,
                None => f["arguments"].clone(),
            };
            return checked(
                f["name"].as_str().ok_or("missing tool name")?.into(),
                args,
                plain,
            );
        }
    }
    parse_text(message["content"].as_str().unwrap_or(""), plain)
}
fn response(
    agent: &ureq::Agent,
    url: &str,
    body: &Value,
    timeout: Duration,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
    let r = agent
        .post(url)
        .timeout(timeout)
        .set("Content-Type", "application/json")
        .send_bytes(&bytes)
        .map_err(|e| format!("upstream request failed: {e}"))?;
    let mut bytes = Vec::new();
    r.into_reader()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("upstream response exceeds 1 MiB".into());
    }
    let v = serde_json::from_slice(&bytes).map_err(|e| format!("invalid completion JSON: {e}"))?;
    repair::check_value(&v)?;
    Ok(v)
}
pub fn run(cfg: Config) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(0)
        .build();
    run_with(cfg, |url, body, timeout| {
        response(&agent, url, body, timeout)
    })
}
/// Embedding entry point with a completion client, sharing all parsing, budgets, and auditing.
/// The callback must honor its timeout and return a bounded completion (at most 1 MiB).
pub fn run_with(
    cfg: Config,
    completion: impl FnMut(&str, &Value, Duration) -> Result<Value, String>,
) -> Result<Value, String> {
    let mut session = mcp::Server::new(mcp::Config {
        endpoint: cfg.endpoint.clone(),
        log: cfg.log.clone(),
    })?;
    run_in_session(cfg, &mut session, completion)
}
/// Run against an existing session, preserving its design and consumed command budget.
/// The session supplies the transport and log; Config supplies driver limits and settings.
/// The completion callback must honor its timeout and the 1 MiB response cap.
pub fn run_in_session(
    cfg: Config,
    session: &mut mcp::Server,
    mut completion: impl FnMut(&str, &Value, Duration) -> Result<Value, String>,
) -> Result<Value, String> {
    if cfg.task.trim().is_empty()
        || cfg.task.len() > 4096
        || cfg.model.is_empty()
        || cfg.model.len() > 256
    {
        return Err("task/model empty or oversized (4096/256 bytes)".into());
    }
    if cfg.wall.is_zero()
        || cfg.wall > Duration::from_secs(900)
        || !(1..=131072).contains(&cfg.tokens)
    {
        return Err("wall/token budgets outside 1–900 seconds / 1–131072 tokens".into());
    }
    let url = url::Url::parse(&format!(
        "{}/chat/completions",
        cfg.upstream.trim_end_matches('/')
    ))
    .map_err(|e| format!("invalid upstream URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("upstream requires http(s), without credentials".into());
    }
    let started = Instant::now();
    let deadline = started + cfg.wall;
    session.driver_options(cfg.plain, cfg.budget, &cfg.model)?;
    let mut system = SYSTEM.to_owned();
    if !cfg.plain {
        let addition = format!("\nPilot help (application help wins): {}\nIn a dialog, typing replaces the focused field; Tab advances, Enter confirms. Read fields before changing values. Never infer that a described action ran.",guidance::help(&cfg.task));
        session.record(
            "prompt",
            &json!(system),
            &json!(format!("{system}{addition}")),
        )?;
        system.push_str(&addition);
    }
    session.driver_deadline(deadline);
    let first = session.final_look();
    let mut stop = if Instant::now() >= deadline {
        "wall_budget".to_owned()
    } else if first.is_err() {
        "remote_error".to_owned()
    } else {
        "step_budget".to_owned()
    };
    let mut errors = u64::from(first.is_err());
    let mut final_error = first.err();
    let mut history: Vec<Vec<Value>> = Vec::new();
    let mut feedback = String::new();
    let mut steps = 0;
    let mut actions = 0;
    let mut tokens = 0u64;
    let mut done = String::new();
    let mut native = 0;
    let mut text_calls = 0;
    let mut retries = 0;
    let mut hand_backs = 0;
    let mut consecutive_errors = 0;
    let mut empty_retry = false;
    let mut previous: Option<(String, usize)> = None;
    while final_error.is_none() && steps < cfg.budget {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            stop = "wall_budget".into();
            break;
        }
        let mut messages = vec![
            json!({"role":"system","content":system}),
            json!({"role":"user","content":cfg.task}),
        ];
        for group in &history {
            messages.extend(group.clone());
        }
        let current = json!({"role":"user","content":format!("SCREEN:\n{}\nRESULT: {feedback}",session.screen_text())});
        messages.push(current.clone());
        while json!(messages).to_string().len() > 1_048_576 && !history.is_empty() {
            let removed = history.remove(0).len();
            messages.drain(2..2 + removed);
        }
        if json!(messages).to_string().len() > 1_048_576 {
            stop = "history_budget".into();
            break;
        }
        let input_bound =
            (json!(messages).to_string().len() + json!(chat_tools()).to_string().len()) as u64; // conservative fallback: at most one token per byte
        if tokens.saturating_add(input_bound).saturating_add(512) > cfg.tokens {
            stop = "token_budget".into();
            break;
        }
        let body = json!({"model":cfg.model,"messages":messages,"tools":chat_tools(),"stream":false,"temperature":0.1,"seed":98,"max_tokens":512});
        session.record("request", &Value::Null, &body)?;
        steps += 1;
        let reply = match completion(url.as_str(), &body, remaining.min(Duration::from_secs(120))) {
            Ok(v) => v,
            Err(e) => {
                errors += 1;
                final_error = Some(e);
                stop = "upstream_error".into();
                break;
            }
        };
        if reply.to_string().len() > 1_048_576 {
            stop = "upstream_error".into();
            errors += 1;
            final_error = Some("completion exceeds 1 MiB".into());
            break;
        }
        repair::check_value(&reply)?;
        if Instant::now() >= deadline {
            stop = "wall_budget".into();
            break;
        }
        let usage = reply["usage"]["total_tokens"]
            .as_u64()
            .unwrap_or(input_bound.saturating_add(reply.to_string().len() as u64));
        tokens = tokens.saturating_add(usage);
        session.record("response", &Value::Null, &reply)?;
        if tokens > cfg.tokens {
            stop = "token_budget".into();
            break;
        }
        let message = &reply["choices"][0]["message"];
        let parsed = if reply["choices"].as_array().is_some_and(|v| v.len() == 1) {
            parse_message(message, cfg.plain)
        } else {
            Err("exactly one completion choice required".into())
        };
        let mut group = vec![current];
        let action_key = match &parsed {
            Ok(call) => format!("{call:?}"),
            Err(e) => format!("format: {e}"),
        };
        let result = match parsed {
            Ok(Call::Done(answer)) => {
                done = answer;
                stop = "done".into();
                break;
            }
            Ok(Call::Tool { name, args }) => {
                if !cfg.plain {
                    if let Some(original) =
                        message["tool_calls"][0]["function"]["arguments"].as_str()
                    {
                        if serde_json::from_str::<Value>(original).ok().as_ref() != Some(&args) {
                            session.record("syntax", &json!(original), &args)?;
                        }
                    }
                }
                actions += 1;
                if message["tool_calls"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty())
                {
                    native += 1;
                } else {
                    text_calls += 1;
                }
                if !cfg.plain
                    && message["tool_calls"]
                        .as_array()
                        .is_none_or(|c| c.is_empty())
                {
                    session.record("text_call", message, &json!({"name":name,"arguments":args}))?;
                }
                let result = session.call(&name, &args);
                // Keep native tool history valid; text calls remain assistant text.
                group.push(message.clone());
                if let Some(id) = message["tool_calls"][0]["id"].as_str() {
                    group.push(
                        json!({"role":"tool","tool_call_id":id,"content":result.to_string()}),
                    );
                } else {
                    group.push(json!({"role":"user","content":result.to_string()}));
                }
                result
            }
            Err(e) => {
                group.push(
                    json!({"role":"assistant","content":message["content"].as_str().unwrap_or("")}),
                );
                json!({"isError":true,"formatError":e})
            }
        };
        let is_error = result["isError"] == true;
        session.record("transcript", message, &result)?;
        // Compare actions/errors and semantic state, never commentary, observation, or clock.
        let key = json!([
            action_key,
            is_error,
            result["structuredContent"]["error"],
            session.stable_state()
        ])
        .to_string();
        let count = match &previous {
            Some((old, n)) if old == &key => n + 1,
            _ => 1,
        };
        previous = Some((key, count));
        feedback = result.to_string();
        if count >= 3 || feedback.contains("three identical") {
            stop = "repeated_result".into();
            errors += u64::from(is_error);
            break;
        }
        if feedback.contains("audit failed") || feedback.contains("cannot audit") {
            stop = "audit_error".into();
            break;
        }
        if feedback.contains("budget exhausted") {
            stop = "remote_budget".into();
            break;
        }
        if is_error {
            errors += 1;
            consecutive_errors += 1;
            let empty = message["content"].as_str().unwrap_or("").trim().is_empty()
                && message["tool_calls"]
                    .as_array()
                    .is_none_or(|c| c.is_empty());
            if empty {
                if empty_retry {
                    stop = "empty_reply".into();
                    break;
                }
                empty_retry = true;
                retries += 1;
            }
            if consecutive_errors > 2 {
                stop = "format_or_action_errors".into();
                break;
            }
            hand_backs += 1;
            if !cfg.plain {
                let help = guidance::help(&cfg.task);
                session.record("help", &Value::Null, &help)?;
                feedback.push_str(&format!("\nPilot help: {help}"));
                session.record("hand_back", &result, &json!(feedback))?;
            }
        } else {
            consecutive_errors = 0;
            empty_retry = false;
        }
        history.push(group);
        while history.len() > 8 || json!(history).to_string().len() > 1_048_576 {
            if history.is_empty() {
                break;
            }
            history.remove(0);
        }
    }
    let final_screen = match session.final_look() {
        Ok(text) => text,
        Err(e) => {
            errors += 1;
            final_error = Some(format!("final look failed: {e}"));
            String::new()
        }
    };
    let summary = json!({"task":cfg.task,"model":cfg.model,"plain":cfg.plain,"steps":steps,"actions":actions,"errors":errors,"rewrites":session.rewrite_count(),"tokens":tokens,"latency":started.elapsed().as_secs_f64(),"stop_reason":stop,"done_message":done,"final_screen_text":final_screen,"error":final_error,"native":native,"text":text_calls,"retries":retries,"hand_backs":hand_backs,"repairs":session.repair_count()});
    session.record("summary", &Value::Null, &summary)?;
    Ok(summary)
}
