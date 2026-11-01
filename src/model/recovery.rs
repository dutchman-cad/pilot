//! Bounded recovery and conversation guard state; no model output is executed here.
use super::*;
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub(super) struct Conversation {
    pub model: String,
    pub facts_seen: [bool; 3],
    pub sc98_dialog_fact_seen: bool,
    pub next_hints_seen: std::collections::BTreeSet<u64>,
    pub history: Vec<u64>,
    counts: BTreeMap<String, usize>,
    pub recipe: Option<(usize, planner::Expected)>,
    pub completion_checked: bool,
    pub direct: Option<(usize, planner::Plan, Value)>,
    pub direct_failed: bool,
    pub direct_index: usize,
    pub direct_handback: Option<String>,
    pub shortcut_args: Option<(usize, Value)>,
    pub pending_recipe: Option<planner::Plan>,
    pub sketch_hint_index: usize,
    pub sketch_hints_seen: [bool; 2],
    pub edge_hint_seen: bool,
    pub joint_hint_seen: bool,
    pub pattern_hint_seen: bool,
    pub plane_hints_seen: [bool; 2],
    pub pattern_inspections: usize,
    pub form_pending: Option<(usize, Value, Value, usize)>,
    pub body_hint: body_hint::State,
}
impl Conversation {
    pub fn allow(&mut self, command: &str, count: usize) -> bool {
        if !self.counts.contains_key(command) && self.counts.len() >= 128 {
            return false;
        }
        let used = self.counts.entry(command.to_string()).or_default();
        if count > 4usize.saturating_sub(*used) {
            return false;
        }
        *used += count;
        true
    }
}

pub(super) fn history(body: &Value) -> Result<Vec<u64>, String> {
    body["messages"]
        .as_array()
        .ok_or("messages must be an array")?
        .iter()
        .map(|m| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            json_bytes(m)?.hash(&mut hash);
            Ok(hash.finish())
        })
        .collect()
}

pub(super) fn previous_accepted(body: &Value) -> bool {
    let Some(messages) = body["messages"].as_array() else {
        return false;
    };
    // Only the latest assistant turn matters, including all of its tool results.
    let Some(start) = messages.iter().rposition(|m| m["role"] == "assistant") else {
        return false;
    };
    let calls = messages[start]["tool_calls"].as_array();
    messages[start + 1..].iter().any(|m| {
        m["role"] == "tool"
            && calls.is_some_and(|calls| {
                calls
                    .iter()
                    .any(|c| c["id"].is_string() && c["id"] == m["tool_call_id"])
            })
            && tool_data(m).is_some()
    })
}

pub(super) fn failing_command(message: &Value, reason: &str) -> String {
    let named = reason
        .strip_prefix("you described ")
        .or_else(|| reason.strip_prefix("unknown "))
        .unwrap_or(reason)
        .split_whitespace()
        .next()
        .unwrap_or("empty")
        .trim_end_matches(';');
    // A guard identifies the rejected batch step or alternative, which need not
    // be the first proposed call.
    if named.contains('.') {
        return named.chars().take(256).collect();
    }
    if let Some(calls) = message["tool_calls"].as_array() {
        for call in calls {
            if let Some(args) = call["function"]["arguments"]
                .as_str()
                .and_then(|s| repair::parse_json(s).ok())
            {
                if let Some(command) = args["command"].as_str() {
                    return split_command(command).0.chars().take(256).collect();
                }
            }
        }
    }
    // Guard messages start with the failing id, or "you described <id>".
    named.chars().take(256).collect()
}

pub(super) fn ollama_raw(upstream: &Upstream) -> Option<String> {
    if upstream.status != 500 {
        return None;
    }
    let body = std::str::from_utf8(&upstream.body).ok()?;
    let value = serde_json::from_str::<Value>(body).ok();
    let text = value
        .as_ref()
        .and_then(|v| {
            v["error"]
                .as_str()
                .or_else(|| v["error"]["message"].as_str())
        })
        .unwrap_or(body);
    let text = text.strip_prefix("error parsing tool call:")?;
    let Some((raw, _)) = text
        .trim()
        .strip_prefix("raw=")
        .and_then(|s| s.rsplit_once(", err="))
    else {
        return Some(String::new());
    };
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        Some(raw[1..raw.len().checked_sub(1)?].to_string())
    } else if raw.starts_with('"') {
        Some(serde_json::from_str::<String>(raw).unwrap_or_default())
    } else {
        Some(String::new())
    }
}

pub(super) fn recover(
    raw: &str,
    tools: &[Value],
    catalog: &Catalog,
    audit: &mut Audit,
    profile: &mut Profile,
) -> Result<Option<Value>, String> {
    // Extraction normally takes the first object. Recovery is stricter: ambiguous
    // arrays or multiple explicit calls must be retried rather than truncated.
    if !repair::single_call_text(raw) {
        return Ok(None);
    }
    let mut message = json!({"role":"assistant","content":raw});
    if !convert_text(&mut message, tools, catalog, audit)? {
        return Ok(None);
    }
    if repair_calls(&mut message, tools, catalog, "", audit, profile)?.is_some() {
        return Ok(None);
    }
    let Some(calls) = message["tool_calls"].as_array().filter(|c| c.len() == 1) else {
        return Ok(None);
    };
    let call = &calls[0];
    let Some(schema) = call["function"]["name"]
        .as_str()
        .and_then(|n| tool_schema(tools, n))
    else {
        return Ok(None);
    };
    let Some(args) = call["function"]["arguments"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    else {
        return Ok(None);
    };
    if !args.is_object() || !valid(&args, schema) {
        return Ok(None);
    }
    let name = call["function"]["name"].as_str().unwrap_or("");
    let valid_command = |step: &Value| {
        step["command"].as_str().is_some_and(|id| {
            catalog.get(id).is_some()
                && valid(
                    step.get("params").unwrap_or(&json!({})),
                    &catalog.parameter_schema(id),
                )
        })
    };
    if name == "execute" && !valid_command(&args)
        || name == "batch"
            && args["commands"].as_array().is_none_or(|steps| {
                steps.len() > repair::MAX_STEPS || !steps.iter().all(valid_command)
            })
    {
        return Ok(None);
    }
    profile.text_written = profile.text_written.saturating_add(1);
    Ok(Some(
        json!({"id":format!("{}{}", crate::identity::RECOVERED_PREFIX, now()),"object":"chat.completion","created":now(),"model":profile.model,"choices":[{"index":0,"message":message,"finish_reason":"tool_calls"}]}),
    ))
}

fn valid(value: &Value, schema: &Value) -> bool {
    if schema["enum"]
        .as_array()
        .is_some_and(|values| !values.contains(value))
        || schema
            .get("const")
            .is_some_and(|expected| expected != value)
    {
        return false;
    }
    let matches_type = schema["type"].as_array().map_or_else(
        || schema_type_matches(value, schema),
        |types| {
            types
                .iter()
                .any(|kind| schema_type_matches(value, &json!({"type":kind})))
        },
    );
    if schema.get("type").is_some() && !matches_type {
        return false;
    }
    if let Some(object) = value.as_object() {
        if schema["required"].as_array().is_some_and(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .any(|k| !object.contains_key(k))
        }) {
            return false;
        }
        if let Some(properties) = schema["properties"].as_object() {
            return object.iter().all(|(k, v)| {
                properties
                    .get(k)
                    .map(|s| valid(v, s))
                    .unwrap_or(schema["additionalProperties"] != false)
            });
        }
    }
    if let (Some(items), Some(schema)) = (value.as_array(), schema.get("items")) {
        return items.iter().all(|v| valid(v, schema));
    }
    true
}
