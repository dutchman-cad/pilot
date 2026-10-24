//! Bounded, conservative plans derived only from the latest user text and tool state.
use crate::{catalog::Catalog, guidance, repair};
use serde_json::{json, Value};

const MAX_TEXT: usize = 4096;
const MAX_BODIES: usize = 256;
const MAX_MM: f64 = 1_000_000.0;

#[derive(Debug, serde::Serialize)]
pub struct Plan {
    pub rule: &'static str,
    pub calls: Vec<Value>,
    pub facts: Option<Value>,
}
impl Plan {
    pub fn message(&self) -> String {
        let mut message = format!("For this request, run exactly this plan: {}. Don't change it; then handle anything else the user asked.", json!(self.calls));
        if let Some(facts) = &self.facts {
            message.push_str(&format!(
                " Computed from the latest tool result: {facts}. Use these exact facts."
            ));
        }
        message
    }
}

// Words and numbers are separate even in `20mm` and `20x30x4`.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut numeric = false;
    for c in text.chars() {
        let number = c.is_ascii_digit() || c == '.' || c == '-';
        if c.is_ascii_alphanumeric() || c == '_' || number {
            if !current.is_empty() && numeric != number {
                out.push(std::mem::take(&mut current));
            }
            numeric = number;
            current.push(c.to_ascii_lowercase());
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}
fn scale(s: &str) -> Option<f64> {
    match s {
        "mm" => Some(1.0),
        "cm" => Some(10.0),
        "m" => Some(1000.0),
        "in" | "inch" | "inches" => Some(25.4),
        _ => None,
    }
}
#[derive(Clone, Copy)]
struct Dimension {
    value: f64,
    index: usize,
}
fn dimensions(t: &[String]) -> Option<Vec<Dimension>> {
    let mut dims = Vec::new();
    let mut units = Vec::new();
    for (i, word) in t.iter().enumerate() {
        let Ok(n) = word.parse::<f64>() else { continue };
        let next = t.get(i + 1).map(String::as_str).unwrap_or("");
        let unit = scale(next);
        if unit.is_none()
            && !matches!(
                next,
                "" | "x"
                    | "by"
                    | "and"
                    | "along"
                    | "on"
                    | "tall"
                    | "high"
                    | "wide"
                    | "long"
                    | "thick"
                    | "across"
                    | "diameter"
                    | "radius"
                    | "height"
                    | "length"
                    | "width"
                    | "cube"
                    | "box"
                    | "sphere"
                    | "cylinder"
                    | "plate"
                    | "with"
            )
        {
            return None;
        }
        dims.push(Dimension { value: n, index: i });
        units.push(unit);
    }
    // A trailing unit on a dimension tuple applies to the whole tuple.
    let common = units.iter().flatten().copied().collect::<Vec<_>>();
    let tuple = dims.first().zip(dims.last()).is_some_and(|(a, b)| {
        t[a.index..b.index]
            .iter()
            .all(|s| s.parse::<f64>().is_ok() || scale(s).is_some() || s == "x" || s == "by")
    });
    let default = if common.len() == 1 && tuple && units.last().is_some_and(Option::is_some) {
        common[0]
    } else {
        1.0
    };
    for (d, unit) in dims.iter_mut().zip(units) {
        d.value *= unit.unwrap_or(default);
        if !d.value.is_finite() || d.value.abs() > MAX_MM || d.value == 0.0 {
            return None;
        }
    }
    (dims.len() <= 8).then_some(dims)
}
fn labelled(t: &[String], d: Dimension, labels: &[&str]) -> bool {
    let before = d.index.checked_sub(1).and_then(|i| t.get(i));
    if let Some(label) =
        before.filter(|s| ["radius", "diameter", "height", "length", "width"].contains(&s.as_str()))
    {
        return labels.contains(&label.as_str());
    }
    let mut after = d.index + 1;
    if t.get(after).is_some_and(|s| scale(s).is_some()) {
        after += 1;
    }
    before
        .into_iter()
        .chain(t.get(after))
        .any(|s| labels.contains(&s.as_str()))
}
fn state(messages: &[Value]) -> Option<Value> {
    // Never fall back to an older snapshot when a newer tool result is incomplete.
    let message = messages.iter().rev().find(|m| m["role"] == "tool")?;
    let content = message.get("content")?;
    let data = if let Some(s) = content.as_str() {
        if s.len() > repair::MAX_BYTES {
            return None;
        }
        // The tools face may append our own short guidance after the JSON.
        let json = s
            .split_once(&format!("\n{}", crate::identity::HINT))
            .map_or(s, |(json, _)| json);
        serde_json::from_str(json).ok()?
    } else {
        content.clone()
    };
    if data.get("isError") == Some(&json!(true)) || data.get("error").is_some() {
        return None;
    }
    let data = guidance::payload(&data);
    if data.get("isError") == Some(&json!(true))
        || data.get("error").is_some()
        || !guidance::paging(&data).is_empty()
    {
        return None;
    }
    let bodies = data.get("bodies")?.as_array()?;
    (bodies.len() <= MAX_BODIES).then_some(data)
}
fn body(text: &str, data: Option<&Value>) -> Option<String> {
    let bodies = data?.get("bodies")?.as_array()?;
    let names: Vec<_> = bodies
        .iter()
        .filter_map(|b| b.get("name").and_then(Value::as_str))
        .filter(|n| !n.is_empty() && n.len() <= 256)
        .collect();
    let lower = text.to_ascii_lowercase();
    let found: Vec<_> = names
        .iter()
        .filter(|n| {
            let name = n.to_ascii_lowercase();
            lower.match_indices(&name).any(|(i, _)| {
                let left = lower[..i].chars().next_back();
                let right = lower[i + name.len()..].chars().next();
                left.is_none_or(|c| !c.is_alphanumeric() && c != '_')
                    && right.is_none_or(|c| !c.is_alphanumeric() && c != '_')
            })
        })
        .collect();
    if found.len() == 1 {
        Some(found[0].to_string())
    } else if found.is_empty() && names.len() == 1 && bodies.len() == 1 && {
        let words = tokens(text);
        words
            .iter()
            .position(|s| s == "move" || s == "rename")
            .is_some_and(|i| {
                let rest = &words[i + 1..];
                let rest = if rest.first().is_some_and(|s| s == "the") {
                    &rest[1..]
                } else {
                    rest
                };
                rest.first().is_some_and(|s| s == "it" || s == "body")
                    && rest
                        .get(1)
                        .is_none_or(|s| s == "by" || s.parse::<f64>().is_ok())
            })
    } {
        Some(names[0].to_string())
    } else {
        None
    }
}
fn call(catalog: &Catalog, id: &str, params: Value) -> Result<Value, String> {
    let command = catalog
        .get(id)
        .ok_or_else(|| format!("planner command missing from catalogue: {id}"))?;
    // The public docs include alternative parameter names, which the conservative
    // repair schema intentionally omits (e.g. radius | diameter).
    let documented = tokens(&command.params);
    for key in params
        .as_object()
        .ok_or("planner parameters must be an object")?
        .keys()
    {
        if !documented.contains(key) {
            return Err(format!(
                "planner parameter {id}.{key} missing from catalogue"
            ));
        }
    }
    Ok(json!({"tool":"execute","arguments":{"command":id,"params":params}}))
}

pub fn plan(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    if messages.len() > 4096 {
        return Err("planner conversation exceeds 4096 messages".into());
    }
    let Some(text) = messages
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
    else {
        return Ok(None);
    };
    if text.len() > MAX_TEXT {
        return Ok(None);
    }
    let t = tokens(text);
    let has = |w: &str| t.iter().any(|s| s == w);
    let data = state(messages);
    let lower = text.to_ascii_lowercase();
    // On continuation requests, an already returned action must not be planned again.
    let user_index = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    let acted = messages[user_index + 1..].iter().any(|m| {
        m.get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| {
                calls
                    .iter()
                    .any(|c| matches!(c["function"]["name"].as_str(), Some("execute" | "batch")))
            })
    });
    let mut calls = Vec::new();
    let mut facts = None;
    let rule;
    if lower.contains("how many bodies")
        || (has("volume") && (has("what") || has("tell") || has("measure")))
    {
        rule = if has("volume") {
            "volume"
        } else {
            "body_count"
        };
        if let Some(bodies) = data.as_ref().and_then(|d| d["bodies"].as_array()) {
            if rule == "body_count" {
                facts = Some(json!({"body_count":bodies.len()}));
            } else {
                // Named queries must resolve uniquely; general volume means total.
                let named = body(text, data.as_ref());
                if named.is_none() && (has("of") || has("for")) && !has("design") && !has("all") {
                    return Ok(None);
                }
                let selected: Vec<_> = bodies
                    .iter()
                    .filter(|b| named.as_ref().is_none_or(|n| b["name"] == *n))
                    .collect();
                let volumes: Option<Vec<_>> = selected
                    .iter()
                    .map(|b| {
                        b.get("volume_mm3")
                            .or_else(|| b.get("volume"))?
                            .as_f64()
                            .filter(|n| n.is_finite() && *n >= 0.0)
                    })
                    .collect();
                if let Some(volumes) = volumes {
                    let total: f64 = volumes.iter().sum();
                    if total.is_finite() {
                        facts = Some(
                            json!({"volume_mm3":total,"body_count":bodies.len(),"bodies":selected.iter().filter_map(|b| b["name"].as_str()).collect::<Vec<_>>()}),
                        );
                    }
                }
            }
        }
        if facts.is_none() {
            // Already looked something up for this request: don't send the model round again.
            let looked = messages[user_index + 1..].iter().any(|m| {
                m.get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|calls| !calls.is_empty())
            });
            if looked {
                return Ok(None);
            }
            calls.push(call(catalog, "document.inspect", json!({"measure":true}))?);
        }
    } else if acted
        || has("not")
        || has("don")
        || has("explain")
        || has("would")
        || has("what")
        || has("how")
    {
        return Ok(None);
    } else if has("rename")
        && t.first()
            .is_some_and(|s| s == "rename" || s == "please" || s == "can")
    {
        let Some((_, new)) = text.rsplit_once(" to ") else {
            return Ok(None);
        };
        let new = new
            .trim()
            .trim_matches(['`', '\'', '"'])
            .trim_end_matches(['.', '?']);
        if new.is_empty() || new.len() > 128 || new.contains(['\n', '\r']) {
            return Ok(None);
        }
        let source = text.rsplit_once(" to ").map_or(text, |(s, _)| s);
        let Some(name) = body(source, data.as_ref()) else {
            return Ok(None);
        };
        rule = "rename";
        calls.push(call(
            catalog,
            "body.rename",
            json!({"body":name,"name":new}),
        )?);
    } else {
        // Body names commonly contain digits (Box001); they are not dimensions.
        let dimension_words = if has("move") {
            body(text, data.as_ref())
                .map(|name| tokens(&lower.replace(&name.to_ascii_lowercase(), " ")))
        } else {
            None
        };
        let dimension_words = dimension_words.as_ref().unwrap_or(&t);
        let Some(dims) = dimensions(dimension_words) else {
            return Ok(None);
        };
        if has("move")
            && !has("not")
            && !has("don")
            && !has("how")
            && !has("what")
            && !has("would")
            && dims.len() == 1
        {
            let axes: Vec<_> = ["x", "y", "z"]
                .iter()
                .enumerate()
                .filter(|(_, a)| dimension_words.iter().any(|s| s == *a))
                .collect();
            if axes.len() != 1 {
                return Ok(None);
            }
            let Some(name) = body(text, data.as_ref()) else {
                return Ok(None);
            };
            let mut delta = [0.0; 3];
            delta[axes[0].0] = dims[0].value;
            rule = "move";
            calls.push(call(
                catalog,
                "solid.move",
                json!({"bodies":[name],"translate":delta}),
            )?);
        } else {
            let intent = ["make", "create", "build", "add"].iter().any(|w| has(w));
            if !intent
                || has("not")
                || has("don")
                || has("explain")
                || has("would")
                || (has("what") || has("how"))
                || dims.iter().any(|d| d.value <= 0.0)
            {
                return Ok(None);
            }
            if has("plate")
                && has("hole")
                && (has("centred")
                    || has("centered")
                    || has("middle")
                    || has("centre")
                    || has("center"))
                && has("through")
                && dims.len() == 4
            {
                let hole = dims[3];
                let diameter = if labelled(&t, hole, &["radius"]) {
                    hole.value * 2.0
                } else {
                    hole.value
                };
                if diameter >= dims[0].value.min(dims[1].value) {
                    return Ok(None);
                }
                rule = "plate_hole";
                let [l, w, h] = [dims[0].value, dims[1].value, dims[2].value];
                calls.push(call(
                    catalog,
                    "solid.box",
                    json!({"length":l,"width":w,"height":h,"operation":"new"}),
                )?);
                calls.push(call(
                    catalog,
                    "solid.hole",
                    json!({"position":[l/2.0,w/2.0,h],"diameter":diameter}),
                )?);
            } else if (has("box") && dims.len() == 3) || (has("cube") && dims.len() == 1) {
                rule = "box";
                let mut l = dims[0].value;
                let mut w = dims.get(1).map_or(l, |d| d.value);
                let mut h = dims.get(2).map_or(l, |d| d.value);
                if has("box") && ["length", "width", "height"].iter().any(|s| has(s)) {
                    let sizes: Option<Vec<_>> = ["length", "width", "height"]
                        .iter()
                        .map(|label| {
                            let found: Vec<_> = dims
                                .iter()
                                .filter(|d| labelled(&t, **d, &[*label]))
                                .collect();
                            (found.len() == 1).then(|| found[0].value)
                        })
                        .collect();
                    let Some(sizes) = sizes else { return Ok(None) };
                    [l, w, h] = [sizes[0], sizes[1], sizes[2]];
                }
                calls.push(call(
                    catalog,
                    "solid.box",
                    json!({"length":l,"width":w,"height":h}),
                )?);
            } else if has("cylinder") && dims.len() == 2 {
                let Some(h) = dims
                    .iter()
                    .find(|d| labelled(&t, **d, &["height", "tall", "high"]))
                else {
                    return Ok(None);
                };
                let Some(r) = dims.iter().find(|d| d.index != h.index) else {
                    return Ok(None);
                };
                let radius = if labelled(&t, *r, &["radius"]) {
                    r.value
                } else if labelled(&t, *r, &["diameter", "across"]) {
                    r.value / 2.0
                } else {
                    return Ok(None);
                };
                rule = "cylinder";
                calls.push(call(
                    catalog,
                    "solid.cylinder",
                    json!({"radius":radius,"height":h.value}),
                )?);
            } else if has("sphere") && dims.len() == 1 {
                let d = dims[0];
                let radius = if labelled(&t, d, &["radius"]) {
                    d.value
                } else if labelled(&t, d, &["diameter", "across"]) {
                    d.value / 2.0
                } else {
                    return Ok(None);
                };
                rule = "sphere";
                calls.push(call(catalog, "solid.sphere", json!({"radius":radius}))?);
            } else {
                return Ok(None);
            }
        }
    }
    // Do not instruct a model to use a tool the client did not offer.
    if !request
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(|t| t["function"]["name"] == "execute"))
    {
        return Ok(None);
    }
    Ok(Some(Plan { rule, calls, facts }))
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
                &json!({"length":20.0,"width":20.0,"height":20.0})
            );
        }
        for text in ["make a box 20 x 30 x 4 mm", "create a 20 by 30 by 4 box"] {
            assert_eq!(
                params(&get(text), 0),
                &json!({"length":20.0,"width":30.0,"height":4.0})
            );
        }
        assert_eq!(params(&get("make a 1 in cube"), 0)["length"], 25.4);
        assert_eq!(
            params(&get("make a box 2x3x4 cm"), 0),
            &json!({"length":20.0,"width":30.0,"height":40.0})
        );
        assert_eq!(
            params(&get("create a box width 30 length 20 height 4"), 0),
            &json!({"length":20.0,"width":30.0,"height":4.0})
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
            assert_eq!(params(&p, 0), &json!({"radius":15.0,"height":50.0}));
        }
        for text in [
            "make a sphere radius 10",
            "create a sphere 20mm diameter",
            "add a sphere 2 cm across",
        ] {
            assert_eq!(params(&get(text), 0), &json!({"radius":10.0}));
        }
        assert_eq!(
            params(&get("make a cylinder radius 1 cm height 2"), 0),
            &json!({"radius":10.0,"height":2.0})
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
            assert_eq!(p.calls.len(), 2);
            assert_eq!(p.calls[1]["arguments"]["command"], "solid.hole");
            assert_eq!(
                params(&p, 1),
                &json!({"position":[20.0,15.0,5.0],"diameter":10.0})
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
