//! Short field guidance shared by file recipes, legacy shapes and fill forms.
use serde_json::{json, Value};

pub(crate) fn describe(s: &mut Value) {
    let mirror = s["x-pilot-expander"] == "mirror";
    let joint = s["x-pilot-expander"] == "revolute_joint"
        || s["properties"]["shape"]["const"] == "revolute_joint";
    let lip = s["properties"].get("side").is_some();
    if let Some(props) = s.get_mut("properties").and_then(Value::as_object_mut) {
        let mut optional = Vec::new();
        for (key, v) in props {
            let (description, default) = match key.as_str() {
                "plane" if mirror => ("Reflection plane", None),
                "plane" => ("Sketch plane", Some(json!("XY"))),
                "centred" => (
                    "True only for centred/centered/on the origin",
                    Some(json!(false)),
                ),
                "edges" => (
                    "vertical: four corner edges through thickness; all: every rim too",
                    Some(json!("vertical")),
                ),
                "side" => (
                    "inner: flush with inner wall face; outer: outer face",
                    Some(json!("inner")),
                ),
                "open" => ("Face to remove", Some(json!("top"))),
                "kind" if v["enum"][0] == "fillet" => ("fillet: round; chamfer: bevel", None),
                "kind" => ("Cross-section outline", None),
                "thickness" => ("Plate thickness or uniform profile legs", None),
                "web_thickness" => ("Vertical leg/stem/web; U bottom; omit with thickness", None),
                "flange_thickness" => ("Horizontal leg/flange; U sides; omit with thickness", None),
                "stem_offset" => ("T stem centre from left; omitted: centred", None),
                "depth" => ("Extrusion distance normal to plane", None),
                "floor" => ("Base thickness; omitted: wall", None),
                "lip" => ("Raised locating ring at opening", None),
                "bolt_circle" => ("Hole centres on this circle", None),
                "grid" => ("Hole centres in rows and columns", None),
                "corners" => ("Outside corner treatment", None),
                "bore_diameter" => ("Centre through bore", None),
                "hole_diameter" => ("Pattern hole diameter; requires bolt_circle or grid", None),
                "start_deg" => ("From +X toward +Y", Some(json!(0))),
                "pitch_x" | "pitch_y" => ("Hole centre spacing", None),
                "size" => (
                    "Fillet radius/chamfer distance; hex span for honeycomb",
                    None,
                ),
                "size_kind" => (
                    "across_flats: edge to edge; across_corners: vertex to vertex",
                    None,
                ),
                "orientation" => (
                    "pointy_top: vertex up; flat_top: edge up",
                    Some(json!("pointy_top")),
                ),
                "mode" => (
                    "fill: fit field; exact: require rows and columns",
                    Some(json!("fill")),
                ),
                "web" => ("Minimum material between hex holes", None),
                "border" => ("Minimum uncut margin", None),
                "rows" | "columns" => ("Instance count; honeycomb requires exact mode", None),
                "target" => ("body: whole solid; last_feature: latest feature", None),
                "body" => ("Exact inspected body name; omit only if unambiguous", None),
                "join" => (
                    "Allow overlap of joined last features only",
                    Some(json!(false)),
                ),
                "direction" | "axis" => ("World axis", None),
                "edge" => ("Inspected straight edge; use instead of direction", None),
                "spacing" | "spacing1" | "spacing2" => ("Centre spacing between instances", None),
                "total_length" => ("First to last centre; use instead of spacing", None),
                "angle_deg" if joint => ("Rotation in degrees; omitted: 0", Some(json!(0))),
                "angle_deg" => ("Rotation in degrees", None),
                "width" if lip => ("Ring width across wall; below wall thickness", None),
                "height" if lip => ("Rise above the opening", None),
                "keep_original" => ("True: retain source and add mirrored copy", None),
                "amount" => ("Fillet radius or chamfer distance", None),
                "delta" => ("Signed [x,y,z] displacement in mm", None),
                "corner" => ("Minimum [x,y,z]; omitted: [0,0,0]", None),
                "position" => ("World [x,y,z] on entry face; omitted: face centre", None),
                "holes" => ("One through hole per distinct axis; maximum 3", None),
                "pin_component" => ("Inspected pin component; omit if unique", None),
                "hole_component" => ("Inspected hole component; omit if unique", None),
                _ => ("", None),
            };
            if !description.is_empty() {
                v["description"] = json!(description);
            }
            if let Some(d) = default {
                v["default"] = d;
                optional.push(key.clone());
            }
            describe(v);
        }
        if let Some(required) = s.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|k| !optional.iter().any(|name| k == name));
        }
    }
    if let Some(items) = s.get_mut("items") {
        describe(items);
    }
}

