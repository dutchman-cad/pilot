//! Named edits use public timeline definitions and certified, feature-owned faces only.
use super::*;

pub fn schema() -> Value {
    json!({"type":"object","additionalProperties":false,"x-pilot-expander":"named_feature",
        "properties":{"feature":{"type":"string","minLength":1,"maxLength":128},
        "parameter":{"type":"string","description":"Inspected dimension or world coordinate","enum":["depth","height","width","length","diameter","x","y","z","start_offset"]},
        "mode":{"type":"string","description":"set: absolute mm; change: signed relative mm","enum":["set","change"]},"value":{"type":"number","minimum":-1000000,"maximum":1000000}},
        "required":["feature","parameter","mode","value"]})
}
fn words(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
pub fn candidate(text: &str) -> bool {
    if text.len() > 2048 {
        return false;
    }
    let w = words(text);
    let has = |s: &str| w.iter().any(|v| v == s);
    let edit = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "raise"
                | "lower"
                | "deepen"
                | "widen"
                | "shift"
                | "move"
                | "resize"
                | "make"
                | "change"
                | "increase"
                | "reduce"
                | "decrease"
                | "set"
                | "extend"
                | "trim"
                | "thicken"
                | "taller"
                | "shorter"
                | "deeper"
                | "shallower"
                | "wider"
                | "narrower"
                | "bigger"
                | "smaller"
                | "higher"
                | "thicker"
                | "thinner"
                | "adjust"
                | "alter"
                | "grow"
                | "shrink"
                | "lengthen"
                | "shorten"
                | "broaden"
        )
    }) || lower_phrase(&w, &["should", "be"])
        || lower_phrase(&w, &["needs", "to", "be"])
        || lower_phrase(&w, &["up", "by"])
        || lower_phrase(&w, &["down", "to"]);
    let feature_move = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "hole" | "pocket" | "recess" | "slot" | "pad" | "boss" | "rib" | "tab" | "counterbore"
        )
    });
    edit && !(has("move") && has("offset") && !feature_move)
        && !(has("move")
            && (text.contains(',')
                || slots::tokens(text)
                    .iter()
                    .filter(|v| slots::number(v).is_some())
                    .count()
                    >= 3))
        && (!has("make")
            || has("the")
            || has("this")
            || w.iter().filter(|v| v.parse::<f64>().is_ok()).count() <= 1)
        && !(has("make")
            && !has("the")
            && !has("this")
            && !has("existing")
            && !w.iter().any(|v| {
                matches!(
                    v.as_str(),
                    "height"
                        | "depth"
                        | "width"
                        | "length"
                        | "diameter"
                        | "distance"
                        | "across"
                        | "tall"
                        | "high"
                        | "thick"
                        | "deep"
                        | "thicker"
                        | "thinner"
                        | "taller"
                        | "shorter"
                        | "deeper"
                        | "shallower"
                        | "wider"
                        | "narrower"
                        | "bigger"
                        | "smaller"
                )
            }))
        && slots::tokens(text)
            .iter()
            .any(|v| slots::number(v).is_some())
        && !(has("make") && (has("a") || has("an")))
        && !lower_phrase(&w, &["whole", "body"])
        && !lower_phrase(&w, &["move", "it"])
        && !w.iter().any(|v| {
            matches!(
                v.as_str(),
                "add"
                    | "create"
                    | "new"
                    | "another"
                    | "copy"
                    | "copies"
                    | "row"
                    | "repeat"
                    | "pattern"
                    | "mirror"
                    | "fillet"
                    | "chamfer"
                    | "shell"
                    | "not"
                    | "no"
                    | "never"
                    | "don"
                    | "dont"
                    | "cannot"
                    | "without"
                    | "what"
                    | "which"
                    | "why"
                    | "explain"
                    | "describe"
                    | "how"
                    | "and"
                    | "then"
                    | "all"
                    | "each"
                    | "two"
                    | "both"
            )
        })
}
fn lower_phrase(words: &[String], phrase: &[&str]) -> bool {
    words
        .windows(phrase.len())
        .any(|w| w.iter().zip(phrase).all(|(a, b)| a == b))
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| {
            v.as_str()
                .and_then(|s| s.trim().strip_suffix(" mm").unwrap_or(s).parse().ok())
        })
        .filter(|v| v.is_finite() && v.abs() <= MAX_MM)
}
fn volume(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite() && *v >= 0.)
}
fn point(v: &Value) -> Option<[f64; 3]> {
    Some([number(&v[0])?, number(&v[1])?, number(&v[2])?])
}
fn axis(v: [f64; 3]) -> Option<(usize, f64)> {
    (0..3)
        .find(|i| v[*i].abs() > 0.999999 && (0..3).filter(|j| j != i).all(|j| v[j].abs() < 1e-6))
        .map(|i| (i, v[i].signum()))
}
#[derive(Clone)]
struct Feature {
    name: String,
    body: String,
    id: u64,
    definition: Value,
    kind: String,
    type_name: String,
    faces: Vec<Value>,
    direction: Option<(usize, f64)>,
    context_faces: Vec<Value>,
    volume: Option<f64>,
    bbox: Value,
}
fn features(request: &Value) -> Vec<Feature> {
    let data = form_context::payloads(request);
    let Some(doc) = form_context::document(&data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // A partial timeline is never enough to claim a unique target.
    if doc["timeline"].as_array().is_none_or(|a| a.len() > 64) {
        return out;
    }
    if doc["bodies"].as_array().is_none_or(|a| a.len() > 12) {
        return out;
    }
    for f in doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|f| f["suppressed"] != true && f["rolled_back"] != true && f["error"].is_null())
    {
        let Some(d) = form_context::detail(&data, f) else {
            continue;
        };
        let Some(id) = f["id"].as_u64() else { continue };
        let Some(name) = f["name"].as_str().filter(|s| s.len() <= 128) else {
            continue;
        };
        let def = &d["definition"];
        let kind = match f["type"].as_str() {
            Some("HoleFeature") => {
                if def["hole"]["type"] == "counterbore" {
                    "counterbore"
                } else {
                    "hole"
                }
            }
            Some("ExtrudeFeature" | "BoxFeature" | "CylinderFeature") => {
                if def["operation"] == "cut" {
                    "pocket"
                } else {
                    "pad"
                }
            }
            _ => continue,
        };
        let prefix = format!("F{id}:");
        let owned: Vec<_> = data
            .iter()
            .filter(|v| v["faces"].as_array().is_some_and(|a| a.len() <= 2048))
            .filter_map(|v| {
                let faces: Vec<_> = v["faces"]
                    .as_array()?
                    .iter()
                    .filter(|f| {
                        f["exact"] == true
                            && f["name"].as_str().is_some_and(|n| n.starts_with(&prefix))
                    })
                    .cloned()
                    .collect();
                (!faces.is_empty()).then(|| (v["body"].as_str().unwrap_or("").to_string(), faces))
            })
            .collect();
        // Use latest face read per body, without treating repeated inspections as ambiguity.
        let mut bodies = std::collections::BTreeMap::new();
        for (b, faces) in owned {
            bodies.insert(b, faces);
        }
        if bodies.len() != 1 {
            continue;
        }
        let Some((body, faces)) = bodies.into_iter().next() else {
            continue;
        };
        if body.is_empty() || body.len() > 128 {
            continue;
        }
        let direction = if f["type"] == "HoleFeature" {
            point(&def["direction"]).and_then(axis)
        } else if f["type"] == "CylinderFeature" {
            point(&def["axis"]).and_then(axis)
        } else if f["type"] == "BoxFeature" {
            Some((2, 1.))
        } else {
            data.iter()
                .rev()
                .find(|s| s["id"] == def["sketch"] && s["definition"].is_object())
                .and_then(|s| {
                    let p = &s["definition"]["plane"];
                    let base = if p["type"] == "offset" { &p["base"] } else { p };
                    let a = match base["name"].as_str()? {
                        "XY" => 2,
                        "XZ" => 1,
                        "YZ" => 0,
                        _ => return None,
                    };
                    if p["flip"] == true || base["flip"] == true {
                        return None;
                    }
                    let sign = match def["extent"]["direction"].as_str() {
                        Some("positive" | "symmetric") => 1.,
                        Some("negative") => -1.,
                        _ => return None,
                    };
                    Some((a, sign))
                })
        };
        let kind = if kind == "pocket"
            && faces.iter().any(|v| v["cylinder"].is_object())
            && !faces.iter().any(|v| {
                v["name"].as_str().is_some_and(|n| n.contains(":side:")) && v["cylinder"].is_null()
            }) {
            "hole"
        } else {
            kind
        };
        out.push(Feature {
            name: name.into(),
            body: body.clone(),
            id,
            definition: def.clone(),
            kind: kind.into(),
            type_name: f["type"].as_str().unwrap_or("").into(),
            faces,
            direction,
            context_faces: data
                .iter()
                .rev()
                .find(|d| d["body"] == body && d["faces"].is_array())
                .and_then(|d| d["faces"].as_array())
                .cloned()
                .unwrap_or_default(),
            bbox: doc["bodies"]
                .as_array()
                .and_then(|bs| bs.iter().find(|b| b["name"] == body))
                .map(|b| b["bbox"].clone())
                .unwrap_or(Value::Null),
            volume: doc["bodies"]
                .as_array()
                .and_then(|bs| bs.iter().find(|b| b["name"] == body))
                .and_then(|b| volume(&b["volume_mm3"])),
        });
    }
    out
}
fn matching(request: &Value) -> Vec<Feature> {
    let text = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let w = words(text);
    let lower = text.to_ascii_lowercase();
    let fs = features(request);
    let named: Vec<_> = fs
        .iter()
        .filter(|f| {
            words(&lower)
                .windows(words(&f.name).len().max(1))
                .any(|v| v == words(&f.name))
        })
        .map(|f| f.name.clone())
        .collect();
    let bodies: Vec<_> = fs
        .iter()
        .filter(|f| {
            words(&lower)
                .windows(words(&f.body).len().max(1))
                .any(|v| v == words(&f.body))
        })
        .map(|f| f.body.clone())
        .collect();
    if w.windows(2)
        .any(|v| v[0] == "body" && !matches!(v[1].as_str(), "by" | "to" | "the"))
        && bodies.is_empty()
    {
        return Vec::new();
    }
    let kind_requested = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "pocket"
                | "recess"
                | "slot"
                | "boss"
                | "pad"
                | "rib"
                | "tab"
                | "hole"
                | "counterbore"
                | "plate"
        )
    });
    let mut matches: Vec<_> = fs
        .into_iter()
        .filter(|f| {
            let kind = w.iter().any(|v| match v.as_str() {
                "pocket" | "recess" | "slot" => {
                    f.kind == "pocket" || (f.kind == "hole" && f.type_name != "HoleFeature")
                }
                "boss" | "pad" | "rib" | "tab" => f.kind == "pad",
                "hole" => f.kind == "hole" || f.kind == "counterbore",
                "counterbore" => f.kind == "counterbore",
                "plate" => f.type_name == "BoxFeature" && f.definition["operation"] == "new_body",
                _ => false,
            });
            (!(f.definition["operation"] == "new_body")
                || named.contains(&f.name)
                || (w.iter().any(|v| v == "plate") && kind)
                || (!bodies.is_empty() && !kind))
                && (if named.is_empty() {
                    if kind_requested {
                        kind
                    } else {
                        !bodies.is_empty() && f.definition["operation"] == "new_body"
                    }
                } else {
                    named.contains(&f.name)
                })
                && (bodies.is_empty() || bodies.contains(&f.body))
        })
        .collect();
    let side = [
        ("left", 0, false),
        ("right", 0, true),
        ("front", 1, false),
        ("back", 1, true),
        ("highest", 2, true),
        ("lowest", 2, false),
    ]
    .into_iter()
    .filter(|(word, _, _)| w.iter().any(|v| v == word))
    .collect::<Vec<_>>();
    if let [(.., a, maximum)] = side.as_slice() {
        let positions: Option<Vec<_>> = matches
            .iter()
            .map(|f| {
                // The terminal plane's centre, or a cylinder axis, is stable and geometric.
                let v = f
                    .faces
                    .iter()
                    .find(|v| v["cylinder"].is_object())
                    .map(|v| v["cylinder"]["origin"].clone())
                    .or_else(|| {
                        f.faces
                            .iter()
                            .find(|v| {
                                v["name"]
                                    .as_str()
                                    .is_some_and(|n| n.ends_with(":bottom") || n.ends_with(":top"))
                            })
                            .map(|v| v["centroid"].clone())
                    })?;
                number(&v[*a])
            })
            .collect();
        if let Some(positions) = positions {
            let extreme =
                positions
                    .iter()
                    .copied()
                    .reduce(|a, b| if *maximum { a.max(b) } else { a.min(b) });
            if let Some(extreme) = extreme {
                let keep: Vec<_> = positions
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| (**v - extreme).abs() < 0.001)
                    .map(|(i, _)| i)
                    .collect();
                if let [index] = keep.as_slice() {
                    matches = vec![matches[*index].clone()];
                }
            }
        }
    }
    matches
}
pub fn reads(request: &Value, catalog: &Catalog) -> Result<Vec<Value>, String> {
    let data = form_context::payloads(request);
    let Some(doc) = form_context::document(&data) else {
        return form_context::reads(request, false, catalog);
    };
    if doc["timeline"].as_array().is_none_or(|t| t.len() > 64) {
        return Ok(Vec::new());
    }
    let mut calls = Vec::new();
    for f in doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|f| f["suppressed"] != true && f["rolled_back"] != true && f["error"].is_null())
    {
        if form_context::detail(&data, f).is_none() {
            calls.push(call(
                catalog,
                "timeline.inspect",
                json!({"feature":f["name"]}),
            )?);
        }
    }
    if !calls.is_empty() {
        return Ok(calls);
    }
    {
        for b in doc["bodies"].as_array().into_iter().flatten().take(12) {
            if !data
                .iter()
                .any(|d| d["body"] == b["name"] && d["faces"].is_array())
            {
                calls.push(call(catalog, "model.faces", json!({"body":b["name"]}))?);
            }
        }
    }
    Ok(std::mem::take(&mut calls))
}
pub fn form(request: &Value) -> Value {
    let mut s = schema();
    let fs = matching(request);
    s["properties"]["feature"]["enum"] = json!(fs.iter().map(|f| &f.name).collect::<Vec<_>>());
    let mut parameters = Vec::new();
    for f in &fs {
        for p in preferred(request, f).into_iter().chain([
            "height",
            "depth",
            "diameter",
            "width",
            "length",
            "x",
            "y",
            "z",
            "start_offset",
        ]) {
            if available(f, p) && !parameters.contains(&p) {
                parameters.push(p);
            }
        }
    }
    s["properties"]["parameter"]["enum"] = json!(parameters);
    s["properties"]["parameter"]["enumNames"] = json!(parameters
        .iter()
        .map(|p| {
            match *p {
                "height" => "Height (mm)",
                "depth" => "Depth (mm)",
                "diameter" => "Diameter (mm)",
                "width" => "Width (mm)",
                "length" => "Length (mm)",
                "start_offset" => "Start offset (mm)",
                "x" => "X position (mm)",
                "y" => "Y position (mm)",
                _ => "Z position (mm)",
            }
        })
        .collect::<Vec<_>>());
    s["properties"]["feature"]["description"] = json!(fs
        .iter()
        .map(|f| format!(
            "{}: {} ({}) in body {}; size {}; position {}",
            f.name,
            f.kind,
            f.type_name,
            f.body,
            json!([
                "depth",
                "height",
                "width",
                "length",
                "diameter",
                "start_offset"
            ]
            .iter()
            .filter_map(|p| field(f, p).map(|(_, n)| {
                let size = witness(f, p)
                    .and_then(|(_, m, _)| exposed_size(f, p, m))
                    .unwrap_or(n);
                (p, json!(size))
            }))
            .collect::<std::collections::BTreeMap<_, _>>()),
            f.faces
                .first()
                .map(|v| v["centroid"].clone())
                .unwrap_or(Value::Null)
        ))
        .collect::<Vec<_>>()
        .join("; "));
    s
}
fn user_text(request: &Value) -> &str {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
}
fn available(f: &Feature, p: &str) -> bool {
    (if matches!(p, "x" | "y" | "z") {
        f.definition["position"].is_array() || f.definition["base"].is_array()
    } else {
        field(f, p).is_some()
    }) && witness(f, p).is_some()
}
// Semantic groups are shared by exact parsing, form ordering and bounded recovery.
fn intentions(request: &Value, f: &Feature) -> Vec<&'static str> {
    let w = words(user_text(request));
    let has = |terms: &[&str]| w.iter().any(|v| terms.contains(&v.as_str()));
    let axial = if f.kind == "pad" { "height" } else { "depth" };
    let mut out = Vec::new();
    if has(&[
        "height",
        "tall",
        "high",
        "thick",
        "taller",
        "thickness",
        "thicker",
        "thinner",
        "higher",
    ]) {
        out.push("height");
    }
    if has(&["raise", "lower"]) {
        out.push(axial);
    }
    if has(&["depth", "deep", "deeper", "deepen", "shallower"]) {
        out.push("depth");
    }
    if has(&["distance"]) {
        out.push(axial);
    }
    if has(&["radius"]) {
        out.push("radius");
    }
    if has(&["position"]) && !has(&["x", "y", "z"]) {
        out.push("position");
    }
    if has(&["diameter", "across"]) {
        out.push("diameter");
    }
    if has(&["widen", "wider", "narrower", "width"]) {
        out.push(if f.faces.iter().any(|v| v["cylinder"].is_object()) {
            "diameter"
        } else {
            "width"
        });
    }
    if has(&["length"]) {
        out.push("length");
    }
    for p in ["x", "y", "z"] {
        if has(&[p]) {
            out.push(p);
        }
    }
    // Offset in a move request belongs to the existing move recipe.
    if has(&["offset"]) && (has(&["start"]) || !has(&["move", "shift"])) {
        out.push("start_offset");
    }
    out.sort_unstable();
    out.dedup();
    out
}
fn unambiguous(request: &Value, f: &Feature) -> bool {
    intentions(request, f).len() == 1
}
fn preferred(request: &Value, f: &Feature) -> Vec<&'static str> {
    let mut out = intentions(request, f);
    let w = words(user_text(request));
    if out.is_empty()
        && w.iter().any(|v| {
            matches!(
                v.as_str(),
                "bigger" | "smaller" | "resize" | "grow" | "shrink" | "shorter" | "thicken"
            )
        })
    {
        out.extend([
            if f.kind == "pad" { "height" } else { "depth" },
            "diameter",
            "width",
            "length",
        ]);
    }
    if w.iter().any(|v| matches!(v.as_str(), "raise" | "lower"))
        && !w.iter().any(|v| {
            matches!(
                v.as_str(),
                "height"
                    | "depth"
                    | "distance"
                    | "tall"
                    | "taller"
                    | "deep"
                    | "deeper"
                    | "shallower"
            )
        })
        && f.direction.is_some_and(|(a, _)| a == 2)
    {
        out.extend(["start_offset", "z"]);
    }
    out.retain(|p| available(f, p));
    out
}
fn parsed(request: &Value) -> Option<Value> {
    let text = request["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|m| m["role"] == "user")?["content"]
        .as_str()?;
    let w = words(text);
    let has = |s: &str| w.iter().any(|v| v == s);
    // Unit-bearing values only: body/feature identifiers may contain digits.
    let t = slots::tokens(text);
    let values: Vec<_> = t
        .windows(2)
        .filter_map(|v| {
            slots::scale(&v[1]).and_then(|scale| slots::number(&v[0]).map(|n| n * scale))
        })
        .collect();
    let [value] = values.as_slice() else {
        return None;
    };
    let fs = matching(request);
    let [f] = fs.as_slice() else { return None };
    let choices = preferred(request, f);
    let parameter = *choices.first()?;
    // A word must identify the dimension itself. A feature noun alone is not intent.
    if !unambiguous(request, f) || !available(f, parameter) {
        return None;
    }
    let relative = has("by")
        || (!has("to")
            && [
                "wider",
                "deeper",
                "taller",
                "shorter",
                "shallower",
                "higher",
                "narrower",
                "thicker",
                "thinner",
            ]
            .iter()
            .any(|s| has(s)));
    if !relative && !has("to") && !has("make") && !has("resize") && !has("set") && !has("be") {
        return None;
    }
    let value = if relative
        && (has("lower")
            || has("shorter")
            || has("shallower")
            || has("negative")
            || has("reduce")
            || has("decrease")
            || has("trim")
            || has("narrower")
            || has("thinner")
            || has("down")
            || has("shrink")
            || has("shorten"))
    {
        -value.abs()
    } else {
        *value
    };
    Some(json!({"parameter":parameter,"mode":if relative{"change"}else{"set"},"value":value}))
}
pub fn exact(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    let fs = matching(request);
    let [f] = fs.as_slice() else { return Ok(None) };
    let Some(mut args) = parsed(request) else {
        return Ok(None);
    };
    if f.definition["extent"]["direction"] == "symmetric" || f.kind == "counterbore" {
        return Ok(None);
    }
    args["feature"] = json!(f.name);
    // Unsupported geometry stays on the form path; it never guesses a field.
    Ok(build(&args, request, catalog)
        .ok()
        .filter(|p| p.slots["attachments"].as_array().is_none_or(Vec::is_empty)))
}
fn field(f: &Feature, p: &str) -> Option<(String, f64)> {
    let d = &f.definition;
    if matches!(p, "height" | "depth" | "start_offset")
        && d["extent"].is_object()
        && (d["extent"]["distance2"].is_string()
            || d["extent"]["distance2"].is_number()
            || d["extent"]["through_all"] == true
            || d["extent"].get("to").is_some())
    {
        return None;
    }
    if (p == "height" && (f.kind == "hole" || f.kind == "counterbore"))
        || (p == "depth" && f.kind == "pad")
    {
        return None;
    }
    if f.kind == "counterbore" && matches!(p, "depth" | "diameter") {
        let path = if p == "depth" {
            "hole.cb_depth"
        } else {
            "hole.cb_diameter"
        };
        return Some((
            path.into(),
            number(d.pointer(&format!("/{}", path.replace('.', "/")))?)?,
        ));
    }
    if p == "diameter" && d.get("radius").is_some() {
        let radius = number(&d["radius"]).or_else(|| {
            f.faces
                .iter()
                .find_map(|v| number(&v["cylinder"]["radius"]))
        })?;
        return Some(("radius".into(), radius * 2.));
    }
    let path = match p {
        "depth" if f.type_name == "CylinderFeature" => "height",
        "depth" | "height" if d["extent"].is_object() => "extent.distance",
        "depth" => "depth",
        "height" => "height",
        "width" => "width",
        "length" => "length",
        "diameter" => "diameter",
        "start_offset" if d["extent"].is_object() => "extent.start_offset",
        _ => return None,
    };
    let v = d.pointer(&format!("/{}", path.replace('.', "/")))?;
    Some((path.into(), number(v)?))
}
// Geometry witness: a certified feature-owned plane or cylinder. Its displacement,
// radius or span must change by the computed amount; parameter echo alone is insufficient.
fn witness(f: &Feature, p: &str) -> Option<(Value, f64, f64)> {
    if matches!(p, "x" | "y" | "z") {
        if let Some(face) = f.faces.iter().find(|v| v["cylinder"].is_object()) {
            let a = match p {
                "x" => 0,
                "y" => 1,
                _ => 2,
            };
            return Some((
                json!({"face":face["name"],"kind":"origin","axis":a}),
                number(&face["cylinder"]["origin"][a])?,
                1.,
            ));
        }
    }
    if p == "diameter" {
        let face = f.faces.iter().find(|v| {
            v["cylinder"].is_object()
                && (f.kind != "counterbore"
                    || v["name"].as_str().is_some_and(|n| n.contains(":cbore:")))
        })?;
        return Some((
            json!({"face":face["name"],"kind":"diameter"}),
            number(&face["cylinder"]["radius"])? * 2.,
            1.,
        ));
    }
    let (a, sign) = if matches!(p, "depth" | "height" | "start_offset") {
        f.direction?
    } else {
        (
            match p {
                "x" | "length" => 0,
                "y" | "width" => 1,
                "z" => 2,
                _ => return None,
            },
            1.,
        )
    };
    let face = if f.definition["extent"].is_object() {
        f.faces.iter().find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.ends_with(":bottom") || n.ends_with(":top"))
                && point(&v["plane_normal"])
                    .and_then(axis)
                    .is_some_and(|(i, _)| i == a)
        })?
    } else if f.type_name == "HoleFeature" {
        f.faces.iter().find(|v| {
            v["name"].as_str().is_some_and(|n| {
                n.ends_with(if f.kind == "counterbore" {
                    ":cbore:base"
                } else {
                    ":bore:base"
                })
            })
        })?
    } else if f.type_name == "CylinderFeature" && matches!(p, "height" | "depth") {
        let terminal =
            number(&f.definition["base"][a])? + f.direction?.1 * number(&f.definition["height"])?;
        f.faces.iter().find(|v| {
            point(&v["plane_normal"])
                .and_then(axis)
                .is_some_and(|(i, _)| i == a)
                && number(&v["centroid"][a]).is_some_and(|v| (v - terminal).abs() < 0.001)
        })?
    } else {
        f.faces
            .iter()
            .find(|v| point(&v["plane_normal"]).and_then(axis) == Some((a, 1.)))?
    };
    Some((
        json!({"face":face["name"],"kind":"plane","axis":a}),
        number(&face["centroid"][a])?,
        if f.definition["extent"]["direction"] == "symmetric"
            && p != "start_offset"
            && f.definition["extent"]["convention"] != "legacy_per_side"
        {
            sign * 0.5
        } else {
            sign
        },
    ))
}
// The exposed size is measured from the supporting material, not necessarily
// from the feature's sketch plane (which may be buried inside that material).
fn exposed_size(f: &Feature, p: &str, measured: f64) -> Option<f64> {
    let (a, sign) = match p {
        "height" | "depth" => f.direction?,
        "length" => (0, 1.),
        "width" => (1, 1.),
        _ => return None,
    };
    let (_, original) = field(f, p)?;
    let scale = witness(f, p)?.2.abs();
    let start = measured - original * scale * sign;
    let prefix = format!("F{}:", f.id);
    let support = f
        .context_faces
        .iter()
        .filter(|v| {
            v["exact"] == true
                && !v["name"].as_str().is_some_and(|n| n.starts_with(&prefix))
                && point(&v["plane_normal"])
                    .and_then(axis)
                    .is_some_and(|(i, _)| i == a)
                && v["name"]
                    .as_str()
                    .and_then(|n| n.split(':').next())
                    .is_some_and(|owner| {
                        // Require the support's footprint to contain the target terminal centre.
                        let terminal = f.faces.iter().find(|q| {
                            number(&q["centroid"][a]).is_some_and(|x| (x - measured).abs() < 0.001)
                        });
                        terminal.is_some_and(|terminal| {
                            (0..3).filter(|i| *i != a).all(|i| {
                                let coordinates: Vec<_> = f
                                    .context_faces
                                    .iter()
                                    .filter(|q| {
                                        q["name"].as_str().and_then(|n| n.split(':').next())
                                            == Some(owner)
                                    })
                                    .filter_map(|q| number(&q["centroid"][i]))
                                    .collect();
                                let x = number(&terminal["centroid"][i]).unwrap_or(f64::NAN);
                                coordinates.iter().any(|v| *v <= x + 0.001)
                                    && coordinates.iter().any(|v| *v >= x - 0.001)
                            })
                        })
                    })
        })
        .filter_map(|v| number(&v["centroid"][a]))
        .filter(|v| (measured - v) * sign > 0.001 && (v - start) * sign >= -0.001)
        .min_by(|x, y| ((measured - x) * sign).total_cmp(&((measured - y) * sign)))?;
    Some((measured - support) * sign)
}
fn volume_change(f: &Feature, p: &str, before: f64, after: f64, witness: &Value) -> Option<f64> {
    let delta = after - before;
    if matches!(p, "x" | "y" | "z") {
        return Some(0.);
    }
    let cut = if matches!(f.kind.as_str(), "pocket" | "hole" | "counterbore") {
        -1.
    } else {
        1.
    };
    if matches!(p, "height" | "depth")
        && (f.type_name == "CylinderFeature"
            || (f.type_name == "ExtrudeFeature" && f.kind == "hole"))
    {
        let radius = f
            .faces
            .iter()
            .find_map(|face| number(&face["cylinder"]["radius"]))?;
        let scale = witness["kind"]
            .as_str()
            .filter(|k| *k == "plane")
            .and_then(|_| self::witness(f, p))
            .map(|(_, _, scale)| scale.abs())?;
        return Some(cut * std::f64::consts::PI * radius * radius * delta * scale);
    }
    if p == "diameter" {
        let depth = if f.kind == "counterbore" {
            number(&f.definition["hole"]["cb_depth"])?
        } else {
            field(f, "depth")
                .or_else(|| field(f, "height"))
                .map(|(_, v)| v)
                .or_else(|| {
                    let (a, _) = f.direction?;
                    Some(number(&f.bbox["max"][a])? - number(&f.bbox["min"][a])?)
                })?
        };
        return Some(cut * std::f64::consts::PI / 4. * (after * after - before * before) * depth);
    }
    if matches!(f.kind.as_str(), "hole" | "counterbore")
        && p == "depth"
        && f.type_name == "HoleFeature"
    {
        let bore = number(&f.definition["diameter"])?;
        let area = if f.kind == "counterbore" {
            let cb = number(&f.definition["hole"]["cb_diameter"])?;
            (cb * cb - bore * bore) * std::f64::consts::PI / 4.
        } else {
            bore * bore * std::f64::consts::PI / 4.
        };
        return Some(-area * delta);
    }
    let face = f.faces.iter().find(|v| v["name"] == witness["face"])?;
    let scale = if f.definition["extent"]["direction"] == "symmetric"
        && f.definition["extent"]["convention"] != "legacy_per_side"
    {
        0.5
    } else {
        1.
    };
    Some(cut * number(&face["area"])? * delta * scale)
}
pub fn build(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(args, &schema())?;
    let fs = matching(request);
    if fs.len() > 1 {
        return Err("args.feature is ambiguous: several inspected features fit equally; ask for a feature name or body before editing".into());
    }
    let f = fs
        .iter()
        .find(|f| args["feature"] == f.name)
        .ok_or_else(|| {
            format!(
                "args.feature {} must be one of the inspected matching feature names: {}",
                args["feature"],
                json!(fs.iter().map(|f| &f.name).collect::<Vec<_>>())
            )
        })?;
    let p = args["parameter"]
        .as_str()
        .ok_or("args.parameter is required")?;
    if !intentions(request, f).is_empty() && !preferred(request, f).contains(&p) {
        return Err("args.parameter conflicts with the requested dimension; choose a parameter matching the request".into());
    }
    let mode = args["mode"]
        .as_str()
        .filter(|s| matches!(*s, "set" | "change"))
        .ok_or("args.mode must be set or change")?;
    let value = number(&args["value"])
        .ok_or("args.value requires a finite millimetre value within 1000000")?;
    let coordinate = match p {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    };
    let (path, before) = if let Some(a) = coordinate {
        (
            if f.definition.get("base").is_some() {
                "base".into()
            } else {
                "position".into()
            },
            number(
                &f.definition[if f.definition.get("base").is_some() {
                    "base"
                } else {
                    "position"
                }][a],
            )
            .ok_or("args.parameter requires an inspected hole position")?,
        )
    } else {
        field(f, p).ok_or(
            "args.parameter is unavailable or its inspected value is an unevaluated expression",
        )?
    };
    let (witness, measured, derivative) = witness(f, p).ok_or(
        "args.parameter has no unique measurable geometry witness; inspect or clarify the feature",
    )?;
    let text = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let w = words(text);
    let vertical = w.iter().any(|v| matches!(v.as_str(), "raise" | "lower"));
    let delta = if mode == "change" && vertical && matches!(p, "depth" | "height") {
        if f.direction.is_none_or(|(a, _)| a != 2) {
            return Err("raise/lower requires an inspected vertical feature direction".into());
        }
        value / derivative
    } else {
        value
    };
    let after = if vertical && mode == "set" && matches!(p, "depth" | "height") {
        before + (value - measured) / derivative
    } else if mode == "change" {
        before
            + if matches!(p, "height" | "depth") && !vertical {
                delta / derivative.abs()
            } else {
                delta
            }
    } else if let Some(size) = exposed_size(f, p, measured) {
        before + (value - size) / derivative.abs()
    } else {
        value
    };
    if !after.is_finite()
        || after.abs() > MAX_MM
        || (!matches!(p, "x" | "y" | "z" | "start_offset") && after <= 0.)
    {
        return Err("args.value produces an invalid feature size; size must be positive and at most 1000000 mm".into());
    }
    if after < before && matches!(p, "height" | "depth") {
        let data = form_context::payloads(request);
        if data.iter().any(|d| {
            d["id"].as_u64().is_some_and(|id| id > f.id)
                && d["definition"]["participants"]
                    .as_array()
                    .is_some_and(|bs| bs.iter().any(|b| b == &f.body))
                && ["corner", "base", "position"].iter().any(|key| {
                    point(&d["definition"][key]).is_some_and(|pos| {
                        f.direction
                            .is_some_and(|(a, _)| (pos[a] - measured).abs() < 0.001)
                    })
                })
        }) {
            return Err("Changing this support would detach a later feature; move its attachment with the support before editing".into());
        }
    }
    let attachments: Vec<_> = form_context::payloads(request)
        .iter()
        .filter(|_| matches!(p, "height" | "depth" | "start_offset"))
        .filter(|d| {
            d["id"].as_u64().is_some_and(|id| id > f.id)
                && d["definition"]["participants"]
                    .as_array()
                    .is_some_and(|bs| bs.iter().any(|b| b == &f.body))
        })
        .filter(|d| {
            ["corner", "base", "position"].iter().any(|key| {
                point(&d["definition"][key]).is_some_and(|pos| {
                    f.direction
                        .is_some_and(|(a, _)| (pos[a] - measured).abs() < 0.001)
                })
            })
        })
        .map(|d| d["name"].clone())
        .collect();
    let mut set = json!({});
    if let Some(a) = coordinate {
        let mut pos = f.definition[&path].clone();
        pos[a] = json!(after);
        set[&path] = pos;
    } else {
        let v = json!(if path == "radius" { after / 2. } else { after });
        if let Some((parent, child)) = path.split_once('.') {
            set[parent] = json!({child: v});
        } else {
            set[&path] = v;
        }
    }
    let expected = measured + derivative * (after - before);
    let mut bbox = f.bbox.clone();
    if witness["kind"] == "plane" && f.kind == "pad" {
        if let Some(a) = witness["axis"].as_u64().map(|n| n as usize) {
            for edge in ["min", "max"] {
                if number(&bbox[edge][a]).is_some_and(|v| (v - measured).abs() < 0.001) {
                    bbox[edge][a] = json!(expected);
                }
            }
        }
    }
    Ok(Plan {
        rule: "named_feature",
        calls: vec![
            call(
                catalog,
                "timeline.edit",
                json!({"feature":f.name,"set":set}),
            )?,
            call(catalog, "model.faces", json!({"body":f.body}))?,
            call(catalog, "timeline.inspect", json!({"feature":f.name}))?,
            call(catalog, "document.inspect", json!({"measure":true}))?,
        ],
        facts: None,
        slots: json!({"feature":f.name,"body":f.body,"id":f.id,"parameter":p,"field":path,"before":before,"after":after,"witness":witness,"measured_before":measured,"measured_after":expected,"attachments":attachments,"bounds_after":bbox,
            "restore_set":original_set(f, &path), "bounds_before":f.bbox,
            "args":args, "recovery_request":request,
            "size_before":exposed_size(f,p,measured).unwrap_or(before),
            "size_after":exposed_size(f,p,measured).unwrap_or(before) + derivative.abs()*(after-before),"volume_before":f.volume,
            "volume_after":f.volume.zip(volume_change(f,p,before,after,&witness)).map(|(v,d)| v+d)}),
        expected: None,
        continuation: None,
    })
}
fn original_set(f: &Feature, path: &str) -> Value {
    if let Some((parent, child)) = path.split_once('.') {
        json!({parent: {child: f.definition[parent][child]}})
    } else {
        json!({path: f.definition[path]})
    }
}
/// Restore the original field, then measure it before considering a single alternative.
pub fn restore(plan: &Plan, catalog: &Catalog) -> Result<Plan, String> {
    let mut restored = plan.clone();
    restored.slots["restoring"] = json!(true);
    for (after, before) in [
        ("after", "before"),
        ("measured_after", "measured_before"),
        ("size_after", "size_before"),
        ("volume_after", "volume_before"),
        ("bounds_after", "bounds_before"),
    ] {
        restored.slots[after] = plan.slots[before].clone();
    }
    restored.slots["attachments"] = json!([]);
    restored.calls[0] = call(
        catalog,
        "timeline.edit",
        json!({"feature":plan.slots["feature"], "set":plan.slots["restore_set"]}),
    )?;
    Ok(restored)
}
pub fn retry(plan: &Plan, catalog: &Catalog) -> Result<Option<Plan>, String> {
    if plan.slots["retried"] == true {
        return Ok(None);
    }
    let request = &plan.slots["recovery_request"];
    let fs = matching(request);
    let [f] = fs.as_slice() else { return Ok(None) };
    let choices = preferred(request, f);
    let Some(index) = choices.iter().position(|p| plan.slots["parameter"] == *p) else {
        return Ok(None);
    };
    let Some(next) = choices.get(index + 1) else {
        return Ok(None);
    };
    let mut args = plan.slots["args"].clone();
    args["parameter"] = json!(next);
    Ok(build(&args, request, catalog).ok().map(|mut p| {
        p.slots["retried"] = json!(true);
        p
    }))
}
fn changed_payloads(request: &Value, plan: &Plan) -> Option<Vec<Value>> {
    let messages = request["messages"].as_array()?;
    let start = messages.iter().rposition(|m| {
        m["role"] == "assistant"
            && m["tool_calls"].as_array().is_some_and(|cs| {
                cs.iter().any(|c| {
                    let args = c["function"]["arguments"]
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| c["function"]["arguments"].clone());
                    args["command"] == "timeline.edit"
                        && args["params"]["feature"] == plan.slots["feature"]
                })
            })
    })?;
    let data: Vec<_> = messages[start + 1..]
        .iter()
        .filter(|m| m["role"] == "tool")
        .filter_map(tool_payload)
        .collect();
    Some(data)
}
fn measured_after(request: &Value, plan: &Plan) -> Option<f64> {
    let data = changed_payloads(request, plan)?;
    let s = &plan.slots;
    let faces = data
        .iter()
        .rev()
        .find(|d| d["body"] == s["body"] && d["faces"].is_array());
    let face = faces.and_then(|d| d["faces"].as_array()).and_then(|fs| {
        fs.iter()
            .find(|f| f["name"] == s["witness"]["face"] && f["exact"] == true)
    });
    let a = s["witness"]["axis"].as_u64().unwrap_or(0) as usize;
    face.and_then(|f| match s["witness"]["kind"].as_str() {
        Some("diameter") => number(&f["cylinder"]["radius"]).map(|r| r * 2.),
        Some("origin") => number(&f["cylinder"]["origin"][a]),
        _ => number(&f["centroid"][a]),
    })
}
pub fn completion(request: &Value, plan: &Plan) -> Option<String> {
    let data = changed_payloads(request, plan).unwrap_or_default();
    let volume_ok = volume(&plan.slots["volume_after"]).is_some_and(|expected| {
        form_context::document(&data)
            .and_then(|d| d["bodies"].as_array())
            .and_then(|bs| bs.iter().find(|b| b["name"] == plan.slots["body"]))
            .and_then(|b| volume(&b["volume_mm3"]))
            .is_some_and(|actual| {
                (actual - expected).abs()
                    <= 0.01_f64.max(
                        (expected - volume(&plan.slots["volume_before"]).unwrap_or(expected)).abs()
                            * 0.002,
                    )
            })
    });
    let bounds_ok = form_context::document(&data)
        .and_then(|d| d["bodies"].as_array())
        .and_then(|bs| bs.iter().find(|b| b["name"] == plan.slots["body"]))
        .is_some_and(|b| {
            ["min", "max"].iter().all(|edge| {
                (0..3).all(|a| {
                    number(&b["bbox"][edge][a])
                        .zip(number(&plan.slots["bounds_after"][edge][a]))
                        .is_some_and(|(actual, expected)| (actual - expected).abs() < 0.001)
                })
            })
        });
    let attachments_ok = plan.slots["attachments"]
        .as_array()
        .is_none_or(Vec::is_empty);
    if attachments_ok
        && volume_ok
        && bounds_ok
        && measured_after(request, plan)
            .zip(number(&plan.slots["measured_after"]))
            .is_some_and(|(a, b)| (a - b).abs() < 0.001)
    {
        None
    } else {
        Some("Named feature edit is unverified: measured geometry does not match the requested change. Inspect the design; do not repeat the edit.".into())
    }
}
pub fn summary(request: &Value, plan: &Plan) -> String {
    if let Some(error) = completion(request, plan) {
        return format!("Exact match (no model): {error}");
    }
    let axis = match plan.slots["witness"]["axis"].as_u64() {
        Some(1) => "Y",
        Some(2) => "Z",
        _ => "X",
    };
    let measurement = match plan.slots["witness"]["kind"].as_str() {
        Some("diameter") => "diameter".into(),
        Some("origin") => format!("axis {axis}"),
        _ => format!(
            "{} {axis}",
            if plan.slots["parameter"] == "depth" {
                "floor"
            } else {
                "surface"
            }
        ),
    };
    format!(
        "Exact match (no model): {} {}: {} → {} mm. Measured {measurement} {} → {} mm.",
        plan.slots["feature"].as_str().unwrap_or("feature"),
        plan.slots["parameter"].as_str().unwrap_or("parameter"),
        plan.slots["size_before"],
        plan.slots["size_after"],
        number(&plan.slots["measured_before"])
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null),
        measured_after(request, plan)
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null)
    )
}
