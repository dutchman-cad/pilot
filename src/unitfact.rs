//! Exact millimetre values for sizes a request states in inches or centimetres.
//!
//! Only a number written next to a unit counts: `1"`, `3/8 in`, `1-1/2"`, `1 1/2 inch`,
//! `2.5 cm`. Bare numbers, a fraction with no unit and mm sizes yield nothing, and no
//! size is ever inferred. The request text is never changed; the caller adds one fact.

/// Start of the fact; also the marker used to deduplicate it across a conversation.
pub const UNIT_MARKER: &str = "Sizes in mm:";
/// At most this many sizes are listed.
pub const MAX_SIZES: usize = 16;
const MAX_SCAN: usize = 8192;

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Inch,
    Cm,
}

struct Value {
    num: u128,
    den: u128,
    label: String,
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\u{201d}' | '\u{2033}')
}

/// Read up to `max` digits at `*i`; `None` when more digits follow.
fn read_digits(c: &[char], i: &mut usize, max: usize) -> Option<u128> {
    let start = *i;
    let mut n: u128 = 0;
    while *i < c.len() && c[*i].is_ascii_digit() {
        if *i - start >= max {
            return None;
        }
        n = n * 10 + u128::from(c[*i] as u8 - b'0');
        *i += 1;
    }
    (*i > start).then_some(n)
}

/// A number continued by `/` or `.digit` (like `1/2/3`) is not a clean size.
fn bad_tail(c: &[char], j: usize) -> bool {
    c.get(j) == Some(&'/') || (c.get(j) == Some(&'.') && digit_at(c, j + 1))
}

fn digit_at(c: &[char], i: usize) -> bool {
    c.get(i).is_some_and(char::is_ascii_digit)
}

/// `n/d` at `*i` (no spaces); `None` leaves `*i` unspecified.
fn read_fraction(c: &[char], i: &mut usize) -> Option<(u128, u128)> {
    let n = read_digits(c, i, 9)?;
    if c.get(*i) != Some(&'/') {
        return None;
    }
    *i += 1;
    let d = read_digits(c, i, 9)?;
    (d > 0).then_some((n, d))
}

/// Parse the number starting at `start`. Returns the value (if well formed) and where
/// the token ends, so the caller can skip a malformed one.
fn number(c: &[char], start: usize) -> (Option<Value>, usize) {
    let text = |end: usize| c[start..end].iter().collect::<String>();
    let mut i = start;
    let Some(whole) = (if c[i] == '.' {
        Some(0)
    } else {
        read_digits(c, &mut i, 9)
    }) else {
        let mut end = i;
        while digit_at(c, end) {
            end += 1;
        }
        return (None, end);
    };
    let int_end = i;
    // Decimal.
    if c.get(i) == Some(&'.') && digit_at(c, i + 1) {
        i += 1;
        let from = i;
        let frac = read_digits(c, &mut i, 6);
        let mut end = i;
        while digit_at(c, end) {
            end += 1;
        }
        let bad = frac.is_none() || matches!(c.get(end), Some('/'));
        if bad || (c.get(end) == Some(&'.') && digit_at(c, end + 1)) {
            return (None, end);
        }
        let k = (i - from) as u32;
        let scale = 10u128.pow(k);
        let n = whole * scale + frac.unwrap_or(0);
        return (
            Some(Value {
                num: n,
                den: scale,
                label: text(i),
            }),
            i,
        );
    }
    // Plain fraction.
    if c.get(i) == Some(&'/') && digit_at(c, i + 1) {
        let mut j = start;
        let Some((n, d)) = read_fraction(c, &mut j) else {
            let mut end = j.max(i + 1);
            while digit_at(c, end) {
                end += 1;
            }
            return (None, end);
        };
        if bad_tail(c, j) {
            return (None, j + 1);
        }
        return (
            Some(Value {
                num: n,
                den: d,
                label: format!("{n}/{d}"),
            }),
            j,
        );
    }
    // Mixed number: whole, one space or hyphen, then a fraction.
    if matches!(c.get(int_end), Some(' ' | '-')) && digit_at(c, int_end + 1) {
        let mut j = int_end + 1;
        if let Some((n, d)) = read_fraction(c, &mut j) {
            if !bad_tail(c, j) {
                return (
                    Some(Value {
                        num: whole * d + n,
                        den: d,
                        label: format!("{whole}-{n}/{d}"),
                    }),
                    j,
                );
            }
        }
    }
    (
        Some(Value {
            num: whole,
            den: 1,
            label: text(int_end),
        }),
        int_end,
    )
}

