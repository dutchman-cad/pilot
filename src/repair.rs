//! Bounded, conservative JSON repair. No prose is ever interpreted as a command.
use serde_json::{Map, Value};

pub const MAX_BYTES: usize = 1_048_576;
pub const MAX_DEPTH: usize = 32;
pub const MAX_NODES: usize = 16_384;
pub const MAX_STEPS: usize = 256;

/// A caller can log each returned rewrite before forwarding the repaired call.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Rewrite {
    pub path: String,
    pub before: Value,
    pub after: Value,
}

pub fn strip_fence(text: &str) -> &str {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("```") {
        if let Some((_, body)) = rest.split_once('\n') {
            return body.trim_end().strip_suffix("```").unwrap_or(body).trim();
        }
    }
    text
}

fn size(text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err(format!("JSON text exceeds {MAX_BYTES} bytes"));
    }
    Ok(())
}

// Only the documented JSON5 subset: single quotes, identifier keys, trailing commas,
// and comments. No evaluation, NaN, Infinity, or arbitrary JavaScript expressions.
fn relaxed(text: &str) -> Result<String, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut string = String::new();
            let mut closed = false;
            while i < chars.len() {
                let c = chars[i];
                i += 1;
                if c == quote {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    let next = *chars.get(i).ok_or("unfinished string escape")?;
                    i += 1;
                    if next == '\'' && quote == '\'' {
                        string.push('\'');
                    } else {
                        // Decode JSON escapes using serde, preserving unicode escapes.
                        let mut escape = format!("\"\\{next}");
                        if next == 'u' {
                            for _ in 0..4 {
                                escape.push(*chars.get(i).ok_or("short unicode escape")?);
                                i += 1;
                            }
                            // Surrogate pairs must be decoded together.
                            if chars.get(i..i + 2) == Some(&['\\', 'u']) {
                                escape.push_str("\\u");
                                i += 2;
                                for _ in 0..4 {
                                    escape.push(*chars.get(i).ok_or("short unicode escape")?);
                                    i += 1;
                                }
                            }
                        }
                        escape.push('"');
                        string.push_str(
                            &serde_json::from_str::<String>(&escape).map_err(|e| e.to_string())?,
                        );
                    }
                } else if c.is_control() {
                    return Err("control character in string".into());
                } else {
                    string.push(c);
                }
            }
            if !closed {
                return Err("unterminated string".into());
            }
            out.push_str(&serde_json::to_string(&string).map_err(|e| e.to_string())?);
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push(' ');
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && chars.get(i..i + 2) != Some(&['*', '/']) {
                i += 1;
            }
            if i + 1 >= chars.len() {
                return Err("unterminated comment".into());
            }
            i += 2;
            out.push(' ');
        } else if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let mut next = i;
            while chars.get(next).is_some_and(|c| c.is_whitespace()) {
                next += 1;
            }
            if chars.get(next) == Some(&':') {
                out.push_str(&serde_json::to_string(&word).map_err(|e| e.to_string())?);
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    // Remove commas before closers without touching quoted strings.
    let mut cleaned = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let chars: Vec<char> = out.chars().collect();
    for (i, c) in chars.iter().copied().enumerate() {
        if !quoted && c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        cleaned.push(c);
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        }
    }
    Ok(cleaned)
}

// Returns the first object's extent, or an unfinished object's suffix. Quotes and
// comments are tracked so braces inside them never terminate the candidate.
fn extent(text: &str, extraction: bool) -> Result<(usize, usize, Vec<char>, Vec<usize>), String> {
    let start = if extraction {
        text.find('{').ok_or("no JSON object")?
    } else {
        0
    };
    let mut stack = Vec::new();
    let mut dropped = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut chars = text[start..].char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            continue;
        }
        if c == '/' && chars.peek().is_some_and(|(_, c)| *c == '/') {
            chars.next();
            for (_, c) in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek().is_some_and(|(_, c)| *c == '*') {
            chars.next();
            let mut closed = false;
            while let Some((_, c)) = chars.next() {
                if c == '*' && chars.peek().is_some_and(|(_, c)| *c == '/') {
                    chars.next();
                    closed = true;
                    break;
                }
            }
            if !closed {
                return Err("unterminated comment".into());
            }
            continue;
        }
        match c {
            '{' | '[' => {
                stack.push(if c == '{' { '}' } else { ']' });
                if stack.len() > MAX_DEPTH {
                    return Err("JSON nesting limit exceeded".into());
                }
            }
            '}' | ']' => {
                // A stray closer cannot close any currently open container. Keep
                // crossing brackets ambiguous, and never edit strings or comments.
                if stack.last() != Some(&c)
                    && !stack.contains(&c)
                    && (stack.is_empty() || c == ']' && stack.last() == Some(&'}'))
                {
                    dropped.push(start + offset);
                    continue;
                }
                if stack.pop() != Some(c) {
                    return Err("mismatched JSON brackets".into());
                }
                if stack.is_empty() && extraction {
                    return Ok((start, start + offset + 1, stack, dropped));
                }
            }
            _ => {}
        }
    }
    if quote.is_some() {
        return Err("unterminated JSON string".into());
    }
    Ok((start, text.len(), stack, dropped))
}

