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
fn noun(w: &str) -> bool {
    matches!(
        w,
        "pocket" | "recess" | "slot" | "hole" | "boss" | "pad" | "rib" | "tab" | "counterbore"
    )
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
            "raise" | "lower" | "deepen" | "widen" | "shift" | "move" | "resize" | "make"
        )
    });
    let existing = has("the") || has("this") || has("existing") || has("body") || !has("make");
    edit && existing
        && w.iter()
            .filter(|v| {
                matches!(
                    v.as_str(),
                    "raise" | "lower" | "deepen" | "widen" | "shift" | "move" | "resize" | "make"
                )
            })
            .count()
            == 1
        && w.iter().any(|v| noun(v))
        && slots::tokens(text)
            .iter()
            .any(|v| slots::number(v).is_some())
        && !w
            .windows(2)
            .any(|v| matches!(v[0].as_str(), "a" | "an") && noun(&v[1]))
        // "make a hexagon pocket" creates one; one adjective may separate them.
        && !w
            .windows(3)
            .any(|v| matches!(v[0].as_str(), "a" | "an") && noun(&v[2]))
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
                    | "don't"
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
fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| {
            v.as_str()
                .and_then(|s| s.trim().strip_suffix(" mm").unwrap_or(s).parse().ok())
        })
        .filter(|v| v.is_finite() && v.abs() <= MAX_MM)
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
}
fn features(request: &Value) -> Vec<Feature> {
    let data = form_context::payloads(request);
    let Some(doc) = form_context::document(&data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // A partial timeline is never enough to claim a unique target.
    if doc["timeline"].as_array().is_none_or(|a| a.len() > 12) {
        return out;
    }
    if doc["bodies"].as_array().is_none_or(|a| a.len() > 12) {
        return out;
    }
    for f in form_context::active_features(doc) {
        let Some(d) = form_context::detail(&data, f) else {
            continue;
        };
        let Some(id) = f["id"].as_u64() else { continue };
        let Some(name) = f["name"].as_str().filter(|s| s.len() <= 128) else {
            continue;
        };
        let def = &d["definition"];
        if f["type"] == "BoxFeature" && def["operation"] == "new_body" {
            continue;
        }
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
                        Some("positive") => 1.,
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
            body,
            id,
            definition: def.clone(),
            kind: kind.into(),
            type_name: f["type"].as_str().unwrap_or("").into(),
            faces,
            direction,
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
    let mut matches: Vec<_> = fs
        .into_iter()
        .filter(|f| {
            let kind = w.iter().any(|v| match v.as_str() {
                "pocket" | "recess" | "slot" => f.kind == "pocket",
                "boss" | "pad" | "rib" | "tab" => f.kind == "pad",
                "hole" => f.kind == "hole" || f.kind == "counterbore",
                "counterbore" => f.kind == "counterbore",
                _ => false,
            });
            kind && (named.is_empty() || named.contains(&f.name))
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
    let mut calls = form_context::reads(request, false, catalog)?;
    if !calls.is_empty() {
        return Ok(calls);
    }
    let data = form_context::payloads(request);
    if let Some(doc) = form_context::document(&data) {
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
    let parameters: std::collections::BTreeSet<_> = fs
        .iter()
        .flat_map(|f| {
            [
                "depth",
                "height",
                "width",
                "length",
                "diameter",
                "x",
                "y",
                "z",
                "start_offset",
            ]
            .into_iter()
            .filter(move |p| {
                let available = if matches!(*p, "x" | "y" | "z") {
                    f.definition["position"].is_array() || f.definition["base"].is_array()
                } else {
                    field(f, p).is_some()
                };
                available && witness(f, p).is_some()
            })
        })
        .collect();
    s["properties"]["parameter"]["enum"] = json!(parameters);
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
            .filter_map(|p| field(f, p).map(|(_, n)| (p, json!(n))))
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
    let parameter = if has("offset") {
        "start_offset"
    } else if has("diameter") {
        "diameter"
    } else if has("widen") || has("wider") {
        if matching(request)
            .iter()
            .all(|f| field(f, "width").is_some())
        {
            "width"
        } else {
            "diameter"
        }
    } else if has("width") {
        "width"
    } else if has("length") {
        "length"
    } else if has("shift") || has("move") {
        if has("x") {
            "x"
        } else if has("y") {
            "y"
        } else if has("z") {
            "z"
        } else {
            return None;
        }
    } else if has("offset") {
        "start_offset"
    } else if has("depth")
        || has("deep")
        || has("deepen")
        || w.iter().any(|v| {
            matches!(
                v.as_str(),
                "pocket" | "recess" | "slot" | "hole" | "counterbore"
            )
        })
    {
        "depth"
    } else {
        "height"
    };
    let relative = has("by")
        || (!has("to")
            && [
                "wider",
                "deeper",
                "taller",
                "shorter",
                "shallower",
                "higher",
            ]
            .iter()
            .any(|s| has(s)));
    if !relative && !has("to") && !has("make") && !has("resize") {
        return None;
    }
    let value =
        if relative && (has("lower") || has("shorter") || has("shallower") || has("negative")) {
            -value
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
    args["feature"] = json!(f.name);
    // Unsupported geometry stays on the form path; it never guesses a field.
    Ok(build(&args, request, catalog).ok())
}
fn field(f: &Feature, p: &str) -> Option<(String, f64)> {
    let d = &f.definition;
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
    } else if f.kind == "hole" || f.kind == "counterbore" {
        if matches!(p, "x" | "y" | "z") {
            let face = f.faces.iter().find(|v| v["cylinder"].is_object())?;
            return Some((
                json!({"face":face["name"],"kind":"origin","axis":a}),
                number(&face["cylinder"]["origin"][a])?,
                1.,
            ));
        }
        f.faces.iter().find(|v| {
            v["name"].as_str().is_some_and(|n| {
                n.ends_with(if f.kind == "counterbore" {
                    ":cbore:base"
                } else {
                    ":bore:base"
                })
            })
        })?
    } else {
        f.faces
            .iter()
            .find(|v| point(&v["plane_normal"]).and_then(axis) == Some((a, 1.)))?
    };
    Some((
        json!({"face":face["name"],"kind":"plane","axis":a}),
        number(&face["centroid"][a])?,
        sign,
    ))
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
        before + delta
    } else {
        value
    };
    if !after.is_finite()
        || after.abs() > MAX_MM
        || (!matches!(p, "x" | "y" | "z" | "start_offset") && after <= 0.)
    {
        return Err("args.value produces an invalid feature size; size must be positive and at most 1000000 mm".into());
    }
    let mut set = json!({});
    if let Some(a) = coordinate {
        let mut pos = f.definition[&path].clone();
        pos[a] = json!(after);
        set[&path] = pos;
    } else {
        set[&path] = json!(if path == "radius" { after / 2. } else { after });
    }
    let expected = measured + derivative * (after - before);
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
        ],
        facts: None,
        slots: json!({"feature":f.name,"body":f.body,"id":f.id,"parameter":p,"field":path,"before":before,"after":after,"witness":witness,"measured_before":measured,"measured_after":expected}),
        expected: None,
        continuation: None,
    })
}
fn measured_after(request: &Value, plan: &Plan) -> Option<f64> {
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
    if measured_after(request, plan)
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
        plan.slots["before"],
        plan.slots["after"],
        number(&plan.slots["measured_before"])
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null),
        measured_after(request, plan)
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null)
    )
}
