//! Complete, bounded creation grammar. Unknown words and leftover tokens decline.
use super::*;

struct Cursor<'a> {
    t: &'a [String],
    at: usize,
    sizes: Vec<(f64, Option<f64>)>,
}
impl Cursor<'_> {
    fn eat(&mut self, words: &[&str]) -> bool {
        if self
            .t
            .get(self.at)
            .is_some_and(|s| words.contains(&s.as_str()))
        {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn size(&mut self) -> Option<f64> {
        let n = slots::number(self.t.get(self.at)?)?;
        self.at += 1;
        let unit = self.t.get(self.at).and_then(|s| match s.as_str() {
            "mm" => Some(1.0),
            "cm" => Some(10.0),
            "in" | "inch" | "inches" => Some(25.4),
            _ => None,
        });
        self.at += usize::from(unit.is_some());
        self.sizes.push((n, unit));
        let v = n * unit.unwrap_or(1.0);
        (v.is_finite() && v > 0.0 && v <= MAX_MM).then_some(v)
    }
    fn count(&mut self) -> Option<usize> {
        let n = match self.t.get(self.at)?.as_str() {
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
            s => s.parse().ok()?,
        };
        self.at += 1;
        (2..=64).contains(&n).then_some(n)
    }
    fn commas(&mut self) {
        while self.eat(&["and"]) {}
    }
}

pub(super) fn parse(i: &Input<'_>) -> Option<Value> {
    let mut c = Cursor {
        t: &i.t,
        at: 0,
        sizes: Vec::new(),
    };
    c.eat(&["hey"]);
    c.commas();
    c.eat(&["make", "build", "create"]);
    c.eat(&["a", "an"]);
    let first = c.size()?;
    let mut args = json!({"shape":"plate"});
    let (length, width) = if c.eat(&["round", "circular"]) {
        if !c.eat(&["base", "plate"]) {
            return None;
        }
        args["diameter"] = json!(first);
        let thickness = c.size()?;
        if !c.eat(&["thick"]) {
            return None;
        }
        args["thickness"] = json!(thickness);
        (first, first)
    } else {
        if !c.eat(&["x", "by"]) {
            return None;
        }
        c.size()?;
        let triplet = c.eat(&["x", "by"]);
        if triplet {
            c.size()?;
        }
        if !c.eat(&["plate", "base", "block"]) {
            return None;
        }
        c.commas();
        if !triplet {
            c.size()?;
        }
        c.eat(&["thick"]);
        // A trailing unit can qualify a whole homogeneous dimension triplet.
        let base = &c.sizes[..3];
        let scales: Vec<_> = base.iter().filter_map(|(_, u)| *u).collect();
        if base.iter().any(|(_, u)| u.is_none()) && scales.windows(2).any(|w| w[0] != w[1]) {
            return None;
        }
        let scale = scales.first().copied().unwrap_or(1.0);
        let values: Vec<_> = base.iter().map(|(n, u)| n * u.unwrap_or(scale)).collect();
        args["length"] = json!(values[0]);
        args["width"] = json!(values[1]);
        args["thickness"] = json!(values[2]);
        (values[0], values[1])
    };
    c.commas();
    if !c.eat(&["with"]) {
        return None;
    }
    let grid;
    let copies;
    let count;
    let mut columns = 0;
    let mut rows = 0;
    let article = c.eat(&["a"]);
    if c.eat(&["row"]) {
        if !c.eat(&["of"]) {
            return None;
        }
        count = c.count()?;
        grid = false;
        copies = false;
    } else if article {
        // Singular seed plus copies/instances means total instances in a new part.
        let save = c.at;
        let maybe = c.count();
        if maybe.is_some() && c.eat(&["x", "by"]) {
            columns = maybe?;
            rows = c.count()?;
            if !c.eat(&["grid"]) || !c.eat(&["of"]) {
                return None;
            }
            count = columns.checked_mul(rows)?;
            grid = true;
            copies = false;
        } else {
            c.at = save;
            count = 0;
            grid = false;
            copies = true;
        }
    } else {
        count = c.count()?;
        grid = false;
        copies = false;
    }
    if count > 64 {
        return None;
    }
    let diameter = c.size()?;
    if !c.eat(&[
        "stud", "studs", "peg", "pegs", "pin", "pins", "leg", "legs", "post", "posts", "boss",
        "bosses",
    ]) {
        return None;
    }
    c.commas();
    let height = c.size()?;
    if !c.eat(&["tall", "high", "long"]) {
        return None;
    }
    args["boss_diameter"] = json!(diameter);
    args["boss_height"] = json!(height);
    c.commas();
    if c.eat(&["on"]) {
        if copies || grid {
            return None;
        }
        c.eat(&["a"]);
        let circle = c.size()?;
        if !c.eat(&["circle"]) {
            return None;
        }
        args["bolt_circle"] = json!({"count":count,"diameter":circle});
    } else {
        let count = if copies {
            let n = c.count()?;
            if !c.eat(&["copies", "instances"]) {
                return None;
            }
            n
        } else {
            count
        };
        c.eat(&["spaced"]);
        let pitch = c.size()?;
        c.eat(&["apart"]);
        if grid {
            let (cols, rs) = if length >= width {
                (columns, rows)
            } else {
                (rows, columns)
            };
            args["grid"] = json!({"columns":cols,"rows":rs,"pitch_x":pitch,"pitch_y":pitch});
        } else {
            let mut axis = "X";
            let mut offset = [0.0; 2];
            if c.eat(&["along"]) {
                c.eat(&["the"]);
                if c.eat(&["x"]) {
                } else if c.eat(&["y"]) {
                    axis = "Y";
                } else {
                    let front = c.eat(&["front"]);
                    let back = !front && c.eat(&["back"]);
                    if !front && !back {
                        return None;
                    }
                    if c.eat(&["long"]) && length <= width {
                        return None;
                    }
                    if !c.eat(&["edges"]) {
                        return None;
                    }
                    let inset = c.size()?;
                    if !c.eat(&["inset"]) || inset >= width / 2.0 {
                        return None;
                    }
                    offset[1] = (width / 2.0 - inset) * if front { -1.0 } else { 1.0 };
                }
            }
            args["row"] = json!({"count":count,"pitch":pitch,"axis":axis,"offset_x":offset[0],"offset_y":offset[1]});
        }
    }
    c.commas();
    if c.at != c.t.len() {
        return None;
    }
    // Across sizes, changing units requires every size to carry its own unit.
    let units: Vec<_> = c.sizes.iter().filter_map(|(_, u)| *u).collect();
    // Only a rectangular dimension group shares a trailing unit. A round
    // diameter and thickness are separate sizes and must state non-mm units.
    let shared_end = if args.get("diameter").is_some() { 0 } else { 3 };
    if units.iter().any(|u| *u != 1.0) && c.sizes[shared_end..].iter().any(|(_, u)| u.is_none()) {
        return None;
    }
    if units.windows(2).any(|w| w[0] != w[1]) && c.sizes.iter().any(|(_, u)| u.is_none()) {
        return None;
    }
    Some(args)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn development_legacy_routing_is_preserved() {
        let catalog = Catalog::bundled().unwrap();
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/dev_repeated_boss.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert!(
                super::super::select_existing(
                    case["prompt"].as_str().unwrap(),
                    None,
                    &[],
                    &catalog
                )
                .unwrap()
                .is_none(),
                "already exact: {}",
                case["id"]
            );
        }
        for text in [
            "make a 20 mm cube",
            "make a cylinder 30 mm diameter and 50 mm tall",
            "make a plate 80x80x8mm centred on the origin with a centred 10mm bore through and six 4mm holes evenly spaced on a 56mm bolt circle then round the four vertical corner edges radius 5mm",
        ] {
            let old = super::super::select_existing(text, None, &[], &catalog).unwrap().unwrap();
            let current = super::super::select(text, None, &[], &catalog).unwrap().unwrap();
            assert_eq!(current.calls, old.calls, "{text}");
            assert_eq!(current.rule, old.rule, "{text}");
        }
    }
}
