//! Recipe table, typed slots, and deterministic catalogue-checked expansions.
use super::*;
use std::f64::consts::PI;
mod closed_profile;
mod compound_plate;
pub(crate) mod cross_drilled;
pub(crate) mod revolute;
mod shell_box;
pub(crate) mod shortcuts;

#[derive(Clone, Copy)]
enum Kind {
    Box,
    Cylinder,
    Sphere,
    Holes,
    Hole,
    Sketch,
    ClosedProfile,
    CompoundPlate,
    Fillet,
    Chamfer,
    Move,
    Rename,
    Mirror,
    Scale,
    Tube,
    Shell,
    Revolve,
}
const LIBRARY: &[(&str, Kind)] = &[
    ("compound_plate", Kind::CompoundPlate),
    ("box", Kind::Box),
    ("cylinder", Kind::Cylinder),
    ("sphere", Kind::Sphere),
    ("plate_holes", Kind::Holes),
    ("hole", Kind::Hole),
    ("sketch_extrude", Kind::Sketch),
    ("closed_profile", Kind::ClosedProfile),
    ("fillet", Kind::Fillet),
    ("chamfer", Kind::Chamfer),
    ("move", Kind::Move),
    ("rename", Kind::Rename),
    ("mirror", Kind::Mirror),
    ("scale", Kind::Scale),
    ("tube", Kind::Tube),
    ("shell", Kind::Shell),
    ("revolve", Kind::Revolve),
];
#[derive(Debug, serde::Serialize)]
enum Slots {
    Box {
        size: [f64; 3],
    },
    Cylinder {
        radius: f64,
        height: f64,
    },
    Sphere {
        radius: f64,
    },
    Holes {
        size: [f64; 3],
        diameter: f64,
        centers: Vec<[f64; 2]>,
    },
    Hole {
        body: String,
        diameter: f64,
        position: [f64; 3],
    },
    Sketch {
        plane: String,
        size: Vec<f64>,
        distance: f64,
    },
    LProfile {
        plane: String,
        width: f64,
        height: f64,
        horizontal: f64,
        vertical: f64,
        distance: f64,
    },
    ClosedProfile(closed_profile::Profile),
    CompoundPlate(compound_plate::Plate),
    CrossDrilled(cross_drilled::BoxHoles),
    Edges {
        body: String,
        edges: Vec<Value>,
        amount: f64,
        fillet: bool,
        top: bool,
    },
    Move {
        body: String,
        delta: [f64; 3],
    },
    Rename {
        body: String,
        name: String,
    },
    Mirror {
        body: String,
        plane: String,
        keep_original: bool,
    },
    Scale {
        body: String,
        factor: f64,
    },
    Tube {
        outer_radius: f64,
        inner_radius: f64,
        height: f64,
    },
    Shell {
        size: [f64; 3],
        wall: f64,
    },
    ShellBox(shell_box::ShellBox),
    Revolve {
        radius: f64,
        height: f64,
    },
}
struct Input<'a> {
    text: &'a str,
    t: Vec<String>,
    data: Option<&'a Value>,
    messages: &'a [Value],
}
impl Input<'_> {
    fn has(&self, w: &str) -> bool {
        self.t.iter().any(|s| s == w)
    }
    fn any(&self, words: &[&str]) -> bool {
        words.iter().any(|w| self.has(w))
    }
    fn dims(&self) -> Option<Vec<slots::Dimension>> {
        let t = tokens(&self.without_body()?);
        dimensions(&t)
    }
    fn without_body(&self) -> Option<String> {
        let mut text = self.text.to_ascii_lowercase();
        if let Some(name) = body(self.text, self.data) {
            let name = name.to_ascii_lowercase();
            let matches: Vec<_> = text
                .match_indices(&name)
                .filter(|(i, _)| {
                    text[..*i]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                        && text[*i + name.len()..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                })
                .map(|(i, _)| i)
                .collect();
            if matches.is_empty() {
                return Some(text);
            }
            if matches.len() != 1 {
                return None;
            }
            text.replace_range(matches[0]..matches[0] + name.len(), " ");
        }
        Some(text)
    }
    fn plane(&self) -> Option<String> {
        let start = self
            .t
            .iter()
            .position(|s| matches!(s.as_str(), "across" | "about" | "on"))?;
        let tail = &self.t[start + 1..];
        let end = tail
            .iter()
            .position(|s| {
                matches!(s.as_str(), "keep" | "keeping" | "discard")
                    || (self.has("revolve") && s == "about")
            })
            .unwrap_or(tail.len());
        if end < tail.len() && !self.has("revolve") {
            let retention: Vec<_> = tail[end..].iter().map(String::as_str).collect();
            if !matches!(
                retention.as_slice(),
                ["keep" | "keeping" | "discard", "original"]
                    | ["keep" | "keeping" | "discard", "the", "original"]
                    | ["keep" | "keeping" | "discard", "original", "and"]
                    | ["keep" | "keeping" | "discard", "the", "original", "and"]
            ) {
                return None;
            }
        }
        principal_plane(&tail[..end])
    }
}
// The catalogue documents these principal planes, but no "top" alias.
fn principal_plane(t: &[String]) -> Option<String> {
    let t = t.strip_suffix(&["and".into()]).unwrap_or(t);
    let t = t.strip_prefix(&["the".into()]).unwrap_or(t);
    let t = t.strip_prefix(&["plane".into()]).unwrap_or(t);
    let t = t.strip_suffix(&["plane".into()]).unwrap_or(t);
    if t.len() != 1 && !(t.len() == 3 && t[1] == "-") {
        return None;
    }
    let joined = t.join("");
    match joined.as_str() {
        "xy" | "x-y" => Some("XY".into()),
        "xz" | "x-z" => Some("XZ".into()),
        "yz" | "y-z" => Some("YZ".into()),
        _ => None,
    }
}
fn positive(d: &[slots::Dimension]) -> bool {
    d.iter().all(|d| d.value > 0.0)
}
pub(super) const UNSUPPORTED: &[&str] = &[
    "rotate",
    "rotation",
    "taper",
    "tapered",
    "rounded",
    "thread",
    "threaded",
    "counterbore",
    "countersink",
    "blind",
    "offset",
    "angled",
    "angle",
    "hollowed",
    "combine",
    "join",
    "union",
    "subtract",
    "intersect",
    "delete",
    "duplicate",
    "copy",
    "copies",
    "twice",
    "oval",
    "ellipse",
    "cone",
    "torus",
    "flat",
    "vertical",
    "curved",
    "triangle",
    "triangular",
    "polygon",
    "sweep",
    "loft",
    "text",
    "engrave",
    "color",
    "colour",
    "material",
    "unless",
    "maybe",
    "possibly",
    "either",
    "but",
];
fn vocabulary(kind: Kind, input: &Input<'_>) -> bool {
    if matches!(kind, Kind::CompoundPlate) {
        return true;
    }
    if matches!(kind, Kind::ClosedProfile) {
        return closed_profile::vocabulary(input);
    }
    if matches!(kind, Kind::Shell) {
        return shell_box::vocabulary(input);
    }
    // Unknown conversational words are fine. Known unsupported geometry is not.
    if input.any(UNSUPPORTED) {
        return false;
    }
    if input.has("discard") && !matches!(kind, Kind::Mirror) {
        return false;
    }
    if input.has("hollow") && !matches!(kind, Kind::Tube) {
        return false;
    }
    if matches!(kind, Kind::Box | Kind::Cylinder | Kind::Sphere)
        && input.any(&["at", "centered", "centred", "origin"])
    {
        return false;
    }
    matches!(kind, Kind::Rename)
        || input.t.iter().enumerate().all(|(i, s)| {
            s != "-"
                || (matches!(kind, Kind::Sketch | Kind::Mirror | Kind::Revolve)
                    && i > 0
                    && input.t.get(i + 1).is_some_and(|next| {
                        matches!(
                            (input.t[i - 1].as_str(), next.as_str()),
                            ("x", "y") | ("x", "z") | ("y", "z")
                        )
                    }))
        })
}

