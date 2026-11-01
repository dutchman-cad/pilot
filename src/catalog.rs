//! Deterministic command discovery from public catalogue JSON.
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
mod holes;

pub const MAX_CATALOG_BYTES: usize = 8 * 1_048_576;
pub const MAX_COMMANDS: usize = 4096;
pub const MAX_QUERY_BYTES: usize = 256;
const SNAPSHOT: &str = include_str!("../tests/fixtures/commands.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub params: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    commands: Vec<Command>,
}

#[derive(Debug, Serialize)]
pub struct Match {
    pub command: Command,
    /// A concrete example call, or null when references or choices are unresolved.
    pub example: Value,
    /// Non-executable typed placeholders, separate from concrete examples.
    pub template: Value,
}

#[derive(Debug, Serialize)]
pub struct How {
    pub message: String,
    pub matches: Vec<Match>,
    pub nearest: Vec<String>,
}

impl Catalog {
    /// Accept CLI arrays, {commands}, JSON-RPC results, and MCP text/structured content.
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > MAX_CATALOG_BYTES {
            return Err("catalogue exceeds 8 MiB".into());
        }
        let value: Value =
            serde_json::from_str(text).map_err(|e| format!("invalid catalogue JSON: {e}"))?;
        let entries = entries(value, 0)?;
        if entries.len() > MAX_COMMANDS {
            return Err(format!("catalogue exceeds {MAX_COMMANDS} commands"));
        }
        let mut commands = Vec::new();
        let mut seen = BTreeSet::new();
        for entry in entries {
            let command: Command = serde_json::from_value(entry)
                .map_err(|e| format!("invalid command record: {e}"))?;
            if command.id.is_empty()
                || command.id.len() > 128
                || command.label.len() > 1024
                || command.params.len() > 32768
                || command
                    .disabled_reason
                    .as_ref()
                    .is_some_and(|s| s.len() > 32768)
            {
                return Err("command fields exceed catalogue limits".into());
            }
            if !seen.insert(command.id.clone()) {
                return Err(format!("duplicate command id: {}", command.id));
            }
            commands.push(command);
        }
        commands.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self { commands })
    }

    pub fn bundled() -> Result<Self, String> {
        Self::from_json(SNAPSHOT)
    }
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
    pub fn get(&self, id: &str) -> Option<&Command> {
        self.commands.iter().find(|c| c.id == id)
    }

    /// Conservative schema from the public catalogue's parameter notation.
    /// Expression parameters deliberately have no scalar type restriction.
    pub fn parameter_schema(&self, id: &str) -> Value {
        parameter_schema(self.get(id).map_or("", |command| command.params.as_str()))
    }

    /// Normalized equality wins, then edit distance, then shared id words; at most three.
    pub fn closest(&self, query: &str) -> Result<Vec<String>, String> {
        query_limit(query)?;
        let normalized = normalize(query);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let query_words = words(query);
        let mut candidates: Vec<_> = self
            .commands
            .iter()
            .map(|command| {
                let id = normalize(&command.id);
                let shared = words(&command.id).intersection(&query_words).count();
                (
                    (
                        id != normalized,
                        distance(&normalized, &id),
                        std::cmp::Reverse(shared),
                        &command.id,
                    ),
                    command,
                )
            })
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(candidates
            .into_iter()
            .take(3)
            .map(|(_, c)| c.id.clone())
            .collect())
    }

    pub fn search(&self, task: &str) -> Result<Vec<&Command>, String> {
        Ok(self
            .search_scores(task)?
            .into_iter()
            .take(3)
            .map(|(_, c)| c)
            .collect())
    }

    pub(crate) fn search_scores(&self, task: &str) -> Result<Vec<(usize, &Command)>, String> {
        query_limit(task)?;
        let tokens: Vec<(String, usize)> = words(task)
            .into_iter()
            .filter(|w| !stop_word(w) && w.parse::<f64>().is_err())
            .map(|w| {
                let mapped = synonym(&w);
                (mapped.to_string(), if mapped == w { 1 } else { 2 })
            })
            .collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let mut ranked = Vec::new();
        for command in &self.commands {
            let ids = words(&command.id);
            let labels = words(&command.label);
            let docs = words(&command.params);
            let mut score = 0;
            let mut matched = 0;
            for (token, boost) in &tokens {
                let weight = if ids.contains(token) {
                    12
                } else if labels.contains(token) {
                    10
                } else if docs.contains(token) {
                    1
                } else {
                    0
                };
                score += weight * boost;
                matched += usize::from(weight > 0);
            }
            // A command name is strong evidence; incidental documentation alone needs
            // all meaningful query words, so "make it fly" never executes a guess.
            if score >= 10 || (matched == tokens.len() && score >= 1) {
                ranked.push((score, command));
            }
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        Ok(ranked)
    }

    /// Combine task evidence from `how` with conservative id similarity.
    pub fn suggestions(&self, id: &str, task: &str) -> Result<Vec<String>, String> {
        query_limit(id)?;
        let task: String = task
            .chars()
            .scan(0usize, |bytes, c| {
                *bytes += c.len_utf8();
                (*bytes <= MAX_QUERY_BYTES).then_some(c)
            })
            .collect();
        let relevance = self.search_scores(&task)?;
        let query = normalize(id);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut ranked = Vec::new();
        for command in &self.commands {
            let candidate = normalize(&command.id);
            let edits = distance(&query, &candidate);
            let length = query.len().max(candidate.len());
            let similarity = 100 * length.saturating_sub(edits) / length.max(1);
            let task_score = relevance
                .iter()
                .find(|(_, c)| c.id == command.id)
                .map_or(0, |(score, _)| *score);
            // Documentation-only matches and distant ids are not useful guidance.
            let close_id = edits <= 2 && similarity >= 60 || similarity >= 80;
            if close_id || task_score >= 10 {
                ranked.push((
                    task_score * 4 + if close_id { similarity } else { 0 },
                    &command.id,
                ));
            }
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        Ok(ranked
            .into_iter()
            .take(3)
            .map(|(_, id)| id.clone())
            .collect())
    }

    pub fn unknown_hint(&self, id: &str, task: &str) -> Result<String, String> {
        let suggestions = self.suggestions(id, task)?;
        if suggestions.is_empty() {
            Ok(format!(
                "unknown {id}; no close command; use the how tool or list_commands"
            ))
        } else {
            Ok(format!("unknown {id}; closest: {}", suggestions.join(", ")))
        }
    }

    /// Compact guidance for small models: one call first, details last.
    pub fn how_text(&self, task: &str) -> Result<String, String> {
        query_limit(task)?;
        if let Some((id, call, rule)) = holes::example(task) {
            if self.get(id).is_some() {
                return Ok(format!("{call}\n{rule}"));
            }
        }
        let how = self.how(task)?;
        let Some(found) = how.matches.first() else {
            return Ok(format!(
                "{}\nNearest: {}",
                how.message,
                how.nearest.join(", ")
            ));
        };
        let call = serde_json::to_string(&found.example)
            .map_err(|e| format!("cannot format example: {e}"))?;
        let params = found
            .template
            .pointer("/arguments/params")
            .and_then(Value::as_object)
            .map(|p| p.keys().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let mut text = format!("{call}\n{} — {}. Key parameters: {}.\nTyped placeholders (template only; never execute): {}\nConcrete examples require dimensions and geometry selectors from your current design (mm).", found.command.id, found.command.label, if params.is_empty() { "none" } else { &params }, found.template);
        if let Some(reason) = &found.command.disabled_reason {
            text.push_str(&format!("\nCurrent catalogue state: {reason}"));
        }
        text.push_str("\nParameters: ");
        let trimmed = found.command.params.chars().count() > 400;
        text.extend(
            found
                .command
                .params
                .chars()
                .take(if trimmed { 399 } else { 400 }),
        );
        if trimmed {
            text.push('…');
        }
        Ok(text)
    }

    pub fn how(&self, task: &str) -> Result<How, String> {
        query_limit(task)?;
        if let Some((id, example, rule)) = holes::example(task) {
            if let Some(command) = self.get(id) {
                return Ok(How {
                    message: rule.into(),
                    matches: vec![Match {
                        command: command.clone(),
                        example,
                        template: template(command),
                    }],
                    nearest: Vec::new(),
                });
            }
        }
        let commands = self.search(task)?;
        if commands.is_empty() {
            return Ok(How {
                message: "SolidCraft has no command for that".into(),
                matches: Vec::new(),
                nearest: self.closest(task)?,
            });
        }
        let mut matches = Vec::new();
        for command in commands {
            matches.push(Match {
                command: command.clone(),
                example: example(command, task)?,
                template: template(command),
            });
        }
        Ok(How { message: "Templates contain typed placeholders and must not be executed. A null example means current design values or a choice are needed; concrete examples still require SolidCraft validation.".into(), matches, nearest: Vec::new() })
    }
}

fn entries(value: Value, depth: usize) -> Result<Vec<Value>, String> {
    if depth > 6 {
        return Err("catalogue wrapper nesting limit exceeded".into());
    }
    if let Value::Array(entries) = value {
        return Ok(entries);
    }
    if value.get("isError") == Some(&Value::Bool(true)) || value.get("error").is_some() {
        return Err("catalogue contains an MCP error".into());
    }
    for key in ["commands", "structuredContent", "result"] {
        if let Some(inner) = value.get(key) {
            return entries(inner.clone(), depth + 1);
        }
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        for block in content.iter().take(32) {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                let parsed = serde_json::from_str(text)
                    .map_err(|e| format!("invalid MCP catalogue text: {e}"))?;
                return entries(parsed, depth + 1);
            }
        }
    }
    Err("expected a command array or MCP list_commands result".into())
}

fn query_limit(query: &str) -> Result<(), String> {
    if query.len() > MAX_QUERY_BYTES {
        return Err(format!("query exceeds {MAX_QUERY_BYTES} bytes"));
    }
    Ok(())
}
fn normalize(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn words(text: &str) -> BTreeSet<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
fn stop_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "the"
            | "it"
            | "all"
            | "out"
            | "of"
            | "to"
            | "please"
            | "make"
            | "mm"
            | "is"
            | "what"
            | "how"
            | "do"
            | "i"
    )
}
fn synonym(word: &str) -> &str {
    match word {
        "round" | "rounding" => "fillet",
        "bevel" => "chamfer",
        "hollow" => "shell",
        "volume" => "measure",
        "cube" => "box",
        "pull" => "extrude",
        "spin" => "revolve",
        _ => word,
    }
}
fn distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.bytes().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.bytes().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(old + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = old;
        }
    }
    row[b.len()]
}

