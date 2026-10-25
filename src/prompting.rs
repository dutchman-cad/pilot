//! Bounded request help shared by the proxy and benchmark callers.
use crate::{catalog::Catalog, model::ModelSettings};
use serde_json::{json, Value};

pub const FALLBACK: &str = "If you cannot call tools, and only then, reply with exactly one JSON object {\"tool\": ..., \"args\": {...}}. Only a tool call gets its arguments checked before the user sees the change.";
const RULES: &str = "\nData about the design must come from a tool call. A message that is only a name is a search, not a request to create. When two things match, ask which one.\nAction rail:\n";
const MAX_BLOCK_BYTES: usize = 1499;
const MAX_RAIL_LINES: usize = 15;

pub const PLACEMENT_FACT: &str = "solid.box puts its corner at the origin unless you pass center:[x,y,z]; a W×L×H box made without center has its centre at (W/2, L/2, H/2), so centre features there or pass center:[0,0,H/2]. solid.sphere defaults to the origin as centre; solid.cylinder defaults to the origin as bottom centre.";
const PLACEMENT_MARKER: &str = "solid.box puts its corner at the origin";
pub const SKETCH_FACT: &str = "Draw a closed profile in ONE sketch.line call with points, closed:true, infer:true and ids. infer adds coincident, horizontal and vertical constraints itself; do not add them by hand. Then use one sketch.dimension per driving size with entities:[\"<one line id>\"] and a number value.";
const SKETCH_MARKER: &str = "Draw a closed profile in ONE sketch.line call";

/// Request help shares the system channel used by prompt-help and fallback.
pub(crate) fn append_placement_fact(body: &mut Value) -> Result<bool, String> {
    append_fact(body, PLACEMENT_FACT, PLACEMENT_MARKER, false)
}

pub(crate) fn append_sketch_fact(body: &mut Value) -> Result<bool, String> {
    append_fact(body, SKETCH_FACT, SKETCH_MARKER, true)
}

fn append_fact(body: &mut Value, fact: &str, marker: &str, sketch: bool) -> Result<bool, String> {
    let offered = body["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|t| matches!(t["function"]["name"].as_str(), Some("execute" | "batch")))
    });
    if !offered {
        return Ok(false);
    }
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    if messages.iter().any(|m| {
        let content = &m["content"];
        content.as_str().is_some_and(|s| s.contains(marker))
            || content.as_array().is_some_and(|parts| {
                parts
                    .iter()
                    .any(|p| p["text"].as_str().is_some_and(|s| s.contains(marker)))
            })
    }) {
        return Ok(false);
    }
    let text = messages
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .map_or_else(String::new, |m| message_text(&m["content"]));
    let lowercase = text.to_lowercase();
    let words = lowercase.split(|c: char| !c.is_alphabetic());
    let has = |choices: &[&str]| words.clone().any(|w| choices.contains(&w));
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\'' | '"'))
        .collect::<String>()
        .to_lowercase();
    let applies = if sketch {
        // "dimensions" alone is how people size a box; it needs a sketch.
        has(&["constrain", "constrained", "driving"])
            || (has(&["sketch"]) && has(&["profile", "dimension", "dimensions"]))
    } else {
        has(&["box", "cube", "block", "plate", "brick", "slab"])
            && has(&[
                "center", "centre", "centred", "centered", "middle", "through",
            ])
            && !compact.contains("center:[")
            && !compact.contains("centre:[")
    };
    if !applies {
        return Ok(false);
    }
    if let Some(system) = messages.iter_mut().find(|m| m["role"] == "system") {
        if let Some(content) = system["content"].as_str() {
            system["content"] = json!(format!("{content}\n{fact}"));
        } else if let Some(parts) = system["content"].as_array_mut() {
            parts.push(json!({"type":"text","text":fact}));
        } else {
            return Err("system content must be text or an array of parts".into());
        }
    } else {
        messages.insert(0, json!({"role":"system","content":fact}));
    }
    Ok(true)
}

/// MCP has no user conversation: offer the fact once in an action tool description.
pub(crate) fn describe_placement(tools: &mut [Value]) -> bool {
    describe_fact(tools, PLACEMENT_FACT, PLACEMENT_MARKER)
}

pub(crate) fn describe_sketch(tools: &mut [Value]) -> bool {
    describe_fact(tools, SKETCH_FACT, SKETCH_MARKER)
}

