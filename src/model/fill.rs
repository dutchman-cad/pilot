//! A recipe form is local to one fresh user turn, including two validation retries.
use super::*;
use std::hash::{Hash, Hasher};

/// Independent of prefix-based conversation identification. At capacity, stop
/// offering forms rather than forget a turn and risk replaying a creation.
#[derive(Default)]
pub(super) struct Turns(Vec<(String, usize, u64)>);

impl Turns {
    pub fn claim(&mut self, body: &Value) -> Result<bool, String> {
        let messages = body["messages"]
            .as_array()
            .ok_or("messages must be an array")?;
        let Some(index) = messages.iter().rposition(|m| m["role"] == "user") else {
            return Ok(false);
        };
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        json_bytes(&messages[index]["content"])?.hash(&mut hash);
        let hash = hash.finish();
        let model = body["model"].as_str().unwrap_or("");
        let position = messages[..index]
            .iter()
            .filter(|m| m["role"] != "system")
            .count();
        // A shorter prefix can move the same user message backwards. Keep the
        // content identity as well as its original position across such trims.
        if self.0.len() >= 4096
            || self
                .0
                .iter()
                .any(|(m, i, h)| m == model && *h == hash && position <= *i)
        {
            return Ok(false);
        }
        self.0.push((model.to_string(), position, hash));
        Ok(messages[index + 1..].iter().all(|m| m["role"] == "system"))
    }
}

/// Inspect every choice and call before accepting any local form. Mixed replies
/// are discarded in full, even if one call is the requested form.
pub(super) fn off_form<'a>(response: &'a Value, offered: &str) -> Vec<&'a str> {
    response["choices"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["message"]["tool_calls"].as_array().into_iter().flatten())
        .map(|c| c["function"]["name"].as_str().unwrap_or("<unnamed>"))
        .filter(|name| *name != offered)
        .collect()
}

#[cfg(test)]
pub(super) fn select(
    library: &crate::recipe_library::Library,
    request: &Value,
) -> Result<Option<Value>, String> {
    let band = library.form_band(last_user(request))?;
    Ok((band["band"] == "fill").then(|| band["recipes"][0].clone()))
}

pub(super) fn offer(
    outgoing: &mut Value,
    recipe: &Value,
    error: Option<&str>,
) -> Result<(), String> {
    let mut schema = crate::recipe_library::forms::compact(&recipe["schema"]);
    schema["properties"]["decline"] = json!({"type":"boolean"});
    // A decline is checked before the recipe fields, so it needs no invented dimensions.
    outgoing["tools"] = json!([{"type":"function","function":{"name":"pilot_fill","description":recipe["description"],"parameters":schema}}]);
    outgoing["tool_choice"] = json!({"type":"function","function":{"name":"pilot_fill"}});
    outgoing["n"] = json!(1);
    let messages = outgoing["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    if messages.len() >= 256 {
        return Err("fill prompt exceeds 256 messages".into());
    }
    let mut line = crate::recipe_library::forms::line(recipe["id"].as_str().unwrap_or(""));
    if let Some(error) = error {
        line.push_str(&format!(" Correct this field error: {error}"));
    }
    messages.push(json!({"role":"system","content":line}));
    Ok(())
}