/// Strict JSON, the JSON5 subset, then repair missing or unmatched closing brackets.
pub fn parse_json(text: &str) -> Result<Value, String> {
    size(text)?;
    let text = strip_fence(text);
    let (_, _, stack, dropped) = extent(text, false)?;
    let cleaned: String = text
        .char_indices()
        .filter(|(i, _)| dropped.binary_search(i).is_err())
        .map(|(_, c)| c)
        .collect();
    let text = cleaned.as_str();
    let parsed = serde_json::from_str::<Value>(text).or_else(|_| {
        let normalized = relaxed(text).map_err(|e| e.to_string())?;
        serde_json::from_str(&normalized).map_err(|e| e.to_string())
    });
    let value = match parsed {
        Ok(value) => value,
        Err(error) if !stack.is_empty() => {
            if text.trim_end().ends_with(['{', '[', ':']) {
                return Err("cannot balance JSON without a value".into());
            }
            let mut balanced = text.to_string();
            balanced.extend(stack.into_iter().rev());
            let normalized = relaxed(&balanced)?;
            serde_json::from_str(&normalized)
                .map_err(|e| format!("invalid JSON: {error}; balancing: {e}"))?
        }
        Err(error) => return Err(format!("invalid JSON: {error}")),
    };
    check_value(&value)?;
    Ok(value)
}

/// Extract only the first explicit object; never skip a bad candidate to run a later one.
pub fn extract_json(text: &str) -> Result<Option<Value>, String> {
    extract(text, false)
}

/// Tool-call extraction requires a complete object; argument repair may balance JSON.
pub fn extract_balanced_json(text: &str) -> Result<Option<Value>, String> {
    extract(text, true)
}

/// Recovery must not silently choose one call from several explicit objects.
#[cfg(feature = "proxy")]
pub(crate) fn single_call_text(text: &str) -> bool {
    let text = strip_fence(text).trim();
    let Ok((start, end, stack, _)) = extent(text, true) else {
        return false;
    };
    stack.is_empty() && !text[..start].contains('[') && !text[end..].contains(['{', '['])
}

fn extract(text: &str, complete: bool) -> Result<Option<Value>, String> {
    size(text)?;
    let text = strip_fence(text);
    if !text.contains('{') {
        return Ok(None);
    }
    let (start, end, stack, _) = extent(text, true)?;
    if complete && !stack.is_empty() {
        return Err("text tool call must contain a balanced JSON object".into());
    }
    let value = parse_json(&text[start..end])?;
    if !value.is_object() {
        return Err("expected a JSON object".into());
    }
    Ok(Some(value))
}

pub(crate) fn check_value(value: &Value) -> Result<(), String> {
    let mut pending = vec![(value, 0)];
    let mut nodes = 0;
    let mut bytes = 0usize;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err("JSON node or nesting limit exceeded".into());
        }
        match value {
            Value::Array(items) => {
                if items.len() > MAX_NODES {
                    return Err("JSON array limit exceeded".into());
                }
                pending.extend(items.iter().map(|v| (v, depth + 1)));
            }
            Value::Object(items) => {
                if items.len() > MAX_NODES {
                    return Err("JSON object limit exceeded".into());
                }
                for (key, value) in items {
                    bytes = bytes.saturating_add(key.len());
                    pending.push((value, depth + 1));
                }
            }
            Value::String(text) => bytes = bytes.saturating_add(text.len()),
            _ => bytes = bytes.saturating_add(8),
        }
        if bytes > MAX_BYTES {
            return Err("JSON value size limit exceeded".into());
        }
    }
    Ok(())
}

