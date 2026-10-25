//! Bounded enclosure and locating-ring recognition from ordinary, explicit sizes.
use super::*;

#[derive(Debug, serde::Serialize)]
pub(super) struct ShellBox {
    pub(super) size: [f64; 3],
    pub(super) wall: f64,
    pub(super) floor: f64,
    pub(super) bottom: bool,
    pub(super) centred: bool,
    pub(super) lip: Option<[f64; 2]>,
    pub(super) outer: bool,
}
fn lip_word(s: &str) -> bool {
    matches!(s, "lip" | "rim" | "ridge")
}
pub(super) fn candidate(i: &Input<'_>) -> bool {
    i.any(&["box", "enclosure", "container"])
        && (i.any(&[
            "shell", "shelled", "hollow", "hollowed", "open", "opening", "remove",
        ]) || i.t.iter().any(|s| lip_word(s)))
}
pub(super) fn vocabulary(i: &Input<'_>) -> bool {
    i.t.iter().enumerate().all(|(n, s)| {
        (s == "-"
            && n > 0
            && i.t[n - 1] == "open"
            && i.t
                .get(n + 1)
                .is_some_and(|s| matches!(s.as_str(), "top" | "bottom")))
            || s.parse::<f64>().is_ok()
            || s.split_once('/')
                .is_some_and(|(a, b)| a.parse::<f64>().is_ok() && b.parse::<f64>().is_ok())
            || slots::scale(s).is_some()
            || matches!(
                s.as_str(),
                "shell"
                    | "shelled"
                    | "hollow"
                    | "hollowed"
                    | "out"
                    | "box"
                    | "enclosure"
                    | "container"
                    | "make"
                    | "build"
                    | "create"
                    | "add"
                    | "a"
                    | "an"
                    | "the"
                    | "with"
                    | "and"
                    | "then"
                    | "of"
                    | "is"
                    | "it"
                    | "its"
                    | "one"
                    | "corner"
                    | "at"
                    | "origin"
                    | "centred"
                    | "centered"
                    | "length"
                    | "long"
                    | "width"
                    | "wide"
                    | "height"
                    | "high"
                    | "tall"
                    | "deep"
                    | "depth"
                    | "x"
                    | "by"
                    | "wall"
                    | "walls"
                    | "thick"
                    | "thickness"
                    | "floor"
                    | "base"
                    | "top"
                    | "bottom"
                    | "open"
                    | "opened"
                    | "opening"
                    | "remove"
                    | "removed"
                    | "left"
                    | "face"
                    | "lip"
                    | "rim"
                    | "ridge"
                    | "locating"
                    | "lid"
                    | "around"
                    | "on"
                    | "for"
                    | "to"
                    | "locate"
                    | "inner"
                    | "inside"
                    | "outer"
                    | "outside"
                    | "edges"
                    | "in"
                    | "uniform"
                    | "rectangular"
                    | "leave"
                    | "leaving"
            )
    })
}
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Wall,
    Floor,
    Width,
    Height,
}
fn label(s: &str) -> bool {
    matches!(
        s,
        "wall"
            | "walls"
            | "floor"
            | "base"
            | "width"
            | "wide"
            | "height"
            | "high"
            | "tall"
            | "thick"
            | "thickness"
            | "of"
            | "is"
    )
}
fn role(words: &[String], in_lip: bool) -> Result<Option<Role>, ()> {
    let has = |ws: &[&str]| words.iter().any(|s| ws.contains(&s.as_str()));
    let mut roles = Vec::new();
    if has(&["wall", "walls"]) {
        roles.push(Role::Wall);
    }
    if has(&["floor", "base"]) {
        roles.push(Role::Floor);
    }
    if has(&["width", "wide"]) {
        roles.push(Role::Width);
    }
    if has(&["height", "high", "tall"]) {
        roles.push(Role::Height);
    }
    if roles.is_empty() && !in_lip && has(&["thickness", "thick"]) {
        roles.push(Role::Wall);
    }
    match roles.as_slice() {
        [r] => Ok(Some(*r)),
        [] => Ok(None),
        _ => Err(()),
    }
}
pub(super) fn parse(i: &Input<'_>) -> Option<Slots> {
    if !vocabulary(i)
        || !i.any(&["box", "enclosure", "container"])
        || !i.any(&[
            "shell", "shelled", "hollow", "hollowed", "open", "remove", "opening",
        ])
    {
        return None;
    }
    if i.t.iter().filter(|s| s.as_str() == "box").count() > 1
        || i.t
            .iter()
            .filter(|s| matches!(s.as_str(), "enclosure" | "container"))
            .count()
            > 1
    {
        return None;
    }
    // Exactly one explicit opening; rim placement mentions do not name another opening.
    let mut openings = Vec::new();
    for (n, s) in i.t.iter().enumerate() {
        if matches!(
            s.as_str(),
            "open" | "opened" | "remove" | "removed" | "opening"
        ) {
            let before = &i.t[n.saturating_sub(3)..n];
            let after = &i.t[n + 1..(n + 4).min(i.t.len())];
            let faces: Vec<_> = before
                .iter()
                .chain(after)
                .filter(|s| matches!(s.as_str(), "top" | "bottom"))
                .collect();
            let [face] = faces.as_slice() else {
                return None;
            };
            openings.push(face.as_str());
        }
    }
    let [opening] = openings.as_slice() else {
        return None;
    };
    let bottom = *opening == "bottom";
    let centred = i.any(&["centred", "centered"]);
    if (i.has("corner") && !i.has("origin")) || (centred && i.has("corner")) {
        return None;
    }
    let d = dimensions(&i.t)?;
    if !(4..=7).contains(&d.len()) || !positive(&d) {
        return None;
    }
    let size = box_size(&i.t, &d[..3], false)?;
    let lip_start = i.t.iter().position(|s| lip_word(s));
    let mut assigned = Vec::new();
    let mut consumed = d[2].index + 1;
    if i.t.get(consumed).is_some_and(|s| slots::scale(s).is_some()) {
        consumed += 1;
    }
    if i.t.get(consumed).is_some_and(|s| {
        matches!(
            s.as_str(),
            "height" | "high" | "tall" | "deep" | "depth" | "thick" | "thickness"
        )
    }) {
        consumed += 1;
    }
    for (n, v) in d.iter().enumerate().skip(3) {
        let in_lip = lip_start.is_some_and(|p| p < v.index);
        let mut start = v.index;
        while start > consumed && label(&i.t[start - 1]) {
            start -= 1;
        }
        let mut after = v.index + 1;
        if i.t.get(after).is_some_and(|s| slots::scale(s).is_some()) {
            after += 1;
        }
        let mut end = after;
        while end < i.t.len() && label(&i.t[end]) {
            end += 1;
        }
        let prefix = role(&i.t[start..v.index], in_lip).ok()?;
        let suffix = role(&i.t[after..end], in_lip).ok()?;
        let next_label = d.get(n + 1).is_some_and(|v| v.index == end);
        let generic = |words: &[String]| {
            words
                .iter()
                .all(|s| matches!(s.as_str(), "thick" | "thickness" | "of" | "is"))
        };
        let r = match (prefix, suffix) {
            (Some(Role::Floor), Some(Role::Wall)) if generic(&i.t[after..end]) => Role::Floor,
            (Some(Role::Wall), Some(Role::Floor)) if generic(&i.t[start..v.index]) => Role::Floor,
            (Some(a), Some(b)) if a != b && !next_label => return None,
            (Some(a), _) => a,
            (_, Some(b)) => b,
            _ => return None,
        };
        if !next_label || prefix.is_none() {
            consumed = end;
        }
        if assigned.iter().any(|(a, _)| *a == r) {
            return None;
        }
        assigned.push((r, v.value));
    }
    let get = |r| assigned.iter().find(|(a, _)| *a == r).map(|(_, v)| *v);
    let wall = get(Role::Wall)?;
    let floor = get(Role::Floor).unwrap_or(wall);
    let lip = match (get(Role::Width), get(Role::Height)) {
        (Some(w), Some(h)) if lip_start.is_some() => Some([w, h]),
        (None, None) if lip_start.is_none() => None,
        _ => return None,
    };
    if i.has("lid") && lip.is_none() {
        return None;
    }
    let outer = i.any(&["outer", "outside"]);
    if outer && (lip.is_none() || i.any(&["inner", "inside"])) {
        return None;
    }
    ShellBox {
        size,
        wall,
        floor,
        bottom,
        centred,
        lip,
        outer,
    }
    .into_slots()
    .ok()
}

