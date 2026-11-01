//! Deterministic state guidance over caller-supplied tool results.
use crate::catalog::Catalog;
use serde_json::{json, Value};
#[cfg(feature = "proxy")]
pub(crate) mod holes;

#[cfg(feature = "proxy")]
pub(crate) const XZ_SIGN_HINT: &str =
    "XZ sketch v is -world Z; use negative v for positive world Z.";
#[cfg(feature = "proxy")]
pub(crate) const PLANE_CLEANUP_HINT: &str = "Before finishing, delete construction planes created this turn that no later feature references.";

/// Bounded evidence from successful public calls; no geometry is rewritten.
#[cfg(feature = "proxy")]
#[derive(Default)]
pub(crate) struct PlaneGuidance {
    ranges: Vec<(String, f64, f64)>,
    xz: bool,
    planes: Vec<Vec<String>>,
    misplaced: bool,
}
#[cfg(feature = "proxy")]
impl PlaneGuidance {
    pub(crate) fn new_turn(&mut self) {
        self.planes.clear();
        self.misplaced = false;
    }
    pub(crate) fn hints(&self) -> [Option<&'static str>; 2] {
        [
            self.misplaced.then_some(XZ_SIGN_HINT),
            (!self.planes.is_empty()).then_some(PLANE_CLEANUP_HINT),
        ]
    }
    pub(crate) fn visit(&mut self, name: &str, args: &Value, result: &Value) {
        let data = payload(result);
        if !paging(&data).is_empty() {
            self.ranges.clear();
            return;
        }
        if name == "batch" {
            if let Some((calls, results)) =
                args["commands"].as_array().zip(data["results"].as_array())
            {
                for (call, result) in calls.iter().zip(results).take(128) {
                    self.visit("execute", call, result);
                }
            }
            return;
        }
        if result["isError"] == true
            || result["result"]["isError"] == true
            || data["isError"] == true
            || data["ok"] == false
            || data.get("error").is_some_and(|e| !e.is_null())
        {
            return;
        }
        if let Some(bodies) = data.get("after").unwrap_or(&data)["bodies"]
            .as_array()
            .filter(|a| {
                a.len() <= 1024
                    && a.iter().all(|b| b.is_object() && b["bbox"].is_object())
                    && (!a.is_empty()
                        || matches!(name, "inspect" | "inspect_design")
                        || data.get("timeline").is_some()
                        || data.get("after").is_some())
            })
        {
            self.ranges.clear();
            for b in bodies {
                if let (Some(n), Some(lo), Some(hi)) = (
                    b["name"].as_str(),
                    b["bbox"]["min"][2].as_f64(),
                    b["bbox"]["max"][2].as_f64(),
                ) {
                    if lo.is_finite() && hi.is_finite() {
                        self.ranges.push((n.to_string(), lo, hi));
                    }
                }
            }
        }
        if name != "execute" {
            return;
        }
        if let Some(removed) = data["removed_bodies"].as_array() {
            for name in removed.iter().take(1024).filter_map(Value::as_str) {
                self.ranges.retain(|(old, _, _)| old != name);
            }
        }
        let command = args["command"].as_str().unwrap_or("");
        let p = &args["params"];
        // Only structural parameter references count, never diagnostic text.
        fn references(v: &Value, names: &[String], depth: usize) -> bool {
            if depth > 16 {
                return false;
            }
            match v {
                Value::String(s) => names.contains(s),
                Value::Array(a) => a.iter().take(128).any(|v| references(v, names, depth + 1)),
                Value::Object(o) => o
                    .values()
                    .take(128)
                    .any(|v| references(v, names, depth + 1)),
                _ => false,
            }
        }
        if command == "sketch.create"
            || command.starts_with("construct.")
            || command.starts_with("solid.")
            || matches!(
                command,
                "feature.delete" | "timeline.delete" | "model.delete"
            )
        {
            self.planes.retain(|names| !references(p, names, 0));
        }
        if command.starts_with("construct.plane.") && self.planes.len() < 128 {
            let names: Vec<_> = [
                p.get("name"),
                data.get("name"),
                data.get("id"),
                data.get("feature"),
            ]
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .map(str::to_string)
            .collect();
            if !names.is_empty() {
                self.planes.push(names);
            }
        }
        if command == "sketch.create" {
            self.xz = p["plane"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("XZ"))
                && p.get("flip").is_none_or(|v| *v == false);
        } else if command == "sketch.finish" || command == "sketch.edit" {
            self.xz = false;
        }
        if command == "solid.box" && self.ranges.len() < 1024 {
            if let Some(height) = p["height"]
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0 && *n <= 1_000_000.0)
            {
                let lo = if let Some(c) = p.get("center") {
                    c[2].as_f64().map(|z| z - height / 2.0)
                } else {
                    p.get("corner").map_or(Some(0.0), |c| c[2].as_f64())
                };
                if let (Some(lo), Some(names)) = (lo, data["created_bodies"].as_array()) {
                    for n in names.iter().take(4).filter_map(Value::as_str) {
                        self.ranges.retain(|(old, _, _)| old != n);
                        self.ranges.push((n.into(), lo, lo + height));
                    }
                }
            }
        } else if matches!(
            command,
            "model.move"
                | "model.rotate"
                | "model.scale"
                | "model.delete"
                | "solid.move"
                | "solid.rotate"
                | "solid.scale"
        ) {
            self.ranges.clear();
        }
        if self.xz
            && (command.starts_with("sketch.circle")
                || command.starts_with("sketch.rectangle")
                || command == "sketch.point")
        {
            let v = p
                .get("center")
                .or_else(|| p.get("point"))
                .and_then(|c| c[1].as_f64())
                .or_else(|| {
                    if command.ends_with("two_point") || command.ends_with("rounded") {
                        Some((p["p0"][1].as_f64()? + p["p1"][1].as_f64()?) / 2.0)
                    } else {
                        None
                    }
                });
            if let Some(v) = v.filter(|v| v.is_finite() && v.abs() <= 1_000_000.0) {
                let inside = |z: f64| self.ranges.iter().any(|(_, lo, hi)| z >= *lo && z <= *hi);
                self.misplaced |= !inside(-v) && inside(v);
            }
        }
    }
}