/// Repairs only schema-declared containers, including each batch step's params.
/// Invalid strings and wrong types are left unchanged for downstream validation.
/// Limit failures are transactional: the original arguments remain unchanged.
pub fn repair_arguments(
    tool: &str,
    args: &mut Value,
    schema: &Value,
) -> Result<Vec<Rewrite>, String> {
    check_value(args)?;
    check_value(schema)?;
    let mut repaired = args.clone();
    let mut changes = Vec::new();
    repair_node(&mut repaired, schema, "$", 0, &mut changes)?;
    if tool == "batch" {
        if let Some(steps) = repaired.get_mut("commands").and_then(Value::as_array_mut) {
            if steps.len() > MAX_STEPS {
                return Err(format!("batch exceeds {MAX_STEPS} steps"));
            }
            for (index, step) in steps.iter_mut().enumerate() {
                if let Some(params) = step.get_mut("params") {
                    repair_node(
                        params,
                        &serde_json::json!({"type":"object"}),
                        &format!("$.commands[{index}].params"),
                        0,
                        &mut changes,
                    )?;
                }
            }
        }
    }
    check_value(&repaired)?;
    *args = repaired;
    Ok(changes)
}

fn repair_node(
    value: &mut Value,
    schema: &Value,
    path: &str,
    depth: usize,
    changes: &mut Vec<Rewrite>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("repair nesting limit exceeded".into());
    }
    let kind = schema.get("type").and_then(Value::as_str);
    if matches!(kind, Some("object" | "array")) {
        if let Some(text) = value.as_str() {
            size(text)?;
            let text = strip_fence(text);
            // Parameter containers must parse completely, without balancing.
            let parsed = serde_json::from_str::<Value>(text).or_else(|_| {
                relaxed(text)
                    .map_err(serde::de::Error::custom)
                    .and_then(|s| serde_json::from_str(&s))
            });
            if let Ok(parsed) = parsed {
                if (kind == Some("object") && parsed.is_object())
                    || (kind == Some("array") && parsed.is_array())
                {
                    if changes.len() >= MAX_NODES {
                        return Err("rewrite count limit exceeded".into());
                    }
                    changes.push(Rewrite {
                        path: path.to_string(),
                        before: value.clone(),
                        after: parsed.clone(),
                    });
                    *value = parsed;
                }
            }
        }
    }
    if let Some(items) = value.as_object_mut() {
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            repair_properties(items, properties, path, depth, changes)?;
        }
    } else if let Some(items) = value.as_array_mut() {
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in items.iter_mut().enumerate() {
                repair_node(
                    item,
                    item_schema,
                    &format!("{path}[{index}]"),
                    depth + 1,
                    changes,
                )?;
            }
        }
    }
    Ok(())
}

