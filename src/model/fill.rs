//! A recipe form is local to one fresh user turn, including two validation retries.
use super::*;
/// Client history is the turn boundary: only system messages may follow a fresh
/// user message. Check before stripping local tools or trimming history. Keeping
/// no turn identities avoids cross-conversation collisions and retains no entries
/// after a turn finishes (or while it is in progress).
pub(super) fn fresh_turn(body: &Value) -> Result<bool, String> {
    let messages = body["messages"]
        .as_array()
        .ok_or("messages must be an array")?;
    let Some(index) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(false);
    };
    Ok(messages[index + 1..].iter().all(|m| m["role"] == "system"))
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

/// Only inference settings cross into the extraction call. Client instructions,
/// tool definitions, history, response formats and routing metadata stay out.
fn clean(
    request: &Value,
    mut line: String,
    needs_names: bool,
    joint: bool,
    joint_origins: bool,
) -> Result<Value, String> {
    let mut out = json!({});
    for key in [
        "model",
        "temperature",
        "top_p",
        "top_k",
        "min_p",
        "seed",
        "max_tokens",
        "max_completion_tokens",
        "frequency_penalty",
        "presence_penalty",
        "logit_bias",
        "typical_p",
        "repetition_penalty",
        "repeat_penalty",
        "stop",
        "options",
        "reasoning_effort",
    ] {
        if let Some(value) = request.get(key) {
            out[key] = value.clone();
        }
    }
    let mut user = last_user(request).to_string();
    if needs_names {
        line.push_str(" Use only inspected design facts. For this/these/it, use default_body (selection, last created body, then only body); decline if ambiguous. Use last_feature for a requested latest boss, rib or hole only when its inspected definition matches. Never rebuild an existing body to shell it.");
        let context = planner::form_context::summary(request, joint, joint_origins);
        if !context.is_empty() {
            user.push_str(&format!("\nDesign context: {context}"));
        }
    }
    out["messages"] = json!([{"role":"system","content":line},{"role":"user","content":user}]);
    out["stream"] = json!(false);
    out["n"] = json!(1);
    repair::check_value(&out)?;
    Ok(out)
}

pub(super) fn retry_note(outgoing: &mut Value, problem: &str) {
    if let Some(line) = outgoing["messages"][0]["content"].as_str() {
        let problem: String = problem
            .chars()
            .take(1024)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        outgoing["messages"][0]["content"] =
            json!(format!("{line}\nPrevious reply problem: {problem}"));
    }
}

pub(super) fn inspection_results(request: &Value, emitted: &Value) -> bool {
    let Some(messages) = request["messages"].as_array() else {
        return false;
    };
    let Some(start) = messages
        .iter()
        .rposition(|m| m["role"] == "assistant" && m["tool_calls"] == emitted["tool_calls"])
    else {
        return false;
    };
    let Some(calls) = emitted["tool_calls"].as_array() else {
        return false;
    };
    let results = &messages[start + 1..];
    results.len() == calls.len()
        && calls.iter().all(|call| {
            let matching: Vec<_> = results
                .iter()
                .filter(|m| m["role"] == "tool" && m["tool_call_id"] == call["id"])
                .collect();
            matching.len() == 1
                && tool_data(matching[0])
                    .is_some_and(|d| !direct::has_error(&d) && guidance::paging(&d).is_empty())
        })
}

pub(super) fn needs_names(recipe: &Value) -> bool {
    matches!(
        recipe["schema"]["x-pilot-expander"].as_str(),
        Some(
            "fillet"
                | "chamfer"
                | "move"
                | "mirror"
                | "linear_pattern"
                | "grid_pattern"
                | "circular_pattern"
                | "revolute_joint"
                | "joint"
                | "shell"
        )
    )
}

