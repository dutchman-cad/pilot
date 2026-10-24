//! Language cases, including the public cube-centre-hole field regression.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;

// Every row asserts the complete typed slots, not merely a non-empty plan.
fn groups() -> Vec<(&'static str, Value, Vec<&'static str>)> {
    vec![
        ("plate_hole", json!({"Holes":{"size":[20.0,20.0,20.0],"diameter":5.0,"centers":[[10.0,10.0]]}}), vec![
            "draw a 20mm cube with a 5mm hole through the center",
            "make a 20mm cube with a 5mm hole through the center",
            "draw a cube 20mm with a 5mm hole through the centre",
            "make a 20 mm cube with a 5mm hole through the middle",
            "draw a box 20x20x20mm with a 5mm through hole in the center",
            "draw a block 20x20x20mm with a centred 5mm through hole",
            "create a 20mm cube with a 5mm hole through the middle",
            "build a cube 20 mm with one centred 5mm through hole",
            "add a 20mm cube with a 5mm through hole in the centre",
            "draw a plate 20 by 20 by 20 mm with a centered 5mm through hole",
            "please draw a 2cm cube with a 0.5cm hole through the center",
            "make a cube 20mm with a centred through hole radius 2.5mm",
        ]),
        ("box", json!({"Box":{"size":[84.0,36.0,9.0]}}), vec![
            "ok so I need a solid brick 84x36x9mm thanks",
            "Length: 8.4cm, width: 3.6cm, height: 0.9cm. I want a block.",
            "a 9mm thick slab, 84 by 36",
            "Plate 84 by 36mm, 9mm thick",
            "could you give me a sheet 84x36x9 mm?",
            "pls a bar 84 mm long, 36 mm wide and 9 mm high",
            "I want width 36mm, height 9mm, length 84mm in a box",
            "add a block 0.084x0.036x0.009m please",
            "a 84mm by 36mm by 9mm plate for my project",
            "please build a 84 by 36 by 9 mm brick",
            "I need a block length: 84 width: 36 height: 9",
            "can you make a plate 9mm thick, 84mm long, 36mm wide?",
        ]),
        ("cylinder", json!({"Cylinder":{"radius":6.0,"height":57.0}}), vec![
            "a 12mm diameter rod, 57mm long", "rod length:57mm, diameter:12mm",
            "I need a cylinder 12mm wide and 57mm tall thanks",
            "pls give me a rod diameter 1.2cm and length 5.7cm",
            "could you add a cylinder radius 6mm high 57mm?",
            "ok so a cylinder 57mm high and 12mm across",
            "I want a rod radius:6mm, height:57mm", "cylinder 12mm in diameter, 57mm long",
            "make a cylinder radius 0.006m height 0.057m",
            "can you please build a rod 12mm across and 57mm long?",
            "please create a cylinder height 57mm diameter 12mm", "rod 57mm tall, 6mm radius",
        ]),
        ("sphere", json!({"Sphere":{"radius":6.0}}), vec![
            "I need a ball 12mm across", "please a sphere diameter:1.2cm thanks",
            "ok so a 6mm radius ball", "can you give me a ball radius 6 mm?",
            "pls add a sphere 12mm wide", "could you make a sphere 12mm in diameter?",
            "give me a ball with a radius of 0.006m", "ball 12mm diameter please",
            "create a sphere radius: 6", "I want a sphere 6mm radius",
            "please build a 12mm across sphere", "a ball diameter 12mm for my project",
        ]),
        ("plate_hole", json!({"Holes":{"size":[84.0,36.0,9.0],"diameter":6.0,"centers":[[42.0,18.0]]}}), vec![
            "a plate 84x36x9mm with one centred 6mm through hole",
            "I need a plate 84 by 36 by 9 mm with a 6mm through hole in the middle",
            "pls a sheet 84x36x9mm with one centered through hole diameter 6mm",
            "plate 84x36x9mm with a through hole 6mm across in the centre",
            "give me a slab 84x36x9mm with a centre through hole radius 3mm",
            "can you build a plate 84x36x9mm with a middle through hole 6mm diameter?",
            "please add a plate 84x36x9mm with one centred through hole radius 3mm thanks",
            "a plate 8.4x3.6x0.9cm with a centred 0.6cm through hole",
            "could you create a sheet 84x36x9mm with one centred 6mm through hole?",
            "plate length:84 width:36 height:9 with a 6mm through hole in the middle",
            "a 9mm thick plate 84 by 36 with a centred 6mm through hole",
            "ok so make a plate 84x36x9 with a centered through hole diameter:6",
        ]),
        ("plate_holes", json!({"Holes":{"size":[84.0,60.0,9.0],"diameter":4.0,"centers":[[32.0,22.0],[52.0,22.0],[32.0,38.0],[52.0,38.0]]}}), vec![
            "I need a plate 84x60x9mm with holes diameter 4mm through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centered",
            "pls a plate 84x60x9mm with holes radius 2mm through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centred",
            "could you add a plate 84x60x9mm with 4 through holes diameter 4mm in a rectangular pattern 2 rows 2 columns pitch 20x16mm centered?",
            "a plate 8.4x6x0.9cm with holes diameter 0.4cm through in a rectangular pattern 2 rows 2 columns pitch 2x1.6cm centred",
            "ok so a plate 84x60x9 with holes diameter 4 through in a rectangular pattern 2 rows 2 columns pitch 20x16 centered thanks",
            "give me a sheet 84x60x9mm with holes radius 2mm through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centred",
            "a plate length 84 width 60 thickness 9 with holes diameter 4 through in a rectangular pattern 2 rows 2 columns pitch 20x16 centered",
            "please a slab 84x60x9mm with holes diameter 4mm through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centred",
            "can you make a plate 84x60x9mm with holes 4mm across through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centered?",
            "I want a plate 84x60x9mm with holes 4mm in diameter through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centred",
            "a 9mm thick plate 84 by 60 with holes diameter 4mm through in a rectangular pattern 2 rows 2 columns pitch 20x16mm centered",
            "plate 84x60x9mm with 4 through holes radius 2mm in a rectangular pattern 2 rows 2 columns pitch 20x16mm centred please",
        ]),
        ("tube", json!({"Tube":{"outer_radius":10.0,"inner_radius":7.0,"height":60.0}}), vec![
            "a hollow tube OD 20mm ID 14mm length 60mm",
            "I need a tube outer diameter 20mm inner diameter 14mm length 60mm",
            "tube length:60mm, OD:20mm, ID:14mm", "tube OD 2cm ID 1.4cm height 6cm",
            "please build a tube outer diameter 20mm wall thickness 3mm length 60mm",
            "give me a hollow tube OD 20mm wall 3mm height 60mm",
            "could you add a tube ID 14mm OD 20mm length 60mm?",
            "pls a tube 20mm OD 14mm ID 60mm long",
            "tube 60mm long and 20mm OD and 3mm wall",
            "ok so a tube OD:20 ID:14 length:60 thanks",
            "can you make a hollow tube OD 0.02m ID 0.014m length 0.06m?",
            "a tube 20mm outside diameter, 14mm inside diameter and 60mm long",
        ]),
        ("hole", json!({"Hole":{"body":"Blank","diameter":4.0,"position":[3.0,4.0,10.0]}}), vec![
            "pls drill a hole 4mm across through this box at (3,4,10)",
            "I need you to bore a hole diameter 4mm through it at (3,4,10)",
            "can you cut a hole radius 2mm through Blank at (3,4,10)?",
            "ok so drill a hole diameter 0.4cm through the body at (3,4,10)",
            "bore a hole 4mm diameter through the part at (3,4,10)",
            "please drill a hole radius 0.002m through Blank at (3,4,10)",
            "could you bore a hole diameter 4mm through the box at (3mm,4mm,10mm)?",
            "cut a hole 4mm in diameter through the body at (3,4,10)",
            "give me a drill hole diameter 4mm through Blank at (3,4,10)",
            "I want you to bore a hole radius 2mm through Blank at (3,4,10)",
            "please cut a hole 0.4cm across through it at (3,4,10)",
            "drill a hole diameter:4mm through this body at (3,4,10)",
        ]),
        ("shell", json!({"Shell":{"size":[80.0,40.0,20.0],"wall":2.0}}), vec![
            "pls shell a box 80x40x20mm wall 2mm open top",
            "I need a shell box 80x40x20mm wall 2mm remove top",
            "could you shell a box 8x4x2cm wall 0.2cm open top?",
            "shell a box length:80 width:40 height:20 wall:2 open top",
            "ok so shell a box 80x40x20mm wall 2mm open top thanks",
            "give me a shell box 80x40x20mm wall 2mm remove top",
            "please shell a box 0.08x0.04x0.02m wall 0.002m open top",
            "can you build a shell box 80x40x20mm wall 2mm open top?",
            "shell a box 80mm long, 40mm wide, 20mm high wall 2mm open top",
            "I want a shell box 80 by 40 by 20mm wall 2mm open top",
            "shell a box width 40mm length 80mm height 20mm wall 2mm remove top",
            "add a shell box 80x40x20mm wall 2mm open top please",
        ]),
        ("revolve", json!({"Revolve":{"radius":8.0,"height":30.0}}), vec![
            "pls revolve a rectangle radius 8mm height 30mm from axis on XZ about Z full turn",
            "I need a revolve rectangle radius 8mm height 30mm from axis on XZ about Z full turn",
            "could you revolve a rectangle radius 0.8cm height 3cm from axis on XZ about Z full turn?",
            "revolve a rectangle radius:8 height:30 from axis on XZ about Z full turn",
            "ok so revolve a rectangle radius 8mm height 30mm from axis on XZ about Z full turn thanks",
            "give me a revolve rectangle radius 8mm height 30mm from axis on XZ about Z full turn",
            "please revolve a rectangle radius 0.008m height 0.03m from axis on XZ about Z full turn",
            "can you build a revolve rectangle radius 8mm height 30mm from axis on XZ about Z full turn?",
            "revolve a rectangle 8mm radius, 30mm high from axis on XZ about Z full turn",
            "I want a revolve rectangle radius 8 height 30 from axis on XZ about Z full turn",
            "revolve a rectangle radius 8mm height 30mm from axis on XZ about Z full turn please",
            "add a revolve rectangle radius 8mm height 30mm from axis on XZ about Z full turn",
        ]),
        ("sketch_extrude", json!({"Sketch":{"plane":"XY","size":[20.0,10.0],"distance":5.0}}), vec![
            "pls sketch a rectangle 20x10mm on XY then extrude 5mm",
            "I need a rectangle 20 by 10mm on XY then extrude by 5mm",
            "could you sketch a rectangle 2x1cm on XY then raise 0.5cm?",
            "rectangle 20x10mm on XY then pull it up 5mm",
            "ok so sketch a rectangle 20x10mm on XY then extrude 5mm thanks",
            "give me a sketch rectangle 20x10mm on XY then extrude 5mm",
            "please sketch a rectangle 0.02x0.01m on XY then extrude 0.005m",
            "can you build a sketch rectangle 20x10mm on XY then extrude 5mm?",
            "sketch a rectangle 20mm by 10mm on XY then extrude 5mm",
            "I want a rectangle 20x10mm on XY then extrude 5mm",
            "a rectangle 20 by 10 on XY then raise 5mm please",
            "add a sketch rectangle 20x10mm on XY then pull it up 5mm",
        ]),
        ("sketch_extrude", json!({"Sketch":{"plane":"XY","size":[5.0],"distance":5.0}}), vec![
            "pls sketch a circle radius 5mm on XY then extrude 5mm",
            "I need a circle 10mm across on XY then extrude by 5mm",
            "could you sketch a circle diameter 1cm on XY then raise 0.5cm?",
            "circle radius 5mm on XY then pull it up 5mm",
            "ok so sketch a circle radius 5mm on XY then extrude 5mm thanks",
            "give me a sketch circle diameter 10mm on XY then extrude 5mm",
            "please sketch a circle radius 0.005m on XY then extrude 0.005m",
            "can you build a sketch circle radius 5mm on XY then extrude 5mm?",
            "sketch a circle 5mm radius on XY then extrude 5mm",
            "I want a circle 10mm in diameter on XY then extrude 5mm",
            "a circle radius 5 on XY then raise 5mm please",
            "add a sketch circle radius 5mm on XY then pull it up 5mm",
        ]),
        ("volume", json!({"query":"volume"}), vec![
            "what's the volume?", "how many cubic millimetres are in this design?",
            "what is its volume in cubic inches?", "please report volume in cm3",
            "can you give me the volume?", "I need volume in cubic metres",
            "ok so measure the volume thanks", "tell me the volume in mm3",
            "give me volume please", "volume in cubic centimeters please",
            "how many cubic millimeters?", "report the volume of all bodies",
        ]),
        ("body_count", json!({"query":"body_count"}), vec![
            "how many solids are there?", "how many bodies are in here?",
            "please count bodies thanks", "could you count the bodies?",
            "I need you to count bodies", "tell me how many solids exist",
            "ok so count the bodies", "pls count bodies", "how many bodies have we got?",
            "can you tell me how many bodies there are?", "how many solids in this design?",
            "I want to count the bodies",
        ]),
        ("move", json!({"Move":{"body":"Blank","delta":[0.0,-16.0,0.0]}}), vec![
            "please move this box 16mm in the negative Y direction",
            "I need you to shift it 16mm along -Y", "slide the body 16mm down Y thanks",
            "can you translate it -16mm along Y?", "ok so move Blank 1.6cm along negative Y",
            "could you slide the part 16mm on negative Y?", "pls shift the box -0.016m on Y",
            "move Blank 16mm along -Y for my project", "give me a move of Blank -16mm along Y",
            "translate the current body 16mm negative Y", "please shift it -16mm along Y",
            "move the part by 16mm down Y",
        ]),
        ("rename", json!({"Rename":{"body":"Blank","name":"Spacer"}}), vec![
            "Call this body Spacer", "name it Spacer", "rename to Spacer", "please rename it to Spacer",
            "could you name the body Spacer?", "can you call it Spacer?", "pls call Blank Spacer",
            "I need you to rename Blank to Spacer", "ok so name the box Spacer", "rename this box to Spacer",
            "give me a rename of Blank to Spacer", "please call the current body Spacer",
        ]),
        ("mirror", json!({"Mirror":{"body":"Blank","plane":"XZ","keep_original":true}}), vec![
            "mirror it across XZ keeping the original", "please mirror this box across the XZ plane",
            "could you mirror Blank about XZ?", "I need a mirror of Blank across XZ",
            "pls mirror the body across XZ keep original", "ok so mirror it across XZ thanks",
            "mirror the part about XZ", "can you mirror the box across XZ?", "mirror Blank across plane XZ",
            "please mirror the current body about XZ", "give me a mirror of Blank across XZ",
            "mirror Blank about the XZ plane keeping original",
        ]),
        ("scale", json!({"Scale":{"body":"Blank","factor":1.5}}), vec![
            "scale it about the origin by 150%", "please scale this box by 1.5 about origin",
            "could you scale Blank factor 1.5 about the origin?", "scale the body uniformly by 150 percent about origin",
            "I need a scale of Blank by 1.5 about origin", "pls scale it 150% about origin thanks",
            "can you scale the part by 1.5 around origin?", "scale Blank by 150% about origin",
            "ok so scale the box about origin factor 1.5", "give me a scale of Blank about origin by 150%",
            "scale the current body about origin by 1.5", "please scale it about origin by 150 percent",
        ]),
        ("fillet", json!({"Edges":{"body":"Blank","edges":[{"body":"Blank","index":0},{"body":"Blank","index":1},{"body":"Blank","index":2}],"amount":2.0,"fillet":true,"top":false}}), vec![
            "round every edge of this box 2mm", "fillet all the edges of Blank radius 2mm",
            "please round off all edges of it 0.2cm", "can you fillet the body all edges 2mm?",
            "give me a 2mm round on all edges of Blank", "pls round all edges of the part radius 2mm",
            "fillet Blank every edge 2mm", "round the box all the edges 2mm",
            "I need you to fillet all edges of Blank radius 0.002m", "ok so round all edges of it radius 2mm",
            "could you fillet the current body all edges 2mm?", "add a 2mm round to all edges of Blank",
        ]),
        ("chamfer", json!({"Edges":{"body":"Blank","edges":[{"body":"Blank","index":0},{"body":"Blank","index":1},{"body":"Blank","index":2}],"amount":2.0,"fillet":false,"top":false}}), vec![
            "bevel every edge of this box 2mm", "chamfer all the edges of Blank distance 2mm",
            "please bevel all edges of it 0.2cm", "can you chamfer the body all edges 2mm?",
            "give me a 2mm chamfer on all edges of Blank", "pls bevel all edges of the part distance 2mm",
            "chamfer Blank every edge 2mm", "bevel the box all the edges 2mm",
            "I need you to chamfer all edges of Blank distance 0.002m", "ok so bevel all edges of it distance 2mm",
            "could you chamfer the current body all edges 2mm?", "add a 2mm chamfer to all edges of Blank",
        ]),
    ]
}

