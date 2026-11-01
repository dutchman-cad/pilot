//! One short design fact for edit requests on a part that already exists.
//!
//! Small models rebuild the part, edit the wrong feature, or place new features
//! without knowing where the body is. On the first turn of an edit-shaped request
//! Pilot reads the design once through public inspection (one `document.inspect`
//! plus one `timeline.inspect` per listed feature) and states the bodies with their
//! bounds, the timeline features with their editable parameters, and, for a plain
//! size change, the exact `timeline.edit` call. It only adds a fact: nothing is
//! rewritten or rejected, and unclear wording adds nothing.
use super::form_context::{detail, document, payloads};
use super::slots::{number, scale, tokens};
use super::*;

pub(crate) const MAX_CHARS: usize = 1200;
pub(crate) const MAX_FEATURES: usize = 12;
const FACT_BODIES: usize = 12;
/// Prefix of the fact; also the marker that prevents repeating it.
pub(crate) const MARKER: &str = "Design:";

pub(crate) enum Step {
    Skip,
    Reads(Vec<Value>),
    Fact(String),
}

/// Edit-shaped wording refers to a part that is already there. A plain creation
/// request ("make a 20 mm cube") and a question are not.
pub(crate) fn edit_shaped(text: &str) -> bool {
    if text.len() > MAX_TEXT {
        return false;
    }
    let t = tokens(text);
    // Two words ("make it") say too little to act on.
    if t.len() < 3 {
        return false;
    }
    let has = |w: &str| t.iter().any(|x| x == w);
    let first = t.first().map(String::as_str).unwrap_or("");
    if matches!(
        first,
        "what"
            | "how"
            | "why"
            | "is"
            | "are"
            | "does"
            | "report"
            | "tell"
            | "count"
            | "list"
            | "show"
            | "explain"
            | "describe"
            | "which"
            | "where"
            | "inspect"
            | "check"
            | "measure"
            | "open"
            | "view"
            | "display"
            | "find"
            | "read"
    ) || text.contains('?')
    {
        return false;
    }
    const PART_NOUNS: &[&str] = &[
        "hole", "holes", "part", "body", "model", "design", "plate", "box", "block", "cube",
        "cylinder", "sphere", "bracket", "base", "tab", "tabs", "ends", "end", "edges", "face",
        "faces", "top", "bottom", "sides", "side", "corners", "corner", "slot", "slots", "boss",
        "pocket", "fillet", "chamfer", "feature", "pin", "lid", "wall", "walls", "flange",
    ];
    let the_noun = t
        .windows(2)
        .any(|w| w[0] == "the" && PART_NOUNS.contains(&w[1].as_str()))
        || t.windows(3).any(|w| {
            w[0] == "the"
                && matches!(
                    w[1].as_str(),
                    "four" | "two" | "three" | "all" | "top" | "bottom" | "first" | "last"
                )
                && PART_NOUNS.contains(&w[2].as_str())
        });
    // Words that point at a part that is already there, wherever they stand.
    let explicit = names_a_feature(text)
        || ["existing", "instead", "those", "these", "same"]
            .iter()
            .any(|w| has(w));
    // Looser references: "the hole", "its corner". A request that builds a
    // new part ("sketch an L ... its corner ... extrude it") is not an edit.
    let strong = explicit || the_noun || has("its");
    const EDIT_VERBS: &[&str] = &[
        "make",
        "change",
        "resize",
        "set",
        "drill",
        "cut",
        "chamfer",
        "fillet",
        "round",
        "bevel",
        "shell",
        "add",
        "move",
        "rotate",
        "scale",
        "mirror",
        "enlarge",
        "shrink",
        "widen",
        "narrow",
        "lengthen",
        "extend",
        "thicken",
        "increase",
        "decrease",
        "reduce",
        "replace",
        "repeat",
        "duplicate",
        "copy",
        "remove",
        "delete",
        "punch",
        "bore",
        "countersink",
        "adjust",
        "edit",
        "update",
        "modify",
        "pattern",
        "turn",
    ];
    let object_pronoun = ["it", "them", "that"].iter().any(|w| has(w))
        && t.iter().any(|w| EDIT_VERBS.contains(&w.as_str()));
    let add_to = t.iter().position(|w| w == "add").is_some_and(|a| {
        t[a + 1..].windows(2).any(|w| {
            w[0] == "to"
                && matches!(
                    w[1].as_str(),
                    "it" | "the" | "this" | "that" | "each" | "both" | "its"
                )
        })
    });
    let creating = t.windows(2).any(|w| {
        matches!(
            w[0].as_str(),
            "make" | "create" | "build" | "design" | "draw" | "model" | "generate" | "sketch"
        ) && (matches!(w[1].as_str(), "a" | "an" | "new") || number(&w[1]).is_some())
    });
    if creating {
        return explicit || add_to;
    }
    strong || object_pronoun || add_to
}