fn repair_properties(
    items: &mut Map<String, Value>,
    properties: &Map<String, Value>,
    path: &str,
    depth: usize,
    changes: &mut Vec<Rewrite>,
) -> Result<(), String> {
    for (key, value) in items {
        if let Some(schema) = properties.get(key) {
            repair_node(value, schema, &format!("{path}.{key}"), depth + 1, changes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_explicit_shapes_and_first_object() {
        for value in [
            json!({"tool":"execute","args":{"command":"solid.box"}}),
            json!({"name":"execute","arguments":{"command":"solid.box"}}),
            json!({"function":{"name":"execute","arguments":{"command":"solid.box"}}}),
            json!({"execute":{"command":"solid.box"}}),
        ] {
            let text =
                format!("Here is the call:\n```json\n{value}\n```\nthen {{\"ignored\":true}}");
            assert_eq!(extract_json(&text).unwrap(), Some(value));
        }
        assert_eq!(extract_json("I'll extrude it 10 mm").unwrap(), None);
        assert!(extract_json("{bad} then {\"ok\":true}").is_err());
        assert!(extract_balanced_json("{\"tool\":\"inspect\"").is_err());
        assert_eq!(
            extract_json("{\"text\":\"escaped \\\" } {\",\"x\":1} {} ").unwrap(),
            Some(json!({"text":"escaped \" } {","x":1}))
        );
    }

    #[test]
    fn strict_relaxed_fenced_and_balanced() {
        for text in [
            "{\"a\":[1,2]}",
            "```json\n{\"a\":[1,2]}\n```",
            "{a: [1, 2,],}",
            "{a: [1, 2",
            "{a: [1, 2,",
            "{a: [1, 2]]}",
            "{a: [1, 2]}]",
            "{/* } ignored */ a: [1, // ] ignored\n2]}",
        ] {
            assert_eq!(parse_json(text).unwrap(), json!({"a":[1,2]}), "{text}");
        }
        assert_eq!(
            parse_json("{'a':'it\\'s \\\"fine\\\"', key: 'é'}").unwrap(),
            json!({"a":"it's \"fine\"", "key":"é"})
        );
        assert_eq!(
            parse_json(r#"{'s':'\uD83D\uDE00','t':'\n'}"#).unwrap(),
            json!({"s":"😀","t":"\n"})
        );
        assert_eq!(
            parse_json(r#"{"commas":",] ,}","braces":"[{}]"}"#).unwrap(),
            json!({"commas":",] ,}","braces":"[{}]"})
        );
        assert_eq!(
            extract_json("prefix {a:[1,2").unwrap(),
            Some(json!({"a":[1,2]}))
        );
    }

    #[test]
    fn refuses_unsafe_or_ambiguous_repairs() {
        for text in [
            "{a:}",
            "[1}",
            "{a:[1}]} ",
            "{a:'unfinished}",
            "{'x':'bad\\q'}",
            "{a:NaN}",
            "{a:Infinity}",
            "{a:run()}",
            "{a:1} trailing",
            "{/* unfinished",
            "{a:1,,}",
            "{a:1 b:2}",
        ] {
            assert!(parse_json(text).is_err(), "{text}");
        }
        assert!(parse_json(&"[".repeat(MAX_DEPTH + 1)).is_err());
        assert!(extract_json(&" ".repeat(MAX_BYTES + 1)).is_err());
        assert!(parse_json(&format!("[{}]", "0,".repeat(MAX_NODES))).is_err());
    }

    #[test]
    fn containers_must_parse_exactly_and_preserve_dimensions_and_units() {
        let schema = json!({"type":"object","properties":{
            "point":{"type":"array"}, "options":{"type":"object"}, "distance":{"type":"string"}
        }});
        for text in ["[15,25", "[15,25]]", "[15,25] trailing", "{x:15}"] {
            let mut args = json!({"point":text});
            let before = args.clone();
            assert!(repair_arguments("execute", &mut args, &schema)
                .unwrap()
                .is_empty());
            assert_eq!(args, before);
        }
        let mut args = json!({"point":"[15,25,0]", "options":"{unit:'in',}", "distance":"2 in"});
        let changes = repair_arguments("execute", &mut args, &schema).unwrap();
        assert_eq!(
            args,
            json!({"point":[15,25,0],"options":{"unit":"in"},"distance":"2 in"})
        );
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn schema_directed_repairs_and_logging() {
        let schema = json!({"type":"object","properties":{
            "objects":{"type":"array","items":{"type":"object","properties":{"nested":{"type":"object"}}}},
            "object":{"type":"object"}, "scalar":{"type":"string"}, "number":{"type":"number"}
        }});
        let mut args = json!({"objects":"```json\n[{nested: '{key: 1}'}]\n```", "object":"{a:[1,2", "scalar":"[1]", "number":"3", "unknown":"{}"});
        let changes = repair_arguments("execute", &mut args, &schema).unwrap();
        assert_eq!(
            args,
            json!({"objects":[{"nested":{"key":1}}],"object":"{a:[1,2","scalar":"[1]","number":"3","unknown":"{}"})
        );
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| c.path == "$.objects[0].nested"));
        assert!(changes
            .iter()
            .all(|c| c.before.is_string() && (c.after.is_object() || c.after.is_array())));
        assert!(repair_arguments("execute", &mut args, &schema)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn wrong_types_and_invalid_strings_stay_intact() {
        for kind in ["object", "array"] {
            let schema = json!({"properties":{"arg":{"type":kind}}});
            for text in [
                "[",
                "{",
                "not JSON",
                "3",
                "null",
                "true",
                "\"{}\"",
                "{broken}",
                "[1}",
                "{'x':'unfinished}",
            ] {
                let mut args = json!({"arg":text});
                let original = args.clone();
                assert!(repair_arguments("execute", &mut args, &schema)
                    .unwrap()
                    .is_empty());
                assert_eq!(args, original);
            }
            let mut args = json!({"arg": if kind == "object" { "[]" } else { "{}" }});
            let original = args.clone();
            repair_arguments("execute", &mut args, &schema).unwrap();
            assert_eq!(args, original);
        }
    }

    #[test]
    fn batch_params_and_transactional_caps() {
        let schema = json!({"properties":{"commands":{"type":"array"}}});
        let mut args = json!({"commands": r#"[{command:'engine.describe',params:'{command: \'solid.box\'}'}]"#});
        let changes = repair_arguments("batch", &mut args, &schema).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(
            args["commands"][0]["params"],
            json!({"command":"solid.box"})
        );
        let mut args = json!({"commands":vec![json!({"params":"{}"}); MAX_STEPS + 1]});
        let original = args.clone();
        assert!(repair_arguments("batch", &mut args, &schema).is_err());
        assert_eq!(args, original);
        let mut args = json!({"arg":"x".repeat(MAX_BYTES + 1)});
        assert!(repair_arguments("execute", &mut args, &schema).is_err());
    }
}
