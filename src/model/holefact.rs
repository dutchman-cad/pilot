//! One short fact after a hole or circle cut: what it actually made.
//!
//! Everything here is derived from data already in the conversation: the call's
//! own arguments, SolidCraft's result (`changed_bodies`), and body bounds seen in
//! earlier results (`solid.box` arguments, `bbox` in inspect results). Pilot sends
//! no read commands of its own for this. It never rewrites, rejects or replays the
//! model's call, and never cuts anything. When a fact cannot be stated from that
//! data, none is added.
use super::tool_data;
use crate::audit::Audit;
use crate::prompting;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Facts added per tool result, and per user turn across all its tool results.
pub(super) const MAX_PER_RESULT: usize = 4;
pub(super) const MAX_PER_TURN: usize = 6;
const TOL: f64 = 1e-6;
const AXES: [&str; 3] = ["X", "Y", "Z"];

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

fn fmt(n: f64) -> String {
    let r = (n * 1000.0).round() / 1000.0;
    if r == 0.0 {
        "0".into()
    } else {
        format!("{r}")
    }
}

fn pt(p: [f64; 3]) -> String {
    format!("({}, {}, {})", fmt(p[0]), fmt(p[1]), fmt(p[2]))
}

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .take(64)
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

struct Body {
    lo: [f64; 3],
    hi: [f64; 3],
    /// A plain block: its thickness along an outer face is the bounding-box extent.
    plain: bool,
}

struct Sketch {
    name: String,
    id: Option<i64>,
    /// (axis of the plane normal, offset); None when the plane is not a numeric XY/XZ/YZ.
    plane: Option<(usize, f64)>,
    circles: Vec<([f64; 2], f64)>,
    /// Other geometry, dimensions or constraints: circles can no longer be trusted.
    other: bool,
}

impl Sketch {
    /// World point of a sketch point (measured with the public CLI: XY keeps (u, v),
    /// XZ maps (u, v) to (u, y, -v), YZ maps (u, v) to (x, u, v); all normals are positive).
    fn world(&self, uv: [f64; 2]) -> Option<[f64; 3]> {
        let (axis, off) = self.plane?;
        Some(match axis {
            2 => [uv[0], uv[1], off],
            1 => [uv[0], off, -uv[1]],
            _ => [off, uv[0], uv[1]],
        })
    }
}

#[derive(Default)]
struct State {
    bodies: BTreeMap<String, Body>,
    sketches: Vec<Sketch>,
    last: Option<usize>,
}

fn failed(data: &Value) -> bool {
    data["isError"] == true
        || data["ok"] == false
        || data.get("error").is_some_and(|e| !e.is_null())
}

impl State {
    fn read_bodies(&mut self, data: &Value) {
        let Some(list) = data["bodies"].as_array().filter(|a| a.len() <= 1024) else {
            return;
        };
        if !list.iter().all(|b| b.is_object() && b["bbox"].is_object()) || list.is_empty() {
            return;
        }
        let mut next = BTreeMap::new();
        for b in list {
            let (Some(name), Some(lo), Some(hi)) = (
                b["name"].as_str(),
                point(&b["bbox"]["min"]),
                point(&b["bbox"]["max"]),
            ) else {
                continue;
            };
            let block = b["faces"] == 6 && b["face_types"]["plane"] == 6;
            let kept = self.bodies.get(name).is_some_and(|o| {
                o.plain
                    && (0..3)
                        .all(|k| (o.lo[k] - lo[k]).abs() < 1e-3 && (o.hi[k] - hi[k]).abs() < 1e-3)
            });
            next.insert(
                name.to_string(),
                Body {
                    lo,
                    hi,
                    plain: block || kept,
                },
            );
        }
        self.bodies = next;
    }

    fn touched(&mut self, data: &Value) {
        for key in ["changed_bodies", "removed_bodies", "created_bodies"] {
            for n in names(&data[key]) {
                self.bodies.remove(&n);
            }
        }
    }