#[test]
fn independent_exact_slots() {
    let c = Catalog::bundled().unwrap();
    for (rule, slots, phrases) in groups() {
        assert!(phrases.len() >= 12);
        for phrase in phrases {
            let p = plan(&cases::request(phrase), &c)
                .unwrap()
                .unwrap_or_else(|| panic!("{phrase}"));
            assert_eq!(p.rule, rule, "{phrase}");
            assert_eq!(p.slots, slots, "{phrase}");
        }
    }
}

#[test]
fn unsupported_requests_stay_with_model() {
    let c = Catalog::bundled().unwrap();
    let negatives = [
        "draw a 20mm cube with a 5mm hole",
        "draw a 20mm cube with a 5mm deep hole through the center",
        "draw a 20mm cube with a blind 5mm hole through the center",
        "draw a 20mm cube with a countersunk 5mm hole through the center",
        "draw a 20mm cube with two 5mm holes through the center",
        "draw a 20mm cube with a 5mm through hole in the corner",
        "how do I draw a 20mm cube with a 5mm hole through the center",
        "do not draw a 20mm cube with a 5mm hole through the center",
        "draw a 20mm cube and make a sphere radius 5mm",
        "make a cube and fillet all edges of it 2mm",
        "add a box then mirror it across XY",
        "a cube then move it 2mm along X",
        "a cube 12mm without keeping the original",
        "you can't create a 12mm cube",
        "you shouldn't count bodies",
        "please don't make a ball radius 6mm",
        "I won't move it 2mm along X",
        "rename it and delete the box to Foo",
        "how many bodies and a ball diameter 12mm",
        "no cube 12mm",
        "avoid a ball radius 6mm",
        "a cube 12mm and describe its shape",
        "instructions for a cube 12mm",
        "a cube 12mm with an explanation",
        "how do I make a 12mm cube",
        "why make a 12mm cube",
        "explain a 12mm cube",
        "do not create a ball diameter 12mm",
        "don't move it 4mm along X",
        "never rename it to Foo",
        "a block 20 by 30",
        "a rod 12mm wide",
        "a ball",
        "a cube",
        "a plate 12x9mm",
        "a cube 12mm cm",
        "a cube 12cm inches",
        "a ball diameter 2in mm",
        "a cube 1 1/2in",
        "a cube 1/0in",
        "a cube -12mm",
        "a cube 1000001mm",
        "a cube 0mm",
        "a ball radius 2mm diameter 4mm",
        "box length 12 width 9",
        "box length 12 width 9 width 3",
        "a cube 12mm and a ball radius 4mm",
        "a cube 12mm with a fillet 2mm",
        "a cube 12mm with chamfer 2mm",
        "a hollow box 12x9x3mm",
        "a tube OD 20mm length 60mm",
        "tube OD 20 ID 24 length 60",
        "tube OD 20 wall 10 length 60",
        "tube OD 20 ID 14 wall 3 length 60",
        "tube diameter 20 length 60",
        "scale it about origin",
        "scale it by 0 about origin",
        "scale it by -2 about origin",
        "scale it by 2 and 3 about origin",
        "scale it by 2 about origin and move it 3mm on X",
        "scale it 2mm about origin",
        "mirror it",
        "mirror it across XY and YZ",
        "mirror it across XY and rename to Foo",
        "round all edges of it",
        "fillet top edges of it 2mm",
        "chamfer curved edges of Blank 2mm",
        "a plate 84x36x9mm with a 6mm hole",
        "a plate 84x36x9mm with a through hole in the middle",
        "a plate 84x36x9mm with a centred blind hole 6mm",
        "a plate 84x36x9mm with two centred 6mm through holes",
        "move it 16mm along X and Y",
        "move it 16mm",
        "move Unknown 16mm on X",
        "move it -16mm negative Y",
        "rename it",
        "name it Foo and Bar",
        "what is the volume and move it 2mm on X",
        "how to measure volume",
        "why is the volume small",
        "do not count bodies",
        "count bodies without changing anything",
        "explain how many bodies",
        "undo move it 2mm on X",
        "a cube 12mm instead of a ball",
        "make a cube 12mm except the top",
        "make a cube 12mm with rounded corners",
        "a ball radius 6mm with flat bottom",
    ];
    assert!(negatives.len() >= 50);
    for text in negatives {
        assert!(plan(&cases::request(text), &c).unwrap().is_none(), "{text}");
    }
}

