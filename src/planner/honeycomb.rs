//! Staggered hexagonal through cuts built from two bounded feature arrays.
use super::*;

pub fn schema() -> Value {
    let mut props = json!({});
    for k in ["length", "width", "thickness", "size", "web"] {
        props[k] = json!({"$ref":"#/$defs/Length"});
    }
    props["border"] = json!({"type":"number","minimum":0,"maximum":1000000});
    props["plane"] = json!({"$ref":"#/$defs/Plane"});
    props["centred"] = json!({"type":"boolean"});
    props["size_kind"] = json!({"type":"string","enum":["across_flats","across_corners"]});
    props["orientation"] = json!({"type":"string","enum":["pointy_top","flat_top"]});
    props["mode"] = json!({"type":"string","enum":["fill","exact"]});
    for k in ["rows", "columns"] {
        props[k] = json!({"type":"integer","minimum":1,"maximum":5000});
    }
    let mut schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":props,"required":["length","width","thickness","size","size_kind","web","border"],"additionalProperties":false,"$defs":crate::recipe_library::definitions(),"x-pilot-expander":"honeycomb_pattern"});
    crate::recipe_library::forms::describe(&mut schema);
    schema
}

#[derive(Debug)]
struct Field {
    af: f64,
    radius: f64,
    pitch: f64,
    row_pitch: f64,
    rows: usize,
    columns: [usize; 2],
    start: [f64; 2],
    flat: bool,
}
impl Field {
    fn count(&self) -> usize {
        self.rows.div_ceil(2) * self.columns[0] + self.rows / 2 * self.columns[1]
    }
}
fn num(a: &Value, key: &str) -> Result<f64, String> {
    a[key]
        .as_f64()
        .ok_or_else(|| format!("honeycomb needs numeric {key}"))
}
fn field(a: &Value) -> Result<Field, String> {
    crate::recipe_library::validate_structure(a, &schema())?;
    let af = num(a, "size")?
        * if a["size_kind"] == "across_corners" {
            3_f64.sqrt() / 2.
        } else {
            1.
        };
    let radius = af / 3_f64.sqrt();
    let web = num(a, "web")?;
    let pitch = af + web;
    let row_pitch = pitch * 3_f64.sqrt() / 2.;
    let flat = a["orientation"] == "flat_top";
    let (along, across) = if flat {
        (num(a, "width")?, num(a, "length")?)
    } else {
        (num(a, "length")?, num(a, "width")?)
    };
    let border = num(a, "border")?;
    let free = [along - 2. * border - af, across - 2. * border - 2. * radius];
    if free.iter().any(|v| *v < -1e-9) {
        return Err(format!("size {} with border {border} needs at least [{},{}] mm but length/width provide [{along},{across}]; reduce size or border", a["size"], 2. * border + af, 2. * border + 2. * radius));
    }
    let capacity = |space: f64, step: f64| -> usize {
        // Even with empty odd rows this sentinel guarantees more than 5000 cuts.
        ((space + 1e-9) / step).floor().clamp(0., 10001.) as usize + 1
    };
    let exact = a["mode"] == "exact";
    if exact != (a.get("rows").is_some() && a.get("columns").is_some())
        || (!exact && (a.get("rows").is_some() || a.get("columns").is_some()))
    {
        return Err(format!("mode {} with rows {} and columns {} requires 2 counts for exact or 0 for fill; supply both counts or omit both", a["mode"], a["rows"], a["columns"]));
    }
    let rows = if exact {
        a["rows"].as_u64().ok_or("honeycomb needs rows")? as usize
    } else {
        capacity(free[1], row_pitch)
    };
    let columns = if exact {
        let n = a["columns"].as_u64().ok_or("honeycomb needs columns")? as usize;
        [n, n]
    } else {
        [
            capacity(free[0], pitch),
            if free[0] + 1e-9 >= pitch / 2. {
                capacity(free[0] - pitch / 2., pitch)
            } else {
                0
            },
        ]
    };
    // Empty odd rows are allowed in a narrow field; retain every fitting even row.
    let rows = if columns[1] == 0 && rows % 2 == 0 {
        rows - 1
    } else {
        rows
    };
    let span = [
        af + ((columns[0] - 1) as f64 * pitch).max(if rows > 1 && columns[1] > 0 {
            (columns[1] - 1) as f64 * pitch + pitch / 2.
        } else {
            0.
        }),
        2. * radius + (rows - 1) as f64 * row_pitch,
    ];
    if span[0] > along - 2. * border + 1e-9 || span[1] > across - 2. * border + 1e-9 {
        return Err(format!("rows {rows} columns {columns:?} span {span:?} must fit inside [{},{}] mm; reduce rows or columns", along - 2. * border, across - 2. * border));
    }
    let f = Field {
        af,
        radius,
        pitch,
        row_pitch,
        rows,
        columns,
        start: [
            (along - span[0]) / 2. + af / 2.,
            (across - span[1]) / 2. + radius,
        ],
        flat,
    };
    if f.count() == 0 || f.count() > 5000 {
        return Err(format!(
            "rows * columns cut count {} must be 1..5000; increase size/web or reduce rows/columns",
            f.count()
        ));
    }
    Ok(f)
}