pub(super) fn offer(
    outgoing: &mut Value,
    recipe: &Value,
    error: Option<&str>,
) -> Result<(), String> {
    let mut schema = crate::recipe_library::forms::compact(&recipe["schema"]);
    requested_fields(&mut schema, recipe, last_user(outgoing))?;
    *outgoing = clean(
        outgoing,
        crate::recipe_library::forms::line(recipe["id"].as_str().unwrap_or("")),
        needs_names(recipe),
        recipe["id"] == "revolute_joint" || recipe["id"] == "joint",
        recipe["id"] == "joint",
    )?;
    schema["properties"]["decline"] = json!({"type":"boolean"});
    // A decline is checked before the recipe fields, so it needs no invented dimensions.
    outgoing["tools"] = json!([{"type":"function","function":{"name":"pilot_fill","description":recipe["description"],"parameters":schema}}]);
    outgoing["tool_choice"] = json!({"type":"function","function":{"name":"pilot_fill"}});
    outgoing["n"] = json!(1);
    if let Some(error) = error {
        retry_note(outgoing, &format!("Correct this field error: {error}"));
    }
    Ok(())
}

/// Gate only fields added after beta.7; full validation/build schemas stay intact.
/// Read the request before clean() appends inspected geometry and retry notes.
fn requested_fields(schema: &mut Value, recipe: &Value, request: &str) -> Result<(), String> {
    if request.len() > 65_536 {
        return Err("fill field request exceeds 65536 bytes".into());
    }
    let text = request.to_ascii_lowercase();
    let has = |terms: &[&str]| {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| terms.contains(&word))
    };
    let words = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty());
    let named_corner = words.clone().zip(words.skip(1)).any(|(count, corner)| {
        matches!(count, "one" | "two" | "1" | "2" | "single")
            && matches!(corner, "corner" | "corners")
    });
    let side = has(&[
        "right",
        "left",
        "top",
        "bottom",
        "side",
        "sides",
        "front",
        "back",
        "upper",
        "lower",
        "topright",
        "topleft",
        "bottomright",
        "bottomleft",
    ]) || named_corner;
    let Some(properties) = schema["properties"].as_object_mut() else {
        return Ok(());
    };
    let mut gate = |field: &str, requested: bool| {
        if !requested {
            properties.remove(field);
        }
    };
    match recipe["id"].as_str() {
        Some("plate") => {
            gate("corner_sides", side);
            let placed = side || has(&["edge", "edges", "offset", "offsets"]);
            gate("row_offset_x", placed);
            gate("row_offset_y", placed);
            gate("bolt_circle_radius", has(&["radius", "radii"]));
        }
        Some("chamfer" | "fillet") => {
            gate("corner_sides", side);
            gate(
                "edges",
                side || has(&["corner", "corners", "vertical", "upright"]),
            );
        }
        Some("enclosure") => {
            gate(
                "inside",
                has(&["inside", "inner", "clear", "internal", "interior"]),
            );
            gate("lip", has(&["lip", "lips"]));
        }
        Some("shell") => gate(
            "open",
            has(&["top", "bottom", "upper", "lower", "underside"]),
        ),
        _ => {}
    }
    Ok(())
}