#[test]
fn composed_filler_units_and_order_preserve_create_slots() {
    let c = Catalog::bundled().unwrap();
    for filler in ["", "ok so I need ", "could you please give me ", "pls add "] {
        for (unit, a, b, z) in [
            ("mm", "20", "10", "5"),
            ("cm", "2", "1", "0.5"),
            ("m", "0.02", "0.01", "0.005"),
        ] {
            for reverse in [false, true] {
                let size = if reverse {
                    format!("{z}{unit} thick, {a} by {b}{unit}")
                } else {
                    format!("{a}x{b}x{z}{unit}")
                };
                let radial = if reverse {
                    format!("{a}{unit} tall, diameter {b}{unit}")
                } else {
                    format!("diameter {b}{unit} height {a}{unit}")
                };
                for (body, expected) in [
                    (format!("a plate {size}"), json!({"Box":{"size":[20.0,10.0,5.0]}})),
                    (format!("a rod {radial}"), json!({"Cylinder":{"radius":5.0,"height":20.0}})),
                    (format!("a ball {b}{unit} across"), json!({"Sphere":{"radius":5.0}})),
                    (format!("a tube OD {a}{unit} ID {b}{unit} length {z}{unit}"), json!({"Tube":{"outer_radius":10.0,"inner_radius":5.0,"height":5.0}})),
                    (format!("shell a box {size} wall 1mm open top"), json!({"Shell":{"size":[20.0,10.0,5.0],"wall":1.0}})),
                    (format!("revolve a rectangle radius {b}{unit} height {a}{unit} from axis on XZ about Z full turn"), json!({"Revolve":{"radius":10.0,"height":20.0}})),
                    (format!("sketch a rectangle {b}x{a}{unit} on XY then extrude {z}{unit}"), json!({"Sketch":{"plane":"XY","size":[10.0,20.0],"distance":5.0}})),
                    (format!("sketch a circle diameter {b}{unit} on XY then extrude {z}{unit}"), json!({"Sketch":{"plane":"XY","size":[5.0],"distance":5.0}})),
                    (format!("a plate {size} with 2 through holes diameter 1mm at (5,5), (15,5)"), json!({"Holes":{"size":[20.0,10.0,5.0],"diameter":1.0,"centers":[[5.0,5.0],[15.0,5.0]]}})),
                    (format!("a plate {size} with holes diameter 1mm through in a rectangular pattern 1 rows 2 columns pitch 10x1mm centered"), json!({"Holes":{"size":[20.0,10.0,5.0],"diameter":1.0,"centers":[[5.0,5.0],[15.0,5.0]]}})),
                    (format!("a plate {size} with 2 through holes diameter 1mm in a circular pattern radius 5mm centred equally spaced starting at +X"), json!({"Holes":{"size":[20.0,10.0,5.0],"diameter":1.0,"centers":[[15.0,5.0],[5.0,5.000000000000001]]}})),
                    (format!("a plate {size} with a centred {z}{unit} through hole"), json!({"Holes":{"size":[20.0,10.0,5.0],"diameter":5.0,"centers":[[10.0,5.0]]}})),
                ] {
                    let phrase = format!("{filler}{body} thanks");
                    let p = plan(&cases::request(&phrase), &c).unwrap().unwrap_or_else(|| panic!("{phrase}"));
                    assert_eq!(p.slots, expected, "{phrase}");
                }
            }
        }
    }
}

