//! Short field guidance shared by file recipes, legacy shapes and fill forms.
use serde_json::{json, Value};

pub(crate) fn describe(s: &mut Value) {
    let mirror = s["x-pilot-expander"] == "mirror";
    let edge_edit = matches!(s["x-pilot-expander"].as_str(), Some("fillet" | "chamfer"));
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
                "edges" if edge_edit => (
                    "all: every edge; vertical: upright bounding-box corners",
                    Some(json!("all")),
                ),
                "edges" => (
                    "vertical: four corner edges through thickness; all: every rim too",
                    Some(json!("vertical")),
                ),
                "sides" | "corner_sides" => ("Upright corners; right=+X, top=+Y", None),
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
                "boss_diameter" => ("Joined round peg/boss diameter; requires boss_height", None),
                "boss_height" => ("Rise above plate top; requires boss_diameter", None),
                "boss_offset_x" | "boss_offset_y" => {
                    ("Single boss offset from plate centre; omitted: 0", None)
                }
                "row" => (
                    "Centred feature row; count includes all; pitch is centre spacing",
                    None,
                ),
                "bolt_circle" => ("Feature centres on this circle", None),
                "grid" => (
                    "Feature centres: columns along sketch X, rows along sketch Y",
                    None,
                ),
                "corners" => ("Outside corner treatment", None),
                "bore_diameter" => ("Centre through bore", None),
                "hole_diameter" => (
                    "Pattern hole diameter; requires bolt_circle, grid or row",
                    None,
                ),
                "start_deg" => ("From +X toward +Y", Some(json!(0))),
                "pitch_x" | "pitch_y" => ("Feature centre spacing", None),
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
            "length" if root["x-pilot-expander"] == "plate" => {
                "Rectangle X; length+width OR diameter"
            }
            "width" if root["x-pilot-expander"] == "plate" => "Rectangle Y; omit with diameter",
            "diameter" if root["x-pilot-expander"] == "plate" => "Round base; omit length/width",
            "boss_diameter" => "Joined peg/boss; needs boss_height",
            "boss_height" => "Rise above plate top",
            "boss_offset_x" | "boss_offset_y" => "From plate centre; single boss only; omit: 0",
            "row_count" => "Total features in centred row",
            "row_pitch" => "Centre spacing",
            "row_axis" => "Sketch axis; omit: X",
            "grid_columns" => "Along X; A x B: A along longer side unless stated",
            "grid_rows" => "Along Y; 7 x 5 on 120 x 80: rows=5, columns=7",
            "grid_pitch_x" | "grid_pitch_y" => "Centre spacing; single pitch: use for both",
            "bolt_hole_count" => "Circle feature count (holes OR bosses)",
            "bolt_circle_diameter" => "Feature-centre circle diameter",
            "hole_diameter" => "Pattern holes",
            "size" if root["x-pilot-expander"] == "honeycomb_pattern" => "Hex span",
            "size" => "Radius/bevel",
            "bore_diameter" => "Centre bore",
            "floor" => "Omit: wall",
            "inside" => "True: clear inside dimensions; outside adds 2 walls and one floor",
            "lip" => "Request ring; omit sizes: height=wall, width=wall/2",
            "corner_sides" => "right=+X; top=+Y",
            "target" => "body: solid; last_feature: latest feature",
            "open" => "Removed face",
            "axis" if root["x-pilot-expander"] == "joint" => {
                "Aligned world axis; omit: Z; named origins keep their frames"
            }
            "direction" | "axis" => "World axis",
            "plane" => "Axes of plane",
            k if root["x-pilot-expander"] == "joint"
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
    if matches!(
        s["x-pilot-expander"].as_str(),
        Some("cross_drilled_box" | "plate")
    ) {
        out["description"] = s["description"].clone();
    }
    out
}