    fn visit(&mut self, command: &str, p: &Value, data: &Value) -> Option<String> {
        if failed(data) {
            return None;
        }
        self.read_bodies(data);
        match command {
            "solid.box" => self.new_box(p, data),
            "sketch.create" => self.new_sketch(p, data),
            "sketch.circle.center" => self.circle(p),
            "sketch.finish" | "sketch.finish_auto_constrain" => {}
            "solid.hole" => return self.hole(p, data),
            "solid.extrude" => return self.extrude(p, data),
            "document.inspect" | "inspect.measure" => {}
            c if c.starts_with("sketch.") => {
                if c == "sketch.edit" {
                    self.last = None;
                } else if let Some(s) = self.last.and_then(|i| self.sketches.get_mut(i)) {
                    s.other = true;
                }
            }
            _ => self.touched(data),
        }
        None
    }

    fn new_box(&mut self, p: &Value, data: &Value) {
        let created = names(&data["created_bodies"]);
        self.touched(data);
        let (Some(name), 1) = (created.first(), created.len()) else {
            return;
        };
        let size = [
            number(&p["length"]),
            number(&p["width"]),
            number(&p["height"]),
        ];
        let (Some(l), Some(w), Some(h)) = (size[0], size[1], size[2]) else {
            return;
        };
        let size = [l, w, h];
        if size.iter().any(|n| *n <= 0.0)
            || (p.get("corner").is_some() && p.get("center").is_some())
        {
            return;
        }
        let lo = if let Some(v) = p.get("center") {
            let Some(c) = point(v) else { return };
            std::array::from_fn(|i| c[i] - size[i] / 2.0)
        } else if let Some(v) = p.get("corner") {
            let Some(c) = point(v) else { return };
            c
        } else {
            [0.0; 3]
        };
        let hi = std::array::from_fn(|i| lo[i] + size[i]);
        self.bodies.insert(
            name.clone(),
            Body {
                lo,
                hi,
                plain: true,
            },
        );
    }

    fn new_sketch(&mut self, p: &Value, data: &Value) {
        let axis = match p["plane"].as_str() {
            Some("XY") => Some(2),
            Some("XZ") => Some(1),
            Some("YZ") => Some(0),
            _ => None,
        };
        let off = if p.get("offset").is_none() {
            Some(0.0)
        } else {
            number(&p["offset"])
        };
        let plain = p.get("flip").is_none_or(|f| f.is_null() || *f == false)
            && p.get("face").is_none()
            && p.get("frame").is_none_or(|f| *f == "world");
        let plane = axis.zip(off).filter(|_| plain);
        if self.sketches.len() >= 64 {
            self.sketches.remove(0);
        }
        self.sketches.push(Sketch {
            name: data["name"].as_str().unwrap_or("").to_string(),
            id: data["sketch"].as_i64(),
            plane,
            circles: Vec::new(),
            other: false,
        });
        self.last = Some(self.sketches.len() - 1);
    }

    fn circle(&mut self, p: &Value) {
        let Some(s) = self.last.and_then(|i| self.sketches.get_mut(i)) else {
            return;
        };
        if p["construction"] == true {
            return;
        }
        let c = p["center"].as_array().filter(|a| a.len() == 2);
        let d = number(&p["diameter"]).or_else(|| number(&p["radius"]).map(|r| r * 2.0));
        match (c.and_then(|c| Some([number(&c[0])?, number(&c[1])?])), d) {
            (Some(c), Some(d)) if d > 0.0 && s.circles.len() < 16 => s.circles.push((c, d)),
            _ => s.other = true,
        }
    }

    /// The body a result touched, when exactly one.
    fn one(&self, data: &Value) -> Option<(String, &Body)> {
        let changed = names(&data["changed_bodies"]);
        let [name] = changed.as_slice() else {
            return None;
        };
        self.bodies.get(name).map(|b| (name.clone(), b))
    }