#[test]
fn fractions_discs_scale_and_mirror_options() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "a 1/2in diameter ball",
        "a ball diameter 1/2 inch",
        "please a sphere 0.5\" across",
    ] {
        assert_eq!(
            plan(&cases::request(text), &c).unwrap().unwrap().slots,
            json!({"Sphere":{"radius":6.35}})
        );
    }
    for noun in ["disc", "puck", "coin", "disk"] {
        for prefix in ["a round", "please make a", "I need a"] {
            let text = format!("{prefix} {noun} 20mm across and 4mm thick");
            assert_eq!(
                plan(&cases::request(&text), &c).unwrap().unwrap().slots,
                json!({"Cylinder":{"radius":10.0,"height":4.0}})
            );
        }
    }
    for (word, factor) in [("double", 2.0_f64), ("half", 0.5_f64)] {
        let p = plan(&cases::request(&format!("{word} it about origin")), &c)
            .unwrap()
            .unwrap();
        assert_eq!(p.slots, json!({"Scale":{"body":"Blank","factor":factor}}));
        assert_eq!(
            p.expected.unwrap().volume_mm3,
            Some(1000.0 * factor.powi(3))
        );
    }
    for wording in [
        "not keeping the original",
        "without keeping the original",
        "do not keep the original",
    ] {
        let text = format!("mirror it across XZ {wording}");
        let p = plan(&cases::request(&text), &c).unwrap().unwrap();
        assert_eq!(
            p.slots,
            json!({"Mirror":{"body":"Blank","plane":"XZ","keep_original":false}})
        );
        assert_eq!(
            p.calls[1]["arguments"],
            json!({"command":"solid.remove","params":{"bodies":["Blank"]}})
        );
        assert_eq!(p.expected.unwrap().bodies, Some(1));
    }
}

