//! Named edits use public timeline definitions and certified, feature-owned faces only.
use super::*;

pub fn schema() -> Value {
    json!({"type":"object","additionalProperties":false,"x-pilot-expander":"named_feature",
        "properties":{"feature":{"type":"string","minLength":1,"maxLength":128},
        "parameter":{"type":"string","description":"Inspected dimension or world coordinate","enum":["depth","height","width","length","diameter","x","y","z","start_offset"]},
        "mode":{"type":"string","description":"set: absolute mm; change: signed relative mm","enum":["set","change"]},"value":{"type":"number","minimum":-1000000,"maximum":1000000}},
        "required":["feature","parameter","mode","value"]})
}
fn words(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
pub fn candidate(text: &str) -> bool {
    if text.len() > 2048 {
        return false;
    }
    let w = words(text);
    let has = |s: &str| w.iter().any(|v| v == s);
    // A capital A after a word is a name letter ("Hole A"), not an article.
    let original: Vec<_> = text
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter(|s| !s.is_empty())
        .collect();
    let article = original.iter().enumerate().any(|(i, v)| {
        matches!(*v, "a" | "an")
            || (matches!(*v, "A" | "An")
                && (i == 0 || original[i - 1].eq_ignore_ascii_case("make")))
    });
    // A number after a capitalised word names a feature ("Pad 1"), not a size.
    let numbers = original
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            v.parse::<f64>().is_ok()
                && !(*i > 1 && original[i - 1].starts_with(|c: char| c.is_ascii_uppercase()))
        })
        .count();
    let edit = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "raise"
                | "lower"
                | "deepen"
                | "widen"
                | "shift"
                | "move"
                | "resize"
                | "make"
                | "change"
                | "increase"
                | "reduce"
                | "decrease"
                | "set"
                | "extend"
                | "trim"
                | "thicken"
                | "taller"
                | "shorter"
                | "deeper"
                | "shallower"
                | "wider"
                | "narrower"
                | "bigger"
                | "smaller"
                | "higher"
                | "thicker"
                | "thinner"
                | "adjust"
                | "alter"
                | "grow"
                | "shrink"
                | "lengthen"
                | "shorten"
                | "broaden"
        )
    }) || lower_phrase(&w, &["should", "be"])
        || lower_phrase(&w, &["needs", "to", "be"])
        || lower_phrase(&w, &["up", "by"])
        || lower_phrase(&w, &["down", "to"]);
    let feature_move = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "hole"
                | "pocket"
                | "recess"
                | "slot"
                | "pad"
                | "boss"
                | "rib"
                | "tab"
                | "counterbore"
                | "countersink"
        )
    });
    edit && !(has("move") && has("offset") && !feature_move)
        && !(has("move")
            && (text.contains(',')
                || slots::tokens(text)
                    .iter()
                    .filter(|v| slots::number(v).is_some())
                    .count()
                    >= 3))
        && (!has("make") || has("the") || has("this") || numbers <= 1)
        && !(has("make")
            && !has("the")
            && !has("this")
            && !has("existing")
            && !w.iter().any(|v| {
                matches!(
                    v.as_str(),
                    "height"
                        | "depth"
                        | "width"
                        | "length"
                        | "diameter"
                        | "distance"
                        | "across"
                        | "tall"
                        | "high"
                        | "thick"
                        | "deep"
                        | "thicker"
                        | "thinner"
                        | "taller"
                        | "shorter"
                        | "deeper"
                        | "shallower"
                        | "wider"
                        | "narrower"
                        | "bigger"
                        | "smaller"
                        | "protrude"
                        | "protrudes"
                        | "stand"
                        | "stands"
                        | "stick"
                        | "sticks"
                )
            }))
        && slots::tokens(text)
            .iter()
            .any(|v| slots::number(v).is_some())
        && !(has("make") && article)
        && !lower_phrase(&w, &["whole", "body"])
        && !lower_phrase(&w, &["move", "it"])
        && !w.iter().any(|v| {
            matches!(
                v.as_str(),
                "add"
                    | "create"
                    | "new"
                    | "another"
                    | "copy"
                    | "copies"
                    | "row"
                    | "repeat"
                    | "pattern"
                    | "mirror"
                    | "fillet"
                    | "chamfer"
                    | "shell"
                    | "not"
                    | "no"
                    | "never"
                    | "don"
                    | "dont"
                    | "cannot"
                    | "without"
                    | "what"
                    | "which"
                    | "why"
                    | "explain"
                    | "describe"
                    | "how"
                    | "and"
                    | "then"
                    | "all"
                    | "each"
                    | "two"
                    | "both"
            )
        })
}
fn lower_phrase(words: &[String], phrase: &[&str]) -> bool {
    words
        .windows(phrase.len())
        .any(|w| w.iter().zip(phrase).all(|(a, b)| a == b))
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| {
            v.as_str()
                .and_then(|s| s.trim().strip_suffix(" mm").unwrap_or(s).parse().ok())
        })
        .filter(|v| v.is_finite() && v.abs() <= MAX_MM)
}
fn volume(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite() && *v >= 0.)
}
fn point(v: &Value) -> Option<[f64; 3]> {
    Some([number(&v[0])?, number(&v[1])?, number(&v[2])?])
}
fn axis(v: [f64; 3]) -> Option<(usize, f64)> {
    (0..3)
        .find(|i| v[*i].abs() > 0.999999 && (0..3).filter(|j| j != i).all(|j| v[j].abs() < 1e-6))
        .map(|i| (i, v[i].signum()))
}
#[derive(Clone)]
struct Feature {
    name: String,
    body: String,
    id: u64,
    definition: Value,
    kind: String,
    type_name: String,
    faces: Vec<Value>,
    direction: Option<(usize, f64)>,
    context_faces: Vec<Value>,
    volume: Option<f64>,
    bbox: Value,
    // The feature and face its sketch is attached to, from the sketch's own definition.
    anchor: Option<(u64, String)>,
    // Area of later levels attached to the terminal face; they move with it.
    nested: Option<f64>,
}
// Normal of a face attachment's frame, when it lies along a world axis.
fn frame_normal(plane: &Value) -> Option<(usize, f64)> {
    let x = point(&plane["x_dir"])?;
    let y = point(&plane["y_dir"])?;
    axis([
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ])
}
// The certified end face of an extrusion, normal to its direction.
fn terminal(f: &Feature) -> Option<&Value> {
    let (a, _) = f.direction?;
    f.faces.iter().find(|v| {
        v["name"]
            .as_str()
            .is_some_and(|n| n.ends_with(":bottom") || n.ends_with(":top"))
            && point(&v["plane_normal"])
                .and_then(axis)
                .is_some_and(|(i, _)| i == a)
    })
}
// A straight extrusion's profile area: its end face plus the levels that fill part of it.
fn footprint(f: &Feature) -> Option<f64> {
    let straight = [&f.definition["taper"], &f.definition["extent"]["taper"]]
        .iter()
        .all(|t| t.is_null() || number(t) == Some(0.));
    if f.type_name != "ExtrudeFeature" || !straight {
        return None;
    }
    Some(number(&terminal(f)?["area"])? + f.nested?)
}
fn features(request: &Value) -> Vec<Feature> {
    let data = form_context::payloads(request);
    let Some(doc) = form_context::document(&data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // A partial timeline is never enough to claim a unique target.
    if doc["timeline"].as_array().is_none_or(|a| a.len() > 64) {
        return out;
    }
    if doc["bodies"].as_array().is_none_or(|a| a.len() > 12) {
        return out;
    }
    for f in doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|f| f["suppressed"] != true && f["rolled_back"] != true && f["error"].is_null())
    {
        let Some(d) = form_context::detail(&data, f) else {
            continue;
        };
        let Some(id) = f["id"].as_u64() else { continue };
        let Some(name) = f["name"].as_str().filter(|s| s.len() <= 128) else {
            continue;
        };
        let def = &d["definition"];
        let kind = match f["type"].as_str() {
            Some("HoleFeature") => match def["hole"]["type"].as_str() {
                Some("counterbore") => "counterbore",
                Some("countersink") => "countersink",
                _ => "hole",
            },
            Some("ExtrudeFeature" | "BoxFeature" | "CylinderFeature") => {
                if def["operation"] == "cut" {
                    "pocket"
                } else {
                    "pad"
                }
            }
            _ => continue,
        };
        let prefix = format!("F{id}:");
        let owned: Vec<_> = data
            .iter()
            .filter(|v| v["faces"].as_array().is_some_and(|a| a.len() <= 2048))
            .filter_map(|v| {
                let faces: Vec<_> = v["faces"]
                    .as_array()?
                    .iter()
                    .filter(|f| {
                        f["exact"] == true
                            && f["name"].as_str().is_some_and(|n| n.starts_with(&prefix))
                    })
                    .cloned()
                    .collect();
                (!faces.is_empty()).then(|| (v["body"].as_str().unwrap_or("").to_string(), faces))
            })
            .collect();
        // Use latest face read per body, without treating repeated inspections as ambiguity.
        let mut bodies = std::collections::BTreeMap::new();
        for (b, faces) in owned {
            bodies.insert(b, faces);
        }
        if bodies.len() != 1 {
            continue;
        }
        let Some((body, faces)) = bodies.into_iter().next() else {
            continue;
        };
        if body.is_empty() || body.len() > 128 {
            continue;
        }
        let plane = def["sketch"]
            .as_u64()
            .and_then(|id| {
                data.iter()
                    .rev()
                    .find(|s| s["id"] == id && s["definition"].is_object())
            })
            .map(|s| &s["definition"]["plane"]);
        let anchor = plane.filter(|p| p["type"] == "face").and_then(|p| {
            let target = &p["anchor"]["target"];
            Some((
                target["origin_feature"].as_u64()?,
                target["name"]
                    .as_str()
                    .filter(|n| n.len() <= 128)?
                    .to_string(),
            ))
        });
        let direction = if f["type"] == "HoleFeature" {
            point(&def["direction"]).and_then(axis)
        } else if f["type"] == "CylinderFeature" {
            point(&def["axis"]).and_then(axis)
        } else if f["type"] == "BoxFeature" {
            Some((2, 1.))
        } else {
            plane.and_then(|p| {
                let base = if p["type"] == "offset" { &p["base"] } else { p };
                let (a, normal) = match base["name"].as_str() {
                    Some("XY") => (2, 1.),
                    Some("XZ") => (1, 1.),
                    Some("YZ") => (0, 1.),
                    // A face attachment carries its frame; positive is the outward normal.
                    _ if p["type"] == "face" => frame_normal(&p["plane"])?,
                    _ => return None,
                };
                if p["flip"] == true || base["flip"] == true {
                    return None;
                }
                let sign = match def["extent"]["direction"].as_str() {
                    Some("positive" | "symmetric") => 1.,
                    Some("negative") => -1.,
                    _ => return None,
                };
                Some((a, normal * sign))
            })
        };
        let kind = if kind == "pocket"
            && faces.iter().any(|v| v["cylinder"].is_object())
            && !faces.iter().any(|v| {
                v["name"].as_str().is_some_and(|n| n.contains(":side:")) && v["cylinder"].is_null()
            }) {
            "hole"
        } else {
            kind
        };
        out.push(Feature {
            name: name.into(),
            body: body.clone(),
            id,
            definition: def.clone(),
            kind: kind.into(),
            type_name: f["type"].as_str().unwrap_or("").into(),
            faces,
            direction,
            context_faces: data
                .iter()
                .rev()
                .find(|d| d["body"] == body && d["faces"].is_array())
                .and_then(|d| d["faces"].as_array())
                .cloned()
                .unwrap_or_default(),
            bbox: doc["bodies"]
                .as_array()
                .and_then(|bs| bs.iter().find(|b| b["name"] == body))
                .map(|b| b["bbox"].clone())
                .unwrap_or(Value::Null),
            volume: doc["bodies"]
                .as_array()
                .and_then(|bs| bs.iter().find(|b| b["name"] == body))
                .and_then(|b| volume(&b["volume_mm3"])),
            anchor,
            nested: Some(0.),
        });
    }
    // Newest first, so every attached level is complete before its support.
    for i in 0..out.len() {
        let Some(face) = terminal(&out[i]).and_then(|v| v["name"].as_str()) else {
            continue;
        };
        let nested = out
            .iter()
            .filter(|c| {
                c.id > out[i].id
                    && c.body == out[i].body
                    && c.anchor
                        .as_ref()
                        .is_some_and(|(id, n)| *id == out[i].id && n == face)
            })
            .try_fold(0., |sum, c| footprint(c).map(|a| sum + a));
        out[i].nested = nested;
    }
    out
}
// Words a position or level adjective can qualify ("the left hole", "the end one").
const QUALIFIED: [&str; 21] = [
    "pocket",
    "recess",
    "slot",
    "boss",
    "pad",
    "rib",
    "tab",
    "hole",
    "counterbore",
    "countersink",
    "bore",
    "one",
    "feature",
    "cut",
    "end",
    "side",
    "step",
    "level",
    "tier",
    "stage",
    "ledge",
];
const LEVELS: [&str; 7] = ["step", "steps", "level", "levels", "tier", "stage", "ledge"];
// A stable point of the feature: a hole's entry, else the mean of its owned face centroids.
fn position(f: &Feature) -> Option<[f64; 3]> {
    if let Some(p) = point(&f.definition["position"]) {
        return Some(p);
    }
    let centres: Vec<_> = f
        .faces
        .iter()
        .filter_map(|v| point(&v["centroid"]))
        .collect();
    if centres.is_empty() {
        return None;
    }
    let n = centres.len() as f64;
    Some([0, 1, 2].map(|a| centres.iter().map(|c| c[a]).sum::<f64>() / n))
}
// The simple chain of levels through f: each one sketched on the previous end face.
fn chain(f: &Feature, fs: &[Feature]) -> Option<Vec<Feature>> {
    let attached = |parent: &Feature, child: &Feature| {
        child.body == parent.body
            && terminal(parent)
                .and_then(|t| t["name"].as_str())
                .is_some_and(|face| {
                    child
                        .anchor
                        .as_ref()
                        .is_some_and(|(id, n)| *id == parent.id && n == face)
                })
    };
    let mut root = f;
    for _ in 0..8 {
        match fs.iter().find(|p| attached(p, root)) {
            Some(p) => root = p,
            None => break,
        }
    }
    let mut out = vec![root.clone()];
    for _ in 0..8 {
        let last = out.last()?;
        let next: Vec<_> = fs.iter().filter(|c| attached(last, c)).collect();
        match next.as_slice() {
            [] => break,
            [c] => out.push((*c).clone()),
            _ => return None,
        }
    }
    Some(out)
}
fn matching(request: &Value) -> Vec<Feature> {
    let w = words(user_text(request));
    let has = |s: &str| w.iter().any(|v| v == s);
    // The counterbore or countersink itself is meant only when named as a noun;
    // "counterbored" and "countersunk" describe the hole, so its bore is edited.
    let counterbore = has("counterbore")
        || has("counterbores")
        || has("cbore")
        || lower_phrase(&w, &["counter", "bore"]);
    let countersink = has("countersink") || has("countersinks") || has("csink");
    let mut out = select(request);
    for f in &mut out {
        if (f.kind == "counterbore" && !counterbore) || (f.kind == "countersink" && !countersink) {
            f.kind = "hole".into();
        }
    }
    out
}
fn select(request: &Value) -> Vec<Feature> {
    let w = words(user_text(request));
    let fs = features(request);
    let spans = |name: &str| -> Vec<(usize, usize)> {
        let n = words(name);
        if n.is_empty() {
            return Vec::new();
        }
        w.windows(n.len())
            .enumerate()
            .filter(|(_, v)| *v == n.as_slice())
            .map(|(i, _)| (i, i + n.len()))
            .collect()
    };
    let found: Vec<_> = fs.iter().map(|f| spans(&f.name)).collect();
    // A longer name containing a shorter one ("Step pocket 2") is the one meant.
    let named: Vec<_> = fs
        .iter()
        .zip(&found)
        .filter(|(_, own)| {
            own.iter().any(|(a, b)| {
                !found
                    .iter()
                    .flatten()
                    .any(|(c, d)| c <= a && b <= d && d - c > b - a)
            })
        })
        .map(|(f, _)| f.name.clone())
        .collect();
    let starts: Vec<_> = found.iter().flatten().map(|(a, _)| *a).collect();
    let bodies: Vec<_> = fs
        .iter()
        .filter(|f| {
            w.windows(words(&f.body).len().max(1))
                .any(|v| v == words(&f.body))
        })
        .map(|f| f.body.clone())
        .collect();
    if w.windows(2)
        .any(|v| v[0] == "body" && !matches!(v[1].as_str(), "by" | "to" | "the"))
        && bodies.is_empty()
    {
        return Vec::new();
    }
    let has = |s: &str| w.iter().any(|v| v == s);
    let kind_requested = w.iter().any(|v| {
        matches!(
            v.as_str(),
            "pocket"
                | "recess"
                | "slot"
                | "boss"
                | "pad"
                | "rib"
                | "tab"
                | "hole"
                | "bore"
                | "counterbore"
                | "counterbores"
                | "counterbored"
                | "cbore"
                | "countersink"
                | "countersinks"
                | "countersunk"
                | "csink"
                | "plate"
        )
    });
    let matches: Vec<_> = fs
        .iter()
        .filter(|f| {
            let kind = w.iter().any(|v| match v.as_str() {
                "pocket" | "recess" | "slot" => {
                    f.kind == "pocket" || (f.kind == "hole" && f.type_name != "HoleFeature")
                }
                "boss" | "pad" | "rib" | "tab" => f.kind == "pad",
                "hole" | "bore" => {
                    matches!(f.kind.as_str(), "hole" | "counterbore" | "countersink")
                }
                "counterbore" | "counterbores" | "counterbored" | "cbore" => {
                    f.kind == "counterbore"
                }
                "countersink" | "countersinks" | "countersunk" | "csink" => f.kind == "countersink",
                "plate" => f.type_name == "BoxFeature" && f.definition["operation"] == "new_body",
                _ => false,
            }) && (!has("counterbored") || f.kind == "counterbore")
                && (!has("countersunk") || f.kind == "countersink");
            (!(f.definition["operation"] == "new_body")
                || named.contains(&f.name)
                || (w.iter().any(|v| v == "plate") && kind)
                || (!bodies.is_empty() && !kind))
                && (if named.is_empty() {
                    if kind_requested {
                        kind
                    } else {
                        !bodies.is_empty() && f.definition["operation"] == "new_body"
                    }
                } else {
                    named.contains(&f.name)
                })
                && (bodies.is_empty() || bodies.contains(&f.body))
        })
        .cloned()
        .collect();
    // Position and level words select by inspected geometry, only when they qualify a noun.
    let qualifies = |i: usize| {
        (w.get(i + 1)
            .is_some_and(|n| QUALIFIED.contains(&n.as_str()))
            && !starts.contains(&(i + 1)))
            || (i >= 2
                && w[i - 1] == "the"
                && matches!(
                    w[i - 2].as_str(),
                    "on" | "at" | "near" | "nearest" | "closest"
                ))
    };
    let mut sides = Vec::new();
    let (mut end, mut middle, mut level) = (false, false, None);
    for (i, v) in w.iter().enumerate() {
        let side = match v.as_str() {
            "left" | "leftmost" => Some((0, false)),
            "right" | "rightmost" => Some((0, true)),
            "front" | "frontmost" => Some((1, false)),
            "back" | "rear" | "rearmost" | "backmost" => Some((1, true)),
            "highest" | "topmost" | "uppermost" => Some((2, true)),
            "lowest" | "bottommost" => Some((2, false)),
            _ => None,
        };
        if let Some(side) = side {
            if v.ends_with("most") || v.ends_with("est") || qualifies(i) {
                sides.push(side);
            }
            continue;
        }
        match v.as_str() {
            "end" if qualifies(i) => end = true,
            "middle" | "centre" | "center" if qualifies(i) => middle = true,
            // A leading raise/lower is the verb; a following name keeps its own words.
            "upper" | "top" | "higher" | "lower" | "bottom" | "first" | "second" | "outer"
            | "inner" | "shallow" | "shallower" | "deep" | "deeper" | "deepest" | "last"
                if i > 0
                    && w.get(i + 1).is_some_and(|n| LEVELS.contains(&n.as_str()))
                    && !starts.contains(&(i + 1)) =>
            {
                level = Some(v.as_str())
            }
            _ => {}
        }
    }
    if sides.is_empty() && !end && !middle && level.is_none() {
        return matches;
    }
    let mut pool = matches;
    if pool.is_empty() && named.is_empty() && !kind_requested && bodies.is_empty() {
        pool = fs
            .iter()
            .filter(|f| f.definition["operation"] != "new_body")
            .cloned()
            .collect();
    }
    if let Some(word) = level {
        let mut chains: Vec<Vec<Feature>> = Vec::new();
        for f in &pool {
            let Some(c) = chain(f, &fs) else {
                return Vec::new();
            };
            if c.len() >= 2 && !chains.iter().any(|o| o[0].id == c[0].id) {
                chains.push(c);
            }
        }
        match (chains.as_slice(), word) {
            ([c], "first" | "outer" | "shallow" | "shallower") => return vec![c[0].clone()],
            ([c], "second") => return vec![c[1].clone()],
            ([c], "inner" | "deep" | "deeper" | "deepest" | "last") => {
                return c.last().cloned().into_iter().collect()
            }
            ([c], _) => {
                // World up and down need vertical levels with distinct end faces.
                let heights: Option<Vec<_>> = c
                    .iter()
                    .map(|f| {
                        f.direction
                            .filter(|(a, _)| *a == 2)
                            .and_then(|_| number(&terminal(f)?["centroid"][2]))
                    })
                    .collect();
                let Some(heights) = heights else {
                    return Vec::new();
                };
                let up = matches!(word, "upper" | "top" | "higher");
                let pick = heights
                    .iter()
                    .copied()
                    .reduce(|a, b| if up { a.max(b) } else { a.min(b) });
                let keep: Vec<_> = c
                    .iter()
                    .zip(&heights)
                    .filter(|(_, h)| pick.is_some_and(|p| (**h - p).abs() < 0.001))
                    .map(|(f, _)| f.clone())
                    .collect();
                return if keep.len() == 1 { keep } else { Vec::new() };
            }
            ([], "upper" | "top" | "higher") => sides.push((2, true)),
            ([], "lower" | "bottom") => sides.push((2, false)),
            _ => return Vec::new(),
        }
    }
    // "The left one" over mixed kinds does not say which kind is meant.
    fn family(f: &Feature) -> &str {
        match f.kind.as_str() {
            "counterbore" | "countersink" => "hole",
            k => k,
        }
    }
    if named.is_empty() && !kind_requested && pool.iter().any(|f| family(f) != family(&pool[0])) {
        return Vec::new();
    }
    let Some(positions) = pool.iter().map(position).collect::<Option<Vec<_>>>() else {
        return Vec::new();
    };
    let mut keep = vec![true; pool.len()];
    for (a, maximum) in sides {
        let extreme =
            positions
                .iter()
                .map(|p| p[a])
                .reduce(|x, y| if maximum { x.max(y) } else { x.min(y) });
        for (k, p) in keep.iter_mut().zip(&positions) {
            *k &= extreme.is_some_and(|e| (p[a] - e).abs() < 0.001);
        }
    }
    if end || middle {
        // Ends lie along the body's clearly longest side, in its outer thirds.
        let Some(bbox) = pool
            .first()
            .map(|f| f.bbox.clone())
            .filter(|b| pool.iter().all(|f| f.bbox == *b))
        else {
            return Vec::new();
        };
        let Some(span) = (0..3)
            .map(|a| Some((number(&bbox["min"][a])?, number(&bbox["max"][a])?)))
            .collect::<Option<Vec<_>>>()
        else {
            return Vec::new();
        };
        let mut order: Vec<_> = (0..3).collect();
        order.sort_by(|x, y| (span[*y].1 - span[*y].0).total_cmp(&(span[*x].1 - span[*x].0)));
        let a = order[0];
        let (low, high) = span[a];
        if high - low < 1.25 * (span[order[1]].1 - span[order[1]].0) {
            return Vec::new();
        }
        let third = (high - low) / 3.;
        if end {
            for (k, p) in keep.iter_mut().zip(&positions) {
                *k &= p[a] < low + third || p[a] > high - third;
            }
        }
        if middle {
            let mut along: Vec<_> = positions.iter().map(|p| p[a]).collect();
            along.sort_by(f64::total_cmp);
            let distinct = along.windows(2).all(|v| v[1] - v[0] > 0.001);
            if !distinct || along.len() % 2 == 0 {
                return Vec::new();
            }
            let median = along[along.len() / 2];
            for (k, p) in keep.iter_mut().zip(&positions) {
                *k &= (p[a] - median).abs() < 0.001;
            }
        }
    }
    let chosen: Vec<_> = pool
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(f, _)| f)
        .collect();
    if chosen.len() == 1 {
        chosen
    } else {
        Vec::new()
    }
}
pub fn reads(request: &Value, catalog: &Catalog) -> Result<Vec<Value>, String> {
    let data = form_context::payloads(request);
    let Some(doc) = form_context::document(&data) else {
        return form_context::reads(request, false, catalog);
    };
    if doc["timeline"].as_array().is_none_or(|t| t.len() > 64) {
        return Ok(Vec::new());
    }
    let mut calls = Vec::new();
    for f in doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|f| f["suppressed"] != true && f["rolled_back"] != true && f["error"].is_null())
    {
        if form_context::detail(&data, f).is_none() {
            calls.push(call(
                catalog,
                "timeline.inspect",
                json!({"feature":f["name"]}),
            )?);
        }
    }
    if !calls.is_empty() {
        return Ok(calls);
    }
    {
        for b in doc["bodies"].as_array().into_iter().flatten().take(12) {
            if !data
                .iter()
                .any(|d| d["body"] == b["name"] && d["faces"].is_array())
            {
                calls.push(call(catalog, "model.faces", json!({"body":b["name"]}))?);
            }
        }
    }
    Ok(std::mem::take(&mut calls))
}
pub fn form(request: &Value) -> Value {
    let mut s = schema();
    let fs = matching(request);
    s["properties"]["feature"]["enum"] = json!(fs.iter().map(|f| &f.name).collect::<Vec<_>>());
    let mut parameters = Vec::new();
    for f in &fs {
        // A named dimension with no witness (a countersink cone) offers no stand-in.
        if !intentions(request, f).is_empty() && preferred(request, f).is_empty() {
            continue;
        }
        for p in preferred(request, f).into_iter().chain(PARAMETERS) {
            if available(f, p) && !parameters.contains(&p) {
                parameters.push(p);
            }
        }
    }
    s["properties"]["parameter"]["enum"] = json!(parameters);
    s["properties"]["parameter"]["enumNames"] = json!(parameters
        .iter()
        .map(|p| {
            match *p {
                "height" => "Height (mm)",
                "depth" => "Depth (mm)",
                "diameter" => "Diameter (mm)",
                "width" => "Width (mm)",
                "length" => "Length (mm)",
                "start_offset" => "Start offset (mm)",
                "x" => "X position (mm)",
                "y" => "Y position (mm)",
                _ => "Z position (mm)",
            }
        })
        .collect::<Vec<_>>());
    s["properties"]["feature"]["description"] = json!(fs
        .iter()
        .map(|f| format!(
            "{}: {} ({}) in body {}; size {}; position {}",
            f.name,
            f.kind,
            f.type_name,
            f.body,
            json!([
                "depth",
                "height",
                "width",
                "length",
                "diameter",
                "start_offset"
            ]
            .iter()
            .filter_map(|p| field(f, p).map(|(_, n)| {
                let size = witness(f, p)
                    .and_then(|(_, m, _)| exposed_size(f, p, m))
                    .unwrap_or(n);
                (p, json!(size))
            }))
            .collect::<std::collections::BTreeMap<_, _>>()),
            f.faces
                .first()
                .map(|v| v["centroid"].clone())
                .unwrap_or(Value::Null)
        ))
        .collect::<Vec<_>>()
        .join("; "));
    s
}
fn user_text(request: &Value) -> &str {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
}
const PARAMETERS: [&str; 9] = [
    "height",
    "depth",
    "diameter",
    "width",
    "length",
    "x",
    "y",
    "z",
    "start_offset",
];
fn available(f: &Feature, p: &str) -> bool {
    (if matches!(p, "x" | "y" | "z") {
        f.definition["position"].is_array() || f.definition["base"].is_array()
    } else {
        field(f, p).is_some()
    }) && witness(f, p).is_some()
}
// Semantic groups are shared by exact parsing, form ordering and bounded recovery.
fn intentions(request: &Value, f: &Feature) -> Vec<&'static str> {
    let w = words(user_text(request));
    let has = |terms: &[&str]| w.iter().any(|v| terms.contains(&v.as_str()));
    let axial = if f.kind == "pad" { "height" } else { "depth" };
    let mut out = Vec::new();
    if has(&[
        "height",
        "tall",
        "high",
        "thick",
        "taller",
        "thickness",
        "thicker",
        "thinner",
        "higher",
    ]) {
        out.push("height");
    }
    if has(&["raise", "lower"]) {
        out.push(axial);
    }
    // How far a raised feature stands up is its height.
    if f.kind == "pad"
        && has(&[
            "protrude",
            "protrudes",
            "proud",
            "stand",
            "stands",
            "stick",
            "sticks",
        ])
    {
        out.push("height");
    }
    if has(&["depth", "deep", "deeper", "deepen", "shallower"]) {
        out.push("depth");
    }
    if has(&["distance"]) {
        out.push(axial);
    }
    if has(&["radius"]) {
        out.push("radius");
    }
    if has(&["position"]) && !has(&["x", "y", "z"]) {
        out.push("position");
    }
    if has(&["diameter", "across"]) {
        out.push("diameter");
    }
    if has(&["widen", "wider", "narrower", "width"]) {
        out.push(if f.faces.iter().any(|v| v["cylinder"].is_object()) {
            "diameter"
        } else {
            "width"
        });
    }
    if has(&["length"]) {
        out.push("length");
    }
    for p in ["x", "y", "z"] {
        if has(&[p]) {
            out.push(p);
        }
    }
    // Offset in a move request belongs to the existing move recipe.
    if has(&["offset"]) && (has(&["start"]) || !has(&["move", "shift"])) {
        out.push("start_offset");
    }
    out.sort_unstable();
    out.dedup();
    out
}
fn unambiguous(request: &Value, f: &Feature) -> bool {
    intentions(request, f).len() == 1
}
fn preferred(request: &Value, f: &Feature) -> Vec<&'static str> {
    let mut out = intentions(request, f);
    let w = words(user_text(request));
    if out.is_empty()
        && w.iter().any(|v| {
            matches!(
                v.as_str(),
                "bigger" | "smaller" | "resize" | "grow" | "shrink" | "shorter" | "thicken"
            )
        })
    {
        out.extend([
            if f.kind == "pad" { "height" } else { "depth" },
            "diameter",
            "width",
            "length",
        ]);
    }
    if w.iter().any(|v| matches!(v.as_str(), "raise" | "lower"))
        && !w.iter().any(|v| {
            matches!(
                v.as_str(),
                "height"
                    | "depth"
                    | "distance"
                    | "tall"
                    | "taller"
                    | "deep"
                    | "deeper"
                    | "shallower"
            )
        })
        && f.direction.is_some_and(|(a, _)| a == 2)
    {
        out.extend(["start_offset", "z"]);
    }
    out.retain(|p| available(f, p));
    out
}
fn parsed(request: &Value) -> Option<Value> {
    let text = request["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|m| m["role"] == "user")?["content"]
        .as_str()?;
    let w = words(text);
    let has = |s: &str| w.iter().any(|v| v == s);
    // Unit-bearing values only: body/feature identifiers may contain digits.
    let t = slots::tokens(text);
    let values: Vec<_> = t
        .windows(2)
        .filter_map(|v| {
            slots::scale(&v[1]).and_then(|scale| slots::number(&v[0]).map(|n| n * scale))
        })
        .collect();
    let [value] = values.as_slice() else {
        return None;
    };
    let fs = matching(request);
    let [f] = fs.as_slice() else { return None };
    let choices = preferred(request, f);
    let parameter = *choices.first()?;
    // A word must identify the dimension itself. A feature noun alone is not intent.
    if !unambiguous(request, f) || !available(f, parameter) {
        return None;
    }
    let relative = has("by")
        || (!has("to")
            && [
                "wider",
                "deeper",
                "taller",
                "shorter",
                "shallower",
                "higher",
                "narrower",
                "thicker",
                "thinner",
            ]
            .iter()
            .any(|s| has(s)));
    if !relative && !has("to") && !has("make") && !has("resize") && !has("set") && !has("be") {
        return None;
    }
    let value = if relative
        && (has("lower")
            || has("shorter")
            || has("shallower")
            || has("negative")
            || has("reduce")
            || has("decrease")
            || has("trim")
            || has("narrower")
            || has("thinner")
            || has("down")
            || has("shrink")
            || has("shorten"))
    {
        -value.abs()
    } else {
        *value
    };
    Some(json!({"parameter":parameter,"mode":if relative{"change"}else{"set"},"value":value}))
}
pub fn exact(request: &Value, catalog: &Catalog) -> Result<Option<Plan>, String> {
    let fs = matching(request);
    let [f] = fs.as_slice() else { return Ok(None) };
    let Some(mut args) = parsed(request) else {
        return Ok(None);
    };
    if f.definition["extent"]["direction"] == "symmetric" || f.kind == "counterbore" {
        return Ok(None);
    }
    // Raise/lower of a sideways feature is unclear wording: leave it to the form.
    if words(user_text(request))
        .iter()
        .any(|v| matches!(v.as_str(), "raise" | "lower"))
        && f.direction.is_none_or(|(a, _)| a != 2)
    {
        return Ok(None);
    }
    args["feature"] = json!(f.name);
    // Unsupported geometry stays on the form path; it never guesses a field.
    Ok(build(&args, request, catalog)
        .ok()
        .filter(|p| p.slots["attachments"].as_array().is_none_or(Vec::is_empty)))
}
/// Wording that changes one size of something already there, never wording that adds one.
/// Unclear wording returns false so creation routing stays as it was.
pub fn change_wording(text: &str) -> bool {
    if text.len() > 2048 {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    let w: Vec<_> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    let has = |terms: &[&str]| w.iter().any(|v| terms.contains(v));
    let sizes = slots::tokens(text)
        .windows(2)
        .filter(|v| slots::number(&v[0]).is_some() && slots::scale(&v[1]).is_some())
        .count();
    let comparative = has(&[
        "taller",
        "shorter",
        "deeper",
        "shallower",
        "wider",
        "narrower",
        "higher",
        "thicker",
        "thinner",
        "longer",
    ]);
    let directional = has(&[
        "raise", "lower", "deepen", "widen", "lengthen", "shorten", "thicken", "heighten",
        "extend", "increase", "reduce", "decrease", "grow", "shrink",
    ]);
    let sized = has(&["make", "set", "change", "resize", "adjust"])
        && has(&[
            "height",
            "depth",
            "width",
            "length",
            "diameter",
            "thickness",
            "tall",
            "deep",
            "wide",
            "long",
            "high",
            "thick",
            "across",
        ]);
    (1..=2).contains(&sizes)
        && (comparative || directional || sized)
        && !w
            .windows(2)
            .any(|v| v[0] == "make" && matches!(v[1], "a" | "an"))
        && !has(&[
            "add",
            "create",
            "build",
            "new",
            "another",
            "second",
            "insert",
            "put",
            "place",
            "with",
            "plus",
            "also",
            "and",
            "then",
            "copy",
            "copies",
            "duplicate",
            "pattern",
            "row",
            "grid",
            "mirror",
            "whole",
            "all",
            "each",
            "every",
            "both",
            "not",
            "no",
            "don",
            "dont",
            "never",
            "without",
            "what",
            "which",
            "why",
            "how",
            "explain",
            "describe",
        ])
}
/// Earlier public tool results in this conversation reported at least one body.
pub fn design_in_history(request: &Value) -> bool {
    let messages = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let end = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    messages[..end]
        .iter()
        .rev()
        .take(4096)
        .filter(|m| m["role"] == "tool")
        .filter_map(tool_payload)
        .any(|d| {
            ["bodies", "created_bodies", "changed_bodies"]
                .iter()
                .any(|k| d[*k].as_array().is_some_and(|a| !a.is_empty()))
        })
}
/// After inspection: the request changes exactly one existing feature with a measured size.
pub fn existing_change(request: &Value) -> bool {
    if !change_wording(user_text(request)) || matching(request).len() != 1 {
        return false;
    }
    let s = form(request);
    ["feature", "parameter"].iter().all(|k| {
        s["properties"][*k]["enum"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    })
}
fn field(f: &Feature, p: &str) -> Option<(String, f64)> {
    let d = &f.definition;
    if matches!(p, "height" | "depth" | "start_offset")
        && d["extent"].is_object()
        && (d["extent"]["distance2"].is_string()
            || d["extent"]["distance2"].is_number()
            || d["extent"]["through_all"] == true
            || d["extent"].get("to").is_some())
    {
        return None;
    }
    // A countersink's cone has no public radius to witness; only its hole is edited.
    if f.kind == "countersink"
        || (p == "height" && (f.kind == "hole" || f.kind == "counterbore"))
        || (p == "depth" && f.kind == "pad")
    {
        return None;
    }
    if f.kind == "counterbore" && matches!(p, "depth" | "diameter") {
        let path = if p == "depth" {
            "hole.cb_depth"
        } else {
            "hole.cb_diameter"
        };
        return Some((
            path.into(),
            number(d.pointer(&format!("/{}", path.replace('.', "/")))?)?,
        ));
    }
    if p == "diameter" && d.get("radius").is_some() {
        let radius = number(&d["radius"]).or_else(|| {
            f.faces
                .iter()
                .find_map(|v| number(&v["cylinder"]["radius"]))
        })?;
        return Some(("radius".into(), radius * 2.));
    }
    let path = match p {
        "depth" if f.type_name == "CylinderFeature" => "height",
        "depth" | "height" if d["extent"].is_object() => "extent.distance",
        "depth" => "depth",
        "height" => "height",
        "width" => "width",
        "length" => "length",
        "diameter" => "diameter",
        "start_offset" if d["extent"].is_object() => "extent.start_offset",
        _ => return None,
    };
    let v = d.pointer(&format!("/{}", path.replace('.', "/")))?;
    Some((path.into(), number(v)?))
}
// Geometry witness: a certified feature-owned plane or cylinder. Its displacement,
// radius or span must change by the computed amount; parameter echo alone is insufficient.
fn witness(f: &Feature, p: &str) -> Option<(Value, f64, f64)> {
    if matches!(p, "x" | "y" | "z") {
        if let Some(face) = f.faces.iter().find(|v| v["cylinder"].is_object()) {
            let a = match p {
                "x" => 0,
                "y" => 1,
                _ => 2,
            };
            return Some((
                json!({"face":face["name"],"kind":"origin","axis":a}),
                number(&face["cylinder"]["origin"][a])?,
                1.,
            ));
        }
    }
    if p == "diameter" {
        // A hole's bore and counterbore are separate certified cylinders.
        let part = if f.kind == "counterbore" {
            ":cbore:"
        } else {
            ":bore:"
        };
        let face = f.faces.iter().find(|v| {
            v["cylinder"].is_object()
                && (f.type_name != "HoleFeature"
                    || v["name"].as_str().is_some_and(|n| n.contains(part)))
        })?;
        return Some((
            json!({"face":face["name"],"kind":"diameter"}),
            number(&face["cylinder"]["radius"])? * 2.,
            1.,
        ));
    }
    let (a, sign) = if matches!(p, "depth" | "height" | "start_offset") {
        f.direction?
    } else {
        (
            match p {
                "x" | "length" => 0,
                "y" | "width" => 1,
                "z" => 2,
                _ => return None,
            },
            1.,
        )
    };
    let face = if f.definition["extent"].is_object() {
        f.faces.iter().find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.ends_with(":bottom") || n.ends_with(":top"))
                && point(&v["plane_normal"])
                    .and_then(axis)
                    .is_some_and(|(i, _)| i == a)
        })?
    } else if f.type_name == "HoleFeature" {
        f.faces.iter().find(|v| {
            v["name"].as_str().is_some_and(|n| {
                n.ends_with(if f.kind == "counterbore" {
                    ":cbore:base"
                } else {
                    ":bore:base"
                })
            })
        })?
    } else if f.type_name == "CylinderFeature" && matches!(p, "height" | "depth") {
        let terminal =
            number(&f.definition["base"][a])? + f.direction?.1 * number(&f.definition["height"])?;
        f.faces.iter().find(|v| {
            point(&v["plane_normal"])
                .and_then(axis)
                .is_some_and(|(i, _)| i == a)
                && number(&v["centroid"][a]).is_some_and(|v| (v - terminal).abs() < 0.001)
        })?
    } else {
        f.faces
            .iter()
            .find(|v| point(&v["plane_normal"]).and_then(axis) == Some((a, 1.)))?
    };
    Some((
        json!({"face":face["name"],"kind":"plane","axis":a}),
        number(&face["centroid"][a])?,
        if f.definition["extent"]["direction"] == "symmetric"
            && p != "start_offset"
            && f.definition["extent"]["convention"] != "legacy_per_side"
        {
            sign * 0.5
        } else {
            sign
        },
    ))
}
// The exposed size is measured from the supporting material, not necessarily
// from the feature's sketch plane (which may be buried inside that material).
fn exposed_size(f: &Feature, p: &str, measured: f64) -> Option<f64> {
    let (a, sign) = match p {
        "height" | "depth" => f.direction?,
        "length" => (0, 1.),
        "width" => (1, 1.),
        _ => return None,
    };
    let (_, original) = field(f, p)?;
    let scale = witness(f, p)?.2.abs();
    let start = measured - original * scale * sign;
    let prefix = format!("F{}:", f.id);
    let support = f
        .context_faces
        .iter()
        .filter(|v| {
            v["exact"] == true
                && !v["name"].as_str().is_some_and(|n| n.starts_with(&prefix))
                && point(&v["plane_normal"])
                    .and_then(axis)
                    .is_some_and(|(i, _)| i == a)
                && v["name"]
                    .as_str()
                    .and_then(|n| n.split(':').next())
                    .is_some_and(|owner| {
                        // Require the support's footprint to contain the target terminal centre.
                        let terminal = f.faces.iter().find(|q| {
                            number(&q["centroid"][a]).is_some_and(|x| (x - measured).abs() < 0.001)
                        });
                        terminal.is_some_and(|terminal| {
                            (0..3).filter(|i| *i != a).all(|i| {
                                let coordinates: Vec<_> = f
                                    .context_faces
                                    .iter()
                                    .filter(|q| {
                                        q["name"].as_str().and_then(|n| n.split(':').next())
                                            == Some(owner)
                                    })
                                    .filter_map(|q| number(&q["centroid"][i]))
                                    .collect();
                                let x = number(&terminal["centroid"][i]).unwrap_or(f64::NAN);
                                coordinates.iter().any(|v| *v <= x + 0.001)
                                    && coordinates.iter().any(|v| *v >= x - 0.001)
                            })
                        })
                    })
        })
        .filter_map(|v| number(&v["centroid"][a]))
        .filter(|v| (measured - v) * sign > 0.001 && (v - start) * sign >= -0.001)
        .min_by(|x, y| ((measured - x) * sign).total_cmp(&((measured - y) * sign)))?;
    Some((measured - support) * sign)
}
fn volume_change(f: &Feature, p: &str, before: f64, after: f64, witness: &Value) -> Option<f64> {
    let delta = after - before;
    if matches!(p, "x" | "y" | "z") {
        return Some(0.);
    }
    let cut = if matches!(f.kind.as_str(), "pocket" | "hole" | "counterbore") {
        -1.
    } else {
        1.
    };
    if matches!(p, "height" | "depth")
        && (f.type_name == "CylinderFeature"
            || (f.type_name == "ExtrudeFeature" && (f.kind == "hole" || round(f))))
    {
        let radius = f
            .faces
            .iter()
            .find_map(|face| number(&face["cylinder"]["radius"]))?;
        let scale = witness["kind"]
            .as_str()
            .filter(|k| *k == "plane")
            .and_then(|_| self::witness(f, p))
            .map(|(_, _, scale)| scale.abs())?;
        return Some(cut * std::f64::consts::PI * radius * radius * delta * scale);
    }
    if p == "diameter" && f.kind == "hole" && f.type_name == "HoleFeature" {
        if let Some(original) = removed(f, before) {
            return Some(original - removed(f, after)?);
        }
    }
    if p == "diameter" {
        let depth = if f.kind == "counterbore" {
            number(&f.definition["hole"]["cb_depth"])?
        } else {
            field(f, "depth")
                .or_else(|| field(f, "height"))
                .map(|(_, v)| v)
                .or_else(|| {
                    let (a, _) = f.direction?;
                    Some(number(&f.bbox["max"][a])? - number(&f.bbox["min"][a])?)
                })?
        };
        return Some(cut * std::f64::consts::PI / 4. * (after * after - before * before) * depth);
    }
    if matches!(f.kind.as_str(), "hole" | "counterbore")
        && p == "depth"
        && f.type_name == "HoleFeature"
    {
        let bore = number(&f.definition["diameter"])?;
        let area = if f.kind == "counterbore" {
            let cb = number(&f.definition["hole"]["cb_diameter"])?;
            (cb * cb - bore * bore) * std::f64::consts::PI / 4.
        } else {
            bore * bore * std::f64::consts::PI / 4.
        };
        return Some(-area * delta);
    }
    let face = f.faces.iter().find(|v| v["name"] == witness["face"])?;
    let scale = if f.definition["extent"]["direction"] == "symmetric"
        && f.definition["extent"]["convention"] != "legacy_per_side"
    {
        0.5
    } else {
        1.
    };
    // Levels attached to the moving end face travel with it and keep their own volume.
    let nested = if terminal(f).is_some_and(|t| t["name"] == face["name"]) {
        f.nested?
    } else {
        0.
    };
    Some(cut * (number(&face["area"])? + nested) * delta * scale)
}
// An extrusion whose only side is one cylinder: its profile is that circle.
fn round(f: &Feature) -> bool {
    let sides: Vec<_> = f
        .faces
        .iter()
        .filter(|v| v["name"].as_str().is_some_and(|n| n.contains(":side:")))
        .collect();
    matches!(sides.as_slice(), [side] if side["cylinder"].is_object())
}
// Material a counterbored or countersunk hole removes for a bore diameter d.
// Simple holes keep the existing per-field arithmetic.
fn removed(f: &Feature, d: f64) -> Option<f64> {
    let def = &f.definition;
    let hole = &def["hole"];
    // A through hole crosses the body's span along its axis.
    let depth = number(&def["depth"]).or_else(|| {
        let (a, _) = f.direction?;
        Some(number(&f.bbox["max"][a])? - number(&f.bbox["min"][a])?)
    })?;
    let quarter = std::f64::consts::PI / 4.;
    match hole["type"].as_str()? {
        "counterbore" => {
            let (cb, cb_depth) = (number(&hole["cb_diameter"])?, number(&hole["cb_depth"])?);
            (cb > d && depth > cb_depth)
                .then_some(quarter * (cb * cb * cb_depth + d * d * (depth - cb_depth)))
        }
        "countersink" => {
            let cs = number(&hole["cs_diameter"])?;
            let angle = number(&hole["cs_angle"])?.to_radians();
            if !(cs > d && angle > 0. && angle < std::f64::consts::PI) {
                return None;
            }
            // The cone runs from the countersink diameter down to the bore.
            let cone = (cs - d) / 2. / (angle / 2.).tan();
            (cone < depth).then_some(
                std::f64::consts::PI * cone / 12. * (cs * cs + cs * d + d * d)
                    + quarter * d * d * (depth - cone),
            )
        }
        _ => None,
    }
}
/// Trimmed, unquoted, lowercase text with single spaces.
fn loose(text: &str) -> String {
    let t = text.trim();
    let t = [('"', '"'), ('\'', '\''), ('`', '`'), ('“', '”'), ('‘', '’')]
        .iter()
        .find_map(|(a, b)| t.strip_prefix(*a).and_then(|s| s.strip_suffix(*b)))
        .unwrap_or(t);
    t.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
// The model may echo an inspected name loosely. Take a unique exact name first, then a
// unique loose one (also as "<body> / <name>" or "<body>:<name>"); never a nearest guess.
fn chosen<'a>(name: &str, fs: &'a [Feature]) -> Option<&'a Feature> {
    let only = |hits: Vec<&'a Feature>| match hits.as_slice() {
        [f] => Some(*f),
        _ => None,
    };
    let exact: Vec<_> = fs.iter().filter(|f| f.name == name).collect();
    if !exact.is_empty() {
        return only(exact);
    }
    let whole = loose(name);
    only(
        fs.iter()
            .filter(|f| {
                let target = loose(&f.name);
                whole == target
                    || [name, whole.as_str()].iter().any(|text| {
                        ['/', ':'].iter().any(|sep| {
                            text.split_once(*sep).is_some_and(|(body, rest)| {
                                loose(body) == loose(&f.body) && loose(rest) == target
                            })
                        })
                    })
            })
            .collect(),
    )
}
pub fn build(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(args, &schema())?;
    let fs = matching(request);
    // Among several equal candidates the fill decides only when the user named that one;
    // otherwise a guessed pick builds the wrong feature, so the ambiguity stands.
    let spaced = |t: &str| {
        let t: String = t
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect();
        format!(" {} ", loose(&t))
    };
    let said = spaced(user_text(request));
    let f = args["feature"]
        .as_str()
        .and_then(|name| chosen(name, &fs))
        .filter(|f| fs.len() < 2 || said.contains(&spaced(&f.name)))
        .ok_or_else(|| {
            if fs.len() > 1 {
                format!(
                    "args.feature {} is ambiguous: several inspected features fit equally; set args.feature to exactly one of: {}",
                    args["feature"],
                    fs.iter()
                        .map(|f| format!("{} (body {})", json!(f.name), f.body))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                format!(
                    "args.feature {} must be one of the inspected matching feature names: {}",
                    args["feature"],
                    json!(fs.iter().map(|f| &f.name).collect::<Vec<_>>())
                )
            }
        })?;
    let p = args["parameter"]
        .as_str()
        .ok_or("args.parameter is required")?;
    // Request words only order the form; an available model choice always stands.
    if !available(f, p) {
        return Err(format!(
            "args.parameter {p} is unavailable on {}; choose one of its available parameters: {}",
            f.name,
            json!(PARAMETERS
                .iter()
                .filter(|q| available(f, q))
                .collect::<Vec<_>>())
        ));
    }
    let mode = args["mode"]
        .as_str()
        .filter(|s| matches!(*s, "set" | "change"))
        .ok_or("args.mode must be set or change")?;
    let value = number(&args["value"])
        .ok_or("args.value requires a finite millimetre value within 1000000")?;
    let coordinate = match p {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    };
    let (path, before) = if let Some(a) = coordinate {
        (
            if f.definition.get("base").is_some() {
                "base".into()
            } else {
                "position".into()
            },
            number(
                &f.definition[if f.definition.get("base").is_some() {
                    "base"
                } else {
                    "position"
                }][a],
            )
            .ok_or("args.parameter requires an inspected hole position")?,
        )
    } else {
        field(f, p).ok_or(
            "args.parameter is unavailable or its inspected value is an unevaluated expression",
        )?
    };
    let (witness, measured, derivative) = witness(f, p).ok_or(
        "args.parameter has no unique measurable geometry witness; inspect or clarify the feature",
    )?;
    let text = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let w = words(text);
    // Raise/lower reads the value as world vertical only along a vertical feature;
    // otherwise the words add nothing and the model's value applies as given.
    let vertical = w.iter().any(|v| matches!(v.as_str(), "raise" | "lower"))
        && f.direction.is_some_and(|(a, _)| a == 2);
    let delta = if mode == "change" && vertical && matches!(p, "depth" | "height") {
        value / derivative
    } else {
        value
    };
    let after = if vertical && mode == "set" && matches!(p, "depth" | "height") {
        before + (value - measured) / derivative
    } else if mode == "change" {
        before
            + if matches!(p, "height" | "depth") && !vertical {
                delta / derivative.abs()
            } else {
                delta
            }
    } else if let Some(size) = exposed_size(f, p, measured) {
        before + (value - size) / derivative.abs()
    } else {
        value
    };
    if !after.is_finite()
        || after.abs() > MAX_MM
        || (!matches!(p, "x" | "y" | "z" | "start_offset") && after <= 0.)
    {
        return Err("args.value produces an invalid feature size; size must be positive and at most 1000000 mm".into());
    }
    if after < before && matches!(p, "height" | "depth") {
        let data = form_context::payloads(request);
        if data.iter().any(|d| {
            d["id"].as_u64().is_some_and(|id| id > f.id)
                && d["definition"]["participants"]
                    .as_array()
                    .is_some_and(|bs| bs.iter().any(|b| b == &f.body))
                && ["corner", "base", "position"].iter().any(|key| {
                    point(&d["definition"][key]).is_some_and(|pos| {
                        f.direction
                            .is_some_and(|(a, _)| (pos[a] - measured).abs() < 0.001)
                    })
                })
        }) {
            return Err("Changing this support would detach a later feature; move its attachment with the support before editing".into());
        }
    }
    let attachments: Vec<_> = form_context::payloads(request)
        .iter()
        .filter(|_| matches!(p, "height" | "depth" | "start_offset"))
        .filter(|d| {
            d["id"].as_u64().is_some_and(|id| id > f.id)
                && d["definition"]["participants"]
                    .as_array()
                    .is_some_and(|bs| bs.iter().any(|b| b == &f.body))
        })
        .filter(|d| {
            ["corner", "base", "position"].iter().any(|key| {
                point(&d["definition"][key]).is_some_and(|pos| {
                    f.direction
                        .is_some_and(|(a, _)| (pos[a] - measured).abs() < 0.001)
                })
            })
        })
        .map(|d| d["name"].clone())
        .collect();
    let mut set = json!({});
    if let Some(a) = coordinate {
        let mut pos = f.definition[&path].clone();
        pos[a] = json!(after);
        set[&path] = pos;
    } else {
        let v = json!(if path == "radius" { after / 2. } else { after });
        if let Some((parent, child)) = path.split_once('.') {
            set[parent] = json!({child: v});
        } else {
            set[&path] = v;
        }
    }
    let expected = measured + derivative * (after - before);
    let mut bbox = f.bbox.clone();
    if witness["kind"] == "plane" && f.kind == "pad" {
        if let Some(a) = witness["axis"].as_u64().map(|n| n as usize) {
            for edge in ["min", "max"] {
                if number(&bbox[edge][a]).is_some_and(|v| (v - measured).abs() < 0.001) {
                    bbox[edge][a] = json!(expected);
                }
            }
        }
    }
    Ok(Plan {
        rule: "named_feature",
        calls: vec![
            call(
                catalog,
                "timeline.edit",
                json!({"feature":f.name,"set":set}),
            )?,
            call(catalog, "model.faces", json!({"body":f.body}))?,
            call(catalog, "timeline.inspect", json!({"feature":f.name}))?,
            call(catalog, "document.inspect", json!({"measure":true}))?,
        ],
        facts: None,
        slots: json!({"feature":f.name,"body":f.body,"id":f.id,"parameter":p,"field":path,"before":before,"after":after,"witness":witness,"measured_before":measured,"measured_after":expected,"attachments":attachments,"bounds_after":bbox,
            "restore_set":original_set(f, &path), "bounds_before":f.bbox,
            "args":args, "recovery_request":request,
            "size_before":exposed_size(f,p,measured).unwrap_or(before),
            "size_after":exposed_size(f,p,measured).unwrap_or(before) + derivative.abs()*(after-before),"volume_before":f.volume,
            "volume_after":f.volume.zip(volume_change(f,p,before,after,&witness)).map(|(v,d)| v+d)}),
        expected: None,
        continuation: None,
    })
}
fn original_set(f: &Feature, path: &str) -> Value {
    if let Some((parent, child)) = path.split_once('.') {
        json!({parent: {child: f.definition[parent][child]}})
    } else {
        json!({path: f.definition[path]})
    }
}
/// Restore the original field, then measure it before considering a single alternative.
pub fn restore(plan: &Plan, catalog: &Catalog) -> Result<Plan, String> {
    let mut restored = plan.clone();
    restored.slots["restoring"] = json!(true);
    for (after, before) in [
        ("after", "before"),
        ("measured_after", "measured_before"),
        ("size_after", "size_before"),
        ("volume_after", "volume_before"),
        ("bounds_after", "bounds_before"),
    ] {
        restored.slots[after] = plan.slots[before].clone();
    }
    restored.slots["attachments"] = json!([]);
    restored.calls[0] = call(
        catalog,
        "timeline.edit",
        json!({"feature":plan.slots["feature"], "set":plan.slots["restore_set"]}),
    )?;
    Ok(restored)
}
pub fn retry(plan: &Plan, catalog: &Catalog) -> Result<Option<Plan>, String> {
    if plan.slots["retried"] == true {
        return Ok(None);
    }
    let request = &plan.slots["recovery_request"];
    let fs = matching(request);
    // The plan names its resolved feature, including one chosen among several candidates.
    let Some(f) = fs.iter().find(|f| plan.slots["feature"] == f.name) else {
        return Ok(None);
    };
    let choices = preferred(request, f);
    let Some(index) = choices.iter().position(|p| plan.slots["parameter"] == *p) else {
        return Ok(None);
    };
    let Some(next) = choices.get(index + 1) else {
        return Ok(None);
    };
    let mut args = plan.slots["args"].clone();
    args["feature"] = json!(f.name);
    args["parameter"] = json!(next);
    Ok(build(&args, request, catalog).ok().map(|mut p| {
        p.slots["retried"] = json!(true);
        p
    }))
}
fn changed_payloads(request: &Value, plan: &Plan) -> Option<Vec<Value>> {
    let messages = request["messages"].as_array()?;
    let start = messages.iter().rposition(|m| {
        m["role"] == "assistant"
            && m["tool_calls"].as_array().is_some_and(|cs| {
                cs.iter().any(|c| {
                    let args = c["function"]["arguments"]
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| c["function"]["arguments"].clone());
                    args["command"] == "timeline.edit"
                        && args["params"]["feature"] == plan.slots["feature"]
                })
            })
    })?;
    let data: Vec<_> = messages[start + 1..]
        .iter()
        .filter(|m| m["role"] == "tool")
        .filter_map(tool_payload)
        .collect();
    Some(data)
}
fn measured_after(request: &Value, plan: &Plan) -> Option<f64> {
    let data = changed_payloads(request, plan)?;
    let s = &plan.slots;
    let faces = data
        .iter()
        .rev()
        .find(|d| d["body"] == s["body"] && d["faces"].is_array());
    let face = faces.and_then(|d| d["faces"].as_array()).and_then(|fs| {
        fs.iter()
            .find(|f| f["name"] == s["witness"]["face"] && f["exact"] == true)
    });
    let a = s["witness"]["axis"].as_u64().unwrap_or(0) as usize;
    face.and_then(|f| match s["witness"]["kind"].as_str() {
        Some("diameter") => number(&f["cylinder"]["radius"]).map(|r| r * 2.),
        Some("origin") => number(&f["cylinder"]["origin"][a]),
        _ => number(&f["centroid"][a]),
    })
}
pub fn completion(request: &Value, plan: &Plan) -> Option<String> {
    let data = changed_payloads(request, plan).unwrap_or_default();
    let volume_ok = volume(&plan.slots["volume_after"]).is_some_and(|expected| {
        form_context::document(&data)
            .and_then(|d| d["bodies"].as_array())
            .and_then(|bs| bs.iter().find(|b| b["name"] == plan.slots["body"]))
            .and_then(|b| volume(&b["volume_mm3"]))
            .is_some_and(|actual| {
                (actual - expected).abs()
                    <= 0.01_f64.max(
                        (expected - volume(&plan.slots["volume_before"]).unwrap_or(expected)).abs()
                            * 0.002,
                    )
            })
    });
    let bounds_ok = form_context::document(&data)
        .and_then(|d| d["bodies"].as_array())
        .and_then(|bs| bs.iter().find(|b| b["name"] == plan.slots["body"]))
        .is_some_and(|b| {
            ["min", "max"].iter().all(|edge| {
                (0..3).all(|a| {
                    number(&b["bbox"][edge][a])
                        .zip(number(&plan.slots["bounds_after"][edge][a]))
                        .is_some_and(|(actual, expected)| (actual - expected).abs() < 0.001)
                })
            })
        });
    let attachments_ok = plan.slots["attachments"]
        .as_array()
        .is_none_or(Vec::is_empty);
    if attachments_ok
        && volume_ok
        && bounds_ok
        && measured_after(request, plan)
            .zip(number(&plan.slots["measured_after"]))
            .is_some_and(|(a, b)| (a - b).abs() < 0.001)
    {
        None
    } else {
        Some("Named feature edit is unverified: measured geometry does not match the requested change. Inspect the design; do not repeat the edit.".into())
    }
}
pub fn summary(request: &Value, plan: &Plan) -> String {
    if let Some(error) = completion(request, plan) {
        return format!("Exact match (no model): {error}");
    }
    let axis = match plan.slots["witness"]["axis"].as_u64() {
        Some(1) => "Y",
        Some(2) => "Z",
        _ => "X",
    };
    let measurement = match plan.slots["witness"]["kind"].as_str() {
        Some("diameter") => "diameter".into(),
        Some("origin") => format!("axis {axis}"),
        _ => format!(
            "{} {axis}",
            if plan.slots["parameter"] == "depth" {
                "floor"
            } else {
                "surface"
            }
        ),
    };
    format!(
        "Exact match (no model): {} {}: {} → {} mm. Measured {measurement} {} → {} mm.",
        plan.slots["feature"].as_str().unwrap_or("feature"),
        plan.slots["parameter"].as_str().unwrap_or("parameter"),
        plan.slots["size_before"],
        plan.slots["size_after"],
        number(&plan.slots["measured_before"])
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null),
        measured_after(request, plan)
            .map(|v| json!(rounded(v, 4)))
            .unwrap_or(Value::Null)
    )
}
/// Development trace of each selection step, for focused tests.
#[cfg(all(test, feature = "proxy"))]
pub(crate) fn trace(request: &Value) -> Value {
    let params = [
        "height",
        "depth",
        "diameter",
        "width",
        "length",
        "x",
        "y",
        "z",
        "start_offset",
    ];
    json!({
        "candidate": candidate(user_text(request)),
        "features": features(request).iter().map(|f| json!({"name":f.name,"kind":f.kind,
            "direction":f.direction.map(|(a, s)| json!([a, s]))})).collect::<Vec<_>>(),
        "matching": matching(request).iter().map(|f| json!({"name":f.name,
            "available":params.iter().filter(|p| available(f, p)).collect::<Vec<_>>()}))
            .collect::<Vec<_>>(),
    })
}
