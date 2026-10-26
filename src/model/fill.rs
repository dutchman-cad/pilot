//! A recipe form is local to one fresh user turn, including its one validation retry.
use super::*;

pub(super) fn select(
    library: &crate::recipe_library::Library,
    request: &Value,
) -> Result<Option<Value>, String> {
    let query = last_user(request);
    if query.trim().is_empty() || query.len() > 2048 {
        return Ok(None);
    }
    let found = library.find(query, &crate::recipe_library::Bm25)?;
    // Retrieval is lexical: conservatively keep explanation requests and unsupported
    // geometry on the normal tools even when they repeat many recipe keywords.
    let lower = query.to_lowercase();
    if lower.split(|c: char| !c.is_alphabetic()).any(|w| {
        matches!(
            w,
            "how"
                | "why"
                | "not"
                | "don"
                | "instead"
                | "explain"
                | "describe"
                | "story"
                | "poem"
                | "compare"
                | "loft"
                | "lofted"
                | "helical"
                | "gear"
                | "twisted"
                | "sweep"
                | "swept"
                | "variable"
        )
    }) {
        return Ok(None);
    }
    let top = &found["recipes"][0];
    if top["score"].as_f64().unwrap_or(0.0) < crate::recipe_library::FILL_THRESHOLD {
        return Ok(None);
    }
    // These expanders create their own geometry or resolve their targets through inspection.
    // Unknown development sequences and existing-body edits stay on the normal path.
    let targets = planner::recipe_targets(request);
    let supported = library.recipes.iter().any(|r| {
        r.id == top["id"]
            && (matches!(
                r.expander.as_str(),
                "profile"
                    | "plate"
                    | "enclosure"
                    | "cross_drilled_box"
                    | "honeycomb_pattern"
                    | "linear_pattern"
                    | "grid_pattern"
                    | "circular_pattern"
                    | "revolute_joint"
                    | "box"
                    | "cylinder"
                    | "sphere"
                    | "tube"
                    | "shell"
                    | "revolve"
            ) || (!targets.is_empty()
                && matches!(
                    r.expander.as_str(),
                    "fillet" | "chamfer" | "move" | "mirror"
                )))
    });
    Ok(supported.then(|| top.clone()))
}

pub(super) fn offer(
    outgoing: &mut Value,
    recipe: &Value,
    error: Option<&str>,
) -> Result<(), String> {
    let mut schema = recipe["schema"].clone();
    schema["properties"]["decline"] =
        json!({"type":"boolean","description":"true if this form does not fit the request"});
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
    let mut line = format!("Recipe {}: fill the form from the user's words; convert inches to mm; leave optional fields out if not stated; set decline if it does not fit.", recipe["id"].as_str().unwrap_or(""));
    if let Some(error) = error {
        line.push_str(&format!(" Correct this field error: {error}"));
    }
    messages.push(json!({"role":"system","content":line}));
    Ok(())
}

pub(super) enum Outcome {
    Build(Value, Box<planner::Plan>),
    Decline,
    Fallback,
    Invalid(String),
}

pub(super) fn accept(
    response: &Value,
    recipe: &Value,
    request: &Value,
    library: &crate::recipe_library::Library,
    catalog: &Catalog,
) -> Outcome {
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
        serde_json::from_str(text).map_err(|e| format!("args: invalid JSON: {e}"))
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
        return Outcome::Invalid("args.decline: must be boolean".into());
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
        return Outcome::Invalid(
            "args.body: must name a body in the latest complete inspection".into(),
        );
    }
    let envelope = json!({"recipe":recipe["id"],"args":args});
    match library.build(&envelope, request, catalog) {
        Ok(plan) => Outcome::Build(envelope, Box::new(plan)),
        Err(e) => Outcome::Invalid(e),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn request(text: &str) -> Value {
        json!({"messages":[{"role":"user","content":text}]})
    }
    #[test]
    fn lexical_near_misses_do_not_force_forms() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        for text in [
            "Explain a mounting plate with a bolt circle",
            "A plate with a helical gear and bolt circle holes",
            "L profile with variable twisted loft",
            "How do I extrude a T profile with a thick web?",
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
        assert!(select(&library, &req).unwrap().is_none());
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
        assert!(select(&library, &req).unwrap().is_none());
    }
}
