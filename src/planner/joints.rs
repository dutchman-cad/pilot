//! Joint forms using only public catalogue commands and inspected geometry.
use super::*;

pub(crate) const TYPES: &[&str] = &[
    "rigid",
    "revolute",
    "slider",
    "cylindrical",
    "pin_slot",
    "planar",
    "ball",
    "parallel",
    "perpendicular",
    "angle",
    "point_on_plane",
    "point_on_line",
    "line_on_plane",
];

pub(crate) fn fields(s: &mut Value) {
    s["properties"]["joint_type"] = json!({"type":"string","enum":TYPES,"description":"Omit for contact pin/hole; otherwise select type. Limits: joint.list order."});
    for key in ["component_a", "component_b", "point_a", "point_b"] {
        s["properties"][key] = json!({"type":"string","minLength":1,"maxLength":128,"description":if key.starts_with("component") {"Inspected component name; omit only for the two remaining candidates"} else {"Named joint.list origin; omit for component box centre"}});
    }
    s["properties"]["axis"] = json!({"type":"string","enum":["X","Y","Z"],"description":"Aligned world axes; omitted: Z"});
    for i in 1..=5 {
        for end in ["min", "max"] {
            s["properties"][format!("limit{i}_{end}")] = json!({"type":"number","minimum":-MAX_MM,"maximum":MAX_MM,"description":if end == "max" { format!("Upper bound; units as limit{i}_min") } else { match i { 1 => "Motion 1: deg for revolute/cylindrical/pin_slot/ball; mm otherwise".into(), 2 => "Motion 2: deg for ball/point_on_line; otherwise mm".into(), 3 => "Motion 3: mm for parallel/perpendicular/angle; otherwise deg".into(), _ => format!("Motion {i}: deg") }}});
        }
    }
}

pub(crate) fn intent(text: &str) -> bool {
    let words = tokens(text);
    let named = TYPES.iter().any(|kind| {
        let kind = tokens(kind);
        !kind.is_empty() && words.windows(kind.len()).any(|w| w == kind.as_slice())
    }) || words.iter().any(|w| {
        matches!(
            w.as_str(),
            "hinge" | "pin" | "slide" | "prismatic" | "spherical"
        )
    });
    let standalone = TYPES.iter().any(|kind| words == tokens(kind))
        || matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "hinge" | "pin" | "slide" | "prismatic" | "spherical" | "ball and socket"
        );
    named
        && (standalone
            || words
                .iter()
                .any(|w| matches!(w.as_str(), "joint" | "connect" | "relation")))
}

pub(crate) fn uses_generic(args: &Value) -> bool {
    args.as_object().is_some_and(|o| {
        o.keys().any(|k| {
            k == "joint_type"
                || k == "axis"
                || k.starts_with("component_")
                || k.starts_with("point_")
                || k.starts_with("limit")
        })
    })
}

pub(crate) fn coerce(args: &mut Value) -> Result<Vec<Value>, String> {
    repair::check_value(args)?;
    let before = args.clone();
    if let Some(v) = args.get_mut("joint_type") {
        if let Some(s) = v.as_str().filter(|s| s.len() <= 128) {
            let s = s.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            let s = s.strip_suffix("_joint").unwrap_or(&s);
            *v = json!(match s {
                "hinge" | "pin" => "revolute",
                "prismatic" | "slide" => "slider",
                "ball_and_socket" | "ball_socket" | "spherical" => "ball",
                _ => s,
            });
        }
    }
    patterns::coerce(args)?;
    Ok(before
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, v)| args[*k] != **v)
        .map(|(k, v)| json!({"field":k,"before":v,"after":args[k]}))
        .collect())
}