pub fn line(id: &str) -> String {
    let mut line = "You fill in one form from the user's CAD request. Call the tool once. Lengths in mm; convert inches (25.4) and cm (10). Leave optional fields out unless stated. If the form does not fit, set decline true.".to_owned();
    if id == "revolute_joint" {
        line.push_str(" Put any requested pivot/rotation angle in angle_deg; a quarter turn is 90 degrees. The builder drives the joint to this angle.");
    }
    if id == "joint" {
        line.push_str(" Use joint_type and component_a/b. Limits are ordered min/max pairs in deg or mm; omit pairs for unlimited motion.");
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
    if schema["x-pilot-expander"] == "plate" {
        out["description"] =
            json!("Base with holes OR bosses; centred layouts; no layout: single boss.");
    }
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

/// Reconcile spelling aliases without hiding conflicting values.
pub(crate) fn aliases(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    let mut changes = Vec::new();
    if schema["properties"].get("open").is_some() && args["open"] == true {
        args["open"] = json!("top");
        changes.push(json!({"field":"open","before":true,"after":"top"}));
    }
    if schema["properties"].get("centred").is_some() {
        let object = args.as_object_mut().ok_or("args requires one object")?;
        for alias in ["centered", "center", "centre"] {
            if let Some(value) = object.remove(alias) {
                if object.get("centred").is_some_and(|v| !agrees(v, &value)) {
                    return Err(format!("args.{alias} conflicts with centred"));
                }
                object.insert("centred".into(), value.clone());
                changes.push(
                    json!({"field":alias,"before":value,"after":null,"converted_to":"centred"}),
                );
            }
        }
    }
    Ok(changes)
}

/// Explicit lip requests may derive their missing dimensions from the wall.
fn lip_defaults(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    if schema["x-pilot-expander"] != "enclosure" {
        return Ok(Vec::new());
    }
    if args["lip"] == false {
        if ["lip_width", "lip_height", "lip_side"]
            .iter()
            .any(|k| args.get(k).is_some())
        {
            return Err("args.lip false conflicts with supplied lip fields".into());
        }
        args.as_object_mut()
            .ok_or("args requires one object")?
            .remove("lip");
        return Ok(vec![json!({"field":"lip","before":false,"after":null})]);
    }
    let nested = args["lip"].is_object();
    let requested = nested
        || args["lip"] == true
        || ["lip_width", "lip_height", "lip_side"]
            .iter()
            .any(|k| args.get(k).is_some());
    if !requested {
        return Ok(Vec::new());
    }
    let wall = args["wall"]
        .as_f64()
        .filter(|w| w.is_finite() && *w > 0.0 && *w <= 1_000_000.)
        .ok_or("args.wall requires a positive bounded millimetre value to default lip sizes")?;
    let mut changes = Vec::new();
    if args["lip"] == true {
        args.as_object_mut()
            .ok_or("args requires one object")?
            .remove("lip");
    }
    for (key, factor) in [("height", 1.0), ("width", 0.5)] {
        let flat = format!("lip_{key}");
        if args.get(&flat).is_none() && args["lip"].get(key).is_none() {
            let value = json!(wall * factor);
            if nested {
                args["lip"][key] = value.clone();
            } else {
                args[&flat] = value.clone();
            }
            changes.push(json!({"field":flat,"before":null,"after":value,"defaulted":true,"rule":"lip height = wall; width = wall / 2"}));
        }
    }
    Ok(changes)
}

/// Coerce supplied values and audit the documented wall-derived lip defaults.
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
    changes.extend(aliases(args, schema)?);
    if schema["x-pilot-expander"] == "joint" {
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
        if key == "corner_edges" && *value == true {
            *value = json!("vertical");
        }
        // A choice written as ["X","Z"], ["XZ"] or {"plane":"XZ"} is read as its text.
        if field["enum"].is_array() {
            let parts: Option<Vec<&str>> = match &*value {
                Value::Array(items) if (1..=3).contains(&items.len()) => {
                    items.iter().map(Value::as_str).collect()
                }
                Value::Object(map) if map.len() == 1 => {
                    map.values().next().and_then(Value::as_str).map(|s| vec![s])
                }
                _ => None,
            };
            if let Some(parts) = parts.filter(|p| p.iter().all(|s| s.len() <= 32)) {
                *value = json!(parts.concat());
            }
        }
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
                } else if length && lower.ends_with("cm") {
                    (lower.trim_end_matches("cm").trim(), 10.0)
                } else if length && lower.ends_with("inches") {
                    (lower.trim_end_matches("inches").trim(), 25.4)
                } else if length && lower.ends_with("inch") {
                    (lower.trim_end_matches("inch").trim(), 25.4)
                } else if length && lower.ends_with("in") {
                    (lower.trim_end_matches("in").trim(), 25.4)
                } else if key.ends_with("_deg") {
                    let number = lower
                        .trim_end_matches("degrees")
                        .trim_end_matches("degree")
                        .trim_end_matches("deg")
                        .trim_end_matches('°')
                        .trim();
                    (number, 1.0)
                } else {
                    (lower.as_str(), 1.0)
                };
                let parsed = number.parse::<f64>().ok().or_else(|| {
                    let parts: Vec<_> = number.split_whitespace().collect();
                    let fraction = parts.last()?.split_once('/')?;
                    let numerator = fraction.0.parse::<f64>().ok()?;
                    let denominator = fraction.1.parse::<f64>().ok().filter(|d| *d > 0.0)?;
                    match parts.len() {
                        1 => Some(numerator / denominator),
                        2 => Some(parts[0].parse::<f64>().ok()? + numerator / denominator),
                        _ => None,
                    }
                });
                if let Some(n) = parsed {
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
                let lower = lower
                    .strip_suffix(" plane")
                    .or_else(|| lower.strip_suffix("-plane"))
                    .unwrap_or(&lower);
                let synonym = match lower {
                    "round" | "rounded" => "fillet",
                    "bevel" | "beveled" => "chamfer",
                    "corners" | "side" | "sides" => "vertical",
                    "inside" => "inner",
                    "outside" => "outer",
                    _ if choices.iter().any(|c| c == "U") => section_kind(lower).unwrap_or(lower),
                    _ => lower,
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
    // Older fills put corner names in corner_edges. Move the explicit subset.
    if matches!(
        schema["x-pilot-expander"].as_str(),
        Some("plate" | "fillet" | "chamfer")
    ) {
        for key in ["corner_edges", "corner_sides"]
            .into_iter()
            .filter(|key| fields.contains_key(*key))
        {
            if let Some(text) = args[key].as_str() {
                let lower = text.trim().to_ascii_lowercase();
                let normalized = match lower.as_str() {
                    "+x" => "right".into(),
                    "-x" => "left".into(),
                    "+y" => "top".into(),
                    "-y" => "bottom".into(),
                    _ => lower
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>(),
                };
                let side = match normalized.as_str() {
                    "right"
                    | "rightside"
                    | "rightx"
                    | "rightsidex"
                    | "righthandside"
                    | "bothrightcorners"
                    | "toprightbottomright"
                    | "bottomrighttopright"
                    | "toprightandbottomright"
                    | "bottomrightandtopright" => Some("right"),
                    "left"
                    | "leftside"
                    | "leftx"
                    | "leftsidex"
                    | "lefthandside"
                    | "bothleftcorners"
                    | "topleftbottomleft"
                    | "bottomlefttopleft"
                    | "topleftandbottomleft"
                    | "bottomleftandtopleft" => Some("left"),
                    "top" | "topside" | "toplefttopright" => Some("top"),
                    "bottom" | "bottomside" | "bottomleftbottomright" => Some("bottom"),
                    "allfour" | "allfourcorners" | "allcorners" | "fourcorners"
                    | "uprightcorners" | "verticalcorners" => Some("all"),
                    "topright" => Some("top_right"),
                    "bottomright" => Some("bottom_right"),
                    "topleft" => Some("top_left"),
                    "bottomleft" => Some("bottom_left"),
                    _ => None,
                };
                if let Some(side) = side {
                    if key == "corner_edges" && args.get("corner_sides").is_some_and(|v| v != side)
                    {
                        return Err("args.corner_edges conflicts with corner_sides".into());
                    }
                    let before = args[key].clone();
                    args["corner_sides"] = json!(side);
                    if key == "corner_edges" {
                        args[key] = json!("vertical");
                    }
                    if before != args[key] {
                        changes.push(json!({"field":key,"before":before,"after":args[key],"corner_sides":side}));
                    }
                }
            }
        }
    }
    changes.extend(lip_defaults(args, schema)?);
    changes.extend(defaults(args, schema)?);
    changes.extend(legacy_hole_coordinates(args, schema)?);
    Ok(changes)
}

/// Read a profile section name such as "T-section", "U_CHANNEL" or "equal angle".
fn section_kind(text: &str) -> Option<&'static str> {
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    // A leading single letter names the section: "T-section", "u channel", "inverted T".
    if let Some(letter) = words.iter().find(|w| w.len() == 1) {
        return match *letter {
            "l" => Some("L"),
            "t" => Some("T"),
            "u" => Some("U"),
            "c" => Some("C"),
            _ => None,
        };
    }
    if has("angle") {
        Some("L")
    } else if has("tee") {
        Some("T")
    } else if has("channel") {
        Some("U")
    } else if words
        .iter()
        .any(|w| matches!(*w, "rect" | "rectangular" | "rectangle" | "bar" | "flat"))
    {
        Some("rectangle")
    } else {
        None
    }
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
        let web = out.get("web_thickness").cloned();
        let flange = out.get("flange_thickness").cloned();
        if web.is_some() || flange.is_some() {
            let object = out.as_object_mut().ok_or("slots require an object")?;
            let same = |v: &Option<Value>| v.as_ref().is_none_or(|v| agrees(&t, v));
            if same(&web) && same(&flange) {
                object.remove("web_thickness");
                object.remove("flange_thickness");
            } else {
                // A specific web or flange value wins; the general thickness fills the other.
                object.remove("thickness");
                object.insert("web_thickness".into(), web.unwrap_or_else(|| t.clone()));
                object.insert("flange_thickness".into(), flange.unwrap_or(t));
            }
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
    let mut args = args.clone();
    aliases(&mut args, schema)?;
    let Some(fill) = schema.get("x-pilot-fill") else {
        return Ok(args);
    };
    let fields = fill["properties"]
        .as_object()
        .ok_or("fill properties missing")?;
    if fields.len() > 128 {
        return Err("fill exceeds 128 scalar fields".into());
    }
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
    lip_defaults(&mut args, schema)?;
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
        let Some(field) = fields
            .get(key)
            .filter(|_| !(key == "lip" && value.is_object()))
        else {
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
    if schema["x-pilot-expander"] == "plate"
        && out.get("hole_diameter").is_none()
        && out.get("boss_diameter").is_none()
    {
        for (layout, field) in [
            ("bolt_circle", "bolt_hole_diameter"),
            ("grid", "grid_hole_diameter"),
            ("row", "hole_diameter"),
        ] {
            if out.get(layout).is_some() {
                return Err(format!("args.{field} missing; supply {field} for holes, or boss_diameter and boss_height for joined raised features"));
            }
        }
    }
    Ok(out)
}