#[cfg(all(test, feature = "proxy"))]
mod plane_tests {
    use super::*;
    fn run(s: &mut PlaneGuidance, command: &str, params: Value) {
        s.visit(
            "execute",
            &json!({"command":command,"params":params}),
            &json!({}),
        );
    }
    #[test]
    fn sign_uses_all_body_ranges_and_profile_centres() {
        for (command, params) in [
            ("sketch.circle.center", json!({"center":[10,10]})),
            (
                "sketch.rectangle.two_point",
                json!({"p0":[8,8],"p1":[12,12]}),
            ),
            ("sketch.point", json!({"point":[10,10]})),
        ] {
            let mut s = PlaneGuidance::default();
            s.visit(
                "inspect",
                &Value::Null,
                &json!({"bodies":[{"name":"A","bbox":{"min":[0,0,0],"max":[20,20,20]}}]}),
            );
            run(&mut s, "sketch.create", json!({"plane":"XZ"}));
            run(&mut s, command, params);
            assert_eq!(s.hints()[0], Some(XZ_SIGN_HINT));
        }
        for (plane, v, ranges) in [
            ("XY", 10., vec![(0., 20.)]),
            ("YZ", 10., vec![(0., 20.)]),
            ("XZ", -10., vec![(0., 20.)]),
            ("XZ", 10., vec![(-20., 20.)]),
            ("XZ", 10., vec![(0., 20.), (-20., -1.)]),
            ("XZ", 10., vec![]),
        ] {
            let mut s = PlaneGuidance {
                ranges: ranges
                    .into_iter()
                    .map(|(lo, hi)| ("A".into(), lo, hi))
                    .collect(),
                ..PlaneGuidance::default()
            };
            run(&mut s, "sketch.create", json!({"plane":plane}));
            run(&mut s, "sketch.circle.center", json!({"center":[10,v]}));
            assert_eq!(s.hints()[0], None);
        }
    }
    #[test]
    fn cleanup_counts_only_successful_unreferenced_planes() {
        let mut s = PlaneGuidance::default();
        run(
            &mut s,
            "construct.plane.offset",
            json!({"name":"P_Z10","base":"XY","offset":10}),
        );
        assert_eq!(s.hints()[1], Some(PLANE_CLEANUP_HINT));
        run(&mut s, "timeline.inspect", json!({"feature":"P_Z10"}));
        assert_eq!(s.hints()[1], Some(PLANE_CLEANUP_HINT));
        run(&mut s, "sketch.create", json!({"plane":"P_Z10"}));
        assert_eq!(s.hints()[1], None);
        s.visit(
            "execute",
            &json!({"command":"construct.plane.offset","params":{"name":"failed"}}),
            &json!({"error":"bad plane"}),
        );
        assert_eq!(s.hints()[1], None);
        run(
            &mut s,
            "construct.plane.offset",
            json!({"name":"Unused","base":"XY","offset":4}),
        );
        run(&mut s, "feature.delete", json!({"feature":"Unused"}));
        assert_eq!(s.hints()[1], None);
    }
}

