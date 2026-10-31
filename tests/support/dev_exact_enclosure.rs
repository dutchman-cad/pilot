//! Independently authored development requests, with careful-reader form values.
use serde_json::{json, Value};

pub fn cases() -> Vec<(String, Value)> {
    let mut cases = Vec::new();
    for (sizes, xyz) in [
        ("72x48x18 mm", [72., 48., 18.]),
        ("7.2 by 4.8 by 1.8 cm", [72., 48., 18.]),
        ("3 x 2 x 1 inches", [76.2, 50.8, 25.4]),
        ("72 mm x 4.8 cm x 0.75 inch", [72., 48., 19.05]),
    ] {
        for wording in 0..3 {
            let prompt = match wording {
                0 => format!("Make an enclosure cover {sizes}, 2 mm walls, with a 3 mm lip around the outside."),
                1 => format!("Build a lid {sizes} with wall 2mm and outside lip 3mm high."),
                _ => format!("A {sizes} enclosure cover with walls 2mm and outer lip 1mm wide and 3mm tall."),
            };
            cases.push((prompt, json!({"length":xyz[0],"width":xyz[1],"height":xyz[2],"wall":2.,"open":"bottom","lip":{"width":1.,"height":3.,"side":"outer"}})));
            let prompt = match wording {
                0 => format!("open top box {sizes}, 2mm walls"),
                1 => format!("box {sizes} open top 2 mm wall"),
                _ => format!("An open-top enclosure {sizes} with walls 2 mm."),
            };
            cases.push((
                prompt,
                json!({"length":xyz[0],"width":xyz[1],"height":xyz[2],"wall":2.,"open":"top"}),
            ));
            let prompt = match wording {
                0 => format!("enclosure {sizes}, walls 2 mm, floor 5 mm thick"),
                1 => format!("Build an enclosure {sizes} with 2mm walls and a 0.5 cm thick floor."),
                _ => format!("An enclosure {sizes}, wall thickness 2mm, base thickness 0.5cm."),
            };
            cases.push((
                prompt,
                json!({"length":xyz[0],"width":xyz[1],"height":xyz[2],"wall":2.,"floor":5.}),
            ));
        }
    }
    for (text, args) in [
        (
            "open top box 66x42x28, 2mm walls",
            json!({"length":66.,"width":42.,"height":28.,"wall":2.}),
        ),
        (
            "box 84 by 52 by 26 mm open top 3 mm wall",
            json!({"length":84.,"width":52.,"height":26.,"wall":3.}),
        ),
        (
            "an enclosure cover 12x8x0.6 cm, walls 0.2cm, outside lip 0.3cm high",
            json!({"length":120.,"width":80.,"height":6.,"wall":2.,"open":"bottom","lip":{"width":1.,"height":3.,"side":"outer"}}),
        ),
        (
            "lid 9x7x0.8cm, 0.2cm walls, outer lip 0.1cm wide and 0.4cm high",
            json!({"length":90.,"width":70.,"height":8.,"wall":2.,"open":"bottom","lip":{"width":1.,"height":4.,"side":"outer"}}),
        ),
        (
            "enclosure 11x7x4cm, walls 0.2cm, floor 0.5cm thick",
            json!({"length":110.,"width":70.,"height":40.,"wall":2.,"floor":5.}),
        ),
        (
            "enclosure 85x55x35mm, wall 2mm, floor 6mm thick",
            json!({"length":85.,"width":55.,"height":35.,"wall":2.,"floor":6.}),
        ),
    ] {
        cases.push((text.into(), args));
    }
    cases
}

pub const DECLINES: &[&str] = &[
    "enclosure cover 9x7 cm, wall 2mm, outer lip 3mm", // Missing height.
    "not an open top box 72x48x18mm, wall 2mm",
    "box 72x48x18mm open top or bottom wall 2mm",
    "enclosure 72x48x18mm, wall 2mm or 3mm, floor 5mm",
    "enclosure 72x48x18mm, wall 2mm, floor 5mm, floor 6mm",
    "enclosure 72x48x18mm, wall 2mm, floor -5mm",
    "enclosure 72x48x18mm, wall 2mm, floor 20mm",
    "lid 72x48x18mm, wall 2mm, inner and outer lip 3mm",
    "enclosure 72x48x18mm, without walls 2mm, floor 5mm",
    "box 72x48x18mm open top and open bottom, wall 2mm",
    "enclosure 72x48x18mm, wall 2mm, floor about 5mm",
    "enclosure 72x48x18mm, wall 2mm, floor 5mm and holes",
    "enclosure 72 x 4.8 cm x 18 mm, wall 2mm, floor 5mm",
    "enclosure 72x48x18mm, wall 2mm, floor 5mm instead of 6mm",
    "enclosure and a cover 72x48x18mm, wall 2mm, outer lip 3mm",
    "cover lid 72x48x18mm, wall 2mm, outer lip 3mm",
    "enclosure 72x48x18mm, wall 2mm, floor 5mm inside",
    "open top box 72x48x18mm wall 2mm and cover lip width 1mm height 3mm",
];
