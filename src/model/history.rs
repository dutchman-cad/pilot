//! Upstream-only history reduction and character accounting. Local readers keep the original.
use super::{Audit, Profile};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn chars(value: &Value) -> usize {
    value
        .as_str()
        .map_or_else(|| value.to_string().chars().count(), |s| s.chars().count())
}

fn calls(body: &Value) -> BTreeMap<String, (String, String)> {
    let mut result = BTreeMap::new();
    for message in body["messages"].as_array().into_iter().flatten() {
        for call in message["tool_calls"].as_array().into_iter().flatten() {
            if let (Some(id), Some(name)) = (call["id"].as_str(), call["function"]["name"].as_str())
            {
                let args = &call["function"]["arguments"];
                let parsed = args
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                result.insert(
                    id.into(),
                    (name.into(), parsed.as_ref().unwrap_or(args).to_string()),
                );
            }
        }
    }
    result
}

fn note(n: usize) -> String {
    format!("[trimmed by Pilot: {n} chars; call the tool again for the full result]")
}

// Preserve object keys and ordinary scalar values; bound long strings and nesting.
fn compact(value: &Value, depth: usize) -> Value {
    match value {
        Value::String(text) if text.chars().count() > 256 => json!(format!(
            "{} [string trimmed] {}",
            text.chars().take(192).collect::<String>(),
            text.chars()
                .rev()
                .take(32)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
        )),
        Value::Array(items) => {
            let len = items.len();
            Value::Array(
                items
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i < 2 || *i >= len.saturating_sub(2))
                    .map(|(_, v)| {
                        if depth < 12 {
                            compact(v, depth + 1)
                        } else {
                            json!("[nested value omitted]")
                        }
                    })
                    .collect(),
            )
        }
        Value::Object(map) if depth < 12 => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let parsed = (k == "text")
                        .then(|| {
                            v.as_str()
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        })
                        .flatten();
                    let child = parsed.map_or_else(
                        || compact(v, depth + 1),
                        |p| json!(compact(&p, depth + 1).to_string()),
                    );
                    (k.clone(), child)
                })
                .collect(),
        ),
        Value::Object(map) => json!({"keys": map.keys().collect::<Vec<_>>()}),
        _ => value.clone(),
    }
}

