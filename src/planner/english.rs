//! Independently authored language coverage; no evaluation corpus is used.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;

const FORMS: &[(&str, &[&str])] = &[
    ("box", &[
        "Make a 19 mm cube.", "BUILD a block 31 mm long, 22 mm wide and 4 mm thick!",
        "Create a plate length 31mm, width 22mm, thickness 4mm.",
        "Add a box 3.1cm long and 2.2cm wide and 0.4cm high.",
        "Please make a [cube] of 0.019m;", "Build a box 31×22×4 mm!",
        "Make a 1/2 inch cube.", "Create a box 31mm long; 22mm wide; 4mm deep.",
    ]),
    ("cylinder", &[
        "Create a cylinder 14mm in diameter and 33mm tall.",
        "Build a rod with a radius of 7mm and a height of 33mm!",
        "Add a cylinder with an 8mm radius and 33mm high.",
        "Make a disc 14mm across, 3mm thick.",
        "CREATE a rod diameter 1/2 inch, height 2 inches.",
        "Make a cylinder 1.4cm diameter and 3.3cm deep!",
        "Build a cylinder radius 0.007m tall 0.033m.",
        "Please add a cylinder (diameter) 14mm and (height) 33mm.",
    ]),
    ("sphere", &[
        "MAKE a sphere with a radius of 6.5mm.", "Create a sphere with an 8mm radius!",
        "Add a sphere 13mm in diameter.", "Build a sphere 1.3cm across;",
        "Make a sphere radius 0.0065m.", "Create a sphere diameter 1/2 inch!",
        "Please build a sphere [radius] 6.5mm.", "Add a sphere 13mm diameter.",
    ]),
    ("plate_hole", &[
        "Draw a 20 mm cube with a centered 5mm through hole.",
        "Draw a cube 20mm with a 5mm through hole in the centre.",
        "Make a 20mm cube with a 5mm through hole in the center.",
        "Draw a plate 20x20x20mm with a centred through hole diameter 5mm.",
        "Draw a block 20 by 30 by 10mm with a 5mm hole through the middle.",
        "Make a plate 66x44x4mm with a centred through hole 6mm in diameter.",
        "BUILD a plate 66mm long, 44mm wide and 4mm thick with a middle through hole radius 3mm!",
        "Add a plate 6.6×4.4×0.4cm with a centered through hole 0.6cm across.",
        "Create a plate length 66 width 44 height 4 with a centre through hole diameter 6.",
        "Make a plate 3x2x1/4 inches with a centred through hole 1/4 inch in diameter.",
        "Build a plate 66x44x4mm with a centred through hole with a radius of 3mm.",
        "Please add a plate 66 by 44 by 4mm with a through hole in the middle 6mm across!",
        "CREATE a plate 66x44x4mm with a centred through hole diameter 6mm;",
    ]),
    ("plate_holes", &[
        "Make a plate 90x70x5mm with 2 through holes 6mm in diameter at (15,18), (72,51).",
        "BUILD a plate 9x7x0.5cm with 2 through holes radius 0.3cm at (1.5cm,1.8cm), (7.2cm,5.1cm)!",
        "Create a plate 90mm long, 70mm wide and 5mm thick with 2 holes through diameter 6mm at (15,18), (72,51).",
        "Add a plate 90×70×5mm with 2 through holes diameter 6mm at (15,18) and (72,51).",
        "Please build a plate 90 by 70 by 5mm with 2 through holes with a radius of 3mm at (15,18), (72,51)!",
        "Create a plate length 90 width 70 height 5 with 2 through holes 6mm across at (15,18), (72,51).",
        "MAKE a plate 4x3x1/4 inches with 2 through holes diameter 1/4 inch at (1inch,1inch), (3inch,2inch).",
        "Add a plate 0.09x0.07x0.005m with 2 holes through diameter 0.006m at (15mm,18mm), (72mm,51mm);",
        "Make a plate 90x70x5mm with holes 6mm across through in a rectangular pattern 2 rows 2 columns pitch 24x24mm centered.",
        "BUILD a plate 9x7x0.5cm with holes radius 0.3cm through in a rectangular pattern 2 rows 2 columns pitch 2.4x2.4cm centred!",
        "Create a plate length 90 width 70 height 5 with holes diameter 6mm through in a rectangular pattern 2 rows 2 columns pitch 24×24mm centered.",
        "Add a plate 90mm long, 70mm wide and 5mm thick with holes 6mm in diameter through in a rectangular pattern 2 rows 2 columns pitch 24 by 24mm centred.",
        "Please make a plate 4x3x1/4 inches with 4 through holes radius 1/8 inch in a rectangular pattern 2 rows 2 columns pitch 1x1 inch centered.",
        "Build a plate 90x70x5mm with holes with a radius of 3mm through in a rectangular pattern 2 rows 2 columns pitch 24x24mm centred!",
        "CREATE a plate 0.09x0.07x0.005m with 4 holes through diameter 6mm in a rectangular pattern 2 rows 2 columns pitch 0.024x0.024m centered;",
        "Make a plate 90 by 70 by 5mm with holes 6mm diameter through in a rectangular pattern 2 rows 2 columns pitch 24x24mm centred.",
        "Create a plate 90x70x5mm with 4 through holes diameter 6mm in a circular pattern radius 21mm centred equally spaced starting at +X.",
        "Add a plate 90x70x5mm with 4 through holes radius 3mm in a circular pattern 42mm in diameter centered equally spaced starting at +X!",
        "Please build a plate 9x7x0.5cm with 4 through holes diameter 0.6cm in a circular pattern 2.1cm radius centered equally spaced starting at +X.",
        "MAKE a plate 90mm long, 70mm wide and 5mm thick with 4 through holes 6mm across in a circular pattern radius 21mm centred equally spaced starting at +X!",
        "Create a plate length 90 width 70 height 5 with 4 holes through diameter 6mm in a circular pattern diameter 42mm centered equally spaced starting at +X.",
        "Add a plate 4x3x1/4 inches with 4 through holes radius 1/8 inch in a circular pattern radius 3/4 inch centred equally spaced starting at +X;",
        "Build a plate 0.09x0.07x0.005m with 4 through holes diameter 0.006m in a circular pattern radius 0.021m centered equally spaced starting at +X.",
        "Make a plate 90 by 70 by 5mm with 4 through holes with a radius of 3mm in a circular pattern radius 21mm centred equally spaced starting at +X!",
    ]),
    ("hole", &[
        "Drill a hole diameter 3mm through the body at (4,5,10).",
        "BORE a hole 3mm in diameter through the box at (4,5,10)!",
        "Cut a hole radius 1.5mm through the part at (4,5,10).",
        "Drill a hole 0.3cm across through it at (0.4cm,0.5cm,1cm).",
        "Bore a hole with a radius of 0.0015m through Blank at (4,5,10).",
        "Please cut a hole diameter 1/8 inch through Blank at (4,5,10)!",
        "Drill a hole with a 1.5mm radius through the body at (4mm,5mm,10mm).",
        "Bore a hole diameter 3mm through the current body at (4,5,10).",
    ]),
    ("sketch_extrude", &[
        "Sketch a rectangle 16x24mm on XY then extrude 5mm.",
        "CREATE a sketch rectangle 1.6×2.4cm on YZ then extrude 0.5cm!",
        "Build a sketch rectangle 1/2 by 1 inch on XZ then extrude 1/8 inch.",
        "Please sketch a rectangle 0.016x0.024m on [XY] then extrude 0.005m.",
        "Make a sketch rectangle 16 by 24mm on YZ then extrude 5mm!",
        "Sketch a rectangle 1.6x2.4cm on XZ, then extrude 0.5cm.",
        "Add a sketch rectangle 16×24mm on XY then extrude 5mm;",
        "Please create a sketch rectangle 16x24mm on YZ then extrude 0.005m.",
        "Sketch a circle with a radius of 8mm on XY then extrude 5mm.",
        "MAKE a sketch circle 16mm in diameter on YZ then extrude 5mm!",
        "Add a sketch circle with a 0.8cm radius on XZ then extrude 0.5cm.",
        "Create a sketch circle diameter 1/2 inch on XY then extrude 1/8 inch.",
        "Please build a sketch circle 1.6cm across on [XY] then extrude 0.5cm.",
        "Sketch a circle 0.008m radius on XZ then extrude 0.005m!",
        "Create a sketch circle radius 8mm on YZ, then extrude 5mm;",
        "Add a sketch circle with an 8mm radius on XY then extrude 5mm.",
    ]),
    ("fillet", &[
        "Round all edges of the box radius 1.5mm.", "ROUND OFF all edges of the part with a radius of 1.5mm!",
        "Fillet the body all edges radius 0.15cm.", "Please round it all edges 0.0015m.",
        "Round off Blank all edges radius 1/16 inch.", "Fillet all edges of Blank with a 1.5mm radius!",
        "Round all edges of it 1.5mm radius.", "Fillet all edges of the current body radius 1.5mm;",
    ]),
    ("chamfer", &[
        "Bevel all edges of the box distance 1.5mm.", "BEVEL top edges of the part 1.5mm!",
        "Chamfer the body all edges distance 0.15cm.", "Please bevel it top edges 0.0015m.",
        "Bevel Blank all edges distance 1/16 inch.", "Chamfer top edges of Blank distance 1.5mm!",
        "Bevel all edges of it 1.5mm.", "Chamfer top edges of the current body distance 1.5mm;",
        "Please BEVEL all edges of the current body distance 0.15cm.",
        "Chamfer Blank all edges 1/16 inch!", "Bevel it all edges 0.0015m.",
        "Chamfer all edges of the part distance 1.5mm;",
        "Please BEVEL top edges of the current body distance 0.15cm.",
        "Chamfer Blank top edges 1/16 inch!", "Bevel it top edges 0.0015m.",
        "Chamfer top edges of the box distance 1.5mm;",
    ]),
    ("move", &[
        "Shift the box 11mm along X.", "SLIDE the part 1.1cm along Y!",
        "Move it -11mm along Z.", "Translate the body 0.011m along x.",
        "Please slide Blank 1/2 inch along y.", "Shift the current body 11mm on z;",
        "Move Blank 11mm along negative X.", "Slide it 11mm along positive Y.",
    ]),
    ("rename", &[
        "Call it Sleeve.", "NAME the body 'Axle support'!", "Rename the box to \"Spacer v2.5\".",
        "Call the part `Left rail`.", "Please name Blank Sleeve;", "RENAME it to 'Half inch'!",
        "Name the current body \"1/2 inch rod\".", "Call Blank 'Mount, upper'.",
    ]),
    ("mirror", &[
        "Mirror the box about XY.", "MIRROR the part across YZ!", "Please mirror it about XZ;",
        "Mirror the body across [XY].", "Mirror Blank about YZ.", "Mirror the current body across XZ!",
        "Mirror it across the XY plane.", "Mirror Blank about the YZ plane.",
    ]),
    ("shell", &[
        "Shell a box 62x46x28mm wall 1.5mm open top.",
        "SHELL a box 62mm long, 46mm wide and 28mm high wall 1.5mm remove top!",
        "Create a shell box length 62 width 46 height 28 wall 1.5 open top.",
        "Build a shell box 6.2×4.6×2.8cm wall 0.15cm remove top.",
        "Please shell a box 0.062x0.046x0.028m wall 0.0015m open top;",
        "Make a shell box 3x2x1 inches wall 1/16 inch open top!",
        "Shell a box 62 by 46 by 28mm wall 1.5mm remove top.",
        "Add a shell box 62x46x28mm wall 1.5mm open top.",
    ]),
    ("revolve", &[
        "Revolve a rectangle radius 6mm height 27mm from axis on XZ about Z full turn.",
        "REVOLVE a rectangle 6mm radius and 27mm high from axis on XZ about Z full turn!",
        "Create a revolve rectangle with a radius of 6mm and height 27mm from axis on XZ about Z full turn.",
        "Build a revolve rectangle radius 0.6cm height 2.7cm from axis on XZ about Z full turn;",
        "Please revolve a rectangle radius 0.006m height 0.027m from axis on XZ about Z full turn.",
        "Make a revolve rectangle radius 1/4 inch height 1 inch from axis on XZ about Z full turn!",
        "Revolve a rectangle radius 6mm, height 27mm from axis on XZ about Z full turn.",
        "Add a revolve rectangle with an 8mm radius and 27mm high from axis on XZ about Z full turn.",
    ]),
    ("volume", &[
        "Report volume.", "MEASURE the volume!", "Give the volume;", "Tell me volume.",
        "What is the volume of the design?", "Please report the volume.",
        "Measure the volume of Blank.", "Give me the volume of all bodies!",
    ]),
    ("body_count", &[
        "Count bodies.", "COUNT the bodies!", "How many bodies are present?", "Please count bodies;",
        "Tell me how many bodies exist.", "Count the bodies in the design.",
        "How many bodies are there?", "Please tell me how many bodies are present!",
    ]),
];

