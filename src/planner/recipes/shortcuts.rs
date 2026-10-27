//! Typed inputs share recipe validation, expansion, targeting and expected state.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
enum Build {
    Profile {
        kind: String,
        #[serde(default = "xy")]
        plane: String,
        width: Option<f64>,
        height: Option<f64>,
        inside_width: Option<f64>,
        inside_height: Option<f64>,
        stem_height: Option<f64>,
        lip: Option<f64>,
        inside_fillet: Option<f64>,
        #[serde(default)]
        symmetric: bool,
        thickness: Option<f64>,
        web_thickness: Option<f64>,
        flange_thickness: Option<f64>,
        stem_offset: Option<f64>,
        depth: f64,
    },
    Plate {
        length: f64,
        width: f64,
        thickness: f64,
        #[serde(default = "xy")]
        plane: String,
        #[serde(default)]
        centred: bool,
        bore_diameter: Option<f64>,
        hole_diameter: Option<f64>,
        bolt_circle: Option<Circle>,
        grid: Option<Grid>,
        corners: Option<Corners>,
    },
    Enclosure {
        length: f64,
        width: f64,
        height: f64,
        wall: f64,
        #[serde(default = "top")]
        open: String,
        floor: Option<f64>,
        #[serde(default)]
        centred: bool,
        lip: Option<Lip>,
    },
    RevoluteJoint {
        pin_component: Option<String>,
        hole_component: Option<String>,
        #[serde(default)]
        angle_deg: f64,
    },
}
#[derive(Deserialize)]
struct Circle {
    count: usize,
    diameter: f64,
    #[serde(default)]
    start_deg: f64,
}
#[derive(Deserialize)]
struct Grid {
    rows: usize,
    columns: usize,
    pitch_x: f64,
    pitch_y: f64,
}
#[derive(Deserialize)]
struct Corners {
    kind: String,
    size: f64,
    #[serde(default = "vertical")]
    edges: String,
}
#[derive(Deserialize)]
struct Lip {
    width: f64,
    height: f64,
    #[serde(default = "inner")]
    side: String,
}
fn xy() -> String {
    "XY".into()
}
fn top() -> String {
    "top".into()
}
fn vertical() -> String {
    "vertical".into()
}
fn inner() -> String {
    "inner".into()
}