pub(super) fn offer_route(outgoing: &mut Value, candidates: &[Value]) -> Result<(), String> {
    let ids: Vec<_> = candidates
        .iter()
        .take(4)
        .map(|r| r["id"].clone())
        .chain(std::iter::once(json!("none")))
        .collect();
    let descriptions = candidates
        .iter()
        .take(4)
        .map(|r| {
            format!(
                "{}: {}",
                r["id"].as_str().unwrap_or(""),
                r["description"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    outgoing["tools"] = json!([{"type":"function","function":{"name":"pilot_route","description":descriptions,"parameters":{"type":"object","properties":{"recipe":{"type":"string","enum":ids}},"required":["recipe"],"additionalProperties":false}}}]);
    outgoing["tool_choice"] = json!({"type":"function","function":{"name":"pilot_route"}});
    outgoing["n"] = json!(1);
    let messages = outgoing["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    if messages.len() >= 256 {
        return Err("route prompt exceeds 256 messages".into());
    }
    messages.push(
        json!({"role":"system","content":"Pick the form that matches the request, or none."}),
    );
    Ok(())
}

pub(super) fn picked(response: &Value, candidates: &[Value]) -> Option<Value> {
    let choices = response["choices"].as_array().filter(|c| c.len() == 1)?;
    let calls = choices[0]["message"]["tool_calls"]
        .as_array()
        .filter(|c| c.len() == 1)?;
    if calls[0]["function"]["name"] != "pilot_route" {
        return None;
    }
    let raw = &calls[0]["function"]["arguments"];
    let args: Value = if let Some(s) = raw.as_str() {
        serde_json::from_str(s).ok()?
    } else {
        raw.clone()
    };
    if args.as_object()?.len() != 1 {
        return None;
    }
    candidates
        .iter()
        .take(4)
        .find(|r| r["id"] == args["recipe"])
        .cloned()
}

pub(super) enum Outcome {
    Build(Value, Box<planner::Plan>),
    Decline,
    Fallback,
    Invalid(String),
}

/// Coercions are returned individually so the proxy audits each supplied field.
pub(super) fn coerced(response: &Value, recipe: &Value) -> Result<(Value, Vec<Value>), String> {
    let mut response = response.clone();
    if response["choices"].as_array().is_none_or(|c| c.len() != 1)
        || !response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .is_some_and(|c| c.len() == 1 && c[0]["function"]["name"] == "pilot_fill")
    {
        return Ok((response, Vec::new()));
    }
    let raw = &response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"];
    if raw.is_null() {
        return Ok((response, Vec::new()));
    }
    let mut args: Value = if let Some(text) = raw.as_str() {
        match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return Ok((response, Vec::new())),
        }
    } else {
        raw.clone()
    };
    let mut schema = recipe["schema"].clone();
    schema["properties"]["decline"] = json!({"type":"boolean"});
    let changes = crate::recipe_library::forms::coerce(&mut args, &schema)?;
    response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] =
        json!(args.to_string());
    Ok((response, changes))
}

pub(super) fn accept(
    response: &Value,
    recipe: &Value,
    request: &Value,
    library: &crate::recipe_library::Library,
    catalog: &Catalog,
) -> Outcome {
    let response = match coerced(response, recipe) {
        Ok((r, _)) => r,
        Err(e) => return Outcome::Invalid(e),
    };
    let Some(choices) = response["choices"].as_array().filter(|c| c.len() == 1) else {
        return Outcome::Fallback;
    };
    let Some(calls) = choices[0]["message"]["tool_calls"]
        .as_array()
        .filter(|c| c.len() == 1 && c[0]["function"]["name"] == "pilot_fill")
    else {
        return Outcome::Fallback;
    };
    let raw = &calls[0]["function"]["arguments"];
    let parsed = if let Some(text) = raw.as_str() {
        serde_json::from_str(text).map_err(|e| {
            format!(
                "args {text:?} requires 1 valid JSON object ({e}); fix JSON quoting and brackets"
            )
        })
    } else {
        Ok(raw.clone())
    };
    let mut args = match parsed {
        Ok(a) => a,
        Err(e) => return Outcome::Invalid(e),
    };
    if args["decline"] == true {
        return Outcome::Decline;
    }
    if args.get("decline").is_some_and(|d| !d.is_boolean()) {
        return Outcome::Invalid(format!(
            "args.decline {} requires 1 boolean; use true to decline or omit it to build",
            args["decline"]
        ));
    }
    if let Some(o) = args.as_object_mut() {
        o.remove("decline");
    }
    if matches!(
        recipe["schema"]["x-pilot-expander"].as_str(),
        Some("fillet" | "chamfer" | "move" | "mirror")
    ) && !planner::recipe_targets(request)
        .iter()
        .any(|name| args["body"] == *name)
    {
        return Outcome::Invalid(format!(
            "args.body {} must match exactly 1 inspected body name; inspect and use its exact name",
            args["body"]
        ));
    }
    let envelope = json!({"recipe":recipe["id"],"args":args});
    match library.build(&envelope, request, catalog) {
        Ok(plan) => Outcome::Build(envelope, Box::new(plan)),
        Err(e) => Outcome::Invalid(e),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn request(text: &str) -> Value {
        json!({"messages":[{"role":"user","content":text}]})
    }
    #[test]
    fn turn_identity_survives_trim_and_system_changes_but_allows_new_users() {
        let mut turns = Turns::default();
        let mut req = request("make a profile");
        req["messages"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"role":"user","content":"earlier task"}));
        assert!(turns.claim(&req).unwrap());
        assert!(!turns.claim(&req).unwrap());
        req["messages"].as_array_mut().unwrap().remove(0);
        req["messages"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"role":"system","content":"new prompt"}));
        assert!(!turns.claim(&req).unwrap());
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"a different profile"}));
        assert!(turns.claim(&req).unwrap());
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"a different profile"}));
        assert!(turns.claim(&req).unwrap());
        // Capacity is fail-closed, including for unseen content.
        turns.0.resize(4096, (String::new(), 0, 0));
        assert!(!turns.claim(&request("another task")).unwrap());
    }

    #[test]
    fn tool_history_blocks_forms_before_local_history_is_stripped() {
        for name in [
            "batch",
            "pilot_fill",
            "pilot_route",
            "pilot_build",
            "inspect",
        ] {
            let mut req = request("make a profile");
            req["messages"].as_array_mut().unwrap().push(json!({"role":"assistant","tool_calls":[{"function":{"name":name,"arguments":"{}"}}]}));
            let mut turns = Turns::default();
            assert!(!turns.claim(&req).unwrap());
            req["messages"].as_array_mut().unwrap().pop();
            assert!(!turns.claim(&req).unwrap());
        }
        let response = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill"}},{"function":{"name":"batch"}}]}}]});
        assert_eq!(off_form(&response, "pilot_fill"), vec!["batch"]);
    }
    #[test]
    fn probe_error_retries_and_inner_lip_defaults_use_the_shared_validator() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let catalog = Catalog::bundled().unwrap();
        let req = request("Make a plate with a bore and bolt circle and rounded corners");
        let recipe = library
            .find("plate with bolt circle", &crate::recipe_library::Bm25)
            .unwrap()["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "plate")
            .unwrap()
            .clone();
        let mut args = json!({"length":80,"width":80,"thickness":8,"centred":true,"bore_diameter":30,"hole_diameter":8,"bolt_circle":{"count":6,"diameter":56},"corners":{"kind":"fillet","size":10,"edges":"all"}});
        let response = |args: &Value| json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill","arguments":args.to_string()}}]}}]});
        let Outcome::Invalid(error) = accept(&response(&args), &recipe, &req, &library, &catalog)
        else {
            panic!("invalid plate accepted");
        };
        assert!(error.contains("below 4 (half the 8 mm thickness)"));
        assert!(error.contains("use edges \"vertical\""));
        let mut retry = req.clone();
        offer(&mut retry, &recipe, Some(&error)).unwrap();
        assert!(
            retry["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains(&error)
        );
        args["corners"].as_object_mut().unwrap().remove("edges");
        assert!(matches!(
            accept(&response(&args), &recipe, &req, &library, &catalog),
            Outcome::Build(_, _)
        ));
        let recipe = library
            .find("enclosure with inner lip", &crate::recipe_library::Bm25)
            .unwrap()["recipes"][0]
            .clone();
        assert_eq!(recipe["id"], "enclosure");
        let args =
            json!({"length":80,"width":50,"height":30,"wall":2,"lip":{"width":1,"height":2}});
        assert!(matches!(
            accept(&response(&args), &recipe, &req, &library, &catalog),
            Outcome::Build(_, _)
        ));
    }
    #[test]
    fn lexical_near_misses_do_not_force_forms() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        for text in [
            "Explain a mounting plate with a bolt circle",
            "A plate with a helical gear and bolt circle holes",
            "L profile with variable twisted loft",
            "Do not make an open enclosure with an inner lip",
            "a cuboid please",
            "write a sonnet",
            "sketch a T shirt illustration",
        ] {
            assert!(
                select(&library, &request(text)).unwrap().is_none(),
                "{text}"
            );
        }
    }
    #[test]
    fn existing_body_forms_require_complete_state_and_known_names() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let mut req = request("fillet all the edges");
        assert_eq!(select(&library, &req).unwrap().unwrap()["id"], "fillet");
        req["messages"].as_array_mut().unwrap().insert(0,json!({"role":"tool","content":json!({"body_count":1,"bodies":[{"name":"Part"}]}).to_string()}));
        let found = select(&library, &req).unwrap().unwrap();
        assert_eq!(found["id"], "fillet");
        let response = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill","arguments":"{\"body\":\"Invented\",\"amount\":3}"}}]}}]});
        assert!(matches!(
            accept(
                &response,
                &found,
                &req,
                &library,
                &Catalog::bundled().unwrap()
            ),
            Outcome::Invalid(_)
        ));
        req["messages"][0]["content"] =
            json!(json!({"body_count":2,"bodies":[{"name":"Part"}]}).to_string());
        assert!(matches!(
            accept(
                &response,
                &found,
                &req,
                &library,
                &Catalog::bundled().unwrap()
            ),
            Outcome::Invalid(_)
        ));
    }
}
