//! Contact-circle revolute joints, measured through the public 0.20 MCP interface.
use super::*;

const JOINT: &str = "PilotPivot";
const EPS: f64 = 0.001;
type Matrix = [[f64; 4]; 4];

#[derive(Debug, serde::Serialize)]
struct JointSlots<'a> {
    base: &'a str,
    arm: &'a str,
    pin_circle: [f64; 3],
    hole_circle: [f64; 3],
    flip: bool,
    angle_deg: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JointExpected {
    base: String,
    arm: String,
    base_transform: Matrix,
    arm_transform: Matrix,
    pivot: [f64; 3],
    angle: f64,
}

pub(crate) fn candidate(text: &str) -> bool {
    let t = tokens(text);
    t.iter()
        .any(|s| matches!(s.as_str(), "revolute" | "pivot" | "pivots" | "hinge"))
        || t.first().is_some_and(|s| s == "pin")
        || text.to_ascii_lowercase().contains("pin joint")
        || text.to_ascii_lowercase().contains("on the pin")
}

// Every semantic word is accounted for: extra operations, choices and vague angles defer.
fn angle(text: &str, names: &[&str]) -> Option<(f64, Vec<(bool, f64)>)> {
    let words = tokens(text);
    let action = words.iter().any(|s| {
        matches!(
            s.as_str(),
            "connect" | "join" | "attach" | "make" | "create" | "add" | "use" | "put" | "rotate"
        )
    }) || words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "hinge" | "pivot" | "pin"));
    if !action && !words.windows(2).any(|w| w == ["revolute", "joint"]) {
        return None;
    }
    let mut text = text.to_ascii_lowercase();
    for name in names {
        text = text.replace(&name.to_ascii_lowercase(), " ");
    }
    let original = tokens(&text.replace("'s", " ").replace('°', " degrees "));
    let mut t = Vec::new();
    let mut sizes = Vec::new();
    let mut n = 0;
    while n < original.len() {
        if matches!(original[n].as_str(), "pin" | "hole")
            && original
                .get(n + 1)
                .is_some_and(|s| matches!(s.as_str(), "diameter" | "radius"))
        {
            let suffix = usize::from(
                original
                    .get(n + 3)
                    .is_some_and(|s| slots::scale(s).is_some()),
            );
            let dims = dimensions(original.get(n + 2..n + 3 + suffix)?)?;
            let [dim] = dims.as_slice() else {
                return None;
            };
            if dim.value <= 0. {
                return None;
            }
            let pin = original[n] == "pin";
            if sizes.iter().any(|(p, _)| *p == pin) {
                return None;
            }
            sizes.push((
                pin,
                dim.value
                    / if original[n + 1] == "diameter" {
                        2.
                    } else {
                        1.
                    },
            ));
            n += 3 + suffix;
        } else {
            t.push(original[n].clone());
            n += 1;
        }
    }
    let mut angles = Vec::new();
    let mut unknown = Vec::new();
    for (n, s) in t.iter().enumerate() {
        if let Ok(v) = s.parse::<f64>() {
            if t.get(n + 1)
                .is_some_and(|s| matches!(s.as_str(), "degrees" | "degree" | "deg"))
            {
                if n > 0 && matches!(t[n - 1].as_str(), "about" | "around") {
                    return None;
                }
                if validate_angle(v).is_err() {
                    return None;
                }
                angles.push(v);
            } else {
                return None;
            }
        } else if !matches!(
            s.as_str(),
            "connect"
                | "join"
                | "attach"
                | "make"
                | "create"
                | "add"
                | "use"
                | "put"
                | "a"
                | "an"
                | "the"
                | "and"
                | "then"
                | "with"
                | "to"
                | "on"
                | "over"
                | "of"
                | "in"
                | "at"
                | "is"
                | "are"
                | "has"
                | "have"
                | "its"
                | "their"
                | "them"
                | "it"
                | "both"
                | "two"
                | "existing"
                | "separate"
                | "component"
                | "components"
                | "revolute"
                | "joint"
                | "pivot"
                | "pivots"
                | "hinge"
                | "pin"
                | "boss"
                | "round"
                | "cylindrical"
                | "hole"
                | "sits"
                | "sitting"
                | "flat"
                | "top"
                | "stays"
                | "stay"
                | "keep"
                | "keeping"
                | "upright"
                | "fixed"
                | "stationary"
                | "grounded"
                | "ground"
                | "must"
                | "should"
                | "not"
                | "move"
                | "unchanged"
                | "so"
                | "that"
                | "around"
                | "about"
                | "drive"
                | "swing"
                | "rotate"
                | "rotated"
                | "turn"
                | "turned"
                | "degrees"
                | "degree"
                | "deg"
        ) {
            if names.is_empty()
                && !matches!(
                    s.as_str(),
                    "or" | "if"
                        | "unless"
                        | "maybe"
                        | "roughly"
                        | "between"
                        | "clockwise"
                        | "counterclockwise"
                        | "radians"
                        | "translate"
                        | "delete"
                        | "copy"
                        | "fillet"
                        | "slider"
                        | "ball"
                        | "rigid"
                )
            {
                if !unknown.contains(s) {
                    unknown.push(s.clone());
                }
                if unknown.len() > 16 {
                    return None;
                }
            } else {
                return None;
            }
        }
    }
    // Negation is permitted only for the explicit stationary-base instruction.
    if t.iter()
        .enumerate()
        .any(|(n, s)| s == "not" && t.get(n + 1).is_none_or(|s| s != "move"))
    {
        return None;
    }
    if t.iter()
        .enumerate()
        .any(|(n, s)| s == "move" && (n == 0 || t[n - 1] != "not"))
    {
        return None;
    }
    // A bare joint name remains a search; terse instructions need a rotation.
    if (!action && angles.is_empty())
        || angles.len() > 1
        || (angles.is_empty()
            && t.iter().any(|s| {
                matches!(
                    s.as_str(),
                    "drive" | "swing" | "rotate" | "rotated" | "turn" | "turned"
                )
            }))
    {
        return None;
    }
    Some((angles.first().copied().unwrap_or(0.0), sizes))
}

