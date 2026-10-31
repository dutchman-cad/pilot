//! Development edit requests on existing designs built through the public CLI.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{json, Value};
#[path = "support/edit_harness.rs"]
mod edit_harness;

fn box_(l: f64, w: f64, h: f64) -> Value {
    json!({"command":"solid.box","params":{"length":l,"width":w,"height":h}})
}
fn hole(x: f64, y: f64, z: f64, d: f64) -> Value {
    json!({"command":"solid.hole","params":{"position":[x,y,z],"diameter":d}})
}
fn plate() -> Vec<Value> {
    vec![box_(60.0, 40.0, 10.0), hole(30.0, 20.0, 10.0, 6.0)]
}
fn two_holes() -> Vec<Value> {
    vec![
        box_(80.0, 30.0, 8.0),
        hole(20.0, 15.0, 8.0, 5.0),
        hole(60.0, 15.0, 8.0, 5.0),
    ]
}
fn pegs() -> Vec<Value> {
    vec![
        json!({"command":"solid.cylinder","params":{"radius":5,"height":20,"base":[100,0,0]}}),
        json!({"command":"solid.sphere","params":{"radius":7,"center":[200,0,0]}}),
    ]
}
fn extruded() -> Vec<Value> {
    vec![
        json!({"command":"sketch.create","params":{"plane":"XY"}}),
        json!({"command":"sketch.rectangle.two_point","params":{"p0":[0,0],"p1":[10,5]}}),
        json!({"command":"solid.extrude","params":{"distance":3,"operation":"new_body"}}),
    ]
}
fn blind() -> Vec<Value> {
    vec![
        box_(50.0, 50.0, 20.0),
        json!({"command":"solid.hole","params":{"position":[25,25,20],"diameter":8,"depth":6}}),
    ]
}
fn many() -> Vec<Value> {
    let mut v = vec![box_(100.0, 100.0, 10.0)];
    for i in 0..14 {
        v.push(hole(5.0 + 6.0 * i as f64, 50.0, 10.0, 3.0));
    }
    v
}

fn fact(t: &edit_harness::Turn) -> Option<String> {
    if t.upstream.is_null() {
        return None;
    }
    edit_harness::systems(&t.upstream)
        .into_iter()
        .find(|s| s.contains("Design:"))
}

/// (design, request, substrings the fact must contain, applied edit: feature, input, value)
type Design = fn() -> Vec<Value>;
type Case = (
    fn() -> Vec<Value>,
    &'static str,
    &'static [&'static str],
    Option<(&'static str, &'static str, &'static str)>,
);

const B1: &str = "Body1 box 60 x 40 x 10 mm at X 0..60, Y 0..40, Z 0..10";
const PLATE_FEATURES: &str =
    "Box1 (length 60, width 40, height 10), Hole1 (diameter 6, through, at (30, 20, 10))";