impl ShellBox {
    pub(super) fn into_slots(self) -> Result<Slots, String> {
        for (field, value) in [
            ("length", self.size[0]),
            ("width", self.size[1]),
            ("height", self.size[2]),
            ("wall", self.wall),
            ("floor", self.floor),
        ] {
            positive_mm(field, value)?;
        }
        if self.wall * 2.0 >= self.size[0].min(self.size[1]) || self.wall >= self.size[2] {
            return Err("wall: must leave a positive interior in every dimension".into());
        }
        if self.floor >= self.size[2] {
            return Err("floor: must be less than height".into());
        }
        if let Some([w, h]) = self.lip {
            positive_mm("lip.width", w)?;
            positive_mm("lip.height", h)?;
            if w >= self.wall {
                return Err("lip.width: must be narrower than wall".into());
            }
        }
        if !self.bottom && !self.centred && self.lip.is_none() && self.floor == self.wall {
            return Ok(Slots::Shell {
                size: self.size,
                wall: self.wall,
            });
        }
        Ok(Slots::ShellBox(self))
    }
    pub(super) fn volume(&self) -> f64 {
        let [x, y, z] = self.size;
        let t = self.wall;
        let ring = self.lip.map_or(0.0, |[w, h]| {
            let inset = if self.outer { 0.0 } else { t - w };
            ((x - 2.0 * inset) * (y - 2.0 * inset)
                - (x - 2.0 * (inset + w)) * (y - 2.0 * (inset + w)))
                * h
        });
        x * y * z - (x - 2.0 * t) * (y - 2.0 * t) * (z - self.floor) + ring
    }
    pub(super) fn commands(&self) -> Vec<(&'static str, Value)> {
        let [x, y, z] = self.size;
        let t = self.wall;
        let [ox, oy, oz] = if self.centred {
            [-x / 2.0, -y / 2.0, -z / 2.0]
        } else {
            [0.0; 3]
        };
        let mut out = vec![
            (
                "solid.box",
                json!({"length":x,"width":y,"height":z,"corner":[ox,oy,oz],"operation":"new"}),
            ),
            (
                "solid.shell",
                json!({"faces":[[ox+x/2.0,oy+y/2.0,oz+if self.bottom {0.0} else {z}]],"thickness":t,"direction":"inside"}),
            ),
        ];
        if self.floor != t {
            let low = self.floor.min(t);
            let dz = (self.floor - t).abs();
            let fz = if self.bottom {
                oz + z - low - dz
            } else {
                oz + low
            };
            out.push(("solid.box",json!({"length":x-2.0*t,"width":y-2.0*t,"height":dz,"corner":[ox+t,oy+t,fz],"operation":if self.floor>t {"join"} else {"cut"}})));
        }
        if let Some([w, h]) = self.lip {
            let inset = if self.outer { 0.0 } else { t - w };
            let rim_z = oz + if self.bottom { 0.0 } else { z };
            out.push((
                "sketch.create",
                json!({"plane":"XY","offset":rim_z,"name":"Locating lip"}),
            ));
            for d in [inset, inset + w] {
                out.push((
                    "sketch.rectangle.two_point",
                    json!({"p0":[ox+d,oy+d],"p1":[ox+x-d,oy+y-d]}),
                ));
            }
            out.push(("sketch.finish", json!({})));
            out.push(("solid.extrude",json!({"distance":h,"profiles":[{"point":[ox+inset+w/2.0,oy+y/2.0]}],"operation":"join","direction":if self.bottom {"negative"} else {"positive"}})));
        }
        out
    }
}
