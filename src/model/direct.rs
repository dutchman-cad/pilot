//! Exact recipe responses use the client's native tools and normal approval path.
use super::*;

pub(super) fn unavailable() -> String {
    format!("No model is configured. I can handle exact requests with every required dimension or slot for: {}. See docs/recipes.md for supported forms.", planner::supported_recipes().join(", "))
}
pub(super) fn reply(request: &Value, text: String, rewrites: usize) -> Result<Reply, String> {
    response(
        request,
        json!({"role":"assistant","content":text}),
        "stop",
        rewrites,
    )
}
fn response(
    request: &Value,
    message: Value,
    finish: &str,
    rewrites: usize,
) -> Result<Reply, String> {
    let value = json!({"id":format!("pilot_direct_{}", now()),"object":"chat.completion","created":now(),"model":"pilot-recipes","choices":[{"index":0,"message":message,"finish_reason":finish}]});
    let stream = request["stream"] == true;
    Ok(Reply {
        status: 200,
        content_type: if stream {
            "text/event-stream"
        } else {
            "application/json"
        }
        .into(),
        body: if stream {
            emit_sse(&value, request["stream_options"]["include_usage"] == true)?
        } else {
            json_bytes(&value)?
        },
        rewrites,
        retry_after: None,
    })
}
pub(super) fn calls(request: &Value, plan: &planner::Plan, index: usize) -> Value {
    let mut planned = plan.calls.clone();
    let batch = request["tools"]
        .as_array()
        .is_some_and(|t| t.iter().any(|t| t["function"]["name"] == "batch"));
    let inspect = planned
        .last()
        .is_some_and(|c| c["arguments"]["command"] == "document.inspect");
    // The Assistant truncates each tool message at 20,000 characters. Keep
    // topology reads separate: even two bodies can overflow a combined batch.
    let topology = planned.iter().any(|c| {
        matches!(
            c["arguments"]["command"].as_str(),
            Some("model.faces" | "model.edges")
        )
    });
    if batch && !topology && planned.len() > 1 + usize::from(inspect) {
        let verification = if inspect { planned.pop() } else { None };
        let commands: Vec<_> = planned.iter().map(|c| c["arguments"].clone()).collect();
        planned = vec![json!({"tool":"batch","arguments":{"commands":commands}})];
        planned.extend(verification);
    }
    json!(planned.iter().enumerate().map(|(i,c)| {
        let mut id = format!("pilot_direct_{index}_{}_{}", plan.rule, i);
        if let Some(messages) = request["messages"].as_array() {
            if messages.iter().any(|m| m["tool_calls"].as_array().is_some_and(|cs| cs.iter().any(|c| c["id"] == id))) {
                id = format!("{id}_{}", messages.len());
            }
        }
        json!({"id":id,"type":"function","function":{"name":c["tool"],"arguments":c["arguments"].to_string()}})
    }).collect::<Vec<_>>())
}
impl Proxy {
    pub(super) fn direct_turn(
        &self,
        request: &Value,
        recipe: &Option<planner::Plan>,
        index: usize,
        conversation: &mut Conversation,
        audit: &mut Audit,
    ) -> Result<Option<Reply>, String> {
        if conversation.direct_index != index {
            conversation.direct = None;
            conversation.direct_failed = false;
            conversation.direct_handback = None;
            conversation.direct_index = index;
        }
        if conversation.direct_failed {
            return Ok(None);
        }
        if let Some((previous_index, previous, emitted)) = conversation.direct.take() {
            if previous_index == index {
                let messages = request["messages"]
                    .as_array()
                    .ok_or("messages must be an array")?;
                let assistant = messages.iter().rposition(|m| {
                    m["role"] == "assistant" && m["tool_calls"] == emitted["tool_calls"]
                });
                let valid = assistant.is_some_and(|start| {
                    let results = &messages[start + 1..];
                    emitted["tool_calls"].as_array().is_some_and(|calls| {
                        results.len() == calls.len()
                            && calls.iter().all(|c| {
                                let matching: Vec<_> = results
                                    .iter()
                                    .filter(|m| m["role"] == "tool" && m["tool_call_id"] == c["id"])
                                    .collect();
                                matching.len() == 1
                                    && tool_data(matching[0]).is_some_and(|data| !has_error(&data))
                            })
                    })
                });
                let difference = if !valid {
                    Some("Recipe completion failed: a tool returned an error, or the expected tool results are missing or invalid. Inspect the design and recover; do not repeat accepted changes.".to_string())
                } else if previous.rule == "resolve_pattern" {
                    recipe.is_none().then(|| "Pattern target is unresolved: inspect a single body or last feature and clarify count or overlap. No pattern was created.".into())
                } else if previous.rule == "resolve_joint" {
                    recipe.is_none().then(|| "Joint target is unresolved: inspect exactly one matching pin and hole pair. No joint was created.".into())
                } else if previous.rule == "resolve_body" {
                    recipe.as_ref().is_none_or(|p| p.rule == "resolve_body").then(||
                        "Recipe target is unresolved: inspection must identify exactly one requested body. No change was made.".into())
                } else if previous.continuation.is_some() {
                    recipe.is_none().then(|| "Recipe completion is unverified: the edge lookup could not resolve the requested edges.".into())
                } else if let Some(expected) = &previous.expected {
                    planner::completion(request, expected)
                } else {
                    recipe
                        .as_ref()
                        .and_then(|p| p.facts.as_ref())
                        .is_none()
                        .then(|| {
                            "Recipe completion is unverified: measured query facts are missing."
                                .into()
                        })
                };
                if let Some(reason) = difference {
                    conversation.direct_failed = true;
                    conversation.completion_checked = true;
                    conversation.pending_recipe = None;
                    audit.record(
                        "direct_recipe_fallback",
                        &json!({"rule":previous.rule,"from_model":false}),
                        &json!({"reason":reason,"from_model":false}),
                    )?;
                    if self.cfg.upstream == "none" {
                        let text = format!(
                            "Exact match (no model): {reason} No model is configured to recover."
                        );
                        audit.record("direct_recipe_reply", &Value::Null, &json!({"rule":previous.rule,"from_model":false,"text":text,"verified":false}))?;
                        return Ok(Some(reply(request, text, audit.count)?));
                    }
                    conversation.direct_handback = Some(reason);
                    // The caller adds this hand-back before entering the ordinary model path.
                    return Ok(None);
                }
                if previous.continuation.is_none()
                    && !matches!(
                        previous.rule,
                        "resolve_body" | "resolve_joint" | "resolve_pattern"
                    )
                {
                    let summary_plan = if previous.expected.is_none() {
                        recipe.as_ref().unwrap_or(&previous)
                    } else {
                        &previous
                    };
                    let text = planner::direct_summary(request, summary_plan);
                    audit.record(
                        "direct_recipe_reply",
                        &Value::Null,
                        &json!({"rule":previous.rule,"from_model":false,"text":text}),
                    )?;
                    return Ok(Some(reply(request, text, audit.count)?));
                }
            }
        }
        let Some(plan) = recipe else {
            return Ok(None);
        };
        if plan.calls.is_empty() {
            let text = planner::direct_summary(request, plan);
            audit.record(
                "direct_recipe_reply",
                &Value::Null,
                &json!({"rule":plan.rule,"from_model":false,"text":text}),
            )?;
            return Ok(Some(reply(request, text, audit.count)?));
        }
        if plan.rule == "resolve_pattern" {
            if conversation.pattern_inspections >= 4 {
                return Ok(Some(reply(request, "Pattern inspection limit reached; clarify the target or geometry. No pattern was created.".into(), audit.count)?));
            }
            conversation.pattern_inspections += 1;
        }
        let message =
            json!({"role":"assistant","content":null,"tool_calls":calls(request, plan, index)});
        audit.record("direct_recipe_calls", &Value::Null, &json!({"request":last_user(request),"rule":plan.rule,"slots":plan.slots,"expected":plan.expected,"calls":plan.calls,"message":message,"from_model":false}))?;
        conversation.direct = Some((index, plan.clone(), message.clone()));
        conversation.recipe = plan.expected.clone().map(|e| (index, e));
        conversation.pending_recipe = plan.continuation.as_ref().map(|_| plan.clone());
        Ok(Some(response(request, message, "tool_calls", audit.count)?))
    }
}

// JSON ingress already bounds size and nesting, including batch result trees.
fn has_error(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            (key == "error" && !value.is_null())
                || (key == "isError" && value == true)
                || (key == "success" && value == false)
                || (key == "ok" && value == false)
                || has_error(value)
        }),
        Value::Array(values) => values.iter().any(has_error),
        _ => false,
    }
}
