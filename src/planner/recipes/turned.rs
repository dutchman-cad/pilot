//! Exact concentric turned parts. All grammar must be consumed; no request defaults.
use super::*;

const MAX_SECTIONS: usize = 8;

#[derive(Debug, serde::Serialize)]
pub(super) struct Section {
    radius: f64,
    height: f64,
}
#[derive(Debug, serde::Serialize)]
pub(super) struct Turned {
    outer: Vec<Section>,
    bore: Vec<Section>,
}
impl Turned {
    pub(super) fn volume(&self) -> f64 {
        let volume = |sections: &[Section]| {
            sections
                .iter()
                .map(|s| PI * s.radius.powi(2) * s.height)
                .sum::<f64>()
        };
        volume(&self.outer) - volume(&self.bore)
    }
    pub(super) fn commands(&self) -> Vec<(&'static str, Value)> {
        let mut calls = Vec::new();
        for (sections, cut) in [(&self.outer, false), (&self.bore, true)] {
            let mut z = 0.;
            for (n, s) in sections.iter().enumerate() {
                calls.push(("solid.cylinder", json!({"radius":s.radius,"height":s.height,"base":[0,0,z],"operation":if cut {"cut"} else if n == 0 {"new"} else {"join"}})));
                z += s.height;
            }
        }
        calls
    }
}

pub(super) fn candidate(i: &Input<'_>) -> bool {
    i.any(&[
        "ring",
        "sleeve",
        "collar",
        "bushing",
        "washer",
        "spacer",
        "stepped",
        "shouldered",
        "tube",
        "pipe",
    ])
}