fn describe_fact(tools: &mut [Value], fact: &str, marker: &str) -> bool {
    if tools.iter().any(|t| {
        t["description"]
            .as_str()
            .is_some_and(|s| s.contains(marker))
    }) {
        return false;
    }
    let index = tools
        .iter()
        .position(|t| t["name"] == "execute")
        .or_else(|| tools.iter().position(|t| t["name"] == "batch"));
    if let Some(index) = index {
        let old = tools[index]["description"].as_str().unwrap_or("");
        tools[index]["description"] = json!(format!("{old}\n{fact}"));
        return true;
    }
    false
}

/// One short system block. The conditional fallback shares this block when active.
pub fn system_block(
    body: &Value,
    catalog: &Catalog,
    help: bool,
    fallback: bool,
) -> Result<Option<String>, String> {
    if !help {
        return Ok(fallback.then(|| FALLBACK.to_string()));
    }
    let messages = body["messages"]
        .as_array()
        .ok_or("messages must be an array")?;
    let latest = messages.iter().rev().find(|m| m["role"] == "user");
    let text = latest.map_or_else(String::new, |m| message_text(&m["content"]));
    let query: String = text
        .chars()
        .scan(0, |bytes, ch| {
            *bytes += ch.len_utf8();
            (*bytes <= crate::catalog::MAX_QUERY_BYTES).then_some(ch)
        })
        .collect();
    // Only tool evidence sets state; a newer explicit null closes an older sketch.
    let open = messages
        .iter()
        .rev()
        .filter(|m| m["role"] == "tool")
        .filter_map(crate::model::tool_data)
        .find_map(|data| {
            let state = data.get("after").unwrap_or(&data);
            state
                .get("active_sketch")
                .map(|v| !v.is_null() && *v != false)
        })
        .unwrap_or(false);
    let scores = catalog.search_scores(&query)?;
    let mut ranked: Vec<_> = catalog
        .commands()
        .iter()
        .filter_map(|command| {
            let mut score = scores
                .iter()
                .find(|(_, c)| c.id == command.id)
                .map_or(0, |(s, _)| *s);
            let sketch = open && command.id.starts_with("sketch.");
            if open && command.id == "sketch.finish" {
                score += 100;
            }
            (sketch || score > 0).then_some((sketch, score, command))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.id.cmp(&b.2.id))
    });
    let mut block = String::new();
    if fallback {
        block.push_str(FALLBACK);
        block.push('\n');
    }
    block.push_str(crate::identity::PROMPT_HELP);
    block.push_str(RULES);
    for (_, _, command) in ranked.into_iter().take(MAX_RAIL_LINES) {
        let purpose = command
            .label
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let line = format!("{}: {}\n", command.id, purpose);
        if block.len() + line.len() <= MAX_BLOCK_BYTES {
            block.push_str(&line);
        }
    }
    Ok(Some(block))
}

