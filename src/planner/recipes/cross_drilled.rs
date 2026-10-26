//! World-space through holes, using only the public primitive API.
use super::*;

pub(crate) fn schema() -> Value {
    let mut schema = json!({"type":"object","additionalProperties":false,"x-pilot-expander":"cross_drilled_box",
        "$defs":{"Length":{"type":"number","exclusiveMinimum":0,"maximum":1000000},"Point3":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-1000000,"maximum":1000000}}},
        "properties":{"length":{"$ref":"#/$defs/Length"},"width":{"$ref":"#/$defs/Length"},"height":{"$ref":"#/$defs/Length"},"corner":{"$ref":"#/$defs/Point3"},"holes":{"type":"array","minItems":1,"maxItems":3,"items":{"type":"object","additionalProperties":false,"properties":{"axis":{"type":"string","enum":["X","Y","Z"]},"diameter":{"$ref":"#/$defs/Length"},"position":{"$ref":"#/$defs/Point3"}},"required":["axis","diameter"]}}},"required":["length","width","height","holes"]});
    crate::recipe_library::forms::describe(&mut schema);
    schema
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Hole {
    axis: String,
    diameter: f64,
    position: Option<[f64; 3]>,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoxHoles {
    length: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    corner: [f64; 3],
    holes: Vec<Hole>,
}
impl BoxHoles {
    fn size(&self) -> [f64; 3] {
        [self.length, self.width, self.height]
    }
    fn axis(h: &Hole) -> Option<usize> {
        match h.axis.as_str() {
            "X" => Some(0),
            "Y" => Some(1),
            "Z" => Some(2),
            _ => None,
        }
    }
    fn position(&self, h: &Hole, axis: usize) -> [f64; 3] {
        h.position.unwrap_or_else(|| {
            std::array::from_fn(|i| {
                self.corner[i] + self.size()[i] * if i == axis { 1.0 } else { 0.5 }
            })
        })
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        for n in self.size() {
            positive_mm("box size", n)?;
        }
        if self.holes.is_empty() || self.holes.len() > 3 {
            return Err("holes: require one to three distinct axes".into());
        }
        let mut seen = [false; 3];
        for i in 0..3 {
            if !self.corner[i].is_finite()
                || self.corner[i].abs() > MAX_MM
                || (self.corner[i] + self.size()[i]).abs() > MAX_MM
            {
                return Err(format!("corner[{i}] {} plus size {} must stay within -{MAX_MM}..{MAX_MM} mm; move corner closer to the origin or reduce size", self.corner[i], self.size()[i]));
            }
        }
        for (n, h) in self.holes.iter().enumerate() {
            let a = Self::axis(h).ok_or("hole axis must be X, Y or Z")?;
            if seen[a] {
                return Err(format!("holes[{n}].axis {:?} permits at most 1 hole per axis; remove the duplicate axis", h.axis));
            }
            seen[a] = true;
            positive_mm("hole diameter", h.diameter)?;
            let p = self.position(h, a);
            for (i, value) in p.iter().enumerate() {
                let lo = self.corner[i];
                let hi = lo + self.size()[i];
                let outside = if i == a {
                    (*value - lo).abs() > 1e-8 && (*value - hi).abs() > 1e-8
                } else {
                    *value - h.diameter / 2.0 <= lo || *value + h.diameter / 2.0 >= hi
                };
                if !value.is_finite() || value.abs() > MAX_MM || outside {
                    return Err(format!("holes[{n}].position[{i}] {value} with diameter {} must {} and stay within -{MAX_MM}..{MAX_MM}; omit position for a centred hole or move it inside the face", h.diameter, if i == a {format!("equal {lo} or {hi}")} else {format!("be above {} and below {}", lo + h.diameter / 2.0, hi - h.diameter / 2.0)}));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn commands(&self) -> Vec<(&'static str, Value)> {
        let mut out = vec![(
            "solid.box",
            json!({"length":self.length,"width":self.width,"height":self.height,"corner":self.corner,"operation":"new"}),
        )];
        for h in &self.holes {
            if let Some(a) = Self::axis(h) {
                let p = self.position(h, a);
                let mut d = [0.0; 3];
                d[a] = if (p[a] - self.corner[a]).abs() < 1e-8 {
                    1.0
                } else {
                    -1.0
                };
                out.push((
                    "solid.hole",
                    json!({"diameter":h.diameter,"position":p,"direction":d}),
                ));
            }
        }
        out
    }
    pub(crate) fn volume(&self) -> Option<f64> {
        let h = self.holes.first()?;
        if self
            .holes
            .iter()
            .any(|other| other.diameter != h.diameter || other.position.is_some())
        {
            return None;
        }
        let r = h.diameter / 2.0;
        let removed: f64 = self
            .holes
            .iter()
            .filter_map(Self::axis)
            .map(|a| PI * r * r * self.size()[a])
            .sum();
        // Inclusion/exclusion of equal, centred orthogonal cylinders (Steinmetz solids).
        let overlap = match self.holes.len() {
            2 => 16.0 * r.powi(3) / 3.0,
            3 => 8.0 * 2.0_f64.sqrt() * r.powi(3),
            _ => 0.0,
        };
        Some(self.size().iter().product::<f64>() - removed + overlap)
    }
}
pub(crate) fn build(args: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    crate::recipe_library::validate_structure(args, &schema())?;
    let b: BoxHoles =
        serde_json::from_value(args.clone()).map_err(|e| format!("cross-drilled box: {e}"))?;
    b.validate()?;
    let data = request["messages"].as_array().and_then(|m| state(m));
    from_slots(
        "cross_drilled_box",
        Slots::CrossDrilled(b),
        data.as_ref(),
        catalog,
    )?
    .ok_or("cross-drilled box exceeds design limits".into())
}
pub(super) fn candidate(i: &Input<'_>) -> bool {
    i.any(&["cube", "box", "block"])
        && i.any(&["hole", "holes", "drilled"])
        && (i.any(&["faces", "face", "axes", "axis", "drilled"])
            || (i.has("along") && i.any(&["x", "y", "z"])))
}
pub(super) fn parse(i: &Input<'_>) -> Option<BoxHoles> {
    // Exact, deliberately small grammar: unknown modifiers and unused numbers decline.
    if !i.t.iter().all(|w| {
        (w.parse::<f64>().is_ok()
            || w.split_once('/').is_some_and(|(a, b)| {
                a.parse::<f64>().is_ok() && b.parse::<f64>().is_ok_and(|n| n > 0.0)
            }))
            || slots::scale(w).is_some()
            || matches!(
                w.as_str(),
                "draw"
                    | "make"
                    | "create"
                    | "build"
                    | "a"
                    | "an"
                    | "the"
                    | "cube"
                    | "box"
                    | "block"
                    | "with"
                    | "hole"
                    | "holes"
                    | "through"
                    | "centre"
                    | "center"
                    | "centred"
                    | "centered"
                    | "of"
                    | "all"
                    | "six"
                    | "faces"
                    | "face"
                    | "each"
                    | "cross"
                    | "drilled"
                    | "-"
                    | "along"
                    | "x"
                    | "y"
                    | "z"
                    | "and"
                    | "on"
                    | "axis"
                    | "axes"
                    | "diameter"
                    | "by"
                    | "length"
                    | "width"
                    | "height"
                    | "long"
                    | "wide"
                    | "high"
            )
    }) {
        return None;
    }
    if !i.has("through") && !i.has("drilled") && !i.has("along") {
        return None;
    }
    let anchor = i.t.iter().position(|w| w == "with")?;
    let ds = dimensions(&i.t[..anchor])?;
    let size = box_size(&i.t[..anchor], &ds, i.has("cube"))?;
    if i.has("cube") && (size[0] != size[1] || size[0] != size[2]) {
        return None;
    }
    let tail: Vec<_> = i.t[anchor + 1..]
        .iter()
        .enumerate()
        .filter(|(n, s)| {
            !((s.as_str() == "six" || s.as_str() == "6")
                && i.t.get(anchor + 2 + n).is_some_and(|w| w == "faces"))
        })
        .map(|(_, s)| s.clone())
        .collect();
    let ds = dimensions(&tail)?;
    if ds.len() != 1 {
        return None;
    }
    let diameter = ds[0].value;
    let axes: Vec<_> = ["x", "y", "z"]
        .into_iter()
        .filter(|a| tail.iter().any(|w| w == a))
        .collect();
    if i.has("six") && !i.t.windows(2).any(|w| w[0] == "six" && w[1] == "faces") {
        return None;
    }
    if (i.has("faces") || i.has("each")) && !axes.is_empty() && axes.len() != 3 {
        return None;
    }
    let axes = if i.has("faces") || i.has("each") || i.has("drilled") && axes.is_empty() {
        vec!["x", "y", "z"]
    } else {
        axes
    };
    if axes.is_empty() {
        return None;
    }
    let b = BoxHoles {
        length: size[0],
        width: size[1],
        height: size[2],
        corner: [0.0; 3],
        holes: axes
            .into_iter()
            .map(|a| Hole {
                axis: a.to_ascii_uppercase(),
                diameter,
                position: None,
            })
            .collect(),
    };
    b.validate().ok()?;
    Some(b)
}