pub(crate) fn build(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    let schema = tool();
    let variants = schema["function"]["parameters"]["oneOf"]
        .as_array()
        .ok_or("shortcut schema missing")?;
    let variant = variants
        .iter()
        .find(|s| s["properties"]["shape"]["const"] == args["shape"])
        .ok_or_else(|| format!("shape {} must be 1 of profile, plate, enclosure, revolute_joint; choose the matching shape", args["shape"]))?;
    check(args, variant, "arguments")?;
    let mut normalized = args.clone();
    crate::recipe_library::forms::consistent(&mut normalized)?;
    let args = &normalized;
    let input: Build =
        serde_json::from_value(args.clone()).map_err(|e| format!("arguments: {e}"))?;
    let (rule, slots) = match input {
        Build::Profile {
            kind,
            plane,
            width,
            height,
            inside_width,
            inside_height,
            stem_height,
            lip,
            inside_fillet,
            symmetric,
            thickness,
            web_thickness,
            flange_thickness,
            stem_offset,
            depth,
        } => {
            let shape = if kind == "rectangle" { "R" } else { &kind };
            let (web, flange) = if shape == "R" {
                if thickness.is_some() || web_thickness.is_some() || flange_thickness.is_some() {
                    return Err(format!("thickness {} web_thickness {} flange_thickness {} allow 0 entries for rectangle; omit thickness fields and use width and height", args["thickness"], args["web_thickness"], args["flange_thickness"]));
                }
                (0.0, 0.0)
            } else {
                match (thickness, web_thickness, flange_thickness) {
                    (Some(t), None, None) => (t,t),
                    (None, Some(w), Some(f)) => (w,f),
                    _ => return Err(format!("thickness {} web_thickness {} flange_thickness {} require 1 uniform or 2 separate leg sizes; supply thickness alone or both web_thickness and flange_thickness", args["thickness"], args["web_thickness"], args["flange_thickness"])),
                }
            };
            let choose = |outside: Option<f64>,
                          inside: Option<f64>,
                          extra: f64,
                          field: &str|
             -> Result<f64, String> {
                match (outside, inside) {
                    (Some(v), None) => Ok(v),
                    (None, Some(v)) => {
                        positive_mm(field, v)?;
                        Ok(v + extra)
                    }
                    _ => Err(format!(
                        "{field}: supply exactly one outside or alternative size"
                    )),
                }
            };
            if (inside_width.is_some() || inside_height.is_some()) && !matches!(shape, "U" | "C") {
                return Err("inside_width/inside_height require U or C".into());
            }
            if stem_height.is_some() && (shape != "T" || inside_height.is_some()) {
                return Err("stem_height requires T and replaces height".into());
            }
            let width = choose(
                width,
                inside_width,
                if shape == "U" { 2.0 * flange } else { web },
                "width/inside_width",
            )?;
            let height = choose(
                height,
                inside_height.or(stem_height),
                if shape == "C" {
                    2.0 * flange
                } else if shape == "T" {
                    flange
                } else {
                    web
                },
                "height/inside_height/stem_height",
            )?;
            let slots = closed_profile::Profile {
                shape: shape.into(),
                plane,
                width,
                height,
                web,
                flange,
                inside_width,
                inside_height,
                stem_height,
                lip,
                inside_fillet,
                symmetric,
                offset: stem_offset,
                separate: web != flange,
                centred: false,
                flipped: false,
                distance: depth,
            }
            .into_slots()?;
            ("closed_profile", slots)
        }
        Build::Enclosure {
            length,
            width,
            height,
            wall,
            open,
            floor,
            centred,
            lip,
        } => {
            let (lip, outer) = lip.map_or((None, false), |l| {
                (Some([l.width, l.height]), l.side == "outer")
            });
            let slots = shell_box::ShellBox {
                size: [length, width, height],
                wall,
                floor: floor.unwrap_or(wall),
                bottom: open == "bottom",
                centred,
                lip,
                outer,
            }
            .into_slots()?;
            ("shell", slots)
        }
        Build::Plate {
            length,
            width,
            thickness,
            plane,
            centred,
            bore_diameter,
            hole_diameter,
            bolt_circle,
            grid,
            corners,
        } => {
            let mut holes = Vec::new();
            if let Some(d) = bore_diameter {
                positive_mm("bore_diameter", d)?;
                holes.push(([0.0, 0.0], d / 2.0));
            }
            if bolt_circle.is_some() || grid.is_some() {
                let d =
                    hole_diameter.ok_or("hole_diameter <missing> requires 1 diameter for bolt_circle or grid; supply the pattern hole diameter in mm")?;
                positive_mm("hole_diameter", d)?;
                if let Some(c) = bolt_circle {
                    count("bolt_circle.count", c.count)?;
                    positive_mm("bolt_circle.diameter", c.diameter)?;
                    revolute::validate_angle(c.start_deg).map_err(|_| {
                        format!("bolt_circle.start_deg {} must be within -360..360 degrees; use 0 for a start on +X", c.start_deg)
                    })?;
                    for k in 0..c.count {
                        let a = c.start_deg.to_radians() + 2.0 * PI * k as f64 / c.count as f64;
                        holes.push((
                            [c.diameter / 2.0 * a.cos(), c.diameter / 2.0 * a.sin()],
                            d / 2.0,
                        ));
                    }
                }
                if let Some(g) = grid {
                    count("grid.rows", g.rows)?;
                    count("grid.columns", g.columns)?;
                    if g.rows * g.columns > 64 {
                        return Err(format!("grid.rows * grid.columns {} * {} = {} must be at most 64 holes; reduce rows or columns", g.rows, g.columns, g.rows * g.columns));
                    }
                    positive_mm("grid.pitch_x", g.pitch_x)?;
                    positive_mm("grid.pitch_y", g.pitch_y)?;
                    for r in 0..g.rows {
                        for c in 0..g.columns {
                            holes.push((
                                [
                                    (c as f64 - (g.columns - 1) as f64 / 2.0) * g.pitch_x,
                                    (r as f64 - (g.rows - 1) as f64 / 2.0) * g.pitch_y,
                                ],
                                d / 2.0,
                            ));
                        }
                    }
                }
            } else if hole_diameter.is_some() {
                return Err(format!("hole_diameter {} requires 1 pattern; add bolt_circle or grid, or use bore_diameter for a centre hole", args["hole_diameter"]));
            }
            let rounding = corners.map(|c| (c.kind == "fillet", c.edges == "all", c.size));
            let plate = compound_plate::Plate {
                size: [length, width, thickness],
                plane,
                centred,
                holes,
                rounding,
                default_start: false,
            };
            plate.validate()?;
            ("compound_plate", Slots::CompoundPlate(plate))
        }
        Build::RevoluteJoint {
            pin_component,
            hole_component,
            angle_deg,
        } => {
            for (field, name) in [
                ("pin_component", &pin_component),
                ("hole_component", &hole_component),
            ] {
                if name.as_ref().is_some_and(|n| {
                    n.trim().is_empty() || n.len() > 128 || n.chars().any(char::is_control)
                }) {
                    return Err(format!("{field} {name:?} must contain 1..128 bytes and 0 control characters; use the exact inspected component name"));
                }
            }
            return revolute::typed(request,catalog,angle_deg,[pin_component.as_deref(),hole_component.as_deref()])?
                .ok_or_else(|| format!("revolute_joint: inspection must resolve exactly 1 contact pair for pin_component {pin_component:?} and hole_component {hole_component:?}; inspect and supply the matching component names"));
        }
    };
    let messages = request["messages"]
        .as_array()
        .ok_or("messages must be an array")?;
    from_slots(rule, slots, state(messages).as_ref(), catalog)?
        .ok_or_else(|| format!("shape {} with dimensions {args} exceeds capacity of {MAX_BODIES} bodies or {MAX_MM} mm; reduce sizes or remove bodies", args["shape"]))
}
fn count(field: &str, n: usize) -> Result<(), String> {
    if !compound_plate::bounded_count(n) {
        return Err(format!(
            "{field} {n} must be between 1 and 64; reduce the count to this range"
        ));
    }
    Ok(())
}
fn object(fields: &[(&str, Value)], required: &[&str]) -> Value {
    let properties: serde_json::Map<String, Value> = fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let mut schema = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    crate::recipe_library::forms::describe(&mut schema);
    schema
}
fn choice(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn check(value: &Value, schema: &Value, field: &str) -> Result<(), String> {
    let valid = match schema["type"].as_str() {
        Some("object") => value.is_object(),
        Some("string") => value.is_string(),
        Some("number") => value.as_f64().is_some_and(f64::is_finite),
        Some("integer") => value.as_u64().is_some(),
        Some("boolean") => value.is_boolean(),
        _ => true,
    };
    if !valid {
        return Err(format!(
            "{field} {value} requires 1 {} value; use that JSON type",
            schema["type"]
        ));
    }
    if schema.get("const").is_some_and(|c| c != value)
        || schema["enum"]
            .as_array()
            .is_some_and(|vs| !vs.contains(value))
    {
        return Err(format!(
            "{field} {value} must match 1 allowed choice from {}; use a listed value",
            schema
                .get("enum")
                .or_else(|| schema.get("const"))
                .unwrap_or(&Value::Null)
        ));
    }
    if let Some(v) = value.as_object() {
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !v.contains_key(key) {
                    return Err(format!(
                        "{field}.{key} <missing> requires 1 value; supply {key} from the request"
                    ));
                }
            }
        }
        for (key, v) in v {
            let s = schema["properties"]
                .get(key)
                .ok_or_else(|| format!("{field}.{key} {v} allows 0 unknown fields; remove {key} and use a listed field"))?;
            check(v, s, &format!("{field}.{key}"))?;
        }
    }
    Ok(())
}
pub(crate) fn tool() -> Value {
    let n = json!({"type":"number"});
    let i = json!({"type":"integer"});
    let b = json!({"type":"boolean"});
    let s = json!({"type":"string"});
    let plane = choice(&["XY", "XZ", "YZ"]);
    let profile = object(
        &[
            ("shape", json!({"const":"profile"})),
            ("kind", choice(&["L", "T", "U", "C", "rectangle"])),
            ("plane", plane.clone()),
            ("width", n.clone()),
            ("height", n.clone()),
            ("thickness", n.clone()),
            (
                "web_thickness",
                json!({"type":"number","description":"Vertical leg/stem/web; U bottom."}),
            ),
            (
                "flange_thickness",
                json!({"type":"number","description":"Horizontal leg/flange; U sides."}),
            ),
            (
                "stem_offset",
                json!({"type":"number","description":"T stem centre from left."}),
            ),
            ("inside_width", n.clone()),
            ("inside_height", n.clone()),
            ("stem_height", n.clone()),
            ("lip", n.clone()),
            ("inside_fillet", n.clone()),
            ("symmetric", b.clone()),
            ("depth", n.clone()),
        ],
        &["shape", "kind", "plane", "depth"],
    );
    let circle = object(
        &[
            ("count", i.clone()),
            ("diameter", n.clone()),
            ("start_deg", n.clone()),
        ],
        &["count", "diameter"],
    );
    let grid = object(
        &[
            ("rows", i.clone()),
            ("columns", i),
            ("pitch_x", n.clone()),
            ("pitch_y", n.clone()),
        ],
        &["rows", "columns", "pitch_x", "pitch_y"],
    );
    let corners = object(
        &[
            ("kind", choice(&["fillet", "chamfer"])),
            ("size", n.clone()),
            ("edges", choice(&["vertical", "all"])),
        ],
        &["kind", "size", "edges"],
    );
    let plate = object(
        &[
            ("shape", json!({"const":"plate"})),
            ("length", n.clone()),
            ("width", n.clone()),
            ("thickness", n.clone()),
            ("plane", plane),
            ("centred", b.clone()),
            ("bore_diameter", n.clone()),
            (
                "hole_diameter",
                json!({"type":"number","description":"Diameter of pattern holes."}),
            ),
            ("bolt_circle", circle),
            ("grid", grid),
            ("corners", corners),
        ],
        &["shape", "length", "width", "thickness", "plane", "centred"],
    );
    let lip = object(
        &[
            ("width", n.clone()),
            ("height", n.clone()),
            ("side", choice(&["inner", "outer"])),
        ],
        &["width", "height", "side"],
    );
    let enclosure = object(
        &[
            ("shape", json!({"const":"enclosure"})),
            ("length", n.clone()),
            ("width", n.clone()),
            ("height", n.clone()),
            ("wall", n.clone()),
            ("open", choice(&["top", "bottom"])),
            ("floor", n.clone()),
            ("centred", b),
            ("lip", lip),
        ],
        &[
            "shape", "length", "width", "height", "wall", "open", "centred",
        ],
    );
    let joint = object(
        &[
            ("shape", json!({"const":"revolute_joint"})),
            ("pin_component", s.clone()),
            ("hole_component", s),
            ("angle_deg", n),
        ],
        &["shape"],
    );
    json!({"type":"function","function":{"name":"pilot_build","description":"Build typed shapes in mm; omit unstated optionals; inspect joints first.","parameters":{"type":"object","oneOf":[profile,plate,enclosure,joint]}}})
}