/// The wire form omits validator-only metadata; validation always uses the full schema.
pub fn compact(s: &Value) -> Value {
    fn walk(s: &Value, root: &Value, key: &str) -> Value {
        let resolved = s["$ref"]
            .as_str()
            .and_then(|r| r.strip_prefix('#'))
            .and_then(|p| root.pointer(p))
            .unwrap_or(s);
        let mut out = json!({});
        for name in [
            "type",
            "enum",
            "const",
            "properties",
            "required",
            "items",
            "default",
        ] {
            if let Some(v) = s.get(name).or_else(|| resolved.get(name)) {
                out[name] = v.clone();
            }
        }
        let description = match key {
            "edges" => "vertical: four corners through thickness; all: rims too",
            "side" => "inner: flush with inner wall face; outer: outer face",
            "centred" => "centred/centered/on the origin",
            "kind" if out["enum"][0] == "fillet" => "round/bevel",
            "kind" => "Section outline",
            "size_kind" => "across_flats: edge to edge; across_corners: vertex to vertex",
            "orientation" => "pointy_top: vertex up; flat_top: edge up",
            "mode" => "fill: fit; exact: rows and columns",
            "web_thickness" => "Vertical leg; U bottom",
            "flange_thickness" => "Horizontal leg; U sides",
            "stem_offset" => "T centre from left; omit: centred",
            "hole_diameter" => "Pattern holes",
            "size" if root["x-pilot-expander"] == "honeycomb_pattern" => "Hex span",
            "size" => "Radius/bevel",
            "bore_diameter" => "Centre bore",
            "floor" => "Omit: wall",
            "target" => "body: solid; last_feature: latest feature",
            "open" => "Removed face",
            "direction" | "axis" => "World axis",
            "plane" => "Axes of plane",
            _ => "",
        };
        if !description.is_empty() {
            out["description"] = json!(description);
        }
        if out.get("enum").is_some() || (!key.is_empty() && out["type"] == "object") {
            if let Some(o) = out.as_object_mut() {
                o.remove("type");
            }
        }
        if let Some(props) = out.get_mut("properties").and_then(Value::as_object_mut) {
            for (name, v) in props {
                *v = walk(v, root, name);
            }
        }
        if let Some(items) = out.get_mut("items") {
            *items = walk(items, root, "items");
        }
        out
    }
    let s = flat(s);
    walk(&s, &s, "")
}

pub fn line(id: &str) -> String {
    format!("{id}: user words; inches to mm; omit unstated; decline on mismatch.")
}

/// File-owned scalar form; typed schemas remain the compatibility contract.
pub(crate) fn check_definition(fill: &Value) -> Result<(), String> {
    let fields = fill["properties"]
        .as_object()
        .ok_or("fill properties must be an object")?;
    if fields.is_empty() || fields.len() > 128 {
        return Err("fill requires 1..128 scalar fields".into());
    }
    for field in fields.values() {
        if !matches!(
            field["type"].as_str(),
            Some("number" | "integer" | "boolean" | "string")
        ) {
            return Err("fill fields must be scalar".into());
        }
        let path = field["x-slot"]
            .as_str()
            .ok_or("fill field needs a slot path")?;
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        if path.len() > 512
            || !path.starts_with('/')
            || parts.len() > 8
            || parts.iter().any(|p| p.is_empty())
        {
            return Err("fill slot requires 1..8 nonempty path components within 512 bytes".into());
        }
    }
    let check_names = |names: &Value| -> Result<(), String> {
        let names = names
            .as_array()
            .ok_or("fill field names must be an array")?;
        if names.len() > 128
            || names
                .iter()
                .any(|n| n.as_str().is_none_or(|n| !fields.contains_key(n)))
        {
            return Err("fill names must reference at most 128 listed fields".into());
        }
        Ok(())
    };
    check_names(&fill["required"])?;
    let groups = fill["groups"]
        .as_array()
        .ok_or("fill groups must be an array")?;
    if groups.len() > 128 {
        return Err("fill exceeds 128 feature groups".into());
    }
    for group in groups {
        check_names(&group["fields"])?;
        check_names(&group["required"])?;
    }
    Ok(())
}

