//! Semantic repairs derived only from the public command catalogue.
use super::{check_value, Rewrite};
use crate::catalog::Catalog;
use serde_json::{json, Map, Value};

fn normalized(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn named(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.trim().is_empty() && s != "*")
}
fn number(value: &Value) -> Option<Value> {
    if value.is_number() {
        return Some(value.clone());
    }
    let s = value.as_str()?;
    if let Ok(Value::Number(n)) = serde_json::from_str::<Value>(s.trim()) {
        return Some(Value::Number(n));
    }
    let n: f64 = s.trim().parse().ok()?;
    serde_json::Number::from_f64(n).map(Value::Number)
}
fn point(value: &Value, count: usize) -> Option<Value> {
    let a = value.as_array()?;
    if a.len() != count {
        return None;
    }
    a.iter()
        .map(number)
        .collect::<Option<Vec<_>>>()
        .map(Value::Array)
}
fn error(reason: &str, id: &str, params: &Map<String, Value>) -> String {
    let mut example = params.clone();
    match id {
        "solid.combine" => {
            if let Some(tool) = example.remove("tool") {
                example.entry("tools").or_insert(json!([tool]));
            }
            if let Some(tool) = example.get("tools").filter(|v| named(v)).cloned() {
                example.insert("tools".into(), json!([tool]));
            }
            example
                .entry("target")
                .or_insert(json!("<target body name>"));
            example
                .entry("tools")
                .or_insert(json!(["<tool body name>"]));
        }
        "solid.extrude" => {
            example.insert("operation".into(), json!("<new|join|cut|intersect|auto>"));
        }
        "solid.hole" => {
            if let Some(o) = example.get("position").and_then(Value::as_object) {
                let v = json!([
                    o.get("x").cloned().unwrap_or(json!("<world x>")),
                    o.get("y").cloned().unwrap_or(json!("<world y>")),
                    o.get("z").cloned().unwrap_or(json!("<world z>"))
                ]);
                example.insert("position".into(), v);
            }
            if let Some(a) = example.get_mut("position").and_then(Value::as_array_mut) {
                if a.len() == 2 {
                    a.push(json!("<missing world coordinate on the named face>"));
                }
            }
            example.remove("face");
        }
        "solid.fillet" => {
            let body = example
                .get("body")
                .cloned()
                .unwrap_or(json!("<body name from inspect>"));
            if let Some(a) = example.get_mut("edges").and_then(Value::as_array_mut) {
                for edge in a {
                    if edge.is_u64() {
                        *edge = json!({"body":body,"index":edge});
                    } else if let Some(name) = edge.as_str() {
                        *edge =
                            json!({"body":body,"index":format!("<index of {name} from inspect>")});
                    }
                }
            }
        }
        "sketch.create" => {
            if let Some(face) = example.remove("face") {
                example.insert("plane".into(), json!({"face":face}));
            } else if example.contains_key("origin") || example.contains_key("normal") {
                let origin = example.remove("origin").unwrap_or(json!([0, 0, 0]));
                let normal = example.remove("normal");
                let y = normal.map_or(json!("<plane y direction>"), |n| {
                    json!(format!("<y direction perpendicular to normal {n}>"))
                });
                example.insert(
                    "plane".into(),
                    json!({"origin":origin,"x_dir":"<plane x direction>","y_dir":y}),
                );
            } else if let Some(s) = example.get("plane").and_then(Value::as_str) {
                if let Some((axis, value)) = s.split_once('=') {
                    let axes = match axis.trim().to_ascii_uppercase().as_str() {
                        "X" => Some("YZ"),
                        "Y" => Some("XZ"),
                        "Z" => Some("XY"),
                        _ => None,
                    };
                    if let Some(axes) = axes {
                        let value = value.trim().to_string();
                        let offset = example.get("offset").map_or_else(
                            || number(&json!(value)).unwrap_or(json!(value)),
                            |offset| json!(format!("<choose {value} or {offset}>")),
                        );
                        example.insert("plane".into(), json!(axes));
                        example.insert("offset".into(), offset);
                    }
                }
            }
        }
        _ => (),
    }
    format!("{reason}; use {}", json!({"command":id,"params":example}))
}

