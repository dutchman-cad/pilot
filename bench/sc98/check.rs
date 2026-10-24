//! State predicates for the SolidCraft 98 remote protocol.
use serde_json::{json, Map, Value};

pub fn rows(screen: &str) -> Result<Vec<Vec<String>>, String> {
    if screen.len() > 262144 {
        return Err("screen exceeds 256 KiB".into());
    }
    let rows: Vec<Vec<String>> = screen
        .lines()
        .map(|line| {
            line.split('|')
                .map(|p| {
                    p.trim()
                        .strip_suffix("<- selected")
                        .unwrap_or(p.trim())
                        .trim()
                        .to_owned()
                })
                .collect::<Vec<_>>()
        })
        .filter(|p| p.len() == 5 && p[0].parse::<u32>().is_ok())
        .collect();
    if rows.len() > 64 {
        return Err("timeline exceeds 64 features".into());
    }
    Ok(rows)
}
pub fn fields(screen: &str) -> Value {
    let mut fields = Map::new();
    for line in screen.lines() {
        if let Some((name, tail)) = line
            .trim()
            .strip_prefix("field \"")
            .and_then(|s| s.split_once("\": ["))
        {
            if let Some(n) = tail
                .split(']')
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|n| n.is_finite())
            {
                fields.insert(name.into(), json!(n));
            }
        }
    }
    Value::Object(fields)
}
fn near(a: &Value, n: f64) -> bool {
    a.as_f64().is_some_and(|v| (v - n).abs() <= 0.001)
}
fn values(a: &[Value], b: &Value) -> bool {
    b.as_array().is_some_and(|b| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| b.as_f64().is_some_and(|b| near(a, b)))
    })
}
pub fn geometry(s: &Value) -> Result<Value, String> {
    let rows = rows(s["screen"].as_str().unwrap_or(""))?;
    Ok(json!(rows
        .iter()
        .map(|r| json!({"kind":r[2],"fields":s["fields"][&r[1]]}))
        .collect::<Vec<_>>()))
}
pub fn score(
    check: &Value,
    summary: &Value,
    state: &Value,
    initial: &Value,
) -> Result<Value, String> {
    let screen = state["screen"].as_str().ok_or("missing screen")?;
    let rows = rows(screen)?;
    let mut reasons = Vec::new();
    if !summary["error"].is_null() || state["incomplete"] == true || screen.contains("Dialog \"") {
        reasons.push("driver error or incomplete state");
    }
    if rows.iter().any(|r| r[4] != "OK") {
        reasons.push("failed feature");
    }
    if check["changed_geometry"] == true {
        if rows.is_empty() && !screen.contains("Timeline: empty.") {
            reasons.push("missing empty-state evidence");
        }
        if geometry(state)? == geometry(initial)? {
            reasons.push("geometry did not change");
        }
        return Ok(json!({"pass":reasons.is_empty(),"reasons":reasons}));
    }
    let sketches: Vec<_> = rows
        .iter()
        .filter(|r| r[2].starts_with("Sketch ("))
        .collect();
    let solids: Vec<_> = rows
        .iter()
        .filter(|r| matches!(r[2].as_str(), "Extrude" | "Revolve"))
        .collect();
    let holes: Vec<_> = rows.iter().filter(|r| r[2] == "Hole").collect();
    let expected_holes = check["holes"].as_array().ok_or("check needs holes array")?;
    if sketches.len() != 1
        || solids.len() != 1
        || holes.len() != expected_holes.len()
        || rows.len() != 2 + expected_holes.len()
    {
        reasons.push("unexpected feature count");
    }
    let sketch = sketches
        .first()
        .map(|r| &state["fields"][&r[1]])
        .unwrap_or(&Value::Null);
    let solid = solids
        .first()
        .map(|r| &state["fields"][&r[1]])
        .unwrap_or(&Value::Null);
    let rectangle = check["shape"] == "rectangle";
    let (size, origin) = if rectangle {
        (
            vec![
                sketch["Width (mm)"].clone(),
                sketch["Height (mm)"].clone(),
                solid["Depth (mm)"].clone(),
            ],
            vec![
                sketch["Corner X (mm)"].clone(),
                sketch["Corner Y (mm)"].clone(),
            ],
        )
    } else {
        (
            vec![
                sketch["Radius (mm)"]
                    .as_f64()
                    .map(|n| json!(2.0 * n))
                    .unwrap_or(Value::Null),
                solid["Depth (mm)"].clone(),
            ],
            vec![
                sketch["Centre X (mm)"].clone(),
                sketch["Centre Y (mm)"].clone(),
            ],
        )
    };
    let expected_kind = if rectangle {
        "Sketch (Rectangle)"
    } else {
        "Sketch (Circle)"
    };
    if sketches.first().is_none_or(|r| r[2] != expected_kind)
        || solids.first().is_none_or(|r| r[2] != "Extrude")
    {
        reasons.push("unexpected feature kind");
    }
    if !values(&size, &check["size"]) || !values(&origin, &check["origin"]) {
        reasons.push("dimensions or origin differ");
    }
    if !near(&sketch["On feature # (0 = ground)"], 0.0)
        || !near(&sketch["Height above that (mm)"], 0.0)
    {
        reasons.push("sketch is not on the ground XY plane");
    }
    let mut actual: Vec<Vec<Value>> = holes
        .iter()
        .map(|r| {
            let p = &state["fields"][&r[1]];
            [
                "Diameter (mm)",
                "Depth (mm, 0 = through all)",
                "Centre X (mm)",
                "Centre Y (mm)",
            ]
            .iter()
            .map(|k| p[k].clone())
            .collect()
        })
        .collect();
    for h in expected_holes {
        if let Some(i) = actual.iter().position(|a| values(a, h)) {
            actual.remove(i);
        } else {
            reasons.push("hole size, depth, or position differs");
        }
    }
    if holes.iter().any(|r| !r[3].contains("on Extrude1 top face")) {
        reasons.push("hole is not on the plate top face");
    }
    let answer = summary["done_message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    let radius = sketch
        .get("Corner radius (mm)")
        .cloned()
        .unwrap_or(json!(0));
    let refusal = [
        "cannot",
        "can't",
        "unavailable",
        "unsupported",
        "unable",
        "not supported",
        "not available",
        "does not support",
    ]
    .iter()
    .any(|w| answer.contains(w));
    match check["unsupported"].as_str() {
        Some("solid-edge rounding") => {
            if !refusal
                || !["edge", "fillet", "round"]
                    .iter()
                    .any(|w| answer.contains(w))
            {
                reasons.push("needs solid-edge rounding limitation");
            }
            if !(near(&radius, 0.0)
                || near(&radius, 3.0)
                    && answer.contains("sketch")
                    && ["alternative", "instead", "corner"]
                        .iter()
                        .any(|w| answer.contains(w)))
            {
                reasons.push("unacknowledged sketch-corner alternative");
            }
        }
        Some("body renaming") => {
            if !refusal
                || !["renam", "body", "bodies", "bracket"]
                    .iter()
                    .any(|w| answer.contains(w))
            {
                reasons.push("needs body-renaming limitation");
            }
            if !near(&radius, 0.0) {
                reasons.push("rename changed geometry");
            }
        }
        _ => {
            if !near(&radius, 0.0) {
                reasons.push("unexpected rounded corners");
            }
        }
    }
    if check.get("calculated_volume_mm3").is_some() {
        // Require an unambiguous calculation with a unit. Reject competing values and negation.
        let words: Vec<_> = answer.split_whitespace().collect();
        let volumes: Vec<_> = words
            .windows(2)
            .filter_map(|w| {
                let n = w[0].replace(',', "").parse::<f64>().ok()?;
                match w[1].trim_end_matches(['.', ',']) {
                    "mm3" | "mm^3" | "mm³" => Some(n),
                    "cm3" | "cm^3" | "cm³" => Some(n * 1000.0),
                    _ => None,
                }
            })
            .collect();
        if !answer.contains("calculat")
            || volumes.is_empty()
            || volumes.iter().any(|n| (*n - 6000.0).abs() > 0.001)
            || ["not ", "isn't", "measur", "rather than", "instead of"]
                .iter()
                .any(|w| answer.contains(w))
        {
            reasons.push("needs an unambiguous calculated 6000 mm3 or 6 cm3");
        }
    }
    Ok(json!({"pass":reasons.is_empty(),"reasons":reasons}))
}