/// Shared conservative contract for freeform public docs. Only explicit declarations
/// can be required. Grouped bare names remain useful for repair, but imply no requirement.
pub(crate) fn parameter_schema(doc: &str) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut groups = Vec::new();
    for section in top_level_parts(doc, ';').unwrap_or_default() {
        let mut certain = true;
        let mut pending = Vec::new();
        for part in top_level_parts(section, ',').unwrap_or_default() {
            let alternatives = top_level_parts(part, '|').unwrap_or_default();
            // Shared declarations require a choice, never the last name alone.
            if alternatives.len() > 1 {
                if let Some((_, desc)) = alternatives.last().and_then(|s| s.split_once(':')) {
                    let names: Vec<_> = alternatives
                        .iter()
                        .map(|s| s.split_once(':').map_or(*s, |(name, _)| name).trim())
                        .collect();
                    if names
                        .iter()
                        .all(|name| parameter_name(name.trim_end_matches('?')))
                    {
                        let mut choices = Vec::new();
                        let optional = names.iter().any(|name| name.ends_with('?'));
                        for (name, alternative) in names.into_iter().zip(&alternatives) {
                            let key = name.trim_end_matches('?');
                            let own_desc = alternative.split_once(':').map_or(desc, |(_, d)| d);
                            let declaration = format!("{key}:{own_desc}");
                            let schema = parameter_schema(&declaration);
                            properties.insert(key.into(), schema["properties"][key].clone());
                            choices.push(json!({"required":[key]}));
                        }
                        if !optional {
                            groups.push(json!({"anyOf":choices}));
                        }
                        continue;
                    }
                }
            }
            let enum_values: Vec<_> = alternatives
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let s = if i == 0 {
                        s.split_once(':').map_or(*s, |(_, d)| d)
                    } else {
                        s
                    };
                    s.split_whitespace().next().unwrap_or("")
                })
                .collect();
            let is_enum = alternatives.len() > 1
                && !section.contains(", or ")
                && alternatives.iter().enumerate().all(|(i, s)| {
                    let desc = if i == 0 {
                        s.split_once(':').map_or(*s, |(_, d)| d)
                    } else {
                        s
                    };
                    let desc = desc.trim();
                    let head = desc.split_whitespace().next().unwrap_or("");
                    let suffix = desc.get(head.len()..).unwrap_or("").trim();
                    suffix.is_empty() || suffix.starts_with('(')
                })
                && enum_values.iter().all(|s| {
                    parameter_name(s)
                        && !matches!(*s, "id" | "name" | "expr" | "number" | "string" | "bool")
                });
            // Distinguish enum values from alternative declarations. A choice of
            // declarations cannot justify requiring any single name.
            let choice = alternatives
                .iter()
                .filter(|s| {
                    s.split_once(':')
                        .is_some_and(|(name, _)| parameter_name(name.trim().trim_end_matches('?')))
                })
                .count()
                > 1;
            let has_pipe = alternatives.len() > 1;
            for alternative in &alternatives {
                let Some((raw, desc)) = alternative.trim().split_once(':') else {
                    let bare = alternative.trim().trim_end_matches('?');
                    if parameter_name(bare) && !has_pipe {
                        pending.push(bare);
                    } else if !has_pipe {
                        certain = false;
                    }
                    continue;
                };
                let name = raw.trim().trim_end_matches('?');
                if !parameter_name(name) {
                    certain = false;
                    pending.clear();
                    continue;
                }
                let desc = desc.trim();
                let head = desc.split_whitespace().next().unwrap_or("");
                let mut schema = match head {
                    "expr" => json!({"type":["number","string"]}),
                    "number" | "float" => json!({"type":"number"}),
                    "integer" => json!({"type":"integer"}),
                    "bool" => json!({"type":"boolean"}),
                    "string" => json!({"type":"string"}),
                    "object" => json!({"type":"object"}),
                    _ if desc.starts_with('[') => json!({"type":"array"}),
                    _ if desc.starts_with('{') => json!({"type":"object"}),
                    _ => json!({}),
                };
                if is_enum {
                    schema = json!({"type":"string", "enum":enum_values});
                }
                if (desc.starts_with("[x,y]") || desc.starts_with("[x,y,z]"))
                    && !desc.contains(" or ")
                {
                    let count = if desc.starts_with("[x,y,z]") { 3 } else { 2 };
                    schema = json!({"type":"array", "minItems":count, "maxItems":count,
                        "items":{"type":["number","string"]}});
                }
                // Strip no prose into a contract. For example, `fixed:true explicitly`
                // and `thin?: true (...) with curves?: ...` are not typed declarations.
                let explicit_type = schema.get("type").is_some();
                let enum_value = parameter_name(head) && desc == head;
                if !explicit_type && !enum_value {
                    certain = false;
                    pending.clear();
                    continue;
                }
                for grouped in pending.drain(..) {
                    properties.insert(grouped.to_string(), schema.clone());
                }
                properties.insert(name.to_string(), schema);
                // Parenthetical alternatives are deliberately not parsed as names.
                // Any uncertainty here is cheaper than an incorrect hand-back.
                let simple = desc == head || desc.starts_with('[') || desc.starts_with('{');
                if !simple {
                    certain = false;
                }
                if certain && !choice && !raw.trim().ends_with('?') && simple {
                    required.push(name.to_string());
                }
            }
        }
        for name in pending {
            properties
                .entry(name.to_string())
                .or_insert_with(|| json!({}));
        }
    }
    json!({"type":"object", "properties":properties, "required":required, "allOf":groups})
}

