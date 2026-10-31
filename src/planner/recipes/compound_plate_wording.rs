//! Additional complete plate grammars. Normalize into the original parser so
//! validation, layout arithmetic and emitted calls have a single implementation.
use super::*;

struct Words<'a> {
    t: Vec<&'a str>,
    p: usize,
}
impl Words<'_> {
    fn peek(&self) -> &str {
        self.t.get(self.p).copied().unwrap_or("")
    }
    fn eat(&mut self, word: &str) -> bool {
        if self.peek() == word {
            self.p += 1;
            true
        } else {
            false
        }
    }
    fn require(&mut self, word: &str) -> Option<()> {
        self.eat(word).then_some(())
    }
    fn raw_length(&mut self) -> Option<(f64, Option<f64>)> {
        let value = slots::number(self.peek())?;
        self.p += 1;
        let scale = match self.peek() {
            "mm" => Some(1.0),
            "cm" => Some(10.0),
            "in" | "inch" | "inches" => Some(25.4),
            _ => None,
        };
        self.p += usize::from(scale.is_some());
        (value.is_finite() && value > 0.0).then_some((value, scale))
    }
    fn length(&mut self) -> Option<f64> {
        let (value, scale) = self.raw_length()?;
        let value = value * scale.unwrap_or(1.0);
        (value <= MAX_MM).then_some(value)
    }
    fn count(&mut self) -> Option<usize> {
        let value = self.peek().parse().ok()?;
        if !compound_plate::bounded_count(value) {
            return None;
        }
        self.p += 1;
        Some(value)
    }
    fn times(&mut self) -> bool {
        self.eat("x") || self.eat("by")
    }
}

// Shared trailing units are permitted, but a mixed-unit dimension group must
// label every size. In particular, `18 cm x 120 x 10 mm` is not guessed.
fn sizes(values: &[(f64, Option<f64>)]) -> Option<Vec<f64>> {
    let scales: Vec<_> = values.iter().filter_map(|(_, s)| *s).collect();
    let scale = scales.first().copied().unwrap_or(1.0);
    if scales.iter().any(|s| *s != scale) && scales.len() != values.len() {
        return None;
    }
    let out: Vec<_> = values.iter().map(|(v, s)| v * s.unwrap_or(scale)).collect();
    out.iter().all(|v| *v <= MAX_MM).then_some(out)
}

pub(super) fn normalize(i: &Input<'_>) -> Option<Vec<String>> {
    if !compound_plate::candidate(i) || i.t.len() > 256 {
        return None;
    }
    // These are grammatical connectors only; every semantic word and every
    // number must subsequently belong to exactly one production.
    let mut w = Words {
        t: i.t
            .iter()
            .map(String::as_str)
            .filter(|s| !matches!(*s, "a" | "an" | "the" | "and" | "with" | "of" | "on" | "at"))
            .collect(),
        p: 0,
    };
    if matches!(w.peek(), "make" | "build" | "create") {
        w.p += 1;
    }
    if matches!(w.peek(), "rectangular" | "mounting") {
        w.p += 1;
    }
    let before = w.eat("plate");
    let a = w.raw_length()?;
    let square = w.eat("square");
    let mut raw = vec![a];
    if !square {
        if !w.times() {
            return None;
        }
        raw.push(w.raw_length()?);
        if w.times() {
            raw.push(w.raw_length()?);
        }
    }
    if !before {
        w.require("plate")?;
    }
    let d = sizes(&raw)?;
    let (x, y, z) = if square || d.len() == 2 {
        let (z, unit) = w.raw_length()?;
        if unit.is_none() && raw.iter().any(|(_, s)| s.is_some_and(|s| s != 1.0)) {
            return None;
        }
        let z = z * unit.unwrap_or(1.0);
        if z > MAX_MM {
            return None;
        }
        w.require("thick")?;
        (d[0], if square { d[0] } else { d[1] }, z)
    } else {
        w.eat("thick");
        (d[0], d[1], d[2])
    };
    let mut plane = None;
    let mut placement = None;
    let mut face = false;
    let mut edge = None;
    let mut layout = None;
    while !w.peek().is_empty() {
        if w.eat("in") && !matches!(w.peek(), "xy" | "xz" | "yz" | "front") {
            return None;
        }
        match w.peek() {
            "xy" | "xz" | "yz" | "front" => {
                let name = w.peek().to_owned();
                w.p += 1;
                if name == "front" && w.eat("face") {
                    if face {
                        return None;
                    }
                    face = true;
                } else {
                    w.require("plane")?;
                    if plane.is_some() {
                        return None;
                    }
                    plane = Some(if name == "front" { "xz".into() } else { name });
                }
            }
            "centred" | "centered" => {
                w.p += 1;
                w.require("origin")?;
                if placement.replace(true).is_some() {
                    return None;
                }
            }
            "one" | "corner" => {
                w.eat("one");
                w.require("corner")?;
                w.require("origin")?;
                if placement.replace(false).is_some() {
                    return None;
                }
            }
            _ => {
                let start = w.p;
                if edge.is_none() {
                    if let Some(e) = chamfer(&mut w) {
                        edge = Some(e);
                        continue;
                    }
                    w.p = start;
                }
                if layout.is_some() {
                    return None;
                }
                layout = Some(pattern(&mut w)?);
            }
        }
    }
    let layout = layout?;
    let placement = if placement.unwrap_or(false) {
        "centred at origin"
    } else {
        "corner at origin"
    };
    let plane = plane.unwrap_or("xy".into());
    let edge = edge.unwrap_or_default();
    Some(slots::tokens(&format!(
        "plate {x} x {y} x {z} mm {placement} in {plane} plane with {layout} {edge}"
    )))
}

fn chamfer(w: &mut Words<'_>) -> Option<String> {
    let amount;
    if w.eat("chamfer") {
        w.require("edges")?;
        amount = w.length()?;
    } else {
        amount = w.length()?;
        w.require("chamfer")?;
        w.require("edges")?;
    }
    // The frozen form defaults an unqualified corner treatment to vertical.
    Some(format!("chamfer four vertical edges {amount} mm"))
}

fn pattern(w: &mut Words<'_>) -> Option<String> {
    let grid_first = w.eat("grid");
    let a = w.count()?;
    if w.times() {
        let b = w.count()?;
        if a * b > 64 {
            return None;
        }
        if !grid_first {
            w.require("grid")?;
        }
        let diameter = if w.eat("holes") {
            let d = w.length()?;
            w.eat("diameter");
            d
        } else {
            let d = w.length()?;
            w.eat("diameter");
            w.require("holes")?;
            d
        };
        let pitch_first = w.eat("pitch");
        let pitch = w.length()?;
        if !pitch_first && !w.eat("pitch") {
            w.require("apart")?;
        }
        Some(format!(
            "centred grid {a} x {b} holes {diameter} mm diameter pitch {pitch} mm"
        ))
    } else {
        if grid_first {
            return None;
        }
        if w.eat("evenly") || w.eat("equally") {
            w.require("spaced")?;
        }
        w.require("holes")?;
        let diameter = w.length()?;
        w.eat("diameter");
        let circle = if w.eat("bolt") {
            w.require("circle")?;
            w.length()?
        } else {
            let value = w.length()?;
            w.require("bolt")?;
            w.require("circle")?;
            value
        };
        Some(format!(
            "{a} evenly spaced holes {diameter} mm diameter on bolt circle {circle} mm"
        ))
    }
}
