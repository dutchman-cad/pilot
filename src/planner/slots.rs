//! Small length lexer. Counts and angles are parsed separately by their recipes.
use super::MAX_MM;
#[derive(Clone, Copy, Debug)]
pub(super) struct Dimension {
    pub value: f64,
    pub index: usize,
}
pub(super) fn tokens(text: &str) -> Vec<String> {
    let mut guarded = text.to_ascii_lowercase().replace('’', "'");
    for contraction in [
        "can't",
        "won't",
        "isn't",
        "shouldn't",
        "wouldn't",
        "couldn't",
        "mustn't",
        "shouldnt",
        "wont",
        "isnt",
    ] {
        guarded = guarded.replace(contraction, "not");
    }
    let text = guarded.as_str();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut numeric = false;
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        let next_digit = chars.peek().is_some_and(char::is_ascii_digit);
        let number = c.is_ascii_digit()
            || (c == '.' && next_digit)
            || (c == '-' && (next_digit || chars.peek() == Some(&'.')))
            || (c == '/' && numeric && !current.is_empty());
        if c.is_ascii_alphanumeric() || c == '_' || number {
            if !current.is_empty() && numeric != number {
                out.push(std::mem::take(&mut current));
            }
            numeric = number;
            current.push(c.to_ascii_lowercase());
        } else {
            let attached_number = numeric && !current.is_empty();
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            match c {
                // A quote attached to a number is an inch mark. Word quotes
                // are punctuation, including a closing quote after a unit.
                '"' if attached_number && !quoted => out.push("in".into()),
                '"' => quoted = !quoted,
                '×' => out.push("x".into()),
                ',' | ';' => out.push("and".into()),
                'Ø' | 'ø' => out.push("diameter".into()),
                '-' | '/' => out.push(c.to_string()),
                _ => (),
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.into_iter()
        .filter_map(|word| {
            // Small, deliberately documented politeness list; semantic words remain.
            if filler(&word) {
                return None;
            }
            Some(
                match word.as_str() {
                    "ball" => "sphere",
                    "puck" | "coin" | "disk" => "disc",
                    "bar" | "brick" => "block",
                    "slab" | "sheet" => "plate",
                    "every" => "all",
                    "edge" => "edges",
                    "this" => "the",
                    "solids" => "bodies",
                    "millimeters" | "millimetres" => "mm",
                    "centimeters" | "centimetres" => "cm",
                    "meters" | "metres" => "m",
                    _ => return Some(word),
                }
                .to_string(),
            )
        })
        .collect()
}
fn filler(word: &str) -> bool {
    matches!(
        word,
        "please"
            | "pls"
            | "thanks"
            | "thank"
            | "you"
            | "can"
            | "could"
            | "i"
            | "need"
            | "want"
            | "give"
            | "me"
            | "ok"
            | "okay"
            | "so"
    )
}
/// A quoted rename destination is data, including words which name actions.
/// Only hide a complete final quoted name; trailing instructions stay visible.
pub(super) fn intent_tokens(text: &str) -> Vec<String> {
    let t = tokens(text);
    if let Some(range) = rename_name_range(text, &t) {
        return tokens(&text[..range.start]);
    }
    t
}
pub(super) fn has_question(text: &str) -> bool {
    if let Some(range) = rename_name_range(text, &tokens(text)) {
        return text[..range.start].contains('?') || text[range.end..].contains('?');
    }
    text.contains('?')
}
fn rename_name_range(text: &str, t: &[String]) -> Option<std::ops::Range<usize>> {
    if t.iter()
        .any(|s| matches!(s.as_str(), "rename" | "call" | "name"))
    {
        if let Some((end, quote)) = text
            .char_indices()
            .rfind(|(_, c)| matches!(c, '"' | '\'' | '`'))
        {
            if let Some(start) = text[..end].rfind(quote) {
                if tokens(&text[end + quote.len_utf8()..]).is_empty() {
                    return Some(start..end + quote.len_utf8());
                }
            }
        }
    }
    None
}
fn number(s: &str) -> Option<f64> {
    if let Some((n, d)) = s.split_once('/') {
        let n = n.parse::<f64>().ok()?;
        let d = d.parse::<f64>().ok()?;
        if d <= 0.0 {
            return None;
        }
        Some(n / d)
    } else {
        s.parse().ok()
    }
}
pub(super) fn scale(s: &str) -> Option<f64> {
    match s {
        "mm" => Some(1.0),
        "cm" => Some(10.0),
        "m" => Some(1000.0),
        "in" | "inch" | "inches" => Some(25.4),
        _ => None,
    }
}
pub(super) fn dimensions(t: &[String]) -> Option<Vec<Dimension>> {
    let mut dims = Vec::new();
    let mut units = Vec::new();
    for (i, word) in t.iter().enumerate() {
        if scale(word).is_some() {
            let previous = i.checked_sub(1).and_then(|i| t.get(i));
            if previous.is_none_or(|s| number(s).is_none())
                && (word != "in"
                    || (previous.is_some_and(|s| scale(s).is_some())
                        && !t.get(i + 1).is_some_and(|s| {
                            matches!(
                                s.as_str(),
                                "a" | "an"
                                    | "the"
                                    | "rectangular"
                                    | "circular"
                                    | "diameter"
                                    | "radius"
                                    | "od"
                                    | "id"
                                    | "from"
                                    | "both"
                            )
                        })))
            {
                return None;
            }
        }
        let Some(n) = number(word) else {
            if word.chars().any(|c| c.is_ascii_digit()) || word.contains('/') {
                return None;
            }
            continue;
        };
        let next = t.get(i + 1).map(String::as_str).unwrap_or("");
        let unit = scale(next);
        // Unknown units and adjacent numbers (including mixed fractions) are refused.
        if unit.is_none()
            && !matches!(
                next,
                "" | "x"
                    | "by"
                    | "and"
                    | "along"
                    | "on"
                    | "tall"
                    | "high"
                    | "wide"
                    | "long"
                    | "thick"
                    | "deep"
                    | "across"
                    | "diameter"
                    | "radius"
                    | "od"
                    | "id"
                    | "height"
                    | "length"
                    | "width"
                    | "depth"
                    | "thickness"
                    | "flange"
                    | "flanges"
                    | "stem"
                    | "web"
                    | "leg"
                    | "legs"
                    | "cap"
                    | "pad"
                    | "horizontal"
                    | "vertical"
                    | "upright"
                    | "bottom"
                    | "offset"
                    | "overall"
                    | "opening"
                    | "opens"
                    | "faces"
                    | "facing"
                    | "cube"
                    | "box"
                    | "block"
                    | "sphere"
                    | "cylinder"
                    | "rod"
                    | "disc"
                    | "plate"
                    | "with"
                    | "wall"
                    | "rectangle"
                    | "circle"
                    | "profile"
                    | "then"
                    | "extrude"
                    | "around"
                    | "about"
                    | "edges"
                    | "all"
                    | "top"
                    | "hole"
                    | "through"
                    | "centered"
                    | "centred"
                    | "round"
                    | "origin"
                    | "factor"
                    | "open"
                    | "remove"
                    | "from"
            )
        {
            return None;
        }
        dims.push(Dimension { value: n, index: i });
        units.push(unit);
    }
    if dims.len() > 128 {
        return None;
    }
    // Only a trailing unit on a contiguous x/by tuple is inherited.
    for start in 0..dims.len() {
        if units[start].is_some() {
            continue;
        }
        for end in start + 1..dims.len() {
            let slice = &t[dims[end - 1].index + 1..dims[end].index];
            if slice.len() != 1 || !matches!(slice[0].as_str(), "x" | "by") {
                break;
            }
            if let Some(unit) = units[end] {
                units[start] = Some(unit);
                break;
            }
        }
    }
    for (d, unit) in dims.iter_mut().zip(units) {
        d.value *= unit.unwrap_or(1.0);
        if !d.value.is_finite() || d.value.abs() > MAX_MM {
            return None;
        }
    }
    Some(dims)
}
pub(super) fn labelled(t: &[String], d: Dimension, labels: &[&str]) -> bool {
    let all_labels = [
        "radius",
        "od",
        "id",
        "diameter",
        "across",
        "height",
        "tall",
        "high",
        "length",
        "long",
        "width",
        "wide",
        "depth",
        "deep",
        "thickness",
        "thick",
        "wall",
        "distance",
    ];
    let mut after = d.index + 1;
    if t.get(after).is_some_and(|s| scale(s).is_some()) {
        after += 1;
    }
    if t.get(after).is_some_and(|s| s == "in") {
        after += 1;
    }
    let first_number = t
        .iter()
        .position(|s| number(s).is_some())
        .unwrap_or(d.index);
    let value_first = !t[..first_number]
        .iter()
        .any(|s| all_labels.contains(&s.as_str()));
    // A following label belongs to the next value in label-first prose.
    // Otherwise it belongs to this value ("7 mm in diameter and ...").
    let suffix = t
        .get(after)
        .filter(|s| all_labels.contains(&s.as_str()))
        .filter(|_| {
            value_first
                || !t
                    .get(after + 1)
                    .is_some_and(|s| number(s).is_some() || s == "of")
        });
    let mut before = d.index;
    if before > 0 && t[before - 1] == "of" {
        before -= 1;
    }
    let mut prefix = before
        .checked_sub(1)
        .and_then(|i| t.get(i))
        .filter(|s| all_labels.contains(&s.as_str()));
    if value_first
        && suffix.is_some()
        && before >= 2
        && (number(&t[before - 2]).is_some() || scale(&t[before - 2]).is_some())
    {
        prefix = None;
    }
    match (prefix, suffix) {
        (Some(a), Some(b)) if a != b => false,
        (Some(s), _) | (_, Some(s)) => labels.contains(&s.as_str()),
        _ => false,
    }
}

/// Phrase normalization keeps coordinates, quoted names and decimal punctuation intact.
pub(super) fn normalize(text: &str) -> String {
    let mut lower = text.replace('’', "'");
    // Scope the modal exception to polite requests; keep other `would` guards.
    lower = replace_phrase(&lower, "would you", "can you");
    for contraction in [
        "can't",
        "won't",
        "isn't",
        "shouldn't",
        "wouldn't",
        "couldn't",
        "mustn't",
    ] {
        lower = replace_phrase(&lower, contraction, "not");
    }
    // Rename destinations are opaque user data, retaining case and punctuation.
    if tokens(text)
        .iter()
        .any(|s| matches!(s.as_str(), "rename" | "call" | "name"))
    {
        return text.to_string();
    }
    let mut out = String::new();
    let mut word = String::new();
    for c in lower.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphabetic() {
            word.push(c);
        } else {
            if !filler(&word.to_ascii_lowercase()) {
                out.push_str(&word);
            }
            word.clear();
            out.push(c);
        }
    }
    let mut out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [
        ("outer diameter", "od"),
        ("outside diameter", "od"),
        ("inner diameter", "id"),
        ("inside diameter", "id"),
        ("wall thickness", "wall"),
        ("pull it up", "extrude"),
        ("raise", "extrude"),
        ("extrude by", "extrude"),
        ("along -", "along negative "),
        ("down x", "negative x"),
        ("down y", "negative y"),
        ("down z", "negative z"),
        ("not keeping the original", "discard original"),
        ("without keeping the original", "discard original"),
        ("do not keep the original", "discard original"),
    ] {
        out = replace_phrase(&out, from, to);
    }
    out
}

fn replace_phrase(text: &str, from: &str, to: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::new();
    let mut previous = 0;
    for (start, _) in lower.match_indices(from) {
        let end = start + from.len();
        if text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphabetic())
            || (from.ends_with(|c: char| c.is_alphabetic())
                && text[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic()))
        {
            continue;
        }
        out.push_str(&text[previous..start]);
        out.push_str(to);
        previous = end;
    }
    out.push_str(&text[previous..]);
    out
}