fn cases() -> Vec<Case> {
    vec![
        (
            plate,
            "make the hole 8 mm instead",
            &[B1, PLATE_FEATURES, "timeline.edit", "\"diameter\":\"8\""],
            Some(("Hole1", "diameter", "8")),
        ),
        (
            plate,
            "change the hole diameter to 9",
            &[B1, "\"diameter\":\"9\""],
            Some(("Hole1", "diameter", "9")),
        ),
        (
            plate,
            "make the hole 0.5 cm instead",
            &["\"diameter\":\"5\""],
            Some(("Hole1", "diameter", "5")),
        ),
        (
            plate,
            "make the box 80 mm long instead",
            &["\"feature\":\"Box1\"", "\"length\":\"80\""],
            Some(("Box1", "length", "80")),
        ),
        (
            plate,
            "resize the plate height to 12 mm",
            &["\"height\":\"12\""],
            Some(("Box1", "height", "12")),
        ),
        (
            plate,
            "make the existing part 50 mm wide",
            &["\"width\":\"50\""],
            Some(("Box1", "width", "50")),
        ),
        (
            plate,
            "set Hole1 diameter to 4 mm",
            &["\"feature\":\"Hole1\"", "\"diameter\":\"4\""],
            Some(("Hole1", "diameter", "4")),
        ),
        (
            two_holes,
            "make Hole2 7 mm instead",
            &[
                "Hole1 (diameter 5, through, at (20, 15, 8))",
                "Hole2 (diameter 5, through, at (60, 15, 8))",
                "\"feature\":\"Hole2\"",
                "\"diameter\":\"7\"",
            ],
            Some(("Hole2", "diameter", "7")),
        ),
        (
            two_holes,
            "make the hole 7 mm instead",
            &["Hole2", "To resize an existing feature"],
            None,
        ),
        (
            two_holes,
            "drill a 10 mm hole through the centre of it",
            &[
                "Body1 box 80 x 30 x 8 mm at X 0..80, Y 0..30, Z 0..8",
                "To resize an existing feature",
            ],
            None,
        ),
        (
            two_holes,
            "make it a row of 6 of those holes",
            &["Hole1", "Hole2"],
            None,
        ),
        (
            two_holes,
            "chamfer the four top edges 2 mm",
            &["Body1 box"],
            None,
        ),
        (
            plate,
            "add two mounting tabs on the ends",
            &[B1, PLATE_FEATURES],
            None,
        ),
        (plate, "add a fillet to it", &[B1], None),
        (
            plate,
            "add a slot along the top of the existing part",
            &["Z 0..10"],
            None,
        ),
        (
            plate,
            "make it 3 mm thicker and keep the hole",
            &["Box1"],
            None,
        ),
        (plate, "add a second hole to the part", &["Hole1"], None),
        (
            pegs,
            "make the cylinder 30 mm tall instead",
            &[
                "Body1 cylinder 10 x 10 x 20 mm",
                "Body2 sphere 14 x 14 x 14 mm",
                "Cylinder1 (radius 5, height 20, at (100, 0, 0))",
                "\"height\":\"30\"",
            ],
            Some(("Cylinder1", "height", "30")),
        ),
        (
            pegs,
            "change the sphere radius to 9 mm",
            &["Sphere1 (radius 7, at (200, 0, 0))", "\"radius\":\"9\""],
            Some(("Sphere1", "radius", "9")),
        ),
        (
            pegs,
            "make the cylinder radius 6 instead",
            &["\"radius\":\"6\""],
            Some(("Cylinder1", "radius", "6")),
        ),
        (
            pegs,
            "move the existing sphere onto the cylinder",
            &["Sphere1"],
            None,
        ),
        (
            extruded,
            "make the extrusion 5 mm instead",
            &[
                "Body1 10 x 5 x 3 mm",
                "Sketch1",
                "Extrude1 (distance 3)",
                "extent.distance",
            ],
            Some(("Extrude1", "distance", "5")),
        ),
        (
            extruded,
            "add a hole to the existing part",
            &["Extrude1"],
            None,
        ),
        (
            blind,
            "make the hole 10 mm instead",
            &[
                "Hole1 (diameter 8, depth 6, at (25, 25, 20))",
                "\"diameter\":\"10\"",
            ],
            Some(("Hole1", "diameter", "10")),
        ),
        (
            blind,
            "make the hole depth 9 mm instead",
            &["\"depth\":\"9\""],
            Some(("Hole1", "depth", "9")),
        ),
        (
            blind,
            "make the hole 6 mm and 12 mm deep instead",
            &["To resize an existing feature"],
            None,
        ),
        (
            many,
            "make the holes bigger instead",
            &["last 12 of 15"],
            None,
        ),
        (many, "add another hole to the plate", &["Hole14"], None),
    ]
}

