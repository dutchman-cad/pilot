//! Bounded recipe plans derived from user instructions and public tool results.
#[cfg(test)]
pub(crate) mod cases;
#[cfg(test)]
mod english;
#[cfg(test)]
mod first_turn;
#[cfg(feature = "proxy")]
pub(crate) mod form_context;
mod honeycomb;
pub(crate) mod joints;
pub(crate) mod patterns;
mod recipes;
#[cfg(test)]
mod robust;
mod slots;
use crate::{catalog::Catalog, guidance, repair};
pub use honeycomb::schema as honeycomb_schema;
pub use patterns::{intent as pattern_intent, repetition_hint, schema as pattern_schema};
use serde_json::{json, Value};
use slots::{dimensions, intent_tokens, labelled, tokens};

/// Search cue only: dimensions still require exact parsing or a model fill.
pub(crate) fn angle_profile_notation(text: &str) -> bool {
    if text.len() > 2048 {
        return false;
    }
    let t: Vec<_> = slots::angle_tokens(&intent_tokens(text))
        .into_iter()
        .filter(|w| slots::scale(w).is_none())
        .collect();
    // "at an angle of 45 degrees" is a rotation, not an angle section.
    let rotation = t.iter().any(|w| {
        matches!(
            w.as_str(),
            "degree" | "degrees" | "deg" | "tilted" | "rotated"
        )
    }) || t.windows(2).any(|w| w[0] == "angle" && w[1] == "of");
    !rotation
        && t.iter().any(|w| matches!(w.as_str(), "l" | "angle"))
        && t.windows(5).any(|w| {
            [0, 2, 4]
                .iter()
                .all(|n| slots::number(&w[*n]).is_some_and(|v| v.is_finite() && v > 0.0))
                && matches!(w[1].as_str(), "x" | "by")
                && matches!(w[3].as_str(), "x" | "by")
        })
}