    fn hole(&mut self, p: &Value, data: &Value) -> Option<String> {
        let plain_kind = p["type"].is_null() || p["type"] == "simple";
        if !plain_kind
            || [
                "sketch",
                "points",
                "thread",
                "clearance",
                "to",
                "references",
                "single",
            ]
            .iter()
            .any(|k| p.get(*k).is_some_and(|v| !v.is_null()))
        {
            return None;
        }
        let at = point(&p["position"])?;
        let d = number(&p["diameter"]).filter(|d| *d > 0.0)?;
        let depth = match p.get("depth") {
            None | Some(Value::Null) => None,
            Some(v) => Some(number(v).filter(|v| *v > 0.0)?),
        };
        let how = depth.map_or("through all".to_string(), |v| format!("{} mm deep", fmt(v)));
        let head = format!("Hole made: {} mm diameter, {how}", fmt(d));
        let Some((name, b)) = self.one(data) else {
            return Some(format!("{head} at {}.", pt(at)));
        };
        let Some((axis, sign)) = face_axis(b, at) else {
            return Some(format!("{head} at {}.", pt(at)));
        };
        if let Some(dir) = p.get("direction").filter(|v| !v.is_null()) {
            let dir = point(dir)?;
            let across = (0..3).any(|k| k != axis && dir[k].abs() > TOL);
            if across || dir[axis].abs() <= TOL {
                return Some(format!("{head} at {}.", pt(at)));
            }
        }
        let label = format!("{}{}", if sign > 0.0 { "+" } else { "-" }, AXES[axis]);
        let mut out = format!("{head} along {label} from {}", pt(at));
        if b.plain {
            let t = b.hi[axis] - b.lo[axis];
            let through = depth.is_none_or(|v| v >= t - TOL);
            out += &format!(
                "; it {} {name} ({} mm thick)",
                if through {
                    "goes through"
                } else {
                    "does not go through"
                },
                fmt(t)
            );
            out += &edge_note(b, at, axis, d / 2.0, &name, "hole");
        }
        Some(out + ".")
    }

