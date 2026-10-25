//! Patterns use a single associative feature, never an expansion into copies.
use super::*;

pub fn schema(id: &str) -> Result<Value, String> {
    let mut defs = crate::recipe_library::definitions();
    defs["Count"] = json!({"type":"integer","minimum":2,"maximum":500});
    defs["Axis"] = json!({"type":"string","enum":["X","Y","Z"]});
    let mut props = json!({
        "target":{"type":"string","enum":["body","last_feature"]},
        "body":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f]+$"},
        "join":{"type":"boolean","description":"Explicit permission for overlapping joined feature instances; body patterns always create new bodies"}
    });
    let required = match id {
        "linear_pattern" => {
            props["count"] = json!({"$ref":"#/$defs/Count"});
            props["spacing"] = json!({"$ref":"#/$defs/Length"});
            props["total_length"] = json!({"$ref":"#/$defs/Length"});
            props["direction"] = json!({"$ref":"#/$defs/Axis"});
            props["edge"] = json!({"type":"string","minLength":1,"maxLength":128});
            vec!["target", "count"]
        }
        "grid_pattern" => {
            for k in ["rows", "columns"] {
                props[k] = json!({"$ref":"#/$defs/Count"});
            }
            for k in ["spacing1", "spacing2"] {
                props[k] = json!({"$ref":"#/$defs/Length"});
            }
            props["plane"] = json!({"$ref":"#/$defs/Plane"});
            vec!["target", "rows", "columns", "spacing1", "spacing2", "plane"]
        }
        "circular_pattern" => {
            props["count"] = json!({"$ref":"#/$defs/Count"});
            props["axis"] = json!({"$ref":"#/$defs/Axis"});
            props["angle_deg"] = json!({"$ref":"#/$defs/Angle"});
            vec!["target", "count", "axis", "angle_deg"]
        }
        _ => return Err("unknown pattern recipe".into()),
    };
    Ok(
        json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":props,"required":required,"additionalProperties":false,"$defs":defs,"x-pilot-expander":id}),
    )
}

/// Deliberately narrow CAD repetition cues; unrelated file/clipboard operations stay out.
pub fn intent(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    if [
        "clipboard",
        "file",
        "files",
        "text",
        "directory",
        "folder",
        "poem",
        "spreadsheet",
    ]
    .iter()
    .any(|w| has(w))
    {
        return None;
    }
    let number = words.iter().any(|w| w.parse::<usize>().is_ok());
    let repeat = [
        "pattern",
        "array",
        "repeat",
        "copies",
        "instances",
        "duplicate",
    ]
    .iter()
    .any(|w| has(w));
    let repeat =
        repeat || (number && has("copy") && (has("times") || has("apart") || has("spacing")));
    let counted_parts = words.windows(2).any(|pair| {
        pair[0].parse::<usize>().is_ok()
            && matches!(
                pair[1],
                "boxes"
                    | "cylinders"
                    | "bosses"
                    | "holes"
                    | "parts"
                    | "pieces"
                    | "discs"
                    | "disks"
                    | "bodies"
            )
    });
    if (repeat || number)
        && ["around", "circular", "radial", "circumference"]
            .iter()
            .any(|w| has(w))
    {
        return Some("circular_pattern");
    }
    if (repeat || number)
        && (has("grid") || has("rows") || has("columns") || (has("array") && has("by")))
    {
        return Some("grid_pattern");
    }
    if repeat
        || counted_parts
        || (number && (has("row") || lower.contains("of these") || lower.contains("of this")))
    {
        return Some("linear_pattern");
    }
    None
}
fn axis(s: &str) -> [f64; 3] {
    match s {
        "Y" => [0., 1., 0.],
        "Z" => [0., 0., 1.],
        _ => [1., 0., 0.],
    }
}
fn vector(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    let result = [a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?];
    result
        .iter()
        .all(|n| n.is_finite() && n.abs() <= MAX_MM)
        .then_some(result)
}
fn snapshot(request: &Value) -> Option<Value> {
    let messages = request["messages"].as_array()?;
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .map_or(0, |i| i + 1);
    for m in messages[start..]
        .iter()
        .rev()
        .take(4096)
        .filter(|m| m["role"] == "tool")
    {
        let Some(v) = tool_payload(m) else {
            continue;
        };
        if v["bodies"]
            .as_array()
            .is_some_and(|a| a.len() <= MAX_BODIES && a.iter().all(Value::is_object))
            && v["timeline"].is_array()
        {
            return Some(v);
        }
        if v.get("created_bodies").is_some()
            || v.get("changed_bodies").is_some()
            || v.get("removed_bodies").is_some()
            || v.get("feature").is_some()
        {
            return None;
        }
    }
    None
}
fn expression(def: &Value, key: &str) -> Result<f64, String> {
    def[key]
        .as_f64()
        .or_else(|| def[key].as_str().and_then(|s| s.parse::<f64>().ok()))
        .filter(|v| v.is_finite() && *v > 0. && *v <= MAX_MM)
        .ok_or_else(|| format!("feature {key} needs a resolved positive numeric length"))
}