#[cfg(feature = "proxy")]
pub(crate) const JOINT_HINT: &str = "Pick the contact circles: pin root on the base top face and hole bottom edge where the parts meet; use flip: true to keep the arm upright.";

/// joint.list reports world frames; recover the local point before comparing placements.
#[cfg(feature = "proxy")]
pub(crate) fn result_hint(request: &Value) -> bool {
    let check = || -> Option<bool> {
        let messages = request["messages"].as_array()?;
        let start = messages.iter().rposition(|m| m["role"] == "user")?;
        let create = messages[start + 1..].iter().position(|m| {
            m["tool_calls"].as_array().is_some_and(|cs| {
                cs.iter().any(|c| {
                    c["function"]["arguments"]
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .is_some_and(|a| {
                            a["command"] == "joint.create"
                                || a["commands"].as_array().is_some_and(|cs| {
                                    cs.iter().any(|c| c["command"] == "joint.create")
                                })
                        })
                })
            })
        })? + start
            + 1;
        let before = payloads(&messages[..create])?;
        let after = payloads(&messages[create + 1..])?;
        let old = hint_occurrences(
            before
                .iter()
                .rev()
                .find(|d| d.get("components").is_some())?,
        )?;
        let new = hint_occurrences(after.iter().rev().find(|d| d.get("components").is_some())?)?;
        let joints = after.iter().rev().find(|d| d.get("joints").is_some())?;
        for j in array(&joints["joints"], 64)? {
            if j["type"] != "revolute" {
                continue;
            }
            for side in ["a", "b"] {
                let was = old.iter().find(|o| o["id"] == j[side])?;
                let now = new.iter().find(|o| o["id"] == j[side])?;
                let a = was.get("transform").map_or(Some(identity()), matrix)?;
                let b = now.get("transform").map_or(Some(identity()), matrix)?;
                let p = point(&j[format!("frame_{side}")]["origin"])?;
                let delta = [p[0] - b[3][0], p[1] - b[3][1], p[2] - b[3][2]];
                let local = [
                    b[0][0] * delta[0] + b[0][1] * delta[1] + b[0][2] * delta[2],
                    b[1][0] * delta[0] + b[1][1] * delta[1] + b[1][2] * delta[2],
                    b[2][0] * delta[0] + b[2][1] * delta[1] + b[2][2] * delta[2],
                ];
                if !close(apply(a, local), p, 0.5) || a[2][2] * b[2][2] < 0. {
                    return Some(true);
                }
            }
        }
        Some(false)
    };
    check() == Some(true)
}

