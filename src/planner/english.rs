//! Independently authored language coverage; no evaluation corpus is used.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;

#[test]
fn ordinary_pin_joint_requests() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "Connect Base and Arm with a revolute joint, then swing Arm 45 degrees.",
        "Join Base and Arm with a pin joint and drive it to 30 degrees.",
        "Attach Arm to Base with a hinge, then turn Arm 60 degrees.",
        "Make Arm pivot on the pin on Base and rotate it 90 degrees.",
        "Use a revolute joint on Base and Arm. Keep Base fixed. Swing Arm 15 degrees.",
        "Connect the existing components with a revolute joint and drive 25 degrees.",
        "Join the two components with a hinge and swing 70 degrees.",
        "Add a revolute joint to Base and Arm.",
        "Create a hinge with Base and Arm.",
        "Connect Arm and Base with a pin joint.",
        "Make a revolute joint with the hole on Arm over the pin on Base.",
        "Connect Arm to Base so Arm pivots on the pin.",
        "Join Base and Arm with a revolute joint then drive 120 deg.",
        "Attach Arm to Base with a hinge and swing it 22.5 degrees.",
        "Use a pin joint with Base and Arm then turn 180 degrees.",
        "Connect Base and Arm with a revolute joint then rotate 0 degrees.",
        "Make a hinge with Base and Arm and drive -20 degrees.",
        "Connect Base and Arm with a revolute joint then swing 45°.",
        "Join Arm and Base with a revolute joint. Keep Arm upright and Base stationary.",
        "Put a hinge on Base and Arm and turn Arm 10 degrees.",
        "Connect Base and Arm with a revolute joint. Base must not move. Arm should stay flat on top of Base. Swing Arm 35 degrees.",
        "Use a revolute joint with Base and Arm, pin diameter 10mm and hole diameter 10.5mm, then drive 45 degrees.",
        "Use a revolute joint with Base and Arm, pin diameter 1cm and hole diameter 1.05cm, then drive 45 degrees.",
        "Use a revolute joint with Base and Arm, pin radius 5mm and hole radius 5.25mm, then drive 45 degrees.",
        "Use a revolute joint with Base and Arm, pin diameter 0.3937007874 inch, then drive 45 degrees.",
        "Please connect Base and Arm with a hinge and swing Arm 33 degrees.",
    ] {
        let req=cases::revolute_request(text);
        let p=plan(&req,&c).unwrap().unwrap_or_else(|| panic!("{text}"));
        assert_eq!(p.rule,"revolute_joint","{text}");
        assert_eq!(p.calls[1]["arguments"]["params"]["flip"],true);
        assert!((p.slots["pin_circle"][2].as_f64().unwrap()-10.).abs()<0.001);
        assert!((p.slots["hole_circle"][2].as_f64().unwrap()-10.).abs()<0.001);
    }
}