/// Text of a message's content: a string, or the text parts of a content array.
fn message_text(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    content
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .take(16)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Only confirmed Ollama servers get settings. Explicit request context is preserved.
pub fn settings_additions(
    body: &Value,
    ollama: bool,
    settings: Option<&ModelSettings>,
    seed: Option<i64>,
) -> Result<Vec<(&'static str, Value)>, String> {
    let Some(settings) = settings.filter(|_| ollama) else {
        return Ok(Vec::new());
    };
    let mut additions = Vec::new();
    if body["tools"]
        .as_array()
        .is_some_and(|tools| !tools.is_empty())
    {
        additions.push(("temperature", json!(settings.temperature.unwrap_or(0.1))));
    }
    if let Some(seed) = seed.or(settings.seed) {
        additions.push(("seed", json!(seed)));
    }
    // A malformed `options` is the client's to fix; leave it for the upstream to report.
    let options = match body.get("options") {
        None => Some(serde_json::Map::new()),
        Some(value) => value.as_object().cloned(),
    };
    if let Some(mut options) = options.filter(|o| !o.contains_key("num_ctx")) {
        options.insert(
            "num_ctx".into(),
            json!(settings.num_ctx.unwrap_or(8192).max(8192)),
        );
        additions.push(("options", Value::Object(options)));
    }
    Ok(additions)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn placement_triggers_and_explicit_coordinates() {
        for (text, expected) in [
            ("draw a 20mm cube with a 5mm hole through the center", true),
            ("a centred BOX", true),
            ("brick through middle", true),
            ("plate with hole through", true),
            ("block center", true),
            ("slab centre", true),
            ("sphere through the center", false),
            ("draw a cube", false),
            ("boxwood through", false),
            ("cube centered at center: [0, 0, 10]", false),
            ("box through with \"center\": [0,0,10]", false),
            ("box middle with centre:[0,0,10]", false),
        ] {
            for tool in ["execute", "batch", "inspect"] {
                let mut body = json!({"tools":[{"function":{"name":tool}}],
                    "messages":[{"role":"system","content":"Existing rules."},
                        {"role":"user","content":text}]});
                assert_eq!(
                    append_placement_fact(&mut body).unwrap(),
                    expected && tool != "inspect",
                    "{text} {tool}"
                );
                assert_eq!(body["messages"].as_array().unwrap().len(), 2);
                if expected && tool != "inspect" {
                    assert_eq!(
                        body["messages"][0]["content"],
                        format!("Existing rules.\n{PLACEMENT_FACT}")
                    );
                    let before = body.clone();
                    assert!(!append_placement_fact(&mut body).unwrap());
                    assert_eq!(body, before);
                    body["messages"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"role":"user","content":"another centred cube"}));
                    assert!(!append_placement_fact(&mut body).unwrap());
                }
            }
        }
        assert!(PLACEMENT_FACT.len() < 400);
        let extra = PLACEMENT_FACT.split("solid.sphere").nth(1).unwrap();
        assert!(extra.split_whitespace().count() < 25);
    }

    #[test]
    fn placement_parts_latest_user_and_mcp_descriptions() {
        let mut body = json!({"tools":[{"function":{"name":"batch"}}], "messages":[
            {"role":"system","content":[{"type":"text","text":"Rules"}]},
            {"role":"user","content":[{"type":"text","text":"cube through middle"}]}]});
        assert!(append_placement_fact(&mut body).unwrap());
        assert_eq!(body["messages"][0]["content"][1]["text"], PLACEMENT_FACT);
        assert!(!append_placement_fact(&mut body).unwrap());
        let mut parts = vec![json!({"type":"text","text":"Rules"}); 20];
        parts.push(json!({"type":"text","text":PLACEMENT_FACT}));
        body["messages"][0]["content"] = json!(parts);
        assert!(!append_placement_fact(&mut body).unwrap());
        body["messages"] = json!([{"role":"user","content":"cube through middle"},
            {"role":"user","content":"hello"}]);
        assert!(!append_placement_fact(&mut body).unwrap());
        body["messages"] = json!([{"role":"user","content":"cube through middle"}]);
        assert!(append_placement_fact(&mut body).unwrap());
        assert_eq!(body["messages"][0]["content"], PLACEMENT_FACT);
        for names in [vec!["batch", "execute"], vec!["batch"], vec!["inspect"]] {
            let mut tools: Vec<_> = names
                .iter()
                .map(|name| json!({"name":name,"description":"Original"}))
                .collect();
            assert_eq!(describe_placement(&mut tools), names[0] != "inspect");
            let expected = if names.len() == 2 { 1 } else { 0 };
            if names[0] != "inspect" {
                assert_eq!(
                    tools[expected]["description"],
                    format!("Original\n{PLACEMENT_FACT}")
                );
            }
            assert!(!describe_placement(&mut tools));
        }
    }

    #[test]
    fn opt_in_short_relevant_rails_and_shared_fallback() {
        let catalog = Catalog::bundled().unwrap();
        for (query, expected) in [
            ("draw a circle in a sketch", "sketch.circle.center:"),
            ("make a solid box", "solid.box:"),
        ] {
            let body = json!({"messages":[{"role":"user","content":query}]});
            assert_eq!(system_block(&body, &catalog, false, false).unwrap(), None);
            assert_eq!(
                system_block(&body, &catalog, false, true).unwrap().unwrap(),
                FALLBACK
            );
            for fallback in [false, true] {
                let block = system_block(&body, &catalog, true, fallback)
                    .unwrap()
                    .unwrap();
                assert!(block.len() < 1500);
                assert!(block.contains(expected), "{block}");
                assert!(block.contains("ask which one"));
                assert_eq!(block.matches(FALLBACK).count(), usize::from(fallback));
                let rail = block.split_once("Action rail:\n").unwrap().1;
                assert!(rail.lines().count() <= 15);
                assert!(!rail.contains("params"));
            }
        }
    }

    #[test]
    fn sketch_state_comes_from_tools_and_newer_state_wins() {
        let catalog = Catalog::bundled().unwrap();
        let mut body = json!({"messages":[
            {"role":"user","content":"make a solid box"},
            {"role":"tool","content":json!({"after":{"active_sketch":"s1"}}).to_string()}
        ]});
        let block = system_block(&body, &catalog, true, false).unwrap().unwrap();
        let rail = block.split_once("Action rail:\n").unwrap().1;
        assert!(rail.starts_with("sketch.finish:"));
        body["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":json!({"active_sketch":null}).to_string()}));
        let block = system_block(&body, &catalog, true, false).unwrap().unwrap();
        assert!(block
            .split_once("Action rail:\n")
            .unwrap()
            .1
            .starts_with("solid.box:"));
        body["messages"] = json!([{"role":"user","content":"é".repeat(10000)}]);
        assert!(
            system_block(&body, &catalog, true, true)
                .unwrap()
                .unwrap()
                .len()
                < 1500
        );
    }

    #[test]
    fn oversized_catalogue_labels_cannot_expand_the_block() {
        let commands: Vec<_> = (0..30).map(|i| json!({
            "id":format!("sketch.circle{i}"), "label":"Circle geometry\n".repeat(50), "params":"parameter docs must not appear"
        })).collect();
        let catalog = Catalog::from_json(&json!(commands).to_string()).unwrap();
        let body = json!({"messages":[{"role":"user","content":"circle"}]});
        let block = system_block(&body, &catalog, true, true).unwrap().unwrap();
        assert!(block.len() < 1500);
        let rail = block.split_once("Action rail:\n").unwrap().1;
        assert!(rail.lines().count() <= 15);
        assert!(!rail.contains("parameter docs"));
    }

    #[test]
    fn settings_require_opt_in_and_ollama_and_preserve_context() {
        let body = json!({"tools":[{}],"options":{"other":true}});
        let settings = ModelSettings::default();
        assert!(settings_additions(&body, false, Some(&settings), Some(42))
            .unwrap()
            .is_empty());
        assert!(settings_additions(&body, true, None, Some(42))
            .unwrap()
            .is_empty());
        let additions = settings_additions(&body, true, Some(&settings), Some(42)).unwrap();
        assert_eq!(
            additions,
            vec![
                ("temperature", json!(0.1)),
                ("seed", json!(42)),
                ("options", json!({"num_ctx":8192,"other":true}))
            ]
        );
        for ctx in [4096, 8192, 32768] {
            let body = json!({"options":{"num_ctx":ctx}});
            assert!(settings_additions(&body, true, Some(&settings), None)
                .unwrap()
                .is_empty());
        }
        assert!(
            settings_additions(&json!({"options":false}), true, Some(&settings), None)
                .unwrap()
                .is_empty()
        );
        let parts = json!({"messages":[{"role":"user","content":[{"type":"text","text":"draw a circle in a sketch"}]}]});
        let block = system_block(&parts, &Catalog::bundled().unwrap(), true, false)
            .unwrap()
            .unwrap();
        assert!(block.contains("sketch.circle.center:"));
    }
    #[test]
    fn sketch_fact_relevance_shared_channel_and_descriptions() {
        assert!(SKETCH_FACT.len() < 400);
        for (text, applies) in [
            ("fully constrain it", true),
            ("constrained sketch", true),
            ("dimension this sketch", true),
            ("a box with dimensions 20 x 30 x 40", false),
            ("four driving sizes", true),
            ("sketch a profile", true),
            ("sketch a circle", false),
            ("profile settings", false),
            ("unconstrained", false),
        ] {
            for tool in ["execute", "batch", "inspect"] {
                let mut body = json!({"tools":[{"function":{"name":tool}}],"messages":[{"role":"user","content":[{"type":"text","text":text}]}]});
                assert_eq!(
                    append_sketch_fact(&mut body).unwrap(),
                    applies && tool != "inspect"
                );
                let before = body.clone();
                assert!(!append_sketch_fact(&mut body).unwrap());
                assert_eq!(before, body);
            }
        }
        let mut body = json!({"tools":[{"function":{"name":"execute"}}],"messages":[{"role":"system","content":[{"type":"text","text":"Rules"}]},{"role":"user","content":"sketch a constrained plate profile with a centered hole"}]});
        assert!(append_placement_fact(&mut body).unwrap());
        assert!(append_sketch_fact(&mut body).unwrap());
        assert_eq!(body["messages"][0]["content"][1]["text"], PLACEMENT_FACT);
        assert_eq!(body["messages"][0]["content"][2]["text"], SKETCH_FACT);
        assert!(!append_sketch_fact(&mut body).unwrap());
        for names in [vec!["execute", "batch"], vec!["batch"], vec!["inspect"]] {
            let mut tools: Vec<_> = names
                .iter()
                .map(|n| json!({"name":n,"description":"Original"}))
                .collect();
            assert_eq!(describe_sketch(&mut tools), names[0] != "inspect");
            assert!(!describe_sketch(&mut tools));
            if names[0] != "inspect" {
                assert_eq!(tools[0]["description"], format!("Original\n{SKETCH_FACT}"));
            }
        }
    }
}