/// The file ids select exactly one existing expander; arguments never become English.
pub(crate) fn file_build(
    id: &str,
    args: &Value,
    request: &Value,
    catalog: &Catalog,
) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(args, &crate::recipe_library::schema(id)?)?;
    if id == "shell" && args.get("body").is_some() {
        return existing_shell(args, request, catalog);
    }
    if matches!(id, "profile" | "plate" | "enclosure" | "revolute_joint") {
        let mut typed = args.clone();
        typed["shape"] = json!(id);
        return build(&typed, request, catalog);
    }
    #[derive(Deserialize)]
    #[serde(tag = "expander", deny_unknown_fields)]
    enum Primitive {
        #[serde(rename = "box")]
        Box {
            length: f64,
            width: f64,
            height: f64,
        },
        #[serde(rename = "cylinder")]
        Cylinder { radius: f64, height: f64 },
        #[serde(rename = "sphere")]
        Sphere { radius: f64 },
        #[serde(rename = "tube")]
        Tube {
            outer_radius: f64,
            inner_radius: f64,
            height: f64,
        },
        #[serde(rename = "fillet")]
        Fillet { body: String, amount: f64 },
        #[serde(rename = "chamfer")]
        Chamfer { body: String, amount: f64 },
        #[serde(rename = "move")]
        Move { body: String, delta: [f64; 3] },
        #[serde(rename = "mirror")]
        Mirror {
            body: String,
            plane: String,
            keep_original: bool,
        },
        #[serde(rename = "shell")]
        Shell {
            length: f64,
            width: f64,
            height: f64,
            wall: f64,
        },
        #[serde(rename = "revolve")]
        Revolve { radius: f64, height: f64 },
    }
    let mut typed = args.clone();
    typed["expander"] = json!(id);
    let p: Primitive = serde_json::from_value(typed).map_err(|e| format!("{id} arguments: {e}"))?;
    let slots = match p {
        Primitive::Box {
            length,
            width,
            height,
        } => Slots::Box {
            size: [length, width, height],
        },
        Primitive::Cylinder { radius, height } => Slots::Cylinder { radius, height },
        Primitive::Sphere { radius } => Slots::Sphere { radius },
        Primitive::Tube {
            outer_radius,
            inner_radius,
            height,
        } => {
            if inner_radius >= outer_radius {
                return Err(format!("inner_radius {inner_radius} must be below outer_radius {outer_radius}; reduce inner_radius"));
            }
            Slots::Tube {
                outer_radius,
                inner_radius,
                height,
            }
        }
        Primitive::Fillet { body, amount } | Primitive::Chamfer { body, amount } => Slots::Edges {
            body,
            amount,
            fillet: id == "fillet",
            top: false,
            edges: Vec::new(),
        },
        Primitive::Move { body, delta } => Slots::Move { body, delta },
        Primitive::Mirror {
            body,
            plane,
            keep_original,
        } => Slots::Mirror {
            body,
            plane,
            keep_original,
        },
        Primitive::Shell {
            length,
            width,
            height,
            wall,
        } => {
            if 2.0 * wall >= length.min(width) || wall >= height {
                return Err(format!(
                    "wall {wall} must be below {} (half length/width or height); reduce wall",
                    (length.min(width) / 2.0).min(height)
                ));
            }
            Slots::Shell {
                size: [length, width, height],
                wall,
            }
        }
        Primitive::Revolve { radius, height } => Slots::Revolve { radius, height },
    };
    let messages = request["messages"]
        .as_array()
        .ok_or("messages must be an array")?;
    // Rule strings are fixed, never borrowed from a file.
    let rule = match id {
        "box" => "box",
        "cylinder" => "cylinder",
        "sphere" => "sphere",
        "tube" => "tube",
        "fillet" => "fillet",
        "chamfer" => "chamfer",
        "move" => "move",
        "mirror" => "mirror",
        "shell" => "shell",
        "revolve" => "revolve",
        _ => return Err("unknown expander".into()),
    };
    from_slots(rule, slots, state(messages).as_ref(), catalog)?
        .ok_or_else(|| format!("args {args} exceeds capacity of {MAX_BODIES} bodies or {MAX_MM} mm; reduce sizes or remove bodies"))
}