#[test]
fn body_prerequisite_is_bounded_and_direct_only() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "move this box 8mm along X",
        "rename to Spacer",
        "mirror it across XZ",
        "double it about origin",
        "round all the edges of the body 2mm",
    ] {
        let mut req = cases::request(text);
        req["messages"].as_array_mut().unwrap().remove(0);
        assert!(plan(&req, &c).unwrap().is_none(), "{text}");
        let p = inspect_for_edit(&req, &c)
            .unwrap()
            .unwrap_or_else(|| panic!("{text}"));
        assert_eq!(
            p.calls,
            vec![
                json!({"tool":"execute","arguments":{"command":"document.inspect","params":{"measure":true}}})
            ]
        );
        req["messages"].as_array_mut().unwrap().push(json!({"role":"assistant","tool_calls":[{"id":"pilot_direct_0_resolve_body_0","function":{"name":"execute","arguments":json!({"command":"document.inspect","params":{"measure":true}}).to_string()}}]}));
        assert!(inspect_for_edit(&req, &c).unwrap().is_none());
        for bodies in [json!([]), json!([{"name":"A"},{"name":"B"}])] {
            let mut ambiguous = req.clone();
            ambiguous["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":json!({"bodies":bodies}).to_string()}));
            assert!(plan_direct(&ambiguous, &c).unwrap().is_none(), "{text}");
            assert!(inspect_for_edit(&ambiguous, &c).unwrap().is_none());
        }
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":cases::design().to_string()}));
        assert!(
            plan(&req, &c).unwrap().is_none(),
            "plan-text must retain acted guard"
        );
        assert!(plan_direct(&req, &c).unwrap().is_some(), "{text}");
    }
    for text in [
        "move it along X",
        "round the edges 2mm",
        "mirror it",
        "rename it",
        "scale it about origin",
        "how do I move it 8mm along X",
        "move it 8mm on X and rename to New",
    ] {
        let mut req = cases::request(text);
        req["messages"].as_array_mut().unwrap().remove(0);
        assert!(inspect_for_edit(&req, &c).unwrap().is_none(), "{text}");
    }
}

