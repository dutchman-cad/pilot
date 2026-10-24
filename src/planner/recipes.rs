//! Recipe table, typed slots, and deterministic catalogue-checked expansions.
use super::*;
use std::f64::consts::PI;

#[derive(Clone, Copy)]
enum Kind {
    Box,
    Cylinder,
    Sphere,
    Holes,
    Sketch,
    Fillet,
    Chamfer,
    Move,
    Rename,
    Mirror,
    Shell,
    Revolve,
}
const LIBRARY: &[(&str, Kind)] = &[
    ("box", Kind::Box),
    ("cylinder", Kind::Cylinder),
    ("sphere", Kind::Sphere),
    ("plate_holes", Kind::Holes),
    ("sketch_extrude", Kind::Sketch),
    ("fillet", Kind::Fillet),
    ("chamfer", Kind::Chamfer),
    ("move", Kind::Move),
    ("rename", Kind::Rename),
    ("mirror", Kind::Mirror),
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
    Sketch {
        plane: String,
        size: Vec<f64>,
        distance: f64,
    },
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
    },
    Shell {
        size: [f64; 3],
        wall: f64,
    },
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
        let found: Vec<_> = self
            .t
            .iter()
            .filter(|s| matches!(s.as_str(), "xy" | "xz" | "yz"))
            .collect();
        (found.len() == 1).then(|| found[0].to_ascii_uppercase())
    }
}
fn positive(d: &[slots::Dimension]) -> bool {
    d.iter().all(|d| d.value > 0.0)
}
fn vocabulary(kind: Kind, input: &Input<'_>) -> bool {
    if matches!(kind, Kind::Rename) {
        return true;
    }
    let mut text = input.text.to_ascii_lowercase();
    if matches!(
        kind,
        Kind::Move | Kind::Mirror | Kind::Fillet | Kind::Chamfer
    ) {
        let Some(stripped) = input.without_body() else {
            return false;
        };
        text = stripped;
    }
    if matches!(kind, Kind::Sketch) {
        if let Some((profile, after)) = text.split_once(" on ") {
            if let Some((_, extent)) = after.split_once(" then extrude ") {
                text = format!("{profile} on then extrude {extent}");
            }
        }
    }
    let common = [
        "make",
        "create",
        "build",
        "add",
        "please",
        "can",
        "could",
        "you",
        "a",
        "an",
        "the",
        "of",
        "mm",
        "cm",
        "m",
        "in",
        "inch",
        "inches",
        "x",
        "by",
        "and",
        "with",
        "length",
        "width",
        "height",
        "depth",
        "thickness",
        "radius",
        "diameter",
        "across",
        "long",
        "wide",
        "high",
        "deep",
        "thick",
        "tall",
    ];
    let specific: &[&str] = match kind {
        Kind::Box => &["box", "block", "plate", "cube"],
        Kind::Cylinder => &["cylinder", "rod", "disc"],
        Kind::Sphere => &["sphere"],
        Kind::Holes => &[
            "plate",
            "hole",
            "holes",
            "through",
            "centred",
            "centered",
            "center",
            "centre",
            "middle",
            "one",
            "in",
            "at",
            "rectangular",
            "circular",
            "pattern",
            "rows",
            "columns",
            "pitch",
            "equally",
            "spaced",
            "starting",
        ],
        Kind::Sketch => &["sketch", "rectangle", "circle", "on", "then", "extrude"],
        Kind::Fillet | Kind::Chamfer => &[
            "fillet", "chamfer", "all", "top", "edges", "it", "body", "current", "distance",
        ],
        Kind::Move => &[
            "move",
            "translate",
            "it",
            "body",
            "current",
            "along",
            "on",
            "y",
            "z",
            "positive",
            "negative",
        ],
        Kind::Mirror => &[
            "mirror", "it", "body", "current", "about", "across", "plane", "xy", "xz", "yz",
        ],
        Kind::Shell => &["shell", "box", "wall", "open", "remove", "top"],
        Kind::Revolve => &[
            "revolve",
            "rectangle",
            "profile",
            "from",
            "axis",
            "on",
            "xz",
            "about",
            "z",
            "full",
            "turn",
        ],
        Kind::Rename => &[],
    };
    tokens(&text).iter().all(|s| {
        common.contains(&s.as_str())
            || specific.contains(&s.as_str())
            || s.parse::<f64>().is_ok()
            || s.split_once('/')
                .is_some_and(|(a, b)| a.parse::<f64>().is_ok() && b.parse::<f64>().is_ok())
    })
}
fn box_size(t: &[String], d: &[slots::Dimension], cube: bool) -> Option<[f64; 3]> {
    if cube && d.len() == 1 && positive(d) {
        return Some([d[0].value; 3]);
    }
    if d.len() != 3 || !positive(d) {
        return None;
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
    let dia = labelled(t, d, &["diameter", "across"]);
    match (r, dia) {
        (true, false) => Some(d.value),
        (false, true) => Some(d.value / 2.0),
        _ => None,
    }
}
fn parse(kind: Kind, input: &Input<'_>) -> Option<Slots> {
    let has = |w| input.has(w);
    let t = &input.t;
    match kind {
        Kind::Rename if has("rename") => {
            let lower = input.text.to_ascii_lowercase();
            let (source, _) = lower.rsplit_once(" to ")?;
            let new = input
                .text
                .get(source.len() + 4..)?
                .trim()
                .trim_end_matches(['.', '?'])
                .trim_matches(['`', '\'', '"']);
            let raw = input
                .text
                .get(source.len() + 4..)?
                .trim()
                .trim_end_matches(['.', '?']);
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
                body: body(source, input.data)?,
                name: new.into(),
            })
        }
        Kind::Move if input.any(&["move", "translate"]) => {
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
        Kind::Mirror if has("mirror") => {
            if !input.dims()?.is_empty() {
                return None;
            }
            Some(Slots::Mirror {
                body: body(input.text, input.data)?,
                plane: input.plane()?,
            })
        }
        Kind::Fillet | Kind::Chamfer
            if (matches!(kind, Kind::Fillet) && has("fillet"))
                || (matches!(kind, Kind::Chamfer) && has("chamfer")) =>
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
        Kind::Holes if has("plate") && input.any(&["hole", "holes"]) => holes(input),
        Kind::Shell if has("shell") && has("box") => {
            // Explicit new box, inward wall, top removed. Other shell profiles defer.
            if !has("top")
                || !input.any(&["open", "remove"])
                || input.any(&["outside", "both", "closed"])
            {
                return None;
            }
            let d = dimensions(t)?;
            if d.len() != 4 || !positive(&d) || !labelled(t, d[3], &["wall", "thickness"]) {
                return None;
            }
            let size = box_size(t, &d[..3], false)?;
            let wall = d[3].value;
            if wall * 2.0 >= size[0].min(size[1]) || wall >= size[2] {
                return None;
            }
            Some(Slots::Shell { size, wall })
        }
        Kind::Sketch if has("sketch") && has("extrude") && input.any(&["rectangle", "circle"]) => {
            if has("rectangle") && has("circle") {
                return None;
            }
            let lower = input.text.to_ascii_lowercase();
            let (profile, after) = lower.split_once(" on ")?;
            let (plane, extent) = after.split_once(" then extrude ")?;
            let plane = input
                .text
                .get(profile.len() + 4..profile.len() + 4 + plane.len())?
                .trim()
                .trim_matches(['`', '\'', '"']);
            if plane.is_empty()
                || plane.len() > 128
                || plane.contains([',', '\n', '\r'])
                || plane.split_whitespace().count() > 4
            {
                return None;
            }
            let profile = tokens(profile);
            let d = dimensions(&profile)?;
            let extent = dimensions(&tokens(extent))?;
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
                vec![radial(&profile, d[0])?]
            } else {
                return None;
            };
            Some(Slots::Sketch {
                plane: if matches!(plane.to_ascii_lowercase().as_str(), "xy" | "xz" | "yz") {
                    plane.to_ascii_uppercase()
                } else {
                    plane.into()
                },
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
    let size = box_size(base, &dimensions(base)?, false)?;
    let mut rest = input.t[split + 1..].to_vec();
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
    let d = dimensions(hole_tokens)?;
    if d.len() != 1 || !positive(&d) {
        return None;
    }
    let diameter = radial(hole_tokens, d[0])? * 2.0;
    let mut centers = Vec::new();
    if boundary.is_some_and(|i| rest[i] == "at") {
        let (_, positions) = input.text.split_once(" at ")?;
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
                .trim_matches(|c: char| c.is_whitespace() || c == '.')
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
        Slots::Mirror { body, plane } => {
            add("solid.mirror", json!({"bodies":[body],"plane":plane}))?;
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
pub(super) fn select(
    text: &str,
    data: Option<&Value>,
    messages: &[Value],
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    let input = Input {
        text,
        t: tokens(text),
        data,
        messages,
    };
    // Reject multiple action families before parsing; sketch + extrude is one recipe.
    let actions = [
        &["move", "translate"][..],
        &["rename"][..],
        &["mirror"][..],
        &["fillet"][..],
        &["chamfer"][..],
        &["shell"][..],
        &["revolve"][..],
        &["extrude"][..],
    ];
    if actions.iter().filter(|a| input.any(a)).count() > 1 {
        return Ok(None);
    }
    if actions
        .iter()
        .any(|a| input.t.iter().filter(|w| a.contains(&w.as_str())).count() > 1)
        || input
            .t
            .iter()
            .filter(|w| matches!(w.as_str(), "make" | "create" | "build" | "add"))
            .count()
            > 1
    {
        return Ok(None);
    }
    let create = input.any(&[
        "make", "create", "build", "add", "sketch", "shell", "revolve",
    ]);
    let mut found = Vec::new();
    for (rule, kind) in LIBRARY {
        let shapes: &[&str] = match kind {
            Kind::Box => &["box", "block", "plate", "cube"],
            Kind::Cylinder => &["cylinder", "rod", "disc"],
            Kind::Sphere => &["sphere"],
            Kind::Sketch => &["rectangle", "circle"],
            Kind::Shell => &["box"],
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
            Kind::Box | Kind::Cylinder | Kind::Sphere | Kind::Holes
        ) && actions.iter().any(|a| input.any(a))
        {
            continue;
        }
        if matches!(
            kind,
            Kind::Box | Kind::Cylinder | Kind::Sphere | Kind::Holes | Kind::Sketch
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
    let (mut rule, slots) = found.remove(0);
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
    // Explicit targets keep compound changes away from older overlapping bodies.
    if matches!(slots, Slots::Holes { .. } | Slots::Shell { .. }) {
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
            if matches!(slots, Slots::Holes { .. }) {
                params["targets"] = json!([created]);
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
    let expected = Expected {
        placement,
        bodies: count,
        volume_mm3: if creates {
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
    let Some(edges) = topology(&input, &c.body, c.top) else {
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
    next.slots =
        serde_json::to_value(slots).map_err(|e| format!("cannot audit edge slots: {e}"))?;
    Ok(Some(next))
}
