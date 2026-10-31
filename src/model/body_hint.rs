//! Evidence-only single-part guidance. Reads use the client's normal approval path.
use super::*;
use std::collections::BTreeSet;

const MAX_BODIES: usize = 24;
const MAX_READS: usize = 12;
const MAX_DOCUMENTS: usize = 4;
const MARKER: &str = "[dutchman-pilot hint] Body check:";

#[derive(Default, Clone)]
pub(super) struct State {
    index: Option<usize>,
    reads: usize,
    documents: usize,
    pub handed_back: bool,
    seen: BTreeSet<String>,
}
impl State {
    pub fn reset(&mut self, index: usize, fresh: bool) {
        if fresh || self.index != Some(index) {
            *self = Self {
                index: Some(index),
                ..Self::default()
            };
        }
    }
}

// Deliberately conservative: plural product nouns, mechanisms and explicit separation
// suppress guidance, even when an assembly's pieces happen to touch.
pub(super) fn allows_one_part(request: &str) -> bool {
    let words: Vec<_> = request
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    if words.iter().any(|w| {
        matches!(
            w.as_str(),
            "assembly"
                | "assemblies"
                | "assemble"
                | "mechanism"
                | "joint"
                | "parts"
                | "bodies"
                | "components"
                | "pieces"
                | "separate"
                | "separately"
                | "independent"
                | "unjoined"
                | "unfused"
                | "copies"
                | "disconnected"
        )
    }) {
        return false;
    }
    // A counted plural solid ("eight boxes", "2 cylinders") asks for several bodies.
    if words.windows(2).any(|w| {
        (w[0].parse::<u32>().is_ok_and(|n| n > 1)
            || matches!(
                w[0].as_str(),
                "two"
                    | "three"
                    | "four"
                    | "five"
                    | "six"
                    | "seven"
                    | "eight"
                    | "nine"
                    | "ten"
                    | "twelve"
                    | "several"
                    | "multiple"
            ))
            && matches!(
                w[1].as_str(),
                "boxes"
                    | "cubes"
                    | "blocks"
                    | "cylinders"
                    | "spheres"
                    | "plates"
                    | "discs"
                    | "disks"
                    | "solids"
            )
    }) {
        return false;
    }
    true
}
fn one_part(request: &str) -> bool {
    if !allows_one_part(request) {
        return false;
    }
    let words: Vec<_> = request
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    words.iter().any(|w| {
        matches!(
            w.as_str(),
            "make"
                | "build"
                | "create"
                | "add"
                | "model"
                | "design"
                | "extrude"
                | "pad"
                | "part"
                | "plate"
                | "bracket"
                | "base"
                | "boss"
                | "gusset"
                | "standoff"
                | "enclosure"
                | "box"
        )
    })
}

fn data(m: &Value) -> Option<Value> {
    let mut m = m.clone();
    if let Some(s) = m["content"].as_str() {
        m["content"] = json!(s
            .split_once(&format!("\n{}", crate::identity::HINT))
            .map_or(s, |(head, _)| head));
    }
    let d = tool_data(&m)?;
    (d["ok"] != false && d["isError"] != true && d.get("error").is_none_or(Value::is_null))
        .then_some(d)
}