/// A small cursor keeps labels local: every length needs its own explicit unit.
#[derive(Clone, Copy)]
struct Cursor<'a> {
    words: &'a [String],
    pos: usize,
}
impl Cursor<'_> {
    fn eat(&mut self, word: &str) -> bool {
        if self.words.get(self.pos).is_some_and(|w| w == word) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn one(&mut self, words: &[&str]) -> bool {
        words.iter().any(|w| self.eat(w))
    }
    fn end(&self) -> bool {
        self.pos == self.words.len()
    }
    fn length(&mut self) -> Option<f64> {
        let raw = self.words.get(self.pos)?;
        let mut n = slots::number(raw)?;
        self.pos += 1;
        if !raw.contains('/') && self.words.get(self.pos).is_some_and(|s| s.contains('/')) {
            let fraction = slots::number(self.words.get(self.pos)?)?;
            if n < 0. || n.fract() != 0. || fraction <= 0. || fraction >= 1. {
                return None;
            }
            n += fraction;
            self.pos += 1;
        }
        let unit = self.words.get(self.pos)?;
        if !matches!(unit.as_str(), "mm" | "cm" | "in" | "inch" | "inches") {
            return None;
        }
        n *= slots::scale(unit)?;
        self.pos += 1;
        (n.is_finite() && n > 0. && n <= MAX_MM).then_some(n)
    }
    fn separators(&mut self) {
        while self.one(&["and", "with"]) {}
    }
    fn label(&mut self) -> Option<Role> {
        if self.one(&["od"]) {
            Some(Role::Outer)
        } else if self.one(&["id", "bore"]) {
            self.eat("diameter");
            Some(Role::Inner)
        } else if self.eat("wall") {
            Some(Role::Wall)
        } else if self.one(&[
            "length",
            "long",
            "height",
            "high",
            "tall",
            "thick",
            "thickness",
        ]) {
            Some(Role::Length)
        } else {
            None
        }
    }
    fn field(&mut self) -> Option<(Role, f64)> {
        let start = self.pos;
        if let Some(role) = self.label() {
            self.eat("of");
            let n = self.length()?;
            if role == Role::Inner {
                self.eat("through");
            }
            return Some((role, n));
        }
        self.pos = start;
        let n = self.length()?;
        let role = self.label()?;
        if role == Role::Inner {
            self.eat("through");
        }
        Some((role, n))
    }
    fn diameter(&mut self) -> Option<f64> {
        if self.one(&["diameter", "dia"]) {
            self.eat("of");
            self.length()
        } else {
            let n = self.length()?;
            self.one(&["diameter", "dia"]).then_some(n)
        }
    }
    fn sections(&mut self) -> Option<Vec<Section>> {
        let mut sections = Vec::new();
        loop {
            let diameter = self.diameter()?;
            if !self.eat("for") {
                return None;
            }
            let height = self.length()?;
            self.eat("long");
            sections.push(Section {
                radius: diameter / 2.,
                height,
            });
            if sections.len() > MAX_SECTIONS {
                return None;
            }
            if sections.windows(2).any(|s| s[0].radius == s[1].radius) {
                return None;
            }
            let conjunction = self.eat("and");
            if !self.eat("then") {
                if conjunction {
                    return None;
                }
                break;
            }
        }
        let total: f64 = sections.iter().map(|s| s.height).sum();
        (sections.len() >= 2 && total <= MAX_MM).then_some(sections)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Outer,
    Inner,
    Wall,
    Length,
}

pub(super) fn parse(i: &Input<'_>) -> Option<Slots> {
    let mut c = Cursor {
        words: &i.t,
        pos: 0,
    };
    c.one(&["make", "create", "build", "add"]);
    c.one(&["a", "an"]);
    let stepped = c.one(&["stepped", "shouldered"]);
    if stepped && c.one(&["pin", "shaft", "rod"]) {
        c.separators();
        let outer = c.sections()?;
        if !c.end() {
            return None;
        }
        return Some(Slots::Turned(Turned {
            outer,
            bore: vec![],
        }));
    }
    let bore_shape = stepped && c.eat("bore");
    c.eat("thrust");
    if !c.one(&[
        "ring", "sleeve", "collar", "bushing", "washer", "spacer", "tube", "pipe",
    ]) && !bore_shape
    {
        return None;
    }
    let mut values = [None; 4];
    c.separators();
    loop {
        if c.eat("counterbore") {
            let diameter = c.diameter()?;
            let depth = c.length()?;
            if !c.eat("deep") || !c.eat("from") {
                return None;
            }
            c.eat("the");
            let top = c.eat("top");
            if !top && !c.eat("bottom") {
                return None;
            }
            if !c.end() || values[2].is_some() {
                return None;
            }
            let outer = values[0]?;
            let inner = values[1]?;
            let height = values[3]?;
            if diameter <= inner || diameter >= outer || depth >= height {
                return None;
            }
            let mut bore = vec![
                Section {
                    radius: diameter / 2.,
                    height: depth,
                },
                Section {
                    radius: inner / 2.,
                    height: height - depth,
                },
            ];
            if top {
                bore.reverse();
            }
            return Some(Slots::Turned(Turned {
                outer: vec![Section {
                    radius: outer / 2.,
                    height,
                }],
                bore,
            }));
        }
        // A stepped bore is a sequence of axial lengths, explicitly from one end.
        let bore_sections = {
            let mut probe = c;
            if probe.eat("bore") {
                probe.sections().inspect(|_| c = probe)
            } else {
                None
            }
        };
        if let Some(mut bore) = bore_sections {
            if !c.eat("from") {
                return None;
            }
            c.eat("the");
            if c.eat("top") {
                bore.reverse();
            } else if !c.eat("bottom") {
                return None;
            }
            if !c.end() || values[1].is_some() || values[2].is_some() {
                return None;
            }
            let diameter = values[0]?;
            let height: f64 = bore.iter().map(|s| s.height).sum();
            if values[3].is_some_and(|h: f64| (h - height).abs() > 1e-7)
                || bore.iter().any(|s| s.radius * 2. >= diameter)
            {
                return None;
            }
            return Some(Slots::Turned(Turned {
                outer: vec![Section {
                    radius: diameter / 2.,
                    height,
                }],
                bore,
            }));
        }
        let (role, n) = c.field()?;
        let index = match role {
            Role::Outer => 0,
            Role::Inner => 1,
            Role::Wall => 2,
            Role::Length => 3,
        };
        if values[index].replace(n).is_some() {
            return None;
        }
        if c.end() {
            break;
        }
        c.separators();
        if c.end() {
            return None;
        }
    }
    if stepped {
        return None;
    }
    let outer = values[0]?;
    let inner = match (values[1], values[2]) {
        (Some(n), None) => n,
        (None, Some(w)) => outer - 2. * w,
        _ => return None,
    };
    if inner <= 0. || inner >= outer {
        return None;
    }
    Some(Slots::Tube {
        outer_radius: outer / 2.,
        inner_radius: inner / 2.,
        height: values[3]?,
    })
}