fn box_size(t: &[String], d: &[slots::Dimension], cube: bool) -> Option<[f64; 3]> {
    if cube && d.len() == 1 && positive(d) {
        return Some([d[0].value; 3]);
    }
    if d.len() != 3 || !positive(d) {
        return None;
    }
    let thickness: Vec<_> = d
        .iter()
        .filter(|v| labelled(t, **v, &["thick", "thickness"]))
        .collect();
    if thickness.len() == 1 {
        let pair: Vec<_> = d.iter().filter(|v| v.index != thickness[0].index).collect();
        let between = &t[pair[0].index + 1..pair[1].index];
        if between
            .iter()
            .all(|s| slots::scale(s).is_some() || matches!(s.as_str(), "x" | "by"))
            && between.iter().any(|s| matches!(s.as_str(), "x" | "by"))
        {
            return Some([pair[0].value, pair[1].value, thickness[0].value]);
        }
    }
    let labels = [["length", "long"], ["width", "wide"], ["height", "high"]];
    if labelled(
        t,
        d[2],
        &[
            "height",
            "thick",
            "tall",
            "high",
            "deep",
            "depth",
            "thickness",
        ],
    ) && !d[..2]
        .iter()
        .any(|v| labels.iter().any(|ls| labelled(t, *v, ls)))
    {
        let between = &t[d[0].index + 1..d[1].index];
        if between
            .iter()
            .all(|s| slots::scale(s).is_some() || matches!(s.as_str(), "x" | "by"))
            && between.iter().any(|s| matches!(s.as_str(), "x" | "by"))
        {
            return Some([d[0].value, d[1].value, d[2].value]);
        }
    }
    if d.iter()
        .any(|v| labels.iter().any(|ls| labelled(t, *v, ls)))
    {
        let mut size = [0.0; 3];
        for (i, ls) in labels.iter().enumerate() {
            let mut aliases = ls.to_vec();
            if i == 2 {
                aliases.extend(["thick", "tall", "deep", "depth", "thickness"]);
            }
            let found: Vec<_> = d.iter().filter(|v| labelled(t, **v, &aliases)).collect();
            if found.len() != 1 {
                return None;
            }
            size[i] = found[0].value;
        }
        return Some(size);
    }
    // A tuple assigns length, width, height in order. No unlabeled loose numbers.
    for w in d.windows(2) {
        let between = &t[w[0].index + 1..w[1].index];
        if !between
            .iter()
            .all(|s| slots::scale(s).is_some() || matches!(s.as_str(), "x" | "by"))
            || !between.iter().any(|s| matches!(s.as_str(), "x" | "by"))
        {
            return None;
        }
    }
    Some([d[0].value, d[1].value, d[2].value])
}
fn radial(t: &[String], d: slots::Dimension) -> Option<f64> {
    if t.iter().any(|s| s == "radius") && t.iter().any(|s| s == "diameter" || s == "across") {
        return None;
    }
    let r = labelled(t, d, &["radius"]);
    let dia = labelled(t, d, &["diameter", "across", "wide"]);
    match (r, dia) {
        (true, false) => Some(d.value),
        (false, true) => Some(d.value / 2.0),
        _ => None,
    }
}
fn rename_separator(text: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in text.char_indices() {
        if matches!(c, '\'' | '"' | '`') {
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(c);
            }
        }
        if quote.is_none() && text[i..].starts_with(" to ") {
            return Some(i);
        }
    }
    None
}
fn parse(kind: Kind, input: &Input<'_>) -> Option<Slots> {
    let has = |w| input.has(w);
    let t = &input.t;
    match kind {
        Kind::CompoundPlate => compound_plate::parse(input),
        Kind::ClosedProfile => closed_profile::parse(input),
        Kind::Hole if input.any(&["drill", "bore", "cut"]) && has("hole") && has("through") => {
            // No inferred hole location or direction: the public command uses Z,
            // and the request must supply all three position coordinates.
            let lower = input.without_body()?;
            let (sizing, position) = lower.split_once(" at ")?;
            let d = dimensions(&tokens(sizing))?;
            if d.len() != 1 || !positive(&d) {
                return None;
            }
            let diameter = radial(&tokens(sizing), d[0])? * 2.0;
            let position = position.trim().trim_end_matches(['.', '!', '?']).trim();
            let position = position.strip_prefix('(')?.strip_suffix(')')?;
            let coordinates: Vec<_> = position.split(',').collect();
            if coordinates.len() != 3 {
                return None;
            }
            let mut p = [0.0; 3];
            for (i, coordinate) in coordinates.iter().enumerate() {
                let t = tokens(coordinate);
                let d = dimensions(&t)?;
                if d.len() != 1 || d[0].index != 0 || t.len() > 2 {
                    return None;
                }
                p[i] = d[0].value;
            }
            let raw = input.text.to_ascii_lowercase();
            let (raw_sizing, _) = raw.split_once(" at ")?;
            let (_, target) = raw_sizing.split_once(" through ")?;
            let target = format!("move {}", target.trim());
            Some(Slots::Hole {
                body: body(&target, input.data)?,
                diameter,
                position: p,
            })
        }
        Kind::Rename if input.any(&["rename", "call", "name"]) => {
            let text = input
                .text
                .trim()
                .trim_end_matches(['.', ',', ';', ':', '!', '?'])
                .trim();
            let lower = text.to_ascii_lowercase();
            let verb = ["rename ", "call ", "name "]
                .iter()
                .filter_map(|v| lower.find(v))
                .min()?;
            let text = &text[verb..];
            let lower = text.to_ascii_lowercase();
            let (source, raw) = if let Some(i) = rename_separator(&lower) {
                (&text[..i], text.get(i + 4..)?)
            } else {
                let verb = ["call ", "name "]
                    .iter()
                    .find_map(|v| lower.find(v).map(|i| i + v.len()))?;
                let rest = &lower[verb..];
                let mut sources = vec![
                    "the current body",
                    "this body",
                    "this box",
                    "the body",
                    "the box",
                    "the part",
                    "it",
                ];
                let names: Vec<_> = input.data?["bodies"]
                    .as_array()?
                    .iter()
                    .filter_map(|b| b["name"].as_str())
                    .map(str::to_ascii_lowercase)
                    .collect();
                sources.extend(names.iter().map(String::as_str));
                let source = sources
                    .iter()
                    .filter(|s| {
                        rest.starts_with(**s)
                            && rest.get(s.len()..).is_some_and(|s| s.starts_with(' '))
                    })
                    .max_by_key(|s| s.len())?;
                let end = verb + source.len();
                (&text[..end], text.get(end..)?)
            };
            let mut raw = raw.trim();
            while let Some((prefix, suffix)) = raw.rsplit_once(char::is_whitespace) {
                if !matches!(
                    suffix.to_ascii_lowercase().as_str(),
                    "thanks" | "please" | "pls"
                ) || prefix.trim().is_empty()
                {
                    break;
                }
                raw = prefix.trim_end_matches([',', ';', ':']).trim();
            }
            let new =
                if let Some(quote) = raw.chars().next().filter(|c| matches!(c, '`' | '\'' | '"')) {
                    let inner = raw.strip_prefix(quote)?.strip_suffix(quote)?;
                    if inner.contains(quote) {
                        return None;
                    }
                    inner
                } else {
                    if raw.contains(['`', '\'', '"', '.', '!', '?', ';', ':', ',']) {
                        return None;
                    }
                    raw
                };
            if new.split_whitespace().count() > 1
                && !(['`', '\'', '"']
                    .iter()
                    .any(|c| raw.starts_with(*c) && raw.ends_with(*c)))
            {
                return None;
            }
            if new.is_empty() || new.len() > 128 || new.contains(['\n', '\r']) {
                return None;
            }
            Some(Slots::Rename {
                body: body(source, input.data).or_else(|| {
                    (tokens(source) == ["rename"])
                        .then(|| body("rename it", input.data))
                        .flatten()
                })?,
                name: new.into(),
            })
        }
        Kind::Move if input.any(&["move", "translate", "shift", "slide"]) => {
            let d = input.dims()?;
            if d.len() != 1 || d[0].value == 0.0 {
                return None;
            }
            let axes: Vec<_> = ["x", "y", "z"]
                .iter()
                .enumerate()
                .filter(|(_, a)| has(a))
                .collect();
            if axes.len() != 1 {
                return None;
            }
            let mut delta = [0.0; 3];
            if has("negative") && (has("positive") || d[0].value < 0.0)
                || has("positive") && d[0].value < 0.0
            {
                return None;
            }
            delta[axes[0].0] = d[0].value * if has("negative") { -1.0 } else { 1.0 };
            Some(Slots::Move {
                body: body(input.text, input.data)?,
                delta,
            })
        }
        Kind::Scale if input.any(&["scale", "double", "half"]) => {
            if !has("origin") || input.any(&["x", "y", "z", "factors", "nonuniform"]) {
                return None;
            }
            let lower = input.without_body()?;
            let numbers: Vec<_> = tokens(&lower)
                .iter()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            let factor = if input.any(&["double", "half"]) {
                if !numbers.is_empty() || (has("double") && has("half")) {
                    return None;
                }
                if has("double") {
                    2.0
                } else {
                    0.5
                }
            } else {
                if numbers.len() != 1 {
                    return None;
                }
                numbers[0]
                    / if lower.contains('%') || has("percent") {
                        100.0
                    } else {
                        1.0
                    }
            };
            if !factor.is_finite() || factor <= 0.0 || factor > 1000.0 {
                return None;
            }
            if tokens(&lower)
                .iter()
                .any(|s| slots::scale(s).is_some() && s != "in")
            {
                return None;
            }
            Some(Slots::Scale {
                body: body(input.text, input.data)?,
                factor,
            })
        }
        Kind::Tube if has("tube") => {
            let d = dimensions(t)?;
            if d.len() != 3 || !positive(&d) {
                return None;
            }
            let one = |labels: &[&str]| {
                let found: Vec<_> = d.iter().filter(|v| labelled(t, **v, labels)).collect();
                if found.len() == 1 {
                    Some(found[0].value)
                } else {
                    None
                }
            };
            let outer = one(&["od"])?;
            let inner = match (one(&["id"]), one(&["wall", "thickness"])) {
                (Some(v), None) => v,
                (None, Some(v)) => outer - 2.0 * v,
                _ => return None,
            };
            let height = one(&["height", "length", "long", "tall"])?;
            if inner <= 0.0 || inner >= outer {
                return None;
            }
            Some(Slots::Tube {
                outer_radius: outer / 2.0,
                inner_radius: inner / 2.0,
                height,
            })
        }
        Kind::Mirror if has("mirror") => {
            if has("discard") && input.any(&["keep", "keeping"]) {
                return None;
            }
            if !input.dims()?.is_empty() {
                return None;
            }
            Some(Slots::Mirror {
                body: body(input.text, input.data)?,
                plane: input.plane()?,
                keep_original: !has("discard"),
            })
        }
        Kind::Fillet | Kind::Chamfer
            if (matches!(kind, Kind::Fillet) && input.any(&["fillet", "round"]))
                || (matches!(kind, Kind::Chamfer) && input.any(&["chamfer", "bevel"])) =>
        {
            let fillet = matches!(kind, Kind::Fillet);
            if !has("edges") || !(has("all") || has("top")) || (fillet && has("top")) {
                return None;
            }
            let d = input.dims()?;
            if d.len() != 1 || !positive(&d) {
                return None;
            }
            let name = body(input.text, input.data)?;
            let edges = topology(input, &name, has("top")).unwrap_or_default();
            Some(Slots::Edges {
                body: name,
                edges,
                amount: d[0].value,
                fillet,
                top: has("top"),
            })
        }
        Kind::Box
            if input.any(&["box", "block", "plate", "cube"])
                && !input.any(&["hole", "holes", "shell", "sketch", "revolve"]) =>
        {
            let d = dimensions(t)?;
            Some(Slots::Box {
                size: box_size(t, &d, has("cube"))?,
            })
        }
        Kind::Cylinder if input.any(&["cylinder", "rod", "disc"]) => {
            let d = dimensions(t)?;
            if d.len() != 2 || !positive(&d) {
                return None;
            }
            let heights: Vec<_> = d
                .iter()
                .filter(|d| {
                    labelled(
                        t,
                        **d,
                        &[
                            "height",
                            "tall",
                            "high",
                            "thick",
                            "thickness",
                            "deep",
                            "depth",
                            "length",
                            "long",
                        ],
                    )
                })
                .collect();
            if heights.len() != 1 {
                return None;
            }
            let h = heights[0];
            let r = *d.iter().find(|d| d.index != h.index)?;
            Some(Slots::Cylinder {
                radius: radial(t, r)?,
                height: h.value,
            })
        }
        Kind::Sphere if has("sphere") => {
            let d = dimensions(t)?;
            if d.len() != 1 || !positive(&d) {
                return None;
            }
            Some(Slots::Sphere {
                radius: radial(t, d[0])?,
            })
        }
        Kind::Holes
            if input.any(&["plate", "box", "block", "cube"]) && input.any(&["hole", "holes"]) =>
        {
            holes(input)
        }
        Kind::Shell => shell_box::parse(input),
        Kind::Sketch
            if has("extrude")
                && input.any(&["rectangle", "circle"])
                && !closed_profile::candidate(input) =>
        {
            if has("rectangle") && has("circle") {
                return None;
            }
            let at = t.iter().position(|s| s == "extrude")?;
            let mut before = &t[..at];
            while before
                .last()
                .is_some_and(|s| matches!(s.as_str(), "and" | "then"))
            {
                before = &before[..before.len() - 1];
            }
            let on = before.iter().position(|s| s == "on")?;
            let (profile, plane) = if on == 0 {
                let end = before
                    .iter()
                    .position(|s| matches!(s.as_str(), "and" | "sketch" | "draw"))?;
                (&before[end..], principal_plane(&before[1..end])?)
            } else {
                (&before[..on], principal_plane(&before[on + 1..])?)
            };
            let d = dimensions(profile)?;
            let extent = dimensions(&t[at + 1..])?;
            if extent.len() != 1 || !positive(&extent) || !positive(&d) {
                return None;
            }
            let size = if has("rectangle") && d.len() == 2 {
                let between = &profile[d[0].index + 1..d[1].index];
                if !between
                    .iter()
                    .all(|s| slots::scale(s).is_some() || matches!(s.as_str(), "x" | "by"))
                    || !between.iter().any(|s| matches!(s.as_str(), "x" | "by"))
                {
                    return None;
                }
                vec![d[0].value, d[1].value]
            } else if has("circle") && d.len() == 1 {
                // An unqualified circle size is its diameter; explicit radius wins.
                vec![radial(profile, d[0]).or_else(|| {
                    (!profile
                        .iter()
                        .any(|s| matches!(s.as_str(), "radius" | "diameter" | "across")))
                    .then_some(d[0].value / 2.0)
                })?]
            } else {
                return None;
            };
            Some(Slots::Sketch {
                plane,
                size,
                distance: extent[0].value,
            })
        }
        Kind::Revolve if has("revolve") && has("rectangle") => {
            // A radial rectangle from the axis, in XZ, makes a full Z revolution.
            if input.plane()?.as_str() != "XZ"
                || !has("z")
                || has("x")
                || has("y")
                || !has("axis")
                || !has("from")
                || !has("full")
            {
                return None;
            }
            let d = dimensions(t)?;
            if d.len() != 2
                || !positive(&d)
                || !labelled(t, d[0], &["radius"])
                || !labelled(t, d[1], &["height", "tall", "high"])
            {
                return None;
            }
            Some(Slots::Revolve {
                radius: d[0].value,
                height: d[1].value,
            })
        }
        _ => None,
    }
}
fn topology(input: &Input<'_>, name: &str, top: bool) -> Option<Vec<Value>> {
    // Use only a current complete, explicitly identified topology result.
    let payload = tool_payload(input.messages.iter().rev().find(|m| m["role"] == "tool")?)?;
    let data = if payload["body"] == name {
        &payload
    } else {
        payload["bodies"]
            .as_array()?
            .iter()
            .find(|b| b["name"] == name)?
    };
    let edges = data["edges"].as_array()?;
    if edges.is_empty() || edges.len() > 256 {
        return None;
    }
    let mut selected = Vec::new();
    let mut highest = f64::NEG_INFINITY;
    if top {
        for edge in edges {
            let mut chord2 = 0.0;
            for i in 0..3 {
                let a = edge["start"][i].as_f64()?;
                let b = edge["end"][i].as_f64()?;
                if !a.is_finite() || !b.is_finite() {
                    return None;
                }
                chord2 += (a - b).powi(2);
            }
            let length = edge["length"].as_f64()?;
            if !length.is_finite() || (length - chord2.sqrt()).abs() > 1e-7 {
                return None;
            }
            for p in [edge.get("start"), edge.get("end")].into_iter().flatten() {
                highest = highest.max(p.get(2)?.as_f64()?);
            }
        }
        if !highest.is_finite() {
            return None;
        }
    }
    for edge in edges {
        let index = edge["index"].as_u64()?;
        if index > 1_000_000 || selected.iter().any(|v: &Value| v["index"] == index) {
            return None;
        }
        if top {
            let a = edge["start"][2].as_f64()?;
            let b = edge["end"][2].as_f64()?;
            if (a - highest).abs() > 1e-7 || (b - highest).abs() > 1e-7 {
                continue;
            }
            // Curved edges need full bounds to establish that the whole edge is on top.
            let mid = edge["mid"][2].as_f64()?;
            if (mid - highest).abs() > 1e-7 {
                continue;
            }
        }
        selected.push(json!({"body":name,"index":index}));
    }
    (!selected.is_empty()).then_some(selected)
}
fn count_before(t: &mut Vec<String>, label: &str) -> Option<usize> {
    let i = t.iter().position(|s| s == label)?;
    let mut before = i.checked_sub(1)?;
    if t[before] == "through" {
        before = before.checked_sub(1)?;
    }
    let n = t.get(before)?.parse::<usize>().ok()?;
    if !(1..=64).contains(&n) {
        return None;
    }
    t.remove(before);
    Some(n)
}
fn holes(input: &Input<'_>) -> Option<Slots> {
    if !input.has("through") {
        return None;
    }
    let split = input.t.iter().position(|s| s == "with")?;
    let base = &input.t[..split];
    let size = box_size(base, &dimensions(base)?, base.iter().any(|s| s == "cube"))?;
    let mut rest = input.t[split + 1..].to_vec();
    if rest
        .iter()
        .any(|s| matches!(s.as_str(), "deep" | "depth" | "blind" | "countersunk"))
    {
        return None;
    }
    let relative = rest
        .iter()
        .any(|s| matches!(s.as_str(), "end" | "ends" | "corner" | "corners"));
    let relative_slots = if relative {
        Some(relative_holes(&rest, size)?)
    } else {
        None
    };
    let n = if rest.iter().position(|s| s == "holes").is_some_and(|i| {
        i > 0
            && (rest[i - 1].parse::<usize>().is_ok()
                || (i > 1 && rest[i - 1] == "through" && rest[i - 2].parse::<usize>().is_ok()))
    }) {
        Some(count_before(&mut rest, "holes")?)
    } else {
        None
    };
    let boundary = rest
        .iter()
        .position(|s| matches!(s.as_str(), "at" | "rectangular" | "circular"));
    let hole_tokens = &rest[..boundary.unwrap_or(rest.len())];
    let d = if relative {
        Vec::new()
    } else {
        dimensions(hole_tokens)?
    };
    if !relative && (d.len() != 1 || !positive(&d)) {
        return None;
    }
    // In a single through-hole noun phrase, an unqualified size is its diameter.
    let diameter = if let Some((diameter, _)) = &relative_slots {
        *diameter
    } else if hole_tokens
        .iter()
        .any(|s| matches!(s.as_str(), "radius" | "diameter" | "across"))
    {
        radial(hole_tokens, d[0])? * 2.0
    } else {
        d[0].value
    };
    let mut centers = Vec::new();
    if let Some((_, positions)) = relative_slots {
        centers = positions;
    } else if boundary.is_some_and(|i| rest[i] == "at") {
        let lower = input.text.to_ascii_lowercase();
        let (_, positions) = lower.split_once(" at ")?;
        let mut tail = positions;
        while let Some((prefix, after)) = tail.split_once('(') {
            if !matches!(prefix.trim(), "" | "," | "and" | ", and") {
                return None;
            }
            let (pair, next) = after.split_once(')')?;
            let parts: Vec<_> = pair.split(',').collect();
            if parts.len() != 2 {
                return None;
            }
            let mut p = [0.0; 2];
            for (i, s) in parts.iter().enumerate() {
                let coordinate = tokens(s);
                let d = dimensions(&coordinate)?;
                if d.len() != 1
                    || d[0].index != 0
                    || coordinate.len() > 2
                    || coordinate.get(1).is_some_and(|s| slots::scale(s).is_none())
                {
                    return None;
                }
                p[i] = d[0].value;
            }
            centers.push(p);
            tail = next;
            if centers.len() > 64 {
                return None;
            }
        }
        if centers.len() != n?
            || !tail
                .trim_matches(|c: char| {
                    c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?')
                })
                .is_empty()
        {
            return None;
        }
    } else if input.has("rectangular") {
        if !input.any(&["centered", "centred"]) || !input.has("pitch") {
            return None;
        }
        let mut pattern = rest[boundary?..].to_vec();
        let rows = count_before(&mut pattern, "rows")?;
        let cols = count_before(&mut pattern, "columns")?;
        if rows * cols > 64 || n.is_some_and(|n| n != rows * cols) {
            return None;
        }
        let pitch = dimensions(&pattern)?;
        if pitch.len() != 2 || !positive(&pitch) {
            return None;
        }
        for row in 0..rows {
            for col in 0..cols {
                centers.push([
                    size[0] / 2.0 + (col as f64 - (cols - 1) as f64 / 2.0) * pitch[0].value,
                    size[1] / 2.0 + (row as f64 - (rows - 1) as f64 / 2.0) * pitch[1].value,
                ]);
            }
        }
    } else if input.has("circular") {
        if !input.any(&["centered", "centred"])
            || !input.has("equally")
            || !input.has("spaced")
            || !input.text.to_ascii_lowercase().contains("starting at +x")
        {
            return None;
        }
        // Explicit +X start avoids silently rotating a bolt circle.
        let n = n?;
        let pattern = &rest[boundary?..];
        let d = dimensions(pattern)?;
        if d.len() != 1 || !positive(&d) {
            return None;
        }
        let radius = radial(pattern, d[0])?;
        for i in 0..n {
            let a = 2.0 * PI * i as f64 / n as f64;
            centers.push([
                size[0] / 2.0 + radius * a.cos(),
                size[1] / 2.0 + radius * a.sin(),
            ]);
        }
    } else if input.any(&["centred", "centered", "middle", "centre", "center"])
        && (input.has("hole") || n == Some(1))
        && n.is_none_or(|n| n == 1)
    {
        centers.push([size[0] / 2.0, size[1] / 2.0]);
    } else {
        return None;
    }
    if centers.is_empty() || centers.len() > 62 {
        return None;
    }
    // Fully enclosed, non-overlapping through holes; expected volume is then exact.
    for (i, p) in centers.iter().enumerate() {
        if p[0] <= diameter / 2.0
            || p[0] >= size[0] - diameter / 2.0
            || p[1] <= diameter / 2.0
            || p[1] >= size[1] - diameter / 2.0
        {
            return None;
        }
        for q in &centers[..i] {
            if (p[0] - q[0]).hypot(p[1] - q[1]) <= diameter {
                return None;
            }
        }
    }
    Some(Slots::Holes {
        size,
        diameter,
        centers,
    })
}
/// Relative layouts use the same origin-corner convention as explicit coordinates.
fn relative_holes(rest: &[String], size: [f64; 3]) -> Option<(f64, Vec<[f64; 2]>)> {
    let has = |w: &str| rest.iter().any(|s| s == w);
    if !has("each") || has("long") || has("rectangular") || has("circular") {
        return None;
    }
    let mut t = rest.to_vec();
    let count = match t.first().map(String::as_str) {
        Some(s)
            if [
                "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
                "eleven", "twelve",
            ]
            .contains(&s) =>
        {
            let n = [
                "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
                "eleven", "twelve",
            ]
            .iter()
            .position(|w| *w == s)?
                + 1;
            t.remove(0);
            Some(n)
        }
        Some(s) if s.parse::<usize>().is_ok() => Some(t.remove(0).parse::<usize>().ok()?),
        _ => None,
    };
    let d = dimensions(&t)?;
    if d.len() != 2 || !positive(&d) {
        return None;
    }
    let from = t.iter().position(|s| s == "from")?;
    if d[1].index >= from || d[0].index >= d[1].index {
        return None;
    }
    let sizing = &t[..d[1].index];
    let diameter = if sizing
        .iter()
        .any(|s| matches!(s.as_str(), "radius" | "diameter" | "across"))
    {
        radial(sizing, d[0])? * 2.0
    } else {
        d[0].value
    };
    let inset = d[1].value;
    let positions = if has("corner") || has("corners") {
        if count != Some(4)
            || !has("both")
            || !has("adjacent")
            || !has("edges")
            || !has("in")
            || inset * 2.0 >= size[0].min(size[1])
        {
            return None;
        }
        vec![
            [inset, inset],
            [size[0] - inset, inset],
            [inset, size[1] - inset],
            [size[0] - inset, size[1] - inset],
        ]
    } else {
        let centre_line = (has("centre") || has("center")) && has("line");
        let halfway = has("halfway") && has("across") && has("width");
        if count.is_some_and(|n| n != 2)
            || !(centre_line || (has("short") && halfway))
            || size[0] == size[1]
            || inset * 2.0 >= size[0].max(size[1])
        {
            return None;
        }
        if size[0] > size[1] {
            vec![[inset, size[1] / 2.0], [size[0] - inset, size[1] / 2.0]]
        } else {
            vec![[size[0] / 2.0, inset], [size[0] / 2.0, size[1] - inset]]
        }
    };
    Some((diameter, positions))
}
type Expansion = (Vec<Value>, Option<f64>, bool, Option<String>);
fn expand(slots: &Slots, catalog: &Catalog) -> Result<Expansion, String> {
    let mut calls = Vec::new();
    let mut add = |id: &str, p: Value| -> Result<(), String> {
        calls.push(call(catalog, id, p)?);
        Ok(())
    };
    let mut v = None;
    let mut creates = false;
    let mut name = None;
    let box_params = |s: &[f64; 3]| json!({"length":s[0],"width":s[1],"height":s[2]});
    match slots {
        Slots::CrossDrilled(b) => {
            for (id, params) in b.commands() {
                add(id, params)?;
            }
            v = b.volume();
            creates = true;
        }
        Slots::CompoundPlate(plate) => {
            for (id, params) in plate.commands() {
                add(id, params)?;
            }
            v = plate.volume();
            creates = true;
        }
        Slots::Box { size } => {
            add("solid.box", box_params(size))?;
            v = Some(size.iter().product());
            creates = true;
        }
        Slots::Cylinder { radius, height } => {
            add("solid.cylinder", json!({"radius":radius,"height":height}))?;
            v = Some(PI * radius * radius * height);
            creates = true;
        }
        Slots::Sphere { radius } => {
            add("solid.sphere", json!({"radius":radius}))?;
            v = Some(4.0 / 3.0 * PI * radius.powi(3));
            creates = true;
        }
        Slots::Holes {
            size,
            diameter,
            centers,
        } => {
            let mut p = box_params(size);
            p["operation"] = json!("new");
            add("solid.box", p)?;
            for p in centers {
                add(
                    "solid.hole",
                    json!({"position":[p[0],p[1],size[2]],"diameter":diameter}),
                )?;
            }
            v = Some(
                size.iter().product::<f64>()
                    - centers.len() as f64 * PI * (diameter / 2.0).powi(2) * size[2],
            );
            creates = true;
        }
        Slots::Hole {
            body,
            diameter,
            position,
        } => {
            add(
                "solid.hole",
                json!({"targets":[body],"diameter":diameter,"position":position}),
            )?;
        }
        Slots::ClosedProfile(profile) => {
            for (id, params) in profile.commands() {
                add(id, params)?;
            }
            v = Some(profile.area() * profile.distance);
            creates = true;
        }
        Slots::LProfile {
            plane,
            width,
            height,
            horizontal,
            vertical,
            distance,
        } => {
            add("sketch.create", json!({"plane":plane,"name":"L profile"}))?;
            add(
                "sketch.line",
                json!({"points":[[0,0],[width,0],[width,horizontal],[vertical,horizontal],[vertical,height],[0,height]],"closed":true,"infer":true,"ids":["l1","l2","l3","l4","l5","l6"]}),
            )?;
            for (id, value) in [
                ("l1", width),
                ("l6", height),
                ("l2", horizontal),
                ("l5", vertical),
            ] {
                add("sketch.dimension", json!({"entities":[id],"value":value}))?;
            }
            add("sketch.finish", json!({}))?;
            add("solid.extrude", json!({"distance":distance}))?;
            v = Some((width * horizontal + (height - horizontal) * vertical) * distance);
            creates = true;
        }
        Slots::Sketch {
            plane,
            size,
            distance,
        } => {
            add("sketch.create", json!({"plane":plane}))?;
            let area = if size.len() == 2 {
                add("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":size}))?;
                size[0] * size[1]
            } else {
                add(
                    "sketch.circle.center",
                    json!({"center":[0,0],"radius":size[0]}),
                )?;
                PI * size[0].powi(2)
            };
            add("sketch.finish", json!({}))?;
            add(
                "solid.extrude",
                json!({"distance":distance,"operation":"new"}),
            )?;
            v = Some(area * distance);
            creates = true;
        }
        Slots::Edges {
            body,
            edges,
            amount,
            fillet,
            ..
        } => {
            if edges.is_empty() {
                add("model.edges", json!({"body":body}))?;
            } else {
                let (id, key) = if *fillet {
                    ("solid.fillet", "radius")
                } else {
                    ("solid.chamfer", "distance")
                };
                let mut p = json!({"body":body,"edges":edges});
                p[key] = json!(amount);
                add(id, p)?;
            }
        }
        Slots::Move { body, delta } => {
            add("solid.move", json!({"bodies":[body],"translate":delta}))?
        }
        Slots::Rename { body, name: new } => {
            add("body.rename", json!({"body":body,"name":new}))?;
            name = Some(new.clone());
        }
        Slots::Mirror {
            body,
            plane,
            keep_original,
        } => {
            add("solid.mirror", json!({"bodies":[body],"plane":plane}))?;
            if !keep_original {
                add("solid.remove", json!({"bodies":[body]}))?;
            }
            creates = *keep_original;
        }
        Slots::Scale { body, factor } => {
            add(
                "solid.scale",
                json!({"bodies":[body],"factor":factor,"origin":[0,0,0]}),
            )?;
        }
        Slots::Tube {
            outer_radius,
            inner_radius,
            height,
        } => {
            add(
                "solid.cylinder",
                json!({"radius":outer_radius,"height":height,"operation":"new"}),
            )?;
            add(
                "solid.hole",
                json!({"diameter":inner_radius * 2.0,"position":[0,0,height]}),
            )?;
            v = Some(PI * (outer_radius.powi(2) - inner_radius.powi(2)) * height);
            creates = true;
        }
        Slots::ShellBox(shell) => {
            for (command, params) in shell.commands() {
                add(command, params)?;
            }
            v = Some(shell.volume());
            creates = true;
        }
        Slots::Shell { size, wall } => {
            let mut p = box_params(size);
            p["operation"] = json!("new");
            add("solid.box", p)?;
            add(
                "solid.shell",
                json!({"faces":[[size[0]/2.0,size[1]/2.0,size[2]]],"thickness":wall,"direction":"inside"}),
            )?;
            v = Some(
                size.iter().product::<f64>()
                    - (size[0] - 2.0 * wall) * (size[1] - 2.0 * wall) * (size[2] - wall),
            );
            creates = true;
        }
        Slots::Revolve { radius, height } => {
            add("sketch.create", json!({"plane":"XZ"}))?;
            add(
                "sketch.rectangle.two_point",
                json!({"p0":[0,0],"p1":[radius,height]}),
            )?;
            add("sketch.finish", json!({}))?;
            add(
                "solid.revolve",
                json!({"axis":"Z","angle":360,"operation":"new"}),
            )?;
            v = Some(PI * radius * radius * height);
            creates = true;
        }
    }
    // Primitives always create a separate body, including in non-empty documents.
    if matches!(
        slots,
        Slots::Box { .. } | Slots::Cylinder { .. } | Slots::Sphere { .. }
    ) {
        calls[0]["arguments"]["params"]["operation"] = json!("new");
        let args = &calls[0]["arguments"];
        calls[0] = call(
            catalog,
            args["command"]
                .as_str()
                .ok_or("recipe command must be a string")?,
            args["params"].clone(),
        )?;
    }
    Ok((calls, v, creates, name))
}
const ACTION_FAMILIES: &[&[&str]] = &[
    &["move", "translate", "shift", "slide"],
    &["rename", "call", "name"],
    &["mirror"],
    &["scale", "double", "half"],
    &["fillet", "round"],
    &["chamfer", "bevel"],
    &["shell"],
    &["revolve"],
    &["extrude", "pad"],
    &["drill", "bore", "cut"],
];
fn intent_decline(input: &Input<'_>) -> Option<&'static str> {
    if shell_box::parse(input).is_some() {
        return None;
    }
    // Reject multiple action families before parsing; sketch + extrude is one recipe.
    if ACTION_FAMILIES
        .iter()
        .filter(|a| input.any(a) && !(a.contains(&"round") && input.has("disc")))
        .count()
        > 1
    {
        return Some("multiple intents");
    }
    if ACTION_FAMILIES
        .iter()
        .any(|a| input.t.iter().filter(|w| a.contains(&w.as_str())).count() > 1)
        || input
            .t
            .iter()
            .filter(|w| matches!(w.as_str(), "make" | "draw" | "create" | "build" | "add"))
            .count()
            > 1
    {
        return Some("multiple intents");
    }
    if let Some(edit) = input
        .t
        .iter()
        .position(|w| ACTION_FAMILIES.iter().any(|a| a.contains(&w.as_str())))
    {
        // An explicit new primitive followed by an edit is two intents, even
        // when the creation lacks dimensions and the edit alone would parse.
        if !(closed_profile::candidate(input)
            && matches!(input.t[edit].as_str(), "extrude" | "pad"))
            && input.t[..edit].iter().enumerate().any(|(i, w)| {
                matches!(
                    w.as_str(),
                    "box"
                        | "block"
                        | "cube"
                        | "sphere"
                        | "plate"
                        | "cylinder"
                        | "rod"
                        | "disc"
                        | "tube"
                ) && (input.t[..i]
                    .iter()
                    .any(|v| matches!(v.as_str(), "make" | "draw" | "create" | "build" | "add"))
                    || i.checked_sub(1)
                        .is_some_and(|i| input.t[i] == "a" || input.t[i] == "an"))
            })
        {
            return Some("multiple intents");
        }
    }
    let shape_families = [
        &["box", "block", "plate", "cube"][..],
        &["cylinder", "rod", "disc"][..],
        &["sphere"][..],
        &["tube"][..],
    ];
    if shape_families.iter().filter(|f| input.any(f)).count() > 1 {
        return Some("multiple intents");
    }
    None
}

#[cfg(feature = "proxy")]
pub(super) fn report_decline(text: &str) -> (&'static str, Vec<String>) {
    let input = Input {
        text,
        t: intent_tokens(text),
        data: None,
        messages: &[],
    };
    if let Some(reason) = intent_decline(&input) {
        return (
            reason,
            input
                .t
                .into_iter()
                .filter(|w| {
                    matches!(
                        w.as_str(),
                        "and"
                            | "then"
                            | "make"
                            | "draw"
                            | "create"
                            | "move"
                            | "rename"
                            | "mirror"
                            | "scale"
                            | "fillet"
                            | "chamfer"
                            | "extrude"
                            | "cube"
                            | "box"
                            | "sphere"
                            | "cylinder"
                    )
                })
                .collect(),
        );
    }
    let tokens: Vec<String> = input
        .t
        .into_iter()
        .filter(|w| UNSUPPORTED.contains(&w.as_str()))
        .collect();
    if !tokens.is_empty() {
        ("unsupported modifier", tokens)
    } else {
        (
            "missing or ambiguous slots, state, or tool offer",
            Vec::new(),
        )
    }
}

pub(super) fn select(
    text: &str,
    data: Option<&Value>,
    messages: &[Value],
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    let input = Input {
        text,
        t: intent_tokens(text),
        data,
        messages,
    };
    if cross_drilled::candidate(&input) {
        return match cross_drilled::parse(&input) {
            Some(b) => from_slots("cross_drilled_box", Slots::CrossDrilled(b), data, catalog),
            None => Ok(None),
        };
    }
    // Measurement verbs are separate work; dimension participles (`measuring`,
    // `that measures`) and `solid` describe the primitive instead.
    if input.any(&["measure", "count", "report"])
        || (!compound_plate::candidate(&input) && intent_decline(&input).is_some())
    {
        return Ok(None);
    }
    let create = input.any(&[
        "box", "block", "plate", "cube", "cylinder", "rod", "disc", "sphere", "tube",
    ]) || input.any(&[
        "make",
        "draw",
        "create",
        "build",
        "add",
        "sketch",
        "rectangle",
        "circle",
        "shell",
        "revolve",
    ]);
    let mut found = Vec::new();
    for (rule, kind) in LIBRARY {
        if compound_plate::candidate(&input) && !matches!(kind, Kind::CompoundPlate) {
            // Keep the original recipes and their calls untouched. Compound
            // requests either match completely or return to the model.
            continue;
        }
        if shell_box::candidate(&input) && !matches!(kind, Kind::Shell) {
            continue;
        }
        // A dimensioned profile is a distinct creation intent, including when
        // its extrusion is expressed as a depth instead of an action verb.
        if closed_profile::candidate(&input)
            && matches!(
                kind,
                Kind::Box
                    | Kind::Cylinder
                    | Kind::Sphere
                    | Kind::Holes
                    | Kind::Tube
                    | Kind::Shell
                    | Kind::Revolve
            )
        {
            continue;
        }
        let shapes: &[&str] = match kind {
            Kind::Box => &["box", "block", "plate", "cube"],
            Kind::Cylinder => &["cylinder", "rod", "disc"],
            Kind::Sphere => &["sphere"],
            Kind::Tube => &["tube"],
            Kind::Sketch => &["rectangle", "circle"],
            Kind::Shell => &[],
            Kind::Revolve => &["rectangle"],
            _ => &[],
        };
        if !shapes.is_empty()
            && input
                .t
                .iter()
                .filter(|w| shapes.contains(&w.as_str()))
                .count()
                != 1
        {
            continue;
        }
        if matches!(
            kind,
            Kind::Box | Kind::Cylinder | Kind::Sphere | Kind::Tube | Kind::Holes
        ) && ACTION_FAMILIES
            .iter()
            .any(|a| input.any(a) && !(a.contains(&"round") && input.has("disc")))
        {
            continue;
        }
        if matches!(
            kind,
            Kind::Box | Kind::Cylinder | Kind::Sphere | Kind::Tube | Kind::Holes | Kind::Sketch
        ) && !create
        {
            continue;
        }
        if vocabulary(*kind, &input) {
            if let Some(slots) = parse(*kind, &input) {
                found.push((*rule, slots));
            }
        }
    }
    if found.len() != 1 {
        return Ok(None);
    }
    let (rule, slots) = found.remove(0);
    from_slots(rule, slots, data, catalog)
}
fn from_slots(
    mut rule: &'static str,
    slots: Slots,
    data: Option<&Value>,
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    if matches!(slots, Slots::LProfile { .. }) {
        rule = "l_profile";
    }
    if matches!(&slots,Slots::Holes{centers,..} if centers.len()==1) {
        rule = "plate_hole";
    }
    let (mut calls, mut v, creates, mut name) = expand(&slots, catalog)?;
    if let Slots::Mirror { body, .. } = &slots {
        v = data
            .and_then(|d| d["bodies"].as_array())
            .and_then(|b| b.iter().find(|b| b["name"] == *body))
            .and_then(|b| volume(&json!({"bodies":[b]})));
    }
    if v.is_some_and(|v| !v.is_finite()) {
        return Ok(None);
    }
    // Explicit targets keep compound changes away from older overlapping bodies.
    if matches!(
        slots,
        Slots::Holes { .. }
            | Slots::Tube { .. }
            | Slots::Shell { .. }
            | Slots::CompoundPlate(_)
            | Slots::ShellBox(_)
            | Slots::CrossDrilled(_)
    ) {
        let names: Vec<_> = data
            .and_then(|d| d["bodies"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|b| b["name"].as_str())
            .collect();
        let created = (1..=MAX_BODIES + 1)
            .map(|i| format!("PilotRecipe{i}"))
            .find(|n| !names.iter().any(|old| old.eq_ignore_ascii_case(n)))
            .ok_or("recipe body name capacity exceeded")?;
        calls[0]["arguments"]["params"]["body_name"] = json!(created);
        for c in &mut calls[1..] {
            let params = &mut c["arguments"]["params"];
            if matches!(slots, Slots::CompoundPlate(_)) {
                if c["arguments"]["command"] == "solid.hole" {
                    c["arguments"]["params"]["targets"] = json!([created]);
                } else {
                    c["arguments"]["params"]["body"] = json!(created);
                }
            } else if matches!(
                slots,
                Slots::Holes { .. } | Slots::Tube { .. } | Slots::CrossDrilled(_)
            ) {
                params["targets"] = json!([created]);
            } else if matches!(slots, Slots::ShellBox(_)) {
                match c["arguments"]["command"].as_str().unwrap_or("") {
                    "solid.shell" => c["arguments"]["params"]["body"] = json!(created),
                    "solid.box" | "solid.extrude" => {
                        c["arguments"]["params"]["targets"] = json!([created])
                    }
                    _ => {}
                }
            } else {
                params["body"] = json!(created);
            }
        }
        for c in &mut calls {
            let args = &c["arguments"];
            *c = call(
                catalog,
                args["command"]
                    .as_str()
                    .ok_or("recipe command must be text")?,
                args["params"].clone(),
            )?;
        }
        name = Some(created);
    }
    let transformed_volume = match &slots {
        Slots::Scale { body, factor } => data
            .and_then(|d| d["bodies"].as_array())
            .and_then(|b| b.iter().find(|b| b["name"] == *body))
            .and_then(|b| volume(&json!({"bodies":[b]})))
            .zip(data.and_then(volume))
            .map(|(body_v, total)| total + body_v * (factor.powi(3) - 1.0)),
        Slots::Mirror {
            keep_original: false,
            ..
        } => data.and_then(volume),
        _ => None,
    };
    if transformed_volume.is_some_and(|v| !v.is_finite()) {
        return Ok(None);
    }
    let count = data
        .and_then(|d| d["bodies"].as_array())
        .map(|b| b.len() + usize::from(creates));
    if count.is_some_and(|n| n > MAX_BODIES) {
        return Ok(None);
    }
    let placement = if let Slots::Move { body, delta } = &slots {
        data.and_then(|d| d["bodies"].as_array())
            .and_then(|b| b.iter().find(|b| b["name"] == *body))
            .and_then(|b| {
                let mut min = [0.0; 3];
                let mut max = [0.0; 3];
                for i in 0..3 {
                    min[i] = b["bbox"]["min"][i].as_f64()? + delta[i];
                    max[i] = b["bbox"]["max"][i].as_f64()? + delta[i];
                    if !min[i].is_finite() || !max[i].is_finite() {
                        return None;
                    }
                }
                Some(Placement {
                    body: body.clone(),
                    min,
                    max,
                })
            })
    } else {
        None
    };
    // Without a baseline, verification requests a measurement but cannot claim a total.
    let rounding_kind = match &slots {
        Slots::Edges { fillet, .. } => Some(*fillet),
        Slots::CompoundPlate(p) => p.rounding.map(|(fillet, _, _)| fillet),
        _ => None,
    };
    let expected = Expected {
        volume_range_mm3: None,
        joint: None,
        feature: if let Some(fillet) = rounding_kind {
            let kind = if fillet {
                "FilletFeature"
            } else {
                "ChamferFeature"
            };
            data.and_then(|d| d["timeline"].as_array())
                .filter(|t| t.len() <= 4096)
                .map(|t| {
                    (
                        kind.into(),
                        1 + t
                            .iter()
                            .filter(|f| {
                                f["type"] == kind
                                    && f["error"].is_null()
                                    && f["rolled_back"] != true
                                    && f["suppressed"] != true
                            })
                            .count(),
                    )
                })
        } else {
            None
        },
        placement,
        bodies: count,
        volume_mm3: if transformed_volume.is_some() {
            transformed_volume
        } else if creates {
            data.and_then(volume).zip(v).map(|(a, b)| a + b)
        } else if matches!(slots, Slots::Move { .. } | Slots::Rename { .. }) {
            data.and_then(volume)
        } else {
            None
        },
        created_volume_mm3: if creates && data.and_then(volume).is_none() {
            v
        } else {
            None
        },
        name,
    };
    let continuation = match &slots {
        Slots::CompoundPlate(p) if p.rounding.is_some_and(|(_, all, _)| all) => p
            .rounding
            .zip(expected.name.as_ref())
            .map(|((fillet, _, amount), body)| EdgeContinuation {
                body: body.clone(),
                amount,
                fillet,
                top: false,
            }),
        Slots::Edges {
            body,
            edges,
            amount,
            fillet,
            top,
        } if edges.is_empty() => Some(EdgeContinuation {
            body: body.clone(),
            amount: *amount,
            fillet: *fillet,
            top: *top,
        }),
        _ => None,
    };
    if continuation.is_none() && catalog.get("document.inspect").is_some() {
        calls.push(call(catalog, "document.inspect", json!({"measure":true}))?);
    }
    Ok(Some(Plan {
        rule,
        calls,
        facts: None,
        slots: serde_json::to_value(slots)
            .map_err(|e| format!("cannot audit recipe slots: {e}"))?,
        expected: Some(expected),
        continuation,
    }))
}
pub(super) fn resume(
    request: &Value,
    previous: &Plan,
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    let Some(c) = &previous.continuation else {
        return Ok(None);
    };
    let Some(messages) = request["messages"].as_array() else {
        return Ok(None);
    };
    let input = Input {
        text: "",
        t: Vec::new(),
        data: None,
        messages,
    };
    let selected = if previous.rule == "compound_plate" {
        // Recover the typed geometry from its audited slots, without replaying
        // the accepted creation prefix.
        compound_plate::all_edges(&previous.slots, messages, &c.body)
    } else {
        topology(&input, &c.body, c.top)
    };
    let Some(edges) = selected else {
        return Ok(None);
    };
    let slots = Slots::Edges {
        body: c.body.clone(),
        edges,
        amount: c.amount,
        fillet: c.fillet,
        top: c.top,
    };
    let (mut calls, _, _, _) = expand(&slots, catalog)?;
    calls.push(call(catalog, "document.inspect", json!({"measure":true}))?);
    let mut next = previous.clone();
    next.calls = calls;
    next.continuation = None;
    if previous.rule != "compound_plate" {
        next.slots =
            serde_json::to_value(slots).map_err(|e| format!("cannot audit edge slots: {e}"))?;
    }
    Ok(Some(next))
}

pub(super) fn names() -> impl Iterator<Item = &'static str> {
    LIBRARY
        .iter()
        .map(|(name, _)| *name)
        .chain(["revolute_joint", "cross_drilled_box"])
}

fn positive_mm(field: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 || value > MAX_MM {
        return Err(format!(
            "{field} {value} must be above 0 and at most {MAX_MM} mm; supply a positive length in mm"
        ));
    }
    Ok(())
}
fn valid_plane(plane: &str) -> Result<(), String> {
    if !matches!(plane, "XY" | "XZ" | "YZ") {
        return Err(format!(
            "plane {plane:?} must be 1 of XY, XZ, YZ; use the requested sketch plane"
        ));
    }
    Ok(())
}
