//! Local-only shortcut wire handling; no local tool id survives to a client.
use super::*;
fn named(call: &Value) -> bool {
    matches!(
        call["function"]["name"].as_str(),
        Some("pilot_build" | "pilot_find" | "pilot_fill")
    )
}
pub(super) fn strip(body: &mut Value) {
    if named(&body["tool_choice"]) {
        if let Some(o) = body.as_object_mut() {
            o.remove("tool_choice");
        }
    }
    if let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) {
        tools.retain(|t| !named(t));
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
        !(m["role"] == "tool"
            && (matches!(
                m["name"].as_str(),
                Some("pilot_build" | "pilot_find" | "pilot_fill")
            ) || ids.contains(&m["tool_call_id"])))
    });
    for m in messages {
        if m["role"] == "system" {
            if let Some(text) = m["content"].as_str().filter(|s| {
                s.contains("Text tool mode.")
                    && s.lines().any(|line| {
                        line.starts_with("pilot_build:") || line.starts_with("pilot_find:")
                    })
            }) {
                m["content"] = json!(text
                    .lines()
                    .filter(|line| !line.starts_with("pilot_build:")
                        && !line.starts_with("pilot_find:"))
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
pub(super) fn supported(body: &Value) -> bool {
    body["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|t| {
            t["function"]["name"] == "execute"
                && t["function"]["parameters"]["properties"]
                    .get("command")
                    .is_some()
        })
    })
}
pub(super) fn offer(body: &mut Value) -> Result<bool, String> {
    if !supported(body) {
        return Ok(false);
    }
    let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    if tools.len() > 126 {
        return Err("cannot inject recipe tools: tools would exceed 128 entries".into());
    }
    tools.push(crate::recipe_library::build_tool());
    tools.push(crate::recipe_library::find_tool());
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
    library: &crate::recipe_library::Library,
    offered: bool,
) -> Result<(Value, planner::Plan), String> {
    if !offered {
        return Err(
            "pilot_build offered false permits 0 calls this turn; use the offered tools".into(),
        );
    }
    let choices = response["choices"]
        .as_array()
        .filter(|cs| cs.len() == 1)
        .ok_or_else(|| {
            format!(
                "pilot_build choices {} requires exactly 1 completion; submit one completion",
                response["choices"]
            )
        })?;
    let calls = choices[0]["message"]["tool_calls"]
        .as_array()
        .filter(|cs| cs.len() == 1 && cs[0]["function"]["name"] == "pilot_build")
        .ok_or_else(|| format!("pilot_build tool_calls {} requires exactly 1 pilot_build call; submit it without other tools", choices[0]["message"]["tool_calls"]))?;
    let args = &calls[0]["function"]["arguments"];
    let args = if let Some(s) = args.as_str() {
        serde_json::from_str(s).map_err(|e| {
            format!(
                "arguments {s:?} requires 1 valid JSON object ({e}); fix JSON quoting and brackets"
            )
        })?
    } else {
        args.clone()
    };
    let plan = if args.get("recipe").is_some() {
        library.build(&args, request, catalog)?
    } else {
        planner::shortcut(&args, request, catalog)?
    };
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
        let name = call["function"]["name"].clone();
        messages.push(json!({"role":"assistant","content":null,"tool_calls":[call]}));
        messages.push(json!({"role":"tool","tool_call_id":id,"name":name,"content":json!({"error":reason}).to_string()}));
    }
    Ok(())
}
pub(super) fn contains_find(response: &Value) -> bool {
    response["choices"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["message"]["tool_calls"].as_array())
        .flatten()
        .any(|c| c["function"]["name"] == "pilot_find")
}
pub(super) fn find(
    body: &mut Value,
    response: &Value,
    library: &crate::recipe_library::Library,
    audit: &mut Audit,
) -> Result<(), String> {
    let choices = response["choices"]
        .as_array()
        .filter(|cs| cs.len() == 1)
        .ok_or("pilot_find requires one choice")?;
    let calls = choices[0]["message"]["tool_calls"]
        .as_array()
        .filter(|cs| cs.len() == 1 && cs[0]["function"]["name"] == "pilot_find")
        .ok_or("pilot_find requires exactly one local search call")?;
    let mut call = calls[0].clone();
    let raw = &call["function"]["arguments"];
    let args: Value = if let Some(s) = raw.as_str() {
        serde_json::from_str(s).map_err(|e| format!("pilot_find invalid JSON: {e}"))?
    } else {
        raw.clone()
    };
    crate::recipe_library::validate(
        &args,
        &crate::recipe_library::find_tool()["function"]["parameters"],
    )?;
    let query = args["request"].as_str().ok_or("request must be a string")?;
    let result = library.find(query, &crate::recipe_library::Bm25)?;
    let top: Vec<_> = result["recipes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| json!({"id":r["id"],"score":r["score"]}))
        .collect();
    audit.record(
        "find",
        &json!({"query":query}),
        &json!({"query":query,"top":top}),
    )?;
    audit.record(
        if result["hit"] == true {
            "find_hit"
        } else {
            "find_miss"
        },
        &json!({"query":query}),
        &json!({"top":top}),
    )?;
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    if messages.len() > 254 {
        return Err("search history exceeds 256 messages".into());
    }
    let id = format!("pilot_find_{}", messages.len());
    call["id"] = json!(id);
    call["function"]["arguments"] = json!(args.to_string());
    messages.push(json!({"role":"assistant","content":null,"tool_calls":[call]}));
    messages.push(
        json!({"role":"tool","tool_call_id":id,"name":"pilot_find","content":result.to_string()}),
    );
    Ok(())
}