fn inspection(
    catalog: &Catalog,
    command: &str,
    params: Value,
    args: &Value,
) -> Result<Plan, String> {
    Ok(Plan {
        rule: "resolve_pattern",
        calls: vec![call(catalog, command, params)?],
        facts: None,
        slots: args.clone(),
        expected: None,
        continuation: None,
    })
}
fn number(args: &Value, k: &str) -> Result<f64, String> {
    args[k].as_f64().ok_or_else(|| format!("pattern needs {k}"))
}
fn count(args: &Value, k: &str) -> Result<usize, String> {
    args[k]
        .as_u64()
        .filter(|n| (2..=500).contains(n))
        .map(|n| n as usize)
        .ok_or_else(|| format!("{k}: pattern count must be 2..500 including the original"))
}

pub(super) fn build(
    id: &str,
    args: &Value,
    request: &Value,
    catalog: &Catalog,
) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(args, &schema(id)?)?;
    if args["target"] == "body" && args["join"] == true {
        return Err("body patterns create new bodies; select a joined last feature to inherit join behavior".into());
    }
    let (n, m) = if id == "grid_pattern" {
        (count(args, "columns")?, count(args, "rows")?)
    } else {
        (count(args, "count")?, 1)
    };
    if n * m > 500 {
        return Err("pattern total count exceeds 500; reduce rows or columns".into());
    }
    let mut dir = axis(args["direction"].as_str().unwrap_or("X"));
    let mut dir2 = [0., 1., 0.];
    let mut spacing = 0.;
    let mut spacing2 = 0.;
    if id == "linear_pattern" {
        if args.get("direction").is_some() == args.get("edge").is_some() {
            return Err("linear pattern needs exactly one direction X/Y/Z or edge name".into());
        }
        spacing = match (args.get("spacing"), args.get("total_length")) {
            (Some(_), None) => number(args, "spacing")?,
            (None, Some(_)) => number(args, "total_length")? / (n - 1) as f64,
            _ => {
                return Err(
                    "linear pattern needs spacing or total_length (first to last), not both".into(),
                )
            }
        };
    } else if id == "grid_pattern" {
        (dir, dir2) = match args["plane"].as_str() {
            Some("XZ") => (axis("X"), axis("Z")),
            Some("YZ") => (axis("Y"), axis("Z")),
            _ => (axis("X"), axis("Y")),
        };
        spacing = number(args, "spacing1")?;
        spacing2 = number(args, "spacing2")?;
    } else if number(args, "angle_deg")?.abs() < 1e-9 {
        return Err("circular pattern angle must be nonzero".into());
    }
    if spacing * (n - 1) as f64 > MAX_MM || spacing2 * (m - 1) as f64 > MAX_MM {
        return Err("pattern extent exceeds 1000000 mm".into());
    }
    // Structural validation runs without I/O; an absent snapshot produces inspection only.
    let Some(data) = snapshot(request) else {
        return inspection(catalog, "document.inspect", json!({"measure":true}), args);
    };
    let bodies = data["bodies"]
        .as_array()
        .ok_or("pattern inspection needs bodies")?;
    let feature_mode = args["target"] == "last_feature";
    let mut chosen = args.get("body").cloned();
    if feature_mode && chosen.is_none() && bodies.len() > 1 {
        let last = data["timeline"]
            .as_array()
            .and_then(|t| t.last())
            .ok_or("pattern needs an active last feature")?;
        let name = last["name"].as_str().ok_or("last feature needs a name")?;
        let detail = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(4096)
            .filter(|m| m["role"] == "tool")
            .filter_map(tool_payload)
            .take_while(|v| !v["timeline"].is_array())
            .find(|v| v["name"] == name && v["definition"].is_object());
        let Some(detail) = detail else {
            return inspection(catalog, "timeline.inspect", json!({"feature":name}), args);
        };
        let names = detail["definition"]
            .get("body_names")
            .or_else(|| detail["definition"].get("participants"))
            .and_then(Value::as_array)
            .filter(|a| a.len() == 1)
            .ok_or("last feature must resolve to exactly one target body")?;
        chosen = names.first().cloned();
    }
    let selected: Vec<_> = bodies
        .iter()
        .filter(|b| chosen.as_ref().is_none_or(|name| b["name"] == *name))
        .collect();
    if selected.len() != 1 {
        return Err("pattern target is ambiguous: inspect and select exactly one body".into());
    }
    let seed = selected[0];
    let body_name = seed["name"].as_str().ok_or("pattern body has no name")?;
    let baseline = volume(&data).ok_or("pattern needs measured document.inspect volume")?;
    let seed_volume = seed["volume_mm3"]
        .as_f64()
        .filter(|v| *v > 0.)
        .ok_or("pattern needs positive target volume")?;
    let mut min = vector(&seed["bbox"]["min"]).ok_or("pattern needs measured target bounds")?;
    let mut max = vector(&seed["bbox"]["max"]).ok_or("pattern needs measured target bounds")?;
    if (0..3).any(|i| min[i] >= max[i]) {
        return Err("pattern target bounds are invalid".into());
    }
    let timeline = data["timeline"]
        .as_array()
        .filter(|t| t.len() <= 4096)
        .ok_or("pattern timeline exceeds limit or is missing")?;
    if timeline
        .iter()
        .any(|f| f["rolled_back"] == true || f["error"] != Value::Null)
    {
        return Err("resolve timeline errors or rollback before patterning".into());
    }
    let mut target = json!([body_name]);
    let mut new_bodies = !feature_mode;
    let mut cut = false;
    let mut expected_volume = Some(baseline + seed_volume * (n * m - 1) as f64);
    if feature_mode {
        let last = timeline
            .last()
            .filter(|f| f["suppressed"] != true)
            .ok_or("pattern needs an active last feature")?;
        let name = last["name"].as_str().ok_or("last feature needs a name")?;
        let detail = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(4096)
            .filter(|m| m["role"] == "tool")
            .filter_map(tool_payload)
            .take_while(|v| !v["timeline"].is_array())
            .find(|v| v["name"] == name && v["definition"].is_object());
        let Some(detail) = detail else {
            return inspection(catalog, "timeline.inspect", json!({"feature":name}), args);
        };
        if detail["id"] != last["id"] || detail["error"] != Value::Null {
            return Err("last feature inspection is stale or failed".into());
        }
        target = json!([name]);
        let def = &detail["definition"];
        new_bodies = def["operation"] == "new_body";
        cut = detail["type"] == "HoleFeature" || def["operation"] == "cut";
        if detail["type"] == "HoleFeature" {
            let d = def["diameter"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.)
                .ok_or("hole pattern needs a resolved numeric diameter")?;
            let p = vector(&def["position"]).ok_or("hole pattern needs position")?;
            let normal = vector(&def["direction"]).ok_or("hole pattern needs direction")?;
            if def["participants"] != json!([body_name]) {
                return Err("last hole must target exactly the selected body".into());
            }
            for i in 0..3 {
                if normal[i].abs() < 1e-9 {
                    min[i] = p[i] - d / 2.;
                    max[i] = p[i] + d / 2.;
                }
            }
            // Feature cuts retain the host body. Their final volume is measured;
            // arbitrary partial intersections cannot be predicted from an AABB.
            expected_volume = None;
        } else if !matches!(
            detail["type"].as_str(),
            Some("BoxFeature" | "CylinderFeature" | "ExtrudeFeature")
        ) {
            return Err(
                "last feature geometry is unsupported for safe overlap checks; select its body"
                    .into(),
            );
        } else {
            if def[if new_bodies {
                "body_names"
            } else {
                "participants"
            }] != json!([body_name])
            {
                return Err("last feature must belong to exactly the selected body".into());
            }
            if !new_bodies {
                expected_volume = None;
            }
            if detail["type"] == "BoxFeature" {
                min = vector(&def["corner"]).ok_or("box feature needs numeric corner")?;
                let size = [
                    expression(def, "length")?,
                    expression(def, "width")?,
                    expression(def, "height")?,
                ];
                max = std::array::from_fn(|k| min[k] + size[k]);
            } else if detail["type"] == "CylinderFeature" {
                let base = vector(&def["base"]).ok_or("cylinder feature needs numeric base")?;
                let normal = vector(&def["axis"]).ok_or("cylinder feature needs axis")?;
                let r = expression(def, "radius")?;
                let h = expression(def, "height")?;
                for k in 0..3 {
                    let radial = r * (1. - normal[k] * normal[k]).max(0.).sqrt();
                    min[k] = base[k].min(base[k] + h * normal[k]) - radial;
                    max[k] = base[k].max(base[k] + h * normal[k]) + radial;
                }
            } else if !new_bodies {
                return Err(
                    "joined extrude bounds are unresolved; select a primitive feature or its body"
                        .into(),
                );
            }
        }
        if args["join"] == true && def["operation"] != "join" {
            return Err("join permission requires a last feature whose operation is join".into());
        }
    }
    if let Some(edge) = args["edge"].as_str() {
        let edges = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(4096)
            .filter(|m| m["role"] == "tool")
            .filter_map(tool_payload)
            .find(|v| v["edges"].is_array() && v["body"] == body_name);
        let Some(edges) = edges else {
            return inspection(catalog, "model.edges", json!({"body":body_name}), args);
        };
        let found: Vec<_> = edges["edges"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["name"] == edge)
            .collect();
        if found.len() != 1 {
            return Err("edge direction must resolve to exactly one straight edge".into());
        }
        let a = vector(&found[0]["start"]).ok_or("edge needs straight start/end points")?;
        let b = vector(&found[0]["end"]).ok_or("edge needs straight start/end points")?;
        let norm = (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt();
        if norm < 1e-9
            || found[0]["length"]
                .as_f64()
                .is_none_or(|length| (length - norm).abs() > 1e-6_f64.max(norm * 1e-6))
        {
            return Err("edge direction must be a nonzero straight edge".into());
        }
        dir = std::array::from_fn(|i| (b[i] - a[i]) / norm);
    }
    let mut boxes = Vec::with_capacity(n * m);
    for j in 0..m {
        for i in 0..n {
            let (lo, hi) = if id == "circular_pattern" {
                let angle = number(args, "angle_deg")?;
                let denom = if (angle.abs() - 360.).abs() < 1e-9 {
                    n
                } else {
                    n - 1
                };
                rotated(
                    min,
                    max,
                    args["axis"].as_str().ok_or("circular pattern needs axis")?,
                    (angle * i as f64 / denom as f64).to_radians(),
                )
            } else {
                let delta: [f64; 3] = std::array::from_fn(|k| {
                    dir[k] * spacing * i as f64 + dir2[k] * spacing2 * j as f64
                });
                (
                    std::array::from_fn(|k| min[k] + delta[k]),
                    std::array::from_fn(|k| max[k] + delta[k]),
                )
            };
            if lo
                .iter()
                .chain(hi.iter())
                .any(|v| !v.is_finite() || v.abs() > MAX_MM)
            {
                return Err("pattern coordinates exceed 1000000 mm".into());
            }
            if args["join"] != true
                && boxes.iter().any(|(a, b): &([f64; 3], [f64; 3])| {
                    (0..3).all(|k| lo[k] < b[k] - 1e-8 && hi[k] > a[k] + 1e-8)
                })
            {
                return Err("pattern instances may overlap; increase spacing or explicitly request joined last features".into());
            }
            boxes.push((lo, hi));
        }
    }
    let (command, kind, mut params) = if id == "circular_pattern" {
        (
            "solid.pattern.circular",
            "CircularPatternFeature",
            json!({"axis":args["axis"],"count":n,"angle":args["angle_deg"]}),
        )
    } else {
        let mut p = json!({"dir1":dir,"count1":n,"spacing1":spacing});
        if m > 1 {
            p["dir2"] = json!(dir2);
            p["count2"] = json!(m);
            p["spacing2"] = json!(spacing2);
        }
        ("solid.pattern.rectangular", "RectangularPatternFeature", p)
    };
    params[if feature_mode { "features" } else { "bodies" }] = target;
    let feature_count = timeline
        .iter()
        .filter(|f| f["type"] == kind && f["suppressed"] != true)
        .count()
        + 1;
    let total_bodies = bodies.len() + if new_bodies { n * m - 1 } else { 0 };
    if total_bodies > MAX_BODIES {
        return Err("pattern result exceeds body limit".into());
    }
    Ok(Plan {
        rule: match id {
            "linear_pattern" => "linear_pattern",
            "grid_pattern" => "grid_pattern",
            _ => "circular_pattern",
        },
        calls: vec![
            call(catalog, command, params)?,
            call(catalog, "document.inspect", json!({"measure":true}))?,
        ],
        facts: None,
        slots: args.clone(),
        expected: Some(Expected {
            bodies: Some(total_bodies),
            volume_mm3: expected_volume,
            volume_range_mm3: if expected_volume.is_none() {
                let bound = (0..3).map(|k| max[k] - min[k]).product::<f64>() * (n * m - 1) as f64;
                let epsilon = (baseline.abs() * 1e-12).max(1e-9);
                Some(if cut {
                    ((baseline - bound - 0.01).max(0.), baseline - epsilon)
                } else {
                    (
                        baseline + if args["join"] == true { -0.01 } else { epsilon },
                        baseline + bound + 0.01,
                    )
                })
            } else {
                None
            },
            name: None,
            created_volume_mm3: None,
            placement: None,
            feature: Some((kind.into(), feature_count)),
            joint: None,
        }),
        continuation: None,
    })
}
fn rotated(min: [f64; 3], max: [f64; 3], axis: &str, angle: f64) -> ([f64; 3], [f64; 3]) {
    let (a, b) = match axis {
        "X" => (1, 2),
        "Y" => (2, 0),
        _ => (0, 1),
    };
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for mask in 0..8 {
        let mut p: [f64; 3] =
            std::array::from_fn(|k| if mask & (1 << k) == 0 { min[k] } else { max[k] });
        let x = p[a];
        let y = p[b];
        p[a] = x * angle.cos() - y * angle.sin();
        p[b] = x * angle.sin() + y * angle.cos();
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

/// Observe only explicit calls in the current user turn; never execute a suggestion.
pub fn repetition_hint(request: &Value) -> Option<&'static str> {
    let messages = request["messages"].as_array()?;
    let start = messages.iter().rposition(|m| m["role"] == "user")?;
    let mut creations = Vec::new();
    let mut sketch_clipboard = 0usize;
    let mut pasted_sketch = None;
    for message in messages
        .iter()
        .skip(start + 1)
        .take(4096)
        .filter(|m| m["role"] == "assistant")
    {
        for call in message["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .take(128)
        {
            let args: Value = if let Some(s) = call["function"]["arguments"].as_str() {
                serde_json::from_str(s).ok()?
            } else {
                call["function"]["arguments"].clone()
            };
            let steps: Vec<&Value> = if call["function"]["name"] == "batch" {
                args["commands"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(128)
                    .collect()
            } else {
                vec![&args]
            };
            for step in steps {
                let command = step["command"].as_str().unwrap_or("");
                if command == "sketch.copy" {
                    sketch_clipboard = sketch_clipboard.saturating_add(1);
                    pasted_sketch = None;
                    continue;
                }
                if command == "sketch.paste" && sketch_clipboard > 0 {
                    let params = &step["params"];
                    let point = params
                        .get("translate")
                        .or_else(|| params.get("at"))?
                        .as_array()
                        .filter(|a| a.len() == 2)?;
                    let position = [point[0].as_f64()?, point[1].as_f64()?, 0.];
                    if position.iter().any(|v| !v.is_finite() || v.abs() > MAX_MM) {
                        return None;
                    }
                    pasted_sketch = Some((sketch_clipboard, position));
                    continue;
                }
                if !matches!(
                    command,
                    "solid.box"
                        | "solid.cylinder"
                        | "solid.extrude"
                        | "body.copy"
                        | "solid.copy"
                        | "timeline.paste"
                        | "occurrence.copy"
                ) {
                    continue;
                }
                let mut shape = step["params"].clone();
                let keys = match command {
                    "solid.box" => ["corner", "center"],
                    "solid.cylinder" => ["base", "center"],
                    "solid.extrude" => ["from", "face"],
                    _ => ["translate", "translate"],
                };
                let key = keys.iter().find(|k| shape.get(**k).is_some()).copied();
                let mut position = key.and_then(|k| vector(&shape[k])).unwrap_or([0.; 3]);
                if command == "solid.extrude" {
                    if let Some((source, point)) = pasted_sketch.take() {
                        position = point;
                        let object = shape.as_object_mut()?;
                        object.remove("sketch");
                        object.insert("copied_sketch_source".into(), json!(source));
                    }
                }
                if let Some(o) = shape.as_object_mut() {
                    for k in keys {
                        o.remove(k);
                    }
                    o.remove("name");
                    o.remove("body_name");
                }
                if creations.len() >= 128 {
                    return None;
                }
                creations.push((command.to_string(), shape, position));
            }
        }
    }
    for triple in creations.windows(3) {
        if triple[0].0 != triple[1].0
            || triple[1].0 != triple[2].0
            || triple[0].1 != triple[1].1
            || triple[1].1 != triple[2].1
        {
            continue;
        }
        let step: [f64; 3] = std::array::from_fn(|k| triple[1].2[k] - triple[0].2[k]);
        if step.iter().any(|v| v.abs() > 1e-8)
            && (0..3).all(|k| ((triple[2].2[k] - triple[1].2[k]) - step[k]).abs() < 1e-8)
        {
            return Some("Use solid.pattern.rectangular with recipe linear_pattern (pilot_find then pilot_build) for equal instances at a constant step.");
        }
    }
    None
}