/// A default feature or body name such as Hole2 or Body1 refers to the design.
fn names_a_feature(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for stem in [
        "hole", "box", "body", "cylinder", "sphere", "fillet", "chamfer", "extrude", "cone",
    ] {
        for (at, _) in lower.match_indices(stem) {
            let before_ok = at == 0 || !bytes[at - 1].is_ascii_alphabetic();
            let after = at + stem.len();
            if before_ok && bytes.get(after).is_some_and(u8::is_ascii_digit) {
                return true;
            }
        }
    }
    false
}

fn fresh(request: &Value) -> bool {
    let messages = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .is_some_and(|i| messages[i + 1..].iter().all(|m| m["role"] == "system"))
}

fn user_text(request: &Value) -> String {
    request["messages"]
        .as_array()
        .and_then(|m| m.iter().rev().find(|m| m["role"] == "user"))
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn timeline(doc: &Value) -> Vec<&Value> {
    let all: Vec<_> = doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["rolled_back"] != true && f["name"].is_string())
        .collect();
    all[all.len().saturating_sub(MAX_FEATURES)..].to_vec()
}

fn is_sketch(feature: &Value) -> bool {
    feature["type"] == "Sketch"
}

/// Would an exact recipe plan this request if the design had one unnamed body?
/// The same probe `inspect_for_edit` uses to decide to read the design.
fn planner_could_answer(request: &Value, catalog: &Catalog) -> bool {
    let Some(messages) = request["messages"].as_array() else {
        return false;
    };
    let mut probe = request.clone();
    let mut synthetic = vec![
        json!({"role":"tool","content":json!({"bodies":[{"name":"PilotUnresolvedBody"}]}).to_string()}),
    ];
    synthetic.extend(messages.iter().cloned());
    probe["messages"] = json!(synthetic);
    super::plan_direct(&probe, catalog).is_ok_and(|p| p.is_some())
}

/// Decide the next step. `mine` says Pilot already issued reads for this user
/// turn; `rounds` counts them. The first turn is the only one that gets a fact.
pub(crate) fn step(
    request: &Value,
    catalog: &Catalog,
    mine: bool,
    rounds: usize,
    planner_active: bool,
) -> Result<Step, String> {
    let text = user_text(request);
    if !edit_shaped(&text) || !(mine || fresh(request)) || rounds > 2 {
        return Ok(Step::Skip);
    }
    // An exact recipe answers once it has the design; main reads it itself and
    // the recipe's own calls must not be preceded by ours.
    if !mine && planner_active && planner_could_answer(request, catalog) {
        return Ok(Step::Skip);
    }
    let data = payloads(request);
    let Some(doc) = document(&data) else {
        if mine {
            return Ok(Step::Skip);
        }
        return Ok(Step::Reads(vec![call(
            catalog,
            "document.inspect",
            json!({"measure":true}),
        )?]));
    };
    if doc["bodies"].as_array().is_none_or(Vec::is_empty) {
        return Ok(Step::Skip);
    }
    let features = timeline(doc);
    let mut reads = Vec::new();
    for f in &features {
        if !is_sketch(f) && detail(&data, f).is_none() {
            if let Some(name) = f["name"].as_str().filter(|n| n.len() <= 128) {
                reads.push(call(catalog, "timeline.inspect", json!({"feature":name}))?);
            }
        }
    }
    if !reads.is_empty() {
        return Ok(if mine && rounds < 2 {
            Step::Reads(reads)
        } else {
            Step::Skip
        });
    }
    if !mine {
        return Ok(Step::Skip);
    }
    Ok(build(doc, &features, &data, &text).map_or(Step::Skip, |(fact, _)| Step::Fact(fact)))
}