const MAX_TEXT: usize = 4096;
const MAX_BODIES: usize = 1024;
const MAX_MM: f64 = 1_000_000.0;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Expected {
    pub bodies: Option<usize>,
    pub volume_mm3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_range_mm3: Option<(f64, f64)>,
    pub name: Option<String>,
    pub created_volume_mm3: Option<f64>,
    pub placement: Option<Placement>,
    pub feature: Option<(String, usize)>,
    pub joint: Option<recipes::revolute::JointExpected>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Placement {
    pub body: String,
    pub min: [f64; 3],
    pub max: [f64; 3],
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Plan {
    pub rule: &'static str,
    pub calls: Vec<Value>,
    pub facts: Option<Value>,
    pub slots: Value,
    pub expected: Option<Expected>,
    pub continuation: Option<EdgeContinuation>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct EdgeContinuation {
    pub body: String,
    pub amount: f64,
    pub fillet: bool,
    pub top: bool,
}
impl Plan {
    pub fn message(&self) -> String {
        let mut message = format!("For this request, run exactly this plan: {}. Don't change it; then reply using the tool results. Slots are converted to mm (including inches and fractions): {}.", json!(self.calls), self.slots);
        if self.slots["CompoundPlate"]["default_start"] == true {
            message.push_str(
                " Tell the user the bolt circle starts at +X because no start angle was specified.",
            );
        }
        if let Some(facts) = &self.facts {
            message.push_str(&format!(
                " Computed from the latest tool result: {facts}. Use these exact facts."
            ));
        }
        if self.expected.is_some() {
            message.push_str(" After the changes, call execute document.inspect with {\"measure\":true} before your final reply to verify completion.");
        }
        message
    }
}
/// A narrow action pledge. Recipe slots always come from the user, never this prose.
pub fn action_prose(message: &Value) -> bool {
    if message["refusal"].as_str().is_some_and(|s| !s.is_empty()) {
        return false;
    }
    let Some(text) = message["content"].as_str().filter(|s| s.len() <= MAX_TEXT) else {
        return false;
    };
    let lower = text.trim().to_ascii_lowercase();
    (lower.starts_with("i will ")
        || lower.starts_with("i'll ")
        || lower.starts_with("let me ")
        || lower.starts_with("i am going to "))
        && !tokens(&lower).iter().any(|s| {
            matches!(
                s.as_str(),
                "not" | "don" | "cannot" | "cant" | "unable" | "if" | "instead"
            )
        })
}
pub fn matches_prose(
    request: &Value,
    catalog: &Catalog,
    expected: &Plan,
    message: &Value,
) -> Result<bool, String> {
    if !action_prose(message) {
        return Ok(false);
    }
    let mut request = request.clone();
    let Some(messages) = request["messages"].as_array_mut() else {
        return Ok(false);
    };
    let Some(index) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(false);
    };
    if expected.rule != "revolute_joint" {
        messages.truncate(index + 1);
    }
    let user = &mut messages[index];
    let text = message["content"].as_str().unwrap_or("").trim();
    let lower = text.to_ascii_lowercase();
    let prefixes = ["i will ", "i'll ", "let me ", "i am going to "];
    let Some(prefix) = prefixes.iter().find(|p| lower.starts_with(**p)) else {
        return Ok(false);
    };
    user["content"] = json!(&text[prefix.len()..]);
    let Some(mut actual) = plan(&request, catalog)? else {
        return Ok(false);
    };
    let mut expected_slots = expected.slots.clone();
    for value in [&mut actual.slots, &mut expected_slots] {
        if let Some(edges) = value.get_mut("Edges").and_then(Value::as_object_mut) {
            edges.remove("edges");
        }
    }
    Ok(actual.rule == expected.rule && actual.slots == expected_slots)
}
/// Continue an already fired edge recipe after its read-only topology lookup.
pub fn resume(request: &Value, previous: &Plan, catalog: &Catalog) -> Result<Option<Plan>, String> {
    recipes::resume(request, previous, catalog)
}
fn tool_payload(message: &Value) -> Option<Value> {
    let content = message.get("content")?;
    let data = if let Some(s) = content.as_str() {
        if s.len() > repair::MAX_BYTES {
            return None;
        }
        let marker = format!("\n{}", crate::identity::HINT);
        serde_json::from_str(s.split_once(&marker).map_or(s, |(s, _)| s)).ok()?
    } else {
        content.clone()
    };
    repair::check_value(&data).ok()?;
    if data.get("isError") == Some(&json!(true))
        || data.get("error").is_some_and(|e| !e.is_null())
        || data["ok"] == false
    {
        return None;
    }
    let data = guidance::payload(&data);
    if data.get("isError") == Some(&json!(true))
        || data.get("error").is_some_and(|e| !e.is_null())
        || data["ok"] == false
        || !guidance::paging(&data).is_empty()
    {
        return None;
    }
    Some(data)
}
fn state(messages: &[Value]) -> Option<Value> {
    let mut found = None;
    for message in messages.iter().rev().filter(|m| m["role"] == "tool") {
        let data = tool_payload(message)?;
        if [
            "created_bodies",
            "changed_bodies",
            "removed_bodies",
            "feature",
        ]
        .iter()
        .any(|key| data.get(key).is_some())
        {
            return None;
        }
        if data["definition"].is_object()
            || data["components"].is_array()
            || data["joints"].is_array()
            || data["faces"].is_array()
            || data["edges"].is_array()
        {
            continue;
        }
        found = Some(data);
        break;
    }
    let data = found?;
    let bodies = data.get("bodies")?.as_array()?;
    if bodies.len() > MAX_BODIES
        || bodies.iter().any(|b| !b.is_object())
        || data
            .get("body_count")
            .is_some_and(|n| n.as_u64() != Some(bodies.len() as u64))
    {
        return None;
    }
    Some(data)
}
/// Names from the same complete latest snapshot used by recipe expansion.
#[cfg(feature = "proxy")]
pub(crate) fn recipe_targets(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .and_then(|ms| state(ms))
        .into_iter()
        .flat_map(|data| data["bodies"].as_array().cloned().unwrap_or_default())
        .filter_map(|body| {
            body["name"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .map(String::from)
        })
        .collect()
}
fn body(text: &str, data: Option<&Value>) -> Option<String> {
    let bodies = data?.get("bodies")?.as_array()?;
    let lower = text.to_ascii_lowercase();
    let names: Vec<_> = bodies
        .iter()
        .filter_map(|b| b["name"].as_str())
        .filter(|n| !n.is_empty() && n.len() <= 256)
        .collect();
    let found: Vec<_> = names
        .iter()
        .filter(|n| {
            let name = n.to_ascii_lowercase();
            if matches!(name.as_str(), "it" | "the body" | "the box" | "the part") {
                return false;
            }
            lower.match_indices(&name).any(|(i, _)| {
                if matches!(name.as_str(), "body" | "box" | "part") {
                    let prefix = tokens(&lower[..i]);
                    if prefix
                        .last()
                        .is_some_and(|s| matches!(s.as_str(), "the" | "current" | "a"))
                    {
                        return false;
                    }
                }
                lower[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                    && lower[i + name.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_')
            })
        })
        .collect();
    if found.len() == 1 {
        return Some(found[0].to_string());
    }
    // Pronouns only; never silently substitute an unknown explicit name.
    let t = intent_tokens(text);
    if found.is_empty()
        && t.iter().position(|s| s == "of").is_some_and(|i| {
            t.get(i + 1).is_some_and(|s| {
                !matches!(s.as_str(), "the" | "current" | "body" | "it")
                    && dimensions(std::slice::from_ref(s)).is_none_or(|d| d.len() != 1)
            })
        })
    {
        return None;
    }
    let verb = t.iter().position(|s| {
        matches!(
            s.as_str(),
            "move"
                | "translate"
                | "shift"
                | "slide"
                | "rename"
                | "call"
                | "name"
                | "mirror"
                | "scale"
                | "double"
                | "half"
                | "fillet"
                | "round"
                | "chamfer"
                | "bevel"
                | "drill"
                | "bore"
                | "cut"
        )
    });
    if found.is_empty()
        && bodies.len() == 1
        && names.len() == 1
        && verb.is_some_and(|i| {
            let rest = &t[i + 1..];
            let rest = if t[i] == "round" && rest.first().is_some_and(|s| s == "off") {
                &rest[1..]
            } else {
                rest
            };
            let rest = &rest[rest
                .iter()
                .take_while(|s| matches!(s.as_str(), "the" | "current" | "a"))
                .count()..];
            rest.first().is_some_and(|s| {
                matches!(
                    s.as_str(),
                    "it" | "body" | "box" | "part" | "all" | "top" | "edges"
                ) || (s == "size" && matches!(t[i].as_str(), "scale" | "double" | "half"))
            }) && !rest.windows(2).any(|w| {
                matches!(w[0].as_str(), "body" | "box" | "part")
                    && !matches!(
                        w[1].as_str(),
                        "to" | "by"
                            | "about"
                            | "across"
                            | "along"
                            | "on"
                            | "with"
                            | "all"
                            | "top"
                            | "radius"
                            | "origin"
                            | "uniformly"
                            | "in"
                            | "distance"
                    )
                    && !w[1].starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.')
            })
        })
    {
        return Some(names[0].to_string());
    }
    None
}
fn call(catalog: &Catalog, id: &str, params: Value) -> Result<Value, String> {
    let command = catalog
        .get(id)
        .ok_or_else(|| format!("planner command missing from catalogue: {id}"))?;
    let documented: Vec<_> = command
        .params
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .collect();
    for key in params
        .as_object()
        .ok_or("planner parameters must be an object")?
        .keys()
    {
        if !documented.contains(&key.as_str()) {
            return Err(format!(
                "planner parameter {id}.{key} missing from catalogue"
            ));
        }
    }
    Ok(json!({"tool":"execute","arguments":{"command":id,"params":params}}))
}
fn volume(data: &Value) -> Option<f64> {
    let bodies = data["bodies"].as_array()?;
    let mut total = 0.0;
    for b in bodies {
        let n = b.get("volume_mm3").or_else(|| b.get("volume"))?.as_f64()?;
        if !n.is_finite() || n < 0.0 {
            return None;
        }
        total += n;
    }
    if data
        .get("total")
        .and_then(|t| t.get("volume_mm3"))
        .is_some_and(|v| {
            v.as_f64().is_none_or(|v| {
                !v.is_finite() || (v - total).abs() > 0.01_f64.max(total.abs() * 1e-9)
            })
        })
    {
        return None;
    }
    total.is_finite().then_some(total)
}
/// Compare only a complete latest snapshot. Missing measurements request one lookup.
pub fn completion(request: &Value, expected: &Expected) -> Option<String> {
    if let Some(joint) = &expected.joint {
        if let Some(reason) = joint.check(request) {
            return Some(reason);
        }
    }
    let messages = request["messages"].as_array()?;
    let Some(data) = state(messages) else {
        return Some("Recipe completion is unverified: call execute document.inspect with {\"measure\":true}, then finish or explain the difference. Do not repeat accepted changes.".into());
    };
    let mut differences = Vec::new();
    if let Some(count) = expected.bodies {
        let actual = data["bodies"].as_array()?.len();
        if count != actual {
            differences.push(format!("expected {count} bodies, observed {actual}"));
        }
    }
    if let Some(want) = expected.volume_mm3 {
        match volume(&data) {
            Some(actual) if (want - actual).abs() <= (want.abs() * 0.001).max(0.01) => (),
            Some(actual) => differences.push(format!(
                "expected volume {want:.6} mm³, observed {actual:.6} mm³"
            )),
            None => differences.push("volume measurement is missing".into()),
        }
    }
    if let Some((min, max)) = expected.volume_range_mm3 {
        if volume(&data).is_none_or(|v| v < min || v > max) {
            differences.push(format!(
                "pattern volume must be measured within {min:.6}..{max:.6} mm³"
            ));
        }
    }
    if let Some(want) = expected.created_volume_mm3 {
        let bodies = data["bodies"].as_array()?;
        let selected: Vec<_> = bodies
            .iter()
            .filter(|b| expected.name.as_ref().is_none_or(|n| b["name"] == *n))
            .cloned()
            .collect();
        let measured = volume(&json!({"bodies":selected}));
        if selected.len() != 1 {
            differences.push(
                "creation baseline is unavailable; verify the added body's count and volume".into(),
            );
        } else if measured
            .is_none_or(|actual| (want - actual).abs() > (want.abs() * 0.001).max(0.01))
        {
            differences.push(format!(
                "expected created body volume {want:.6} mm³, observed {}",
                measured.map_or_else(|| "unmeasured".into(), |v| format!("{v:.6} mm³"))
            ));
        }
    }
    if let Some(name) = &expected.name {
        if !data["bodies"]
            .as_array()?
            .iter()
            .any(|b| b["name"] == *name)
        {
            differences.push(format!("expected body named {name}"));
        }
    }
    if let Some(p) = &expected.placement {
        let found = data["bodies"]
            .as_array()?
            .iter()
            .find(|b| b["name"] == p.body);
        let matches = found.is_some_and(|b| {
            (0..3).all(|i| {
                b["bbox"]["min"][i]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (n - p.min[i]).abs() <= 1e-6)
                    && b["bbox"]["max"][i]
                        .as_f64()
                        .is_some_and(|n| n.is_finite() && (n - p.max[i]).abs() <= 1e-6)
            })
        });
        if !matches {
            differences.push(format!(
                "expected {} bounds {:?} to {:?}; observed bounds {}",
                p.body,
                p.min,
                p.max,
                found.map_or(&Value::Null, |b| &b["bbox"])
            ));
        }
    }
    if let Some((kind, want)) = &expected.feature {
        if kind.ends_with("PatternFeature") && volume(&data).is_none_or(|v| v <= 0.0) {
            differences.push("pattern volume measurement is missing or nonpositive".into());
        }
        let actual = data["timeline"]
            .as_array()
            .filter(|t| t.len() <= 4096)
            .map(|t| {
                t.iter()
                    .filter(|f| {
                        f["type"] == *kind
                            && f["error"].is_null()
                            && f["rolled_back"] != true
                            && f["suppressed"] != true
                    })
                    .count()
            });
        if actual != Some(*want) {
            differences.push(format!(
                "expected {want} successful {kind} features, observed {}",
                actual.map_or_else(|| "unverified".into(), |n| n.to_string())
            ));
        }
    }
    (!differences.is_empty()).then(|| format!("Recipe completion differs: {}. Inspect with measure:true and finish the remaining work or explain the difference. Do not repeat accepted changes.", differences.join("; ")))
}

const REPORT_GUARD_WORDS: &[&str] = &[
    "not",
    "no",
    "avoid",
    "skip",
    "stop",
    "neither",
    "nor",
    "explanation",
    "describe",
    "tutorial",
    "instructions",
    "don",
    "dont",
    "never",
    "explain",
    "would",
    "what",
    "how",
    "why",
    "whether",
    "example",
    "if",
    "or",
    "instead",
    "except",
    "without",
    "undo",
    "cannot",
    "cant",
];

pub fn plan(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    plan_impl(request, catalog, false)
}
/// Direct mode may continue after its own read-only body prerequisite.
pub fn plan_direct(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    plan_impl(request, catalog, true)
}
fn plan_impl(
    request: &Value,
    catalog: &Catalog,
    allow_inspection: bool,
) -> Result<Option<Plan>, String> {
    repair::check_value(request)?;
    let Some(messages) = request["messages"].as_array() else {
        return Ok(None);
    };
    if messages.len() > 4096 {
        return Err("planner conversation exceeds 4096 messages".into());
    }
    let Some(user_index) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(None);
    };
    let Some(text) = messages[user_index]["content"]
        .as_str()
        .filter(|s| s.len() <= MAX_TEXT)
    else {
        return Ok(None);
    };
    if !request["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|t| t["function"]["name"] == "execute"))
    {
        return Ok(None);
    }
    let polite_question = text.to_ascii_lowercase().contains("can you ")
        || text.to_ascii_lowercase().contains("could you ")
        || text.to_ascii_lowercase().contains("would you ");
    let normalized = slots::normalize(text);
    let text = normalized.as_str();
    if recipes::revolute::candidate(text) {
        return recipes::revolute::select(request, text, catalog);
    }
    let t = intent_tokens(text);
    let has = |w: &str| t.iter().any(|s| s == w);
    let data = state(messages);
    let query_text = t
        .iter()
        .filter(|s| s.as_str() != "and")
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let count = query_text.contains("how many bodies")
        || query_text.contains("how many solid bodies")
        || query_text.contains("how many parts")
        || query_text.contains("count the bodies")
        || query_text.contains("count bodies");
    let query = (has("volume") || (has("how") && has("many") && (has("cubic") || has("mm3"))))
        && ["what", "tell", "measure", "report", "give", "volume", "how"]
            .iter()
            .any(|w| has(w));
    if count && query {
        return Ok(None);
    }
    if count || query {
        if t.windows(2).any(|w| {
            matches!(w[0].as_str(), "a" | "an")
                && matches!(
                    w[1].as_str(),
                    "cube"
                        | "box"
                        | "block"
                        | "plate"
                        | "sphere"
                        | "cylinder"
                        | "rod"
                        | "disc"
                        | "tube"
                )
        }) {
            return Ok(None);
        }
        if has("how") && !query_text.contains("how many") {
            return Ok(None);
        }
        if [
            "make",
            "draw",
            "create",
            "build",
            "add",
            "sketch",
            "fillet",
            "chamfer",
            "move",
            "translate",
            "rename",
            "mirror",
            "shell",
            "revolve",
            "extrude",
            "round",
            "bevel",
            "drill",
            "bore",
            "cut",
            "shift",
            "slide",
            "call",
            "name",
            "not",
            "no",
            "avoid",
            "skip",
            "stop",
            "neither",
            "nor",
            "explanation",
            "describe",
            "tutorial",
            "instructions",
            "don",
            "dont",
            "cannot",
            "cant",
            "discard",
            "without",
            "never",
            "why",
            "explain",
            "instead",
            "except",
            "undo",
            "scale",
            "double",
            "half",
        ]
        .iter()
        .any(|w| has(w))
        {
            return Ok(None);
        }
        let rule = if count { "body_count" } else { "volume" };
        let mut facts = None;
        if let Some(data) = &data {
            let bodies = data["bodies"]
                .as_array()
                .ok_or("planner bodies must be an array")?;
            if count {
                facts = Some(json!({"body_count":bodies.len()}));
            } else {
                let named = body(text, Some(data));
                if (has("of") || has("for")) && !has("design") && !has("all") {
                    let reference = t
                        .iter()
                        .position(|s| matches!(s.as_str(), "of" | "for"))
                        .map(|i| &t[i + 1..])
                        .unwrap_or(&[]);
                    let qualifiers = reference
                        .iter()
                        .take_while(|s| matches!(s.as_str(), "the" | "current" | "existing"))
                        .count();
                    let reference = &reference[qualifiers..];
                    let generic = reference
                        .first()
                        .is_some_and(|s| matches!(s.as_str(), "body" | "solid" | "part" | "it"))
                        && reference.get(1).is_none_or(|s| {
                            matches!(s.as_str(), "in" | "leaving" | "unchanged" | "as")
                        });
                    if (named.is_none() && (!generic || bodies.len() != 1))
                        || (generic && qualifiers > 0 && bodies.len() != 1)
                    {
                        return Ok(None);
                    }
                }
                let selected: Vec<_> = bodies
                    .iter()
                    .filter(|b| named.as_ref().is_none_or(|n| b["name"] == *n))
                    .cloned()
                    .collect();
                if let Some(total) = volume(data).and_then(|_| volume(&json!({"bodies":selected})))
                {
                    facts = Some(
                        json!({"volume_mm3":total,"body_count":bodies.len(),"bodies":selected.iter().filter_map(|b| b["name"].as_str()).collect::<Vec<_>>()}),
                    );
                }
            }
        }
        let mut calls = Vec::new();
        if !count && volume_unit(text).is_none() {
            return Ok(None);
        }
        if facts.is_none() {
            if messages[user_index + 1..]
                .iter()
                .any(|m| m["tool_calls"].as_array().is_some_and(|c| !c.is_empty()))
            {
                return Ok(None);
            }
            calls.push(call(catalog, "document.inspect", json!({"measure":true}))?);
        }
        return Ok(Some(Plan {
            rule,
            calls,
            facts,
            slots: json!({"query":rule}),
            expected: None,
            continuation: None,
        }));
    }
    let acted = messages[user_index + 1..].iter().any(|m| {
        m["tool_calls"].as_array().is_some_and(|calls| {
            calls.iter().any(|c| {
                matches!(c["function"]["name"].as_str(), Some("execute" | "batch"))
                    && !(allow_inspection && body_inspection(c))
            })
        })
    });
    if acted || REPORT_GUARD_WORDS.iter().any(|w| has(w)) {
        return Ok(None);
    }
    if slots::has_question(text) && !polite_question {
        return Ok(None);
    }
    if honeycomb::candidate(text) {
        return honeycomb::select(text, request, catalog);
    }
    recipes::select(text, data.as_ref(), messages, catalog)
}
/// A read-only prerequisite is offered only when every edit slot already parses.
/// A hypothetical sole body checks syntax; its name never reaches a mutation.
pub fn inspect_for_edit(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    repair::check_value(request)?;
    let Some(messages) = request["messages"].as_array() else {
        return Ok(None);
    };
    if messages.len() > 4096 || state(messages).is_some() {
        return Ok(None);
    }
    let Some(index) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(None);
    };
    if messages[index + 1..]
        .iter()
        .any(|m| m["tool_calls"].as_array().is_some_and(|c| !c.is_empty()))
    {
        return Ok(None);
    }
    let mut probe = request.clone();
    let mut synthetic = vec![
        json!({"role":"tool","content":json!({"bodies":[{"name":"PilotUnresolvedBody"}]}).to_string()}),
    ];
    synthetic.extend(messages.iter().cloned());
    probe["messages"] = json!(synthetic);
    let Some(candidate) = plan(&probe, catalog)? else {
        return Ok(None);
    };
    if !matches!(
        candidate.rule,
        "move" | "rename" | "fillet" | "chamfer" | "mirror" | "scale" | "hole"
    ) {
        return Ok(None);
    }
    Ok(Some(Plan {
        rule: "resolve_body",
        calls: vec![call(catalog, "document.inspect", json!({"measure":true}))?],
        facts: None,
        slots: json!({"prerequisite":"unique body"}),
        expected: None,
        continuation: None,
    }))
}
fn body_inspection(call: &Value) -> bool {
    call["id"]
        .as_str()
        .is_some_and(|id| id.starts_with("pilot_direct_") && id.ends_with("_resolve_body_0"))
        && call["function"]["name"] == "execute"
        && call["function"]["arguments"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            == Some(json!({"command":"document.inspect","params":{"measure":true}}))
}
/// Names come from the same bounded table that selects recipes.
pub fn supported_recipes() -> Vec<&'static str> {
    recipes::names()
        .chain(["honeycomb pattern", "body count", "volume"])
        .collect()
}
#[cfg(feature = "proxy")]
pub(crate) fn joint_hint(request: &Value) -> Option<&'static str> {
    recipes::revolute::result_hint(request).then_some(recipes::revolute::JOINT_HINT)
}
/// Report measured facts only after the ordinary completion check succeeds.
pub fn direct_summary(request: &Value, plan: &Plan) -> String {
    if let Some(facts) = &plan.facts {
        if plan.rule == "body_count" {
            return format!("Exact match (no model): {} bodies", facts["body_count"]);
        }
        if plan.rule == "volume" {
            let text = request["messages"]
                .as_array()
                .and_then(|m| m.iter().rev().find(|m| m["role"] == "user"))
                .and_then(|m| m["content"].as_str())
                .unwrap_or("");
            if let (Some(value), Some((unit, divisor))) =
                (facts["volume_mm3"].as_f64(), volume_unit(text))
            {
                let conversion = if divisor == 1.0 {
                    String::new()
                } else {
                    format!(" (converted from {value} mm³; 1 {unit} = {divisor} mm³)")
                };
                return format!(
                    "Exact match (no model): volume {} {unit}{conversion}",
                    value / divisor
                );
            }
        }
    }
    let mut text = format!(
        "Exact match (no model): {} completed.",
        plan.rule.replace('_', " ")
    );
    if plan.slots["CompoundPlate"]["default_start"] == true {
        text.push_str(" Bolt circle starts at +X (no start angle was specified).");
    }
    if let Some(data) = request["messages"].as_array().and_then(|m| state(m)) {
        if let Some(bodies) = data["bodies"].as_array() {
            text.push_str(&format!(" Measured {} bodies.", bodies.len()));
            if let Some(total) = volume(&data) {
                text.push_str(&format!(" Total volume {} mm³.", rounded(total, 0)));
            }
            for body in bodies.iter().take(8) {
                if let (Some(min), Some(max)) = (
                    body["bbox"]["min"].as_array(),
                    body["bbox"]["max"].as_array(),
                ) {
                    if min.len() == 3 && max.len() == 3 {
                        let size: Option<Vec<f64>> = (0..3)
                            .map(|i| Some(max[i].as_f64()? - min[i].as_f64()?))
                            .collect();
                        if let Some(size) =
                            size.filter(|s| s.iter().all(|n| n.is_finite() && *n >= 0.0))
                        {
                            text.push_str(&format!(
                                " Measured size {} × {} × {} mm.",
                                rounded(size[0], 3),
                                rounded(size[1], 3),
                                rounded(size[2], 3)
                            ));
                        }
                    }
                }
            }
        }
    }
    if let Some(facts) = &plan.facts {
        text.push_str(&format!(" Measured facts: {facts}."));
    }
    text.push_str(" Length slots use mm (1 inch = 25.4 mm; 1 cm = 10 mm; 1 m = 1000 mm).");
    text
}

/// A measured value for the reply text: rounded so float noise such as 6.000000000000004 reads 6.
fn rounded(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    let value = (value * scale).round() / scale;
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

/// Cubic units are explicit and bounded by the request size. Conflicting units defer.
fn volume_unit(text: &str) -> Option<(&'static str, f64)> {
    let lower = text.to_ascii_lowercase().replace('³', "3").replace('^', "");
    let mut found = None;
    for (unit, divisor, aliases) in [
        (
            "mm³",
            1.0,
            &[
                "mm3",
                "cubic millimetres",
                "cubic millimeters",
                "cubic millimetre",
                "cubic millimeter",
            ][..],
        ),
        (
            "cm³",
            1000.0,
            &[
                "cm3",
                "cubic centimetres",
                "cubic centimeters",
                "cubic centimetre",
                "cubic centimeter",
            ][..],
        ),
        (
            "m³",
            1_000_000_000.0,
            &[
                "m3",
                "cubic metres",
                "cubic meters",
                "cubic metre",
                "cubic meter",
            ][..],
        ),
        (
            "in³",
            16_387.064,
            &["in3", "cubic inches", "cubic inch"][..],
        ),
        (
            "ft³",
            28_316_846.592,
            &["ft3", "cubic feet", "cubic foot"][..],
        ),
    ] {
        if aliases.iter().any(|alias| {
            lower.match_indices(*alias).any(|(at, _)| {
                lower[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_ascii_alphabetic())
                    && lower[at + alias.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_ascii_alphabetic())
            })
        }) {
            if found.is_some() {
                return None;
            }
            found = Some((unit, divisor));
        }
    }
    if found.is_none()
        && (lower.contains("cubic") || lower.contains("cubed") || text.contains(['³', '^']))
    {
        return None;
    }
    Some(found.unwrap_or(("mm³", 1.0)))
}

/// Offer a fully matched leading clause, leaving compound work to the model.
pub fn partial_plan(request: &Value, catalog: &Catalog) -> Result<Option<(Plan, String)>, String> {
    let Some(messages) = request["messages"].as_array() else {
        return Ok(None);
    };
    let Some(index) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(None);
    };
    if index + 1 != messages.len() {
        return Ok(None);
    }
    let Some(text) = messages[index]["content"]
        .as_str()
        .filter(|s| s.len() <= MAX_TEXT)
    else {
        return Ok(None);
    };
    // Quoted destinations are indivisible name data.
    if text.contains(['"', '\'']) {
        return Ok(None);
    }
    let lower = text.to_ascii_lowercase();
    for separator in [" and then ", " and also ", " and ", "; ", " then "] {
        for (at, _) in lower.match_indices(separator).take(32) {
            let rest = text[at + separator.len()..].trim();
            if rest.is_empty() {
                continue;
            }
            let mut prefix = request.clone();
            prefix["messages"][index]["content"] = json!(text[..at].trim());
            if let Some(plan) = plan(&prefix, catalog)? {
                return Ok(Some((plan, rest.to_string())));
            }
        }
    }
    Ok(None)
}

/// Privacy-safe decline diagnostics derived from the planner's own guards.
#[cfg(feature = "proxy")]
pub fn report_decline(text: &str) -> (&'static str, Vec<String>) {
    if text.len() > MAX_TEXT {
        return ("request exceeds planner text limit", Vec::new());
    }
    let text = report_input_tokens(text)
        .into_iter()
        .filter(|token| !token.starts_with('['))
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = slots::intent_tokens(&text);
    let guarded: Vec<String> = tokens
        .iter()
        .filter(|w| REPORT_GUARD_WORDS.contains(&w.as_str()))
        .cloned()
        .collect();
    if !guarded.is_empty() {
        return ("guarded request", guarded);
    }
    recipes::report_decline(&text)
}
/// Only words in Pilot's fixed planner lists may appear in request diagnostics.
#[cfg(feature = "proxy")]
pub fn report_tokens(text: &str) -> Vec<String> {
    report_input_tokens(text)
        .into_iter()
        .map(|word| {
            if word.starts_with('[')
                || REPORT_GUARD_WORDS.contains(&word.as_str())
                || recipes::UNSUPPORTED.contains(&word.as_str())
                || matches!(
                    word.as_str(),
                    "make"
                        | "draw"
                        | "create"
                        | "build"
                        | "add"
                        | "sketch"
                        | "fillet"
                        | "chamfer"
                        | "move"
                        | "translate"
                        | "rename"
                        | "mirror"
                        | "shell"
                        | "revolve"
                        | "cube"
                        | "box"
                        | "plate"
                        | "sphere"
                        | "cylinder"
                        | "rod"
                        | "disc"
                        | "tube"
                        | "volume"
                        | "count"
                        | "bodies"
                        | "and"
                        | "then"
                        | "hole"
                        | "holes"
                        | "extrude"
                        | "radius"
                        | "diameter"
                        | "height"
                        | "width"
                        | "length"
                        | "mm"
                        | "cm"
                        | "inch"
                        | "inches"
                )
            {
                word
            } else {
                "[word]".into()
            }
        })
        .collect()
}

/// Redaction placeholders are single opaque tokens, including multiword labels.
#[cfg(feature = "proxy")]
fn report_input_tokens(text: &str) -> Vec<String> {
    let mut end = text.len().min(MAX_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut rest = &text[..end];
    let mut tokens = Vec::new();
    while !rest.is_empty() && tokens.len() < 256 {
        let start = rest.find('[').unwrap_or(rest.len());
        tokens.extend(
            slots::intent_tokens(&slots::normalize(&rest[..start]))
                .into_iter()
                .take(256 - tokens.len()),
        );
        rest = &rest[start..];
        if rest.is_empty() || tokens.len() == 256 {
            break;
        }
        if let Some(end) = rest.find(']') {
            let placeholder = &rest[..=end];
            // Only redactor-owned labels may pass through verbatim.
            tokens.push(
                if matches!(
                    placeholder,
                    "[email]"
                        | "[path]"
                        | "[token]"
                        | "[URL]"
                        | "[IP address]"
                        | "[hostname]"
                        | "[environment name]"
                        | "[word]"
                ) {
                    placeholder.into()
                } else {
                    "[word]".into()
                },
            );
            rest = &rest[end + 1..];
        } else {
            // An incomplete placeholder (for example at the capture limit) is opaque.
            tokens.push("[word]".into());
            break;
        }
    }
    tokens
}

/// Typed model shortcut, using the same recipe validators and expanders.
pub fn shortcut(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    repair::check_value(args)?;
    if args.get("recipe").is_some() {
        return crate::recipe_library::Library::embedded()?.build(args, request, catalog);
    }
    recipes::shortcuts::build(args, request, catalog)
}
/// File-backed recipe dispatch into the existing checked expanders.
pub fn recipe_expand(
    id: &str,
    args: &Value,
    request: &Value,
    catalog: &Catalog,
) -> Result<Plan, String> {
    if id == "joint" {
        return joints::build(args, request, catalog);
    }
    if id == "cross_drilled_box" {
        return recipes::cross_drilled::build(args, request, catalog);
    }
    if id == "honeycomb_pattern" {
        return honeycomb::build(args, request, catalog);
    }
    if id.ends_with("_pattern") {
        return patterns::build(id, args, request, catalog);
    }
    recipes::shortcuts::file_build(id, args, request, catalog)
}
pub fn cross_drilled_schema() -> Value {
    recipes::cross_drilled::schema()
}
/// Legacy typed shape schema, also the source of the four original file contracts.
pub fn shortcut_tool() -> Value {
    recipes::shortcuts::tool()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn request(text: &str, state: Option<Value>) -> Value {
        let mut messages = Vec::new();
        if let Some(state) = state {
            messages.push(json!({"role":"tool","content":state.to_string()}));
        }
        messages.push(json!({"role":"user","content":text}));
        json!({"messages":messages,"tools":[{"function":{"name":"execute"}}]})
    }
    #[test]
    fn measured_values_drop_float_noise() {
        assert_eq!(rounded(6.000000000000004, 3).to_string(), "6");
        assert_eq!(rounded(12.7, 3).to_string(), "12.7");
        assert_eq!(rounded(35528.839376, 0).to_string(), "35529");
        assert_eq!(rounded(-0.0001, 3).to_string(), "0");
    }
    #[test]
    fn angle_notation_cue_skips_rotations() {
        assert!(angle_profile_notation("L50x30x5 on XZ, 80 long"));
        assert!(angle_profile_notation("a 40 x 40 x 4 angle"));
        assert!(!angle_profile_notation(
            "a 100 x 50 x 10 block at an angle of 45 degrees"
        ));
        assert!(!angle_profile_notation(
            "100x50x10 plate rotated to an angle"
        ));
        assert!(!angle_profile_notation("a 100 x 50 x 10 block"));
    }
    fn get(text: &str) -> Plan {
        plan(&request(text, None), &Catalog::bundled().unwrap())
            .unwrap()
            .unwrap()
    }
    fn params(p: &Plan, i: usize) -> &Value {
        &p.calls[i]["arguments"]["params"]
    }
    #[test]
    fn volume_lookup_is_planned_once_per_request() {
        let catalog = Catalog::bundled().unwrap();
        let mut req = request("what is the volume?", None);
        assert_eq!(
            plan(&req, &catalog).unwrap().unwrap().calls[0]["arguments"]["command"],
            "document.inspect"
        );
        req["messages"].as_array_mut().unwrap().extend([
            json!({"role":"assistant","tool_calls":[{"id":"1","function":{"name":"inspect","arguments":"{}"}}]}),
            json!({"role":"tool","tool_call_id":"1","content":"{\"bodies\":[{\"name\":\"A\"}]}"}),
        ]);
        assert!(plan(&req, &catalog).unwrap().is_none());
    }
    #[test]
    fn boxes_and_units() {
        for text in [
            "make a 20 mm cube",
            "Please create a cube of 2cm",
            "can you build a 0.02 m cube?",
        ] {
            let p = get(text);
            assert_eq!(p.rule, "box");
            assert_eq!(
                params(&p, 0),
                &json!({"length":20.0,"width":20.0,"height":20.0,"operation":"new"})
            );
        }
        for text in ["make a box 20 x 30 x 4 mm", "create a 20 by 30 by 4 box"] {
            assert_eq!(
                params(&get(text), 0),
                &json!({"length":20.0,"width":30.0,"height":4.0,"operation":"new"})
            );
        }
        assert_eq!(params(&get("make a 1 in cube"), 0)["length"], 25.4);
        assert_eq!(
            params(&get("make a box 2x3x4 cm"), 0),
            &json!({"length":20.0,"width":30.0,"height":40.0,"operation":"new"})
        );
        assert_eq!(
            params(&get("create a box width 30 length 20 height 4"), 0),
            &json!({"length":20.0,"width":30.0,"height":4.0,"operation":"new"})
        );
    }
    #[test]
    fn cylinders_and_spheres() {
        for text in [
            "make a cylinder 30 mm across and 50 tall",
            "create a cylinder diameter 30 height 50",
            "add a cylinder radius 15 mm height 50 mm",
        ] {
            let p = get(text);
            assert_eq!(p.rule, "cylinder");
            assert_eq!(
                params(&p, 0),
                &json!({"radius":15.0,"height":50.0,"operation":"new"})
            );
        }
        for text in [
            "make a sphere radius 10",
            "create a sphere 20mm diameter",
            "add a sphere 2 cm across",
        ] {
            assert_eq!(
                params(&get(text), 0),
                &json!({"radius":10.0,"operation":"new"})
            );
        }
        assert_eq!(
            params(&get("make a cylinder radius 1 cm height 2"), 0),
            &json!({"radius":10.0,"height":2.0,"operation":"new"})
        );
    }
    #[test]
    fn centred_through_holes() {
        for text in [
            "make a plate 40 x 30 x 5 mm with a centred through hole diameter 10 mm",
            "create a 40 by 30 by 5 plate with a hole through the middle 10 mm diameter",
            "build a plate 40x30x5 with a centered through hole 10 diameter",
        ] {
            let p = get(text);
            assert_eq!(p.rule, "plate_hole");
            assert_eq!(p.calls.len(), 3);
            assert_eq!(p.calls[1]["arguments"]["command"], "solid.hole");
            assert_eq!(
                params(&p, 1),
                &json!({"position":[20.0,15.0,5.0],"diameter":10.0,"targets":["PilotRecipe1"]})
            );
        }
    }
    fn design() -> Value {
        json!({"bodies":[{"name":"Block","volume_mm3":24}]})
    }
    #[test]
    fn moving_and_renaming_require_known_names() {
        let c = Catalog::bundled().unwrap();
        for text in [
            "move it 50 mm along X",
            "please move Block by 5 cm on x",
            "move the body 0.05 m along X",
        ] {
            let p = plan(&request(text, Some(design())), &c).unwrap().unwrap();
            assert_eq!(
                params(&p, 0),
                &json!({"bodies":["Block"],"translate":[50.0,0.0,0.0]})
            );
            assert!(plan(&request(text, None), &c).unwrap().is_none());
        }
        for text in [
            "rename the body to Bracket",
            "please rename Block to Bracket",
            "rename it to Bracket",
        ] {
            let p = plan(&request(text, Some(design())), &c).unwrap().unwrap();
            assert_eq!(params(&p, 0), &json!({"body":"Block","name":"Bracket"}));
        }
        let two = json!({"bodies":[{"name":"Block"},{"name":"Pin"}]});
        let numbered = json!({"bodies":[{"name":"Box001"}]});
        let p = plan(&request("move Box001 2 mm along y", Some(numbered)), &c)
            .unwrap()
            .unwrap();
        assert_eq!(
            params(&p, 0),
            &json!({"bodies":["Box001"],"translate":[0.0,2.0,0.0]})
        );
        assert!(plan(&request("move it 20 along x", Some(two)), &c)
            .unwrap()
            .is_none());
        assert!(plan(&request("rename Unknown to Pin", Some(design())), &c)
            .unwrap()
            .is_none());
    }
    #[test]
    fn inspection_and_computed_answers() {
        let c = Catalog::bundled().unwrap();
        for (rule, texts) in [
            (
                "volume",
                [
                    "what is the volume?",
                    "tell me the volume",
                    "measure the volume",
                ],
            ),
            (
                "body_count",
                [
                    "how many bodies?",
                    "tell me how many bodies there are",
                    "how many bodies are in the design?",
                ],
            ),
        ] {
            for text in texts {
                let p = get(text);
                assert_eq!(p.rule, rule);
                assert_eq!(p.calls[0]["arguments"]["command"], "document.inspect");
                let state =
                    json!({"bodies":[{"name":"A","volume_mm3":24},{"name":"B","volume_mm3":6}]});
                let p = plan(&request(text, Some(state)), &c).unwrap().unwrap();
                assert!(p.calls.is_empty());
                assert_eq!(
                    p.facts.unwrap()[if rule == "volume" {
                        "volume_mm3"
                    } else {
                        "body_count"
                    }],
                    if rule == "volume" {
                        json!(30.0)
                    } else {
                        json!(2)
                    }
                );
            }
        }
        let partial = json!({"bodies":[],"truncated":true,"total":30});
        let p = plan(&request("how many bodies?", Some(partial)), &c)
            .unwrap()
            .unwrap();
        assert!(!p.calls.is_empty());
    }
    #[test]
    fn conservative_refusals_and_bounds() {
        let c = Catalog::bundled().unwrap();
        for text in [
            "what is a 20 mm cube?",
            "how would I make a 20mm cube?",
            "make a cube 20 furlongs",
            "make a cylinder diameter 20 feet height 50",
            "make a sphere radius 5 bananas",
            "make a 0 mm cube",
            "make a -2 cube",
            "don't make a 20 mm cube",
            "make a 1000001 mm cube",
            "make a plate with a hole through the middle",
            "make a plate 40x30x5 with a centred through hole diameter 2 cubits",
            "what does it mean to rename the body to Pin?",
            "move the body Unknown 2 mm along x",
            "make a box 20x30",
            "move it 2 parsecs along x",
            "what happens if I move Block 5 along x?",
        ] {
            assert!(
                plan(&request(text, Some(design())), &c).unwrap().is_none(),
                "{text}"
            );
        }
        assert!(plan(&request(&"x".repeat(MAX_TEXT + 1), None), &c)
            .unwrap()
            .is_none());
        assert!(plan(
            &request("make a 2 cube", None),
            &Catalog::from_json("[]").unwrap()
        )
        .is_err());
        let mut req = request("make a 2 cube", None);
        req["tools"] = json!([]);
        assert!(plan(&req, &c).unwrap().is_none());
        let mut req = request("make a 2 cube", None);
        req["messages"].as_array_mut().unwrap().extend([
            json!({"role":"assistant","tool_calls":[{"function":{"name":"execute","arguments":"{}"}}]}),
            json!({"role":"tool","content":design().to_string()}),
        ]);
        assert!(plan(&req, &c).unwrap().is_none());
        let mut req = request("move it 5 along x", Some(design()));
        req["messages"][0]["content"] =
            json!(format!("{}\n[dutchman-pilot hint] A hint.", design()));
        assert!(plan(&req, &c).unwrap().is_some());
        // The latest tool result invalidates an older complete snapshot.
        let mut req = request("move it 5 along x", Some(design()));
        req["messages"]
            .as_array_mut()
            .unwrap()
            .insert(1, json!({"role":"tool","content":"{}"}));
        assert!(plan(&req, &c).unwrap().is_none());
        let mut req = request("make a 2 cube", None);
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"thanks"}));
        assert!(plan(&req, &c).unwrap().is_none());
    }
}
