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
                    "Omit: rows along longer side; pointy_top: X rows; flat_top: Y rows",
                    None,
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
            "orientation" => "Omit: longer-side rows; pointy_top: X rows; flat_top: Y rows",
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
            "axis" if root["x-pilot-expander"] == "revolute_joint" => {
                "Aligned world axis; omit: Z; named origins keep their frames"
            }
            "direction" | "axis" => "World axis",
            "plane" => "Axes of plane",
            k if root["x-pilot-expander"] == "revolute_joint"
                && (k.starts_with("limit")
                    || k.starts_with("component_")
                    || k.starts_with("point_")
                    || k == "joint_type") =>
            {
                resolved["description"].as_str().unwrap_or("")
            }
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
    let mut out = walk(&s, &s, "");
    if s["x-pilot-expander"] == "cross_drilled_box" {
        out["description"] = s["description"].clone();
    }
    out
}

pub fn line(id: &str) -> String {
    let mut line = "You fill in one form from the user's CAD request. Call the tool once. Lengths in mm; convert inches (25.4) and cm (10). Leave optional fields out unless stated. If the form does not fit, set decline true.".to_owned();
    if id == "revolute_joint" {
        line.push_str(" For contact pin/hole mating use only pin_component, hole_component and angle_deg. Otherwise use joint_type and component_a/b. Limits are ordered min/max pairs in deg or mm; omit pairs for unlimited motion.");
    }
    line
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
        if let Some(scale) = field.get("x-scale") {
            if scale
                .as_f64()
                .is_none_or(|s| !s.is_finite() || s <= 0.0 || s > 1.0)
            {
                return Err("fill scale must be positive and at most 1".into());
            }
        }
        if let Some(trigger) = field.get("x-default-when") {
            if !trigger.as_str().is_some_and(|t| fields.contains_key(t))
                || !matches!(field["type"].as_str(), Some("string" | "boolean"))
                || field.get("default").is_none()
            {
                return Err("conditional defaults need a listed trigger and a string or boolean default; never a dimension".into());
            }
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
    let mut out = json!({"type":"object","additionalProperties":false,"properties":fill["properties"],"required":fill["required"],"x-pilot-expander":schema["x-pilot-expander"]});
    if schema["x-pilot-expander"] == "cross_drilled_box" {
        out["description"] =
            json!("Holes are centred unless you give offset_u/offset_v from the face centre.");
    }
    out
}

fn legacy_hole_key(key: &str) -> bool {
    ["x", "y", "z"].iter().any(|axis| {
        ["x", "y", "z"]
            .iter()
            .any(|coordinate| key == format!("hole_{axis}_position_{coordinate}"))
    })
}

/// Retired flat coordinates are independent hints, never a partial vector.
fn legacy_hole_coordinates(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    if schema["x-pilot-expander"] != "cross_drilled_box" {
        return Ok(Vec::new());
    }
    let mut changes = Vec::new();
    for (axis, name) in ["x", "y", "z"].iter().enumerate() {
        for (coordinate, label) in ["x", "y", "z"].iter().enumerate() {
            let key = format!("hole_{name}_position_{label}");
            let Some(value) = args.get(&key).cloned() else {
                continue;
            };
            let offset = if (0..3).filter(|i| *i != axis).position(|i| i == coordinate) == Some(1) {
                "offset_v"
            } else {
                "offset_u"
            };
            let fix = format!(
                "use hole_{name}_{offset} from the face centre, e.g. hole_{name}_{offset}:0"
            );
            let number = value
                .as_f64()
                .filter(|v| v.is_finite() && v.abs() <= 1_000_000.)
                .ok_or_else(|| format!("args.{key} requires a bounded number; {fix}"))?;
            let dimension = ["length", "width", "height"][coordinate];
            let size = args[dimension]
                .as_f64()
                .filter(|v| *v > 0. && *v <= 1_000_000.)
                .ok_or_else(|| format!("args.{dimension} missing or invalid for {key}; {fix}"))?;
            let corner_key = format!("corner_{label}");
            let corner = args
                .get(&corner_key)
                .or_else(|| args["corner"].get(coordinate));
            let lo = match corner {
                Some(v) => v
                    .as_f64()
                    .filter(|v| v.is_finite() && v.abs() <= 1_000_000.)
                    .ok_or_else(|| format!("args.{corner_key} requires a bounded number; {fix}"))?,
                None => 0.,
            };
            let centre = lo + size / 2.;
            let centred = (number - centre).abs() < 1e-8;
            let entry = (number - lo).abs() < 1e-8 || (number - lo - size).abs() < 1e-8;
            let supported = centred
                || if coordinate == axis {
                    entry
                } else {
                    number > lo && number < lo + size
                };
            if !supported {
                return Err(format!(
                    "args.{key} {number} is not a supported hole coordinate; {fix}"
                ));
            }
            let target = format!("hole_{name}_{offset}");
            if coordinate != axis && !centred {
                let converted = json!(number - centre);
                if args.get(&target).is_some_and(|v| !agrees(v, &converted)) {
                    return Err(format!("args.{key} conflicts with {target}; {fix}"));
                }
                changes.push(json!({"field":target,"before":args.get(&target),"after":converted}));
                args[&target] = converted;
            }
            args.as_object_mut()
                .ok_or("args requires one object")?
                .remove(&key);
            changes.push(json!({"field":key,"before":value,"after":null,"converted_to":if coordinate != axis && !centred {Some(target)} else {None}}));
        }
    }
    Ok(changes)
}

/// Coerce explicitly supplied values, including unit axes. No dimensions or counts are inferred.
pub fn coerce(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    crate::repair::check_value(args)?;
    let pattern_changes = if crate::planner::patterns::is_pattern(
        schema["x-pilot-expander"].as_str().unwrap_or(""),
    ) {
        crate::planner::patterns::coerce(args)?
    } else {
        Vec::new()
    };
    let fields = schema["properties"]
        .as_object()
        .ok_or("fill schema needs properties")?;
    let mut changes = pattern_changes;
    if schema["x-pilot-expander"] == "revolute_joint" {
        changes.extend(crate::planner::joints::coerce(args)?);
    }
    let object = args.as_object_mut().ok_or("args requires one object")?;
    for (key, value) in object {
        let compatibility = json!({"type":"number","x-unit":"mm"});
        let Some(field) = fields.get(key).or_else(|| {
            (schema["x-pilot-expander"] == "cross_drilled_box" && legacy_hole_key(key))
                .then_some(&compatibility)
        }) else {
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
    changes.extend(defaults(args, schema)?);
    changes.extend(legacy_hole_coordinates(args, schema)?);
    Ok(changes)
}

/// Infer only file-declared, conditional non-dimensional defaults.
pub fn defaults(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    let supplied = args.clone();
    let object = args.as_object_mut().ok_or("args requires one object")?;
    let mut changes = Vec::new();
    for (key, field) in schema["properties"].as_object().into_iter().flatten() {
        if field["x-default-when"]
            .as_str()
            .is_some_and(|trigger| object.contains_key(trigger))
            && !object.contains_key(key)
            && field["x-slot"]
                .as_str()
                .is_none_or(|path| supplied.pointer(path).is_none())
            && matches!(field["type"].as_str(), Some("string" | "boolean"))
        {
            if let Some(value) = field.get("default") {
                object.insert(key.clone(), value.clone());
                changes.push(json!({"field":key,"before":null,"after":value,"defaulted":true}));
            }
        }
    }
    Ok(changes)
}

fn agrees(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0),
        _ => a == b,
    }
}

pub(crate) fn enum_meanings(field: &Value) -> String {
    field["enum"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let meaning = match v.as_str().unwrap_or("") {
                "vertical" => "the four corner edges through the thickness",
                "all" => "every edge, including rims",
                "fillet" => "round with a radius",
                "chamfer" => "bevel with a distance",
                "inner" => "flush with the inner wall face",
                "outer" => "flush with the outer wall face",
                "top" => "remove the top face",
                "bottom" => "remove the bottom face",
                "L" => "angle section",
                "T" => "tee section",
                "U" => "channel opening toward sketch +Y",
                "C" => "channel opening toward sketch +X",
                "rectangle" => "rectangular section",
                "XY" => "sketch X/Y, normal +Z",
                "XZ" => "sketch X/-Z, normal +Y",
                "YZ" => "sketch Y/Z, normal +X",
                "X" => "world X axis",
                "Y" => "world Y axis",
                "Z" => "world Z axis",
                "body" => "whole solid",
                "last_feature" => "latest feature",
                "across_flats" => "edge to edge",
                "across_corners" => "vertex to vertex",
                "pointy_top" => "hex vertex up",
                "flat_top" => "hex edge up",
                "fill" => "fit complete hexes within the border",
                "exact" => "use explicit rows and columns",
                _ => field["description"].as_str().unwrap_or("listed choice"),
            };
            format!("{v} ({meaning})")
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn merge(out: &mut Value, value: &Value, path: &str) -> Result<(), String> {
    if let (Some(left), Some(right)) = (out.as_array_mut(), value.as_array()) {
        if left.len() == right.len() {
            for (index, (a, b)) in left.iter_mut().zip(right).enumerate() {
                merge(a, b, &format!("{path}/{index}"))?;
            }
            return Ok(());
        }
    }
    if let (Some(left), Some(right)) = (out.as_object_mut(), value.as_object()) {
        for (key, value) in right {
            if let Some(previous) = left.get_mut(key) {
                merge(previous, value, &format!("{path}/{key}"))?;
            } else {
                left.insert(key.clone(), value.clone());
            }
        }
        return Ok(());
    }
    if agrees(out, value) {
        Ok(())
    } else {
        Err(format!(
            "conflicting {path}: {out} and {value}; supply agreeing values"
        ))
    }
}

/// Reconcile alternative representations once, before typed recipe validation.
pub(crate) fn consistent(out: &mut Value) -> Result<(), String> {
    if let Some(t) = out.get("thickness").cloned() {
        let mut expanded = false;
        for key in ["web_thickness", "flange_thickness"] {
            if let Some(value) = out.get(key) {
                expanded = true;
                if !agrees(&t, value) {
                    return Err(format!(
                        "thickness {t} conflicts with {key} {value}; supply agreeing values"
                    ));
                }
            }
        }
        if expanded {
            let object = out.as_object_mut().ok_or("slots require an object")?;
            object.remove("web_thickness");
            object.remove("flange_thickness");
        }
    }
    if let (Some(s), Some(t), Some(n)) = (
        out.get("spacing"),
        out.get("total_length"),
        out["count"].as_f64(),
    ) {
        if let Some(s) = s.as_f64() {
            let expected = json!(s * (n - 1.0));
            if !agrees(t, &expected) {
                return Err(format!("total_length {t} conflicts with spacing {s} and count {n} (first-to-last length {expected})"));
            }
            if let Some(object) = out.as_object_mut() {
                object.remove("total_length");
            }
        }
    }
    Ok(())
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
    let mut args = args.clone();
    defaults(&mut args, &flat(schema))?;
    legacy_hole_coordinates(&mut args, schema)?;
    let object = args.as_object().ok_or("args requires one object")?;
    for (key, value) in object {
        if let Some(field) = fields.get(key).filter(|f| f.get("enum").is_some()) {
            if value.is_array() || value.is_object() {
                return Err(format!(
                    "args.{key} takes {}; not a list or an object; use one listed string",
                    enum_meanings(field)
                ));
            }
        }
    }
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
                    if !object.contains_key(key)
                        && fields[key]["x-slot"]
                            .as_str()
                            .is_none_or(|path| args.pointer(path).is_none())
                    {
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
                if let Some(previous) = out.get_mut(key) {
                    merge(previous, value, key)?;
                } else {
                    out[key] = value.clone();
                }
                continue;
            }
            return Err(format!("args.{key} is unknown; use a listed flat field"));
        };
        let mut validation = field.clone();
        if let Some(o) = validation.as_object_mut() {
            o.retain(|k, _| !k.starts_with("x-"));
        }
        super::validate_at(value, &validation, &validation, &format!("args.{key}"), 0)?;
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
            if let Some(array) = out.as_array_mut() {
                let index = key
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < 3)
                    .ok_or("vector slot requires a coordinate index 0..2")?;
                let coordinate = array
                    .get_mut(index)
                    .ok_or("vector slot requires three coordinates")?;
                return if rest.is_empty() {
                    merge(coordinate, value, key)
                } else {
                    insert(coordinate, rest, value)
                };
            }
            if rest.is_empty() {
                if let Some(previous) = out.get_mut(*key) {
                    return merge(previous, value, key);
                }
                out[*key] = value.clone();
            } else {
                if out.get(*key).is_none() {
                    out[*key] = json!({});
                }
                if !out[*key].is_object() && !out[*key].is_array() {
                    return Err(format!("conflicting flat and nested field {key}"));
                }
                insert(&mut out[*key], rest, value)?;
            }
            Ok(())
        }
        let scaled;
        let value = if let Some(scale) = field["x-scale"].as_f64() {
            let n = value
                .as_f64()
                .filter(|n| *n > 0.0 && *n <= 1_000_000.0)
                .ok_or_else(|| format!("args.{key} {value} must be above 0 and at most 1000000"))?;
            scaled = json!(n * scale);
            &scaled
        } else {
            value
        };
        insert(&mut out, &parts, value).map_err(|e| {
            let supplied = fields
                .iter()
                .filter(|(_, f)| f["x-slot"] == field["x-slot"])
                .filter_map(|(k, _)| object.get(k).map(|v| format!("{k} {v}")))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{supplied}: {e}")
        })?;
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
        if let Some(previous) = out.get_mut("holes") {
            merge(previous, &json!(holes), "holes")?;
        } else {
            out["holes"] = json!(holes);
        }
    }
    consistent(&mut out)?;
    Ok(out)
}