/// Words after "in" that make it a preposition ("3 in the middle").
const PREPOSITION_TAIL: &[&str] = &[
    "the", "a", "an", "this", "that", "these", "those", "it", "its", "each", "all", "one", "my",
    "your", "our", "their", "front", "back", "total", "between", "total",
];

fn unit_at(c: &[char], mut k: usize) -> Option<Unit> {
    if c.get(k).is_some_and(|x| is_quote(*x)) {
        return Some(Unit::Inch);
    }
    if c.get(k) == Some(&'-') {
        k += 1;
    } else {
        while c.get(k) == Some(&' ') {
            k += 1;
        }
    }
    let word = |from: usize| -> (String, usize) {
        let mut e = from;
        while c.get(e).is_some_and(|x| x.is_alphabetic()) {
            e += 1;
        }
        (c[from..e].iter().collect::<String>().to_lowercase(), e)
    };
    let (w, end) = word(k);
    match w.as_str() {
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => Some(Unit::Cm),
        "inch" | "inches" => Some(Unit::Inch),
        "in" => {
            let mut n = end;
            while matches!(c.get(n), Some(' ' | '.' | ',')) {
                n += 1;
            }
            if digit_at(c, n) && n > end && c[end] == ' ' {
                return None;
            }
            let (next, _) = word(n);
            (!PREPOSITION_TAIL.contains(&next.as_str())).then_some(Unit::Inch)
        }
        _ => None,
    }
}

/// True when the number at `start` follows text that makes it part of a name, code or
/// negative number rather than a size.
fn bad_prefix(c: &[char], start: usize) -> bool {
    let Some(p) = start.checked_sub(1).map(|i| c[i]) else {
        return false;
    };
    let before = start.checked_sub(2).map(|i| c[i]);
    match p {
        '.' | '/' | '_' | '\\' | '#' | '"' | '\u{201c}' | '\u{201d}' | '\u{2033}' => true,
        '-' => before.is_none_or(|b| !b.is_alphanumeric()),
        'x' | 'X' | '\u{d7}' => before.is_some_and(char::is_alphabetic),
        p => p.is_alphanumeric(),
    }
}

fn millimetres(v: &Value, unit: Unit) -> Option<String> {
    // Value in 1/10000 mm: inch = 25.4 mm, cm = 10 mm.
    let factor: u128 = match unit {
        Unit::Inch => 254_000,
        Unit::Cm => 100_000,
    };
    let scaled = (v.num * factor * 2 + v.den) / (v.den * 2);
    if scaled == 0 {
        return None;
    }
    let (int, frac) = (scaled / 10_000, scaled % 10_000);
    Some(if frac == 0 {
        int.to_string()
    } else {
        format!("{int}.{frac:04}").trim_end_matches('0').to_string()
    })
}

/// Bounded numeric prefix shared with the overall-size check (including fractions).
pub(crate) fn dimension_number(text: &str) -> Option<(f64, usize)> {
    let c: Vec<char> = text.chars().take(128).collect();
    if c.is_empty() || !(c[0].is_ascii_digit() || c[0] == '.') {
        return None;
    }
    let (v, end) = number(&c, 0);
    let v = v?;
    let n = v.num as f64 / v.den as f64;
    (n > 0.0 && n <= 1_000_000.0).then(|| (n, c[..end].iter().map(|c| c.len_utf8()).sum()))
}

