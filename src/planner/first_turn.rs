//! Independent regression tables for ordinary first-turn language.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;

#[test]
fn principal_planes_are_bounded_slots() {
    let c = Catalog::bundled().unwrap();
    for plane in ["XY", "xy plane", "the X-Y plane", "the xy plane"] {
        let text = format!("Draw a rectangle 46 by 28 mm on {plane} and extrude it 13 mm.");
        let p = plan(&cases::request(&text), &c)
            .unwrap()
            .unwrap_or_else(|| panic!("{text}"));
        assert_eq!(
            p.slots,
            json!({"Sketch":{"plane":"XY","size":[46.0,28.0],"distance":13.0}})
        );
    }
    for plane in [
        "top plane",
        "Datum",
        "XY garbage",
        "XY plane extra",
        "XY and YZ",
        "X Y",
    ] {
        for text in [
            format!("rectangle 46x28 on {plane} then extrude 13"),
            format!("mirror it across {plane}"),
        ] {
            assert!(
                plan(&cases::request(&text), &c).unwrap().is_none(),
                "{text}"
            );
        }
    }
    for (text, name) in [
        ("Call this body Mount.", "Mount"),
        ("rename it to 'Mount. Two'", "Mount. Two"),
        ("name the body `Axis XY`!", "Axis XY"),
    ] {
        assert_eq!(
            plan(&cases::request(text), &c).unwrap().unwrap().slots["Rename"]["name"],
            name
        );
    }
    for text in [
        "call this body Mount.Other",
        "rename it to Mount. More words",
        "move it 7 along XX",
        "move it 7 along X and Y",
        "mirror it across XY about YZ",
        "mirror it across XY keeping the original on YZ",
    ] {
        assert!(plan(&cases::request(text), &c).unwrap().is_none(), "{text}");
    }
}
#[test]
fn circle_profile_order_and_units() {
    let c = Catalog::bundled().unwrap();
    for (text, plane) in [
        (
            "On XY, draw a circle of diameter 18 mm and extrude the sketch 7 mm.",
            "XY",
        ),
        ("draw an 18 mm circle on XZ and extrude it 7", "XZ"),
        ("circle radius 9 on YZ, extrude 7 mm", "YZ"),
        (
            "On the X-Y plane sketch a circle radius 0.9cm then extrude by 0.7cm",
            "XY",
        ),
        (
            "sketch a circle 18mm in diameter on yz and extrude the sketch 7mm",
            "YZ",
        ),
    ] {
        assert_eq!(
            plan(&cases::request(text), &c)
                .unwrap()
                .unwrap_or_else(|| panic!("{text}"))
                .slots,
            json!({"Sketch":{"plane":plane,"size":[9.0],"distance":7.0}}),
            "{text}"
        );
    }
}
#[test]
fn relative_through_hole_layouts() {
    let c = Catalog::bundled().unwrap();
    for (size, layout, centers) in [
        (
            "90x40x6",
            "two 6mm through holes, one 12mm from each short end, halfway across the width",
            json!([[12.0, 20.0], [78.0, 20.0]]),
        ),
        (
            "90x40x6",
            "a 6mm through hole 12mm from each end on the centre line",
            json!([[12.0, 20.0], [78.0, 20.0]]),
        ),
        (
            "40x90x6",
            "2 holes diameter 6mm through, one 12mm from each short end halfway across the width",
            json!([[20.0, 12.0], [20.0, 78.0]]),
        ),
        (
            "90x40x6",
            "four 6mm holes right through, one near each corner, 12mm in from both adjacent edges",
            json!([[12.0, 12.0], [78.0, 12.0], [12.0, 28.0], [78.0, 28.0]]),
        ),
        (
            "90x40x6",
            "4 through holes diameter 0.6cm one at each corner 1.2cm in from both adjacent edges",
            json!([[12.0, 12.0], [78.0, 12.0], [12.0, 28.0], [78.0, 28.0]]),
        ),
    ] {
        let text = format!("Build a plate {size}mm with {layout}.");
        let p = plan(&cases::request(&text), &c).unwrap().unwrap();
        assert_eq!(p.slots["Holes"]["centers"], centers, "{text}");
        assert_eq!(p.slots["Holes"]["diameter"], 6.0);
        for (call, center) in p.calls[1..p.calls.len() - 1]
            .iter()
            .zip(centers.as_array().unwrap())
        {
            assert_eq!(
                call["arguments"]["params"]["position"],
                json!([center[0], center[1], 6.0])
            );
        }
    }
    for layout in [
        "two 6mm through holes 12mm from an end halfway across the width",
        "two 6mm through holes one near each short end halfway across the width",
        "two 6mm holes one 12mm from each short end halfway across the width",
        "four 6mm through holes near each corner",
        "four 6mm through holes one near each corner 12mm from one edge",
        "three 6mm through holes one 12mm from each short end halfway across the width",
        "two 6mm through holes 12mm from each end",
        "four 6mm through holes one near each corner 1mm in from both adjacent edges",
    ] {
        let text = format!("plate 90x40x6mm with {layout}");
        assert!(
            plan(&cases::request(&text), &c).unwrap().is_none(),
            "{text}"
        );
    }
    let square = "plate 40x40x6mm with two 6mm through holes one 12mm from each short end halfway across the width";
    assert!(plan(&cases::request(square), &c).unwrap().is_none());
}
#[test]
fn body_count_language_and_how_to_guard() {
    let c = Catalog::bundled().unwrap();
    for text in [
        "how many solid bodies are here?",
        "How many parts are in the design?",
        "how many solids are in the design?",
        "count the bodies",
        "tell me how many bodies are here",
    ] {
        let mut req = cases::request(text);
        req["messages"].as_array_mut().unwrap().remove(0);
        let p = plan(&req, &c).unwrap().unwrap();
        assert_eq!(p.rule, "body_count", "{text}");
        assert_eq!(
            p.calls,
            vec![
                json!({"tool":"execute","arguments":{"command":"document.inspect","params":{"measure":true}}})
            ]
        );
        let p = plan(&cases::request(text), &c).unwrap().unwrap();
        assert_eq!(p.facts, Some(json!({"body_count":1})));
    }
    for text in [
        "how do I count the bodies?",
        "explain how many parts there are",
        "count the bodies and move it 4 along X",
    ] {
        assert!(plan(&cases::request(text), &c).unwrap().is_none(), "{text}");
    }
}