#[test]
fn normalization_preserves_names_and_removes_trailing_politeness() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "please name it Spacer thanks",
        "I need you to rename it to Spacer please",
        "call this body Spacer, thanks",
    ] {
        assert_eq!(
            plan(&cases::request(text), &c).unwrap().unwrap().slots,
            json!({"Rename":{"body":"Blank","name":"Spacer"}})
        );
    }
    assert!(plan(
        &cases::request("Please sketch a rectangle 20x10mm on FixtureDatum then extrude 5mm"),
        &c,
    )
    .unwrap()
    .is_none());
    let mut req = cases::request("Move RaisedPart 2mm along X");
    req["messages"][0]["content"] = json!(json!({"bodies":[{"name":"RaisedPart"}]}).to_string());
    assert_eq!(
        plan(&req, &c).unwrap().unwrap().slots["Move"]["body"],
        "RaisedPart"
    );
}

#[test]
fn field_cube_hole_expansion_remains_targeted_at_top_centre() {
    let c = Catalog::bundled().unwrap();
    let p = plan(
        &cases::request("draw a 20mm cube with a 5mm hole through the center"),
        &c,
    )
    .unwrap()
    .unwrap();
    assert_eq!(p.calls[0]["arguments"]["command"], "solid.box");
    assert_eq!(p.calls[0]["arguments"]["params"]["length"], 20.0);
    assert_eq!(
        p.calls[1]["arguments"],
        json!({"command":"solid.hole","params":{"position":[10.0,10.0,20.0],"diameter":5.0,"targets":["PilotRecipe1"]}})
    );
}
