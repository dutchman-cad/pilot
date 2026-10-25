//! Wire-only adaptation for chat templates without native tool support.
use crate::audit::Audit;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAX_PROMPT: usize = 32_768;
const MAX_HISTORY_TEXT: usize = 16_384;

fn bounded(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = text[..end].to_string();
    if end < text.len() {
        result.push_str(" [truncated]");
    }
    result
}

// Deliberately narrow: only Ollama's explicit unsupported-tools phrase, at HTTP 400.
// Do not interpret generic schema, tool_choice, authentication or model errors as this.
pub(super) fn rejected(status: u16, body: &[u8]) -> bool {
    status == 400
        && String::from_utf8_lossy(body)
            .to_ascii_lowercase()
            .contains("does not support tools")
}

fn schema(schema: &Value, depth: usize) -> String {
    if depth > 0 {
        if let Some(variants) = schema["oneOf"].as_array() {
            return variants
                .iter()
                .take(8)
                .map(|v| self::schema(v, depth))
                .collect::<Vec<_>>()
                .join(" | ");
        }
        if let Some(value) = schema.get("const") {
            return value.to_string();
        }
        if let Some(values) = schema.get("enum") {
            return values.to_string();
        }
    }
    let kind = schema["type"].as_str().unwrap_or("any");
    if depth == 0 {
        return kind.to_string();
    }
    if kind == "array" {
        return format!("array<{}>", self::schema(&schema["items"], depth - 1));
    }
    if let Some(properties) = schema["properties"].as_object() {
        let fields = properties
            .iter()
            .take(64)
            .map(|(name, value)| {
                format!("{}:{}", bounded(name, 128), self::schema(value, depth - 1))
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("object{{{fields}}}");
    }
    kind.to_string()
}

pub(super) fn activate(
    body: &mut Value,
    tools: &[Value],
    rule: &str,
    audit: &mut Audit,
) -> Result<(), String> {
    let mut prompt = String::from("Text tool mode. To act, reply with exactly one JSON object {\"tool\": NAME, \"args\": {...}}. When no action is needed, reply in plain text. Available tools:\n");
    for tool in tools {
        let function = &tool["function"];
        let purpose = function["description"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .take(32)
            .collect::<Vec<_>>()
            .join(" ");
        prompt.push_str(&format!(
            "{}: {}; parameters {}\n",
            function["name"].as_str().unwrap_or(""),
            bounded(&purpose, 160),
            schema(&function["parameters"], 3)
        ));
        if prompt.len() > MAX_PROMPT {
            return Err("text tool descriptions exceed 32768 bytes".into());
        }
    }
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    // One system message: some chat templates reject or misplace a second one.
    match messages.first_mut() {
        Some(first) if first["role"] == "system" && first["content"].is_string() => {
            let existing = first["content"].as_str().unwrap_or("");
            first["content"] = json!(format!("{existing}\n\n{prompt}"));
        }
        _ => messages.insert(0, json!({"role":"system","content":prompt})),
    }
    audit.record(
        "text_tool_mode",
        &Value::Null,
        &json!({"rule":rule,"prompt":prompt}),
    )
}

/// Keep the working request's native history for recipes/guards; adapt a wire clone.
pub(super) fn wire(body: &Value, audit: &mut Audit) -> Result<Value, String> {
    let mut wire = body.clone();
    let object = wire.as_object_mut().ok_or("request must be an object")?;
    let mut removed = serde_json::Map::new();
    for key in ["tools", "tool_choice", "parallel_tool_calls"] {
        if let Some(value) = object.remove(key) {
            removed.insert(key.into(), value);
        }
    }
    if !removed.is_empty() {
        audit.record("tools_stripped", &Value::Object(removed), &Value::Null)?;
    }
    let messages = wire["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let mut names = BTreeMap::new();
    for message in messages {
        let before = message.clone();
        if message["role"] == "assistant" && message.get("tool_calls").is_some() {
            let mut text = match &message["content"] {
                Value::Null => String::new(),
                Value::String(text) => text.clone(),
                content => content.to_string(),
            };
            if let Some(calls) = message["tool_calls"].as_array() {
                for call in calls.iter().take(64) {
                    let name = call["function"]["name"].as_str().unwrap_or("unknown");
                    if let Some(id) = call["id"].as_str() {
                        names.insert(id.to_string(), name.to_string());
                    }
                    // Echo past calls in the reply format the model is asked to use.
                    let args = call["function"]["arguments"]
                        .as_str()
                        .and_then(|a| serde_json::from_str::<Value>(a).ok())
                        .unwrap_or_else(|| call["function"]["arguments"].clone());
                    text.push_str(&format!("\n{}", json!({"tool":name,"args":args})));
                }
            }
            *message = json!({"role":"assistant","content":bounded(&text, MAX_HISTORY_TEXT)});
        } else if message["role"] == "tool" {
            let name = message["tool_call_id"]
                .as_str()
                .and_then(|id| names.get(id).map(String::as_str))
                .or_else(|| message["name"].as_str())
                .unwrap_or("unknown");
            let content = message["content"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| message["content"].to_string());
            *message = json!({"role":"user","content":bounded(&format!("Tool {name} returned: {content}"), MAX_HISTORY_TEXT)});
        }
        if *message != before {
            audit.record("tool_history_flattened", &before, message)?;
        }
    }
    Ok(wire)
}
