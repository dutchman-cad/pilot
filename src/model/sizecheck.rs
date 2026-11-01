//! Conservative completion feedback from stated overall sizes and public bounds.
use super::*;

#[derive(Clone, Default)]
pub(super) struct State {
    index: Option<usize>,
    read: bool,
    pub handed_back: bool,
}
impl State {
    pub fn reset(&mut self, index: usize, fresh: bool) {
        if fresh || self.index != Some(index) {
            *self = Self {
                index: Some(index),
                ..Self::default()
            };
        }
    }
}

struct Dimension {
    start: usize,
    end: usize,
    value: f64,
    factor: Option<f64>,
}

fn stated(text: &str) -> Option<Vec<f64>> {
    if text.len() > 8192 || !body_hint::allows_one_part(text) {
        return None;
    }
    let text = text.to_lowercase();
    let words: Vec<_> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if ["at least", "at most", "up to"]
        .iter()
        .any(|phrase| text.contains(phrase))
    {
        return None;
    }
    // These contexts can describe several objects or a nominal/inside size.
    // Deliberately leave them to the model rather than infer which number is the envelope.
    if words.iter().any(|w| {
        matches!(
            *w,
            "inside"
                | "inner"
                | "internal"
                | "clearance"
                | "approximately"
                | "about"
                | "roughly"
                | "around"
                | "maybe"
                | "or"
                | "between"
                | "each"
                | "both"
                | "bars"
                | "rods"
                | "plates"
                | "boxes"
                | "blocks"
                | "cylinders"
                | "cubes"
                | "and"
                | "plus"
                | "then"
                | "another"
                | "second"
                | "lid"
                | "cover"
                | "nominal"
                | "maximum"
                | "minimum"
                | "two"
                | "three"
                | "four"
                | "five"
                | "six"
                | "several"
                | "multiple"
                | "profile"
                | "rectangle"
                | "not"
                | "never"
                | "avoid"
                | "unsure"
                | "uncertain"
                | "within"
                | "fit"
                | "fits"
        )
    }) {
        return None;
    }
    let products = [
        "bar",
        "block",
        "plate",
        "box",
        "enclosure",
        "bracket",
        "beam",
        "strip",
        "slab",
        "rod",
        "cylinder",
    ];
    if words.iter().filter(|w| products.contains(w)).count() > 1 {
        return None;
    }
    let mut dims = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let tail = &text[i..];
        let c = tail.chars().next()?;
        if c.is_ascii_digit()
            || (c == '.' && tail.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
        {
            if i > 0
                && text[..i].chars().next_back().is_some_and(|c| {
                    c.is_alphanumeric() && c != 'x' || matches!(c, '-' | '/' | '.')
                })
            {
                return None;
            }
            let (value, len) = crate::unitfact::dimension_number(tail)?;
            let end = i + len;
            let unit_start = end + text[end..].len() - text[end..].trim_start().len();
            let unit = crate::unitfact::dimension_unit(&text[unit_start..]);
            let end = unit.map_or(end, |(_, n)| unit_start + n);
            dims.push(Dimension {
                start: i,
                end,
                value,
                factor: unit.map(|(f, _)| f),
            });
            if dims.len() > 16 {
                return None;
            }
            i = end;
        } else {
            i += c.len_utf8();
        }
    }
    let feature_words = [
        "hole", "holes", "slot", "slots", "boss", "bosses", "rib", "ribs", "fillet", "chamfer",
        "pocket", "cutout", "sketch", "circle", "radius",
    ];
    let mut candidates = Vec::new();
    for ds in dims.windows(3) {
        if !ds
            .windows(2)
            .all(|d| matches!(text[d[0].end..d[1].start].trim(), "x" | "×" | "by"))
        {
            continue;
        }
        let before = text[..ds[0].start].trim_end();
        let after = text[ds[2].end..].trim_start();
        let next_word = after
            .split(|c: char| !c.is_alphabetic())
            .next()
            .unwrap_or("");
        let previous_word = before.split_whitespace().next_back().unwrap_or("");
        if feature_words.contains(&next_word) {
            continue;
        }
        let explicit = ["overall", "overall size", "overall dimensions"]
            .iter()
            .any(|label| before.ends_with(label));
        // A triple must be directly attached to the product, not merely somewhere
        // in a request containing a product noun (e.g. a plate's rectangular slot).
        if !products.contains(&next_word) && !products.contains(&previous_word) && !explicit {
            continue;
        }
        if before
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| feature_words.contains(&w))
        {
            continue;
        }
        if !separate_features(after, &feature_words) {
            continue;
        }
        let Some(unit) = ds[2].factor else {
            continue;
        };
        if !ds.iter().all(|d| d.factor.is_some()) && ds[..2].iter().any(|d| d.factor.is_some()) {
            continue;
        }
        candidates.push(
            ds.iter()
                .map(|d| d.value * d.factor.unwrap_or(unit))
                .collect::<Vec<_>>(),
        );
    }
    // A single explicit overall dimension can coexist with other feature sizes,
    // provided its own clause names the product, or has no feature context.
    for d in &dims {
        let before = text[..d.start].trim_end();
        let after = text[d.end..].trim();
        let labels = [
            "overall length",
            "overall width",
            "overall height",
            "overall thickness",
            "overall diameter",
        ];
        let labelled = labels.iter().any(|label| {
            after
                .strip_prefix(label)
                .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()))
                || before.ends_with(label)
        });
        if !labelled {
            continue;
        }
        let next_word = after
            .split(|c: char| !c.is_alphabetic())
            .next()
            .unwrap_or("");
        let labelled_feature = labels
            .iter()
            .filter_map(|label| after.strip_prefix(label))
            .any(|rest| {
                let word = rest
                    .trim_start()
                    .split(|c: char| !c.is_alphabetic())
                    .next()
                    .unwrap_or("");
                feature_words.contains(&word)
            });
        if feature_words.contains(&next_word)
            || labelled_feature
            || !separate_features(after, &feature_words)
        {
            continue;
        }
        let clause = before.rsplit(['.', ';', '\n']).next().unwrap_or(before);
        if clause
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| feature_words.contains(&w))
        {
            continue;
        }
        if let Some(factor) = d.factor {
            candidates.push(vec![d.value * factor]);
        }
    }
    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