#[cfg(feature = "proxy")]
pub(crate) const EDGE_HINT: &str = "HINT: Edges accept [x,y,z], {point:[x,y,z]}, or {body,index}; take edge midpoints from model.edges (box vertical edges: corners at mid-height).";

#[cfg(feature = "proxy")]
pub(crate) fn edge_hints(
    name: &str,
    args: &Value,
    result: &Value,
    seen: &mut bool,
) -> Vec<&'static str> {
    if *seen {
        return Vec::new();
    }
    let data = payload(result);
    let pairs: Vec<(&Value, &Value)> = match name {
        "execute" => vec![(args, result)],
        "batch" => args["commands"]
            .as_array()
            .zip(data["results"].as_array())
            .map(|(a, b)| a.iter().zip(b).take(128).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for (call, result) in pairs {
        if !matches!(
            call["command"].as_str(),
            Some("solid.fillet" | "solid.chamfer")
        ) {
            continue;
        }
        let text = result
            .to_string()
            .chars()
            .take(8192)
            .collect::<String>()
            .to_ascii_lowercase();
        if text.contains("no edge at")
            || text.contains("an edge is a point")
            || (text.contains("edge")
                && text.contains("{body")
                && text.contains("index")
                && text.contains("error"))
        {
            *seen = true;
            return vec![EDGE_HINT];
        }
    }
    Vec::new()
}

#[cfg(feature = "proxy")]
pub(crate) const SKETCH_ENTITY_HINT: &str = "HINT: One line's length: entities:[\"l1\"], value:<mm number>; distance between two lines: entities:[\"l2\",\"l5\"].";
#[cfg(feature = "proxy")]
pub(crate) const SKETCH_EXISTING_HINT: &str = "HINT: sketch.line with infer:true already adds coincident/horizontal/vertical constraints; move on to dimensions.";

/// Two bounded hints per sketch turn, shared by both proxy faces.
#[cfg(feature = "proxy")]
pub(crate) fn sketch_hints(
    name: &str,
    args: &Value,
    result: &Value,
    seen: &mut [bool; 2],
) -> Vec<&'static str> {
    fn visit(call: &Value, result: &Value, seen: &mut [bool; 2], out: &mut Vec<&'static str>) {
        let Some(command) = call["command"].as_str() else {
            return;
        };
        let data = payload(result);
        // Result sizes are capped by the caller; inspect only a bounded diagnostic.
        let text: String = result
            .to_string()
            .chars()
            .take(8192)
            .collect::<String>()
            .to_ascii_lowercase();
        let entities = &call["params"]["entities"];
        let bad_entities = entities.as_array().is_none_or(|a| a.len() > 2);
        let failed = result["isError"] == true
            || result["result"]["isError"] == true
            || data["isError"] == true
            || data["ok"] == false
            || data.get("error").is_some_and(|v| !v.is_null())
            || result.get("error").is_some_and(|v| !v.is_null());
        let hint = if command == "sketch.dimension"
            && failed
            && text.contains("entities")
            && (bad_entities
                || text.contains("one or two")
                || text.contains("array")
                || text.contains("list")
                || text.contains("two"))
        {
            Some((0, SKETCH_ENTITY_HINT))
        } else if command.starts_with("sketch.constraint.") && data["existing"] == true {
            Some((1, SKETCH_EXISTING_HINT))
        } else {
            None
        };
        if let Some((index, hint)) = hint {
            if !seen[index] {
                seen[index] = true;
                out.push(hint);
            }
        }
    }
    let mut out = Vec::new();
    match name {
        "execute" => visit(args, result, seen, &mut out),
        "batch" => {
            let data = payload(result);
            if let Some((calls, results)) =
                args["commands"].as_array().zip(data["results"].as_array())
            {
                for (call, result) in calls.iter().zip(results).take(128) {
                    visit(call, result, seen, &mut out);
                }
            }
        }
        _ => (),
    }
    out
}