#[test]
fn ordinary_english_and_conservative_refusals() {
    let catalog = Catalog::bundled().unwrap();
    for (rule, phrases) in FORMS {
        assert!(phrases.len() >= 8, "{rule}");
        for phrase in *phrases {
            let p = plan(&cases::request(phrase), &catalog)
                .unwrap()
                .unwrap_or_else(|| panic!("missing {rule}: {phrase}"));
            assert_eq!(p.rule, *rule, "{phrase}");
            if !matches!(*rule, "volume" | "body_count") {
                for negative in [
                    format!("How do I {phrase}"),
                    format!("{}?", phrase.trim_end_matches(['.', '!', ';', ':'])),
                    format!("Don't {phrase}"),
                    format!("Explain how to {phrase}"),
                    format!("{phrase} and shift Blank 3mm along X"),
                ] {
                    assert!(
                        plan(&cases::request(&negative), &catalog)
                            .unwrap()
                            .is_none(),
                        "{negative}"
                    );
                }
            }
        }
    }
}

#[test]
fn values_are_assigned_to_their_own_labels() {
    let c = Catalog::bundled().unwrap();
    let p = plan(&cases::request("Make a 19 \"cube\"."), &c)
        .unwrap()
        .unwrap();
    assert_eq!(p.slots, json!({"Box":{"size":[19.0,19.0,19.0]}}));
    for text in [
        "Build a box 4mm thick, 31mm long and 22mm wide.",
        "Build a box width 22mm and thickness 4mm, length 31mm.",
    ] {
        let p = plan(&cases::request(text), &c).unwrap().unwrap();
        assert_eq!(p.slots, json!({"Box":{"size":[31.0,22.0,4.0]}}));
    }
    let p = plan(
        &cases::request("Make a cylinder 33mm tall and 14mm in diameter."),
        &c,
    )
    .unwrap()
    .unwrap();
    assert_eq!(p.slots, json!({"Cylinder":{"radius":7.0,"height":33.0}}));
    let p = plan(&cases::request("Name the body \"Spacer, v2.5!\"."), &c)
        .unwrap()
        .unwrap();
    assert_eq!(p.slots["Rename"]["name"], "Spacer, v2.5!");
    for name in [
        "Don't Move",
        "Round, bevel and slide!",
        "How to drill",
        "Name: [Part 2.5]",
        "Why?",
    ] {
        let text = format!("Call it \"{name}\".");
        let p = plan(&cases::request(&text), &c).unwrap().unwrap();
        assert_eq!(p.slots["Rename"]["name"], name);
    }
    let p = plan(&cases::request("Rename 'Blank' to \"Round off?\"."), &c)
        .unwrap()
        .unwrap();
    assert_eq!(p.slots["Rename"]["name"], "Round off?");
}