fn limits(args: &Value, kind: &str) -> Result<Value, String> {
    let count = match kind {
        "revolute" | "slider" => 1,
        "cylindrical" | "pin_slot" => 2,
        "planar" | "ball" => 3,
        "parallel" | "point_on_line" | "line_on_plane" => 4,
        "perpendicular" | "angle" | "point_on_plane" => 5,
        _ => 0,
    };
    let mut out = Vec::new();
    for i in 1..=5 {
        let lo = &args[format!("limit{i}_min")];
        let hi = &args[format!("limit{i}_max")];
        if lo.is_null() && hi.is_null() {
            if i <= count {
                out.push(Value::Null);
            }
            continue;
        }
        let pair = lo.as_f64().zip(hi.as_f64()).filter(|(a, b)| {
            a.is_finite() && b.is_finite() && a.abs() <= MAX_MM && b.abs() <= MAX_MM && a <= b
        });
        let (lo, hi) = pair.ok_or_else(|| format!("limit{i}_min and limit{i}_max require two finite numbers within ±1000000, min <= max"))?;
        if i > count {
            return Err(format!(
                "limit{i}_min: {kind} has only {count} motion values"
            ));
        }
        out.push(json!([lo, hi]));
    }
    Ok(json!(out))
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn resolve<'a>(
    name: Option<&str>,
    candidates: &[&'a Value],
    excluded: Option<&Value>,
    field: &str,
) -> Result<&'a Value, String> {
    let available: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|c| excluded.is_none_or(|e| c["id"] != e["id"]))
        .collect();
    if let Some(name) = name {
        let exact: Vec<_> = available
            .iter()
            .copied()
            .filter(|c| c["name"] == name)
            .collect();
        if let [c] = exact.as_slice() {
            return Ok(c);
        }
        let query = normalize(name);
        let normalized: Vec<_> = available
            .iter()
            .copied()
            .filter(|c| c["name"].as_str().is_some_and(|n| normalize(n) == query))
            .collect();
        if let [c] = normalized.as_slice() {
            return Ok(c);
        }
        if normalized.is_empty() && !query.is_empty() {
            let near: Vec<_> = available
                .iter()
                .copied()
                .filter(|c| {
                    c["name"].as_str().is_some_and(|n| {
                        normalize(n).contains(&query) || query.contains(&normalize(n))
                    })
                })
                .collect();
            if let [c] = near.as_slice() {
                return Ok(c);
            }
        }
    } else if let [c] = available.as_slice() {
        return Ok(c);
    }
    Err(format!(
        "args.{field} {name:?} is ambiguous or unknown; use one inspected component name"
    ))
}