/// File-owned scalar form; typed schemas remain the compatibility contract.
pub fn flat(schema: &Value) -> Value {
    let Some(fill) = schema.get("x-pilot-fill") else {
        return schema.clone();
    };
    json!({"type":"object","additionalProperties":false,"properties":fill["properties"],"required":fill["required"],"x-pilot-expander":schema["x-pilot-expander"]})
}

/// Coerce only explicitly supplied scalars. No dimensions or counts are inferred.
pub fn coerce(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    crate::repair::check_value(args)?;
    let fields = schema["properties"]
        .as_object()
        .ok_or("fill schema needs properties")?;
    let object = args.as_object_mut().ok_or("args requires one object")?;
    let mut changes = Vec::new();
    for (key, value) in object {
        let Some(field) = fields.get(key) else {
            continue;
        };
        let before = value.clone();
        if let Some(text) = value.as_str() {
            let text = text.trim();
            if field["type"] == "boolean" {
                match text.to_ascii_lowercase().as_str() {
                    "true" | "yes" => *value = json!(true),
                    "false" | "no" => *value = json!(false),
                    _ => {}
                }
            } else if matches!(field["type"].as_str(), Some("number" | "integer")) {
                let lower = text.to_ascii_lowercase();
                let length = field["x-unit"] == "mm";
                let (number, scale) = if length && lower.ends_with("mm") {
                    (lower.trim_end_matches("mm").trim(), 1.0)
                } else if length && lower.ends_with("in") {
                    (lower.trim_end_matches("in").trim(), 25.4)
                } else {
                    (lower.as_str(), 1.0)
                };
                if let Ok(n) = number.parse::<f64>() {
                    let n = n * scale;
                    if n.is_finite() {
                        *value = if field["type"] == "integer"
                            && n.fract() == 0.0
                            && (0.0..=64.0).contains(&n)
                        {
                            json!(n as u64)
                        } else {
                            json!(n)
                        };
                    }
                }
            } else if let Some(choices) = field["enum"].as_array() {
                let lower = text.to_ascii_lowercase();
                let synonym = match lower.as_str() {
                    "round" | "rounded" => "fillet",
                    "bevel" | "beveled" => "chamfer",
                    "corners" | "side" | "sides" => "vertical",
                    "inside" => "inner",
                    "outside" => "outer",
                    _ => lower.as_str(),
                };
                if let Some(choice) = choices
                    .iter()
                    .find(|c| c.as_str().is_some_and(|s| s.eq_ignore_ascii_case(synonym)))
                {
                    *value = choice.clone();
                }
            }
        }
        if *value != before {
            changes.push(json!({"field":key,"before":before,"after":value}));
        }
    }
    Ok(changes)
}