fn existing_shell(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    if ["length", "width", "height"]
        .iter()
        .any(|k| args.get(k).is_some())
    {
        return Err("args.body with shell dimensions is ambiguous; omit dimensions to shell the existing body".into());
    }
    let messages = request["messages"]
        .as_array()
        .ok_or("shell requires messages")?;
    let Some(data) = state(messages) else {
        return Ok(Plan {
            rule: "resolve_body",
            calls: vec![call(catalog, "document.inspect", json!({"measure":true}))?],
            facts: None,
            slots: args.clone(),
            expected: None,
            continuation: None,
        });
    };
    let bodies = data["bodies"]
        .as_array()
        .ok_or("shell requires inspected bodies")?;
    let matches: Vec<_> = bodies
        .iter()
        .filter(|b| b["name"] == args["body"])
        .collect();
    let [body] = matches.as_slice() else {
        return Err("args.body must match exactly one inspected rectangular body".into());
    };
    let min = &body["bbox"]["min"];
    let max = &body["bbox"]["max"];
    let mut lo = [0.; 3];
    let mut hi = [0.; 3];
    for k in 0..3 {
        lo[k] = min[k].as_f64().ok_or("shell needs measured bbox minimum")?;
        hi[k] = max[k].as_f64().ok_or("shell needs measured bbox maximum")?;
        if !lo[k].is_finite()
            || !hi[k].is_finite()
            || lo[k].abs() > MAX_MM
            || hi[k].abs() > MAX_MM
            || hi[k] <= lo[k]
            || hi[k] - lo[k] > MAX_MM
        {
            return Err(
                "shell bounds must have positive dimensions and coordinates within 1000000 mm"
                    .into(),
            );
        }
    }
    let size: [f64; 3] = std::array::from_fn(|k| hi[k] - lo[k]);
    let before = body["volume_mm3"]
        .as_f64()
        .ok_or("shell needs measured body volume")?;
    if body["face_types"]["plane"] != 6 || (before - size.iter().product::<f64>()).abs() > 0.01 {
        return Err("args.body must be an axis-aligned rectangular box for this shell form".into());
    }
    let wall = args["wall"].as_f64().ok_or("shell needs wall thickness")?;
    if 2. * wall >= size[0].min(size[1]) || wall >= size[2] {
        return Err("args.wall must be below half the length/width and below the height".into());
    }
    let removed = (size[0] - 2. * wall) * (size[1] - 2. * wall) * (size[2] - wall);
    Ok(Plan {
        rule: "shell",
        slots: args.clone(),
        facts: None,
        continuation: None,
        calls: vec![
            call(
                catalog,
                "solid.shell",
                json!({"body":args["body"],"faces":[[(lo[0]+hi[0])/2.,(lo[1]+hi[1])/2.,hi[2]]],"thickness":wall,"direction":"inside"}),
            )?,
            call(catalog, "document.inspect", json!({"measure":true}))?,
        ],
        expected: Some(Expected {
            bodies: Some(bodies.len()),
            volume_mm3: Some(
                volume(&data).ok_or("shell needs measured document volume")? - removed,
            ),
            volume_range_mm3: None,
            name: None,
            created_volume_mm3: None,
            placement: None,
            feature: None,
            joint: None,
        }),
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    fn request(text: &str) -> Value {
        json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
    }
    fn run(args: &Value) -> Result<Plan, String> {
        build(
            args,
            &request("make the requested part"),
            &Catalog::bundled().unwrap(),
        )
    }
    fn profile(kind: &str) -> Value {
        json!({"shape":"profile","kind":kind,"plane":"XY","width":40,"height":30,"thickness":3,"depth":12})
    }
    fn plate() -> Value {
        json!({"shape":"plate","length":80,"width":80,"thickness":8,"plane":"XY","centred":true,"bore_diameter":10,"hole_diameter":4,"bolt_circle":{"count":6,"diameter":56,"start_deg":0},"corners":{"kind":"fillet","size":5,"edges":"vertical"}})
    }
    fn enclosure() -> Value {
        json!({"shape":"enclosure","length":80,"width":50,"height":30,"wall":2,"floor":4,"open":"top","centred":false,"lip":{"width":1,"height":2,"side":"inner"}})
    }
    fn bad(mut args: Value, path: &str, value: Value, field: &str) {
        *args.pointer_mut(path).unwrap() = value;
        let error = run(&args).unwrap_err();
        assert!(error.contains(field), "{path}: {error}");
    }
    #[test]
    fn typed_calls_equal_english_expansions() {
        let catalog = Catalog::bundled().unwrap();
        for (args,text) in [
            (profile("L"),"Make an L profile width 40 height 30 thickness 3 extrude 12"),
            ({let mut p=profile("T");p.as_object_mut().unwrap().remove("thickness");p["web_thickness"]=json!(6);p["flange_thickness"]=json!(4);p},"Make a T profile width 40 height 30 flange 4 stem 6 extrude 12"),
            (profile("U"),"Make a U profile width 40 height 30 thickness 3 extrude 12"),
            (profile("C"),"Make a C profile width 40 height 30 thickness 3 extrude 12"),
            ({let mut p=profile("rectangle");p.as_object_mut().unwrap().remove("thickness");p},"Make a fully constrained rectangle width 40 height 30 extrude 12"),
            (plate(),"Make a plate 80x80x8mm centred on the origin with a centred 10mm bore through and six 4mm holes evenly spaced on a 56mm bolt circle then round the four vertical corner edges radius 5mm"),
            (enclosure(),"Make an enclosure a box 80x50x30mm wall 2mm floor 4mm open top with one corner at the origin inner lip width 1mm height 2mm"),
        ] {
            let req=request(text);
            let english=crate::planner::plan_direct(&req,&catalog).unwrap().unwrap_or_else(||panic!("{text}"));
            let typed=build(&args,&req,&catalog).unwrap();
            assert_eq!(typed.calls,english.calls,"{text}");
            assert_eq!(serde_json::to_value(typed.expected).unwrap(),serde_json::to_value(english.expected).unwrap());
        }
    }
    #[test]
    fn profile_arguments_and_every_field() {
        for kind in ["L", "T", "U", "C"] {
            let p = profile(kind);
            assert!(run(&p).is_ok());
            for field in ["width", "height", "depth", "thickness"] {
                for value in [
                    json!(0),
                    json!(-1),
                    json!(1_000_001),
                    json!("NaN"),
                    Value::Null,
                ] {
                    bad(
                        p.clone(),
                        &format!("/{field}"),
                        value,
                        if field == "thickness" {
                            "thickness"
                        } else {
                            field
                        },
                    );
                }
            }
        }
        let mut p = profile("T");
        p.as_object_mut().unwrap().remove("thickness");
        p["web_thickness"] = json!(6);
        p["flange_thickness"] = json!(4);
        p["stem_offset"] = json!(18);
        assert!(run(&p).is_ok());
        for (field, v) in [
            ("web_thickness", 40),
            ("flange_thickness", 30),
            ("stem_offset", 2),
        ] {
            bad(p.clone(), &format!("/{field}"), json!(v), field);
        }
        bad(profile("L"), "/plane", json!("ZX"), "plane");
        bad(profile("L"), "/kind", json!("Z"), "kind");
        let mut p = profile("L");
        p["stem_offset"] = json!(10);
        assert!(run(&p).unwrap_err().contains("stem_offset"));
        for (kind, t) in [("L", 30), ("T", 30), ("U", 20), ("C", 15)] {
            bad(profile(kind), "/thickness", json!(t), "thickness");
        }
    }
    #[test]
    fn plate_arguments_and_every_field() {
        assert!(run(&plate()).is_ok());
        for field in [
            "length",
            "width",
            "thickness",
            "bore_diameter",
            "hole_diameter",
        ] {
            bad(plate(), &format!("/{field}"), json!(0), field);
        }
        for (path, field) in [
            ("/bolt_circle/count", "bolt_circle.count"),
            ("/bolt_circle/diameter", "bolt_circle.diameter"),
            ("/corners/size", "corners.size"),
        ] {
            bad(plate(), path, json!(0), field);
        }
        for (path, value, field) in [
            ("/plane", json!("ZX"), "plane"),
            ("/centred", json!(1), "centred"),
            ("/bolt_circle/count", json!(65), "bolt_circle.count"),
            (
                "/bolt_circle/start_deg",
                json!(361),
                "bolt_circle.start_deg",
            ),
            ("/corners/size", json!(40), "corners.size"),
            ("/corners/kind", json!("round"), "corners.kind"),
            ("/corners/edges", json!("top"), "corners.edges"),
        ] {
            bad(plate(), path, value, field);
        }
        bad(plate(), "/bore_diameter", json!(79), "holes");
        let mut p = plate();
        p.as_object_mut().unwrap().remove("bolt_circle");
        p.as_object_mut().unwrap().remove("bore_diameter");
        p["grid"] = json!({"rows":2,"columns":3,"pitch_x":20,"pitch_y":20});
        assert!(run(&p).is_ok());
        for field in ["rows", "columns", "pitch_x", "pitch_y"] {
            bad(
                p.clone(),
                &format!("/grid/{field}"),
                json!(0),
                &format!("grid.{field}"),
            );
        }
        bad(p.clone(), "/grid/pitch_x", json!(3), "holes");
        bad(p.clone(), "/grid/rows", json!(64), "grid");
        p["bore_diameter"] = json!(40);
        assert!(run(&p).unwrap_err().contains("holes"));
    }
    #[test]
    fn enclosure_arguments_and_every_field() {
        assert!(run(&enclosure()).is_ok());
        for field in ["length", "width", "height", "wall", "floor"] {
            bad(enclosure(), &format!("/{field}"), json!(0), field);
        }
        for field in ["width", "height"] {
            bad(
                enclosure(),
                &format!("/lip/{field}"),
                json!(0),
                &format!("lip.{field}"),
            );
        }
        for (path, value, field) in [
            ("/wall", json!(25), "wall"),
            ("/floor", json!(30), "floor"),
            ("/lip/width", json!(2), "lip.width"),
            ("/lip/side", json!("top"), "lip.side"),
            ("/open", json!("side"), "open"),
            ("/centred", json!("yes"), "centred"),
        ] {
            bad(enclosure(), path, value, field);
        }
    }
    #[test]
    fn joint_validation_and_inspection_first() {
        let p=run(&json!({"shape":"revolute_joint","pin_component":"Base","hole_component":"Arm","angle_deg":45})).unwrap();
        assert_eq!(p.rule, "resolve_joint");
        let english=crate::planner::plan_direct(&request("Connect Base and Arm with a revolute joint and swing Arm 45 degrees. Keep Base fixed and Arm upright."),&Catalog::bundled().unwrap()).unwrap().unwrap();
        assert_eq!(p.calls, english.calls);
        for value in [json!(361), json!(-361), json!("NaN")] {
            assert!(run(&json!({"shape":"revolute_joint","angle_deg":value}))
                .unwrap_err()
                .contains("angle_deg"));
        }
        for field in ["pin_component", "hole_component"] {
            assert!(run(&json!({"shape":"revolute_joint",field:""}))
                .unwrap_err()
                .contains(field));
        }
        assert!(run(&json!({"shape":"revolute_joint"})).is_ok());
        let req =
            crate::planner::cases::revolute_request("Connect Base and Arm with a revolute joint.");
        for field in ["pin_component", "hole_component"] {
            let args = json!({"shape":"revolute_joint",field:"Unknown component"});
            assert!(build(&args, &req, &Catalog::bundled().unwrap())
                .unwrap_err()
                .contains(field));
        }
    }
    #[test]
    fn strict_schema_and_prompt_budget() {
        fn descriptions(v: &Value) -> usize {
            match v {
                Value::Object(o) => o
                    .iter()
                    .map(|(k, v)| {
                        if k == "description" {
                            v.as_str().unwrap().chars().count()
                        } else {
                            descriptions(v)
                        }
                    })
                    .sum(),
                Value::Array(a) => a.iter().map(descriptions).sum(),
                _ => 0,
            }
        }
        let chars = descriptions(&tool());
        // Legacy union documents all four shapes; each forced form has its own
        // stricter 1,200-character schema + system-line budget.
        assert!(chars < 1400, "{chars} description characters");
        for mut a in [
            profile("L"),
            plate(),
            enclosure(),
            json!({"shape":"revolute_joint"}),
        ] {
            a["unexpected"] = json!(true);
            assert!(run(&a).unwrap_err().contains("unexpected"));
        }
        assert!(run(&json!({"shape":"other"}))
            .unwrap_err()
            .contains("shape"));
    }
}