fn array(v: &Value, cap: usize) -> Option<&Vec<Value>> {
    v.as_array().filter(|a| a.len() <= cap)
}

#[cfg(feature = "proxy")]
fn hint_occurrences(data: &Value) -> Option<Vec<&Value>> {
    let mut out = Vec::new();
    for c in array(&data["components"], 64)? {
        for o in array(&c["occurrences"], 64)? {
            out.push(o);
            if out.len() > 128 {
                return None;
            }
        }
    }
    Some(out)
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite() && v.abs() <= MAX_MM)
}
fn point(v: &Value) -> Option<[f64; 3]> {
    let a = array(v, 3)?;
    if a.len() != 3 {
        return None;
    }
    Some([number(&a[0])?, number(&a[1])?, number(&a[2])?])
}
fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f64>() <= tol * tol
}
fn matrix(v: &Value) -> Option<Matrix> {
    let rows = array(v, 4)?;
    if rows.len() != 4 {
        return None;
    }
    let mut out = [[0.0; 4]; 4];
    for (i, row) in rows.iter().enumerate() {
        let row = array(row, 4)?;
        if row.len() != 4 {
            return None;
        }
        for (j, v) in row.iter().enumerate() {
            out[i][j] = number(v)?;
        }
    }
    Some(out)
}
fn identity() -> Matrix {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}
fn same_matrix(a: Matrix, b: Matrix) -> bool {
    a.iter()
        .flatten()
        .zip(b.iter().flatten())
        .all(|(a, b)| (a - b).abs() < EPS)
}
#[cfg(feature = "proxy")]
fn apply(m: Matrix, p: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

// Flatten only the documented bounded batch envelope; malformed/error pages never supply facts.
fn payloads(messages: &[Value]) -> Option<Vec<Value>> {
    let mut out = Vec::new();
    for m in messages.iter().filter(|m| m["role"] == "tool") {
        let data = tool_payload(m)?;
        if let Some(results) = data.get("results") {
            for result in array(results, 64)? {
                let result = result.get("result").unwrap_or(result);
                if result.get("error").is_some_and(|e| !e.is_null()) || result["isError"] == true {
                    return None;
                }
                out.push(guidance::payload(result));
            }
        } else {
            out.push(data);
        }
        if out.len() > 256 {
            return None;
        }
    }
    Some(out)
}

fn occurrences(data: &Value) -> Option<Vec<(&Value, &Value)>> {
    if !array(&data["pending_moves"], 64)?.is_empty() {
        return None;
    }
    let components = array(&data["components"], 64)?;
    let root = components.iter().find(|c| c["id"] == 0)?;
    if !array(&root["bodies"], 64)?.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for c in components.iter().filter(|c| c["id"] != 0) {
        let name = c["name"].as_str()?;
        if name.is_empty()
            || name.len() > 128
            || c["parent"] != 0
            || !array(&c["children"], 64)?.is_empty()
        {
            return None;
        }
        let bodies = array(&c["bodies"], 2)?;
        if bodies.len() != 1 {
            return None;
        }
        let os = array(&c["occurrences"], 2)?;
        let [o] = os.as_slice() else {
            return None;
        };
        if o["parent"] != 0
            || o["component"] != c["id"]
            || o["name"].as_str()?.is_empty()
            || o["name"].as_str()?.len() > 128
        {
            return None;
        }
        c["id"].as_u64()?;
        o["id"].as_u64()?;
        o["grounded"].as_bool()?;
        out.push((c, o));
    }
    if let [(a, oa), (b, ob)] = out.as_slice() {
        if a["id"] == b["id"]
            || a["name"] == b["name"]
            || a["bodies"] == b["bodies"]
            || oa["id"] == ob["id"]
            || oa["name"] == ob["name"]
        {
            return None;
        }
    }
    (out.len() == 2).then_some(out)
}

#[derive(Clone)]
struct Circle {
    center: [f64; 3],
    pick: [f64; 3],
    radius: f64,
    pin: bool,
}
#[cfg(feature = "proxy")]
pub(crate) fn contact_summary(faces: &Value, edges: &Value) -> Vec<Value> {
    circles(faces, edges).unwrap_or_default().into_iter().take(24).map(|c| json!({"body":faces["body"],"kind":if c.pin { "pin" } else { "hole" },"centre":c.center,"radius":c.radius,"axis":[0,0,1],"pick":c.pick})).collect()
}
fn circles(faces: &Value, edges: &Value) -> Option<Vec<Circle>> {
    let faces = array(&faces["faces"], 256)?;
    let edges = array(&edges["edges"], 256)?;
    let mut out: Vec<Circle> = Vec::new();
    for face in faces.iter().filter(|f| f.get("cylinder").is_some()) {
        if face["exact"] != true {
            return None;
        }
        let cylinder = &face["cylinder"];
        let origin = point(&cylinder["origin"])?;
        let axis = point(&cylinder["axis"])?;
        let pin = close(axis, [0., 0., 1.], EPS);
        if !pin && !close(axis, [0., 0., -1.], EPS) {
            return None;
        }
        let radius = number(&cylinder["radius"])?;
        if radius <= EPS {
            return None;
        }
        let name = face["name"].as_str()?;
        for edge in edges {
            let names: Vec<_> = edge["name"].as_str()?.split('|').collect();
            if !names.contains(&name) {
                continue;
            }
            let other = faces.iter().find(|f| {
                f["name"]
                    .as_str()
                    .is_some_and(|n| n != name && names.contains(&n))
            });
            let Some(other) = other else {
                continue;
            };
            if other["exact"] != true {
                return None;
            }
            // A pin cap ends at its radius; its root adjoins the supporting face.
            if pin && number(&other["area"])? <= PI * radius * radius * 1.05 {
                continue;
            }
            let Some(normal) = point(&other["plane_normal"]) else {
                continue;
            };
            if !close(normal, [0., 0., if pin { 1. } else { -1. }], EPS) {
                continue;
            }
            let pick = point(&edge["mid"])?;
            let center = [origin[0], origin[1], pick[2]];
            let mut valid = true;
            for key in ["start", "mid", "end"] {
                let p = point(&edge[key])?;
                valid &= (p[2] - pick[2]).abs() < EPS
                    && (((p[0] - origin[0]).powi(2) + (p[1] - origin[1]).powi(2)).sqrt() - radius)
                        .abs()
                        < EPS;
            }
            if valid
                && !out.iter().any(|c| {
                    close(c.center, center, EPS) && (c.radius - radius).abs() < EPS && c.pin == pin
                })
            {
                out.push(Circle {
                    center,
                    pick,
                    radius,
                    pin,
                });
            }
        }
    }
    Some(out)
}

fn read_plan(calls: Vec<Value>) -> Plan {
    Plan {
        rule: "resolve_joint",
        calls,
        facts: None,
        slots: json!({"prerequisite":"inspect components and contact circles"}),
        expected: None,
        continuation: None,
    }
}

fn requested(messages: &[Value], planned: &[Value]) -> bool {
    messages
        .iter()
        .filter_map(|m| m["tool_calls"].as_array())
        .flatten()
        .any(|c| {
            let Some(args) = c["function"]["arguments"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
            else {
                return false;
            };
            let same = |a: &Value| planned.iter().any(|p| p["arguments"] == *a);
            same(&args)
                || args["commands"]
                    .as_array()
                    .is_some_and(|cs| cs.iter().any(same))
        })
}

pub(crate) fn select(
    request: &Value,
    text: &str,
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    select_inner(request, text, catalog, None)
}
pub(super) fn typed(
    request: &Value,
    catalog: &Catalog,
    angle: f64,
    names: [Option<&str>; 2],
) -> Result<Option<Plan>, String> {
    validate_angle(angle)?;
    select_inner(request, "", catalog, Some((angle, names)))
}
pub(super) fn validate_angle(value: f64) -> Result<(), String> {
    if !value.is_finite() || value.abs() > 360.0 {
        return Err(format!(
            "angle_deg {value} must be within -360..360 degrees; reduce the rotation to this range"
        ));
    }
    Ok(())
}
fn select_inner(
    request: &Value,
    text: &str,
    catalog: &Catalog,
    typed: Option<(f64, [Option<&str>; 2])>,
) -> Result<Option<Plan>, String> {
    let Some(messages) = request["messages"].as_array() else {
        return Ok(None);
    };
    let Some(start) = messages.iter().rposition(|m| m["role"] == "user") else {
        return Ok(None);
    };
    // No mutations or arbitrary model calls may precede a newly selected action.
    for m in &messages[start + 1..] {
        if let Some(calls) = m["tool_calls"].as_array() {
            for c in calls {
                let args = c["function"]["arguments"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or(Value::Null);
                let read = |c: &Value| {
                    matches!(
                        c["command"].as_str(),
                        Some(
                            "document.inspect"
                                | "timeline.inspect"
                                | "component.list"
                                | "joint.list"
                                | "model.faces"
                                | "model.edges"
                        )
                    )
                };
                if !(c["function"]["name"] == "execute" && read(&args)
                    || c["function"]["name"] == "batch"
                        && array(&args["commands"], 64).is_some_and(|cs| cs.iter().all(read)))
                {
                    return Ok(None);
                }
            }
        }
    }
    let Some(data) = payloads(&messages[start + 1..]) else {
        return Ok(None);
    };
    let components = data.iter().rev().find(|d| d.get("components").is_some());
    let mut names: Vec<_> = components
        .and_then(occurrences)
        .unwrap_or_default()
        .iter()
        .flat_map(|(c, o)| [c["name"].as_str(), o["name"].as_str()])
        .flatten()
        .collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let (angle, sizes) = if let Some((angle, _)) = typed {
        (angle, Vec::new())
    } else {
        let Some(parsed) = angle(text, &names) else {
            return Ok(None);
        };
        parsed
    };
    let user_text = messages[start]["content"].as_str().unwrap_or("");
    if user_text.len() > 65_536 {
        return Err("joint rotation request exceeds 65536 bytes".into());
    }
    if data.is_empty() {
        if messages[start + 1..]
            .iter()
            .any(|m| m["tool_calls"].is_array())
        {
            return Ok(None);
        }
        return Ok(Some(read_plan(vec![
            call(catalog, "document.inspect", json!({"measure":true}))?,
            call(catalog, "component.list", json!({}))?,
            call(catalog, "joint.list", json!({}))?,
        ])));
    }
    let Some(baseline) = data.iter().rev().find(|d| d.get("bodies").is_some()) else {
        return Ok(None);
    };
    let Some(os) = components.and_then(occurrences) else {
        return Ok(None);
    };
    let Some(joints) = data.iter().rev().find(|d| d.get("joints").is_some()) else {
        return Ok(None);
    };
    if !array(&joints["joints"], 64).is_some_and(|j| j.is_empty())
        || !array(&joints["origins"], 64).is_some_and(|j| j.is_empty())
    {
        return Ok(None);
    }
    if array(&baseline["bodies"], 2).is_none_or(|b| b.len() != 2) || volume(baseline).is_none() {
        return Ok(None);
    }
    let mut reads = Vec::new();
    let mut all = Vec::new();
    for (c, o) in &os {
        if !same_matrix(matrix(&o["transform"]).unwrap_or([[0.; 4]; 4]), identity()) {
            return Ok(None);
        }
        let Some(body) = c["bodies"][0].as_str() else {
            return Ok(None);
        };
        let faces = data
            .iter()
            .rev()
            .find(|d| d["body"] == body && d.get("faces").is_some());
        let edges = data
            .iter()
            .rev()
            .find(|d| d["body"] == body && d.get("edges").is_some());
        match (faces, edges) {
            (Some(f), Some(e)) => {
                let Some(cs) = circles(f, e) else {
                    return Ok(None);
                };
                for circle in cs {
                    all.push((*o, circle));
                }
            }
            _ => {
                reads.push(call(catalog, "model.faces", json!({"body":body}))?);
                reads.push(call(catalog, "model.edges", json!({"body":body}))?);
            }
        }
    }
    if !reads.is_empty() {
        if requested(&messages[start + 1..], &reads) {
            return Ok(None);
        }
        return Ok(Some(read_plan(reads)));
    }
    let mut pairs = Vec::new();
    for (base, pin) in all.iter().filter(|(_, c)| c.pin) {
        for (arm, hole) in all.iter().filter(|(_, c)| !c.pin) {
            if base["id"] != arm["id"]
                && close(pin.center, hole.center, 0.5)
                && (pin.center[2] - hole.center[2]).abs() < EPS
                && hole.radius >= pin.radius
            {
                pairs.push((*base, pin, *arm, hole));
            }
        }
    }
    let [(base, pin, arm, hole)] = pairs.as_slice() else {
        return Ok(None);
    };
    if let Some((_, names)) = typed {
        for ((field, name), occurrence) in
            [("pin_component", names[0]), ("hole_component", names[1])]
                .into_iter()
                .zip([base, arm])
        {
            if let Some(name) = name {
                let matches = os
                    .iter()
                    .filter(|(c, o)| {
                        [o["name"].as_str(), c["name"].as_str()]
                            .into_iter()
                            .flatten()
                            .any(|n| n.eq_ignore_ascii_case(name))
                    })
                    .collect::<Vec<_>>();
                if matches.len() != 1 || matches[0].1["id"] != occurrence["id"] {
                    return Err(format!(
                        "{field} {name:?} matched {} components but requires exactly 1 measured contact component; inspect and use its exact name", matches.len()
                    ));
                }
            }
        }
    }
    if sizes
        .iter()
        .any(|(p, r)| (if *p { pin.radius } else { hole.radius } - r).abs() > EPS)
    {
        return Ok(None);
    }
    if arm["grounded"] != false {
        return Ok(None);
    }
    let Some(base_name) = base["name"].as_str() else {
        return Ok(None);
    };
    let Some(arm_name) = arm["name"].as_str() else {
        return Ok(None);
    };
    let base_component = os
        .iter()
        .find(|(_, o)| o["id"] == base["id"])
        .and_then(|(c, _)| c["name"].as_str());
    let arm_component = os
        .iter()
        .find(|(_, o)| o["id"] == arm["id"])
        .and_then(|(c, _)| c["name"].as_str());
    let lower = user_text.to_ascii_lowercase();
    for name in [Some(base_name), base_component].into_iter().flatten() {
        for verb in ["swing", "rotate", "rotated", "turn", "turned", "drive"] {
            if [
                format!("{verb} {}", name.to_ascii_lowercase()),
                format!("{verb} the {}", name.to_ascii_lowercase()),
            ]
            .iter()
            .any(|s| lower.contains(s))
            {
                return Ok(None);
            }
        }
    }
    for name in [Some(arm_name), arm_component].into_iter().flatten() {
        let name = name.to_ascii_lowercase();
        if [
            format!("{name} must not move"),
            format!("{name} should not move"),
            format!("keep {name} fixed"),
            format!("keep the {name} fixed"),
            format!("keep {name} stationary"),
            format!("ground {name}"),
        ]
        .iter()
        .any(|s| lower.contains(s))
        {
            return Ok(None);
        }
    }
    let (s, c) = angle.to_radians().sin_cos();
    let mut transform = identity();
    transform[0][0] = c;
    transform[0][1] = -s;
    transform[1][0] = s;
    transform[1][1] = c;
    // component.list uses row vectors: the translation occupies the final row.
    transform[3][0] = pin.center[0] - c * hole.center[0] - s * hole.center[1];
    transform[3][1] = pin.center[1] + s * hole.center[0] - c * hole.center[1];
    let expected = JointExpected {
        base: base_name.into(),
        arm: arm_name.into(),
        base_transform: identity(),
        arm_transform: transform,
        pivot: pin.center,
        angle,
    };
    let mut calls = vec![
        call(
            catalog,
            "occurrence.ground",
            json!({"occurrence":base_name,"grounded":true}),
        )?,
        call(
            catalog,
            "joint.create",
            json!({"name":JOINT,"type":"revolute","a":{"occurrence":base_name,"circle":pin.pick},"b":{"occurrence":arm_name,"circle":hole.pick},"flip":true,"angle":0}),
        )?,
    ];
    if typed.is_some() || angle != 0. {
        calls.push(call(
            catalog,
            "joint.drive",
            json!({"joint":JOINT,"value":angle}),
        )?);
    }
    calls.extend([
        call(catalog, "component.list", json!({}))?,
        call(catalog, "joint.list", json!({}))?,
        call(catalog, "document.inspect", json!({"measure":true}))?,
    ]);
    Ok(Some(Plan {
        rule: "revolute_joint",
        calls,
        facts: None,
        slots: serde_json::to_value(JointSlots {
            base: base_name,
            arm: arm_name,
            pin_circle: pin.pick,
            hole_circle: hole.pick,
            flip: true,
            angle_deg: angle,
        })
        .map_err(|e| format!("cannot audit revolute joint slots: {e}"))?,
        expected: Some(Expected {
            volume_range_mm3: None,
            bodies: Some(2),
            volume_mm3: volume(baseline),
            name: None,
            created_volume_mm3: None,
            placement: None,
            feature: None,
            joint: Some(expected),
        }),
        continuation: None,
    }))
}

impl JointExpected {
    pub(crate) fn check(&self, request: &Value) -> Option<String> {
        let valid = || -> Option<bool> {
            let messages = request["messages"].as_array()?;
            let start = messages.iter().rposition(|m| {
                m["role"] == "assistant"
                    && m["tool_calls"].as_array().is_some_and(|cs| {
                        cs.iter().any(|c| {
                            c["function"]["arguments"]
                                .as_str()
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                .is_some_and(|a| {
                                    let mutation = |v: &Value| {
                                        matches!(
                                            v["command"].as_str(),
                                            Some(
                                                "joint.create"
                                                    | "joint.drive"
                                                    | "occurrence.ground"
                                            )
                                        )
                                    };
                                    mutation(&a)
                                        || a["commands"]
                                            .as_array()
                                            .is_some_and(|cs| cs.iter().any(mutation))
                                })
                        })
                    })
            })?;
            let data = payloads(&messages[start + 1..])?;
            let components = data.iter().rev().find(|d| d.get("components").is_some())?;
            let os = occurrences(components)?;
            let base = os.iter().find(|(_, o)| o["name"] == self.base)?.1;
            let arm = os.iter().find(|(_, o)| o["name"] == self.arm)?.1;
            let joints = data.iter().rev().find(|d| d.get("joints").is_some())?;
            let js = array(&joints["joints"], 64)?;
            let [j] = js.as_slice() else {
                return Some(false);
            };
            Some(
                base["grounded"] == true
                    && same_matrix(matrix(&base["transform"])?, self.base_transform)
                    && same_matrix(matrix(&arm["transform"])?, self.arm_transform)
                    && j["name"] == JOINT
                    && j["type"] == "revolute"
                    && j["suppressed"] == false
                    && j["flip"] == true
                    && j["a"] == base["id"]
                    && j["b"] == arm["id"]
                    && close(point(&j["frame_a"]["origin"])?, self.pivot, EPS)
                    && close(point(&j["frame_b"]["origin"])?, self.pivot, EPS)
                    && (number(&j["values"][0]["value"])? - self.angle).abs() < EPS
                    && array(&joints["errors"], 64)?.is_empty()
                    && array(&joints["conflicts"], 64)?.is_empty()
                    && array(&joints["floating"], 64)?.is_empty()
                    && array(&joints["fixed"], 64)?.contains(&base["id"]),
            )
        };
        (valid()!=Some(true)).then(|| "Revolute joint verification failed: inspect component.list and joint.list; the base must stay fixed, the arm upright, and both contact-circle pivots coincident at the requested angle. Do not repeat accepted changes.".into())
    }
}
