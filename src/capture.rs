//! Bounded, memory-only diagnostic turns. Never retain tool payloads or prompts.
use crate::{catalog::Catalog, planner};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;

pub const MAX_TURNS: usize = 5;
pub const MAX_EVENTS: usize = 256;
pub const MAX_TEXT: usize = 16_384;
pub fn bounded(text: &str) -> String {
    let mut end = text.len().min(MAX_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: String,
    pub facts: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub id: u64,
    pub request: String,
    pub request_bytes: usize,
    pub model: String,
    pub version: String,
    pub truncated_events: usize,
    pub flags: Value,
    pub shape: Value,
    pub events: Vec<Event>,
    pub names: Vec<(String, String)>,
    pub failed: bool,
}
#[derive(Default)]
pub struct Ring {
    turns: VecDeque<Turn>,
    next: u64,
}
impl Ring {
    pub fn push(&mut self, mut turn: Turn) {
        self.next = self.next.saturating_add(1);
        turn.id = self.next;
        if self.turns.len() == MAX_TURNS {
            self.turns.pop_front();
        }
        self.turns.push_back(turn);
    }
    pub fn turns(&self) -> &VecDeque<Turn> {
        &self.turns
    }
}
impl Turn {
    pub fn from_request(body: &Value, catalog: &Catalog, flags: Value) -> Result<Self, String> {
        crate::repair::check_value(body)?;
        let request = body["messages"]
            .as_array()
            .and_then(|m| m.iter().rev().find(|m| m["role"] == "user"))
            .and_then(|m| m["content"].as_str())
            .unwrap_or("");
        let planning_enabled = flags["planner"] != false;
        let replay = if !planning_enabled {
            Ok(None)
        } else if flags["direct_recipes"] == true {
            planner::plan_direct(body, catalog)
        } else {
            planner::plan(body, catalog)
        };
        let (plan, planning_error) = match replay {
            Ok(plan) => (plan, None),
            Err(error) => (None, Some(bounded(&error))),
        };
        let mut names = Vec::new();
        collect_names(body, "", &mut names, 0);
        let (decline_reason, decline_tokens) = planner::report_decline(request);
        let mut shape = match &plan {
            Some(p) => {
                collect_names(&p.slots, "", &mut names, 0);
                json!({"intent":p.rule,"slots":p.slots,"length_bytes":request.len()})
            }
            None => {
                json!({"intent":"unmatched","slots":{},"length_bytes":request.len(),"decline_tokens":decline_tokens,"token_shape":planner::report_tokens(request)})
            }
        };
        if plan.is_none() && flags["planner"] != false {
            if let Ok(Some((partial, _))) = planner::partial_plan(body, catalog) {
                shape["partial_intent"] = json!(partial.rule);
                shape["slots"] = partial.slots;
                collect_names(&shape["slots"], "", &mut names, 0);
            }
        }
        let mut turn = Self {
            id: 0,
            request: bounded(request),
            request_bytes: request.len(),
            model: bounded(body["model"].as_str().unwrap_or("unknown")),
            version: env!("CARGO_PKG_VERSION").into(),
            truncated_events: 0,
            flags,
            shape,
            events: Vec::new(),
            names,
            failed: planning_enabled && plan.is_none(),
        };
        turn.event(
            if !planning_enabled {"Planner disabled"} else if plan.is_some() {
                "Recipe matched"
            } else {
                "Recipe declined"
            },
            json!({"reason":if !planning_enabled {"disabled by flag"} else if plan.is_some() {"matched recipe"} else {decline_reason}}),
        );
        if let Some(plan) = &plan {
            turn.event("plan decision",json!({"rule":plan.rule,"commands":commands(&json!(plan.calls)),"tools":plan.calls.iter().take(64).filter_map(|c|c["name"].as_str()).map(bounded).collect::<Vec<_>>(),"executed":false}));
        }
        if let Some(error) = planning_error {
            turn.event("planner error", json!({"error":error}));
        }
        if let Some(messages) = body["messages"].as_array() {
            let user = messages
                .iter()
                .rposition(|m| m["role"] == "user")
                .unwrap_or(0);
            for message in messages[user..].iter().rev().take(256).rev() {
                if message["role"] == "tool" {
                    let content = message["content"]
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| message["content"].clone());
                    let facts = if let Some(text) = content.as_str().filter(|s| {
                        s.to_ascii_lowercase().contains("error")
                            || s.to_ascii_lowercase().contains("failed")
                    }) {
                        json!({"error":bounded(text)})
                    } else {
                        facts(&content)
                    };
                    if facts != json!({}) {
                        turn.event("tool result", facts);
                    }
                }
                if let Some(calls) = message["tool_calls"].as_array() {
                    for call in calls.iter().take(64) {
                        turn.call(call);
                    }
                }
            }
        }
        Ok(turn)
    }
    pub fn event(&mut self, kind: &str, facts: Value) {
        if kind.contains("error")
            || kind.contains("fallback")
            || kind.contains("declin")
            || facts.to_string().contains("\"error\"")
        {
            self.failed = true;
        }
        if self.events.len() < MAX_EVENTS {
            self.events.push(Event {
                kind: bounded(kind),
                facts: small_facts(facts),
            });
        } else {
            self.truncated_events = self.truncated_events.saturating_add(1);
        }
    }
    pub fn call(&mut self, call: &Value) {
        let args = call["function"]["arguments"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or(Value::Null);
        self.event("tool call", json!({"tool":bounded(call["function"]["name"].as_str().unwrap_or("unknown")),"commands":commands(&args)}));
    }
    pub fn finish(&mut self, response: &Value, status: u16) {
        self.failed |= status >= 400;
        if let Some(choices) = response["choices"].as_array() {
            for choice in choices.iter().take(8) {
                if self.events.len() == MAX_EVENTS {
                    self.events.pop();
                    self.truncated_events = self.truncated_events.saturating_add(1);
                }
                self.event("finish", json!({"finish_reason":choice["finish_reason"]}));
                if let Some(calls) = choice["message"]["tool_calls"].as_array() {
                    for call in calls.iter().take(64) {
                        self.call(call);
                    }
                }
            }
        }
        if let Some(error) = response.get("error") {
            self.failed = true;
            self.event("error", facts(&json!({"error":error})));
        }
    }
}
fn collect_names(value: &Value, context: &str, out: &mut Vec<(String, String)>, depth: usize) {
    if depth > 24 || out.len() >= 256 {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter().take(256) {
                if out.len() >= 256 {
                    break;
                }
                if context == "parameters"
                    && !map.contains_key("name")
                    && out.len() < 256
                    && !out.iter().any(|(n, _)| n == key)
                {
                    out.push((bounded(key), "Param".into()));
                }
                if matches!(key.as_str(), "tools" | "tool_calls" | "functions") {
                    continue;
                }
                let class = match key.as_str() {
                    "body" | "bodyName" | "body_name" | "newName" | "new_name" => "Body",
                    "component" => "Component",
                    "sketch" => "Sketch",
                    "parameter" | "param" => "Param",
                    "file" | "filename" => "File",
                    "name" => match context {
                        "parameters" | "parameter" | "param" => "Param",
                        "function" | "properties" => "",
                        "sketches" | "sketch" => "Sketch",
                        "components" | "component" => "Component",
                        "files" | "file" => "File",
                        _ => "Body",
                    },
                    _ => "",
                };
                if !class.is_empty() && out.len() < 256 {
                    if let Some(s) = value.as_str().filter(|s| !s.is_empty() && s.len() <= 256) {
                        if !out.iter().any(|(n, _)| n == s) {
                            out.push((s.into(), class.into()));
                        }
                    }
                }
                collect_names(
                    value,
                    if matches!(
                        key.as_str(),
                        "name" | "bodyName" | "body_name" | "newName" | "new_name"
                    ) {
                        context
                    } else {
                        key
                    },
                    out,
                    depth + 1,
                );
            }
        }
        Value::Array(a) => {
            for value in a.iter().take(256) {
                collect_names(value, context, out, depth + 1);
            }
        }
        Value::String(s) if s.len() <= crate::repair::MAX_BYTES => {
            let class = match context {
                "bodies" => "Body",
                "components" => "Component",
                "sketches" => "Sketch",
                "files" => "File",
                _ => "",
            };
            if !class.is_empty()
                && !s.is_empty()
                && s.len() <= 256
                && !out.iter().any(|(n, _)| n == s)
            {
                out.push((s.clone(), class.into()));
            }

            if let Ok(v) = serde_json::from_str::<Value>(s) {
                if v.is_object() || v.is_array() {
                    collect_names(&v, context, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}
pub fn commands(value: &Value) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>, depth: usize) {
        if depth > 16 || out.len() >= 64 {
            return;
        }
        match v {
            Value::Object(m) => {
                for (k, v) in m.iter().take(128) {
                    if k == "command" {
                        if let Some(s) = v.as_str() {
                            out.push(bounded(s));
                        }
                    } else {
                        walk(v, out, depth + 1);
                    }
                }
            }
            Value::Array(a) => {
                for v in a.iter().take(64) {
                    walk(v, out, depth + 1);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(value, &mut out, 0);
    out
}
/// Allowlist only measured facts and error text, never raw design JSON.
pub fn facts(value: &Value) -> Value {
    fn walk(v: &Value, out: &mut Vec<Value>, depth: usize, context: &str) {
        if depth > 24 || out.len() >= 64 {
            return;
        }
        match v {
            Value::Object(m) => {
                for (key, v) in m.iter().take(256) {
                    match key.as_str() {
                        "bodyCount" | "body_count" | "volume" | "volume_mm3"
                            if v.is_number()
                                || (v.is_array()
                                    && v.as_array().is_some_and(|a| {
                                        a.len() <= 3 && a.iter().all(Value::is_number)
                                    })) =>
                        {
                            out.push(json!({key:v}))
                        }
                        "bbox" | "boundingBox" | "bounding_box" => {
                            if let (Some(min), Some(max)) =
                                (v["min"].as_array(), v["max"].as_array())
                            {
                                if min.len() == 3 && max.len() == 3 {
                                    let size: Option<Vec<f64>> = min
                                        .iter()
                                        .zip(max)
                                        .map(|(a, b)| Some(b.as_f64()? - a.as_f64()?))
                                        .collect();
                                    if let Some(size) = size {
                                        out.push(json!({"bbox_size":size}));
                                    }
                                }
                            }
                        }
                        "type" | "featureType"
                            if v.is_string()
                                && (key == "featureType"
                                    || matches!(context, "features" | "feature")) =>
                        {
                            out.push(json!({"feature_type":bounded(v.as_str().unwrap_or(""))}))
                        }
                        "error" | "errors" => {
                            if let Some(s) = v.as_str().or_else(|| v["message"].as_str()) {
                                out.push(json!({"error":bounded(s)}));
                            } else {
                                walk(v, out, depth + 1, key);
                            }
                        }
                        "bodies" if v.is_array() => {
                            out.push(json!({"body_count":v.as_array().map_or(0,Vec::len)}));
                            if let Some(bodies) = v.as_array() {
                                for (index, body) in bodies.iter().take(64).enumerate() {
                                    if out.len() >= 64 {
                                        break;
                                    }
                                    out.push(json!({"body_index":index+1}));
                                    walk(body, out, depth + 1, "bodies");
                                    if out.len() >= 64 {
                                        break;
                                    }
                                }
                            }
                        }
                        "content" if v.is_array() => {
                            for item in v.as_array().into_iter().flatten().take(64) {
                                if out.len() >= 64 {
                                    break;
                                }
                                if let Some(s) = item["text"].as_str() {
                                    if let Ok(v) = serde_json::from_str::<Value>(s) {
                                        walk(&v, out, depth + 1, context);
                                    } else if value_is_error(m) {
                                        out.push(json!({"error":bounded(s)}));
                                    }
                                }
                            }
                        }
                        _ => walk(v, out, depth + 1, key),
                    }
                    if out.len() >= 64 {
                        break;
                    }
                }
            }
            Value::Array(a) => {
                for v in a.iter().take(64) {
                    walk(v, out, depth + 1, context);
                }
            }
            _ => {}
        }
    }
    fn value_is_error(m: &serde_json::Map<String, Value>) -> bool {
        m.get("isError") == Some(&Value::Bool(true))
    }
    let mut out = Vec::new();
    walk(value, &mut out, 0, "");
    if out.is_empty() {
        json!({})
    } else {
        json!({"facts":out})
    }
}
/// Audit data is reduced immediately; arbitrary before/after text is never retained.
pub fn audit_event(kind: &str, after: &Value) -> Event {
    let mut data = json!({"commands":commands(after)});
    for key in ["reason", "rule", "finish_reason", "code"] {
        if let Some(s) = after[key].as_str() {
            data[key] = json!(bounded(s));
        }
    }
    if let Some(s) = after
        .as_str()
        .filter(|_| kind.starts_with("hand_back") || kind.contains("fallback"))
    {
        data["reason"] = json!(bounded(s));
    }
    data["results"] = facts(after);
    if let Some(calls) = after["message"]["tool_calls"]
        .as_array()
        .or_else(|| after["tool_calls"].as_array())
    {
        data["tools"] = json!(calls
            .iter()
            .take(64)
            .filter_map(|c| c["function"]["name"].as_str())
            .map(bounded)
            .collect::<Vec<_>>());
    }
    if let Some(slots) = after.get("slots") {
        if slots.to_string().len() <= 1024 {
            data["slots"] = slots.clone();
        }
    }
    Event {
        kind: bounded(kind),
        facts: small_facts(data),
    }
}

/// Each retained event has at most 2 KiB of facts; oversized data is marked.
fn small_facts(value: Value) -> Value {
    if value.to_string().len() > 2048 {
        json!({"capture_truncated":true})
    } else {
        value
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn ring_bounds_and_selection() {
        let body = json!({"model":"test","messages":[{"role":"user","content":"create a cube 20mm"}],"tools":[{"function":{"name":"execute"}}]});
        let turn = Turn::from_request(&body, &Catalog::bundled().unwrap(), json!({})).unwrap();
        let mut ring = Ring::default();
        for _ in 0..9 {
            ring.push(turn.clone());
        }
        assert_eq!(ring.turns.len(), 5);
        assert_eq!(ring.turns[0].id, 5);
        assert_eq!(ring.turns[4].id, 9);
        let mut large = turn;
        for _ in 0..1000 {
            large.event("x", json!({"long":"x".repeat(3000)}));
        }
        assert_eq!(large.events.len(), 256);
        assert!(serde_json::to_vec(&large).unwrap().len() < 600_000);
    }
    #[test]
    fn fact_reduction() {
        let raw = json!({"bodies":[{"name":"secret","volume":8000,"bbox":{"min":[1,2,3],"max":[21,22,23]},"features":[{"type":"Extrude","name":"private"}]}],"design":{"vertices":["do not retain"],"expressions":"private design"},"error":{"message":"failed /tmp/private.step"}});
        let text = facts(&raw).to_string();
        assert!(text.contains("8000"));
        assert!(text.contains("bbox_size"));
        assert!(text.contains("Extrude"));
        assert!(text.contains("failed"));
        for private in ["vertices", "private design", "secret"] {
            assert!(!text.contains(private));
        }
    }
}
