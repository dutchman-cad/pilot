//! Exact recipe responses use the client's native tools and normal approval path.
use super::*;

pub(super) fn unavailable() -> String {
    format!("No model is configured. I can handle exact requests with every required dimension or slot for: {}. See docs/recipes.md for supported forms.", planner::supported_recipes().join(", "))
}
/// The measured reply; only a plan Pilot read without the model is called an exact match.
fn summary(request: &Value, plan: &planner::Plan, from_model: bool) -> String {
    let text = planner::direct_summary(request, plan);
    if from_model {
        text.replacen("Exact match (no model):", "Built from the model's form:", 1)
    } else {
        text
    }
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
pub(super) fn native(request: &Value, message: Value, rewrites: usize) -> Result<Reply, String> {
    response(request, message, "tool_calls", rewrites)
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
pub(super) enum Outcome {
    Reply(Reply),
    Form,
    Continue,
}

impl Proxy {
    pub(super) fn direct_turn(
        &self,
        request: &Value,
        recipe: &Option<planner::Plan>,
        index: usize,
        conversation: &mut Conversation,
        audit: &mut Audit,
    ) -> Result<Outcome, String> {
        if conversation.direct_index != index {
            conversation.direct = None;
            conversation.direct_failed = false;
            conversation.direct_handback = None;
            conversation.direct_index = index;
        }
        if conversation.direct_failed {
            return Ok(Outcome::Continue);
        }
        // A plan the model filled in (a form or its own shortcut call) is not an exact match.
        let from_model = conversation
            .shortcut_args
            .as_ref()
            .is_some_and(|(i, _)| *i == index);
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
                    recipe.is_none().then(|| "Pattern target is unresolved: inspect a single body or last feature and clarify count or overlap. No pattern plan was built.".into())
                } else if previous.rule == "resolve_joint" {
                    recipe.is_none().then(|| "Joint target is unresolved: inspect exactly one matching pin and hole pair. No joint plan was built.".into())
                } else if previous.rule == "resolve_body" {
                    recipe.as_ref().is_none_or(|p| p.rule == "resolve_body").then(||
                        "Recipe target is unresolved: inspection must identify exactly one requested body. No edit plan was built. No change was made.".into())
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
                    if let Some(text) = shell_edge_failure(request, &previous, from_model) {
                        conversation.direct_failed = true;
                        conversation.completion_checked = true;
                        conversation.pending_recipe = None;
                        audit.record("direct_recipe_reply", &Value::Null, &json!({"rule":previous.rule,"from_model":from_model,"text":text,"edge_failed":true}))?;
                        return Ok(Outcome::Reply(reply(request, text, audit.count)?));
                    }
                    // These recipes have only read the design. Their successful reads
                    // are also valid form context; no accepted mutation needs recovery.
                    let provisional = matches!(
                        previous.rule,
                        "resolve_body" | "resolve_joint" | "resolve_pattern"
                    );
                    if provisional
                        && fill::inspection_results(request, &emitted)
                        && self.cfg.upstream != "none"
                        && !self.cfg.no_fill
                        && !self.cfg.no_shortcuts
                        && !self.cfg.solidcraft98
                        && shortcuts::supported(request)
                    {
                        conversation.recipe = None;
                        conversation.pending_recipe = None;
                        conversation.completion_checked = false;
                        audit.record(
                            "direct_recipe_form_handback",
                            &json!({"rule":previous.rule}),
                            &json!({"reason":reason}),
                        )?;
                        return Ok(Outcome::Form);
                    }
                    conversation.direct_failed = true;
                    conversation.completion_checked = true;
                    conversation.pending_recipe = None;
                    audit.record(
                        "direct_recipe_fallback",
                        &json!({"rule":previous.rule,"from_model":from_model}),
                        &json!({"reason":reason,"from_model":from_model}),
                    )?;
                    if self.cfg.upstream == "none" {
                        let text = format!(
                            "Exact match (no model): {reason} No model is configured to recover."
                        );
                        audit.record("direct_recipe_reply", &Value::Null, &json!({"rule":previous.rule,"from_model":false,"text":text,"verified":false}))?;
                        return Ok(Outcome::Reply(reply(request, text, audit.count)?));
                    }
                    conversation.direct_handback = Some(reason);
                    // The caller adds this hand-back before entering the ordinary model path.
                    return Ok(Outcome::Continue);
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
                    let text = summary(request, summary_plan, from_model);
                    audit.record(
                        "direct_recipe_reply",
                        &Value::Null,
                        &json!({"rule":previous.rule,"from_model":from_model,"text":text}),
                    )?;
                    return Ok(Outcome::Reply(reply(request, text, audit.count)?));
                }
            }
        }
        let Some(plan) = recipe else {
            return Ok(Outcome::Continue);
        };
        if plan.calls.is_empty() {
            let text = summary(request, plan, from_model);
            audit.record(
                "direct_recipe_reply",
                &Value::Null,
                &json!({"rule":plan.rule,"from_model":from_model,"text":text}),
            )?;
            return Ok(Outcome::Reply(reply(request, text, audit.count)?));
        }
        if plan.rule == "resolve_pattern" {
            if conversation.pattern_inspections >= 4 {
                return Ok(Outcome::Reply(reply(request, "Pattern inspection limit reached; clarify the target or geometry. No pattern plan was built.".into(), audit.count)?));
            }
            conversation.pattern_inspections += 1;
        }
        let message =
            json!({"role":"assistant","content":null,"tool_calls":calls(request, plan, index)});
        audit.record("direct_recipe_calls", &Value::Null, &json!({"request":last_user(request),"rule":plan.rule,"slots":plan.slots,"expected":plan.expected,"calls":plan.calls,"message":message,"from_model":from_model}))?;
        conversation.direct = Some((index, plan.clone(), message.clone()));
        conversation.recipe = plan.expected.clone().map(|e| (index, e));
        conversation.pending_recipe = plan.continuation.as_ref().map(|_| plan.clone());
        Ok(Outcome::Reply(response(
            request,
            message,
            "tool_calls",
            audit.count,
        )?))
    }
}