    fn extrude(&mut self, p: &Value, data: &Value) -> Option<String> {
        if p["operation"] != "cut" {
            self.touched(data);
            return None;
        }
        let sketch = match (&p["sketch"], p.get("face"), p.get("faces")) {
            (Value::Null, None, None) => self.last,
            (Value::String(n), None, None) => self.sketches.iter().rposition(|s| &s.name == n),
            (Value::Number(n), None, None) => self
                .sketches
                .iter()
                .rposition(|s| s.id.is_some() && s.id == n.as_i64()),
            _ => None,
        }
        .and_then(|i| self.sketches.get(i));
        let one = sketch
            .filter(|s| !s.other && s.circles.len() == 1)
            .filter(|_| p.get("profiles").is_none() && p.get("taper").is_none());
        let circle = one.and_then(|s| {
            let (uv, d) = s.circles.first()?;
            Some((s.world(*uv)?, *d, s.plane?.0))
        });
        // Interval swept along the plane normal; infinite ends use +-1e9.
        let sweep = circle.and_then(|(c, _, axis)| {
            let off = c[axis];
            let far = 1e9;
            let dist = if p["through_all"] == true {
                far
            } else {
                number(&p["distance"]).filter(|v| *v > 0.0)?
            };
            if p["convention"] == "legacy_per_side" {
                return None;
            }
            let dir = p["direction"].as_str().unwrap_or("positive");
            let sym = matches!(dir, "symmetric");
            Some(match dir {
                "positive" => (off, off + dist, dist, 1.0),
                "negative" => (off - dist, off, dist, -1.0),
                _ if sym => (off - dist / 2.0, off + dist / 2.0, dist, 0.0),
                _ => return None,
            })
        });
        let result_bodies = names(&data["bodies"]);
        let known =
            !result_bodies.is_empty() && result_bodies.iter().all(|n| self.bodies.contains_key(n));
        let mut text = None;
        if let (Some((c, d, axis)), Some((s0, s1, dist, dir)), true) = (circle, sweep, known) {
            let r = d / 2.0;
            let reasons: Vec<Option<String>> = result_bodies
                .iter()
                .filter_map(|n| self.bodies.get(n).map(|b| (n, b)))
                .map(|(n, b)| miss(b, n, c, r, axis, (s0, s1, dist, dir)))
                .collect();
            if reasons.iter().all(Option::is_some) {
                let first = reasons.into_iter().next().flatten();
                text = first.map(|why| format!("This cut removed no material: {why}."));
            }
        }
        if text.is_none() && names(&data["changed_bodies"]).is_empty() {
            text = Some(
                "This cut removed no material: SolidCraft reports that no body changed.".into(),
            );
        }
        if text.is_none() {
            if let (Some((c, d, axis)), Some((s0, s1, dist, dir)), Some((name, b))) =
                (circle, sweep, self.one(data))
            {
                let dir_label = if dir == 0.0 {
                    format!("both ways along {}", AXES[axis])
                } else {
                    format!("along {}{}", if dir > 0.0 { "+" } else { "-" }, AXES[axis])
                };
                let how = if p["through_all"] == true {
                    "through all".to_string()
                } else if dir == 0.0 {
                    format!("{} mm deep in total", fmt(dist))
                } else {
                    format!("{} mm deep", fmt(dist))
                };
                let mut out = format!(
                    "Cut made: {} mm diameter circle at {}, {how} {dir_label}",
                    fmt(d),
                    pt(c)
                );
                if b.plain {
                    let through = s0 <= b.lo[axis] + TOL && s1 >= b.hi[axis] - TOL;
                    out += &format!(
                        "; it {} {name} ({} mm thick)",
                        if through {
                            "goes through"
                        } else {
                            "does not go through"
                        },
                        fmt(b.hi[axis] - b.lo[axis])
                    );
                    out += &edge_note(b, c, axis, d / 2.0, &name, "circle");
                }
                text = Some(out + ".");
            }
        }
        text
    }
}

/// Axis and inward direction (+1 or -1) of the single bounding-box face the point lies on.
fn face_axis(b: &Body, at: [f64; 3]) -> Option<(usize, f64)> {
    let t = TOL * 1e3;
    let mut hit = None;
    for (k, a) in at.iter().enumerate() {
        if *a < b.lo[k] - t || *a > b.hi[k] + t {
            return None;
        }
        let lo = (a - b.lo[k]).abs() <= t;
        let hi = (a - b.hi[k]).abs() <= t;
        if lo || hi {
            if hit.is_some() || (lo && hi) {
                return None;
            }
            hit = Some((k, if lo { 1.0 } else { -1.0 }));
        }
    }
    hit
}

fn edge_note(b: &Body, c: [f64; 3], axis: usize, r: f64, name: &str, what: &str) -> String {
    let near = (0..3)
        .filter(|k| *k != axis)
        .map(|k| (c[k] - b.lo[k]).min(b.hi[k] - c[k]))
        .fold(f64::INFINITY, f64::min);
    if near.is_finite() && r > near + 1e-3 {
        format!(
            "; the {what} reaches past the edge of {name} (its centre is {} mm from the edge, its radius is {} mm)",
            fmt(near.max(0.0)),
            fmt(r)
        )
    } else {
        String::new()
    }
}

