//! Original phrasings, independent of benchmark evaluation data.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;
pub(crate) const CASES: &[(&str, &[&str])] = &[
("box", &[
"build a block 17 by 23 by 6 mm", "add a plate 3×4×0.5 cm", "create a cube 1/4 inch",
"make a box length 2 inches width 1 inch height 1/4 inch", "please build a 0.03 m cube", "make a 2\" cube"]),
("cylinder", &[
"build a rod diameter 7mm tall 29mm", "add a disc radius 13mm thick 2mm", "create a cylinder Ø 1/4 in high 1 inch",
"make a rod 2 cm across and 0.1 m tall", "please create a cylinder radius 3/4 inch height 2 inches"]),
("sphere", &[
"build a sphere radius 7mm", "add a sphere diameter 3cm", "create a sphere Ø 1/4 inch",
"make a sphere 2 inches across", "please build a sphere 0.01 m radius"]),
("plate_hole", &[
"build a plate 72x48x6 mm with a centred through hole diameter 8mm",
"add a plate 3x2x1/4 inches with a hole through the middle radius 1/4 inch",
"create a plate 72 by 48 by 6mm with one centered through hole Ø 8mm",
"make a plate 7.2x4.8x0.6 cm with a center through hole 0.8 cm diameter",
"please build a plate 72×48×6mm with a through hole in the centre diameter 8mm"]),
("plate_holes", &[
"build a plate 80x60x6mm with 2 through holes diameter 4mm at (12mm,15mm), (65mm,40mm)",
"add a plate 4x3x1/4 inches with 2 through holes radius 1/8 inch at (1inch,1inch), (3inch,2inch)",
"create a plate 80 by 60 by 6mm with 2 through holes Ø 4mm at (12,15), (65,40)",
"make a plate 80×60×6mm with 2 holes through diameter 4mm at (12mm,15mm), (65mm,40mm)",
"please build a plate 8x6x0.6cm with 2 through holes diameter 0.4cm at (1.2cm,1.5cm), (6.5cm,4cm)",
"build a plate 80x60x6mm with holes diameter 4mm through in a rectangular pattern 2 rows 3 columns pitch 20x20mm centered",
"add a plate 4x3x1/4 inches with holes radius 1/8 inch through in a rectangular pattern 2 rows 2 columns pitch 1x1 inch centred",
"create a plate 80x60x6mm with 6 through holes Ø 4mm in a rectangular pattern 2 rows 3 columns pitch 20 by 20mm centered",
"make a plate 8x6x0.6cm with holes diameter 0.4cm through in a rectangular pattern 2 rows 3 columns pitch 2×2cm centred",
"please build a plate 80x60x6mm with holes radius 2mm through in a rectangular pattern 2 rows 3 columns pitch 20x20mm centered",
"build a plate 80x60x6mm with 5 through holes diameter 4mm in a circular pattern radius 18mm centered equally spaced starting at +X",
"add a plate 4x3x1/4 inches with 4 through holes radius 1/8 inch in a circular pattern radius 1inch centred equally spaced starting at +X",
"create a plate 80x60x6mm with 6 through holes Ø 4mm in a circular pattern diameter 36mm centered equally spaced starting at +X",
"make a plate 8x6x0.6cm with 5 through holes diameter 0.4cm in a circular pattern radius 1.8cm centred equally spaced starting at +X",
"please build a plate 80x60x6mm with 5 holes through radius 2mm in a circular pattern radius 18mm centered equally spaced starting at +X"]),
("sketch_extrude", &[
"sketch a rectangle 13x21mm on XY then extrude 6mm", "create a sketch rectangle 1x2 inches on XZ then extrude 1/4 inch",
"please sketch a rectangle 1.3 by 2.1cm on YZ then extrude 0.6cm", "build a sketch rectangle 13×21mm on XY then extrude 6mm",
"make a sketch rectangle 0.013x0.021m on XZ then extrude 0.006m",
"sketch a circle radius 9mm on XY then extrude 7mm", "create a sketch circle Ø 1 inch on YZ then extrude 1/4 inch",
"please sketch a circle diameter 1.8cm on XZ then extrude 0.7cm", "build a sketch circle 18mm across on XY then extrude 7mm",
"make a sketch circle 0.009m radius on XZ then extrude 0.007m"]),
("fillet", &[
"fillet all edges of Blank radius 2mm", "please fillet all edges of the current body 1/16 inch",
"fillet Blank all edges radius 0.2cm", "fillet all edges of it radius 0.002m", "fillet all edges of Blank radius 0.08 inches"]),
("chamfer", &[
"chamfer all edges of Blank distance 2mm", "please chamfer all edges of the current body 1/16 inch",
"chamfer Blank all edges distance 0.2cm", "chamfer all edges of it distance 0.002m", "chamfer all edges of Blank distance 0.08 inches",
"chamfer top edges of Blank distance 2mm", "please chamfer top edges of the current body 1/16 inch",
"chamfer Blank top edges distance 0.2cm", "chamfer top edges of it distance 0.002m", "chamfer top edges of Blank distance 0.08 inches"]),
("move", &[
"translate Blank 7mm along X", "please move the body by 1/4 inch along y", "move it -2 inches along z",
"move Blank 0.7cm on x", "translate the current body 0.007m along Y"]),
("rename", &[
"rename Blank to Spacer", "please rename the body to Quarter-inch", "rename it to '1/4 inch stock'",
"rename the current body to Bearing", "rename Blank to `Rough blank`"]),
("mirror", &[
"mirror Blank about XY", "please mirror the body across XZ", "mirror it about YZ",
"mirror the current body across XY", "mirror Blank across YZ"]),
("shell", &[
"shell a box 70x50x30mm wall 2mm open top", "shell a box 3x2x1 inches wall 1/8 inch open top",
"please shell a box 7 by 5 by 3cm wall 0.2cm remove top", "create a shell box 70×50×30mm wall 2mm open top",
"build a shell box 0.07x0.05x0.03m wall 0.002m open top"]),
("revolve", &[
"revolve a rectangle radius 8mm height 35mm from axis on XZ about Z full turn",
"please revolve a rectangle radius 1/4 inch height 2 inches from axis on XZ about Z full turn",
"create a revolve rectangle radius 0.8cm height 3.5cm from axis on XZ about Z full turn",
"build a revolve rectangle radius 0.008m height 0.035m from axis on XZ about Z full turn",
"make a revolve rectangle radius 0.5 inches height 1 inch from axis on XZ about Z full turn"]),
("volume", &["report the volume", "give me the volume", "tell me the volume", "measure volume", "what is the volume of the design?"]),
("body_count", &["count the bodies", "count bodies", "how many bodies?", "tell me how many bodies are there", "how many bodies are in the design?"]),
];
pub(crate) fn design() -> Value {
    json!({"bodies":[{"name":"Blank","volume_mm3":1000,"edges":[
    {"index":0,"length":10,"start":[0,0,0],"mid":[5,0,0],"end":[10,0,0]},
    {"index":1,"length":10,"start":[0,0,10],"mid":[5,0,10],"end":[10,0,10]},
    {"index":2,"length":10,"start":[0,0,0],"mid":[0,0,5],"end":[0,0,10]}
    ]}]})
}
pub(crate) fn request(text: &str) -> Value {
    json!({"messages":[{"role":"tool","content":design().to_string()},{"role":"user","content":text}],"tools":[{"type":"function","function":{"name":"execute","parameters":{"type":"object","properties":{"command":{"type":"string"},"params":{"type":"object"}},"required":["command","params"]}}}]})
}
#[test]
fn every_recipe_has_independent_phrasings_and_refusals() {
    let c = Catalog::bundled().unwrap();
    for (rule, phrases) in CASES {
        assert!(phrases.len() >= 5);
        for phrase in *phrases {
            let p = plan(&request(phrase), &c)
                .unwrap()
                .unwrap_or_else(|| panic!("did not match {rule}: {phrase}"));
            assert_eq!(p.rule, *rule, "{phrase}");
            assert!(!p.slots.is_null());
            assert!(p.calls.len() <= 64);
            if !matches!(*rule, "volume" | "body_count") {
                for negative in [
                    format!("don't {phrase}"),
                    format!("explain how to {phrase}"),
                    format!("why would I {phrase}?"),
                    format!("{phrase} and move Blank 2mm along Z"),
                ] {
                    assert!(
                        plan(&request(&negative), &c).unwrap().is_none(),
                        "unsafe match: {negative}"
                    );
                }
            }
        }
    }
}
#[test]
fn missing_slots_and_ambiguous_intents_never_fire() {
    let c = Catalog::bundled().unwrap();
    for text in ["build a box", "create a rod radius 3mm", "add a sphere 7mm", "make a plate 80x60x6mm with a hole",
"make a plate 80x60x6mm with 2 holes diameter 4mm through at (12,15)",
"make a plate 80x60x6mm with holes diameter 4mm through in a rectangular pattern 2 rows 3 columns",
"make a plate 80x60x6mm with 4 holes diameter 4mm through in a circular pattern radius 10mm centered",
"sketch a rectangle 13x21mm then extrude 6mm", "sketch a circle on XY then extrude 6mm",
"fillet all edges of Blank", "chamfer edges of Blank 2mm", "move Blank 7mm", "rename Blank", "mirror Blank", "shell a box 70x50x30mm open top",
"revolve a rectangle 8x35mm", "build a 7mm cube and mirror it about XY", "create a sphere radius 7mm and a cube 7mm",
"make a 7mm cube or a sphere radius 7mm", "fillet all edges and chamfer top edges of Blank 2mm",
"make a cylinder radius 4mm diameter 8mm height 20mm", "make a 1/0 inch cube", "make a box 0x10x20mm",
"make a plate 80x60x6mm with 65 through holes diameter 1mm at (12,15)"] {
assert!(plan(&request(text),&c).unwrap().is_none(),"{text}");}
}
#[test]
fn numeric_slots_and_expected_geometry_are_exact() {
    let c = Catalog::bundled().unwrap();
    let p = plan(&request("create a cube 1/4 inch"), &c)
        .unwrap()
        .unwrap();
    assert_eq!(p.calls[0]["arguments"]["params"]["length"], 6.35);
    assert!((p.expected.unwrap().volume_mm3.unwrap() - 1256.047875).abs() < 1e-8);
    let p = plan(&request(CASES[4].1[5]), &c).unwrap().unwrap();
    assert_eq!(p.calls.len(), 8);
    assert_eq!(
        p.calls[1]["arguments"]["params"]["position"],
        json!([20.0, 20.0, 6.0])
    );
    assert_eq!(
        p.calls[6]["arguments"]["params"]["position"],
        json!([60.0, 40.0, 6.0])
    );
    let want = 1000.0 + 80.0 * 60.0 * 6.0 - 6.0 * std::f64::consts::PI * 4.0 * 6.0;
    assert!((p.expected.unwrap().volume_mm3.unwrap() - want).abs() < 1e-8);
}
#[test]
fn edge_lookup_continues_without_replaying_an_action() {
    let c = Catalog::bundled().unwrap();
    let mut req = request("chamfer top edges of Blank distance 2mm");
    req["messages"][0]["content"] =
        json!(json!({"bodies":[{"name":"Blank","volume_mm3":1000}]}).to_string());
    let p = plan(&req, &c).unwrap().unwrap();
    assert_eq!(p.calls.len(), 1);
    assert_eq!(p.calls[0]["arguments"]["command"], "model.edges");
    req["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","tool_calls":[{"id":"lookup","function":{"name":"execute","arguments":"{\"command\":\"model.edges\",\"params\":{\"body\":\"Blank\"}}"}}]}),
        json!({"role":"tool","tool_call_id":"lookup","content":json!({"body":"Blank","edges":design()["bodies"][0]["edges"]}).to_string()}),
    ]);
    assert!(plan(&req, &c).unwrap().is_none());
    let next = resume(&req, &p, &c).unwrap().unwrap();
    assert!(next.continuation.is_none());
    assert_eq!(next.calls[0]["arguments"]["command"], "solid.chamfer");
    assert_eq!(
        next.calls[0]["arguments"]["params"]["edges"],
        json!([{"body":"Blank","index":1}])
    );
    assert_eq!(next.calls[1]["arguments"]["command"], "document.inspect");
    assert!(matches_prose(
        &req,
        &c,
        &next,
        &json!({"content":"I will chamfer top edges of Blank distance 2mm"})
    )
    .unwrap());
    req["messages"].as_array_mut().unwrap().last_mut().unwrap()["content"] =
        json!("{\"body\":\"Blank\",\"edges\":[],\"truncated\":true}");
    assert!(resume(&req, &p, &c).unwrap().is_none());
}
#[test]
fn placement_units_negatives_and_partial_prose_are_conservative() {
    let c = Catalog::bundled().unwrap();
    let p = plan(&request("move Blank 1/4 inch along negative X"), &c)
        .unwrap()
        .unwrap();
    assert_eq!(
        p.calls[0]["arguments"]["params"]["translate"],
        json!([-6.35, 0.0, 0.0])
    );
    assert_eq!(
        plan(&request("make a plate 20x30mm 4mm thick"), &c)
            .unwrap()
            .unwrap()
            .calls[0]["arguments"]["params"]["height"],
        4.0
    );
    let mut req = request("move Blank 7mm along X");
    let original = json!({"bodies":[{"name":"Blank","volume_mm3":1000,"bbox":{"min":[0,0,0],"max":[10,10,10]}}]});
    req["messages"][0]["content"] = json!(original.to_string());
    let p = plan(&req, &c).unwrap().unwrap();
    assert!(completion(&req, p.expected.as_ref().unwrap())
        .unwrap()
        .contains("bounds"));
    let after = json!({"bodies":[{"name":"Blank","volume_mm3":1000,"bbox":{"min":[7,0,0],"max":[17,10,10]}}]});
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"tool","content":after.to_string()}));
    assert!(completion(&req, p.expected.as_ref().unwrap()).is_none());
    for text in ["make a 7mm cube with rounded corners", "make a 7mm cube centered at origin", "make a sphere radius 3mm with a flat bottom", "fillet all vertical edges of Blank radius 2mm", "chamfer top curved edges of Blank 2mm", "move Blank 2mm along X twice", "rename Blank to Spacer and group the bodies", "count bodies and mirror Blank about XY", "build a plate 80x60x6mm with 4 through holes diameter 4mm in a circular pattern radius 18mm centered equally spaced starting at -X"] {
        assert!(plan(&request(text),&c).unwrap().is_none(),"{text}");
    }
}
#[test]
fn lexical_collisions_and_incomplete_hole_counts_do_not_change_meaning() {
    let c = Catalog::bundled().unwrap();
    let mut req = request("move cm 2cm along X");
    req["messages"][0]["content"] =
        json!(json!({"bodies":[{"name":"cm","volume_mm3":1000}]}).to_string());
    let p = plan(&req, &c).unwrap().unwrap();
    assert_eq!(
        p.calls[0]["arguments"]["params"]["translate"],
        json!([20.0, 0.0, 0.0])
    );
    for text in ["make a 20mm cm cube", "build a plate 80x60x6mm with centered through holes diameter 4mm", "revolve a rectangle radius 8mm height 35mm from axis on XZ about Z and X full turn", "build a plate 80x60x6mm with 2 through holes diameter 4mm at (radius 12mm,15mm), (65mm,40mm)"] {
        assert!(plan(&request(text),&c).unwrap().is_none(),"{text}");
    }
}
#[test]
fn repeated_action_or_shape_is_not_a_single_recipe() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "make a box and a plate 20x30x4mm",
        "build a cylinder and a rod diameter 4mm height 20mm",
        "move Blank 2mm along X and move it",
        "sketch rectangle and rectangle 13x21mm on XY then extrude 6mm",
    ] {
        assert!(plan(&request(text), &c).unwrap().is_none(), "{text}");
    }
}
