//! Patterns use a single associative feature, never an expansion into copies.
use super::*;

pub fn schema(id: &str) -> Result<Value, String> {
    let mut defs = crate::recipe_library::definitions();
    defs["Count"] = json!({"type":"integer","minimum":2,"maximum":500});
    defs["Axis"] = json!({"type":"string","enum":["X","Y","Z"],"description":"World axis"});
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
            props["direction"] = json!({"type":"string","enum":["X","Y","Z","-X","-Y","-Z"]});
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
    let mut schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":props,"required":required,"additionalProperties":false,"$defs":defs,"x-pilot-expander":id});
    crate::recipe_library::forms::describe(&mut schema);
    Ok(schema)
}

pub(crate) fn is_pattern(id: &str) -> bool {
    matches!(id, "linear_pattern" | "circular_pattern" | "grid_pattern")
}

/// Normalize explicit form values only; every substitution is returned for auditing.
pub(crate) fn coerce(args: &mut Value) -> Result<Vec<Value>, String> {
    crate::repair::check_value(args)?;
    let before = args.clone();
    let obj = args
        .as_object_mut()
        .ok_or("pattern args requires one object")?;
    for key in ["axis", "direction"] {
        let Some(value) = obj.get_mut(key) else {
            continue;
        };
        let numeric = axis_vector(value);
        let parsed = if let Some(v) = numeric {
            (0..3)
                .find(|i| v[*i].abs() == 1. && (0..3).all(|j| j == *i || v[j] == 0.))
                .map(|i| (i, v[i] < 0.))
        } else if let Some(text) = value.as_str() {
            let lower = text.trim().to_ascii_lowercase();
            let stripped = lower.strip_prefix("along ").unwrap_or(&lower);
            let stripped = stripped.strip_prefix("positive ").unwrap_or(stripped);
            let stripped = stripped.strip_suffix(" axis").unwrap_or(stripped).trim();
            let negative = stripped.starts_with('-') || stripped.starts_with("negative ");
            let letter = stripped
                .trim_start_matches(['+', '-'])
                .strip_prefix("negative ")
                .unwrap_or(stripped.trim_start_matches(['+', '-']));
            match letter {
                "x" => Some((0, negative)),
                "y" => Some((1, negative)),
                "z" => Some((2, negative)),
                _ => None,
            }
        } else {
            None
        };
        let nearest = numeric.map_or(0, |v| {
            (0..3)
                .max_by(|a, b| v[*a].abs().total_cmp(&v[*b].abs()))
                .unwrap_or(0)
        });
        let letter = ["X", "Y", "Z"][nearest];
        let example: [u8; 3] = std::array::from_fn(|k| u8::from(k == nearest));
        let (i, negative) = parsed.ok_or_else(|| {
            format!("args.{key} {value} requires a unit axis; use {letter} or {example:?}")
        })?;
        if negative && key != "direction" {
            return Err(format!("args.{key} {value} requires a positive world axis in this form; use {} (or an inspected edge for a signed linear direction)", ["X", "Y", "Z"][i]));
        }
        *value = json!(if negative {
            ["-X", "-Y", "-Z"][i]
        } else {
            ["X", "Y", "Z"][i]
        });
    }
    Ok(changes(&before, args))
}

fn axis_vector(value: &Value) -> Option<[f64; 3]> {
    if let Some(text) = value.as_str().filter(|s| s.len() <= 128) {
        let text = text.trim();
        let text = text
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .or_else(|| text.strip_prefix('(').and_then(|s| s.strip_suffix(')')))
            .unwrap_or(text);
        let mut numbers = text
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty());
        let v = [
            numbers.next()?.parse::<f64>().ok()?,
            numbers.next()?.parse::<f64>().ok()?,
            numbers.next()?.parse::<f64>().ok()?,
        ];
        return (numbers.next().is_none() && v.iter().all(|n| n.is_finite() && n.abs() <= MAX_MM))
            .then_some(v);
    }
    vector(value).or_else(|| {
        let o = value.as_object()?;
        if o.is_empty() || o.keys().any(|k| !matches!(k.as_str(), "x" | "y" | "z")) {
            return None;
        }
        let v = [
            o.get("x").map_or(Some(0.), Value::as_f64)?,
            o.get("y").map_or(Some(0.), Value::as_f64)?,
            o.get("z").map_or(Some(0.), Value::as_f64)?,
        ];
        v.iter()
            .all(|n| n.is_finite() && n.abs() <= MAX_MM)
            .then_some(v)
    })
}