pub(crate) fn build(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(
        args,
        &crate::recipe_library::schema("revolute_joint")?,
    )?;
    let kind = args["joint_type"]
        .as_str()
        .filter(|s| TYPES.contains(s))
        .ok_or("joint_type requires a listed joint type")?;
    let limits = limits(args, kind)?;
    if ["pin_component", "hole_component", "angle_deg"]
        .iter()
        .any(|k| args.get(*k).is_some())
    {
        return Err("joint_type uses component_a/component_b and limit pairs; omit legacy pin_component, hole_component and angle_deg".into());
    }
    let messages = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .map_or(0, |i| i + 1);
    let data: Vec<_> = messages[start..]
        .iter()
        .take(4096)
        .filter_map(tool_payload)
        .collect();
    let components = data
        .iter()
        .rev()
        .find_map(|d| d["components"].as_array())
        .filter(|a| a.len() <= 64);
    let doc = data.iter().rev().find(|d| d["bodies"].is_array());
    let (Some(components), Some(doc)) = (components, doc) else {
        return Err(
            "revolute_joint: inspection must resolve components and body bounds first".into(),
        );
    };
    if data
        .iter()
        .rev()
        .find(|d| d["components"].is_array())
        .is_some_and(|d| d["pending_moves"].as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err(
            "apply pending component moves and inspect again before creating a joint".into(),
        );
    }
    let candidates: Vec<_> = components.iter().filter(|c| c["id"] != 0).collect();
    if candidates.iter().any(|c| {
        c["id"].as_u64().is_none()
            || c["name"]
                .as_str()
                .is_none_or(|n| n.is_empty() || n.len() > 128 || n.chars().any(char::is_control))
    }) {
        return Err(
            "component.list requires bounded component names and numeric ids; inspect again".into(),
        );
    }
    let mut a_name = args["component_a"].as_str();
    let mut b_name = args["component_b"].as_str();
    if candidates.len() == 2 && a_name.is_none() && b_name.is_none() {
        a_name = candidates[0]["name"].as_str();
        b_name = candidates[1]["name"].as_str();
    }
    let (a, b) = if a_name.is_none() && b_name.is_some() {
        let b = resolve(b_name, &candidates, None, "component_b")?;
        (resolve(a_name, &candidates, Some(b), "component_a")?, b)
    } else {
        let a = resolve(a_name, &candidates, None, "component_a")?;
        (a, resolve(b_name, &candidates, Some(a), "component_b")?)
    };
    let axis = match args["axis"].as_str().unwrap_or("Z") {
        "X" => [1, 0, 0],
        "Y" => [0, 1, 0],
        "Z" => [0, 0, 1],
        _ => return Err("axis requires X, Y or Z".into()),
    };
    let origin = |c: &Value, field: &str| -> Result<Value, String> {
        let os = c["occurrences"]
            .as_array()
            .filter(|a| a.len() == 1)
            .ok_or("component requires exactly one inspected occurrence")?;
        let occurrence = os[0]["name"]
            .as_str()
            .filter(|n| !n.is_empty() && n.len() <= 128 && !n.chars().any(char::is_control))
            .ok_or("occurrence requires inspected name")?;
        if let Some(name) = args[field].as_str() {
            let origins = data
                .iter()
                .rev()
                .find_map(|d| d["origins"].as_array())
                .filter(|o| o.len() <= 128)
                .ok_or("named point requires joint.list origins")?;
            let matches: Vec<_> = origins
                .iter()
                .filter(|o| {
                    o["name"] == name
                        && (o["occurrence"] == os[0]["id"] || o["occurrence"] == occurrence)
                })
                .collect();
            if matches.len() != 1 {
                return Err(format!(
                    "{field} requires one named joint origin belonging to {occurrence}"
                ));
            }
            return Ok(json!(name));
        }
        let names = c["bodies"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
            .ok_or("component needs 1..64 inspected bodies")?;
        let bodies = doc["bodies"]
            .as_array()
            .filter(|a| a.len() <= MAX_BODIES)
            .ok_or("inspect body bounds first")?;
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for name in names {
            let body = bodies
                .iter()
                .find(|b| b["name"] == *name)
                .ok_or("component body missing from inspection")?;
            for i in 0..3 {
                let min = body["bbox"]["min"][i]
                    .as_f64()
                    .filter(|v| v.is_finite() && v.abs() <= MAX_MM)
                    .ok_or("body requires bounded box minimum")?;
                let max = body["bbox"]["max"][i]
                    .as_f64()
                    .filter(|v| v.is_finite() && v.abs() <= MAX_MM && *v >= min)
                    .ok_or("body requires bounded box maximum")?;
                lo[i] = lo[i].min(min);
                hi[i] = hi[i].max(max);
            }
        }
        let point: [f64; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.);
        Ok(json!({"occurrence":occurrence,"point":point,"z":axis}))
    };
    Ok(Plan {
        rule: "revolute_joint",
        calls: vec![
            call(
                catalog,
                "joint.create",
                json!({"type":kind,"a":origin(a,"point_a")?,"b":origin(b,"point_b")?,"flip":true,"limits":limits}),
            )?,
            call(catalog, "joint.list", json!({}))?,
            call(catalog, "document.inspect", json!({"measure":true}))?,
        ],
        slots: args.clone(),
        facts: None,
        expected: None,
        continuation: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"messages":[{"role":"user","content":"Connect the two parts"},{"role":"tool","content":json!({"bodies":[{"name":"Base","bbox":{"min":[0,0,0],"max":[10,20,30]}},{"name":"Link","bbox":{"min":[40,0,0],"max":[50,20,30]}}]}).to_string()},{"role":"tool","content":json!({"pending_moves":[],"components":[{"id":1,"name":"Base","bodies":["Base"],"occurrences":[{"id":2,"name":"Base:1"}]},{"id":3,"name":"Link","bodies":["Link"],"occurrences":[{"id":4,"name":"Link:1"}]}]}).to_string()}]})
    }
    #[test]
    fn names_defaults_sparse_limits_and_freshness() {
        let cat = Catalog::bundled().unwrap();
        let req = request();
        let args = json!({"joint_type":"ball","limit3_min":-30,"limit3_max":40});
        let p = build(&args, &req, &cat).unwrap();
        let params = &p.calls[0]["arguments"]["params"];
        assert_eq!(params["a"]["occurrence"], "Base:1");
        assert_eq!(params["b"]["occurrence"], "Link:1");
        assert_eq!(params["a"]["point"], json!([5., 10., 15.]));
        assert_eq!(params["limits"], json!([null, null, [-30., 40.]]));
        for bad in [
            json!({"joint_type":"ball","component_a":"unknown"}),
            json!({"joint_type":"ball","component_a":"Base","component_b":"Base"}),
            json!({"joint_type":"ball","point_a":"Missing"}),
        ] {
            assert!(build(&bad, &req, &cat).is_err());
        }
        let p = build(
            &json!({"joint_type":"slider","component_b":"Base"}),
            &req,
            &cat,
        )
        .unwrap();
        assert_eq!(
            p.calls[0]["arguments"]["params"]["a"]["occurrence"],
            "Link:1"
        );
        let mut stale = req;
        stale["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"new task"}));
        assert!(build(&args, &stale, &cat).is_err());
    }
    #[test]
    fn model_words_and_vectors_coerce_before_validation() {
        for (word, kind) in [
            ("Pin", "revolute"),
            ("hinge joint", "revolute"),
            ("slide", "slider"),
            ("Prismatic", "slider"),
            ("BALL AND SOCKET", "ball"),
            ("spherical", "ball"),
            ("pin-slot", "pin_slot"),
        ] {
            for axis in [
                json!("Z axis"),
                json!([0, 0, 1]),
                json!("0, 0, 1"),
                json!({"z":1}),
            ] {
                let mut args = json!({"joint_type":word,"axis":axis});
                assert!(!coerce(&mut args).unwrap().is_empty());
                assert_eq!(args["joint_type"], kind);
                assert_eq!(args["axis"], "Z");
            }
        }
        assert!(coerce(&mut json!({"axis":[1,1,0]})).is_err());
        assert!(coerce(&mut json!({"axis":"negative Z"})).is_err());
    }
}
