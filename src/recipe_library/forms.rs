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
    walk(s, s, "")
}

pub fn line(id: &str) -> String {
    format!("{id}: user words; inches to mm; omit unstated; decline on mismatch.")
}