fn changes(before: &Value, after: &Value) -> Vec<Value> {
    before
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
        .chain(after.as_object().into_iter().flatten().map(|(k, _)| k))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|k| before[*k] != after[*k])
        .map(|k| json!({"field":k,"before":before[k],"after":after[k]}))
        .collect()
}

fn owner_names(def: &Value) -> Option<&Vec<Value>> {
    ["participants", "targets", "body_names"]
        .into_iter()
        .filter_map(|key| def[key].as_array())
        .find(|names| !names.is_empty())
}

pub(crate) fn resolve(args: &mut Value, request: &Value) -> Result<Vec<Value>, String> {
    let before = args.clone();
    crate::repair::check_value(args)?;
    // Models sometimes supply the step of a full circle as the total arc.
    // Correct only that exact equivalence, for an unqualified around-axis request.
    let text = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .filter(|s| s.len() <= MAX_TEXT)
        .unwrap_or("")
        .to_lowercase();
    if text.split_whitespace().any(|w| w == "around")
        && ![
            "degree", "deg", "°", "partial", "arc", "radian", "half", "quarter", "semi", "span",
            "sweep", "angle", "sector", "between",
        ]
        .iter()
        .any(|w| text.contains(w))
        && args["count"].as_u64().is_some_and(|n| {
            (2..=500).contains(&n)
                && args["angle_deg"]
                    .as_f64()
                    .is_some_and(|a| (a.abs() * n as f64 - 360.).abs() < 1e-8)
        })
    {
        args["angle_deg"] = json!(args["angle_deg"].as_f64().unwrap_or(360.).signum() * 360.);
    }
    let original_target = args["target"]
        .as_str()
        .map(|s| s.trim().to_ascii_lowercase());
    let obj = args
        .as_object_mut()
        .ok_or("pattern args requires one object")?;
    if obj.get("body") == Some(&json!(true)) {
        obj.remove("body");
        if obj.get("target").is_some_and(|v| v != "body") {
            return Err("args.body true conflicts with target; use target: body".into());
        }
        obj.insert("target".into(), json!("body"));
    }
    if let Some(value) = obj.remove("last_feature") {
        if value != true || obj.get("target").is_some_and(|t| t != "last_feature") {
            return Err(
                "args.last_feature requires true and target last_feature; use target: last_feature"
                    .into(),
            );
        }
        obj.insert("target".into(), json!("last_feature"));
    }
    if let Some(target) = obj.get_mut("target") {
        if let Some(text) = target.as_str() {
            let canonical = match text.trim().to_ascii_lowercase().as_str() {
                "feature" | "hole" | "the hole" | "boss" | "post" | "last_feature" => {
                    Some("last_feature")
                }
                "body" | "part" | "copy the part" => Some("body"),
                _ => None,
            };
            if let Some(canonical) = canonical {
                *target = json!(canonical);
            }
        }
    }

    let Some(data) = snapshot(request) else {
        return Ok(changes(&before, args));
    };
    if let Some(target @ ("hole" | "the hole" | "boss" | "post")) = original_target.as_deref() {
        let kind = data["timeline"]
            .as_array()
            .and_then(|t| t.last())
            .and_then(|f| f["type"].as_str());
        let matches = if matches!(target, "hole" | "the hole") {
            kind == Some("HoleFeature")
        } else {
            matches!(
                kind,
                Some("CylinderFeature" | "BoxFeature" | "ExtrudeFeature")
            )
        };
        if !matches {
            return Err(format!("args.target {target:?} does not match the inspected last feature {kind:?}; use body or inspect the requested feature"));
        }
    }
    let bodies = data["bodies"]
        .as_array()
        .ok_or("pattern needs inspected bodies")?;
    if bodies.is_empty() {
        return Err(
            "args.body requires an existing solid body; create the part before patterning it"
                .into(),
        );
    }
    let mut chosen = args["body"].as_str().map(str::to_owned);
    if args["target"] == "last_feature" {
        let last = data["timeline"].as_array().and_then(|t| t.last());
        if let Some(detail) = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(4096)
            .filter(|m| m["role"] == "tool")
            .filter_map(tool_payload)
            .take_while(|v| !v["timeline"].is_array())
            .find(|v| {
                last.is_some_and(|last| v["id"] == last["id"] && v["name"] == last["name"])
                    && v["definition"].is_object()
            })
        {
            if let Some(names) = owner_names(&detail["definition"]).filter(|n| n.len() == 1) {
                chosen = names[0].as_str().map(str::to_owned);
            }
        }
    }
    let normalize = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let names: Vec<_> = bodies.iter().filter_map(|b| b["name"].as_str()).collect();
    if names.is_empty() {
        return Err("args.body requires inspected body names; inspect the document again".into());
    }
    let exact: Vec<_> = names
        .iter()
        .copied()
        .filter(|name| chosen.as_deref() == Some(*name))
        .collect();
    let matches = if !exact.is_empty() {
        exact
    } else if names.len() == 1 {
        names.clone()
    } else {
        let query = normalize(chosen.as_deref().unwrap_or(""));
        let near: Vec<_> = names
            .iter()
            .copied()
            .filter(|name| !query.is_empty() && normalize(name) == query)
            .collect();
        if !near.is_empty() {
            near
        } else {
            names
                .iter()
                .copied()
                .filter(|name| {
                    query.is_empty()
                        || normalize(name).contains(&query)
                        || query.contains(&normalize(name))
                })
                .collect()
        }
    };
    if matches.len() == 1 {
        args["body"] = json!(matches[0]);
    } else if args["target"] != "last_feature" || chosen.is_some() {
        return Err(format!(
            "args.body {} is ambiguous or unmatched; choose one exact body name from {names:?}",
            args["body"]
        ));
    }
    Ok(changes(&before, args))
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
    if super::honeycomb::candidate(text) {
        return Some("honeycomb_pattern");
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
        "-X" => [-1., 0., 0.],
        "-Y" => [0., -1., 0.],
        "-Z" => [0., 0., -1.],
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
        .ok_or_else(|| format!("target last_feature {key} {} must be above 0 and at most {MAX_MM} mm; inspect a feature with resolved numeric dimensions", def[key]))
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
        return Err("join true with target body allows 0 overlapping joins; use target last_feature for an explicitly joined feature".into());
    }
    let (n, m) = if id == "grid_pattern" {
        (count(args, "columns")?, count(args, "rows")?)
    } else {
        (count(args, "count")?, 1)
    };
    if n * m > 500 {
        return Err(format!(
            "rows * columns {m} * {n} = {} exceeds 500 instances; reduce rows or columns",
            n * m
        ));
    }
    let mut dir = axis(args["direction"].as_str().unwrap_or("X"));
    let mut dir2 = [0., 1., 0.];
    let mut spacing = 0.;
    let mut spacing2 = 0.;
    if id == "linear_pattern" {
        if args.get("direction").is_some() == args.get("edge").is_some() {
            return Err(format!("direction {} and edge {} require exactly 1 selection; supply direction X/Y/Z or one inspected edge, omitting the other", args["direction"], args["edge"]));
        }
        spacing = match (args.get("spacing"), args.get("total_length")) {
            (Some(_), None) => number(args, "spacing")?,
            (None, Some(_)) => number(args, "total_length")? / (n - 1) as f64,
            _ => {
                return Err(
                    format!("spacing {} and total_length {} require exactly 1 length; supply spacing or first-to-last total_length, omitting the other", args["spacing"], args["total_length"]),
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
        return Err("angle_deg 0 must have absolute value above 0 and at most 360; use 360 for a full circle".into());
    }
    if spacing * (n - 1) as f64 > MAX_MM || spacing2 * (m - 1) as f64 > MAX_MM {
        return Err(format!(
            "spacing extent [{},{}] must be at most 1000000 mm; reduce spacing or instance counts",
            spacing * (n - 1) as f64,
            spacing2 * (m - 1) as f64
        ));
    }
    // Structural validation runs without I/O; an absent snapshot produces inspection only.
    let Some(data) = snapshot(request) else {
        return inspection(catalog, "document.inspect", json!({"measure":true}), args);
    };
    let bodies = data["bodies"].as_array().ok_or_else(|| {
        format!(
            "body {} requires 1 complete inspected bodies array; inspect the document",
            args["body"]
        )
    })?;
    if bodies.is_empty() {
        return Err(
            "args.body requires an existing solid body; create the part before patterning it"
                .into(),
        );
    }
    let feature_mode = args["target"] == "last_feature";
    let mut chosen = args.get("body").cloned();
    if feature_mode && chosen.is_none() && bodies.len() > 1 {
        let last = data["timeline"]
            .as_array()
            .and_then(|t| t.last())
            .ok_or_else(|| format!("target {} requires exactly 1 active last feature; inspect or select target body", args["target"]))?;
        let name = last["name"].as_str().ok_or_else(|| {
            format!(
                "target last_feature has name {} requiring 1 string; inspect a named feature",
                last["name"]
            )
        })?;
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
        let names = owner_names(&detail["definition"])
            .filter(|a| a.len() == 1)
            .ok_or_else(|| format!("target last_feature has definition {} requiring exactly 1 target body; select target body or inspect a feature belonging to one body", detail["definition"]))?;
        chosen = names.first().cloned();
    }
    let selected: Vec<_> = bodies
        .iter()
        .filter(|b| chosen.as_ref().is_none_or(|name| b["name"] == *name))
        .collect();
    if selected.len() != 1 {
        return Err(format!("body {} matched {} bodies but requires exactly 1; inspect and supply one exact body name", args["body"], selected.len()));
    }
    let seed = selected[0];
    let body_name = seed["name"].as_str().ok_or_else(|| {
        format!(
            "body name {} requires 1 string; inspect and select a named body",
            seed["name"]
        )
    })?;
    let baseline = volume(&data).ok_or_else(|| {
        format!(
            "body {} requires 1 measured document volume; inspect with measure true",
            args["body"]
        )
    })?;
    let seed_volume = seed["volume_mm3"]
        .as_f64()
        .filter(|v| *v > 0.)
        .ok_or_else(|| {
            format!(
                "body {body_name:?} volume {} must be above 0 mm3; inspect and select a solid body",
                seed["volume_mm3"]
            )
        })?;
    let mut min = vector(&seed["bbox"]["min"]).ok_or_else(|| format!("body {body_name:?} bbox.min {} requires 3 numeric coordinates; inspect with measure true", seed["bbox"]["min"]))?;
    let mut max = vector(&seed["bbox"]["max"]).ok_or_else(|| format!("body {body_name:?} bbox.max {} requires 3 numeric coordinates; inspect with measure true", seed["bbox"]["max"]))?;
    if (0..3).any(|i| min[i] >= max[i]) {
        return Err(format!("body {body_name:?} has bounds {min:?}..{max:?} requiring 3 positive extents; inspect a valid solid body"));
    }
    let timeline = data["timeline"]
        .as_array()
        .filter(|t| t.len() <= 4096)
        .ok_or_else(|| format!("target {} timeline requires 1 array of at most 4096 features; inspect a smaller valid timeline", args["target"]))?;
    if timeline
        .iter()
        .any(|f| f["rolled_back"] == true || f["error"] != Value::Null)
    {
        return Err(format!("target {} has an invalid timeline requiring 0 errors, rollbacks or suppressed last features; resolve timeline errors or rollback before patterning", args["target"]));
    }
    let mut target = json!([body_name]);
    let mut new_bodies = !feature_mode;
    let mut cut = false;
    let mut expected_volume = Some(baseline + seed_volume * (n * m - 1) as f64);
    let mut overlap_skipped = false;
    if feature_mode {
        let last = timeline
            .last()
            .filter(|f| f["suppressed"] != true)
            .ok_or_else(|| format!("target {} requires exactly 1 active last feature; inspect or select target body", args["target"]))?;
        let name = last["name"].as_str().ok_or_else(|| {
            format!(
                "target last_feature has name {} requiring 1 string; inspect a named feature",
                last["name"]
            )
        })?;
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
            return Err(format!("target {} with feature {name:?} requires 1 current successful feature inspection; inspect the active last feature again", args["target"]));
        }
        target = json!([name]);
        // Dimensions are expressions in the public definition (diameter-created
        // cylinders, for example, expose radius as "(10 mm) / 2"). Read their
        // evaluated inputs rather than falling back to the joined body's bounds.
        let mut definition = detail["definition"].clone();
        let dimensions: &[&str] = match detail["type"].as_str() {
            Some("CylinderFeature") => &["radius", "height"],
            Some("BoxFeature") => &["length", "width", "height"],
            Some("HoleFeature") => &["diameter"],
            _ => &[],
        };
        if dimensions
            .iter()
            .any(|key| expression(&definition, key).is_err())
        {
            let parameters = request["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .rev()
                .take(4096)
                .filter(|m| m["role"] == "tool")
                .filter_map(tool_payload)
                .take_while(|v| !v["timeline"].is_array())
                .find(|v| v["parameters"].is_array());
            let Some(parameters) = parameters else {
                return inspection(
                    catalog,
                    "parameters.list",
                    json!({"filter":"feature"}),
                    args,
                );
            };
            let parameters = parameters["parameters"]
                .as_array()
                .filter(|p| p.len() <= 4096)
                .ok_or("pattern feature dimensions require at most 4096 evaluated parameters")?;
            for key in dimensions {
                if expression(&definition, key).is_ok() {
                    continue;
                }
                let input = detail["inputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(128)
                    .find(|input| {
                        input["label"]
                            .as_str()
                            .is_some_and(|label| label.eq_ignore_ascii_case(key))
                    });
                let values: Vec<_> = parameters
                    .iter()
                    .filter(|p| {
                        p["feature"] == name
                            && input.is_some_and(|input| p["name"] == input["param"])
                            && p["expression"] == definition[*key]
                            && p["error"].is_null()
                            && p["unit"] == "mm"
                    })
                    .collect();
                if values.len() != 1 || values[0]["value"].as_f64().is_none() {
                    return Err(format!("target last_feature {key} {} needs exactly 1 current evaluated input in mm; inspect parameters for {name:?}", definition[*key]));
                }
                definition[*key] = values[0]["value"].clone();
                expression(&definition, key)?;
            }
        }
        let def = &definition;
        new_bodies = def["operation"] == "new_body";
        cut = detail["type"] == "HoleFeature" || def["operation"] == "cut";
        if detail["type"] == "HoleFeature" {
            let d = expression(def, "diameter")?;
            let p = vector(&def["position"]).ok_or_else(|| format!("target last_feature position {} requires 3 numeric coordinates; inspect the hole definition", def["position"]))?;
            let normal = vector(&def["direction"]).ok_or_else(|| format!("target last_feature direction {} requires 3 numeric coordinates; inspect the hole definition", def["direction"]))?;
            if owner_names(def).map_or(bodies.len() != 1, |names| {
                names.as_slice() != [json!(body_name)]
            }) {
                return Err(format!("target last_feature with hole targets {} must match exactly 1 body {body_name:?}; select the hole's body", def["targets"]));
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
                format!("target last_feature has type {} but supports only 4 types HoleFeature, BoxFeature, CylinderFeature, ExtrudeFeature; select target body", detail["type"]),
            );
        } else {
            if owner_names(def).map_or(bodies.len() != 1, |names| {
                names.as_slice() != [json!(body_name)]
            }) {
                return Err(format!("target last_feature with bodies {} and participants {} must belong to exactly 1 body {body_name:?}; select the feature's body", def["body_names"], def["participants"]));
            }
            if !new_bodies {
                expected_volume = None;
            }
            if detail["type"] == "BoxFeature" {
                min = vector(&def["corner"]).ok_or_else(|| format!("target last_feature corner {} requires 3 numeric coordinates; inspect the box definition", def["corner"]))?;
                let size = [
                    expression(def, "length")?,
                    expression(def, "width")?,
                    expression(def, "height")?,
                ];
                max = std::array::from_fn(|k| min[k] + size[k]);
            } else if detail["type"] == "CylinderFeature" {
                let base = vector(&def["base"]).ok_or_else(|| format!("target last_feature base {} requires 3 numeric coordinates; inspect the cylinder definition", def["base"]))?;
                let normal = vector(&def["axis"]).ok_or_else(|| format!("target last_feature axis {} requires 3 numeric coordinates; inspect the cylinder definition", def["axis"]))?;
                let r = expression(def, "radius")?;
                let h = expression(def, "height")?;
                for k in 0..3 {
                    let radial = r * (1. - normal[k] * normal[k]).max(0.).sqrt();
                    min[k] = base[k].min(base[k] + h * normal[k]) - radial;
                    max[k] = base[k].max(base[k] + h * normal[k]) + radial;
                }
            } else if !new_bodies {
                overlap_skipped = true;
            }
        }
        if args["join"] == true && def["operation"] != "join" {
            return Err(format!("join true with operation {} requires 1 joined last feature; omit join or select an explicitly joined feature", def["operation"]));
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
            return Err(format!("edge {edge:?} matched {} edges but requires exactly 1 straight edge; inspect and use one straight edge name", found.len()));
        }
        let a = vector(&found[0]["start"]).ok_or_else(|| {
            format!(
                "edge {edge:?} start {} requires 3 numeric coordinates; inspect a straight edge",
                found[0]["start"]
            )
        })?;
        let b = vector(&found[0]["end"]).ok_or_else(|| {
            format!(
                "edge {edge:?} end {} requires 3 numeric coordinates; inspect a straight edge",
                found[0]["end"]
            )
        })?;
        let norm = (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt();
        if norm < 1e-9
            || found[0]["length"]
                .as_f64()
                .is_none_or(|length| (length - norm).abs() > 1e-6_f64.max(norm * 1e-6))
        {
            return Err(format!("edge {edge:?} has chord length {norm} and measured length {} requiring a chord above 0.000000001 and length agreement within 0.000001 relative; select a nonzero straight edge", found[0]["length"]));
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
                    [0.; 3], // World axes pass through the origin, as in the emitted call.
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
                return Err(format!("count {n} rows {m} produce bounds {lo:?}..{hi:?} outside -1000000..1000000 mm; reduce count or spacing"));
            }
            if !overlap_skipped
                && args["join"] != true
                && boxes.iter().any(|(a, b): &([f64; 3], [f64; 3])| {
                    (0..3).all(|k| lo[k] < b[k] - 1e-8 && hi[k] > a[k] + 1e-8)
                })
            {
                return Err(if id == "circular_pattern" {
                    format!("circular_pattern axis {} centre [0,0,0] angle_deg {} produces overlapping rotated instance bounds {lo:?}..{hi:?} with limit 0 overlaps; increase the arc angle or explicitly request joined last features", args["axis"], args["angle_deg"])
                } else {
                    format!("spacing [{spacing},{spacing2}] produce overlapping instance bounds {lo:?}..{hi:?} with limit 0 overlaps; increase spacing or explicitly request joined last features")
                });
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
        return Err(format!("count {n} rows {m} produce {total_bodies} bodies, exceeding {MAX_BODIES}; reduce counts or remove bodies"));
    }
    let mut slots = args.clone();
    if overlap_skipped {
        slots["overlap_precheck"] = json!({"status":"skipped","reason":"last feature extent is unmeasurable; SolidCraft validates the pattern"});
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
        slots,
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
fn rotated(
    min: [f64; 3],
    max: [f64; 3],
    axis: &str,
    centre: [f64; 3],
    angle: f64,
) -> ([f64; 3], [f64; 3]) {
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
        let x = p[a] - centre[a];
        let y = p[b] - centre[b];
        p[a] = centre[a] + x * angle.cos() - y * angle.sin();
        p[b] = centre[b] + x * angle.sin() + y * angle.cos();
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod form_tests {
    use super::*;
    #[test]
    fn model_axes_are_canonical_and_signed_directions_survive() {
        for input in [
            json!([1, 0, 0]),
            json!({"x":1}),
            json!("positive X"),
            json!("X axis"),
            json!("+x"),
            json!("along X"),
            json!("x"),
        ] {
            let mut args = json!({"direction":input});
            assert!(!coerce(&mut args).unwrap().is_empty());
            assert_eq!(args["direction"], "X");
        }
        for input in [
            json!([-1, 0, 0]),
            json!({"x":-1}),
            json!("negative X"),
            json!("-x"),
        ] {
            let mut args = json!({"direction":input});
            coerce(&mut args).unwrap();
            assert_eq!(args["direction"], "-X");
        }
        for input in [
            json!("z"),
            json!("Z Axis"),
            json!([0, 0, 1]),
            json!("0, 0, 1"),
            json!("[0, 0, 1]"),
            json!("(0 0 1)"),
        ] {
            let mut args = json!({"axis":input});
            coerce(&mut args).unwrap();
            assert_eq!(args["axis"], "Z");
        }
        let error = coerce(&mut json!({"axis":"0, 0.1, 2"})).unwrap_err();
        assert!(error.contains("use Z or [0, 0, 1]"), "{error}");
        for bad in ["(0 0 1", "[0,0,1,2]", "0,NaN,1", "0,0,inf"] {
            assert!(coerce(&mut json!({"axis":bad})).is_err(), "{bad}");
        }
        for input in [json!([1, 1, 0]), json!({"x":1,"typo":2}), json!("-z")] {
            let error = coerce(&mut json!({"axis":input})).unwrap_err();
            assert!(error.contains("args.axis") && error.contains("use"));
        }
    }
    #[test]
    fn inspected_selections_replace_inventions_and_preserve_ambiguity() {
        let mut request = json!({"messages":[{"role":"user","content":"copy the part"},{"role":"tool","content":json!({"bodies":[{"name":"Body1"}],"timeline":[]}).to_string()}]});
        for body in [Value::Null, json!("10x10x10 Cube")] {
            let mut args = json!({"target":"body","body":body});
            let audit = resolve(&mut args, &request).unwrap();
            assert_eq!(args["body"], "Body1");
            assert_eq!(audit.len(), 1);
        }
        request["messages"][1]["content"] = json!(json!({"bodies":[{"name":"Base Plate"},{"name":"Post"},{"name":"Post 2"}],"timeline":[{"id":1,"name":"Boss"}]}).to_string());
        for name in ["base-plate", "Plate"] {
            let mut args = json!({"target":"body","body":name});
            resolve(&mut args, &request).unwrap();
            assert_eq!(args["body"], "Base Plate");
        }
        let error = resolve(&mut json!({"target":"body","body":"pos"}), &request).unwrap_err();
        assert!(error.contains("Post") && error.contains("Post 2"));
        request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"id":1,"name":"Boss","definition":{"targets":["Post"]}}).to_string()}));
        let mut args = json!({"target":"last_feature"});
        resolve(&mut args, &request).unwrap();
        assert_eq!(args["body"], "Post");
    }
    #[test]
    fn empty_design_error_never_offers_empty_choices() {
        let request = json!({"messages":[{"role":"user","content":"a post at X=30, 6 copies around Z"},{"role":"tool","content":json!({"bodies":[],"timeline":[]}).to_string()}]});
        for body in [Value::Null, json!("post")] {
            let error = resolve(&mut json!({"target":"body","body":body}), &request).unwrap_err();
            assert!(error.contains("create the part"), "{error}");
            assert!(!error.contains("[]"), "{error}");
        }
        let unnamed = json!({"messages":[{"role":"user","content":"copy the part"},{"role":"tool","content":json!({"bodies":[{}],"timeline":[]}).to_string()}]});
        let error = resolve(&mut json!({"target":"body"}), &unnamed).unwrap_err();
        assert!(error.contains("inspect the document again"), "{error}");
        assert!(!error.contains("[]"), "{error}");
    }
    #[test]
    fn rotated_bounds_use_the_axis_and_centre() {
        for (axis, min, max, expected_min, expected_max) in [
            (
                "Z",
                [26., -4., 0.],
                [34., 4., 8.],
                [-4., 26., 0.],
                [4., 34., 8.],
            ),
            (
                "X",
                [0., 26., -4.],
                [8., 34., 4.],
                [0., -4., 26.],
                [8., 4., 34.],
            ),
            (
                "Y",
                [-4., 0., 26.],
                [4., 8., 34.],
                [26., 0., -4.],
                [34., 8., 4.],
            ),
        ] {
            let centre = [7., 9., 11.];
            let shift = |p: [f64; 3]| std::array::from_fn(|k| p[k] + centre[k]);
            let (lo, hi) = rotated(
                shift(min),
                shift(max),
                axis,
                centre,
                std::f64::consts::FRAC_PI_2,
            );
            for k in 0..3 {
                assert!((lo[k] - shift(expected_min)[k]).abs() < 1e-9);
                assert!((hi[k] - shift(expected_max)[k]).abs() < 1e-9);
            }
        }
    }
}