/// A refused finishing operation must not send an accepted shell back for rebuilding.
fn shell_edge_failure(request: &Value, plan: &planner::Plan, from_model: bool) -> Option<String> {
    let shell = plan.slots.get("ShellBox")?;
    let fillet = shell["rounding"][0].as_bool()?;
    if plan.continuation.is_some()
        || !matches!(
            plan.calls.first()?["arguments"]["command"].as_str(),
            Some("solid.fillet" | "solid.chamfer")
        )
    {
        return None;
    }
    let name = plan.expected.as_ref()?.name.as_ref()?;
    let messages = request["messages"].as_array()?;
    let refused = messages
        .iter()
        .rev()
        .filter(|m| m["role"] == "tool")
        .take(2)
        .any(|m| {
            let raw = m["content"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| m["content"].clone());
            has_error(&raw) || has_error(&guidance::payload(&raw))
        });
    if !refused {
        return None;
    }
    let measured = request["messages"]
        .as_array()?
        .iter()
        .rev()
        .filter(|m| m["role"] == "tool")
        .take(2)
        .filter_map(tool_data)
        .find(|d| d["bodies"].is_array());
    let retained = measured.as_ref().is_some_and(|d| {
        d["bodies"].as_array().is_some_and(|b| {
            b.iter().any(|b| {
                b["name"] == *name
                    && b["volume_mm3"]
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && v > 0.0)
            })
        })
    });
    let prefix = if from_model {
        "Built from the model's form:"
    } else {
        "Exact match (no model):"
    };
    let edge = if fillet { "fillet" } else { "chamfer" };
    Some(format!(
        "{prefix} The requested all-edge {edge} could not build. {} No finishing retry was made.",
        if retained {
            "The shell remains in place."
        } else {
            "Shell creation was accepted, but its current measurements are unverified."
        }
    ))
}

// JSON ingress already bounds size and nesting, including batch result trees.
pub(super) fn has_error(value: &Value) -> bool {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod shell_tests {
    use super::*;
    #[test]
    fn refused_shell_fillet_reports_preserved_shell() {
        let cat = Catalog::bundled().unwrap();
        let mut req = json!({"messages":[{"role":"user","content":"open top box 68x46x32mm walls 2mm fillet all edges 3mm"}],"tools":[{"function":{"name":"execute"}}]});
        let plan = planner::plan_direct(&req, &cat).unwrap().unwrap();
        // The refusal reply is only for the finishing phase, never the creation prefix.
        assert!(shell_edge_failure(&req, &plan, false).is_none());
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":{"body":"PilotRecipe1","edges":[{"index":0}]}}));
        let next = planner::resume(&req, &plan, &cat).unwrap().unwrap();
        assert_eq!(next.calls.len(), 2);
        req["messages"].as_array_mut().unwrap().extend([
            json!({"role":"tool","content":{"isError":true,"error":"radius cannot build"}}),
            json!({"role":"tool","content":{"bodies":[{"name":"PilotRecipe1","volume_mm3":15552.}],"timeline":[]}}),
        ]);
        for from_model in [false, true] {
            let text = shell_edge_failure(&req, &next, from_model).unwrap();
            assert!(text.contains("fillet could not build"));
            assert!(text.contains("shell remains in place"));
            assert_eq!(text.starts_with("Built from the model's form:"), from_model);
        }
    }
}