#[test]
fn punctuation_and_case_do_not_change_dimensions() {
    let c = Catalog::bundled().unwrap();
    for punctuation in [".", ",", ";", ":", "!"] {
        for word in ["cube", "CUBE", "[Cube]", "(cube)", "'cube'", "\"cube\""] {
            let text = format!("MAKE a 1.25 cm {word}{punctuation}");
            let p = plan(&cases::request(&text), &c).unwrap().unwrap();
            assert_eq!(p.slots, json!({"Box":{"size":[12.5,12.5,12.5]}}), "{text}");
        }
    }
    let p = plan(&cases::request("Could you MAKE a 1/2 inch cube?"), &c)
        .unwrap()
        .unwrap();
    assert_eq!(p.slots, json!({"Box":{"size":[12.7,12.7,12.7]}}));
    for alias in ["the box", "the part", "the body", "it"] {
        let mut req = cases::request(&format!("Move {alias} 3mm along X."));
        req["messages"][0]["content"] = json!(
            json!({"bodies":[{"name":"Box"},{"name":"Part"},{"name":"Body"},{"name":"It"}]})
                .to_string()
        );
        assert!(plan(&req, &c).unwrap().is_none(), "{alias}");
    }
}

#[test]
fn missing_conflicting_and_ambiguous_slots_defer() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "Make a box 31mm long and 22mm wide.",
        "Make a cylinder 14mm across.",
        "Round off the box all edges.",
        "Bevel the part 2mm.",
        "Slide it 11mm.",
        "Call the body.",
        "Drill a hole diameter 3mm through the body.",
        "Bore a hole through it at (4,5,10).",
        "Cut a hole diameter 3mm through Unknown at (4,5,10).",
        "Make a box 31mm long, 22mm long and 4mm thick.",
        "Make a sphere radius 6mm diameter 12mm.",
        "Round all edges and bevel top edges 1mm.",
        "Make a cube 19mm and a sphere radius 4mm.",
        "Slide it 11mm along X and Y.",
        "Is it possible to build a 19mm cube?",
        "How do I drill a hole?",
        "Don't bore a hole diameter 3mm through Blank at (4,5,10).",
        "Call it 'A' and shift it 2mm along X to 'B'.",
        "Rename it to 'A' and name the body 'B'.",
        "Make a - 19mm cube.",
    ] {
        assert!(plan(&cases::request(text), &c).unwrap().is_none(), "{text}");
    }
    for text in [
        "Move the box 11mm along X.",
        "Shift the part 11mm along Y.",
        "Call it Sleeve.",
        "Name the body Sleeve.",
        "Round off all edges of the box radius 1mm.",
        "Bevel top edges of the part 1mm.",
        "Mirror it across XY.",
        "Drill a hole diameter 3mm through the box at (4,5,10).",
    ] {
        let mut req = cases::request(text);
        req["messages"][0]["content"] =
            json!(json!({"bodies":[{"name":"Alpha"},{"name":"Beta"}]}).to_string());
        assert!(plan(&req, &c).unwrap().is_none(), "{text}");
    }
}