/// The fact for an edit-shaped request whose design Pilot already read for a
/// form that was then declined: ordinary tools resume with those results in the
/// history, so no further read is needed. Only Pilot's own reads may sit between
/// the user message and now (the first model call of the turn).
pub(crate) fn from_form_reads(request: &Value) -> Option<String> {
    let text = user_text(request);
    if !edit_shaped(&text) {
        return None;
    }
    let messages = request["messages"].as_array()?;
    let user = messages.iter().rposition(|m| m["role"] == "user")?;
    let own = messages[user + 1..].iter().all(|m| {
        m["role"] == "tool"
            || m["role"] == "system"
            || (m["role"] == "assistant"
                && m["tool_calls"].as_array().is_some_and(|calls| {
                    !calls.is_empty()
                        && calls.iter().all(|c| {
                            c["id"]
                                .as_str()
                                .is_some_and(|id| id.starts_with("pilot_context_"))
                        })
                }))
    });
    if !own {
        return None;
    }
    let data = payloads(request);
    let doc = document(&data)?;
    if doc["bodies"].as_array().is_none_or(Vec::is_empty) {
        return None;
    }
    let features = timeline(doc);
    if features
        .iter()
        .any(|f| !is_sketch(f) && detail(&data, f).is_none())
    {
        return None;
    }
    // The model already holds these reads in its history; the fact is worth a
    // system message only when it carries the exact edit command.
    build(doc, &features, &data, &text).and_then(|(fact, exact)| exact.then_some(fact))
}

fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn short(s: &str) -> String {
    s.chars().take(24).collect()
}

fn point(v: &Value) -> Option<String> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    let n: Option<Vec<String>> = a.iter().map(|x| x.as_f64().map(num)).collect();
    Some(format!("({})", n?.join(", ")))
}

fn shape_word(feature_type: &str) -> Option<&'static str> {
    match feature_type {
        "BoxFeature" => Some("box"),
        "CylinderFeature" => Some("cylinder"),
        "SphereFeature" => Some("sphere"),
        "ConeFeature" => Some("cone"),
        _ => None,
    }
}

fn describe_body(body: &Value, details: &[&Value]) -> Option<String> {
    let name = body["name"].as_str().filter(|n| n.len() <= 128)?;
    let bbox = &body["bbox"];
    let min: Vec<f64> = bbox["min"]
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let max: Vec<f64> = bbox["max"]
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    if min.len() != 3 || max.len() != 3 {
        return None;
    }
    let shape = details
        .iter()
        .find(|d| {
            d["definition"]["operation"] == "new_body"
                && d["definition"]["body_names"]
                    .as_array()
                    .is_some_and(|b| b.iter().any(|n| n == name))
        })
        .and_then(|d| d["type"].as_str())
        .and_then(shape_word)
        .map(|s| format!("{s} "))
        .unwrap_or_default();
    let size: Vec<String> = (0..3).map(|i| num(max[i] - min[i])).collect();
    let axes = ["X", "Y", "Z"];
    let at: Vec<String> = (0..3)
        .map(|i| format!("{} {}..{}", axes[i], num(min[i]), num(max[i])))
        .collect();
    Some(format!(
        "{name} {shape}{} mm at {}",
        size.join(" x "),
        at.join(", ")
    ))
}