fn parameter_name(name: &str) -> bool {
    name.as_bytes()
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// Bounded, quote-aware scanner. Malformed or oversized docs provide no contract.
fn top_level_parts(doc: &str, delimiter: char) -> Option<Vec<&str>> {
    if doc.len() > 32768 {
        return None;
    }
    let mut parts = Vec::new();
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut previous = ' ';
    let mut start = 0;
    for (i, c) in doc.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
        } else {
            match c {
                '"' | '`' => quote = Some(c),
                '\'' if !previous.is_alphanumeric() => quote = Some(c),
                '(' | '[' | '{' => {
                    if stack.len() >= 64 {
                        return None;
                    }
                    stack.push(c);
                }
                ')' | ']' | '}' => {
                    let opening = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    if stack.pop() != Some(opening) {
                        return None;
                    }
                }
                _ if c == delimiter && stack.is_empty() => {
                    if parts.len() >= 255 {
                        return None;
                    }
                    parts.push(doc[start..i].trim());
                    start = i + c.len_utf8();
                }
                _ => {}
            }
        }
        previous = c;
    }
    if quote.is_some() || !stack.is_empty() {
        return None;
    }
    parts.push(doc[start..].trim());
    Some(parts)
}

// Split freeform parameter docs only at top-level delimiters. Grouped names such as
// "length, width, height: expr" share their description.
fn clauses(doc: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (i, c) in doc.char_indices() {
        match c {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            ',' | ';' if depth == 0 => {
                parts.push(doc[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(doc[start..].trim());
    parts
}

fn template(command: &Command) -> Value {
    let schema = parameter_schema(&command.params);
    let mut params = Map::new();
    if let Some(properties) = schema["properties"].as_object() {
        for (name, shape) in properties {
            let placeholder = if shape.get("enum").is_some() {
                format!("<choose from {}>", shape["enum"])
            } else if name == "body" || name == "target" {
                "<body id from model.bodies>".into()
            } else if name == "sketch" {
                "<sketch id returned by sketch.create>".into()
            } else if shape["type"] == "array" {
                "<array matching parameter docs>".into()
            } else if matches!(
                name.as_str(),
                "radius"
                    | "diameter"
                    | "length"
                    | "width"
                    | "height"
                    | "depth"
                    | "distance"
                    | "thickness"
                    | "offset"
            ) {
                "<number mm>".into()
            } else if matches!(name.as_str(), "angle" | "angle2" | "taper") {
                "<number deg>".into()
            } else if shape["type"] == "integer" {
                "<integer>".into()
            } else if shape["type"] == "number" || shape["type"].is_array() {
                "<number or expression; units from parameter docs>".into()
            } else {
                format!("<{name} from parameter docs or current model>")
            };
            params.insert(name.clone(), json!(placeholder));
        }
    }
    json!({"tool":"execute", "arguments":{"command":command.id, "params":params}})
}

fn example(command: &Command, task: &str) -> Result<Value, String> {
    let mut params = Map::new();
    let mut pending = Vec::new();
    let amount = task
        .split_whitespace()
        .find_map(|w| w.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(3.0);
    for clause in clauses(&command.params) {
        let (name, desc) = clause.split_once(':').unwrap_or((clause, ""));
        let name = name.trim();
        if name.ends_with('?') {
            let requested_enum = desc.split_whitespace().next().is_some_and(|head| {
                head.contains('|') && head.split('|').any(|option| words(task).contains(option))
            });
            if !requested_enum {
                pending.clear();
                continue;
            }
        }
        let name = name.trim_end_matches('?');
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            pending.clear();
            continue;
        }
        pending.push(name);
        if desc.is_empty() {
            continue;
        }
        for name in pending.drain(..) {
            let desc = desc.trim();
            let value = if name == "faces" {
                json!([[10, 10, 20]])
            } else if name == "edges" {
                json!([[10, 0, 0]])
            } else if matches!(name, "radius" | "thickness" | "distance") {
                json!(amount)
            } else if desc.starts_with("expr")
                || desc.starts_with("number")
                || desc.starts_with("integer")
            {
                json!(20)
            } else if desc.starts_with("bool") {
                json!(true)
            } else if desc.starts_with("[[") {
                json!([[0, 0], [20, 10]])
            } else if desc.starts_with("[x,y,z]") {
                json!([0, 0, 0])
            } else if desc.starts_with("[x,y]") {
                if name == "p1" {
                    json!([20, 10])
                } else {
                    json!([0, 0])
                }
            } else if desc.starts_with("object") || desc.starts_with('{') {
                json!({})
            } else if desc.contains('|') {
                let options: Vec<_> = desc
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('|')
                    .collect();
                let requested: Vec<_> = options
                    .iter()
                    .filter(|option| words(task).contains(**option))
                    .collect();
                if let [choice] = requested.as_slice() {
                    json!(choice)
                } else {
                    return Ok(Value::Null);
                }
            } else {
                return Ok(Value::Null);
            };
            params.insert(name.to_string(), value);
        }
    }
    if !pending.is_empty() {
        return Ok(Value::Null);
    }
    Ok(json!({"tool":"execute", "arguments":{"command":command.id, "params":params}}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn shared_alternatives_and_enum_contracts() {
        let schema = parameter_schema("radius | diameter: number");
        assert_eq!(schema["required"], json!([]));
        for name in ["radius", "diameter"] {
            assert_eq!(schema["properties"][name]["type"], "number");
        }
        assert_eq!(
            schema["allOf"],
            json!([{"anyOf":[{"required":["radius"]},{"required":["diameter"]}]}])
        );
        assert_eq!(
            parameter_schema("corner? | center?: [x,y,z]")["allOf"],
            json!([])
        );
        let schema = Catalog::bundled()
            .unwrap()
            .parameter_schema("solid.extrude");
        assert_eq!(
            schema["properties"]["operation"]["enum"],
            json!(["new", "join", "cut", "intersect", "auto"])
        );
        assert!(parameter_schema("sketch?: id|name")["properties"]["sketch"]
            .get("enum")
            .is_none());
        for doc in [
            "plane: XY|XZ|YZ|plane name",
            "type: open shape|closed shape",
            "type: rigid|revolute, or a relation: parallel|angle",
        ] {
            assert!(
                parameter_schema(doc)["properties"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(|shape| shape.get("enum").is_none()),
                "{doc}"
            );
        }
    }

    #[test]
    fn typed_templates_are_separate_and_enum_examples_follow_task() {
        let catalog = Catalog::from_json(
            &json!([
                {"id":"joint.create","label":"Joint","params":"type: rigid|revolute; body: name"},
                {"id":"test.type","label":"Test","params":"type: rigid|revolute"},
                {"id":"test.extrude","label":"Extrude","params":"distance: number; operation?: new|join|cut"},
                {"id":"test.radius","label":"Radius","params":"radius: number"}
            ])
            .to_string(),
        )
        .unwrap();
        let command = catalog.get("test.type").unwrap();
        assert_eq!(
            example(command, "make revolute").unwrap()["arguments"]["params"]["type"],
            "revolute"
        );
        assert!(example(command, "make a joint").unwrap().is_null());
        assert_eq!(
            example(catalog.get("test.extrude").unwrap(), "cut 10 mm").unwrap()["arguments"]
                ["params"]["operation"],
            "cut"
        );
        let command = catalog.get("joint.create").unwrap();
        assert!(example(command, "revolute").unwrap().is_null());
        assert_eq!(
            template(command)["arguments"]["params"]["body"],
            "<body id from model.bodies>"
        );
        assert_eq!(
            template(catalog.get("test.radius").unwrap())["arguments"]["params"]["radius"],
            "<number mm>"
        );
        let text = Catalog::bundled()
            .unwrap()
            .how_text("create a revolute joint")
            .unwrap();
        assert!(text.contains("template only; never execute"));
        assert!(!text.contains("\"type\":\"rigid\""));
    }

    #[test]
    fn every_required_name_is_an_explicit_top_level_declaration() {
        let catalog = Catalog::bundled().unwrap();
        for command in catalog.commands() {
            let names: BTreeSet<_> = top_level_parts(&command.params, ';')
                .unwrap_or_default()
                .into_iter()
                .flat_map(|part| top_level_parts(part, ',').unwrap_or_default())
                .flat_map(|part| top_level_parts(part, '|').unwrap_or_default())
                .filter_map(|part| part.split_once(':'))
                .map(|(name, _)| name.trim())
                .filter(|name| parameter_name(name))
                .collect();
            let schema = catalog.parameter_schema(&command.id);
            for name in schema["required"].as_array().unwrap() {
                assert!(
                    names.contains(name.as_str().unwrap()),
                    "{}: {name}",
                    command.id
                );
            }
        }
        assert_eq!(
            catalog.parameter_schema("sketch.create")["required"],
            json!(["plane"])
        );
        // The parenthetical alternative is not a declaration. Defer this choice
        // to SolidCraft instead of falsely requiring distance or thin-wall thickness.
        assert_eq!(
            catalog.parameter_schema("solid.extrude")["required"],
            json!([])
        );
        assert_eq!(
            catalog.parameter_schema("solid.box")["required"],
            json!(["height"])
        );
        assert_eq!(
            parameter_schema("target: object; radius: expr")["required"],
            json!(["target", "radius"])
        );
    }

    #[test]
    fn docs_do_not_promote_prose_nested_fields_or_choices() {
        for (doc, expected) in [
            (
                "plane: XY|XZ|YZ; fixed:true explicitly freezes it",
                json!(["plane"]),
            ),
            ("distance: expr | through_all: bool", json!([])),
            ("distance: expr (or through_all: true)", json!([])),
            ("plane?: {fixed: bool, nested: [x,y]}; name?", json!([])),
            (
                "note?: string (fixed:true; radius: expr), radius?: expr",
                json!([]),
            ),
            (
                "note?: string; \"quoted; fake: expr\"; real: expr",
                json!(["real"]),
            ),
            (
                "note?: string; 'quoted, fake: expr'; real: expr",
                json!(["real"]),
            ),
            (
                "note?: string; `quoted; fake: expr`; real: expr",
                json!(["real"]),
            ),
            (
                "note?: string; \"escaped \\\"; fake: expr\"; real: expr",
                json!(["real"]),
            ),
            (
                "description of a thing: expr, fixed:true explicitly freezes it",
                json!([]),
            ),
            (
                "thin?: true (wall) with curves?: [ids], thickness: expr",
                json!([]),
            ),
            ("length, width, height: expr", json!(["height"])),
            ("a?: expr; b: bool; c: [x,y]", json!(["b", "c"])),
            ("a: expr; broken: {", json!([])),
            ("a: expr; broken: [)", json!([])),
            ("a: expr; \"unclosed", json!([])),
        ] {
            assert_eq!(parameter_schema(doc)["required"], expected, "{doc}");
        }
        assert_eq!(
            parameter_schema(&"a: expr;".repeat(300))["required"],
            json!([])
        );
        assert_eq!(parameter_schema(&"x".repeat(32769))["required"], json!([]));
        assert_eq!(
            parameter_schema(&format!("a: {}{}", "[".repeat(65), "]".repeat(65)))["required"],
            json!([])
        );
    }

    #[test]
    fn snapshot_search_and_examples() {
        let catalog = Catalog::bundled().unwrap();
        assert!(catalog.commands().len() > 100);
        let how = catalog.how("round all edges 3 mm").unwrap();
        assert_eq!(how.matches[0].command.id, "solid.fillet");
        assert_eq!(how.matches[0].example["arguments"]["params"]["radius"], 3.0);
        assert!(how.matches[0].example["arguments"]["params"]["edges"].is_array());
        assert_eq!(
            catalog.how("hollow out the box").unwrap().matches[0]
                .command
                .id,
            "solid.shell"
        );
        let how = catalog.how("make it fly").unwrap();
        assert!(how.matches.is_empty());
        assert_eq!(how.message, "SolidCraft has no command for that");
        assert!(!how.nearest.is_empty());
        assert_eq!(catalog.closest("solid.bxo").unwrap()[0], "solid.box");
        for query in ["SOLID_BOX", "solid-box", "solid box", "solid.box"] {
            assert_eq!(catalog.closest(query).unwrap()[0], "solid.box");
        }
        assert!(catalog.closest("nonsense").unwrap().len() <= 3);
        assert_eq!(
            catalog.search("bevel edges").unwrap()[0].id,
            "solid.chamfer"
        );
        assert_eq!(
            catalog.how("solid.box").unwrap().matches[0].example["arguments"]["params"],
            json!({"length":20,"width":20,"height":20})
        );
    }

    #[test]
    fn compact_how_leads_with_one_call_and_caps_docs() {
        let doc = format!("radius: number; {}", "é".repeat(600));
        let catalog = Catalog::from_json(
            &json!([{"id":"solid.fillet","label":"Fillet","params":doc}]).to_string(),
        )
        .unwrap();
        let text = catalog.how_text("round edges 3 mm").unwrap();
        let call: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(call["tool"], "execute");
        assert_eq!(call["arguments"]["params"]["radius"], 3.0);
        assert!(text.lines().nth(1).unwrap().contains("Key parameters:"));
        assert!(text.contains("placeholders"));
        let docs = text.rsplit_once("Parameters: ").unwrap().1;
        assert_eq!(docs.chars().count(), 400);
    }

    #[test]
    fn search_uses_docs_and_label() {
        let c = Catalog::from_json(r#"[{"id":"one","label":"Special Shape","params":"radius: expr; tangent: bool"},{"id":"two","label":"Other","params":""}]"#).unwrap();
        assert_eq!(c.search("special").unwrap()[0].id, "one");
        assert_eq!(c.search("radius tangent").unwrap()[0].id, "one");
    }

    #[test]
    fn loads_cli_and_mcp_formats() {
        let record = json!([{"id":"solid.box","label":"Box","params":"length: expr"}]);
        for wrapper in [
            record.clone(),
            json!({"commands":record}),
            json!({"result":{"content":[{"type":"text","text":record.to_string()}]}}),
            json!({"structuredContent":{"commands":record}}),
        ] {
            let c = Catalog::from_json(&wrapper.to_string()).unwrap();
            assert_eq!(c.commands().len(), 1);
            assert_eq!(c.get("solid.box").unwrap().label, "Box");
        }
    }

    #[test]
    fn catalogue_and_query_refusals() {
        for bad in [
            "null",
            "{}",
            "[{\"id\":\"x\"}]",
            r#"{"isError":true,"commands":[]}"#,
            r#"{"error":{"message":"bad"},"result":[]}"#,
        ] {
            assert!(Catalog::from_json(bad).is_err(), "{bad}");
        }
        let record = json!({"id":"x","label":"X","params":""});
        assert!(Catalog::from_json(&json!([record, record]).to_string()).is_err());
        assert!(Catalog::from_json(&json!(vec![record; MAX_COMMANDS + 1]).to_string()).is_err());
        assert!(Catalog::from_json(&" ".repeat(MAX_CATALOG_BYTES + 1)).is_err());
        let c = Catalog::bundled().unwrap();
        assert!(c.how(&"x".repeat(MAX_QUERY_BYTES + 1)).is_err());
        assert!(c.closest(&"x".repeat(MAX_QUERY_BYTES + 1)).is_err());
        assert!(c.closest(" !!! ").unwrap().is_empty());
        assert!(c.search("the it all").unwrap().is_empty());
    }
}
