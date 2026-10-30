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
        || parse_explicit(i).is_some()
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
                    | "cover"
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
    parse_original(i).or_else(|| parse_explicit(i))
}

fn parse_original(i: &Input<'_>) -> Option<Slots> {
    // `cover` belongs to the new grammar; it was outside this vocabulary before.
    if i.has("cover")
        || !vocabulary(i)
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

/// A small clause grammar for additional explicit requests. The original recognizer
/// wins first, so its slots and command selection remain unchanged.
fn parse_explicit(i: &Input<'_>) -> Option<Slots> {
    if !vocabulary(i) || i.t.len() > 128 {
        return None;
    }
    let d = dimensions(&i.t)?;
    if !(4..=7).contains(&d.len()) || !positive(&d) {
        return None;
    }
    let part = |s: &str| matches!(s, "box" | "enclosure" | "container" | "cover" | "lid");
    let parts: Vec<_> =
        i.t.iter()
            .enumerate()
            .filter(|(_, s)| part(s))
            .map(|(n, _)| n)
            .collect();
    if parts.windows(2).any(|p| {
        i.t[p[0] + 1..p[1]]
            .iter()
            .any(|s| matches!(s.as_str(), "and" | "a" | "an"))
    }) {
        return None;
    }
    // Only a contiguous three-size tuple is accepted by this grammar.
    for pair in d[..3].windows(2) {
        let mut between = &i.t[pair[0].index + 1..pair[1].index];
        if between.first().is_some_and(|s| slots::scale(s).is_some()) {
            between = &between[1..];
        }
        if !matches!(between, [s] if s == "x" || s == "by") {
            return None;
        }
    }
    let units: Vec<_> = d[..3]
        .iter()
        .map(|v| i.t.get(v.index + 1).and_then(|s| slots::scale(s)))
        .collect();
    if units
        .iter()
        .flatten()
        .any(|u| units.iter().flatten().any(|v| u != v))
        && units.iter().any(Option::is_none)
    {
        return None;
    }
    let mut words = i.t.clone();
    // Replace lengths with their already bounded mm values, removing unit tokens.
    for v in d.iter().rev() {
        if words
            .get(v.index + 1)
            .is_some_and(|s| slots::scale(s).is_some())
        {
            words.remove(v.index + 1);
        }
        words[v.index] = v.value.to_string();
    }
    words.retain(|s| {
        !matches!(
            s.as_str(),
            "and" | "with" | "a" | "an" | "the" | "of" | "is" | "thick" | "thickness" | "around"
        )
    });
    // Consume the part phrase before the size tuple (or immediately after it).
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "make" | "build" | "create"))
    {
        words.remove(0);
    }
    let mut bottom = None;
    let mut n = 0;
    while n < words.len() {
        let pair = words.get(n + 1).map(String::as_str);
        let opening = match (words[n].as_str(), pair) {
            ("open", Some("top")) | ("top", Some("open")) => Some(false),
            ("open", Some("bottom")) | ("bottom", Some("open")) => Some(true),
            ("open", Some("-"))
                if words
                    .get(n + 2)
                    .is_some_and(|s| s == "top" || s == "bottom") =>
            {
                words.remove(n + 1);
                Some(words[n + 1] == "bottom")
            }
            _ => None,
        };
        if let Some(value) = opening {
            if bottom.replace(value).is_some() {
                return None;
            }
            words.drain(n..n + 2);
        } else {
            n += 1;
        }
    }
    let mut nouns = Vec::new();
    while words.first().is_some_and(|s| part(s)) {
        nouns.push(words.remove(0));
    }
    if words.len() < 5 {
        return None;
    }
    let size = [
        words[0].parse().ok()?,
        words[2].parse().ok()?,
        words[4].parse().ok()?,
    ];
    if !matches!(words[1].as_str(), "x" | "by") || !matches!(words[3].as_str(), "x" | "by") {
        return None;
    }
    words.drain(..5);
    while words.first().is_some_and(|s| part(s)) {
        nouns.push(words.remove(0));
    }
    if nouns.is_empty()
        || nouns.len() > 2
        || (nouns.len() == 2
            && (matches!(nouns[0].as_str(), "cover" | "lid")
                || !matches!(nouns[1].as_str(), "cover" | "lid")))
    {
        return None;
    }
    let cover = nouns
        .last()
        .is_some_and(|s| matches!(s.as_str(), "cover" | "lid"));
    let bottom = bottom.unwrap_or(cover);
    // Bare boxes need an explicit opening; enclosure itself names the open form.
    if !cover && !i.any(&["open", "enclosure"]) {
        return None;
    }
    let mut wall = None;
    let mut floor = None;
    let mut lip = None;
    let outer = words
        .iter()
        .any(|s| matches!(s.as_str(), "outer" | "outside"));
    // Side words describe the lip only, never the dimensions of the cavity.
    let sides: Vec<_> = words
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s.as_str(), "outer" | "outside" | "inner" | "inside"))
        .collect();
    if sides.len() > 1
        || sides.iter().any(|(n, _)| {
            !words[n.saturating_sub(1)..(*n + 2).min(words.len())]
                .iter()
                .any(|s| lip_word(s))
                && !(*n == words.len() - 1 && words.iter().any(|s| lip_word(s)))
        })
    {
        return None;
    }
    if outer
        && words
            .iter()
            .any(|s| matches!(s.as_str(), "inner" | "inside"))
    {
        return None;
    }
    words.retain(|s| {
        !matches!(
            s.as_str(),
            "outer" | "outside" | "inner" | "inside" | "locating"
        )
    });
    while !words.is_empty() {
        let (name, value, used) = if let Some(v) = words.first().and_then(|s| s.parse::<f64>().ok())
        {
            (words.get(1)?.clone(), v, 2)
        } else {
            (words[0].clone(), words.get(1)?.parse::<f64>().ok()?, 2)
        };
        words.drain(..used);
        match name.as_str() {
            "wall" | "walls" if wall.is_none() => wall = Some(value),
            "floor" | "base" if floor.is_none() => floor = Some(value),
            "lip" | "rim" | "ridge" if lip.is_none() => {
                let mut width = None;
                let height;
                if words
                    .first()
                    .is_some_and(|s| matches!(s.as_str(), "wide" | "width"))
                {
                    words.remove(0);
                    width = Some(value);
                    height = words.first()?.parse::<f64>().ok()?;
                    words.remove(0);
                    if !words
                        .first()
                        .is_some_and(|s| matches!(s.as_str(), "high" | "height" | "tall"))
                    {
                        return None;
                    }
                    words.remove(0);
                } else {
                    height = value;
                    if words
                        .first()
                        .is_some_and(|s| matches!(s.as_str(), "high" | "height" | "tall"))
                    {
                        words.remove(0);
                    }
                }
                lip = Some((width, height));
            }
            _ => return None,
        }
    }
    let wall = wall?;
    if outer && lip.is_none() {
        return None;
    }
    ShellBox {
        size,
        wall,
        floor: floor.unwrap_or(wall),
        bottom,
        centred: false,
        lip: lip.map(|(w, h)| [w.unwrap_or(wall / 2.0), h]),
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
            return Err(format!("wall {} must be below {} (half length/width or height); reduce wall or increase the enclosure dimensions", self.wall, (self.size[0].min(self.size[1]) / 2.0).min(self.size[2])));
        }
        if self.floor >= self.size[2] {
            return Err(format!(
                "floor {} must be below height {}; reduce floor",
                self.floor, self.size[2]
            ));
        }
        if let Some([w, h]) = self.lip {
            positive_mm("lip.width", w)?;
            positive_mm("lip.height", h)?;
            if w >= self.wall {
                return Err(format!(
                    "lip.width {w} must be below wall {}; reduce lip.width",
                    self.wall
                ));
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
        if self.floor > t {
            // Overlap the shell rather than only touching the cavity faces:
            // converted inch dimensions can differ there by floating-point noise.
            let fz = oz + if self.bottom { z - self.floor } else { 0.0 };
            out.push(("solid.box",json!({"length":x,"width":y,"height":self.floor,"corner":[ox,oy,fz],"operation":"join"})));
        } else if self.floor < t {
            let low = self.floor.min(t);
            let dz = (self.floor - t).abs();
            let fz = if self.bottom {
                oz + z - low - dz
            } else {
                oz + low
            };
            out.push(("solid.box",json!({"length":x-2.0*t,"width":y-2.0*t,"height":dz,"corner":[ox+t,oy+t,fz],"operation":"cut"})));
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
