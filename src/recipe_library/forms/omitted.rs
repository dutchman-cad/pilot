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
    let words = tokens(&text.to_ascii_lowercase());
    let mut changes = Vec::new();
    if schema["x-pilot-expander"] == "profile" {
        profile(args, &words, &mut changes)?;
    }
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

fn profile(args: &mut Value, words: &[String], changes: &mut Vec<Value>) -> Result<(), String> {
    if args.get("plane").is_none() {
        let mut planes = Vec::new();
        for (i, word) in words.iter().enumerate() {
            let plane = match word.as_str() {
                "xy" => Some("XY"),
                "xz" => Some("XZ"),
                "yz" => Some("YZ"),
                "top" if words.get(i + 1).is_some_and(|s| s == "plane") => Some("XY"),
                "front" if words.get(i + 1).is_some_and(|s| s == "plane") => Some("XZ"),
                "right" if words.get(i + 1).is_some_and(|s| s == "plane") => Some("YZ"),
                _ => None,
            };
            if let Some(plane) = plane {
                if !planes.contains(&plane) {
                    planes.push(plane);
                }
            }
        }
        if planes.len() > 1
            || (!planes.is_empty()
                && words
                    .iter()
                    .any(|s| matches!(s.as_str(), "not" | "without" | "or" | "either")))
        {
            // Ambiguous wording infers nothing; the fill keeps its own plane.
        } else if let Some(plane) = planes.first() {
            args["plane"] = json!(plane);
            changes.push(json!({"field":"plane","before":null,"after":plane,"defaulted":true,"rule":"explicit sketch plane in request"}));
        }
    }
    if args["kind"] != "T" || args.get("stem_offset").is_some() {
        return Ok(());
    }
    let offsets: Vec<_> = words
        .iter()
        .enumerate()
        .filter(|(_, s)| *s == "offset")
        .collect();
    let Some((i, _)) = offsets.first() else {
        return Ok(());
    };
    // Only one clear "offset N from the left/centre" is inferred; anything else
    // leaves the fill as the model wrote it.
    if offsets.len() != 1
        || words
            .iter()
            .any(|s| matches!(s.as_str(), "not" | "without" | "or" | "either"))
    {
        return Ok(());
    }
    let Some((value, end)) = length(words, i + 1) else {
        return Ok(());
    };
    let Some(mut tail) = words[end..].strip_prefix(&["from".into()]) else {
        return Ok(());
    };
    if tail.first().is_some_and(|s| s == "the") {
        tail = &tail[1..];
    }
    let offset = match tail.first().map(String::as_str) {
        Some("left") => value,
        Some("centre" | "center") => match args["width"].as_f64() {
            Some(width) => width / 2. + value,
            None => return Ok(()),
        },
        _ => return Ok(()),
    };
    if !offset.is_finite() || !(0. ..=1_000_000.).contains(&offset) {
        return Ok(());
    }
    default(
        args,
        changes,
        "stem_offset",
        offset,
        "explicit stem centre offset; converted to distance from left",
    );
    Ok(())
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
        return Err(
            "single feature requires one position; supply an explicit layout for multiple features"
                .into(),
        );
    }
    let mentions: Vec<_> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| feature_words.contains(&w.as_str()))
        .collect();
    let ambiguous = || {
        "single feature position is ambiguous; supply one position or an explicit layout"
            .to_string()
    };
    if mentions.len() > 1 {
        return Err(ambiguous());
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
        return Err(ambiguous());
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
        return Err(ambiguous());
    }
    for i in 0..tail.len() {
        if matches!(
            tail[i].as_str(),
            "centred" | "centered" | "centre" | "center" | "middle"
        ) {
            if centre {
                return Err(ambiguous());
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
                    return Err(ambiguous());
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
                return Err(ambiguous());
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
                    return Err(ambiguous());
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
        return Err(ambiguous());
    }
    if !centre && coordinates.iter().all(Option::is_none) {
        if tail.iter().any(|s| matches!(s.as_str(), "at" | "from")) {
            return Err(ambiguous());
        }
        return Ok(());
    }
    for (axis, key) in ["boss_offset_x", "boss_offset_y"].iter().enumerate() {
        default(args, changes, key, coordinates[axis].unwrap_or(0.),
            "single feature position in request; absolute coordinates and edge distances converted to plate-centre offsets");
    }
    Ok(())
}