/// Explicit length unit prefix; reuse the inch/preposition rules above.
pub(crate) fn dimension_unit(text: &str) -> Option<(f64, usize)> {
    let c: Vec<char> = text.chars().take(64).collect();
    let word: String = c.iter().take_while(|c| c.is_alphabetic()).collect();
    let factor = match word.to_lowercase().as_str() {
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => 1.0,
        _ => match unit_at(&c, 0)? {
            Unit::Inch => 25.4,
            Unit::Cm => 10.0,
        },
    };
    let end = if c.first().is_some_and(|c| is_quote(*c)) {
        c[0].len_utf8()
    } else {
        word.len()
    };
    (end > 0).then_some((factor, end))
}

/// `1" = 25.4` style entries for every unambiguous inch or cm size, in request order.
pub fn sizes(text: &str) -> Vec<String> {
    let c: Vec<char> = text.chars().take(MAX_SCAN).collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < c.len() && out.len() < MAX_SIZES {
        let starts = c[i].is_ascii_digit() || (c[i] == '.' && digit_at(&c, i + 1));
        if !starts || (i > 0 && c[i - 1].is_ascii_digit()) {
            i += 1;
            continue;
        }
        let (value, end) = number(&c, i);
        let skip = bad_prefix(&c, i);
        if let (Some(v), false) = (value, skip) {
            if let Some(unit) = unit_at(&c, end) {
                if let Some(mm) = millimetres(&v, unit) {
                    let entry = match unit {
                        Unit::Inch => format!("{}\" = {mm}", v.label),
                        Unit::Cm => format!("{} cm = {mm}", v.label),
                    };
                    if !out.contains(&entry) {
                        out.push(entry);
                    }
                }
            }
        }
        i = end.max(i + 1);
    }
    out
}