fn describe_feature(feature: &Value, d: Option<&Value>) -> String {
    let name = feature["name"].as_str().unwrap_or("?");
    let mut parts = Vec::new();
    if let Some(d) = d {
        for input in d["inputs"].as_array().into_iter().flatten() {
            let (Some(label), Some(expr)) = (input["label"].as_str(), input["expression"].as_str())
            else {
                continue;
            };
            let unit = match input["unit"].as_str() {
                Some("" | "mm") | None => String::new(),
                Some(u) => format!(" {u}"),
            };
            parts.push(format!(
                "{} {}{unit}",
                short(&label.to_lowercase()),
                short(expr)
            ));
        }
        if d["type"] == "HoleFeature" && d["definition"]["depth"].is_null() {
            parts.push("through".into());
        }
        if let Some(at) = ["position", "center", "base"]
            .iter()
            .find_map(|k| point(&d["definition"][*k]))
        {
            parts.push(format!("at {at}"));
        }
    }
    if feature["suppressed"] == true {
        parts.push("suppressed".into());
    }
    if !feature["error"].is_null() {
        parts.push("error".into());
    }
    if parts.is_empty() {
        name.to_string()
    } else {
        format!("{name} ({})", parts.join(", "))
    }
}

/// The definition key that holds an input, found by label, then by unique value.
fn field_for(d: &Value, label: &str, expression: &str) -> Option<String> {
    fn leaves(prefix: &str, v: &Value, out: &mut Vec<(String, String)>, depth: usize) {
        if depth > 3 {
            return;
        }
        if let Some(map) = v.as_object() {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match v {
                    Value::String(s) => out.push((key, s.clone())),
                    Value::Object(_) => leaves(&key, v, out, depth + 1),
                    _ => (),
                }
            }
        }
    }
    let mut all = Vec::new();
    leaves("", &d["definition"], &mut all, 0);
    let label = label.to_lowercase();
    if let Some((k, _)) = all
        .iter()
        .find(|(k, v)| k.rsplit('.').next() == Some(label.as_str()) && v == expression)
    {
        return Some(k.clone());
    }
    let same: Vec<_> = all.iter().filter(|(_, v)| v == expression).collect();
    match same.as_slice() {
        [(k, _)] => Some(k.clone()),
        _ => None,
    }
}

/// Lengths with units, plus a bare number right after a size word, in mm.
fn lengths(t: &[String]) -> Vec<f64> {
    const SIZE_WORDS: &[&str] = &[
        "diameter",
        "radius",
        "length",
        "width",
        "height",
        "depth",
        "thickness",
        "distance",
        "spacing",
        "size",
        "to",
        "by",
    ];
    let mut out = Vec::new();
    for (i, w) in t.iter().enumerate() {
        let Some(n) = number(w) else { continue };
        let unit = t.get(i + 1).and_then(|u| scale(u));
        let after_size = i > 0 && SIZE_WORDS.contains(&t[i - 1].as_str());
        if let Some(u) = unit {
            out.push(n * u);
        } else if after_size
            && (n.fract() != 0.0
                || !t.get(i + 1).is_some_and(|x| {
                    matches!(x.as_str(), "holes" | "copies" | "times" | "tabs" | "of")
                }))
        {
            out.push(n);
        }
    }
    out
}