/// Transactional: errors leave the entire call intact; every accepted call change is audited.
/// Existing catalogue commands always win over invented aliases.
pub fn repair_command(step: &mut Value, catalog: &Catalog) -> Result<Vec<Rewrite>, String> {
    check_value(step)?;
    let before = step.clone();
    let mut fixed = before.clone();
    reshape(&mut fixed, catalog)?;
    check_value(&fixed)?;
    if fixed == before {
        return Ok(Vec::new());
    }
    *step = fixed.clone();
    Ok(vec![Rewrite {
        path: "$".into(),
        before,
        after: fixed,
    }])
}
fn reshape(step: &mut Value, catalog: &Catalog) -> Result<(), String> {
    let Some(id) = step["command"].as_str().map(str::to_owned) else {
        return Ok(());
    };
    let Some(p) = step.get_mut("params").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let boolean = match normalized(&id).as_str() {
        "solidcut"
        | "solidsubtract"
        | "solidsubtraction"
        | "soliddifference"
        | "solidbooleancut"
        | "solidbooleansubtract"
        | "solidbooleandifference" => Some("cut"),
        "solidunion" | "solidunite" | "solidbooleanunion" => Some("join"),
        _ => None,
    };
    if let Some(op) =
        boolean.filter(|_| catalog.get(&id).is_none() && catalog.get("solid.combine").is_some())
    {
        let mut example = p.clone();
        example.insert("operation".into(), json!(op));
        if let Some(tool) = example.remove("tool") {
            example.insert("tools".into(), json!([tool]));
        }
        if !named(&p.get("target").cloned().unwrap_or(Value::Null))
            || p.contains_key("tool") && p.contains_key("tools")
            || p.get("operation").is_some_and(|v| v != op)
        {
            return Err(error("solid.combine needs one explicit target and a tools array without conflicting operations", "solid.combine", &example));
        }
        if let Some(tool) = p.remove("tool") {
            p.insert("tools".into(), json!([tool]));
        }
        if let Some(tool) = p.get("tools").filter(|v| named(v)).cloned() {
            p.insert("tools".into(), json!([tool]));
        }
        let tools = p.get("tools").and_then(Value::as_array);
        if tools.is_none_or(|a| {
            a.is_empty() || a.iter().any(|v| !named(v) || Some(v) == p.get("target"))
        }) {
            return Err(error(
                "solid.combine needs distinct named target and tool bodies",
                "solid.combine",
                &example,
            ));
        }
        if p.keys().any(|k| {
            ![
                "target",
                "tools",
                "operation",
                "keep_tools",
                "new_component",
                "component_name",
            ]
            .contains(&k.as_str())
        }) {
            return Err(error(
                "solid.combine accepts target, tools and operation",
                "solid.combine",
                &example,
            ));
        }
        p.insert("operation".into(), json!(op));
        step["command"] = json!("solid.combine");
        return Ok(());
    }
    if catalog.get(&id).is_none() {
        return Ok(());
    }
    match id.as_str() {
        "solid.extrude" => {
            if let Some(op) = p.get("operation").and_then(Value::as_str) {
                let replacement = match normalized(op).as_str() {
                    "newsolid" | "newbody" => Some("new"),
                    "add" | "union" => Some("join"),
                    "subtract" | "remove" => Some("cut"),
                    _ => None,
                };
                if let Some(op) = replacement {
                    p.insert("operation".into(), json!(op));
                } else if !["new", "join", "cut", "intersect", "auto"].contains(&op) {
                    return Err(error(
                        "solid.extrude operation must be new, join, cut, intersect or auto",
                        &id,
                        p,
                    ));
                }
            }
        }
        "sketch.create" => plane(p)?,
        "solid.hole" => {
            if let Some(pos) = p.get("position").cloned() {
                if let Some(o) = pos.as_object() {
                    if o.len() == 3 && ["x", "y", "z"].iter().all(|k| o.contains_key(*k)) {
                        let v = json!([o["x"], o["y"], o["z"]]);
                        if let Some(v) = point(&v, 3) {
                            p.insert("position".into(), v);
                        } else if v
                            .as_array()
                            .is_some_and(|a| a.iter().all(|v| v.is_number() || v.is_string()))
                        {
                            p.insert("position".into(), v);
                        } else {
                            return Err(error(
                                "solid.hole position needs three world coordinates",
                                &id,
                                p,
                            ));
                        }
                    } else {
                        return Err(error(
                            "solid.hole position needs [x,y,z] in world coordinates",
                            &id,
                            p,
                        ));
                    }
                } else if let Some(v) = point(&pos, 3) {
                    p.insert("position".into(), v);
                } else if pos.as_array().is_some_and(|a| {
                    a.len() == 3 && a.iter().all(|v| v.is_number() || v.is_string())
                }) {
                    // Length expressions are valid native coordinates and must stay expressions.
                } else {
                    return Err(error("solid.hole needs position [x,y,z] in world coordinates, a face name does not supply the missing coordinate", &id,p));
                }
                if p.contains_key("face") {
                    return Err(error(
                        "solid.hole uses a world position [x,y,z] and optional targets, not face",
                        &id,
                        p,
                    ));
                }
            }
        }
        "solid.move" => {
            if let Some(body) = p.get("bodies").filter(|v| named(v)).cloned() {
                p.insert("bodies".into(), json!([body]));
            }
        }
        "solid.fillet" => {
            if let Some(edges) = p.get("edges").and_then(Value::as_array).cloned() {
                let mut repaired = Vec::new();
                for edge in edges {
                    if edge.is_u64() {
                        if let Some(body) = p.get("body").filter(|v| named(v)) {
                            repaired.push(json!({"body":body,"index":edge}));
                        } else {
                            return Err(error(
                                "solid.fillet edge indices need {body,index} with an explicit body",
                                &id,
                                p,
                            ));
                        }
                    } else if edge.is_number() {
                        return Err(error("solid.fillet needs nonnegative integer edge indices with an explicit body", &id,p));
                    } else if edge.is_string() {
                        return Err(error("solid.fillet needs edge points [x,y,z] or {body,index} from inspect, edge names are not indices", &id,p));
                    } else {
                        repaired.push(edge);
                    }
                }
                p.insert("edges".into(), json!(repaired));
            }
        }
        _ => (),
    }
    Ok(())
}
fn plane(p: &mut Map<String, Value>) -> Result<(), String> {
    let mut base = p.get("plane").cloned().unwrap_or(Value::Null);
    if let Some(s) = base.as_str() {
        if let Some((axis, offset)) = s.split_once('=') {
            let axes = match axis.trim().to_ascii_uppercase().as_str() {
                "X" => Some("YZ"),
                "Y" => Some("XZ"),
                "Z" => Some("XY"),
                _ => None,
            };
            if let Some(axes) = axes {
                let n = number(&json!(offset)).ok_or_else(|| {
                    error(
                        "sketch.create offset plane needs a numeric coordinate",
                        "sketch.create",
                        p,
                    )
                })?;
                if p.contains_key("offset")
                    || p.contains_key("origin")
                    || p.contains_key("normal")
                    || p.contains_key("face")
                {
                    return Err(error(
                        "sketch.create has competing plane definitions",
                        "sketch.create",
                        p,
                    ));
                }
                p.insert("plane".into(), json!(axes));
                p.insert("offset".into(), n);
                return Ok(());
            }
        }
    }
    let siblings = ["face", "origin", "normal", "x_dir", "y_dir"];
    if !siblings.iter().any(|k| p.contains_key(*k)) {
        return Ok(());
    }
    if p.contains_key("face") {
        if point(&p["face"], 3).is_none() {
            return Err(error(
                "sketch.create plane.face needs a world point [x,y,z], not a face label",
                "sketch.create",
                p,
            ));
        }
        if base.as_object().is_some_and(Map::is_empty) || base.is_null() || base == "face" {
            base = json!({"face":p["face"]});
            p.remove("face");
            if p.contains_key("origin")
                || p.contains_key("normal")
                || p.contains_key("x_dir")
                || p.contains_key("y_dir")
            {
                return Err(error(
                    "sketch.create face and explicit axes are competing planes",
                    "sketch.create",
                    p,
                ));
            }
        } else {
            return Err(error(
                "sketch.create face belongs in plane:{face:[x,y,z]} without another plane",
                "sketch.create",
                p,
            ));
        }
    } else {
        let axes = match base.as_str() {
            Some("XY") => Some((json!([1, 0, 0]), json!([0, 1, 0]), json!([0, 0, 1]))),
            Some("XZ") => Some((json!([1, 0, 0]), json!([0, 0, -1]), json!([0, 1, 0]))),
            Some("YZ") => Some((json!([0, 1, 0]), json!([0, 0, 1]), json!([1, 0, 0]))),
            _ => None,
        };
        if let Some((x, y, n)) = axes {
            if p.get("normal").is_some_and(|v| point(v, 3) != point(&n, 3))
                || p.contains_key("x_dir")
                || p.contains_key("y_dir")
            {
                return Err(error("sketch.create needs explicit plane:{origin,x_dir,y_dir} for a different orientation", "sketch.create",p));
            }
            base = json!({"origin":p.get("origin").cloned().unwrap_or(json!([0,0,0])),"x_dir":x,"y_dir":y});
            p.remove("origin");
            p.remove("normal");
        } else if base.is_object() || base.is_null() {
            if base.is_null() {
                base = json!({});
            }
            for k in ["origin", "x_dir", "y_dir"] {
                if p.contains_key(k) && base.get(k).is_some() {
                    return Err(error(
                        "sketch.create has competing plane fields",
                        "sketch.create",
                        p,
                    ));
                }
            }
            for k in ["origin", "x_dir", "y_dir"] {
                if let Some(v) = p.remove(k) {
                    base[k] = v;
                }
            }
            if p.contains_key("normal")
                || base.get("x_dir").is_none()
                || base.get("y_dir").is_none()
            {
                return Err(error("sketch.create needs plane:{origin,x_dir,y_dir}, normal alone leaves sketch rotation unspecified", "sketch.create",p));
            }
        } else {
            return Err(error("sketch.create needs explicit plane:{origin,x_dir,y_dir} instead of a named plane with extra axes", "sketch.create",p));
        }
    }
    p.insert("plane".into(), base);
    Ok(())
}

