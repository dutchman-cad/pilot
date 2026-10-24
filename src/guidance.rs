//! Deterministic state guidance over caller-supplied tool results.
use crate::catalog::Catalog;
use serde_json::{json, Value};

#[cfg(feature = "proxy")]
pub(crate) fn how_tool() -> Value {
    json!({"name":"how","description":"Find a SolidCraft command, a typed template, and a concrete example when possible. Templates must be filled from the request and current model; never executes anything.","inputSchema":{"type":"object","properties":{"task":{"type":"string","maxLength":256}},"required":["task"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false}})
}

#[cfg(feature = "proxy")]
pub(crate) fn how_answer(catalog: &Catalog, args: &Value) -> Result<String, String> {
    let task = args
        .get("task")
        .and_then(Value::as_str)
        .ok_or("how requires a string task")?;
    catalog.how_text(task)
}
pub(crate) fn payload(v: &Value) -> Value {
    let v = v.get("result").unwrap_or(v);
    if let Some(s) = v.get("structuredContent") {
        return s.clone();
    }
    if let Some(blocks) = v.get("content").and_then(Value::as_array) {
        for b in blocks.iter().take(32) {
            if let Some(text) = b.get("text").and_then(Value::as_str) {
                if let Ok(data) = serde_json::from_str(text) {
                    return data;
                }
            }
        }
    }
    v.clone()
}
// Only report counts backed by metadata; a truncated page's total stays unknown.
pub fn paging(data: &Value) -> Vec<String> {
    fn visit(v: &Value, depth: usize, out: &mut Vec<String>) {
        if depth > 6 || out.len() >= 16 {
            return;
        }
        if let Some(obj) = v.as_object() {
            let arrays: Vec<_> = obj.values().filter_map(Value::as_array).collect();
            let shown = obj.get("shown").and_then(Value::as_u64).or_else(|| {
                if arrays.len() == 1 {
                    Some(arrays[0].len() as u64)
                } else {
                    None
                }
            });
            let total = obj
                .get("total")
                .or_else(|| obj.get("total_count"))
                .and_then(Value::as_u64);
            let partial = obj.get("truncated") == Some(&json!(true))
                || obj.get("partial") == Some(&json!(true))
                || obj.get("has_more") == Some(&json!(true))
                || obj.get("nextCursor").is_some_and(|v| !v.is_null())
                || matches!((shown, total), (Some(n), Some(t)) if n < t);
            if partial {
                out.push(format!(
                    "shown {} of {} (partial)",
                    shown
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown".into()),
                    total
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown total".into())
                ));
            }
            for value in obj.values() {
                visit(value, depth + 1, out);
            }
        } else if let Some(array) = v.as_array() {
            for value in array.iter().take(64) {
                visit(value, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(data, 0, &mut out);
    out
}
fn body_volumes(v: &Value) -> Option<Vec<f64>> {
    if !paging(v).is_empty() {
        return None;
    }
    let mut volumes: Vec<f64> = v
        .get("bodies")?
        .as_array()?
        .iter()
        .map(|b| b.get("volume_mm3").or_else(|| b.get("volume"))?.as_f64())
        .collect::<Option<_>>()?;
    volumes.sort_by(f64::total_cmp);
    Some(volumes)
}
pub fn state_hint(
    before: Option<&Value>,
    after: &Value,
    change: bool,
    catalog: Option<&Catalog>,
) -> Option<String> {
    let mut hints = Vec::new();
    if change {
        if let (Some(a), Some(b)) = (before.and_then(body_volumes), body_volumes(after)) {
            if a == b {
                hints.push("The design didn't change in body count or volume; verify the intended result before saying done.".to_string());
            }
        }
    }
    let active = after
        .get("active_sketch")
        .filter(|v| !v.is_null() && **v != json!(false));
    if active.is_some() {
        hints.push("An open sketch is active; finish it before solid features.".into());
        if catalog.is_some_and(|c| c.get("sketch.finish").is_some()) {
            hints.push("Say \"yes\" to do: {\"tool\":\"execute\",\"arguments\":{\"command\":\"sketch.finish\",\"params\":{}}}".into());
        }
    } else if after
        .get("bodies")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
        && after
            .get("sketches")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && paging(after).is_empty()
    {
        hints.push("Empty design: start with a sketch or a primitive.".into());
    }
    if let Some(sketches) = after.get("sketches").and_then(Value::as_array) {
        for sketch in sketches.iter().take(64) {
            if let Some(dof) = sketch
                .get("degrees_of_freedom")
                .or_else(|| sketch.get("dof"))
                .and_then(Value::as_u64)
                .filter(|n| *n > 0)
            {
                hints.push(format!(
                    "Sketch {} has {dof} free degrees of freedom; add dimensions or constraints.",
                    sketch
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("(unnamed)")
                ));
                break;
            }
        }
    }
    if hints.is_empty() {
        None
    } else {
        Some(hints.join("\n"))
    }
}

#[cfg(feature = "proxy")]
fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
/// The one catalogue id a mistyped id plainly means: a normalized match, or the only
/// close candidate within one edit or one adjacent swap. Shared by both faces.
#[cfg(feature = "proxy")]
pub(crate) fn unique_close(catalog: &Catalog, id: &str) -> Result<Option<String>, String> {
    let closest = catalog.closest(id)?;
    let wanted = norm(id);
    if let Some(exact) = closest.iter().find(|c| norm(c) == wanted) {
        return Ok(Some(exact.clone()));
    }
    let matches: Vec<_> = closest.iter().filter(|c| near(id, c)).collect();
    Ok(match matches.as_slice() {
        [one] => Some((*one).clone()),
        _ => None,
    })
}
#[cfg(feature = "proxy")]
fn near(a: &str, b: &str) -> bool {
    let a = norm(a);
    let b = norm(b);
    if a == b {
        return true;
    }
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut row: Vec<_> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(old + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = old;
        }
    }
    if row[b.len()] <= 1 {
        return true;
    }
    let differences: Vec<_> = a
        .iter()
        .zip(b)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    a.len() == b.len()
        && differences.len() == 2
        && differences[1] == differences[0] + 1
        && a[differences[0]] == b[differences[1]]
        && a[differences[1]] == b[differences[0]]
}