pub(super) fn offer_route(outgoing: &mut Value, candidates: &[Value]) -> Result<(), String> {
    *outgoing = clean(outgoing, "You select one form for the user's CAD request. Call pilot_route once with the matching recipe, or none if no form fits.".into(), candidates.iter().take(4).any(needs_names), candidates.iter().take(4).any(|r| r["id"] == "revolute_joint" || r["id"] == "joint"), candidates.iter().take(4).any(|r| r["id"] == "joint"))?;
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

/// A corner-only form cannot fulfill a separate request to create holes.
/// Leave the complete request and native tools available instead of silently
/// completing just the bevel on the unperforated starting body.
pub(super) fn partial_edge_edit(recipe: &Value, request: &Value) -> bool {
    if !matches!(recipe["id"].as_str(), Some("chamfer" | "fillet")) {
        return false;
    }
    let text = last_user(request).to_ascii_lowercase();
    let words: Vec<_> = text
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    words.iter().any(|w| matches!(*w, "add" | "drill"))
        && words
            .iter()
            .any(|w| matches!(*w, "hole" | "holes" | "bore" | "bores"))
}

/// Coercions are returned individually so the proxy audits each supplied field.
pub(super) fn coerced(
    response: &Value,
    recipe: &Value,
    request: &Value,
) -> Result<(Value, Vec<Value>), String> {
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
    let mut changes = crate::recipe_library::forms::coerce(&mut args, &schema)?;
    changes.extend(crate::recipe_library::forms::request_defaults(
        &mut args, &schema, request,
    )?);
    if args["decline"] != true && planner::patterns::is_pattern(recipe["id"].as_str().unwrap_or(""))
    {
        changes.extend(planner::patterns::resolve(&mut args, request)?);
    }
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
    if partial_edge_edit(recipe, request) {
        return Outcome::Fallback;
    }
    let response = match coerced(response, recipe, request) {
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
    if needs_names(recipe)
        && recipe["id"] != "revolute_joint"
        && recipe["id"] != "joint"
        && !planner::patterns::is_pattern(recipe["id"].as_str().unwrap_or(""))
    {
        let pronoun = last_user(request)
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| matches!(w.to_ascii_lowercase().as_str(), "this" | "these" | "it"));
        let explicit = planner::recipe_targets(request).iter().any(|n| {
            last_user(request)
                .to_ascii_lowercase()
                .contains(&n.to_ascii_lowercase())
        });
        if pronoun && !explicit && args["target"] != "last_feature" {
            match planner::form_context::default_body(request) {
                Ok(name) => {
                    if args.get("body").is_some_and(|b| *b != name) {
                        return Outcome::Invalid(format!("args.body must use the current target {name:?} (selection, last created body, or only body)"));
                    }
                    args["body"] = json!(name);
                }
                Err(e) => return Outcome::Invalid(e),
            }
        }
    }
    if matches!(
        recipe["schema"]["x-pilot-expander"].as_str(),
        Some("fillet" | "chamfer" | "move" | "mirror")
    ) && planner::recipe_targets(request)
        .iter()
        .filter(|name| args["body"] == **name)
        .count()
        != 1
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
    fn new_fields_are_offered_only_for_request_words_and_beta7_fields_remain() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        for (id, field, plain, named) in [
            (
                "plate",
                "corner_sides",
                "Make a plate with six holes on a bolt circle and rounded corners",
                "Make a plate rounded on the right side",
            ),
            (
                "plate",
                "row_offset_x",
                "Make a plate with a row of five holes 15 mm apart",
                "Make a plate with a row of five studs along the front edge",
            ),
            (
                "plate",
                "row_offset_y",
                "Make a plate with a row of five holes 15 mm apart",
                "Make a plate with a row of holes offset 10 mm",
            ),
            (
                "plate",
                "bolt_circle_radius",
                "Make a plate with six holes on a 60 mm bolt circle",
                "Make a plate with six holes on a 30 mm radius circle",
            ),
            (
                "chamfer",
                "corner_sides",
                "Bevel all edges",
                "Bevel the two corners on the left",
            ),
            (
                "fillet",
                "corner_sides",
                "Round all edges",
                "Round the upper right corner",
            ),
            (
                "chamfer",
                "edges",
                "Bevel all edges",
                "Bevel upright corners",
            ),
            ("fillet", "edges", "Round all edges", "Round vertical edges"),
            (
                "enclosure",
                "inside",
                "Make an enclosure 80 by 60 by 30",
                "Make an enclosure with clear inside dimensions 80 by 60 by 30",
            ),
            (
                "enclosure",
                "lip",
                "Make an open enclosure",
                "Make an enclosure with a lip",
            ),
            (
                "shell",
                "open",
                "Shell this box with 2 mm walls",
                "Shell this box removing the bottom face",
            ),
        ] {
            let source = library.recipes.iter().find(|r| r.id == id).unwrap();
            let recipe =
                json!({"id":id,"schema":crate::recipe_library::forms::flat(&source.schema)});
            let full = crate::recipe_library::forms::compact(&recipe["schema"]);
            for (text, offered) in [(plain, false), (named, true)] {
                let mut req = request("old request: right side inside lip bottom");
                req["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"user","content":text}));
                req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"bodies":[{"name":"Right side inside lip bottom"}],"timeline":[]}).to_string()}));
                offer(&mut req, &recipe, Some("right side inside lip bottom")).unwrap();
                let properties = &req["tools"][0]["function"]["parameters"]["properties"];
                assert_eq!(
                    properties.get(field).is_some(),
                    offered,
                    "{id}.{field}: {text}"
                );
                let new_fields: &[&str] = match id {
                    "plate" => &[
                        "corner_sides",
                        "row_offset_x",
                        "row_offset_y",
                        "bolt_circle_radius",
                    ],
                    "chamfer" | "fillet" => &["corner_sides", "edges"],
                    "enclosure" => &["inside", "lip"],
                    "shell" => &["open"],
                    _ => &[],
                };
                for (key, value) in full["properties"].as_object().unwrap() {
                    if !new_fields.contains(&key.as_str()) {
                        assert_eq!(&properties[key], value, "beta.7 {id}.{key}");
                    }
                }
            }
        }
    }

    #[test]
    fn plate_corner_words_are_case_insensitive_and_match_whole_words() {
        let recipe = json!({"id":"plate"});
        for text in [
            "RIGHT",
            "left-hand",
            "top",
            "bottom",
            "sides",
            "front",
            "back",
            "upper",
            "lower",
            "one corner",
            "two corners",
            "1 corner",
            "single corner",
            "top_right",
        ] {
            let mut schema = json!({"properties":{"corner_sides":{}}});
            requested_fields(&mut schema, &recipe, text).unwrap();
            assert!(schema["properties"].get("corner_sides").is_some(), "{text}");
        }
        for text in [
            "rounded corners",
            "all four corners",
            "bolt circle",
            "bright plate",
            "backplate",
            "topology",
        ] {
            let mut schema = json!({"properties":{"corner_sides":{}}});
            requested_fields(&mut schema, &recipe, text).unwrap();
            assert!(schema["properties"].get("corner_sides").is_none(), "{text}");
        }
        assert!(requested_fields(&mut json!({}), &recipe, &"x".repeat(65_537)).is_err());
    }

    #[test]
    fn joint_forms_are_separate_and_origins_only_reach_general_joint() {
        let lib = crate::recipe_library::Library::embedded().unwrap();
        for recipe in &lib.recipes {
            let mut req = request("Connect the inspected parts");
            req["messages"].as_array_mut().unwrap().extend([
                json!({"role":"tool","content":json!({"bodies":[{"name":"Base"}],"timeline":[]}).to_string()}),
                json!({"role":"tool","content":json!({"origins":[{"name":"BaseAnchor","occurrence":2}]}).to_string()}),
            ]);
            let found = json!({"id":recipe.id,"description":recipe.description,"schema":crate::recipe_library::forms::flat(&recipe.schema)});
            offer(&mut req, &found, None).unwrap();
            let user = req["messages"][1]["content"].as_str().unwrap();
            assert_eq!(
                user.contains("joint_origins"),
                recipe.id == "joint",
                "{}",
                recipe.id
            );
            let props = req["tools"][0]["function"]["parameters"]["properties"]
                .as_object()
                .unwrap();
            if recipe.id == "revolute_joint" {
                assert_eq!(
                    props.keys().map(String::as_str).collect::<Vec<_>>(),
                    ["angle_deg", "decline", "hole_component", "pin_component"]
                );
            }
            if recipe.id == "joint" {
                assert!(props.contains_key("joint_type"));
                assert!(props.contains_key("limit5_max"));
                assert!(!props.contains_key("pin_component"));
                assert!(!props.contains_key("angle_deg"));
            }
        }
    }
    #[test]
    fn pattern_fills_accept_model_axes_names_and_target_keys() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let catalog = Catalog::bundled().unwrap();
        let recipe = library
            .find("linear pattern", &crate::recipe_library::Bm25)
            .unwrap()["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "linear_pattern")
            .unwrap()
            .clone();
        let mut request = request("copy this part");
        request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"bodies":[{"name":"Body1","volume_mm3":1000,"bbox":{"min":[0,0,0],"max":[10,10,10]}}],"timeline":[{"id":1,"name":"Box1","type":"BoxFeature","error":null}]}).to_string()}));
        request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"id":1,"name":"Box1","type":"BoxFeature","error":null,"definition":{"operation":"new_body","corner":[0,0,0],"length":"10","width":"10","height":"10"}}).to_string()}));
        for extra in [
            json!({"target":"part","body":"10x10x10 Cube"}),
            json!({"target":"post"}),
            json!({"last_feature":true}),
            json!({"body":true}),
            json!({"target":"feature","body":null}),
        ] {
            for direction in [
                json!([1, 0, 0]),
                json!({"x":1}),
                json!("positive X"),
                json!([-1, 0, 0]),
            ] {
                let mut args = json!({"count":3,"spacing":20,"direction":direction});
                args.as_object_mut()
                    .unwrap()
                    .extend(extra.as_object().unwrap().clone());
                let response = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill","arguments":args.to_string()}}]}}]});
                let (_, audit) = coerced(&response, &recipe, &request).unwrap();
                assert!(audit
                    .iter()
                    .any(|c| c["field"] == "body" && c["after"] == "Body1"));
                let Outcome::Build(envelope, plan) =
                    accept(&response, &recipe, &request, &library, &catalog)
                else {
                    panic!("rejected {args}");
                };
                assert_eq!(envelope["args"]["body"], "Body1");
                assert_eq!(plan.rule, "linear_pattern");
                assert_eq!(
                    plan.calls[0]["arguments"]["params"]["dir1"][0],
                    if direction == json!([-1, 0, 0]) {
                        -1.
                    } else {
                        1.
                    }
                );
            }
        }
    }

    #[test]
    fn omitted_joint_angle_uses_one_request_angle_and_audits_inference() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let recipe = library
            .recipes
            .iter()
            .find(|r| r.id == "revolute_joint")
            .unwrap();
        let recipe =
            json!({"id":recipe.id,"schema":crate::recipe_library::forms::flat(&recipe.schema)});
        for (text, angle) in [
            ("pivot at 90°", 90.),
            ("pivot 90 deg", 90.),
            ("pivot a quarter turn", 90.),
            ("pivot at 30 degrees.", 30.),
            ("pivot -45 degrees", -45.),
        ] {
            let response = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill","arguments":"{\"hole_component\":\"Lever\",\"pin_component\":\"Pedestal\"}"}}]}}]});
            let (converted, changes) = coerced(&response, &recipe, &request(text)).unwrap();
            let args: Value = serde_json::from_str(
                converted["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(args["angle_deg"], angle);
            assert!(changes.iter().any(|c| c["field"] == "angle_deg"
                && c["defaulted"] == true
                && c["after"] == angle));
        }
    }

    #[test]
    fn clean_shapes_keep_only_client_inference_settings_and_latest_user() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let mut req = request("old user");
        req["messages"].as_array_mut().unwrap().extend([
            json!({"role":"system","content":"Always call batch"}),
            json!({"role":"assistant","content":"old answer"}),
            json!({"role":"user","content":"Extrude an L profile 47 wide 33 tall legs 5 thick depth 22"})]);
        req["model"] = json!("client-model");
        req["temperature"] = json!(0.73);
        req["top_p"] = json!(0.91);
        req["options"] = json!({"num_ctx":8192});
        req["tools"] = json!([{"function":{"name":"batch"}}]);
        req["response_format"] = json!({"type":"json_object"});
        let found = select(&library, &req).unwrap().unwrap();
        for route in [false, true] {
            let mut out = req.clone();
            if route {
                offer_route(&mut out, std::slice::from_ref(&found)).unwrap();
            } else {
                offer(&mut out, &found, None).unwrap();
            }
            assert_eq!(out["messages"].as_array().unwrap().len(), 2);
            assert_eq!(out["messages"][0]["role"], "system");
            assert_eq!(out["messages"][1], req["messages"][3]);
            assert_eq!(out["tools"].as_array().unwrap().len(), 1);
            assert!(!out.to_string().contains("batch"));
            assert!(out.get("response_format").is_none());
            for key in ["model", "temperature", "top_p", "options"] {
                assert_eq!(out[key], req[key]);
            }
        }
    }

    #[test]
    fn inspected_context_is_only_for_target_recipes() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let mut req = request("fillet all the edges");
        req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"body_count":1,"bodies":[{"name":"Part"}],"timeline":[]}).to_string()}));
        let recipe = select(&library, &req).unwrap().unwrap();
        let creation = library
            .find("L profile", &crate::recipe_library::Bm25)
            .unwrap()["recipes"][0]
            .clone();
        let mut creation_out = req.clone();
        offer(&mut creation_out, &creation, None).unwrap();
        assert_eq!(
            creation_out["messages"][1]["content"],
            req["messages"][0]["content"]
        );
        let mut route = req.clone();
        offer_route(&mut route, std::slice::from_ref(&recipe)).unwrap();
        assert!(route["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("Design context: "));
        let mut out = req.clone();
        offer(&mut out, &recipe, None).unwrap();
        assert_eq!(out["messages"][1], route["messages"][1]);
        req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"components":[{"name":"Pin"},{"name":"Bracket"}]}).to_string()}));
        let joint = library
            .find("revolute joint", &crate::recipe_library::Bm25)
            .unwrap()["recipes"][0]
            .clone();
        assert_eq!(joint["id"], "revolute_joint");
        offer(&mut req, &joint, None).unwrap();
        assert!(req["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("\"name\":\"Pin\""));
    }

    #[test]
    fn freshness_depends_only_on_history_after_the_latest_user() {
        let mut req = request("make a profile");
        for _ in 0..3 {
            assert!(fresh_turn(&req).unwrap());
        }
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"system","content":"updated instructions"}));
        assert!(fresh_turn(&req).unwrap());
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","content":"Working on it"}));
        for _ in 0..3 {
            assert!(!fresh_turn(&req).unwrap());
        }
        // A repeated user message finishes the prior turn, even at the same text.
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"make a profile"}));
        assert!(fresh_turn(&req).unwrap());
        assert!(!fresh_turn(&json!({"messages":[]})).unwrap());
        assert!(!fresh_turn(&json!({"messages":[{"role":"system"}]})).unwrap());
        assert!(fresh_turn(&json!({})).is_err());
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
            assert!(!fresh_turn(&req).unwrap());
            // Trimming the assistant call still leaves a tool result in progress.
            req["messages"][1] = json!({"role":"tool","content":"{}"});
            assert!(!fresh_turn(&req).unwrap());
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
        assert!(retry["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains(&error));
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

    #[test]
    fn implicit_form_targets_default_from_selection_and_reject_ambiguity() {
        let library = crate::recipe_library::Library::embedded().unwrap();
        let catalog = Catalog::bundled().unwrap();
        let mut req = request("fillet all edges of these with radius 0.5 mm");
        req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"bodies":[{"name":"Plate"},{"name":"Peg"}],"timeline":[]}).to_string()}));
        let recipe = select(&library, &req).unwrap().unwrap();
        assert_eq!(recipe["id"], "fillet");
        let response = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"pilot_fill","arguments":"{\"amount\":0.5}"}}]}}]});
        assert!(
            matches!(accept(&response,&recipe,&req,&library,&catalog),Outcome::Invalid(e) if e.contains("ambiguous"))
        );
        req["selection"] = json!(["Plate", "Peg"]);
        assert!(
            matches!(accept(&response,&recipe,&req,&library,&catalog),Outcome::Invalid(e) if e.contains("ambiguous"))
        );
        req["selection"] = json!(["Peg"]);
        let Outcome::Build(envelope, _) = accept(&response, &recipe, &req, &library, &catalog)
        else {
            panic!("selected target not defaulted");
        };
        assert_eq!(envelope["args"]["body"], "Peg");
    }
}