#[cfg(feature = "proxy")]
pub(crate) fn how_tool() -> Value {
    json!({"name":"how","description":"Find commands, typed templates and examples. Fill templates from the request and model state. Read-only.","inputSchema":{"type":"object","properties":{"task":{"type":"string","maxLength":256}},"required":["task"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false}})
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

/// Placement facts use only numeric call parameters and explicitly created bodies.
/// SolidCraft 0.20's public binary confirms the default box corner is the origin.
#[cfg(feature = "proxy")]
pub(crate) fn placement_hints(name: &str, args: &Value, result: &Value) -> Vec<String> {
    fn number(v: &Value) -> Option<f64> {
        let n = v
            .as_f64()
            .or_else(|| v.as_str().filter(|s| s.len() <= 64)?.parse().ok())?;
        (n.is_finite() && n.abs() <= 1_000_000.0).then_some(n)
    }
    fn point(v: &Value) -> Option<[f64; 3]> {
        let a = v.as_array()?;
        if a.len() != 3 {
            return None;
        }
        Some([number(&a[0])?, number(&a[1])?, number(&a[2])?])
    }
    fn box_hint(call: &Value, result: &Value) -> Option<(Vec<String>, String)> {
        if call["command"] != "solid.box"
            || result["isError"] == true
            || result["ok"] == false
            || result.get("error").is_some_and(|v| !v.is_null())
        {
            return None;
        }
        let p = &call["params"];
        let size = [
            number(&p["length"])?,
            number(&p["width"])?,
            number(&p["height"])?,
        ];
        if size.iter().any(|n| *n <= 0.0)
            || (p.get("corner").is_some() && p.get("center").is_some())
        {
            return None;
        }
        let lo = if let Some(v) = p.get("center") {
            let c = point(v)?;
            std::array::from_fn(|i| c[i] - size[i] / 2.0)
        } else if let Some(v) = p.get("corner") {
            point(v)?
        } else {
            [0.0; 3]
        };
        let hi: [f64; 3] = std::array::from_fn(|i| lo[i] + size[i]);
        let c: [f64; 3] = std::array::from_fn(|i| lo[i] + size[i] / 2.0);
        let names = result["created_bodies"]
            .as_array()?
            .iter()
            .take(4)
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
            .map(str::to_owned)
            .collect();
        Some((names, format!("spans X {}..{}, Y {}..{}, Z {}..{} mm; its centre is ({}, {}, {}) and its top face centre is ({}, {}, {}).", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], c[0], c[1], c[2], c[0], c[1], hi[2])))
    }
    if result["isError"] == true
        || result["ok"] == false
        || result.get("error").is_some_and(|v| !v.is_null())
    {
        return Vec::new();
    }
    let mut hints = Vec::new();
    let mut seen = Vec::new();
    let pairs: Vec<(&Value, &Value)> = match name {
        "execute" => vec![(args, result)],
        "batch" => args["commands"]
            .as_array()
            .zip(result["results"].as_array())
            .map(|(calls, results)| calls.iter().zip(results).take(128).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for (call, result) in pairs {
        if let Some((names, facts)) = box_hint(call, result) {
            for name in names {
                if hints.len() < 4 && !seen.contains(&name) {
                    let line = format!("{name} {facts}");
                    if line.len() <= 1024 {
                        hints.push(line);
                        seen.push(name);
                    }
                }
            }
        }
    }
    hints
}

#[cfg(all(test, feature = "proxy"))]
#[allow(clippy::unwrap_used)]
mod placement_tests {
    use super::*;

    #[test]
    fn sketch_entity_errors_existing_constraints_and_bounded_batches() {
        for entities in [json!(["l1", "l2", "l3"]), json!("l1"), json!({"line":"l1"})] {
            let call = json!({"command":"sketch.dimension","params":{"entities":entities}});
            for result in [
                json!({"error":"`entities` must list one or two sketch entities"}),
                json!({"isError":true,"content":[{"type":"text","text":"entities must be a list"}]}),
                json!({"error":{"message":"entities must be an array"}}),
            ] {
                let mut seen = [false; 2];
                assert_eq!(
                    sketch_hints("execute", &call, &result, &mut seen),
                    [SKETCH_ENTITY_HINT]
                );
                assert!(sketch_hints("execute", &call, &result, &mut seen).is_empty());
            }
        }
        let dimension = json!({"command":"sketch.dimension","params":{"entities":["l1"]}});
        for result in [
            json!({"ok":true,"entities":["l1"]}),
            json!({"error":"value must be a number"}),
            json!({"isError":false,"entities":["l1"]}),
        ] {
            assert!(sketch_hints("execute", &dimension, &result, &mut [false; 2]).is_empty());
        }
        let constraint = json!({"command":"sketch.constraint.coincident"});
        assert_eq!(
            sketch_hints(
                "execute",
                &constraint,
                &json!({"content":[{"type":"text","text":"{\"existing\":true}"}]}),
                &mut [false; 2]
            ),
            [SKETCH_EXISTING_HINT]
        );
        assert!(sketch_hints(
            "execute",
            &constraint,
            &json!({"existing":false}),
            &mut [false; 2]
        )
        .is_empty());
        assert!(sketch_hints(
            "execute",
            &dimension,
            &json!({"existing":true}),
            &mut [false; 2]
        )
        .is_empty());
        let calls = vec![constraint; 200];
        let results = vec![json!({"existing":true}); 200];
        assert_eq!(
            sketch_hints(
                "batch",
                &json!({"commands":calls}),
                &json!({"results":results}),
                &mut [false; 2]
            ),
            [SKETCH_EXISTING_HINT]
        );
    }

    fn call(params: Value) -> Value {
        json!({"command":"solid.box", "params":params})
    }
    fn created(name: &str) -> Value {
        json!({"created_bodies":[name]})
    }

    #[test]
    fn numeric_box_placements_and_expressions() {
        let p = json!({"length":20,"width":20,"height":20});
        let hints = placement_hints("execute", &call(p.clone()), &created("Body1"));
        assert_eq!(hints, ["Body1 spans X 0..20, Y 0..20, Z 0..20 mm; its centre is (10, 10, 10) and its top face centre is (10, 10, 20)."]);
        let mut p = p;
        p["corner"] = json!([-10, 2, 3]);
        assert!(
            placement_hints("execute", &call(p.clone()), &created("Body1"))[0]
                .contains("X -10..10, Y 2..22, Z 3..23")
        );
        p.as_object_mut().unwrap().remove("corner");
        p["center"] = json!([0, 0, 0]);
        assert!(
            placement_hints("execute", &call(p.clone()), &created("Body1"))[0]
                .contains("centre is (0, 0, 0) and its top face centre is (0, 0, 10)")
        );
        p["length"] = json!("20");
        assert_eq!(
            placement_hints("execute", &call(p.clone()), &created("Body1")).len(),
            1
        );
        for bad in [
            json!("W / 2"),
            json!(0),
            json!(-1),
            json!(1e20),
            json!(null),
        ] {
            p["length"] = bad;
            assert!(placement_hints("execute", &call(p.clone()), &created("Body1")).is_empty());
        }
    }

    #[test]
    fn success_evidence_and_bounded_batches() {
        let c = call(json!({"length":20,"width":20,"height":20}));
        for result in [
            json!({}),
            json!({"bodies":["Body1"]}),
            json!({"created_bodies":["Body1"],"error":"failed"}),
            json!({"created_bodies":["Body1"],"isError":true}),
        ] {
            assert!(placement_hints("execute", &c, &result).is_empty());
        }
        let calls = vec![c; 8];
        let results: Vec<Value> = (0..8).map(|i| created(&format!("Body{i}"))).collect();
        assert_eq!(
            placement_hints(
                "batch",
                &json!({"commands":calls}),
                &json!({"results":results})
            )
            .len(),
            4
        );
        let c = call(json!({"length":20,"width":20,"height":20}));
        assert_eq!(
            placement_hints(
                "batch",
                &json!({"commands":[{"command":"sketch.create"},c]}),
                &json!({"results":[{},created("Body2")]})
            )
            .len(),
            1
        );
        assert!(placement_hints(
            "execute",
            &json!({"command":"solid.sphere","params":{"radius":5}}),
            &created("Body1")
        )
        .is_empty());
    }
}

#[cfg(all(test, feature = "proxy"))]
mod edge_tests {
    use super::*;
    #[test]
    fn edge_errors_are_scoped_bounded_and_deduplicated() {
        for command in ["solid.fillet", "solid.chamfer"] {
            for error in [
                "an edge is a point [x,y,z], {point} or {body, index}",
                "no edge at [0, 0, 0]: the nearest edge is 40.000 mm away",
            ] {
                let args = json!({"command":command});
                for result in [
                    json!({"error":error}),
                    json!({"isError":true,"content":[{"type":"text","text":error}]}),
                ] {
                    let mut seen = false;
                    assert_eq!(
                        edge_hints("execute", &args, &result, &mut seen),
                        [EDGE_HINT]
                    );
                    assert!(edge_hints("execute", &args, &result, &mut seen).is_empty());
                    assert_eq!(
                        edge_hints(
                            "batch",
                            &json!({"commands":[args,args]}),
                            &json!({"results":[result,result]}),
                            &mut false
                        ),
                        [EDGE_HINT]
                    );
                }
            }
        }
        assert!(edge_hints(
            "execute",
            &json!({"command":"solid.box"}),
            &json!({"error":"no edge at [0,0,0]"}),
            &mut false
        )
        .is_empty());
        assert!(edge_hints(
            "execute",
            &json!({"command":"solid.fillet"}),
            &json!({"ok":true}),
            &mut false
        )
        .is_empty());
    }
}
