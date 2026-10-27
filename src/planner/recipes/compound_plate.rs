//! Bounded compound plates, expressed through public primitive and feature commands.
use super::*;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Plate {
    pub(super) size: [f64; 3],
    #[serde(default)]
    pub(super) diameter: Option<f64>,
    #[serde(default)]
    pub(super) bosses: Vec<([f64; 2], f64, f64)>,
    pub(super) plane: String,
    pub(super) centred: bool,
    pub(super) holes: Vec<([f64; 2], f64)>,
    pub(super) rounding: Option<(bool, bool, f64)>, // fillet, all edges, amount
    pub(super) default_start: bool,
}
pub(super) fn candidate(i: &Input<'_>) -> bool {
    i.has("plate")
        && (i.any(&[
            "bore", "bolt", "pcd", "grid", "fillet", "round", "rounded", "chamfer", "bevel",
        ]) || (i.has("origin") && i.any(&["hole", "holes"])))
}
pub(super) fn all_edges(slots: &Value, messages: &[Value], body: &str) -> Option<Vec<Value>> {
    let plate: Plate = serde_json::from_value(slots.get("CompoundPlate")?.clone()).ok()?;
    plate.all_edges(messages, body)
}
fn number(s: &str) -> Option<f64> {
    if let Some((a, b)) = s.split_once('/') {
        let b = b.parse::<f64>().ok()?;
        (b > 0.0).then_some(a.parse::<f64>().ok()? / b)
    } else {
        s.parse().ok()
    }
}
fn count(s: &str) -> Option<usize> {
    let v = match s {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "twelve" => 12,
        _ => s.parse().ok()?,
    };
    bounded_count(v).then_some(v)
}
pub(super) fn bounded_count(value: usize) -> bool {
    (1..=64).contains(&value)
}
fn length(t: &[String], n: usize) -> Option<f64> {
    let v = number(t.get(n)?)? * t.get(n + 1).and_then(|s| slots::scale(s)).unwrap_or(1.0);
    (v.is_finite() && v > 0.0 && v <= MAX_MM).then_some(v)
}
fn allowed(s: &str) -> bool {
    number(s).is_some()
        || slots::scale(s).is_some()
        || count(s).is_some()
        || matches!(
            s,
            "make"
                | "build"
                | "create"
                | "a"
                | "an"
                | "the"
                | "plate"
                | "square"
                | "rectangular"
                | "mounting"
                | "on"
                | "in"
                | "xy"
                | "xz"
                | "yz"
                | "plane"
                | "lying"
                | "centred"
                | "centered"
                | "centre"
                | "center"
                | "origin"
                | "corner"
                | "bottom"
                | "left"
                | "at"
                | "and"
                | "with"
                | "then"
                | "put"
                | "drill"
                | "cut"
                | "through"
                | "middle"
                | "it"
                | "its"
                | "of"
                | "to"
                | "around"
                | "bore"
                | "hole"
                | "holes"
                | "bolt"
                | "circle"
                | "circular"
                | "pattern"
                | "pcd"
                | "diameter"
                | "radius"
                | "equal"
                | "equally"
                | "evenly"
                | "spaced"
                | "spacing"
                | "grid"
                | "rows"
                | "columns"
                | "pitch"
                | "x"
                | "by"
                | "thick"
                | "thickness"
                | "wide"
                | "long"
                | "width"
                | "length"
                | "height"
                | "fillet"
                | "round"
                | "rounded"
                | "chamfer"
                | "bevel"
                | "vertical"
                | "side"
                | "edges"
                | "all"
                | "starting"
                | "start"
                | "from"
                | "distance"
                | "size"
        )
}
pub(super) fn parse(i: &Input<'_>) -> Option<Slots> {
    let t = &i.t;
    if !candidate(i) || !t.iter().all(|s| allowed(s)) {
        return None;
    }
    if t.iter().enumerate().any(|(n, s)| {
        slots::scale(s).is_some()
            && s != "in"
            && n.checked_sub(1).and_then(|p| number(&t[p])).is_none()
    }) {
        return None;
    }
    let has = |s: &str| i.has(s);
    let planes: Vec<_> = t
        .iter()
        .filter(|s| matches!(s.as_str(), "xy" | "xz" | "yz"))
        .collect();
    if planes.len() > 1 {
        return None;
    }
    let plane = planes
        .first()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or("XY".into());
    // Placement must be stated. A corner placement means the lower left corner.
    if !has("origin") {
        return None;
    }
    let first_feature = t.iter().position(|s| {
        matches!(
            s.as_str(),
            "bore"
                | "hole"
                | "holes"
                | "bolt"
                | "pcd"
                | "grid"
                | "fillet"
                | "round"
                | "rounded"
                | "chamfer"
                | "bevel"
        )
    })?;
    let numeric: Vec<_> = t
        .iter()
        .enumerate()
        .filter_map(|(n, s)| number(s).map(|_| n))
        .collect();
    let first = *numeric.first()?;
    let mut base_end = *numeric.get(2)?
        + 1
        + usize::from(
            t.get(numeric[2] + 1)
                .is_some_and(|s| slots::scale(s).is_some()),
        );
    if t.get(base_end)
        .is_some_and(|s| matches!(s.as_str(), "thick" | "thickness" | "high" | "height"))
    {
        base_end += 1;
    }
    if base_end > first_feature {
        return None;
    }
    let d = dimensions(&t[first..base_end])?;
    let size = box_size(&t[first..base_end], &d, false)?;
    let centred = t[..first_feature].iter().enumerate().any(|(n, s)| {
        matches!(s.as_str(), "centred" | "centered")
            && t.get(n + 1)
                .is_some_and(|s| matches!(s.as_str(), "on" | "at"))
            && (t.get(n + 2).is_some_and(|s| s == "origin")
                || (t.get(n + 2).is_some_and(|s| s == "the")
                    && t.get(n + 3).is_some_and(|s| s == "origin")))
    });
    let corner = t[..first_feature].iter().any(|s| s == "corner");
    if centred == corner {
        return None;
    }
    if has("square") && size[0] != size[1] {
        return None;
    }
    let groups: Vec<_> = t
        .iter()
        .enumerate()
        .filter(|(_, s)| *s == "holes")
        .map(|(n, _)| n)
        .collect();
    if groups.len() > 1 {
        if groups.len() != 2 || !i.any(&["bolt", "pcd"]) || !i.any(&["grid", "rows"]) {
            return None;
        }
        let split =
            (groups[0] + 1..groups[1]).rfind(|n| matches!(t[*n].as_str(), "and" | "with"))?;
        let prefix_end = (base_end..first_feature)
            .find(|n| matches!(t[*n].as_str(), "with" | "put" | "then"))?;
        let left = Input {
            text: i.text,
            t: t[..split].to_vec(),
            data: i.data,
            messages: i.messages,
        };
        let mut right_tokens = t[..prefix_end].to_vec();
        right_tokens.extend_from_slice(&t[split + 1..]);
        let right = Input {
            text: i.text,
            t: right_tokens,
            data: i.data,
            messages: i.messages,
        };
        let Slots::CompoundPlate(mut a) = parse(&left)? else {
            return None;
        };
        let Slots::CompoundPlate(b) = parse(&right)? else {
            return None;
        };
        a.holes.extend(b.holes);
        a.rounding = b.rounding;
        a.default_start |= b.default_start;
        return a.validate().ok().map(|()| Slots::CompoundPlate(a));
    }
    let edge_start = t.iter().position(|s| {
        matches!(
            s.as_str(),
            "fillet" | "round" | "rounded" | "chamfer" | "bevel"
        )
    });
    let end = edge_start.unwrap_or(t.len());
    let mut used = vec![false; t.len()];
    used[..base_end].fill(true);
    let rounding = if let Some(n) = edge_start {
        let tail = &t[n..];
        if tail
            .iter()
            .filter(|s| {
                matches!(
                    s.as_str(),
                    "fillet" | "round" | "rounded" | "chamfer" | "bevel"
                )
            })
            .count()
            != 1
            || !tail.iter().any(|s| s == "edges")
        {
            return None;
        }
        let all = tail.iter().any(|s| s == "all")
            && !tail
                .iter()
                .any(|s| matches!(s.as_str(), "four" | "4" | "vertical" | "corner" | "side"));
        if !(all
            || (tail.iter().any(|s| matches!(s.as_str(), "four" | "4"))
                && tail
                    .iter()
                    .any(|s| matches!(s.as_str(), "vertical" | "corner" | "side"))))
        {
            return None;
        }
        let ns: Vec<_> = numeric
            .iter()
            .copied()
            .filter(|p| {
                *p >= n
                    && !(t[*p] == "4"
                        && t.get(*p + 1)
                            .is_some_and(|s| matches!(s.as_str(), "vertical" | "corner" | "side")))
            })
            .collect();
        let [p] = ns.as_slice() else {
            return None;
        };
        let amount = length(t, *p)?;
        used[n..].fill(true);
        Some((
            matches!(t[n].as_str(), "fillet" | "round" | "rounded"),
            all,
            amount,
        ))
    } else {
        None
    };
    let mut holes = Vec::new();
    if t[..end]
        .iter()
        .filter(|s| matches!(s.as_str(), "bore" | "hole"))
        .count()
        > 1
    {
        return None;
    }
    let mut default_start = false;
    let mut take_size = |anchor: usize| -> Option<(f64, bool)> {
        // Diameter/radius can precede or follow the size, on either side of its noun.
        let candidates: Vec<_> = numeric
            .iter()
            .copied()
            .filter(|n| !used[*n] && *n < end && n.abs_diff(anchor) <= 4)
            .collect();
        let p = *candidates.iter().min_by_key(|n| n.abs_diff(anchor))?;
        let lo = p.min(anchor).saturating_sub(1).max(base_end);
        let hi = (p.max(anchor) + 3).min(end);
        let radius = t[lo..hi].iter().any(|s| s == "radius");
        if radius && t[lo..hi].iter().any(|s| s == "diameter") {
            return None;
        }
        used[p] = true;
        Some((length(t, p)?, radius))
    };
    let circle = t.iter().position(|s| s == "pcd").or_else(|| {
        t.windows(2)
            .position(|w| w[0] == "bolt" && w[1] == "circle")
            .map(|n| n + 1)
    });
    let circle_size = if let Some(n) = circle {
        Some(take_size(n)?)
    } else {
        None
    };
    // Centre bore is distinct from the equal pattern holes.
    if let Some(n) = t[..end].iter().position(|s| s == "bore" || s == "hole") {
        if !t[base_end..end].iter().enumerate().any(|(p, s)| {
            (base_end + p).abs_diff(n) <= 5
                && (matches!(s.as_str(), "middle" | "centre" | "center")
                    || (matches!(s.as_str(), "centred" | "centered")
                        && t.get(base_end + p + 1)
                            .is_none_or(|s| !matches!(s.as_str(), "on" | "at"))))
        }) {
            return None;
        }
        let (v, r) = take_size(n)?;
        holes.push(([0.0, 0.0], if r { v } else { v / 2.0 }));
    }
    // Release the mutable borrow before parsing counts and grid pitches.
    let mut pattern = Vec::new();
    if let Some(n) = t[..end].iter().position(|s| s == "holes") {
        let available: Vec<_> = (base_end..end)
            .filter(|p| !used[*p] && number(&t[*p]).is_some())
            .collect();
        let (diameter_index, count_value) = if circle.is_some() {
            let cp = (base_end..n).find(|p| count(&t[*p]).is_some() && !used[*p])?;
            let count_value = count(&t[cp])?;
            used[cp] = true;
            let ds: Vec<_> = available.into_iter().filter(|p| *p != cp).collect();
            let [p] = ds.as_slice() else {
                return None;
            };
            (*p, count_value)
        } else if has("grid") || has("rows") {
            let pitch = t.iter().position(|s| s == "pitch")?;
            let (row_at, col_at) = if let (Some(a), Some(b)) = (
                t.iter().position(|s| s == "rows"),
                t.iter().position(|s| s == "columns"),
            ) {
                (a.checked_sub(1)?, b.checked_sub(1)?)
            } else {
                let grid = t.iter().position(|s| s == "grid")?;
                let pairs: Vec<_> = available
                    .windows(2)
                    .filter(|w| {
                        w[1] < pitch
                            && w[1] == w[0] + 2
                            && matches!(t[w[0] + 1].as_str(), "x" | "by")
                            && w[0].abs_diff(grid) <= 5
                    })
                    .collect();
                let [pair] = pairs.as_slice() else {
                    return None;
                };
                // Unlabelled A x B puts A on the longer sketch side.
                if size[0] >= size[1] {
                    (pair[1], pair[0])
                } else {
                    (pair[0], pair[1])
                }
            };
            let rows = count(&t[row_at])?;
            let cols = count(&t[col_at])?;
            if rows * cols > 64 {
                return None;
            }
            used[row_at] = true;
            used[col_at] = true;
            let ps: Vec<_> = available.iter().copied().filter(|p| *p > pitch).collect();
            let a = *ps.first()?;
            let b = *ps.last()?;
            if ps.len() > 2 {
                return None;
            }
            let dims = dimensions(&t[a..(b + 2).min(end)])?;
            if dims.is_empty() || dims.len() > 2 || !positive(&dims) {
                return None;
            }
            used[a] = true;
            used[b] = true;
            let px = dims[0].value;
            let py = dims.get(1).map_or(px, |d| d.value);
            for row in 0..rows {
                for col in 0..cols {
                    pattern.push([
                        (col as f64 - (cols - 1) as f64 / 2.0) * px,
                        (row as f64 - (rows - 1) as f64 / 2.0) * py,
                    ]);
                }
            }
            if !t[base_end..end].iter().enumerate().any(|(p, s)| {
                matches!(s.as_str(), "centred" | "centered")
                    && t.get(base_end + p + 1)
                        .is_none_or(|s| !matches!(s.as_str(), "on" | "at"))
            }) {
                return None;
            }
            let ds: Vec<_> = available.into_iter().filter(|p| !used[*p]).collect();
            let [p] = ds.as_slice() else {
                return None;
            };
            (*p, rows * cols)
        } else {
            return None;
        };
        used[diameter_index] = true;
        let mut diameter = length(t, diameter_index)?;
        let after = diameter_index
            + 1
            + usize::from(
                t.get(diameter_index + 1)
                    .is_some_and(|s| slots::scale(s).is_some()),
            );
        if diameter_index
            .checked_sub(1)
            .and_then(|p| t.get(p))
            .is_some_and(|s| s == "radius")
            || t.get(after).is_some_and(|s| s == "radius")
        {
            diameter *= 2.0;
        }
        if let Some((v, r)) = circle_size {
            if !i.any(&["evenly", "equally", "equal"]) || !has("spaced") {
                return None;
            }
            let radius = if r { v } else { v / 2.0 };
            default_start = !i.any(&["starting", "start"]);
            if !default_start
                && !t
                    .windows(2)
                    .any(|w| matches!(w[0].as_str(), "at" | "from") && w[1] == "x")
            {
                return None;
            }
            for k in 0..count_value {
                let angle = 2.0 * PI * k as f64 / count_value as f64;
                pattern.push([radius * angle.cos(), radius * angle.sin()]);
            }
        }
        holes.extend(pattern.into_iter().map(|p| (p, diameter / 2.0)));
    } else if circle.is_some() || has("grid") {
        return None;
    }
    if numeric.iter().any(|n| !used[*n])
        || t.iter()
            .enumerate()
            .any(|(n, s)| !used[n] && count(s).is_some() && s != "one")
        || holes.len() > 65
        || (holes.is_empty() && rounding.is_none())
    {
        return None;
    }
    let plate = Plate {
        diameter: None,
        bosses: Vec::new(),
        size,
        plane,
        centred,
        holes,
        rounding,
        default_start,
    };
    plate.validate().ok().map(|()| Slots::CompoundPlate(plate))
}
impl Plate {
    pub(super) fn all_edges(&self, messages: &[Value], body: &str) -> Option<Vec<Value>> {
        let data = tool_payload(messages.iter().rev().find(|m| m["role"] == "tool")?)?;
        if data["body"] != body {
            return None;
        }
        let edges = data["edges"].as_array()?;
        if edges.is_empty() || edges.len() > 512 {
            return None;
        }
        let mut out = Vec::new();
        for edge in edges {
            let p = &edge["mid"];
            let point = [p[0].as_f64()?, p[1].as_f64()?, p[2].as_f64()?];
            if point.iter().any(|v| !v.is_finite()) {
                return None;
            }
            let (u, v, h) = match self.plane.as_str() {
                "XZ" => (point[0], -point[2], point[1]),
                "YZ" => (point[1], point[2], point[0]),
                _ => (point[0], point[1], point[2]),
            };
            let [w, d, t] = self.size;
            let (x, y) = if self.centred {
                (-w / 2.0, -d / 2.0)
            } else {
                (0.0, 0.0)
            };
            // Every cap rim is sharp. Only the four outside corners are sharp
            // along the extrusion; cylinder seams are smooth and must be omitted.
            let cap = h.abs() < 1e-6 || (h - t).abs() < 1e-6;
            let corner = ((u - x).abs() < 1e-6 || (u - x - w).abs() < 1e-6)
                && ((v - y).abs() < 1e-6 || (v - y - d).abs() < 1e-6);
            if cap || corner {
                out.push(json!(point));
            }
        }
        (!out.is_empty()).then_some(out)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        let Self {
            size,
            holes,
            rounding,
            ..
        } = self;
        valid_plane(&self.plane)?;
        if self.diameter.is_some() && rounding.is_some() {
            return Err("corners require a rectangular plate; omit corners with diameter".into());
        }
        if !self.bosses.is_empty() && rounding.is_some() {
            return Err("corners with bosses are unsupported; omit corners".into());
        }
        if self.bosses.len() > 64 {
            return Err("boss count must be at most 64".into());
        }
        for (n, (p, r, h)) in self.bosses.iter().enumerate() {
            positive_mm("boss_diameter", 2.0 * r)?;
            positive_mm("boss_height", *h)?;
            if size[2] + h > MAX_MM || p.iter().any(|v| !v.is_finite() || v.abs() > MAX_MM) {
                return Err("boss offsets and combined thickness/height must be finite and within 1000000 mm".into());
            }
            if p[0].abs() + r >= size[0] / 2.0
                || p[1].abs() + r >= size[1] / 2.0
                || self
                    .diameter
                    .is_some_and(|d| p[0].hypot(p[1]) + r >= d / 2.0)
            {
                return Err(format!("bosses[{n}] must fit strictly inside the plate; reduce diameter, offset or pitch"));
            }
            if self.bosses[..n]
                .iter()
                .any(|(q, s, _)| (p[0] - q[0]).hypot(p[1] - q[1]) <= r + s)
                || holes
                    .iter()
                    .any(|(q, s)| (p[0] - q[0]).hypot(p[1] - q[1]) <= r + s)
            {
                return Err(format!("bosses[{n}] overlaps another boss or hole; increase spacing or reduce diameters"));
            }
        }
        for (field, value) in [
            ("length", size[0]),
            ("width", size[1]),
            ("thickness", size[2]),
        ] {
            positive_mm(field, value)?;
        }
        if let Some((_, all, amount)) = rounding {
            positive_mm("corners.size", *amount)?;
            if *amount >= size[0].min(size[1]) / 2.0 || (*all && *amount >= size[2] / 2.0) {
                let side = if *all {
                    size[0].min(size[1]).min(size[2])
                } else {
                    size[0].min(size[1])
                };
                let edges = if *all { "all" } else { "vertical" };
                let fix = if *all && *amount < size[0].min(size[1]) / 2.0 {
                    "for the four corner edges through the thickness use edges \"vertical\""
                } else {
                    "reduce corners.size or increase the plate length and width"
                };
                return Err(format!("corners.size {amount} with edges \"{edges}\" must be below {} (half the {side} mm {}); {fix}", side / 2.0, if *all && side == size[2] { "thickness" } else { "side" }));
            }
        }
        for (p, r) in holes {
            positive_mm("hole_diameter", 2.0 * r)?;
            if p.iter().any(|v| !v.is_finite()) {
                return Err("holes: positions must be finite".into());
            }
        }
        if holes.len() > 65 {
            return Err(format!("holes count {} must be at most 65 (1 bore plus 64 pattern holes); reduce pattern counts", holes.len()));
        }
        // Refuse tangent/overlapping holes, and conservatively keep holes out of corner blends.
        for (n, (p, r)) in holes.iter().enumerate() {
            if p[0].abs() + r >= size[0] / 2.0
                || p[1].abs() + r >= size[1] / 2.0
                || self
                    .diameter
                    .is_some_and(|d| p[0].hypot(p[1]) + r >= d / 2.0)
            {
                return Err(format!("holes[{n}] centre {p:?} radius {r} must fit strictly inside half-sides [{},{}]; reduce bore_diameter/hole_diameter or pattern spacing", size[0] / 2.0, size[1] / 2.0));
            }
            if let Some((_, all, a)) = rounding {
                if *all
                    && (p[0].abs() + r + a >= size[0] / 2.0
                        || p[1].abs() + r + a >= size[1] / 2.0
                        || *r <= *a)
                {
                    return Err(format!("holes[{n}] centre {p:?} radius {r} needs corners.size {a} clearance inside half-sides [{},{}] and radius above {a}; reduce corners.size or use corners.edges \"vertical\"", size[0] / 2.0, size[1] / 2.0));
                }
                if p[0].abs() + r > size[0] / 2.0 - a && p[1].abs() + r > size[1] / 2.0 - a {
                    return Err(format!("holes[{n}] centre {p:?} radius {r} must avoid corner zones starting at [{},{}]; reduce hole_diameter, spacing or corners.size", size[0] / 2.0 - a, size[1] / 2.0 - a));
                }
            }
            if let Some((q, s)) = holes[..n]
                .iter()
                .find(|(q, s)| (p[0] - q[0]).hypot(p[1] - q[1]) <= r + s)
            {
                return Err(format!("holes[{n}] centre distance {} must exceed radii sum {}; reduce hole_diameter/bore_diameter or increase pattern spacing", (p[0]-q[0]).hypot(p[1]-q[1]), r+s));
            }
        }
        Ok(())
    }
    fn point(&self, u: f64, v: f64, h: f64) -> [f64; 3] {
        match self.plane.as_str() {
            "XZ" => [u, h, -v],
            "YZ" => [h, u, v],
            _ => [u, v, h],
        }
    }
    pub(super) fn commands(&self) -> Vec<(&'static str, Value)> {
        let [w, h, d] = self.size;
        let (x, y) = if self.centred {
            (-w / 2.0, -h / 2.0)
        } else {
            (0.0, 0.0)
        };
        let a = self.point(x, y, 0.0);
        let b = self.point(x + w, y + h, d);
        let min = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let extent = [
            (a[0] - b[0]).abs(),
            (a[1] - b[1]).abs(),
            (a[2] - b[2]).abs(),
        ];
        let mut out = vec![(
            "solid.box",
            json!({"length":extent[0],"width":extent[1],"height":extent[2],"corner":min,"operation":"new"}),
        )];
        let axis = match self.plane.as_str() {
            "XZ" => "Y",
            "YZ" => "X",
            _ => "Z",
        };
        if let Some(diameter) = self.diameter {
            out[0] = (
                "solid.cylinder",
                json!({"diameter":diameter,"height":d,"base":self.point(x+w/2.0,y+h/2.0,0.0),"axis":axis,"operation":"new"}),
            );
        }
        for (p, r, height) in &self.bosses {
            out.push(("solid.cylinder", json!({"radius":r,"height":height,"base":self.point(x+w/2.0+p[0],y+h/2.0+p[1],d),"axis":axis,"operation":"join"})));
        }
        for (p, r) in &self.holes {
            out.push((
                "solid.hole",
                json!({"position":self.point(x+w/2.0+p[0],y+h/2.0+p[1],d),"diameter":2.0*r}),
            ));
        }
        if let Some((fillet, all, amount)) = self.rounding {
            if all {
                out.push(("model.edges", json!({})));
                return out;
            }
            let mut edges = vec![];
            for u in [x, x + w] {
                for v in [y, y + h] {
                    edges.push(self.point(u, v, d / 2.0));
                }
            }
            let mut params = json!({"edges":edges});
            params[if fillet { "radius" } else { "distance" }] = json!(amount);
            out.push((
                if fillet {
                    "solid.fillet"
                } else {
                    "solid.chamfer"
                },
                params,
            ));
        }
        out
    }
    pub(super) fn volume(&self) -> Option<f64> {
        let [w, h, d] = self.size;
        let area = self
            .diameter
            .map_or(w * h, |diameter| PI * (diameter / 2.0).powi(2));
        let mut v = area * d - self.holes.iter().map(|(_, r)| PI * r * r * d).sum::<f64>()
            + self
                .bosses
                .iter()
                .map(|(_, r, height)| PI * r * r * height)
                .sum::<f64>();
        if let Some((fillet, all, a)) = self.rounding {
            if all {
                return None;
            }
            v -= if fillet {
                (4.0 - PI) * a * a * d
            } else {
                2.0 * a * a * d
            };
        }
        Some(v)
    }
}