/// Why a circle cut cannot touch a body (bounding-box disjoint), or None when it may.
fn miss(
    b: &Body,
    name: &str,
    c: [f64; 3],
    r: f64,
    axis: usize,
    (s0, s1, dist, dir): (f64, f64, f64, f64),
) -> Option<String> {
    for k in (0..3).filter(|k| *k != axis) {
        if c[k] + r <= b.lo[k] + TOL || c[k] - r >= b.hi[k] - TOL {
            let others: Vec<String> = (0..3)
                .filter(|j| *j != axis)
                .map(|j| format!("{} {}..{}", AXES[j], fmt(b.lo[j]), fmt(b.hi[j])))
                .collect();
            return Some(format!(
                "the circle at {} is outside {name} ({})",
                pt(c),
                others.join(", ")
            ));
        }
    }
    if s1 <= b.lo[axis] + TOL || s0 >= b.hi[axis] - TOL {
        let a = AXES[axis];
        let range = format!("{a} {}..{}", fmt(b.lo[axis]), fmt(b.hi[axis]));
        let run = format!("{a} {}..{}", fmt(s0.max(-1e8)), fmt(s1.min(1e8)));
        let mut why = format!("the cut covers {run}, which does not reach {name} ({range})");
        if dir != 0.0 {
            let off = if dir > 0.0 { s0 } else { s1 };
            let (m0, m1) = if dir > 0.0 {
                (off - dist, off)
            } else {
                (off, off + dist)
            };
            if m1 > b.lo[axis] + TOL && m0 < b.hi[axis] - TOL {
                why += &format!(
                    "; the other direction ({}) would reach it",
                    if dir > 0.0 { "negative" } else { "positive" }
                );
            }
        }
        return Some(why);
    }
    None
}

/// Appends facts to the trailing tool results of a request.
pub(super) fn append(body: &mut Value, audit: &mut Audit) -> Result<(), String> {
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("messages must be an array")?;
    let turn = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .map_or(0, |n| n + 1);
    let start = messages
        .iter()
        .rposition(|m| m["role"] != "tool")
        .map_or(0, |i| i + 1);
    let mut state = State::default();
    let mut calls: BTreeMap<String, Value> = BTreeMap::new();
    let mut count = 0;
    for i in 0..messages.len().min(4096) {
        if let Some(cs) = messages[i]["tool_calls"].as_array() {
            for c in cs.iter().take(128) {
                if let Some(id) = c["id"].as_str() {
                    calls.insert(id.to_string(), c.clone());
                }
            }
        }
        if messages[i]["role"] != "tool" {
            continue;
        }
        let Some(call) = messages[i]["tool_call_id"]
            .as_str()
            .and_then(|id| calls.get(id))
        else {
            continue;
        };
        let name = call["function"]["name"].as_str().unwrap_or("");
        let args = &call["function"]["arguments"];
        let args = args
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or_else(|| args.clone());
        let Some(data) = tool_data(&messages[i]) else {
            continue;
        };
        let pairs: Vec<(Value, Value)> = match name {
            "execute" => vec![(args.clone(), data.clone())],
            "batch" if data["ok"] != false => args["commands"]
                .as_array()
                .zip(data["results"].as_array())
                .map(|(c, r)| c.iter().cloned().zip(r.iter().cloned()).take(128).collect())
                .unwrap_or_default(),
            _ => continue,
        };
        let mut facts = Vec::new();
        for (call, result) in &pairs {
            let command = call["command"].as_str().unwrap_or("");
            if let Some(fact) = state.visit(command, &call["params"], result) {
                if i >= turn {
                    count += 1;
                    if count <= MAX_PER_TURN && facts.len() < MAX_PER_RESULT && i >= start {
                        facts.push(fact);
                    }
                }
            }
        }
        if facts.is_empty() {
            continue;
        }
        let existing = prompting::message_text(&messages[i]["content"]);
        facts.retain(|f| !existing.contains(f.as_str()));
        if facts.is_empty() {
            continue;
        }
        let line = format!("{} {}", crate::identity::HINT, facts.join(" "));
        let content = &mut messages[i]["content"];
        let before = content.clone();
        if let Some(text) = content.as_str() {
            *content = json!(format!("{text}\n{line}"));
        } else if let Some(blocks) = content.as_array_mut() {
            blocks.push(json!({"type":"text","text":line}));
        }
        if *content != before {
            audit.record("hole_fact", &before, content)?;
        }
    }
    Ok(())
}