const FORMS: &[(&str, &[&str])] = &[
    ("l_profile", &[
        "Sketch an L profile 35mm wide, 55mm tall, both legs 5mm thick, then extrude 12mm.",
        "On XY, draw an L-shaped profile width 35mm, height 55mm, both legs 5mm thick then extrude 12mm.",
        "Make an angle bracket sketch 35mm wide and 55mm high with 5mm thick legs then extrude 12mm.",
        "On XZ sketch an L bracket width 35mm and height 55mm with horizontal leg thickness 4mm and vertical leg thickness 6mm then extrude 12mm.",
        "Draw an L profile on YZ, 35mm wide, 55mm tall, horizontal leg 4mm thick and vertical leg 6mm thick then extrude 12mm.",
        "Build an angle profile 3.5cm wide, 5.5cm tall, both legs 0.5cm thick then extrude 1.2cm.",
        "Sketch an L shaped bracket on XY width 35mm height 55mm both legs 5mm thick then extrude 12mm.",
        "Create a fully constrained closed L profile 35mm wide and 55mm tall with 5mm thick legs then extrude 12mm.",
    ]),
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

#[test]
fn ordinary_closed_profile_wording() {
    let catalog = Catalog::bundled().unwrap();
    for text in [
        "Please make me an angle 40 wide and 30 high with legs 3 thick, extrude it 12",
        "Can you sketch an L-bracket width 40 mm height 30 mm thickness 3 mm then pad 12 mm?",
        "I need an angle bracket 4 by 3 cm with 0.3 cm thick legs, 1.2 cm deep",
        "Draw an L cross-section 40x30 with bottom leg 3 mm thick and upright leg 4 mm thick, 12 long",
        "Make an L 40 mm wide 30 mm tall thickness 3 mm, extrude 12 mm",
        "On the front plane draw an angle 40x30 mm thickness 3 mm then extrude 12 mm",
        "Sketch an L on XZ width 40 height 30 leg thickness 3 then pad 12",
        "Make an angle on the right plane width 40 height 30 horizontal leg thickness 3 and vertical leg thickness 4 then extrude 12",
        "Sketch an L in the XZ plane width 2 inch height 1 inch thickness 1/8 inch then pad 1/2 inch",
        "Make an L width 2 in height 1 in thickness 0.125 in then extrude 0.5 in",
        "Make a T-section width 40 height 30 flange thickness 4 stem thickness 6 then extrude 12",
        "Sketch a tee on XZ 40 mm wide and 30 mm tall with flange 4 mm and web 6 mm then pad 12 mm",
        "Please make a rail overall width 40 overall height 30 cap thickness 4 stem thickness 6, 12 mm deep",
        "Draw a T cross-section 40 by 30 mm with top thickness 4 mm and stem 6 mm, extrude it 12",
        "I need a tee flange width 4 cm height 3 cm flange thickness 0.4 cm stem thickness 0.6 cm, 1.2 cm long",
        "Make a T 40x30 mm flange 4 mm stem 6 mm then extrude 12 mm",
        "Sketch a T on the top plane width 40 height 30 flange 4 stem 6 then pad 12",
        "Sketch a tee on the right plane width 40 height 30 flange 4 stem 6 then extrude 12",
        "Make a rail width 2 inch height 1 inch flange 1/8 inch web 1/4 inch then pad 1/2 inch",
        "Make a T width 40 height 30 flange 4 stem 6 stem offset 15 mm from the left then extrude 12",
        "Make a U width 40 height 30 wall 3 then extrude 12",
        "Please sketch a channel 40 mm wide and 30 mm high thickness 3 mm, 12 mm deep",
        "Can you make a U-section on XZ 40x30 mm wall 3 mm then pad 12 mm?",
        "I need a channel on the front plane width 4 cm height 3 cm wall 0.3 cm, 1.2 cm long",
        "Make a U on the right plane width 40 height 30 web 3 flange 4 then extrude 12",
        "Draw a channel width 40 height 30 web thickness 3 and leg thickness 4 then pad 12",
        "Make a U 40 by 30 mm with 3 mm thick walls then extrude 12 mm",
        "Sketch a U width 2 inch height 1 inch wall 1/8 inch then extrude 1/2 inch",
        "Make a C-section width 40 height 30 wall 3 then extrude 12",
        "Please sketch a C-channel 40 mm wide and 30 mm high thickness 3 mm, 12 mm deep",
        "Can you make a C on XZ 40x30 mm wall 3 mm then pad 12 mm?",
        "I need a C-section on the front plane width 4 cm height 3 cm wall 0.3 cm, 1.2 cm long",
        "Make a C on the right plane width 40 height 30 web 3 flange 4 then extrude 12",
        "Draw a C-channel width 40 height 30 web thickness 3 and flange thickness 4 then pad 12",
        "Make a C 40 by 30 mm with 3 mm thick walls then extrude 12 mm",
        "Sketch a C width 2 inch height 1 inch wall 1/8 inch then extrude 1/2 inch",
        "Make a fully constrained rectangle width 40 height 30 then extrude 12",
        "Please sketch a dimensioned plate 40 mm wide and 30 mm high, 12 mm deep",
        "Can you make a fully constrained bar on XZ 40x30 mm then pad 12 mm?",
        "I need a dimensioned rectangle on the front plane width 4 cm height 3 cm, 1.2 cm long",
        "Make a fully constrained plate on the right plane width 40 height 30 then extrude 12",
        "Draw a dimensioned rectangle width 40 and height 30 then pad 12",
        "Make a fully constrained rectangle 40 by 30 mm then extrude 12 mm",
        "Sketch a dimensioned rectangle width 2 inch height 1 inch then extrude 1/2 inch",
        "Make a fully constrained rectangle width 2\" height 1\" then pad 1/2\"",
        "Sketch a U centred on the origin width 40 height 30 wall 3 then extrude 12",
        "Sketch a C centered on the origin width 40 height 30 web 3 flange 4 then extrude 12",
        "Make a T centred on the origin width 40 height 30 flange 4 stem 6 then pad 12",
        "Make a tee width 40 height 30 flange 4 stem 6 with the stem centred under the flange then pad 12",
        "Make a T width 40 height 30 flange 4 stem 6 stem offset 0 from the centre then extrude 12",
        "Make a T width 40 height 30 flange 4 stem 6 stem offset -3 from the centre then extrude 12",
        "Make a T flange 40 mm wide 30 mm high flange 4 mm thick and stem 6 mm thick then extrude 12",
        "Make a tee with its bottom left corner at the origin width 40 height 30 flange 4 stem 6 stem offset 3 from the centre then pad 12",
        "Sketch a fully constrained rectangular cross-section 40x30 mm then extrude 12 mm",
        "Make a dimensioned plate width 40 height 30 and 12 deep",
        "Make a U width 40 height 30 wall 3 opening down then extrude 12",
        "Make a C width 40 height 30 wall 3 opening left then extrude 12",
    ] {
        let p = plan(&cases::request(text), &catalog).unwrap().unwrap_or_else(|| panic!("{text}"));
        assert!(matches!(p.rule,"l_profile"|"closed_profile"),"{text}: {}",p.rule);
        assert_eq!(p.calls[1]["arguments"]["command"],"sketch.line","{text}");
        assert_eq!(p.calls[1]["arguments"]["params"]["closed"],true);
        assert_eq!(p.calls[1]["arguments"]["params"]["infer"],true);
    }
}