#[derive(Default)]
struct Evidence {
    document: Option<Value>,
    created: BTreeSet<String>,
    measurements: Vec<Value>,
    attempted: BTreeSet<String>,
    acted: bool,
    needs_document: bool,
}
fn visit(e: &mut Evidence, d: Value, depth: usize) {
    if depth > 6 {
        return;
    }
    if let Some(a) = d.as_array().or_else(|| d["results"].as_array()) {
        for v in a.iter().take(128) {
            visit(e, guidance::payload(v), depth + 1);
        }
        return;
    }
    if d["isError"] == true || d["ok"] == false || d.get("error").is_some_and(|v| !v.is_null()) {
        return;
    }
    if let Some(after) = d.get("after") {
        visit(e, after.clone(), depth + 1);
        return;
    }
    if d.get("created_bodies").is_some()
        || d.get("changed_bodies").is_some()
        || d.get("removed_bodies").is_some()
        || d.get("feature").is_some()
    {
        e.document = None;
        e.measurements.clear();
        e.attempted.clear();
        e.acted = true;
        for n in d["removed_bodies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            e.created.remove(n);
        }
        for n in d["created_bodies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(MAX_BODIES)
        {
            if n.len() <= 128 && e.created.len() < MAX_BODIES {
                e.created.insert(n.to_owned());
            }
        }
        e.needs_document =
            !e.created.is_empty() && d["bodies"].as_array().is_none_or(|b| b.len() > 1);
    }
    if let Some(b) = d["bodies"].as_array() {
        if b.len() <= MAX_BODIES
            && b.iter().all(|b| b["name"].is_string() && b.is_object())
            && guidance::paging(&d).is_empty()
            && d.get("body_count")
                .is_none_or(|n| n.as_u64() == Some(b.len() as u64))
        {
            if e.document.as_ref().is_some_and(|previous| {
                previous["bodies"] != d["bodies"] || previous["revision"] != d["revision"]
            }) {
                e.measurements.clear();
            }
            e.document = Some(d.clone());
            e.needs_document = false;
        }
    }
    if d["items"].is_array() || d["interferences"].is_array() {
        e.measurements.push(d);
    }
}
fn evidence(request: &Value) -> Evidence {
    let mut e = Evidence::default();
    let ms = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let start = ms.iter().rposition(|m| m["role"] == "user").unwrap_or(0);
    let mut calls = BTreeMap::new();
    for (i, m) in ms.iter().enumerate().take(4096) {
        if let Some(cs) = m["tool_calls"].as_array() {
            for c in cs.iter().take(128) {
                if let Some(id) = c["id"].as_str() {
                    calls.insert(id, c);
                }
            }
        }
        if m["role"] != "tool" {
            continue;
        }
        let call = m["tool_call_id"].as_str().and_then(|id| calls.get(id));
        let args = call.and_then(|c| {
            c["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str::<Value>(a).ok())
                .or_else(|| {
                    c["function"]["arguments"]
                        .is_object()
                        .then(|| c["function"]["arguments"].clone())
                })
        });
        if i >= start {
            // A failed probe still consumes its attempt; never retry it automatically.
            if let Some(a) = &args {
                if matches!(
                    a["command"].as_str(),
                    Some("inspect.measure" | "inspect.interference" | "document.inspect")
                ) {
                    e.attempted.insert(read_key(a));
                }
            }
        }
        if let Some(d) = data(m) {
            // Undo, parameter edits and transforms need not return body-change metadata.
            if args
                .as_ref()
                .and_then(|a| a["command"].as_str())
                .is_some_and(|command| {
                    !command.starts_with("inspect.")
                        && !command.starts_with("model.")
                        && !matches!(
                            command,
                            "document.inspect" | "timeline.inspect" | "parameters.list"
                        )
                        && !command.ends_with(".list")
                })
            {
                e.document = None;
                e.measurements.clear();
                e.attempted.clear();
                e.acted = true;
                e.needs_document = !e.created.is_empty();
            }
            // Old complete inspections are useful until a subsequent mutation invalidates them.
            visit(&mut e, d, 0);
        }
        if i < start {
            e.created.clear();
            e.acted = false;
            e.needs_document = false;
        }
    }
    e
}
fn read_key(args: &Value) -> String {
    let mut args = args.clone();
    if let Some(bodies) = args["params"]["bodies"].as_array_mut() {
        bodies.sort_by_key(Value::to_string);
    }
    if let Some(items) = args["params"]["items"].as_array_mut() {
        items.sort_by_key(Value::to_string);
    }
    args.to_string()
}
fn name(b: &Value) -> Option<&str> {
    if b["closed"] == false {
        return None;
    }
    b["name"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
}
fn near(a: &Value, b: &Value) -> bool {
    (0..3).all(|i| {
        let Some((amin, amax, bmin, bmax)) = a["bbox"]["min"][i]
            .as_f64()
            .zip(a["bbox"]["max"][i].as_f64())
            .zip(b["bbox"]["min"][i].as_f64())
            .zip(b["bbox"]["max"][i].as_f64())
            .map(|(((a, b), c), d)| (a, b, c, d))
        else {
            return false;
        };
        [amin, amax, bmin, bmax].iter().all(|v| v.is_finite())
            && amin <= amax
            && bmin <= bmax
            && amin <= bmax + 1e-7
            && bmin <= amax + 1e-7
    })
}
fn reads(a: &str, b: &str) -> [Value; 2] {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    [
        json!({"command":"inspect.interference","params":{"bodies":[a,b],"budget_s":3}}),
        json!({"command":"inspect.measure","params":{"items":[{"type":"body","name":a},{"type":"body","name":b}]}}),
    ]
}
fn contact(e: &Evidence, a: &str, b: &str) -> bool {
    e.measurements.iter().any(|m| {
        let pair = |x: &str, y: &str| (x == a && y == b) || (x == b && y == a);
        m["interferences"].as_array().is_some_and(|v| {
            v.iter().any(|v| {
                v["a"]
                    .as_str()
                    .zip(v["b"].as_str())
                    .is_some_and(|(x, y)| pair(x, y))
                    && v["volume_mm3"].as_f64().is_some_and(|v| v > 0.0)
            })
        }) || (m["items"].as_array().is_some_and(|v| {
            v.len() == 2
                && v[0]["name"]
                    .as_str()
                    .zip(v[1]["name"].as_str())
                    .is_some_and(|(x, y)| pair(x, y))
        }) && m["distance_mm"]
            .as_f64()
            .is_some_and(|d| (0.0..=1e-7).contains(&d)))
    })
}
fn fact(e: &Evidence, catalog: &Catalog) -> Option<String> {
    catalog.get("solid.combine")?;
    let bs = e.document.as_ref()?["bodies"].as_array()?;
    let mut names = BTreeSet::new();
    for b in bs {
        if !names.insert(name(b)?) {
            return None;
        }
    }
    for b in bs.iter().rev() {
        for a in bs {
            let (a, b) = (name(a)?, name(b)?);
            if a != b && contact(e, a, b) {
                let call = json!({"tool":"execute","arguments":{"command":"solid.combine",
                    "params":{"target":a,"tools":[b],"operation":"join","keep_tools":false}}});
                return Some(format!("{MARKER} {b} is a separate body touching or overlapping {a}; to make one part, join it: {call}"));
            }
        }
    }
    None
}

/// A recipe form or exact match built this conversation; recipes check their own result.
fn recipe_history(request: &Value) -> bool {
    request["messages"].as_array().is_some_and(|ms| {
        ms.iter()
            .filter_map(|m| m["tool_calls"].as_array())
            .flatten()
            .any(|c| {
                c["function"]["name"] == "pilot_fill"
                    || c["id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("pilot_direct_"))
            })
    })
}
pub(super) fn eligible(request: &Value) -> bool {
    !recipe_history(request)
        && one_part(last_user(request))
        && request["tools"]
            .as_array()
            .is_some_and(|ts| ts.iter().any(|t| t["function"]["name"] == "execute"))
}
/// Return only read calls, never a boolean or a replacement for a valid model call.
pub(super) fn prepare(
    request: &mut Value,
    state: &mut State,
    catalog: &Catalog,
    audit: &mut Audit,
    final_reply: bool,
) -> Result<Option<Value>, String> {
    let e = evidence(request);
    if fact(&e, catalog).is_some() {
        return Ok(None);
    }
    if state.reads >= MAX_READS {
        return Ok(None);
    }
    let inspect = json!({"command":"document.inspect","params":{"measure":true}});
    let mut next = None;
    if e.document.is_none()
        && (e.needs_document || (final_reply && e.acted))
        && state.documents < MAX_DOCUMENTS
        && !e.attempted.contains(&read_key(&inspect))
    {
        next = Some(inspect);
        state.documents += 1;
    } else if let Some(bs) = e.document.as_ref().and_then(|d| d["bodies"].as_array()) {
        'pairs: for b in bs
            .iter()
            .rev()
            .filter(|b| final_reply || name(b).is_some_and(|n| e.created.contains(n)))
        {
            for a in bs {
                let Some((an, bn)) = name(a).zip(name(b)) else {
                    continue;
                };
                if an == bn || !near(a, b) {
                    continue;
                }
                for read in reads(an, bn) {
                    if !e.attempted.contains(&read_key(&read)) {
                        next = Some(read);
                        break 'pairs;
                    }
                }
            }
        }
    }
    if let Some(args) = next.filter(|v| catalog.get(v["command"].as_str().unwrap_or("")).is_some())
    {
        state.reads += 1;
        let len = request["messages"].as_array().map_or(0, Vec::len);
        let message = json!({"role":"assistant","content":null,"tool_calls":[{
            "id":format!("pilot_bodyhint_{len}_{}",state.reads),"type":"function",
            "function":{"name":"execute","arguments":args.to_string()}}]});
        audit.record("body_hint_read", &Value::Null, &message)?;
        return Ok(Some(message));
    }
    Ok(None)
}
pub(super) fn append(
    request: &mut Value,
    state: &mut State,
    catalog: &Catalog,
    audit: &mut Audit,
) -> Result<(), String> {
    let e = evidence(request);
    if let Some(f) = fact(&e, catalog) {
        if state.seen.len() < 32 && state.seen.insert(f.clone()) {
            if let Some(m) = request["messages"]
                .as_array_mut()
                .and_then(|ms| ms.iter_mut().rev().find(|m| m["role"] == "tool"))
            {
                if let Some(s) = m["content"].as_str() {
                    m["content"] = json!(format!("{s}\n{f}"));
                } else if let Some(blocks) = m["content"].as_array_mut() {
                    blocks.push(json!({"type":"text","text":f}));
                } else {
                    m["content"] = json!(format!("{}\n{f}", m["content"]));
                }
                audit.record("body_hint", &Value::Null, &json!(f))?;
            }
        }
    }
    Ok(())
}
pub(super) fn completion(request: &Value, catalog: &Catalog) -> Option<String> {
    let e = evidence(request);
    if let Some(f) = fact(&e, catalog) {
        return Some(f);
    }
    let n = e.document.as_ref()?["bodies"].as_array()?.len();
    (n > 1).then(|| format!("{MARKER} The design has {n} separate bodies for this one-part request; verify contact and join the intended features, or explain why they must remain separate."))
}

/// Complete, current public document evidence, invalidated by successful mutations.
pub(super) fn size_evidence(request: &Value) -> (Option<Value>, bool) {
    let e = evidence(request);
    (e.document, e.acted)
}
