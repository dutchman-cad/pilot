//! Conservative, bounded closed orthogonal profiles, using only the public catalogue.
use super::*;

#[derive(Debug, serde::Serialize)]
pub(super) struct Profile {
    pub(super) shape: String,
    pub(super) plane: String,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) web: f64,
    pub(super) flange: f64,
    pub(super) inside_width: Option<f64>,
    pub(super) inside_height: Option<f64>,
    pub(super) stem_height: Option<f64>,
    pub(super) lip: Option<f64>,
    pub(super) inside_fillet: Option<f64>,
    pub(super) symmetric: bool,
    pub(super) offset: Option<f64>,
    pub(super) separate: bool,
    pub(super) centred: bool,
    pub(super) flipped: bool,
    pub(super) distance: f64,
}

pub(super) fn candidate(i: &Input<'_>) -> bool {
    i.any(&[
        "l", "angle", "bracket", "t", "tee", "rail", "u", "channel", "c",
    ]) || (i.any(&["rectangle", "rectangular", "plate", "block"])
        && i.any(&[
            "constrained",
            "dimensioned",
            "dimensions",
            "constrain",
            "defined",
        ]))
}

pub(super) fn vocabulary(i: &Input<'_>) -> bool {
    i.t.iter().all(|s| {
        s.parse::<f64>().is_ok()
            || s.split_once('/')
                .is_some_and(|(a, b)| a.parse::<f64>().is_ok() && b.parse::<f64>().is_ok())
            || slots::scale(s).is_some()
            || matches!(
                s.as_str(),
                "on" | "in"
                    | "xy"
                    | "xz"
                    | "yz"
                    | "plane"
                    | "front"
                    | "top"
                    | "right"
                    | "sketch"
                    | "draw"
                    | "make"
                    | "create"
                    | "build"
                    | "add"
                    | "a"
                    | "an"
                    | "l"
                    | "t"
                    | "u"
                    | "c"
                    | "tee"
                    | "rail"
                    | "channel"
                    | "section"
                    | "cross"
                    | "rectangle"
                    | "rectangular"
                    | "plate"
                    | "block"
                    | "-"
                    | "shaped"
                    | "angle"
                    | "profile"
                    | "closed"
                    | "bracket"
                    | "with"
                    | "its"
                    | "outside"
                    | "outer"
                    | "bottom"
                    | "left"
                    | "corner"
                    | "at"
                    | "the"
                    | "origin"
                    | "width"
                    | "wide"
                    | "height"
                    | "tall"
                    | "high"
                    | "overall"
                    | "both"
                    | "legs"
                    | "leg"
                    | "thick"
                    | "thickness"
                    | "thicknesses"
                    | "horizontal"
                    | "vertical"
                    | "upright"
                    | "and"
                    | "then"
                    | "extrude"
                    | "pad"
                    | "fully"
                    | "defined"
                    | "along"
                    | "constrain"
                    | "constrained"
                    | "dimensioned"
                    | "it"
                    | "four"
                    | "five"
                    | "two"
                    | "driving"
                    | "dimensions"
                    | "dimension"
                    | "of"
                    | "is"
                    | "are"
                    | "x"
                    | "by"
                    | "deep"
                    | "depth"
                    | "long"
                    | "length"
                    | "distance"
                    | "flange"
                    | "flanges"
                    | "cap"
                    | "stem"
                    | "web"
                    | "wall"
                    | "walls"
                    | "equal"
                    | "uniform"
                    | "centred"
                    | "centered"
                    | "centre"
                    | "center"
                    | "under"
                    | "offset"
                    | "from"
                    | "opening"
                    | "opens"
                    | "open"
                    | "faces"
                    | "facing"
                    | "up"
                    | "upward"
                    | "positive"
                    | "negative"
                    | "down"
                    | "downward"
                    | "y"
            )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Width,
    Height,
    Web,
    Flange,
    Thickness,
    Offset,
    Depth,
}
fn role(words: &[String], shape: &str) -> Option<Role> {
    let has = |xs: &[&str]| words.iter().any(|s| xs.contains(&s.as_str()));
    if has(&["offset"]) {
        return Some(Role::Offset);
    }
    if has(&["width", "wide"]) {
        return Some(if has(&["stem", "web"]) {
            Role::Web
        } else {
            Role::Width
        });
    }
    if has(&["height", "high", "tall"]) {
        return Some(Role::Height);
    }
    if has(&[
        "depth", "deep", "long", "length", "distance", "extrude", "pad",
    ]) {
        return Some(Role::Depth);
    }
    if has(&["stem", "web", "wall", "walls", "vertical", "upright"]) {
        return Some(Role::Web);
    }
    if has(&[
        "flange",
        "flanges",
        "cap",
        "horizontal",
        "bottom",
        "leg",
        "legs",
        "top",
    ]) {
        return Some(if shape == "L" && !has(&["horizontal", "bottom"]) {
            Role::Thickness
        } else {
            Role::Flange
        });
    }
    has(&["thick", "thickness", "thicknesses"]).then_some(Role::Thickness)
}
fn label_word(s: &str) -> bool {
    matches!(
        s,
        "width"
            | "wide"
            | "height"
            | "high"
            | "tall"
            | "depth"
            | "deep"
            | "long"
            | "length"
            | "distance"
            | "extrude"
            | "pad"
            | "it"
            | "overall"
            | "of"
            | "is"
            | "are"
            | "thick"
            | "thickness"
            | "thicknesses"
            | "stem"
            | "web"
            | "wall"
            | "walls"
            | "vertical"
            | "upright"
            | "flange"
            | "flanges"
            | "cap"
            | "horizontal"
            | "bottom"
            | "leg"
            | "legs"
            | "top"
            | "both"
            | "offset"
    )
}

pub(super) fn parse(i: &Input<'_>) -> Option<Slots> {
    if !candidate(i) || !vocabulary(i) {
        return None;
    }
    let mut shapes = Vec::new();
    for (name, words) in [
        ("L", &["l", "angle", "bracket"][..]),
        ("T", &["t", "tee", "rail"]),
        ("U", &["u"]),
        ("C", &["c"]),
        ("R", &["rectangle", "rectangular", "plate", "block"]),
    ] {
        if i.any(words) {
            shapes.push(name);
        }
    }
    if shapes.is_empty() && i.has("channel") {
        shapes.push("U");
    }
    let [shape] = shapes.as_slice() else {
        return None;
    };
    let mut shape = *shape;
    if shape == "R"
        && !i.any(&[
            "constrained",
            "dimensioned",
            "dimensions",
            "constrain",
            "defined",
        ])
    {
        return None;
    }
    let stem_centred = shape == "T" && i.has("stem") && i.has("under") && i.has("flange");
    if !i.has("origin")
        && (i.any(&["at", "corner"]) || (i.any(&["centred", "centered"]) && !stem_centred))
    {
        return None;
    }
    let centred = i.t.iter().enumerate().any(|(n, s)| {
        matches!(s.as_str(), "centred" | "centered")
            && i.t.get(n + 1).is_some_and(|s| s == "on" || s == "at")
            && (i.t.get(n + 2).is_some_and(|s| s == "origin")
                || (i.t.get(n + 2).is_some_and(|s| s == "the")
                    && i.t.get(n + 3).is_some_and(|s| s == "origin")))
    });
    if centred && shape == "L" {
        return None;
    }
    // Every plane designation must agree and an unknown named plane is refused.
    let mut planes = Vec::new();
    for (n, s) in i.t.iter().enumerate() {
        let p = match s.as_str() {
            "xy" => Some("XY"),
            "xz" => Some("XZ"),
            "yz" => Some("YZ"),
            "front" if i.t.get(n + 1).is_some_and(|s| s == "plane") => Some("XZ"),
            "right" if i.t.get(n + 1).is_some_and(|s| s == "plane") => Some("YZ"),
            "top" if i.t.get(n + 1).is_some_and(|s| s == "plane") => Some("XY"),
            _ => None,
        };
        if let Some(p) = p {
            planes.push(p);
        }
        if s == "on"
            || (s == "in"
                && i.t
                    .get(n + 1)
                    .is_some_and(|s| s == "the" || s.ends_with('z') || s == "xy"))
        {
            let n = n + 1 + usize::from(i.t.get(n + 1).is_some_and(|s| s == "the"));
            if !i.t.get(n).is_some_and(|s| {
                matches!(
                    s.as_str(),
                    "xy" | "xz" | "yz" | "front" | "top" | "right" | "origin"
                )
            }) {
                return None;
            }
        }
    }
    if planes.len() > 1 {
        return None;
    }
    let plane = planes.first().copied().unwrap_or("XY").to_string();
    let mut flipped = false;
    if let Some(n) = i.t.iter().position(|s| {
        matches!(
            s.as_str(),
            "opening" | "opens" | "open" | "faces" | "facing"
        )
    }) {
        if !matches!(shape, "U" | "C") {
            return None;
        }
        let mut tail = &i.t[n + 1..];
        while tail
            .first()
            .is_some_and(|s| matches!(s.as_str(), "faces" | "facing" | "is" | "the" | "positive"))
        {
            tail = &tail[1..];
        }
        let negative = tail
            .first()
            .is_some_and(|s| matches!(s.as_str(), "-" | "negative"));
        if negative {
            tail = &tail[1..];
        }
        let direction = tail.first()?.as_str();
        let rest = tail[1..]
            .strip_prefix(&["and".into()])
            .unwrap_or(&tail[1..]);
        if rest
            .first()
            .is_some_and(|s| matches!(s.as_str(), "up" | "down" | "left" | "right" | "x" | "y"))
            && rest.get(1).is_none_or(|s| s != "plane")
        {
            return None;
        }
        match direction {
            "up" | "upward" | "y" if !negative => shape = "U",
            "down" | "downward" => {
                shape = "U";
                flipped = true;
            }
            "y" if negative => {
                shape = "U";
                flipped = true;
            }
            "right" | "x" if !negative => shape = "C",
            "left" => {
                shape = "C";
                flipped = true;
            }
            "x" if negative => {
                shape = "C";
                flipped = true;
            }
            _ => return None,
        }
    }
    let d = dimensions(&i.t)?;
    if d.len() < 3 || d.len() > 7 {
        return None;
    }
    let mut assigned = Vec::new();
    let mut consumed = 0;
    let between = &i.t[d[0].index + 1..d[1].index];
    let tuple = between.iter().any(|s| s == "x" || s == "by")
        && between
            .iter()
            .all(|s| s == "x" || s == "by" || slots::scale(s).is_some());
    for (n, v) in d.iter().enumerate() {
        if tuple && n < 2 {
            assigned.push((if n == 0 { Role::Width } else { Role::Height }, v.value));
            continue;
        }
        let mut start = v.index;
        while start > consumed && label_word(&i.t[start - 1]) {
            start -= 1;
        }
        let mut end = v.index + 1;
        if i.t.get(end).is_some_and(|s| slots::scale(s).is_some()) {
            end += 1;
        }
        let after = end;
        // A span along the bottom is the overall width, rather than the
        // bottom leg's thickness. Consume only this explicit span phrase.
        let bottom_span = i.t.get(after).is_some_and(|s| s == "along")
            && (i.t.get(after + 1).is_some_and(|s| s == "bottom")
                || (i.t.get(after + 1).is_some_and(|s| s == "the")
                    && i.t.get(after + 2).is_some_and(|s| s == "bottom")));
        if bottom_span {
            end += if i.t[after + 1] == "the" { 3 } else { 2 };
        }
        while !bottom_span && end < i.t.len() && label_word(&i.t[end]) {
            end += 1;
        }
        // A suffix size label ends its phrase before the next size label.
        if let Some(first) = i.t.get(after) {
            if matches!(
                first.as_str(),
                "wide" | "width" | "height" | "tall" | "high" | "deep" | "long"
            ) {
                end = after + 1;
            }
        }
        let prefix = role(&i.t[start..v.index], shape);
        let suffix = if bottom_span {
            Some(Role::Width)
        } else {
            role(&i.t[after..end], shape)
        };
        let r = match (prefix, suffix) {
            (Some(Role::Flange), Some(Role::Width))
                if i.t.get(after).is_some_and(|s| s == "wide")
                    || d.get(n + 1).is_none_or(|v| v.index != end) =>
            {
                Some(Role::Width)
            }
            (Some(a @ (Role::Web | Role::Flange)), Some(Role::Thickness))
            | (Some(Role::Thickness), Some(a @ (Role::Web | Role::Flange))) => Some(a),
            (Some(a), Some(b)) if a != b => {
                // In label-first lists the next label belongs to the next number.
                if d.get(n + 1).is_some_and(|v| v.index == end) {
                    Some(a)
                } else {
                    return None;
                }
            }
            (Some(a), _) | (_, Some(a)) => Some(a),
            _ => None,
        };
        if suffix.is_some()
            && (r == suffix || suffix == Some(Role::Thickness))
            && !(prefix.is_some()
                && r == prefix
                && prefix != suffix
                && d.get(n + 1).is_some_and(|v| v.index == end))
        {
            consumed = end;
        }
        assigned.push((r?, v.value));
    }
    if assigned
        .iter()
        .any(|(r, v)| *r != Role::Offset && *v <= 0.0)
    {
        return None;
    }
    let get = |r| {
        let vs: Vec<_> = assigned.iter().filter(|(a, _)| *a == r).collect();
        if let [v] = vs.as_slice() {
            Some(v.1)
        } else {
            None
        }
    };
    // No number may have a duplicate role, even if the values happen to agree.
    for (n, (r, _)) in assigned.iter().enumerate() {
        if assigned[..n].iter().any(|(a, _)| a == r) {
            return None;
        }
    }
    let width = get(Role::Width)?;
    let height = get(Role::Height)?;
    let distance = get(Role::Depth)?;
    let common = get(Role::Thickness);
    let (web, flange) = if shape == "R" {
        (0.0, 0.0)
    } else if shape == "T" {
        if common.is_some() {
            return None;
        }
        (get(Role::Web)?, get(Role::Flange)?)
    } else {
        match (get(Role::Web), get(Role::Flange), common) {
            (None, None, Some(t)) => (t, t),
            (Some(w), Some(f), None) => (w, f),
            (Some(t), None, None) | (None, Some(t), None) if shape != "L" => (t, t),
            _ => return None,
        }
    };
    if shape == "R" && assigned.len() != 3 {
        return None;
    }
    let offset = get(Role::Offset);
    if i.has("offset") != offset.is_some() || (offset.is_some() && shape != "T") {
        return None;
    }
    let offset = if let Some(v) = offset {
        let n = assigned.iter().position(|(r, _)| *r == Role::Offset)?;
        let mut tail = &i.t[d[n].index + 1..];
        if tail.first().is_some_and(|s| slots::scale(s).is_some()) {
            tail = &tail[1..];
        }
        tail = tail.strip_prefix(&["from".into()])?;
        if tail.first().is_some_and(|s| s == "the") {
            tail = &tail[1..];
        }
        match tail.first()?.as_str() {
            "left" => Some(v),
            "centre" | "center" => Some(width / 2.0 + v),
            _ => return None,
        }
    } else {
        None
    };
    Profile {
        shape: shape.into(),
        plane,
        width,
        height,
        web,
        flange,
        inside_width: None,
        inside_height: None,
        stem_height: None,
        lip: None,
        inside_fillet: None,
        symmetric: false,
        offset,
        separate: web != flange && get(Role::Web).is_some() && get(Role::Flange).is_some(),
        centred,
        flipped,
        distance,
    }
    .into_slots()
    .ok()
}

impl Profile {
    pub(super) fn into_slots(self) -> Result<Slots, String> {
        valid_plane(&self.plane)?;
        for (field, value) in [
            ("width", self.width),
            ("height", self.height),
            ("depth", self.distance),
        ] {
            positive_mm(field, value)?;
        }
        if self.shape != "R" {
            positive_mm("web_thickness", self.web)?;
            positive_mm("flange_thickness", self.flange)?;
            let (web_limit, flange_limit) = match self.shape.as_str() {
                "L" | "T" => (self.width, self.height),
                "U" => (self.height, self.width / 2.0),
                "C" => (self.width, self.height / 2.0),
                _ => return Err("kind: must be L, T, U, C or rectangle".into()),
            };
            if self.web >= web_limit {
                return Err(format!("web_thickness {} must be below {web_limit} (cut side); reduce thickness or web_thickness", self.web));
            }
            if self.flange >= flange_limit {
                return Err(format!("flange_thickness {} must be below {flange_limit} (open interior limit); reduce thickness or flange_thickness", self.flange));
            }
        }
        if let Some(x) = self.offset {
            if self.shape != "T"
                || !x.is_finite()
                || x - self.web / 2.0 <= 0.0
                || x + self.web / 2.0 >= self.width
            {
                return Err(format!("stem_offset {x} requires kind T and must be above {} and below {}; move the stem centre inside the width or omit stem_offset", self.web / 2.0, self.width - self.web / 2.0));
            }
        }
        if let Some(lip) = self.lip {
            positive_mm("lip", lip)?;
            if self.shape != "C"
                || lip <= self.flange
                || 2.0 * lip >= self.height
                || self.width <= 2.0 * self.web
            {
                return Err("lip: requires C, longer than flange thickness, below half height, and room beyond the web".into());
            }
        }
        if let Some(radius) = self.inside_fillet {
            positive_mm("inside_fillet", radius)?;
            if self.shape != "L" || radius >= (self.width - self.web).min(self.height - self.flange)
            {
                return Err(
                    "inside_fillet: requires L and radius below both inside leg lengths".into(),
                );
            }
        }
        if self.shape == "L" && !self.symmetric && self.inside_fillet.is_none() {
            return Ok(Slots::LProfile {
                plane: self.plane,
                width: self.width,
                height: self.height,
                horizontal: self.flange,
                vertical: self.web,
                distance: self.distance,
            });
        }
        Ok(Slots::ClosedProfile(self))
    }
    pub(super) fn area(&self) -> f64 {
        let Self {
            width: w,
            height: h,
            web: b,
            flange: f,
            ..
        } = *self;
        match self.shape.as_str() {
            "T" => w * f + (h - f) * b,
            "U" => w * b + 2.0 * f * (h - b),
            "C" => h * b + 2.0 * f * (w - b) + self.lip.map_or(0.0, |lip| 2.0 * b * (lip - f)),
            "L" => {
                w * f
                    + (h - f) * b
                    + self
                        .inside_fillet
                        .map_or(0.0, |r| r * r * (1.0 - std::f64::consts::PI / 4.0))
            }
            _ => w * h,
        }
    }
    pub(super) fn commands(&self) -> Vec<(&'static str, Value)> {
        let Self {
            width: w,
            height: h,
            web: b,
            flange: f,
            ..
        } = *self;
        let center = self.offset.unwrap_or(w / 2.0);
        let (l, r) = (center - b / 2.0, center + b / 2.0);
        let lip = self.lip.unwrap_or(0.0);
        let mut points = match self.shape.as_str() {
            "T" => vec![
                [0.0, h],
                [w, h],
                [w, h - f],
                [r, h - f],
                [r, 0.0],
                [l, 0.0],
                [l, h - f],
                [0.0, h - f],
            ],
            "U" => vec![
                [0.0, 0.0],
                [w, 0.0],
                [w, h],
                [w - f, h],
                [w - f, b],
                [f, b],
                [f, h],
                [0.0, h],
            ],
            "L" => vec![[0.0, 0.0], [w, 0.0], [w, f], [b, f], [b, h], [0.0, h]],
            "C" if self.lip.is_some() => vec![
                [0.0, 0.0],
                [w, 0.0],
                [w, lip],
                [w - b, lip],
                [w - b, f],
                [b, f],
                [b, h - f],
                [w - b, h - f],
                [w - b, h - lip],
                [w, h - lip],
                [w, h],
                [0.0, h],
            ],
            "C" => vec![
                [0.0, 0.0],
                [w, 0.0],
                [w, f],
                [b, f],
                [b, h - f],
                [w, h - f],
                [w, h],
                [0.0, h],
            ],
            _ => vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]],
        };
        if self.flipped {
            for p in &mut points {
                if self.shape == "U" {
                    p[1] = h - p[1];
                } else {
                    p[0] = w - p[0];
                }
            }
        }
        if self.centred {
            for p in &mut points {
                p[0] -= w / 2.0;
                p[1] -= h / 2.0;
            }
        }
        let ids: Vec<_> = (1..=points.len()).map(|n| format!("p{n}")).collect();
        let mut out = vec![
            (
                "sketch.create",
                json!({"plane":self.plane,"name":"Closed profile"}),
            ),
            (
                "sketch.line",
                json!({"points":points,"closed":true,"infer":true,"ids":ids}),
            ),
        ];
        // Anchor the bounding box, without spending a driving size on placement.
        if self.centred {
            let (a, b) = match self.shape.as_str() {
                "T" => ("p1.start", "p5.start"),
                "U" => ("p1.start", "p2.end"),
                "C" => ("p1.start", "p6.end"),
                _ => ("p1.start", "p2.end"),
            };
            if self.shape == "T" {
                // Two named construction points anchor the bounding-box centre.
                // The stem may still slide until its centring/offset constraint.
                out.push((
                    "sketch.point",
                    json!({"point":[0,h/2.0],"id":"profile_top","connect":false}),
                ));
                out.push((
                    "sketch.point",
                    json!({"point":[0,-h/2.0],"id":"profile_bottom","connect":false}),
                ));
                out.push((
                    "sketch.constraint.midpoint",
                    json!({"point":"profile_top","line":"p1"}),
                ));
                out.push((
                    "sketch.constraint.horizontal_vertical",
                    json!({"points":["profile_top","origin"],"mode":"vertical"}),
                ));
                out.push((
                    "sketch.constraint.horizontal_vertical",
                    json!({"points":["profile_bottom","p5.start"],"mode":"horizontal"}),
                ));
                out.push((
                    "sketch.constraint.symmetry",
                    json!({"a":"profile_top","b":"profile_bottom","point":"origin"}),
                ));
            } else {
                out.push((
                    "sketch.constraint.symmetry",
                    json!({"a":a,"b":b,"point":"origin"}),
                ));
            }
        } else if self.shape == "T" {
            out.push((
                "sketch.constraint.coincident",
                json!({"a":"p1.start","b":"y"}),
            ));
            out.push((
                "sketch.constraint.coincident",
                json!({"a":"p5.start","b":"x"}),
            ));
        }
        let mut dims: Vec<(Value, f64, Option<&str>)> = vec![(json!(["p1"]), w, None)];
        if let Some(v) = self.inside_width {
            dims[0] = (
                if self.shape == "U" {
                    json!(["p5"])
                } else if self.lip.is_some() {
                    json!(["p6.start", "p2.start"])
                } else {
                    json!(["p3"])
                },
                v,
                Some("horizontal"),
            );
        }
        match self.shape.as_str() {
            "L" => dims.extend([
                (json!(["p6"]), h, None),
                (json!(["p2"]), f, None),
                (json!(["p5"]), b, None),
            ]),
            "C" if self.lip.is_some() => {
                out.extend([
                    ("sketch.constraint.collinear", json!({"a":"p2","b":"p10"})),
                    ("sketch.constraint.collinear", json!({"a":"p4","b":"p8"})),
                    ("sketch.constraint.equal", json!({"a":"p2","b":"p10"})),
                    ("sketch.constraint.equal", json!({"a":"p3","b":"p9"})),
                ]);
                dims.extend([
                    (
                        if self.inside_height.is_some() {
                            json!(["p6"])
                        } else {
                            json!(["p12"])
                        },
                        self.inside_height.unwrap_or(h),
                        None,
                    ),
                    (json!(["p1.start", "p5.end"]), b, Some("horizontal")),
                    (json!(["p1.start", "p5.start"]), f, Some("vertical")),
                    (json!(["p7.start", "p11.end"]), f, Some("vertical")),
                    (json!(["p3"]), b, None),
                    (json!(["p2"]), lip, None),
                ]);
            }
            "T" => {
                out.push(("sketch.constraint.collinear", json!({"a":"p3","b":"p7"})));
                dims.extend([
                    (
                        if self.stem_height.is_some() {
                            json!(["p4"])
                        } else {
                            json!(["p1.start", "p5.start"])
                        },
                        self.stem_height.unwrap_or(h),
                        Some("vertical"),
                    ),
                    (json!(["p2"]), f, None),
                    (json!(["p5"]), b, None),
                ]);
                if self.offset.is_some() {
                    let point = if self.centred {
                        [center - w / 2.0, -h / 2.0]
                    } else {
                        [center, 0.0]
                    };
                    out.push((
                        "sketch.point",
                        json!({"point":point,"id":"profile_stem","connect":false}),
                    ));
                    out.push((
                        "sketch.constraint.midpoint",
                        json!({"point":"profile_stem","line":"p5"}),
                    ));
                    dims.push((
                        json!(["p1.start", "profile_stem"]),
                        center,
                        Some("horizontal"),
                    ));
                } else {
                    out.push((
                        "sketch.constraint.horizontal_vertical",
                        json!({"points":[{"midpoint":"p1"},{"midpoint":"p5"}],"mode":"vertical"}),
                    ));
                }
            }
            "U" => {
                out.push(("sketch.constraint.collinear", json!({"a":"p3","b":"p7"})));
                dims.extend([
                    (
                        if self.inside_height.is_some() {
                            json!(["p4"])
                        } else {
                            json!(["p2"])
                        },
                        self.inside_height.unwrap_or(h),
                        None,
                    ),
                    (json!(["p1.start", "p4.end"]), b, Some("vertical")),
                    (json!(["p3"]), f, None),
                ]);
                if self.separate {
                    dims.push((json!(["p7"]), f, None));
                } else {
                    out.push(("sketch.constraint.equal", json!({"a":"p3","b":"p7"})));
                }
            }
            "C" => {
                out.push(("sketch.constraint.collinear", json!({"a":"p2","b":"p6"})));
                dims.extend([
                    (
                        if self.inside_height.is_some() {
                            json!(["p4"])
                        } else {
                            json!(["p8"])
                        },
                        self.inside_height.unwrap_or(h),
                        None,
                    ),
                    (json!(["p1.start", "p3.end"]), b, Some("horizontal")),
                    (json!(["p2"]), f, None),
                ]);
                if self.separate {
                    dims.push((json!(["p6"]), f, None));
                } else {
                    out.push(("sketch.constraint.equal", json!({"a":"p2","b":"p6"})));
                }
            }
            _ => dims.push((json!(["p2"]), h, None)),
        }
        for (entities, value, kind) in dims {
            let mut p = json!({"entities":entities,"value":value});
            if let Some(kind) = kind {
                p["type"] = json!(kind);
            }
            out.push(("sketch.dimension", p));
        }
        if let Some(radius) = self.inside_fillet {
            out.push(("sketch.fillet", json!({"point":"p3.end","radius":radius})));
        }
        out.push(("sketch.finish", json!({})));
        out.push(("solid.extrude", json!({"distance":self.distance,"direction":if self.symmetric {"symmetric"} else {"positive"}})));
        out
    }
}
