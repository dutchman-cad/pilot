//! Small length lexer. Counts and angles are parsed separately by their recipes.
use super::MAX_MM;
#[derive(Clone, Copy, Debug)]
pub(super) struct Dimension {
    pub value: f64,
    pub index: usize,
}
pub(super) fn tokens(text: &str) -> Vec<String> {
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
                ',' | ';' | ':' => out.push("and".into()),
                'Ø' | 'ø' => out.push("diameter".into()),
                '-' | '/' => out.push(c.to_string()),
                _ => (),
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
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
    let verb = t
        .iter()
        .find(|s| !matches!(s.as_str(), "please" | "can" | "could" | "you"));
    if verb.is_some_and(|s| matches!(s.as_str(), "rename" | "call" | "name")) {
        if let Some((end, quote)) = text
            .char_indices()
            .rfind(|(_, c)| matches!(c, '"' | '\'' | '`'))
        {
            if let Some(start) = text[..end].rfind(quote) {
                if text[end + quote.len_utf8()..]
                    .trim_matches(|c: char| {
                        c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?')
                    })
                    .is_empty()
                {
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
                    | "height"
                    | "length"
                    | "width"
                    | "depth"
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
    // A following label belongs to the next value in label-first prose.
    // Otherwise it belongs to this value ("7 mm in diameter and ...").
    let suffix = t
        .get(after)
        .filter(|s| all_labels.contains(&s.as_str()))
        .filter(|_| {
            !t.get(after + 1)
                .is_some_and(|s| number(s).is_some() || s == "of")
        });
    let mut before = d.index;
    if before > 0 && t[before - 1] == "of" {
        before -= 1;
    }
    let prefix = before
        .checked_sub(1)
        .and_then(|i| t.get(i))
        .filter(|s| all_labels.contains(&s.as_str()));
    match (prefix, suffix) {
        (Some(a), Some(b)) if a != b => false,
        (Some(s), _) | (_, Some(s)) => labels.contains(&s.as_str()),
        _ => false,
    }
}
