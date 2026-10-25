//! Local-only shortcut wire handling; no local tool id survives to a client.
use super::*;
fn named(call: &Value) -> bool {
    call["function"]["name"] == "pilot_build"
}
pub(super) fn strip(body: &mut Value) {
    if body["tool_choice"]["function"]["name"] == "pilot_build" {
        if let Some(o) = body.as_object_mut() {
            o.remove("tool_choice");
        }
    }
    if let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) {
        tools.retain(|t| t["function"]["name"] != "pilot_build");
    }
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };
    let ids: Vec<Value> = messages
        .iter()
        .filter_map(|m| m["tool_calls"].as_array())
        .flatten()
        .filter(|c| named(c))
        .map(|c| c["id"].clone())
        .collect();
    messages.retain(|m| {
        !(m["role"] == "tool" && (m["name"] == "pilot_build" || ids.contains(&m["tool_call_id"])))
    });
    for m in messages {
        if m["role"] == "system" {
            if let Some(text) = m["content"].as_str().filter(|s| {
                s.contains("Text tool mode.")
                    && s.lines().any(|line| line.starts_with("pilot_build:"))
            }) {
                m["content"] = json!(text
                    .lines()
                    .filter(|line| !line.starts_with("pilot_build:"))
                    .collect::<Vec<_>>()
                    .join("\n"));
            }
        }
        if let Some(calls) = m.get_mut("tool_calls").and_then(Value::as_array_mut) {
            let before = calls.len();
            calls.retain(|c| !named(c));
            if calls.len() != before && calls.is_empty() {
                if let Some(o) = m.as_object_mut() {
                    o.remove("tool_calls");
                }
            }
        }
    }
}
pub(super) fn offer(body: &mut Value) -> Result<bool, String> {
    let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    // The creation expanders require execute; the public SolidCraft tools identify this dialect.
    if !tools.iter().any(|t| {
        t["function"]["name"] == "execute"
            && t["function"]["parameters"]["properties"]
                .get("command")
                .is_some()
    }) {
        return Ok(false);
    }
    if tools.len() >= 128 {
        return Err("cannot inject pilot_build: tools would exceed 128 entries".into());
    }
    tools.push(planner::shortcut_tool());
    Ok(true)
}
pub(super) fn contains(response: &Value) -> bool {
    response["choices"].as_array().is_some_and(|choices| {
        choices.iter().any(|c| {
            let m = &c["message"];
            m["tool_calls"]
                .as_array()
                .is_some_and(|cs| cs.iter().any(named))
        })
    })
}
pub(super) fn expand(
    response: &Value,
    request: &Value,
    catalog: &Catalog,
    offered: bool,
) -> Result<(Value, planner::Plan), String> {
    if !offered {
        return Err("pilot_build: not offered for this turn".into());
    }
    let choices = response["choices"]
        .as_array()
        .filter(|cs| cs.len() == 1)
        .ok_or("pilot_build: requires one completion choice")?;
    let calls = choices[0]["message"]["tool_calls"]
        .as_array()
        .filter(|cs| cs.len() == 1 && named(&cs[0]))
        .ok_or("pilot_build: submit exactly one call without other tools")?;
    let args = &calls[0]["function"]["arguments"];
    let args = if let Some(s) = args.as_str() {
        serde_json::from_str(s).map_err(|e| format!("arguments: invalid JSON: {e}"))?
    } else {
        args.clone()
    };
    let plan = planner::shortcut(&args, request, catalog)?;
    Ok((args, plan))
}
pub(super) fn reject(body: &mut Value, response: &Value, reason: &str) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    // Only rejected local calls enter retry history; no other proposed changes have run.
    let calls: Vec<_> = response["choices"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["message"]["tool_calls"].as_array())
        .flatten()
        .filter(|c| named(c))
        .cloned()
        .collect();
    if messages.len() > 254 {
        return Err("shortcut retry history exceeds 256 messages".into());
    }
    for (i, mut call) in calls.into_iter().take(1).enumerate() {
        let id = format!("pilot_shortcut_rejected_{}_{}", messages.len(), i);
        call["id"] = json!(id);
        if !call["function"]["arguments"].is_string() {
            call["function"]["arguments"] = json!(call["function"]["arguments"].to_string());
        }
        messages.push(json!({"role":"assistant","content":null,"tool_calls":[call]}));
        messages.push(json!({"role":"tool","tool_call_id":id,"name":"pilot_build","content":json!({"error":reason}).to_string()}));
    }
    Ok(())
}