/// The one-line fact, or `None` when the text states no inch or cm size.
pub fn fact(text: &str) -> Option<String> {
    let list = sizes(text);
    (!list.is_empty()).then(|| format!("{UNIT_MARKER} {}.", list.join("; ")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stated_sizes_convert_exactly() {
        let cases: &[(&str, &str)] = &[
            (
                "box 1\" by 3/8\" by 1/8\"",
                "Sizes in mm: 1\" = 25.4; 3/8\" = 9.525; 1/8\" = 3.175.",
            ),
            ("a 2 in cube", "Sizes in mm: 2\" = 50.8."),
            ("a 2 inch cube", "Sizes in mm: 2\" = 50.8."),
            ("a 3 inches wide plate", "Sizes in mm: 3\" = 76.2."),
            ("a 1in cube", "Sizes in mm: 1\" = 25.4."),
            ("slot 1/8\" wide", "Sizes in mm: 1/8\" = 3.175."),
            ("slot 3/8 in wide", "Sizes in mm: 3/8\" = 9.525."),
            ("slot 3/8 inch wide", "Sizes in mm: 3/8\" = 9.525."),
            ("slot 3/8-inch wide", "Sizes in mm: 3/8\" = 9.525."),
            ("pipe 1-1/2\" long", "Sizes in mm: 1-1/2\" = 38.1."),
            ("pipe 1 1/2 in long", "Sizes in mm: 1-1/2\" = 38.1."),
            ("pipe 2 3/4 inches long", "Sizes in mm: 2-3/4\" = 69.85."),
            ("pipe 1-1/2 inch long", "Sizes in mm: 1-1/2\" = 38.1."),
            ("a 0.5 in hole", "Sizes in mm: 0.5\" = 12.7."),
            ("a .25\" hole", "Sizes in mm: .25\" = 6.35."),
            ("a 3.8 inch bar", "Sizes in mm: 3.8\" = 96.52."),
            ("a 5/16\" bolt", "Sizes in mm: 5/16\" = 7.9375."),
            ("a 1/64\" shim", "Sizes in mm: 1/64\" = 0.3969."),
            ("a 7/32 in pin", "Sizes in mm: 7/32\" = 5.5563."),
            ("a 2 cm cube", "Sizes in mm: 2 cm = 20."),
            ("a 2.5cm cube", "Sizes in mm: 2.5 cm = 25."),
            ("a 0.75 cm gap", "Sizes in mm: 0.75 cm = 7.5."),
            ("a 12 centimeters tall post", "Sizes in mm: 12 cm = 120."),
            ("a 3 centimetres tall post", "Sizes in mm: 3 cm = 30."),
            (
                "10 cm by 5 cm by 1 cm",
                "Sizes in mm: 10 cm = 100; 5 cm = 50; 1 cm = 10.",
            ),
            (
                "plate 4 in x 2 in x 1/4 in",
                "Sizes in mm: 4\" = 101.6; 2\" = 50.8; 1/4\" = 6.35.",
            ),
            (
                "plate 4\"x2\"x1/4\"",
                "Sizes in mm: 4\" = 101.6; 2\" = 50.8; 1/4\" = 6.35.",
            ),
            ("plate 10x2cm", "Sizes in mm: 2 cm = 20."),
            (
                "1/2 in wide and 3 cm long",
                "Sizes in mm: 1/2\" = 12.7; 3 cm = 30.",
            ),
            (
                "a 25 mm tall peg on a 1 in base",
                "Sizes in mm: 1\" = 25.4.",
            ),
            (
                "two 1/4\" holes and a 1/4\" fillet",
                "Sizes in mm: 1/4\" = 6.35.",
            ),
            ("1/2 x 3/4 in", "Sizes in mm: 3/4\" = 19.05."),
            (
                "Make a 12\" ruler, 1\" wide",
                "Sizes in mm: 12\" = 304.8; 1\" = 25.4.",
            ),
            ("a 1\u{201d} cube", "Sizes in mm: 1\" = 25.4."),
            ("a 100 in. tall tower", "Sizes in mm: 100\" = 2540."),
            ("a 4 INCH cube", "Sizes in mm: 4\" = 101.6."),
            (
                "hole 5 cm, depth 3/4\".",
                "Sizes in mm: 5 cm = 50; 3/4\" = 19.05.",
            ),
            ("plate 1/3 in", "Sizes in mm: 1/3\" = 8.4667."),
            ("a 1.5-in wide strip", "Sizes in mm: 1.5\" = 38.1."),
        ];
        for (text, expected) in cases {
            assert_eq!(fact(text).unwrap_or_default(), *expected, "{text}");
        }
        assert!(cases.len() >= 30);
    }

    #[test]
    fn unstated_or_unclear_sizes_get_nothing() {
        for text in [
            "a 20 mm cube",
            "a 20 box",
            "a box 20 by 30 by 40",
            "slot 3/8 wide",
            "a 1/8 gap",
            "make a box",
            "a 2.5 mm hole and a 10 mm post",
            "put 3 in the middle",
            "3 in a row",
            "an M6 bolt",
            "use A4 paper",
            "a 5 in 4 places",
            "a -5 cm offset",
            "a 1/2/3 in thing",
            "a 0 in gap",
            "a 1.2.3 cm gap",
            "a 1 foot post",
            "a 2 centi tall",
            "name it \"20\" please",
            "",
        ] {
            assert_eq!(fact(text), None, "{text}");
        }
    }

    #[test]
    fn list_is_capped_and_deduplicated() {
        let text = (1..=30)
            .map(|n| format!("{n} cm"))
            .collect::<Vec<_>>()
            .join(" and ");
        assert_eq!(sizes(&text).len(), MAX_SIZES);
        assert_eq!(sizes("1 cm 1 cm 1 cm").len(), 1);
    }
}