/// Explain dependent sketch failures using only results from this exact batch.
/// No state is carried across batches or inferred from unexecuted commands.
pub fn failed_sketch_context(args: &Value, result: &Value) -> Option<String> {
    check_value(args).ok()?;
    check_value(result).ok()?;
    let commands = args["commands"].as_array()?;
    if commands.len() > super::MAX_STEPS {
        return None;
    }
    let data = crate::guidance::payload(result);
    let results = data["results"].as_array();
    let mut failure: Option<(usize, String)> = None;
    for (i, call) in commands.iter().enumerate() {
        let entry = results.and_then(|r| r.get(i));
        let entry = entry.map(crate::guidance::payload);
        let reported = (data.pointer("/failed/index").and_then(Value::as_u64) == Some(i as u64))
            .then(|| data["failed"].clone());
        let entry = reported.or(entry);
        let reason = entry.as_ref().and_then(|e| {
            e["error"]
                .as_str()
                .or_else(|| e.pointer("/error/message").and_then(Value::as_str))
        });
        if call["command"] == "sketch.create" {
            failure = reason.map(|r| (i, r.chars().take(1024).collect()));
        } else if call["command"] == "sketch.finish" && reason.is_none() && entry.is_some() {
            failure = None;
        } else if matches!(
            call["command"].as_str(),
            Some("sketch.circle.center" | "sketch.rectangle.two_point")
        ) {
            let no_sketch = reason.is_some_and(|r| {
                let r = r.to_ascii_lowercase();
                r.contains("no sketch")
                    || r.contains("not editing a sketch")
                    || r.contains("no active sketch")
            });
            let not_run = data["not_run"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(i)));
            if no_sketch || not_run {
                if let Some((index, reason)) = &failure {
                    return Some(format!("{} cannot draw because the earlier sketch.create at batch step {index} failed: {reason}",call["command"].as_str().unwrap_or("sketch command")));
                }
            }
        }
    }
    None
}