pub(super) fn build(a: &Value, request: &Value, catalog: &Catalog) -> Result<Plan, String> {
    let f = field(a)?;
    let length = num(a, "length")?;
    let width = num(a, "width")?;
    let thickness = num(a, "thickness")?;
    let plane = a["plane"].as_str().unwrap_or("XY");
    // Explicit orthonormal frames keep the plate corner convention on all planes.
    let (u, v) = match plane {
        "XZ" => ([1., 0., 0.], [0., 0., 1.]),
        "YZ" => ([0., 1., 0.], [0., 0., 1.]),
        _ => ([1., 0., 0.], [0., 1., 0.]),
    };
    let offset = if a["centred"] == true {
        [-length / 2., -width / 2.]
    } else {
        [0., 0.]
    };
    let origin: [f64; 3] = std::array::from_fn(|i| u[i] * offset[0] + v[i] * offset[1]);
    let dims = match plane {
        "XZ" => [length, thickness, width],
        "YZ" => [thickness, length, width],
        _ => [length, width, thickness],
    };
    let mut corner = origin;
    if plane == "XZ" {
        corner[1] -= thickness;
    }
    let mut calls = vec![call(
        catalog,
        "solid.box",
        json!({"length":dims[0],"width":dims[1],"height":dims[2],"corner":corner,"operation":"new","body_name":"HoneycombPlate"}),
    )?];
    for parity in 0..2 {
        let row_count = (f.rows + 1 - parity) / 2;
        let col_count = f.columns[parity];
        if row_count == 0 || col_count == 0 {
            continue;
        }
        let along = f.start[0] + parity as f64 * f.pitch / 2.;
        let across = f.start[1] + parity as f64 * f.row_pitch;
        let center = if f.flat {
            [across, along]
        } else {
            [along, across]
        };
        let name = format!("HoneycombCut{parity}");
        calls.push(call(catalog,"sketch.create",json!({"plane":{"origin":origin,"x_dir":u,"y_dir":v},"name":format!("HoneycombSeed{parity}")}))?);
        calls.push(call(
            catalog,
            "sketch.polygon.inscribed",
            json!({"center":center,"radius":f.radius,"sides":6,"angle":if f.flat {0} else {30}}),
        )?);
        calls.push(call(catalog, "sketch.finish", json!({}))?);
        calls.push(call(
            catalog,
            "solid.extrude",
            json!({"through_all":true,"operation":"cut","targets":["HoneycombPlate"],"name":name}),
        )?);
        if col_count * row_count > 1 {
            let (d1, d2) = if f.flat { (v, u) } else { (u, v) };
            calls.push(call(catalog,"solid.pattern.rectangular",json!({"features":[name],"dir1":d1,"count1":col_count,"spacing1":f.pitch,"dir2":d2,"count2":row_count,"spacing2":2.*f.row_pitch}))?);
        }
    }
    calls.push(call(catalog, "document.inspect", json!({"measure":true}))?);
    let cut_volume = f.count() as f64 * 3_f64.sqrt() / 2. * f.af * f.af * thickness;
    let new_volume = length * width * thickness - cut_volume;
    let baseline = state(
        request["messages"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    );
    let (bodies, volume) = if let Some(data) = baseline {
        if data["bodies"].as_array().is_some_and(|b| {
            b.len() >= MAX_BODIES || b.iter().any(|b| b["name"] == "HoneycombPlate")
        }) {
            return Err(
                format!("body HoneycombPlate with {} existing bodies requires fewer than {MAX_BODIES} bodies and 0 name collisions; remove bodies or rename the existing HoneycombPlate", data["bodies"].as_array().map_or(0, Vec::len)),
            );
        }
        (
            data["bodies"].as_array().map(|b| b.len() + 1),
            volume(&data).map(|v| v + new_volume),
        )
    } else {
        (None, None)
    };
    let max: [f64; 3] = std::array::from_fn(|i| corner[i] + dims[i]);
    Ok(Plan {
        rule: "honeycomb_pattern",
        calls,
        facts: Some(
            json!({"hex_count":f.count(),"pitch_mm":f.pitch,"row_pitch_mm":f.row_pitch,"orientation":if f.flat {"flat_top"} else {"pointy_top"}}),
        ),
        slots: a.clone(),
        expected: Some(Expected {
            bodies,
            volume_mm3: volume,
            volume_range_mm3: None,
            name: Some("HoneycombPlate".into()),
            created_volume_mm3: Some(new_volume),
            placement: Some(Placement {
                body: "HoneycombPlate".into(),
                min: corner,
                max,
            }),
            feature: None,
            joint: None,
        }),
        continuation: None,
    })
}

pub(super) fn candidate(text: &str) -> bool {
    slots::tokens(text).iter().any(|w| {
        matches!(
            w.as_str(),
            "honeycomb" | "hexagon" | "hexagons" | "hexagonal" | "hexes"
        )
    })
}

/// Tidy forms specify plate dimensions first, then hex size, web and border.
/// A vocabulary gate and exact numeric consumption keep unrelated edits out.
pub(super) fn select(
    text: &str,
    request: &Value,
    catalog: &Catalog,
) -> Result<Option<Plan>, String> {
    let t = slots::tokens(text);
    let allowed = "draw make create build cut perforate a an the plate panel rectangle rectangular honeycomb staggered hexagon hexagons hexagonal pattern of holes through cuts on in plane xy xz yz with and then that was is has it depth deep thickness thick length long width wide height x by mm cm m inch inches spacing web gap between neighbouring neighboring hexes each other apart from leave leaving border margin around all edges edge to flat flats across af point points corner corners origin at centred centered fill exact rows columns row column top pointy - flat top";
    if !t.iter().any(|w| {
        matches!(
            w.as_str(),
            "draw" | "make" | "create" | "build" | "cut" | "perforate"
        )
    }) || !t
        .iter()
        .any(|w| matches!(w.as_str(), "plate" | "rectangle" | "panel"))
        || t.iter()
            .any(|w| !allowed.split_whitespace().any(|a| a == w) && slots::number(w).is_none())
    {
        return Ok(None);
    }
    let has = |w: &str| t.iter().any(|v| v == w);
    let words = t
        .iter()
        .filter(|w| w.as_str() != "-")
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let af = has("af")
        || words.contains("edges to edges")
        || words.contains("flat to flat")
        || words.contains("across flats");
    let ac = words.contains("point to point")
        || words.contains("corner to corner")
        || words.contains("across corners");
    // `width` is a size qualifier only when it follows the plate dimensions.
    let mut numbers = Vec::new();
    for (i, w) in t.iter().enumerate() {
        if let Some(n) = slots::number(w) {
            let scale = t.get(i + 1).and_then(|u| slots::scale(u)).unwrap_or(1.);
            numbers.push((i, n * scale));
        }
    }
    let exact = has("exact");
    if numbers.len() != if exact { 8 } else { 6 } || af && ac {
        return Ok(None);
    }
    let size_width = t[numbers[2].0 + 1..numbers[4].0]
        .iter()
        .any(|w| w == "width");
    if !(af || ac || size_width)
        || !has("border") && !has("margin")
        || !has("web") && !has("spacing") && !has("gap")
    {
        return Ok(None);
    }
    let separator = &t[numbers[0].0 + 1..numbers[1].0];
    if !separator.iter().any(|w| matches!(w.as_str(), "x" | "by")) {
        return Ok(None);
    }
    let thickness = &t[numbers[1].0 + 1..numbers[3].0];
    if !thickness.iter().any(|w| {
        matches!(
            w.as_str(),
            "x" | "by" | "depth" | "deep" | "thick" | "thickness"
        )
    }) {
        return Ok(None);
    }
    // Shared trailing units in L x W x T are propagated to the dimension group.
    let mut values: Vec<_> = numbers.iter().map(|(_, n)| *n).collect();
    for j in 0..2 {
        if t.get(numbers[j].0 + 1)
            .and_then(|u| slots::scale(u))
            .is_some()
        {
            continue;
        }
        for next in j + 1..3 {
            let separator = &t[numbers[next - 1].0 + 1..numbers[next].0];
            if separator.len() != 1 || !matches!(separator[0].as_str(), "x" | "by") {
                break;
            }
            if let Some(unit) = t.get(numbers[next].0 + 1).and_then(|u| slots::scale(u)) {
                values[j] *= unit;
                break;
            }
        }
    }
    let plane: Vec<_> = t
        .iter()
        .filter(|w| matches!(w.as_str(), "xy" | "xz" | "yz"))
        .collect();
    if plane.len() > 1 || has("centred") && has("centered") || has("fill") && exact {
        return Ok(None);
    }
    let size_region = &t[numbers[2].0 + 1..numbers[4].0];
    if !size_region.iter().any(|w| {
        matches!(
            w.as_str(),
            "hexagon"
                | "hexagons"
                | "hexagonal"
                | "hexes"
                | "af"
                | "across"
                | "edge"
                | "edges"
                | "flat"
                | "point"
                | "corner"
                | "width"
        )
    }) {
        return Ok(None);
    }
    // Size/web/border must occur in that order; refuse ambiguous or extra operations.
    let web_region = &t[numbers[3].0 + 1..numbers[5].0];
    let border_region = &t[numbers[4].0 + 1..];
    if !web_region
        .iter()
        .any(|w| matches!(w.as_str(), "web" | "spacing" | "gap" | "apart"))
        || !border_region
            .iter()
            .any(|w| matches!(w.as_str(), "border" | "margin"))
    {
        return Ok(None);
    }
    let flat = text.to_lowercase().contains("flat-top") || text.to_lowercase().contains("flat top");
    let pointy =
        text.to_lowercase().contains("pointy-top") || text.to_lowercase().contains("pointy top");
    if flat && pointy {
        return Ok(None);
    }
    let mut a = json!({"length":values[0],"width":values[1],"thickness":values[2],"size":values[3],"size_kind":if ac {"across_corners"} else {"across_flats"},"web":values[4],"border":values[5],"plane":plane.first().map_or("XY".into(),|p|p.to_uppercase()),"centred":has("centred")||has("centered"),"orientation":if flat {"flat_top"} else {"pointy_top"},"mode":if exact {"exact"} else {"fill"}});
    if exact {
        let r = t.iter().position(|w| w == "rows");
        let c = t.iter().position(|w| w == "columns");
        let ordered = r.zip(c).is_some_and(|(r, c)| {
            r < c
                && numbers[6].0 < c
                && r < numbers[7].0
                && if r < numbers[6].0 {
                    c < numbers[7].0
                } else {
                    c > numbers[7].0
                }
        });
        let counts = numbers[6..].iter().all(|(i, _)| {
            t[*i].parse::<u64>().is_ok() && t.get(*i + 1).and_then(|u| slots::scale(u)).is_none()
        });
        if !ordered || !counts || values[6].fract() != 0. || values[7].fract() != 0. {
            return Ok(None);
        }
        a["rows"] = json!(values[6] as u64);
        a["columns"] = json!(values[7] as u64);
    }
    match build(&a, request, catalog) {
        Ok(p) => Ok(Some(p)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    fn args() -> Value {
        json!({"length":150,"width":300,"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4})
    }
    fn req(text: &str) -> Value {
        json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]})
    }
    #[test]
    fn geometry_counts_pitches_and_vertices() {
        for flat in [false, true] {
            let mut a = args();
            a["orientation"] = json!(if flat { "flat_top" } else { "pointy_top" });
            let f = field(&a).unwrap();
            assert_eq!(f.count(), if flat { 646 } else { 624 });
            assert!((f.pitch - 8.5).abs() < 1e-10);
            assert!((f.row_pitch - 8.5 * 3_f64.sqrt() / 2.).abs() < 1e-10);
            for row in 0..f.rows {
                for col in 0..f.columns[row % 2] {
                    let along = f.start[0] + col as f64 * f.pitch + (row % 2) as f64 * f.pitch / 2.;
                    let across = f.start[1] + row as f64 * f.row_pitch;
                    let center = if flat {
                        [across, along]
                    } else {
                        [along, across]
                    };
                    for vertex in 0..6 {
                        let angle =
                            (vertex as f64 * 60. + if flat { 0. } else { 30. }).to_radians();
                        let p = [
                            center[0] + f.radius * angle.cos(),
                            center[1] + f.radius * angle.sin(),
                        ];
                        assert!(
                            p[0] >= 4. - 1e-9
                                && p[0] <= 146. + 1e-9
                                && p[1] >= 4. - 1e-9
                                && p[1] <= 296. + 1e-9,
                            "{p:?}"
                        );
                    }
                }
            }
            // Neighbour separation is measured normal to facing hex sides.
            assert!((f.pitch - f.af - 1.5).abs() < 1e-10);
        }
        let mut a = args();
        a["size_kind"] = json!("across_corners");
        a["size"] = json!(14. / 3_f64.sqrt());
        assert_eq!(field(&a).unwrap().count(), 624);
        a["mode"] = json!("exact");
        a["rows"] = json!(3);
        a["columns"] = json!(4);
        assert_eq!(field(&a).unwrap().count(), 12);
        a["columns"] = json!(5000);
        assert!(field(&a).is_err());
        a = args();
        a["length"] = json!(10);
        assert!(field(&a).is_err());
        a = args();
        a["length"] = json!(1000);
        a["width"] = json!(1000);
        assert!(field(&a).unwrap_err().contains("5000"));
        a = args();
        a["length"] = json!(15);
        let f = field(&a).unwrap();
        assert_eq!(f.columns, [1, 0]);
        assert!(f.rows > 1);
        a["width"] = json!(1_000_000);
        assert!(field(&a).unwrap_err().contains("5000"));
        a = args();
        a["mode"] = json!("exact");
        a["length"] = json!(10000);
        a["width"] = json!(10000);
        a["rows"] = json!(100);
        a["columns"] = json!(50);
        assert_eq!(field(&a).unwrap().count(), 5000);
        a["rows"] = json!(101);
        assert!(field(&a).unwrap_err().contains("5000"));
        a = args();
        a["length"] = json!(33);
        a["width"] = json!(50);
        assert_eq!(field(&a).unwrap().columns, [3, 2]);
    }
    #[test]
    fn own_wording_and_schema_agree() {
        let c = Catalog::bundled().unwrap();
        let lib = crate::recipe_library::Library::embedded().unwrap();
        let starts = [
            "Make a plate",
            "Draw a rectangle",
            "Create a panel",
            "Build a plate",
            "Cut a plate",
        ];
        let sizes = [
            "7mm edge to edge",
            "7mm flat to flat",
            "7mm AF",
            "7mm width",
        ];
        for start in starts {
            for size in sizes {
                let text=format!("{start} 150x300mm thickness 3mm with a honeycomb pattern {size} spacing 1.5mm border 4mm");
                let p = plan(&req(&text), &c)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{text}"));
                assert_eq!(p.facts.as_ref().unwrap()["hex_count"], 624);
                crate::recipe_library::validate(&p.slots, &schema()).unwrap();
                let typed = lib
                    .build(
                        &json!({"recipe":"honeycomb_pattern","args":p.slots}),
                        &req(&text),
                        &c,
                    )
                    .unwrap();
                assert_eq!(p.calls, typed.calls);
                assert!(p.calls.len() <= 12);
                assert_eq!(
                    lib.find(&text, &crate::recipe_library::Bm25).unwrap()["recipes"][0]["id"],
                    "honeycomb_pattern"
                );
            }
        }
        for text in [
            "Make a plate 100x140x2mm honeycomb 8mm point to point web 2mm border 5mm",
            "Build a plate 100 by 140mm depth 2mm hexagonal cuts 8mm corner to corner gap 2mm margin 5mm",
            "Create a plate 100x140x2mm honeycomb 8mm across corners web 2mm border 5mm centred flat-top on XZ",
            "Draw a plate 100x140x2mm honeycomb 8mm across flats spacing 2mm border 5mm pointy-top on YZ",
            "Perforate a plate 100x140x2mm honeycomb 8mm AF web 2mm border 5mm exact 3 rows 4 columns",
            "Make a plate 100x140x2mm honeycomb 8mm AF web 2mm border 5mm exact rows 3 columns 4",
            "Make a plate 10x14cm thickness 2mm honeycomb 8mm AF spacing 2mm border 5mm",
            "Build a plate 4x6x1/8 inch honeycomb 1/4 inch AF spacing 1/16 inch border 1/4 inch",
        ] {
            let p=plan(&req(text),&c).unwrap().unwrap_or_else(||panic!("{text}"));
            let typed=lib.build(&json!({"recipe":"honeycomb_pattern","args":p.slots}),&req(text),&c).unwrap();
            assert_eq!(typed.calls,p.calls);
            if text.contains("10x14cm") {assert_eq!(p.slots["length"],100.);assert_eq!(p.slots["width"],140.);}
            if text.contains("exact") {assert_eq!(p.facts.as_ref().unwrap()["hex_count"],12);}
        }
        for text in [
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm spacing 1.5mm border 4mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 0mm border 4mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing -1mm border 4mm",
            "Make a plate 10x10mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm",
            "Make a plate 1000x1000mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm",
            "Do not make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm then fillet 2mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF point to point spacing 1.5mm border 4mm",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm flat-top pointy-top",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm exact 4 columns 3 rows",
            "Make a plate 150x300mm thickness 3mm honeycomb 7mm AF spacing 1.5mm border 4mm exact 3mm rows 4 columns",
        ] {assert!(plan(&req(text),&c).unwrap().is_none(),"{text}");}
    }

    #[test]
    fn schema_rejects_the_same_invalid_slots_as_expansion() {
        let c = Catalog::bundled().unwrap();
        for (key, value) in [
            ("web", json!(0)),
            ("web", json!(-1)),
            ("border", json!(-1)),
            ("rows", json!(3)),
            ("mode", json!("exact")),
            ("orientation", json!("diagonal")),
            ("size_kind", json!("edge_length")),
            ("length", json!(1)),
            ("length", json!(1_000_001)),
            ("columns", json!(1.5)),
            ("unexpected", json!(true)),
        ] {
            let mut a = args();
            a[key] = value;
            assert!(
                crate::recipe_library::validate(&a, &schema()).is_err(),
                "{a}"
            );
            assert!(build(&a, &json!({"messages":[]}), &c).is_err(), "{a}");
        }
    }
}