fn recipe_turn(request: &Value) -> bool {
    let Some(messages) = request["messages"].as_array() else {
        return false;
    };
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    messages
        .iter()
        .skip(start)
        .filter_map(|m| m["tool_calls"].as_array())
        .flatten()
        .any(|c| {
            matches!(
                c["function"]["name"].as_str(),
                Some("pilot_fill" | "pilot_build")
            ) || c["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("pilot_direct_"))
        })
}

// A feature after a size needs an explicit separator, e.g. "bar with a slot".
// "overall length of a slot" still describes the feature, not the product.
fn separate_features(after: &str, feature_words: &[&str]) -> bool {
    let words: Vec<_> = after
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let Some(feature) = words.iter().position(|w| feature_words.contains(w)) else {
        return true;
    };
    words[..feature].contains(&"with")
}

pub(super) fn eligible(request: &Value) -> bool {
    !recipe_turn(request)
        && stated(last_user(request)).is_some()
        && request["tools"]
            .as_array()
            .is_some_and(|ts| ts.iter().any(|t| t["function"]["name"] == "execute"))
}

fn has_bounds(document: &Value) -> bool {
    document["bodies"].as_array().is_some_and(|bodies| {
        bodies.len() != 1
            || (bodies[0]["bbox"]["min"].is_array() && bodies[0]["bbox"]["max"].is_array())
    })
}

fn pilot_measured_this_turn(request: &Value) -> bool {
    let Some(messages) = request["messages"].as_array() else {
        return false;
    };
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    messages
        .iter()
        .skip(start)
        .filter_map(|m| m["tool_calls"].as_array())
        .flatten()
        .any(|c| {
            if !c["id"].as_str().is_some_and(|id| id.starts_with("pilot_"))
                || c["function"]["name"] != "execute"
            {
                return false;
            }
            let args = &c["function"]["arguments"];
            let parsed = args
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok());
            let args = parsed.as_ref().unwrap_or(args);
            args["command"] == "document.inspect" && args["params"]["measure"] == true
        })
}

/// Called only after an actual final reply, never instead of a model call.
pub(super) fn prepare(
    request: &Value,
    state: &mut State,
    catalog: &Catalog,
    audit: &mut Audit,
) -> Result<Option<Value>, String> {
    let (document, acted) = body_hint::size_evidence(request);
    if document.as_ref().is_some_and(has_bounds)
        || !acted
        || state.read
        || pilot_measured_this_turn(request)
    {
        return Ok(None);
    }
    if catalog.get("document.inspect").is_none() {
        return Ok(None);
    }
    state.read = true;
    let n = request["messages"].as_array().map_or(0, Vec::len);
    let message = json!({"role":"assistant","content":null,"tool_calls":[{
        "id":format!("pilot_sizecheck_{n}"),"type":"function","function":{"name":"execute",
        "arguments":json!({"command":"document.inspect","params":{"measure":true}}).to_string()}}]});
    audit.record("size_check_read", &Value::Null, &message)?;
    Ok(Some(message))
}

pub(super) fn completion(request: &Value) -> Option<String> {
    let mut expected = stated(last_user(request))?;
    let (document, acted) = body_hint::size_evidence(request);
    if !acted {
        return None;
    }
    let document = document?;
    let bodies = document["bodies"].as_array()?;
    if bodies.len() != 1 || bodies[0]["closed"] == false {
        return None;
    }
    let bbox = &bodies[0]["bbox"];
    let mut measured = Vec::new();
    for i in 0..3 {
        let min = bbox["min"][i].as_f64()?;
        let max = bbox["max"][i].as_f64()?;
        let size = max - min;
        if !min.is_finite() || !max.is_finite() || !size.is_finite() || size <= 0.0 {
            return None;
        }
        measured.push(size);
    }
    let close = |a: f64, b: f64| (a - b).abs() <= 0.01_f64.max(b.abs() * 0.001);
    expected.sort_by(f64::total_cmp);
    measured.sort_by(f64::total_cmp);
    let matches = if expected.len() == 1 {
        measured.iter().any(|m| close(*m, expected[0]))
    } else {
        measured.iter().zip(&expected).all(|(m, e)| close(*m, *e))
    };
    if matches {
        return None;
    }
    let show = |v: &[f64]| {
        v.iter()
            .map(|n| {
                format!("{n:.4}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join(" x ")
    };
    Some(format!("[dutchman-pilot hint] Overall size check: measured body bounds {} mm; stated overall size {} mm (any axis order). Check the overall dimensions before finishing; do not repeat accepted changes.", show(&measured), show(&expected)))
}