/// Size words in the request, with "long", "wide", "tall" and the like folded in.
fn attribute_words(t: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (word, name) in [
        ("diameter", "diameter"),
        ("radius", "radius"),
        ("length", "length"),
        ("long", "length"),
        ("width", "width"),
        ("wide", "width"),
        ("height", "height"),
        ("tall", "height"),
        ("high", "height"),
        ("depth", "depth"),
        ("deep", "depth"),
        ("thickness", "thickness"),
        ("thick", "thickness"),
        ("distance", "distance"),
        ("spacing", "spacing"),
    ] {
        if t.iter().any(|x| x == word) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// The exact edit for a plain size change on one existing feature, when the
/// request names it unambiguously.
fn size_edit(text: &str, features: &[&Value], data: &[Value]) -> Option<String> {
    let t = tokens(text);
    let has = |w: &str| t.iter().any(|x| x == w);
    let cue = [
        "make", "change", "set", "resize", "enlarge", "increase", "reduce", "shrink", "widen",
        "narrow", "lengthen", "shorten", "thicken", "adjust", "edit", "update", "modify",
        "instead", "bigger", "larger", "smaller", "wider", "longer", "shorter", "taller",
    ]
    .iter()
    .any(|w| has(w));
    let new_things = [
        "add",
        "drill",
        "another",
        "second",
        "extra",
        "additional",
        "new",
        "row",
        "array",
        "pattern",
        "copies",
        "grid",
        "mirror",
        "cut",
        "chamfer",
        "fillet",
        "round",
        "move",
        "rename",
    ]
    .iter()
    .any(|w| has(w));
    if !cue || new_things {
        return None;
    }
    let values = lengths(&t);
    let [value] = values.as_slice() else {
        return None;
    };
    if *value <= 0.0 {
        return None;
    }
    // Candidate features: named in the request, else the one of the named kind.
    let lower = text.to_lowercase();
    let named: Vec<_> = features
        .iter()
        .filter(|f| {
            f["name"]
                .as_str()
                .is_some_and(|n| lower.contains(&n.to_lowercase()))
        })
        .collect();
    let kinds: &[(&[&str], &str)] = &[
        (&["hole", "holes"], "HoleFeature"),
        (&["box", "block", "plate", "cube", "slab"], "BoxFeature"),
        (&["cylinder", "pin", "rod", "boss"], "CylinderFeature"),
        (&["sphere", "ball"], "SphereFeature"),
        (&["fillet", "round", "rounds"], "FilletFeature"),
        (&["chamfer"], "ChamferFeature"),
        (&["extrusion", "extrude"], "ExtrudeFeature"),
    ];
    let feature: &Value = if let [f] = named.as_slice() {
        f
    } else if named.is_empty() {
        let kind = kinds
            .iter()
            .filter(|(words, _)| words.iter().any(|w| has(w)))
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>();
        let same: Vec<_> = if let [ty] = kind.as_slice() {
            features
                .iter()
                .filter(|f| f["type"] == *ty && f["suppressed"] != true)
                .collect()
        } else if kind.is_empty() {
            // "the part 50 mm wide": the one feature with that parameter.
            let attrs = attribute_words(&t);
            let [attr] = attrs.as_slice() else {
                return None;
            };
            features
                .iter()
                .filter(|f| {
                    f["suppressed"] != true
                        && detail(data, f).is_some_and(|d| {
                            d["inputs"].as_array().is_some_and(|i| {
                                i.iter().any(|i| {
                                    i["label"]
                                        .as_str()
                                        .is_some_and(|l| l.eq_ignore_ascii_case(attr))
                                })
                            })
                        })
                })
                .collect()
        } else {
            return None;
        };
        let [f] = same.as_slice() else {
            return None;
        };
        f
    } else {
        return None;
    };
    let d = detail(data, feature)?;
    let inputs = d["inputs"].as_array()?;
    // The size word picks the input; a lone input needs none.
    let said = attribute_words(&t);
    let input = match said.as_slice() {
        [] => {
            let usable: Vec<_> = inputs
                .iter()
                .filter(|i| i["unit"].as_str() == Some("mm"))
                .collect();
            match usable.as_slice() {
                [only] => *only,
                _ if d["type"] == "HoleFeature" => inputs
                    .iter()
                    .find(|i| i["label"].as_str() == Some("Diameter"))?,
                _ => return None,
            }
        }
        [w] => inputs.iter().find(|i| {
            i["label"]
                .as_str()
                .is_some_and(|l| l.eq_ignore_ascii_case(w))
        })?,
        _ => return None,
    };
    if input["unit"].as_str() != Some("mm") {
        return None;
    }
    let field = field_for(d, input["label"].as_str()?, input["expression"].as_str()?)?;
    let name = feature["name"].as_str()?;
    Some(format!(
        "To change {name}'s {}, edit it in place (do not rebuild the part): execute timeline.edit {}.",
        input["label"].as_str()?.to_lowercase(),
        json!({"feature":name,"set":{field:num(*value)}})
    ))
}

const GENERIC_EDIT: &str = "To resize an existing feature: execute timeline.edit {\"feature\":\"<name>\",\"set\":{\"<field>\":\"<mm>\"}}; the field is the parameter name above.";

fn build(doc: &Value, features: &[&Value], data: &[Value], text: &str) -> Option<(String, bool)> {
    let details: Vec<&Value> = features.iter().filter_map(|f| detail(data, f)).collect();
    let bodies: Vec<String> = doc["bodies"]
        .as_array()?
        .iter()
        .take(FACT_BODIES)
        .filter_map(|b| describe_body(b, &details))
        .collect();
    if bodies.is_empty() {
        return None;
    }
    let total = doc["timeline"].as_array().map_or(0, Vec::len);
    let mut listed: Vec<String> = features
        .iter()
        .map(|f| describe_feature(f, detail(data, f)))
        .collect();
    let exact = size_edit(text, features, data);
    let tail = exact.clone().unwrap_or_else(|| GENERIC_EDIT.to_string());
    let more = doc["bodies"]
        .as_array()
        .map_or(0, Vec::len)
        .saturating_sub(FACT_BODIES);
    loop {
        let mut out = format!("{MARKER} {}.", bodies.join("; "));
        if more > 0 {
            out.push_str(&format!(" (+{more} more bodies)"));
        }
        if !listed.is_empty() {
            let hidden = total.saturating_sub(listed.len());
            let note = if hidden > 0 {
                format!(" (last {} of {total})", listed.len())
            } else {
                String::new()
            };
            out.push_str(&format!(" Features{note}: {}.", listed.join(", ")));
        }
        let with_tail = format!("{out} {tail}");
        if with_tail.chars().count() <= MAX_CHARS {
            return Some((with_tail, exact.is_some()));
        }
        if out.chars().count() <= MAX_CHARS && exact.is_none() {
            return Some((out, false));
        }
        if listed.pop().is_none() {
            // Bodies alone are over the cap: cut at a body boundary.
            return Some((out.chars().take(MAX_CHARS).collect(), false));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn edit_wording_is_recognised_and_creation_is_not() {
        for yes in [
            "change the hole to 9 mm instead",
            "drill a 7 mm hole through the middle of it",
            "chamfer the top four corners by 1.5 mm",
            "add two mounting tabs on the ends of the plate",
            "turn those holes into a row of 5",
            "add a fillet to it",
            "Please change the existing plate to 80 mm long",
        ] {
            assert!(edit_shaped(yes), "{yes}");
        }
        for no in [
            "make a 20 mm cube",
            "create a new plate 60x40x10 with a 6 mm hole",
            "how many holes are there?",
            "what is the volume",
            "build a bracket",
            "inspect the design",
            "make it",
            "On XZ, sketch an L with its outside corner at the origin, 50 mm wide, 40 mm tall. Fully constrain it with four driving dimensions, then extrude 30 mm.",
            "Make an enclosure: a box 80 x 50 x 30 mm, hollowed out with 2 mm walls and the top left open. Then add a lip around the top rim for the lid.",
            "",
        ] {
            assert!(!edit_shaped(no), "{no}");
        }
    }

    #[test]
    fn requests_an_exact_recipe_answers_get_no_edit_read() {
        let catalog = Catalog::bundled().unwrap();
        let request = |text: &str| json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        for text in [
            "move it 50 mm along X",
            "rename the body to Mount",
            "mirror it across YZ",
            "fillet all the edges 2 mm",
        ] {
            assert!(planner_could_answer(&request(text), &catalog), "{text}");
            let step = step(&request(text), &catalog, false, 0, true).unwrap();
            assert!(matches!(step, Step::Skip), "{text}");
        }
        // Without recipes active, or for wording no recipe takes, the read is allowed.
        let text = "move it 50 mm along X";
        assert!(matches!(
            step(&request(text), &catalog, false, 0, false).unwrap(),
            Step::Reads(_)
        ));
        let text = "add a pair of lugs to the plate";
        assert!(!planner_could_answer(&request(text), &catalog));
    }

    #[test]
    fn numbers_trim_and_noise_is_zero() {
        assert_eq!(num(10.000000000000005), "10");
        assert_eq!(num(-4.9e-16), "0");
        assert_eq!(num(2.5), "2.5");
    }
}
