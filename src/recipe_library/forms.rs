//! Short field guidance shared by file recipes, legacy shapes and fill forms.
use serde_json::{json, Value};
mod omitted;

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
            "centred" if root["x-pilot-expander"] == "enclosure" => {
                "Box centred in XYZ; else corner origin"
            }
            "centred" if root["x-pilot-expander"] == "profile" => {
                "Centre section in sketch plane; extrusion starts there"
            }
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
            "length" if root["x-pilot-expander"] == "enclosure" => "Box X",
            "width" if root["x-pilot-expander"] == "enclosure" => "Box Y",
            "diameter" if root["x-pilot-expander"] == "plate" => "Round base; omit length/width",
            "boss_diameter" => "Joined peg/boss; needs boss_height",
            "boss_height" => "Rise above plate top",
            "boss_offset_x" | "boss_offset_y" => "From plate centre; single boss only; omit: 0",
            "row_count" => "Total features in centred row",
            "row_pitch" => "Centre spacing",
            "row_offset_x" | "row_offset_y" => "Row centre - plate centre",
            "row_axis" => "Sketch axis; omit: X",
            "grid_columns" => "Sketch X; A x B: A along longer side",
            "grid_rows" => "Sketch Y; explicit rows/columns win",
            "grid_pitch_x" | "grid_pitch_y" => "Centre spacing; single pitch: use for both",
            "bolt_hole_count" => "Circle feature count (holes OR bosses)",
            "bolt_circle_diameter" => "Feature-centre circle diameter",
            "bolt_circle_radius" => "Circle radius",
            "hole_diameter" => "Pattern holes",
            "size" if root["x-pilot-expander"] == "honeycomb_pattern" => "Hex span",
            "size" => "Radius/bevel",
            "bore_diameter" => "Centre bore",
            "floor" => "Base/roof thickness; omit: wall",
            "inside" => "True: clear inside dimensions; outside adds 2 walls and one floor",
            "lip" => "Request ring; omit sizes: height=wall, width=wall/2",
            "corner_sides" => "right=+X; top=+Y",
            "target" => "body: solid; last_feature: latest feature",
            "lip_side" => "Flush with inner/outer wall face",
            "lip_height" => "Rise beyond opening",
            "open" => "Removed face",
            "axis" if root["x-pilot-expander"] == "joint" => {
                "Aligned world axis; omit: Z; named origins keep their frames"
            }
            "axis" if root["x-pilot-expander"] == "circular_pattern" => {
                "Through origin; seed sets circle radius"
            }
            "direction" | "axis" => "World axis",
            "count"
                if crate::planner::patterns::is_pattern(
                    root["x-pilot-expander"].as_str().unwrap_or(""),
                ) =>
            {
                "Includes seed; N copies/more: N+1; N total/slots/row: N"
            }
            "spacing" => "Centre pitch in mm; 1 in apart = 25.4",
            "total_length" => "First to last centre = spacing*(count-1)",
            "edge" => "Inspected straight edge, start to end; seed stays fixed",
            "angle_deg" if root["x-pilot-expander"] == "circular_pattern" => {
                "Sweep: full 360; partial arcs include both ends"
            }
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
    if s["x-pilot-expander"] == "enclosure" {
        out["description"] =
            json!("Outside box incl floor, excl lip; inside:true: cavity; cover/lid opens bottom.");
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
                .is_none_or(|s| !s.is_finite() || s <= 0.0 || s > 2.0)
            {
                return Err("fill scale must be positive and at most 2".into());
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
    // Reconcile singular/plural words only against unambiguous listed fields.
    let singular = |name: &str| {
        name.split('_')
            .map(|word| {
                if word.ends_with("sses")
                    || word.ends_with("ches")
                    || word.ends_with("shes")
                    || word.ends_with("xes")
                {
                    word.strip_suffix("es").unwrap_or(word)
                } else if word.ends_with("ss") {
                    word
                } else {
                    word.strip_suffix('s').unwrap_or(word)
                }
            })
            .collect::<Vec<_>>()
            .join("_")
    };
    let fields = schema
        .get("x-pilot-fill")
        .and_then(|f| f["properties"].as_object())
        .or_else(|| schema["properties"].as_object())
        .ok_or("fill schema needs properties")?;
    let object = args.as_object_mut().ok_or("args requires one object")?;
    let keys: Vec<_> = object.keys().cloned().collect();
    for key in keys {
        if fields.contains_key(&key) {
            continue;
        }
        let candidates: Vec<_> = fields
            .keys()
            .filter(|name| singular(name) == singular(&key))
            .collect();
        if let [target] = candidates.as_slice() {
            if let Some(value) = object.remove(&key) {
                if object.get(*target).is_some_and(|v| !agrees(v, &value)) {
                    return Err(format!("args.{key} conflicts with {target}"));
                }
                object.insert((*target).clone(), value.clone());
                changes
                    .push(json!({"field":key,"before":value,"after":null,"converted_to":target}));
            }
        }
    }
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

/// These markers make keyword-only inference unreliable. Abstaining does not
/// validate or change any value supplied by the model.
pub(crate) fn unclear_request(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| unclear_word(&w.to_ascii_lowercase()))
}

fn unclear_word(w: &str) -> bool {
    matches!(
        w,
        "not" | "no" | "never" | "without" | "or" | "either" | "neither" | "instead"
    )
}

/// A negation or alternative within three words of a cue makes that cue unclear;
/// one elsewhere in the request ("a cover with no lip") does not.
fn unclear_near(words: &[&str], at: usize) -> bool {
    words[at.saturating_sub(3)..(at + 4).min(words.len())]
        .iter()
        .any(|w| unclear_word(w))
}

/// Infer bounded choices and unambiguous omissions from the latest request; never
/// override supplied slots.
pub fn request_defaults(
    args: &mut Value,
    schema: &Value,
    request: &Value,
) -> Result<Vec<Value>, String> {
    crate::repair::check_value(args)?;
    let id = schema["x-pilot-expander"].as_str().unwrap_or("");
    if args["decline"] == true {
        return Ok(Vec::new());
    }
    let mut recovered = if matches!(id, "profile" | "plate") {
        omitted::recover(args, schema, request)?
    } else {
        Vec::new()
    };
    if !matches!(id, "revolute_joint" | "enclosure" | "plate")
        || (id == "revolute_joint" && args.get("angle_deg").is_some())
    {
        return Ok(recovered);
    }
    let text = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    if text.len() > 65_536 {
        return Err(format!(
            "{} request exceeds 65536 bytes",
            if id == "revolute_joint" {
                "joint angle"
            } else {
                "form default"
            }
        ));
    }
    let text = text.to_ascii_lowercase().replace('°', " degrees ");
    let words: Vec<_> = text
        .split(|c: char| !c.is_alphanumeric() && !matches!(c, '.' | '-' | '+'))
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| !s.is_empty())
        .collect();
    if id != "revolute_joint" {
        // Hyphens are separators for open-top and front-plane, not axis signs.
        let words: Vec<_> = text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        let has_pair = |a, b| {
            words
                .windows(2)
                .enumerate()
                .any(|(i, w)| w == [a, b] && !unclear_near(&words, i))
        };
        let choice = if id == "enclosure" && args.get("open").is_none() {
            let top =
                has_pair("open", "top") || has_pair("top", "open") || has_pair("top", "opening");
            let bottom = has_pair("open", "bottom")
                || has_pair("open", "underside")
                || has_pair("bottom", "open")
                || has_pair("underside", "open")
                || has_pair("bottom", "opening");
            // A lip "for a lid" does not turn an enclosure into a cover.
            let cover = words
                .iter()
                .position(|w| {
                    matches!(
                        *w,
                        "cover"
                            | "lid"
                            | "enclosure"
                            | "housing"
                            | "box"
                            | "case"
                            | "container"
                            | "tray"
                    )
                })
                .is_some_and(|n| {
                    matches!(words[n], "cover" | "lid")
                        && !words[..n]
                            .iter()
                            .any(|w| matches!(*w, "no" | "not" | "without"))
                });
            match (top, bottom) {
                (true, false) => Some(("open", "top", "explicit opening in request")),
                (false, true) => Some(("open", "bottom", "explicit opening in request")),
                (false, false) if cover => Some(("open", "bottom", "cover/lid opens underneath")),
                _ => None,
            }
        } else if id == "plate"
            && args.get("plane").is_none()
            && has_pair("front", "plane")
            && !words.iter().any(|w| matches!(*w, "xy" | "yz"))
            && !["top", "right", "back", "side", "bottom"]
                .iter()
                .any(|w| has_pair(w, "plane"))
        {
            Some(("plane", "XZ", "front plane is XZ"))
        } else {
            None
        };
        if id == "plate" && !unclear_request(&text) {
            recovered.extend(grid_rows(args, &text)?);
        }
        if let Some((field, value, rule)) = choice {
            args.as_object_mut()
                .ok_or("args requires one object")?
                .insert(field.into(), json!(value));
            recovered.push(
                json!({"field":field,"before":null,"after":value,"defaulted":true,"rule":rule}),
            );
        }
        return Ok(recovered);
    }
    if words
        .iter()
        .any(|w| matches!(*w, "limit" | "limits" | "range"))
    {
        return Ok(Vec::new());
    }
    let mut angles = Vec::new();
    for (i, pair) in words.windows(2).enumerate() {
        let angle = if matches!(pair[1], "deg" | "degree" | "degrees") {
            pair[0].parse::<f64>().ok().filter(|n| n.is_finite())
        } else if pair == ["quarter", "turn"] {
            Some(90.)
        } else {
            None
        };
        if let Some(angle) = angle {
            if unclear_near(&words, i) {
                return Ok(Vec::new());
            }
            angles.push(angle);
        }
    }
    if let [angle] = angles.as_slice() {
        args.as_object_mut()
            .ok_or("args requires one object")?
            .insert("angle_deg".into(), json!(angle));
        return Ok(vec![
            json!({"field":"angle_deg","before":null,"after":angle,"defaulted":true,"rule":"single explicit rotation angle in request"}),
        ]);
    }
    Ok(Vec::new())
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
    let fields = schema["properties"]
        .as_object()
        .ok_or("fill schema needs properties")?;
    let mut changes = Vec::new();
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
        let field = field["$ref"]
            .as_str()
            .and_then(|reference| reference.strip_prefix('#'))
            .and_then(|pointer| schema.pointer(pointer))
            .unwrap_or(field);
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
                // Exact choices stay intact, even if another choice is their prefix.
                if choices.iter().any(|choice| choice == value) {
                    if *value != before {
                        changes.push(json!({"field":key,"before":before,"after":value}));
                    }
                    continue;
                }
                let candidates: Vec<_> = choices
                    .iter()
                    .filter(|choice| {
                        choice.as_str().is_some_and(|choice| {
                            !choice.is_empty()
                                && (text.eq_ignore_ascii_case(choice)
                                    || text.get(..choice.len()).is_some_and(|prefix| {
                                        prefix.eq_ignore_ascii_case(choice)
                                            && text[choice.len()..]
                                                .chars()
                                                .next()
                                                .is_some_and(|c| !c.is_alphanumeric())
                                    }))
                        })
                    })
                    .collect();
                if candidates.len() == 1 {
                    *value = candidates[0].clone();
                    changes.push(json!({"field":key,"before":before,"after":value}));
                    continue;
                }
                if candidates.len() > 1 {
                    if *value != before {
                        changes.push(json!({"field":key,"before":before,"after":value}));
                    }
                    continue;
                }
                let lower = text.to_ascii_lowercase();
                let lower = lower
                    .strip_suffix(" plane")
                    .or_else(|| lower.strip_suffix("-plane"))
                    .unwrap_or(&lower);
                let synonym = match lower {
                    "round" | "rounded" | "radius" => "fillet",
                    "bevel" | "beveled" => "chamfer",
                    "corners" | "side" | "sides" => "vertical",
                    "front" if key == "plane" => "XZ",
                    "top" if key == "plane" => "XY",
                    "right" | "r" if key == "plane" => "YZ",
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
    if crate::planner::patterns::is_pattern(schema["x-pilot-expander"].as_str().unwrap_or("")) {
        changes.extend(crate::planner::patterns::coerce(args)?);
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
    changes.extend(drop_unused_stem(args, schema)?);
    changes.extend(lip_defaults(args, schema)?);
    changes.extend(defaults(args, schema)?);
    changes.extend(legacy_hole_coordinates(args, schema)?);
    Ok(changes)
}

/// A stem offset has no geometric meaning for a known non-T section.
fn drop_unused_stem(args: &mut Value, schema: &Value) -> Result<Vec<Value>, String> {
    if schema["x-pilot-expander"] == "profile"
        && matches!(args["kind"].as_str(), Some("L" | "U" | "C" | "rectangle"))
    {
        if let Some(before) = args
            .as_object_mut()
            .ok_or("args requires one object")?
            .remove("stem_offset")
        {
            return Ok(vec![
                json!({"field":"stem_offset","before":before,"after":null,"rule":"stem_offset is unused for non-T profiles"}),
            ]);
        }
    }
    Ok(Vec::new())
}

/// Infer counts only from a single count pair explicitly attached to a hole grid.
fn grid_rows(args: &mut Value, text: &str) -> Result<Vec<Value>, String> {
    if args.get("grid_rows").is_some() || args.get("grid").is_some() {
        return Ok(Vec::new());
    }
    let columns = args["grid_columns"]
        .as_u64()
        .or_else(|| args["grid_columns"].as_str()?.parse().ok());
    let text = text.replace(['×', 'x'], " x ");
    let words: Vec<_> = text
        .split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '-' | '+'))
        .filter(|s| !s.is_empty())
        .collect();
    if words
        .iter()
        .any(|w| matches!(*w, "no" | "not" | "without" | "or" | "rows" | "columns"))
    {
        return Ok(Vec::new());
    }
    let pairs: Vec<_> = words
        .windows(3)
        .enumerate()
        .filter(|(_, w)| {
            let numeric =
                |s: &str| s.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-'));
            matches!(w[1], "x" | "by") && numeric(w[0]) && numeric(w[2])
        })
        .collect();
    if let [(i, pair)] = pairs.as_slice() {
        let (Ok(a), Ok(b)) = (
            pair[0].parse::<u64>(),
            pair[2].trim_end_matches('.').parse::<u64>(),
        ) else {
            return Ok(Vec::new());
        };
        let context = &words[i.saturating_sub(2)..(i + 5).min(words.len())];
        let units = ["mm", "cm", "in", "inch", "inches"];
        if Some(a) == columns
            && (1..=64).contains(&b)
            && context
                .iter()
                .any(|w| matches!(*w, "grid" | "array" | "holes"))
            && !context.iter().any(|w| units.contains(w))
        {
            args.as_object_mut()
                .ok_or("args requires one object")?
                .insert("grid_rows".into(), json!(b));
            return Ok(vec![
                json!({"field":"grid_rows","before":null,"after":b,"rule":"single explicit hole grid count pair with matching grid_columns"}),
            ]);
        }
    }
    Ok(Vec::new())
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
        Some("C")
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
    drop_unused_stem(&mut args, schema)?;
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
            // Offset slots also represent a single recovered hole; they do not
            // activate the boss dimensions in that case. The expander still
            // rejects offsets combined with layouts or incomplete bosses.
            if schema["x-pilot-expander"] == "plate"
                && object.contains_key("hole_diameter")
                && !object.contains_key("boss_diameter")
                && !object.contains_key("boss_height")
                && members.iter().any(|k| k == "boss_diameter")
            {
                continue;
            }
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
                        && !fields.iter().any(|(alternate, field)| {
                            field["x-slot"] == fields[key]["x-slot"]
                                && object.contains_key(alternate)
                        })
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod choice_tests {
    use super::*;

    #[test]
    fn unclear_request_defaults_preserve_model_slots() {
        for (id, text, supplied) in [
            ("plate", "front plane and top plane", json!({})),
            ("plate", "front plane XY", json!({})),
            ("plate", "not on front plane", json!({})),
            ("plate", "front plane or right plane", json!({"plane":"XY"})),
            ("plate", "not a 3 by 2 hole grid", json!({"grid_columns":3})),
            ("enclosure", "not open top", json!({})),
            ("enclosure", "open top or bottom", json!({"open":"bottom"})),
            ("revolute_joint", "not 45 degrees", json!({})),
            ("revolute_joint", "45 degrees or a quarter turn", json!({})),
        ] {
            let mut args = supplied.clone();
            let schema = json!({"x-pilot-expander":id});
            let request = json!({"messages":[{"role":"user","content":text}]});
            assert!(
                request_defaults(&mut args, &schema, &request)
                    .unwrap()
                    .is_empty(),
                "{text}"
            );
            assert_eq!(args, supplied, "{text}");
        }
        for (id, text, field, expected) in [
            ("plate", "front plane", "plane", json!("XZ")),
            ("enclosure", "open top", "open", json!("top")),
            // A negation elsewhere in the request leaves a clear cue alone.
            (
                "plate",
                "a plate on the front plane with four holes, no chamfer on it",
                "plane",
                json!("XZ"),
            ),
            (
                "enclosure",
                "make a cover with an outer lip and no screw bosses",
                "open",
                json!("bottom"),
            ),
            (
                "revolute_joint",
                "do not move the base, then rotate the arm 45 degrees",
                "angle_deg",
                json!(45.),
            ),
            (
                "revolute_joint",
                "rotate 45 degrees",
                "angle_deg",
                json!(45.),
            ),
        ] {
            let mut args = json!({});
            let schema = json!({"x-pilot-expander":id});
            let req = |text| json!({"messages":[{"role":"user","content":text}]});
            assert_eq!(
                request_defaults(&mut args, &schema, &req(text))
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(args[field], expected);
            assert!(request_defaults(&mut json!({}), &schema, &req(&"x".repeat(65_537))).is_err());
        }
        let schema = json!({"x-pilot-expander":"enclosure","properties":{}});
        assert!(coerce(&mut json!({"lip":false,"lip_width":2}), &schema).is_err());
        assert!(coerce(&mut json!({"lip":true,"wall":-1}), &schema).is_err());
    }

    #[test]
    fn unique_choice_prefixes_and_trimmed_case_are_audited() {
        let schema = json!({"properties":{"plane":{"type":"string","enum":["XY","XZ","YZ"]}}});
        for input in [
            "XY:+Z",
            "XY plane",
            "XY(normal)",
            "XY-plane",
            "XY, +Z",
            " xy ",
        ] {
            let mut args = json!({"plane":input});
            assert_eq!(
                coerce(&mut args, &schema).unwrap(),
                vec![json!({"field":"plane","before":input,"after":"XY"})]
            );
            assert_eq!(args["plane"], "XY");
        }
        for input in ["XYZ", "XY1", "YX", "prefix XY", "XYé"] {
            let mut args = json!({"plane":input});
            assert!(coerce(&mut args, &schema).unwrap().is_empty());
            assert_eq!(args["plane"], input);
            assert!(crate::recipe_library::validate_structure(&args, &schema).is_err());
        }
        let mut args = json!({"plane":"XY"});
        assert!(coerce(&mut args, &schema).unwrap().is_empty());
    }

    #[test]
    fn ambiguous_choices_stay_invalid_and_exact_choices_survive() {
        for choices in [json!(["A", "A-B"]), json!(["XY", "xy"])] {
            let schema = json!({"properties":{"choice":{"type":"string","enum":choices}}});
            let input = if choices[0] == "A" {
                "A-B: detail"
            } else {
                "xY"
            };
            let mut args = json!({"choice":input});
            assert!(coerce(&mut args, &schema).unwrap().is_empty());
            assert_eq!(args["choice"], input);
            assert!(crate::recipe_library::validate_structure(&args, &schema).is_err());
            let mut exact = json!({"choice":choices[1]});
            assert!(coerce(&mut exact, &schema).unwrap().is_empty());
        }
    }

    #[test]
    fn referenced_pattern_choices_are_coerced_before_validation() {
        let schema = crate::planner::patterns::schema("grid_pattern").unwrap();
        let mut args = json!({"target":"body: whole solid","plane":"XY:+Z","rows":2,"columns":2,"spacing1":20,"spacing2":20});
        let changes = coerce(&mut args, &schema).unwrap();
        assert_eq!(changes.len(), 2);
        crate::recipe_library::validate_structure(&args, &schema).unwrap();
    }
}