#[test]
fn edit_requests_get_a_design_fact_and_a_working_edit_command() {
    let Some(cli) = edit_harness::cli() else {
        return;
    };
    assert!(cases().len() >= 25);
    for direct in [true, false] {
        let p = edit_harness::proxy(direct, false);
        for (design, text, wanted, edit) in cases() {
            let start = design();
            let t = p.ask(&cli, &start, text);
            let f = fact(&t).unwrap_or_else(|| {
                let log = std::fs::read_to_string(p.dir.join("audit.jsonl")).unwrap_or_default();
                let kinds: Vec<String> = log
                    .lines()
                    .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                    .map(|v| v["kind"].to_string())
                    .collect();
                panic!(
                    "direct={direct} no fact for {text}: reads {:?} direct {:?} upstream {} audit {:?}",
                    t.pilot_reads, t.direct, t.upstream["messages"], kinds
                )
            });
            assert!(
                f.chars().count() <= 1200 + 40,
                "{text}: {}",
                f.chars().count()
            );
            for w in wanted {
                assert!(f.contains(w), "direct={direct} {text}: missing {w} in {f}");
            }
            assert!(t.pilot_reads[0].contains("document.inspect"), "{text}");
            assert!(t.pilot_reads.len() <= 14, "{text}");
            // The fact appears once, on the first model call only.
            assert_eq!(
                edit_harness::systems(&t.upstream)
                    .iter()
                    .filter(|s| s.contains("Design:"))
                    .count(),
                1
            );
            let Some((feature, input, value)) = edit else {
                continue;
            };
            let at = f
                .find("execute timeline.edit ")
                .unwrap_or_else(|| panic!("{text}: {f}"));
            let json_text = f[at + "execute timeline.edit ".len()..]
                .trim_end_matches('.')
                .to_string();
            let end = json_text.rfind("}}").unwrap() + 2;
            let params: Value = serde_json::from_str(&json_text[..end]).unwrap();
            assert_eq!(params["feature"], feature, "{text}");
            let mut script = start.clone();
            script.push(json!({"command":"timeline.edit","params":params}));
            script.push(json!({"command":"timeline.inspect","params":{"feature":feature}}));
            let results = edit_harness::run_script(&cli, &script);
            let inspected = results.last().unwrap();
            let changed = inspected["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["label"].as_str().unwrap().eq_ignore_ascii_case(input))
                .unwrap_or_else(|| panic!("{text}: {inspected}"));
            assert_eq!(changed["expression"], value, "{text}");
        }
    }
}

#[test]
fn requests_that_are_not_edits_get_no_fact() {
    let Some(cli) = edit_harness::cli() else {
        return;
    };
    let no_fact: Vec<(Design, &str)> = vec![
        (plate, "make a 20 mm cube"),
        (plate, "create a new plate 60x40x10 with a 6 mm hole"),
        (plate, "how many holes are there?"),
        (plate, "what is the volume"),
        (plate, "build a bracket"),
        (plate, "Report the volume of the existing part"),
        (
            pegs,
            "draw a circle of diameter 12 mm on XY and extrude it 4 mm",
        ),
        (plate, "tell me about the hole"),
        (plate, "hello"),
        (plate, "design a 30 mm cylinder 10 mm tall"),
    ];
    assert!(no_fact.len() + 2 >= 12);
    for direct in [true, false] {
        let p = edit_harness::proxy(direct, false);
        for (design, text) in &no_fact {
            let t = p.ask(&cli, &design(), text);
            assert!(fact(&t).is_none(), "direct={direct} {text}");
        }
    }
    // Recipe-handled turns (default mode) get nothing.
    let p = edit_harness::proxy(true, false);
    for text in ["move it 50 mm along X", "rename the body to Mount"] {
        let t = p.ask(&cli, &plate(), text);
        assert!(fact(&t).is_none(), "{text}");
    }
    // --no-hints: no fact and no extra reads, even for an edit request.
    let p = edit_harness::proxy(true, true);
    let t = p.ask(&cli, &plate(), "make the hole 8 mm instead");
    assert!(fact(&t).is_none());
    assert!(t
        .pilot_reads
        .iter()
        .all(|r| !r.contains("timeline.inspect")));
    // An empty design: the edit wording is read once and nothing is added.
    let p = edit_harness::proxy(false, false);
    let t = p.ask(&cli, &[], "make the hole 8 mm instead");
    assert!(fact(&t).is_none());
    assert_eq!(t.pilot_reads.len(), 1);
}