fn command_ids(value: &Value, ids: &mut BTreeSet<String>, depth: usize) {
    if depth > 12 {
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(id) = map
                .get("id")
                .or_else(|| map.get("command"))
                .and_then(Value::as_str)
            {
                ids.insert(id.into());
            }
            for (key, child) in map {
                if key == "text" {
                    if let Some(parsed) = child
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    {
                        command_ids(&parsed, ids, depth + 1);
                    }
                } else if matches!(
                    key.as_str(),
                    "commands"
                        | "command"
                        | "ids"
                        | "items"
                        | "result"
                        | "data"
                        | "content"
                        | "structuredContent"
                ) {
                    command_ids(child, ids, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Some(id) = item.as_str() {
                    ids.insert(id.into());
                } else {
                    command_ids(item, ids, depth + 1);
                }
            }
        }
        _ => {}
    }
}

/// Keep tool envelopes so native tool-call/result pairing remains valid.
pub(super) fn trim(body: &mut Value, cap: usize, protected: bool) -> usize {
    if protected {
        return 0;
    }
    let calls = calls(body);
    let Some(messages) = body["messages"].as_array_mut() else {
        return 0;
    };
    let Some(latest_user) = messages.iter().rposition(|m| m["role"] == "user") else {
        return 0;
    };
    let mut seen = BTreeSet::new();
    let mut changed = 0;
    for (index, message) in messages.iter_mut().enumerate().rev() {
        if message["role"] != "tool" {
            continue;
        }
        let identity = message["tool_call_id"]
            .as_str()
            .and_then(|id| calls.get(id));
        let name = identity.map_or_else(
            || message["name"].as_str().unwrap_or("unknown"),
            |(name, _)| name.as_str(),
        );
        let content = &message["content"];
        // Unknown arguments are insufficient evidence for deduplication.
        let duplicate = identity.is_some_and(|(name, args)| {
            !seen.insert((name.clone(), args.clone(), content.to_string()))
        });
        if index > latest_user {
            continue;
        }
        let n = chars(content);
        let parsed = content
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .or_else(|| (content.is_object() || content.is_array()).then(|| content.clone()));
        let catalogue = matches!(name, "list_commands" | "describe_command");
        let summary = if duplicate {
            Some(format!(
                "Identical result retained at its latest occurrence.\n{}",
                note(n)
            ))
        } else if catalogue {
            parsed.as_ref().and_then(|value| {
                let mut ids = BTreeSet::new();
                command_ids(value, &mut ids, 0);
                (!ids.is_empty()).then(|| format!("{}\n{}", json!({"ids":ids}), note(n)))
            })
        } else if n > cap {
            Some(match parsed {
                Some(value) => format!("{}\n{}", compact(&value, 0), note(n)),
                None => format!(
                    "{}\n{}",
                    content
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(cap.saturating_sub(100))
                        .collect::<String>(),
                    note(n)
                ),
            })
        } else {
            None
        };
        if let Some(summary) = summary.filter(|s| duplicate || catalogue || s.chars().count() < n) {
            message["content"] = json!(summary);
            changed += 1;
        }
    }
    changed
}

fn breakdown(body: &Value) -> BTreeMap<String, u64> {
    let calls = calls(body);
    let mut sizes = BTreeMap::new();
    for key in ["system", "tools", "user", "assistant", "tool_results"] {
        sizes.insert(key.into(), 0);
    }
    sizes.insert(
        "tools".into(),
        body.get("tools").map_or(0, |v| chars(v) as u64),
    );
    for message in body["messages"].as_array().into_iter().flatten() {
        let role = message["role"].as_str().unwrap_or("other");
        let size = message
            .get("content")
            .filter(|v| !v.is_null())
            .map_or(0, chars)
            + message.get("tool_calls").map_or(0, chars);
        if role == "tool" {
            *sizes.entry("tool_results".into()).or_default() += size as u64;
            let name = message["tool_call_id"]
                .as_str()
                .and_then(|id| calls.get(id))
                .map_or_else(
                    || message["name"].as_str().unwrap_or("unknown"),
                    |(name, _)| name.as_str(),
                );
            *sizes.entry(format!("tool_results.{name}")).or_default() += size as u64;
        } else {
            *sizes.entry(role.into()).or_default() += size as u64;
        }
    }
    sizes.insert("total".into(), chars(body) as u64);
    sizes
}

fn additions(client: &Value, body: &Value) -> BTreeMap<String, u64> {
    let mut sizes = BTreeMap::new();
    let texts = |v: &Value| {
        v["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| crate::prompting::message_text(&m["content"]))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let before = texts(client);
    let after = texts(body);
    let mut facts = 0;
    for (name, fact) in [
        ("sketch_fact", crate::prompting::SKETCH_FACT),
        ("placement_fact", crate::prompting::PLACEMENT_FACT),
    ] {
        let size = after
            .matches(fact)
            .count()
            .saturating_sub(before.matches(fact).count())
            * fact.chars().count();
        sizes.insert(name.into(), size as u64);
        facts += size;
    }
    sizes.insert("facts".into(), facts as u64);
    let hints = |body: &Value| {
        body["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| {
                let text = crate::prompting::message_text(&m["content"]);
                [text.find(crate::identity::HINT), text.find("HINT:")]
                    .into_iter()
                    .flatten()
                    .min()
                    .map_or(0, |start| text[start..].chars().count())
            })
            .sum::<usize>()
    };
    sizes.insert(
        "hints".into(),
        hints(body).saturating_sub(hints(client)) as u64,
    );
    for name in ["how", "pilot_build", "pilot_find"] {
        let size = |v: &Value| {
            v["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|t| t["function"]["name"] == name)
                .map(chars)
                .sum::<usize>()
        };
        sizes.insert(
            format!("{name}_schema"),
            size(body).saturating_sub(size(client)) as u64,
        );
    }
    sizes
}

pub(super) fn audit_request(
    client: &Value,
    augmented: &Value,
    wire: &Value,
    enabled: bool,
    changed: usize,
    profile: &mut Profile,
    audit: &mut Audit,
) -> Result<(), String> {
    let client_sizes = breakdown(client);
    let upstream = breakdown(wire);
    let pilot = additions(client, augmented);
    if changed > 0 {
        audit.record(
            "history_trim",
            &json!({"characters":breakdown(augmented)["tool_results"]}),
            &json!({"results":changed,"characters":upstream["tool_results"]}),
        )?;
    }
    let details = json!({"trim_enabled":enabled,"trimmed_results":changed,"client":client_sizes,"pilot":pilot,"upstream":upstream});
    audit.record_context(
        "request_size",
        &Value::Null,
        &details,
        &json!({"dutchman-pilot":details}),
    )?;
    profile.trim_enabled = enabled;
    profile.upstream_requests = profile.upstream_requests.saturating_add(1);
    for (prefix, sizes) in [
        ("client", client_sizes),
        ("pilot", pilot),
        ("upstream", upstream),
    ] {
        for (key, value) in sizes {
            let key = format!("{prefix}.{key}");
            // At most 512 profile parts over the lifetime of a model. A changing
            // client tool vocabulary cannot grow the persistent profile indefinitely.
            if !profile.request_chars_total.contains_key(&key)
                && profile.request_chars_total.len() >= 512
            {
                continue;
            }
            let total = profile.request_chars_total.entry(key.clone()).or_default();
            *total = total.saturating_add(value);
            let max = profile.request_chars_max.entry(key).or_default();
            *max = (*max).max(value);
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(super) fn replay_fixture() -> Vec<Value> {
    let fixture: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/history-replay.json")).unwrap();
    let rows = fixture["rows"].as_u64().unwrap();
    let mut body = json!({"model":"unit","messages":[{"role":"system","content":"Use tools for measured geometry. Ask before changes."}],"tools":[{"type":"function","function":{"name":"execute","parameters":{"type":"object"}}},{"type":"function","function":{"name":"inspect","parameters":{"type":"object"}}}]});
    let mut requests = Vec::new();
    for (i, turn) in fixture["turns"].as_array().unwrap().iter().enumerate() {
        let name = fixture["tools"][i].as_str().unwrap();
        let args = if name == "execute" {
            json!({"command":"model.edges","params":{"body":"Body1"}})
        } else {
            json!({})
        };
        let id = format!("history_{i}");
        let result = json!({"success":true,"total":rows,"items":(0..rows).map(|j| json!({"id":format!("item.{j}"),"label":"A detailed geometry or command listing row", "measure":j, "description":"Measured public tool data with enough detail to resemble topology listings and catalogue entries."})).collect::<Vec<_>>()});
        let messages = body["messages"].as_array_mut().unwrap();
        messages.push(json!({"role":"user","content":turn}));
        messages.push(json!({"role":"assistant","content":null,"tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]}));
        messages.push(json!({"role":"tool","tool_call_id":id,"content":result.to_string()}));
        requests.push(body.clone());
        body["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","content":"The requested listing is shown."}));
    }
    requests
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn conversation(name: &str, content: Value) -> Value {
        json!({"messages":[{"role":"system","content":"rules"},{"role":"user","content":"old turn"},
            {"role":"assistant","tool_calls":[{"id":"a","function":{"name":name,"arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"a","content":content},
            {"role":"user","content":"new turn"}]})
    }

    #[test]
    fn threshold_unicode_plaintext_and_current_turn() {
        let mut small = conversation("inspect", json!("é".repeat(1500)));
        let original = small.clone();
        assert_eq!(trim(&mut small, 1500, false), 0);
        assert_eq!(small, original);
        let mut large = conversation("inspect", json!("é".repeat(1501)));
        assert_eq!(trim(&mut large, 1500, false), 1);
        let content = large["messages"][3]["content"].as_str().unwrap();
        assert!(content.chars().count() <= 1500);
        assert!(content.contains(&note(1501)));
        // A different configured cap also applies.
        let mut capped = conversation("inspect", json!("é".repeat(1500)));
        assert_eq!(trim(&mut capped, 500, false), 1);
        assert!(chars(&capped["messages"][3]["content"]) <= 500);
        let messages = large["messages"].as_array_mut().unwrap();
        messages.push(json!({"role":"assistant","tool_calls":[{"id":"b","function":{"name":"inspect","arguments":"{}"}}]}));
        let current = json!({"role":"tool","tool_call_id":"b","content":"x".repeat(5000)});
        messages.push(current.clone());
        trim(&mut large, 1500, false);
        assert_eq!(large["messages"][6], current);
    }

    #[test]
    fn json_keeps_keys_scalars_and_array_ends_including_mcp_text() {
        let data = json!({"ok":true,"count":100,"name":"Body1","nothing":null,"items":(0..100).collect::<Vec<_>>()});
        for content in [
            json!(data.to_string()),
            json!({"content":[{"type":"text","text":data.to_string()}]})
                .to_string()
                .into(),
        ] {
            let mut body = conversation("inspect", content);
            assert_eq!(trim(&mut body, 256, false), 1);
            let summary = body["messages"][3]["content"].as_str().unwrap();
            let mut parsed: Value = serde_json::from_str(summary.lines().next().unwrap()).unwrap();
            if parsed.get("content").is_some() {
                parsed =
                    serde_json::from_str(parsed["content"][0]["text"].as_str().unwrap()).unwrap();
            }
            assert_eq!(parsed["items"], json!([0, 1, 98, 99]));
            for key in ["ok", "count", "name", "nothing"] {
                assert_eq!(parsed[key], data[key]);
            }
            assert!(summary.contains("call the tool again"));
        }
    }

    #[test]
    fn duplicate_identity_includes_tool_arguments_and_content() {
        let mut body = conversation("inspect", json!("x".repeat(800)));
        // Add a duplicate in the latest user turn. Only the old copy changes.
        body["messages"].as_array_mut().unwrap().extend([
            json!({"role":"assistant","tool_calls":[{"id":"b","function":{"name":"inspect","arguments":"{ }"}}]}),
            json!({"role":"tool","tool_call_id":"b","content":"x".repeat(800)})]);
        for difference in ["none", "tool", "arguments", "content"] {
            let mut trial = body.clone();
            match difference {
                "tool" => {
                    trial["messages"][5]["tool_calls"][0]["function"]["name"] = json!("execute")
                }
                "arguments" => {
                    trial["messages"][5]["tool_calls"][0]["function"]["arguments"] =
                        json!("{\"body\":2}")
                }
                "content" => trial["messages"][6]["content"] = json!("y".repeat(800)),
                _ => {}
            }
            assert_eq!(
                trim(&mut trial, 1500, false),
                usize::from(difference == "none")
            );
            assert_eq!(
                trial["messages"][6],
                if difference == "content" {
                    json!({"role":"tool","tool_call_id":"b","content":"y".repeat(800)})
                } else {
                    body["messages"][6].clone()
                }
            );
        }
    }

    #[test]
    fn catalogue_and_descriptions_collapse_to_ids() {
        for name in ["list_commands", "describe_command"] {
            let mut body = conversation(name, json!(json!({"commands":[{"id":"solid.box","description":"details".repeat(300)},{"id":"solid.sphere","params":{"type":"object","properties":{"plane":{"enum":["XY","XZ","YZ"]}}}}]}).to_string()));
            assert_eq!(trim(&mut body, 1500, false), 1);
            let summary = body["messages"][3]["content"].as_str().unwrap();
            let ids: Value = serde_json::from_str(summary.lines().next().unwrap()).unwrap();
            assert_eq!(ids, json!({"ids":["solid.box","solid.sphere"]}));
            assert!(!summary.contains("details"));
        }
    }

    #[test]
    fn pending_readers_preserve_all_history() {
        let mut body = conversation("execute", json!("full geometry".repeat(1000)));
        let original = body.clone();
        assert_eq!(trim(&mut body, 1500, true), 0);
        assert_eq!(body, original);
    }
}
