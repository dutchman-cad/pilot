//! Conservative request evidence for omissions in otherwise completed forms.
use serde_json::{json, Value};

fn scale(word: &str) -> Option<f64> {
    match word {
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => Some(1.),
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => Some(10.),
        "in" | "inch" | "inches" | "\"" => Some(25.4),
        _ => None,
    }
}

fn number(word: &str) -> Option<f64> {
    let n = if let Some((a, b)) = word.split_once('/') {
        a.parse::<f64>().ok()? / b.parse::<f64>().ok()?
    } else {
        word.parse::<f64>().ok()?
    };
    (n.is_finite() && n.abs() <= 1_000_000.).then_some(n)
}

// Keep punctuation boundaries, and split attached units such as 50mm.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut numeric = false;
    for c in text.chars() {
        let digit = c.is_ascii_digit() || matches!(c, '.' | '/' | '-' | '+');
        if c.is_ascii_alphabetic() || digit {
            if !current.is_empty() && numeric != digit {
                out.push(std::mem::take(&mut current));
            }
            current.push(c);
            numeric = digit;
        } else {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn length(words: &[String], i: usize) -> Option<(f64, usize)> {
    let n = number(words.get(i)?)?;
    if words.get(i + 1).is_some_and(|w| {
        matches!(
            w.as_str(),
            "m" | "metre" | "metres" | "meter" | "meters" | "ft" | "feet"
        )
    }) {
        return None;
    }
    let unit = words.get(i + 1).and_then(|s| scale(s));
    let n = n * unit.unwrap_or(1.);
    (n.is_finite() && n.abs() <= 1_000_000.).then_some((n, i + 1 + usize::from(unit.is_some())))
}

fn default(args: &mut Value, changes: &mut Vec<Value>, key: &str, value: f64, rule: &str) {
    args[key] = json!(value);
    changes.push(json!({"field":key,"before":null,"after":value,"defaulted":true,"rule":rule}));
}

pub(super) fn recover(
    args: &mut Value,
    schema: &Value,
    request: &Value,
) -> Result<Vec<Value>, String> {
    if !args.is_object() {
        return Err("args requires one object to recover omitted fields".into());
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
        return Err("omitted field request exceeds 65536 bytes".into());
    }
    if super::unclear_request(text) {
        return Ok(Vec::new());
    }
    let words = tokens(&text.to_ascii_lowercase());
    let mut changes = Vec::new();
    if schema["x-pilot-expander"] == "profile" && args.get("depth").is_none() {
        let mut unused = Vec::new();
        for i in 0..words.len() {
            if let Some((n, end)) = length(&words, i) {
                if n > 0.
                    && !args
                        .as_object()
                        .into_iter()
                        .flat_map(|o| o.values())
                        .any(|v| {
                            v.as_f64()
                                .is_some_and(|v| (v - n).abs() <= 1e-6_f64.max(n.abs() * 1e-9))
                        })
                {
                    unused.push((i, end, n));
                }
            }
        }
        if let [(i, end, n)] = unused.as_slice() {
            let extrusion = |s: &str| {
                matches!(
                    s,
                    "long" | "length" | "deep" | "depth" | "extrude" | "extruded" | "pad"
                ) || (s == "thick" && args.get("thickness").is_some())
            };
            if i.checked_sub(1)
                .and_then(|i| words.get(i))
                .is_some_and(|s| extrusion(s))
                || words.get(*end).is_some_and(|s| extrusion(s))
            {
                default(args, &mut changes, "depth", *n,
                    "single unused request length adjacent to extrusion word; lengths compared in mm");
            }
        }
    }
    if schema["x-pilot-expander"] == "plate" {
        plate(args, &words, &mut changes)?;
    }
    Ok(changes)
}

fn plate(args: &mut Value, words: &[String], changes: &mut Vec<Value>) -> Result<(), String> {
    let hole = args.get("hole_diameter").is_some();
    let boss = args.get("boss_diameter").is_some();
    if hole == boss
        || [
            "row",
            "grid",
            "bolt_circle",
            "row_count",
            "grid_rows",
            "grid_columns",
            "bolt_hole_count",
            "bolt_circle_diameter",
            "boss_offset_x",
            "boss_offset_y",
        ]
        .iter()
        .any(|k| args.get(*k).is_some())
    {
        return Ok(());
    }
    // Scope placement to the feature, never to a centred base plate.
    let feature_words = if hole {
        ["hole", "bore"]
    } else {
        ["boss", "peg"]
    };
    if words.iter().any(|w| {
        if hole {
            w == "holes"
        } else {
            w == "bosses" || w == "pegs"
        }
    }) {
        return Ok(());
    }
    let mentions: Vec<_> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| feature_words.contains(&w.as_str()))
        .collect();
    if mentions.len() > 1 {
        return Ok(());
    }
    let [(start, _)] = mentions.as_slice() else {
        return Ok(());
    };
    let tail = &words[start + 1..];
    if tail.iter().any(|w| {
        matches!(
            w.as_str(),
            "or" | "either" | "each" | "two" | "both" | "not" | "off" | "never"
        )
    }) {
        return Ok(());
    }
    let size = [
        args["length"]
            .as_f64()
            .or_else(|| args["diameter"].as_f64()),
        args["width"].as_f64().or_else(|| args["diameter"].as_f64()),
    ];
    let mut coordinates: [Option<f64>; 2] = [None, None];
    let prefix_start = words[..*start]
        .iter()
        .rposition(|w| w == "with")
        .map(|i| i + 1)
        .unwrap_or(*start);
    let mut centre = words[prefix_start..*start]
        .iter()
        .any(|s| matches!(s.as_str(), "centred" | "centered"))
        || start
            .checked_sub(1)
            .and_then(|i| words.get(i))
            .is_some_and(|s| matches!(s.as_str(), "centred" | "centered"));
    if centre
        && words[prefix_start..*start]
            .iter()
            .any(|s| matches!(s.as_str(), "not" | "off"))
    {
        return Ok(());
    }
    for i in 0..tail.len() {
        if matches!(
            tail[i].as_str(),
            "centred" | "centered" | "centre" | "center" | "middle"
        ) {
            if centre {
                return Ok(());
            }
            centre = true;
        }
        if matches!(tail[i].as_str(), "x" | "y")
            && (tail.get(i + 1).is_some_and(|s| s == "=")
                || i.checked_sub(1)
                    .and_then(|i| tail.get(i))
                    .is_some_and(|s| s == "at")
                || coordinates.iter().any(Option::is_some))
        {
            let axis = usize::from(tail[i] == "y");
            let at = i + 1 + usize::from(tail.get(i + 1).is_some_and(|s| s == "="));
            if let Some((n, _)) = length(tail, at) {
                let Some(size) = size[axis] else {
                    return Ok(());
                };
                if coordinates[axis].is_some() {
                    return Ok(());
                }
                coordinates[axis] = Some(
                    n - if args["centred"] == true {
                        0.
                    } else {
                        size / 2.
                    },
                );
            }
        }
        if let Some((n, end)) = length(tail, i) {
            // An additional bare value after a placement may be another
            // position. A second edge distance is handled below by axis.
            if (centre || coordinates.iter().any(Option::is_some))
                && i.checked_sub(1)
                    .and_then(|i| tail.get(i))
                    .is_some_and(|s| matches!(s.as_str(), "and" | ","))
                && tail.get(end).is_none_or(|s| s != "from")
            {
                return Ok(());
            }
            if tail.get(end).is_some_and(|w| w == "from") {
                let mut edge = end + 1;
                if tail.get(edge).is_some_and(|w| w == "the") {
                    edge += 1;
                }
                let Some(side) = tail.get(edge) else {
                    continue;
                };
                let axis = match side.as_str() {
                    "left" | "right" => 0,
                    "bottom" | "top" => 1,
                    _ => continue,
                };
                let Some(size) = size[axis] else {
                    return Ok(());
                };
                if coordinates[axis].is_some() {
                    return Ok(());
                }
                coordinates[axis] = Some(if matches!(side.as_str(), "left" | "bottom") {
                    n - size / 2.
                } else {
                    size / 2. - n
                });
            }
        }
    }
    if centre && coordinates.iter().any(Option::is_some) {
        return Ok(());
    }
    if !centre && coordinates.iter().all(Option::is_none) {
        return Ok(());
    }
    for (axis, key) in ["boss_offset_x", "boss_offset_y"].iter().enumerate() {
        default(args, changes, key, coordinates[axis].unwrap_or(0.),
            "single feature position in request; absolute coordinates and edge distances converted to plate-centre offsets");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_positions_abstain_without_partial_offsets() {
        let schema = json!({"x-pilot-expander":"plate"});
        for text in [
            "holes at X=40",
            "hole and bore at X=40",
            "hole at X=40 or X=60",
            "not centred hole",
            "hole not centred",
            "centred hole at centre",
            "hole at X=40 and X=60",
            "hole at X=40 and 60",
            "hole 20 from left and 30 from right",
            "hole at centre X=40",
            "hole at an offset of 20",
            "hole 20 from",
            "hole at either edge",
            "hole at each corner",
            "hole off centre",
            "hole at two positions",
            "hole at both sides",
            "hole never centred",
        ] {
            let original = json!({"length":100,"width":80,"thickness":6,"hole_diameter":8});
            let mut args = original.clone();
            let request = json!({"messages":[{"role":"user","content":text}]});
            assert!(
                recover(&mut args, &schema, &request).unwrap().is_empty(),
                "{text}"
            );
            assert_eq!(args, original, "{text}");
            // Explicit model offsets also survive uncertain request evidence.
            args["boss_offset_x"] = json!(12);
            let supplied = args.clone();
            assert!(recover(&mut args, &schema, &request).unwrap().is_empty());
            assert_eq!(args, supplied);
        }
    }

    #[test]
    fn clear_positions_and_depth_still_infer_and_bounds_still_reject() {
        for (id, text, original, expected) in [
            (
                "plate",
                "hole 20 from left",
                json!({"length":100,"width":80,"hole_diameter":8}),
                json!({"boss_offset_x":-30.,"boss_offset_y":0.}),
            ),
            (
                "plate",
                "hole at X=40 Y=30",
                json!({"length":100,"width":80,"hole_diameter":8}),
                json!({"boss_offset_x":-10.,"boss_offset_y":-10.}),
            ),
            (
                "plate",
                "centred hole",
                json!({"length":100,"width":80,"hole_diameter":8}),
                json!({"boss_offset_x":0.,"boss_offset_y":0.}),
            ),
            (
                "profile",
                "extrude 75 mm",
                json!({"width":40}),
                json!({"depth":75.}),
            ),
        ] {
            let schema = json!({"x-pilot-expander":id});
            let mut args = original;
            let request = json!({"messages":[{"role":"user","content":text}]});
            assert_eq!(
                recover(&mut args, &schema, &request).unwrap().len(),
                expected.as_object().unwrap().len()
            );
            for (key, value) in expected.as_object().unwrap() {
                assert_eq!(args[key], *value);
            }
            let too_large = json!({"messages":[{"role":"user","content":"x".repeat(65_537)}]});
            assert!(recover(&mut args, &schema, &too_large)
                .unwrap_err()
                .contains("65536"));
            assert!(recover(&mut json!(false), &schema, &request).is_err());
        }
        let mut args = json!({"width":40});
        let original = args.clone();
        assert!(recover(
            &mut args,
            &json!({"x-pilot-expander":"profile"}),
            &json!({"messages":[{"role":"user","content":"do not extrude 75 mm"}]})
        )
        .unwrap()
        .is_empty());
        assert_eq!(args, original);
    }
}