/// The sole flat-to-typed mapping. Nested build arguments pass through unchanged.
pub fn to_slots(args: &Value, schema: &Value) -> Result<Value, String> {
    crate::repair::check_value(args)?;
    let Some(fill) = schema.get("x-pilot-fill") else {
        return Ok(args.clone());
    };
    let fields = fill["properties"]
        .as_object()
        .ok_or("fill properties missing")?;
    if fields.len() > 128 {
        return Err("fill exceeds 128 scalar fields".into());
    }
    let object = args.as_object().ok_or("args requires one object")?;
    if let Some(groups) = fill["groups"].as_array() {
        if groups.len() > 128 {
            return Err("fill exceeds 128 feature groups".into());
        }
        for group in groups {
            let members = group["fields"]
                .as_array()
                .ok_or("fill group fields missing")?;
            if members
                .iter()
                .filter_map(Value::as_str)
                .any(|k| object.contains_key(k))
            {
                for key in group["required"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if !object.contains_key(key) {
                        return Err(format!(
                            "args.{key} missing; supply {key} to complete this feature"
                        ));
                    }
                }
            }
        }
    }
    let mut out = json!({});
    for (key, value) in object {
        let Some(field) = fields.get(key) else {
            if schema["properties"].get(key).is_some() {
                let legacy = &schema["properties"][key];
                let legacy = legacy["$ref"]
                    .as_str()
                    .and_then(|r| r.strip_prefix('#'))
                    .and_then(|p| schema.pointer(p))
                    .unwrap_or(legacy);
                if matches!(legacy["type"].as_str(), Some("object" | "array"))
                    && !value.is_object()
                    && !value.is_array()
                {
                    let names: Vec<_> = fields
                        .iter()
                        .filter(|(_, f)| {
                            f["x-slot"].as_str().is_some_and(|p| {
                                p.starts_with(&format!("/{key}/"))
                                    || (key == "holes" && p.starts_with("/hole_"))
                            })
                        })
                        .map(|(k, _)| k.as_str())
                        .collect();
                    return Err(format!(
                        "args.{key} requires its legacy container; use flat fields {}",
                        names.join(", ")
                    ));
                }
                if out.get(key).is_some() {
                    return Err(format!("conflicting flat and nested field {key}"));
                }
                out[key] = value.clone();
                continue;
            }
            return Err(format!("args.{key} is unknown; use a listed flat field"));
        };
        let path = field["x-slot"].as_str().ok_or("fill field slot missing")?;
        if path.len() > 512 || !path.starts_with('/') {
            return Err("fill slot requires a path of at most 512 bytes".into());
        }
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        if parts.len() > 8 || parts.iter().any(|p| p.is_empty()) {
            return Err("fill slot requires 1..8 nonempty path components".into());
        }
        fn insert(out: &mut Value, parts: &[&str], value: &Value) -> Result<(), String> {
            let Some((key, rest)) = parts.split_first() else {
                return Err("empty fill slot".into());
            };
            if rest.is_empty() {
                if let Some(previous) = out.get(*key) {
                    if *key == "hole_diameter" && previous == value {
                        return Ok(());
                    }
                    return Err(format!("conflicting flat and nested field {key}"));
                }
                out[*key] = value.clone();
            } else {
                if out.get(*key).is_none() {
                    out[*key] = json!({});
                }
                if !out[*key].is_object() {
                    return Err(format!("conflicting flat and nested field {key}"));
                }
                insert(&mut out[*key], rest, value)?;
            }
            Ok(())
        }
        insert(&mut out, &parts, value)?;
    }
    fn vectors(v: &mut Value) {
        if let Some(o) = v.as_object_mut() {
            for child in o.values_mut() {
                vectors(child);
            }
            if o.len() == 3 && ["0", "1", "2"].iter().all(|k| o.contains_key(*k)) {
                *v = json!([o["0"], o["1"], o["2"]]);
            }
        }
    }
    vectors(&mut out);
    let mut holes = Vec::new();
    for axis in ["X", "Y", "Z"] {
        if let Some(mut h) = out
            .as_object_mut()
            .and_then(|o| o.remove(&format!("hole_{axis}")))
        {
            if !h.is_object() {
                return Err("flat hole slots must form an object".into());
            }
            h["axis"] = json!(axis);
            holes.push(h);
        }
    }
    if !holes.is_empty() {
        if out.get("holes").is_some() {
            return Err("conflicting flat holes and nested holes".into());
        }
        out["holes"] = json!(holes);
    }
    Ok(out)
}
