//! Independently authored development cases, with careful-reader form arguments.
use serde_json::{json, Value};

pub fn cases() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for n in 0..12 {
        let (x, y) = if n % 2 == 0 {
            (180., 120.)
        } else {
            (120., 180.)
        };
        let size = match n % 4 {
            0 => format!("{x} x {y} x 10"),
            1 => format!("{x} mm x {y} mm x 10 mm"),
            2 => format!("{} cm x {} cm x 1 cm", x / 10., y / 10.),
            _ => format!("{} cm x {y} mm x 10 mm", x / 10.),
        };
        let layout = match n % 3 {
            0 => "4 x 3 grid of 8 mm holes at 30 mm pitch",
            1 => "grid 4 by 3 of holes 8 mm diameter pitch 30 mm",
            _ => "4 by 3 grid of holes 8 mm diameter 30 mm apart",
        };
        let face = if n % 2 == 0 { ", front face" } else { "" };
        out.push((format!("Build a plate {size}{face}, {layout}."), json!({"length":x,"width":y,"thickness":10,"grid_columns":if x>=y {4} else {3},"grid_rows":if x>=y {3} else {4},"grid_pitch_x":30,"grid_pitch_y":30,"grid_hole_diameter":8})));
    }
    for n in 0..12 {
        let x = 20 + n;
        let edge = if n % 2 == 0 {
            "1 mm chamfer on the edges"
        } else {
            "chamfer edges 1 mm"
        };
        let grid = if n % 3 == 0 {
            "5 x 3 grid of 6 mm holes 3 cm apart"
        } else {
            "grid 5 by 3 of holes 6 mm diameter pitch 3 cm"
        };
        let tail = if n % 2 == 0 {
            format!("{edge}, {grid}")
        } else {
            format!("{grid}, {edge}")
        };
        out.push((format!("Make a {x} x 12 cm plate 8 mm thick, {tail}."),json!({"length":x*10,"width":120,"thickness":8,"grid_columns":5,"grid_rows":3,"grid_pitch_x":30,"grid_pitch_y":30,"grid_hole_diameter":6,"corner_kind":"chamfer","corner_size":1})));
    }
    for n in 0..12 {
        let size = ["4in", "4 inches", "4 inch", "4\""][n % 4];
        let count = 5 + n / 4;
        let holes = if n % 2 == 0 {
            format!("{count} holes 0.25in on a 3in bolt circle")
        } else {
            format!("{count} holes 1/4 inches on a 3 inches bolt circle")
        };
        out.push((format!("Need a {size} square plate, 1/4in thick, {holes}, thanks."),json!({"length":"4 in","width":"4 in","thickness":"1/4 in","bolt_hole_count":count,"bolt_hole_diameter":"1/4 in","bolt_circle_diameter":"3 in"})));
    }
    for (text, args) in [
        ("Make a plate 180 x 120 x 10 mm with one corner at origin with centred grid 4 x 3 holes 8 mm diameter pitch 30 mm", json!({"length":180,"width":120,"thickness":10,"grid_columns":4,"grid_rows":3,"grid_pitch_x":30,"grid_pitch_y":30,"grid_hole_diameter":8})),
        ("Make a plate 180 x 120 x 10 mm centred at origin with centred grid 4 x 3 holes 8 mm diameter pitch 30 mm", json!({"length":180,"width":120,"thickness":10,"centred":true,"grid_columns":4,"grid_rows":3,"grid_pitch_x":30,"grid_pitch_y":30,"grid_hole_diameter":8})),
        ("Make a plate 180 x 120 x 10 mm with one corner at origin with 5 evenly spaced holes 8 mm diameter on bolt circle 70 mm", json!({"length":180,"width":120,"thickness":10,"bolt_hole_count":5,"bolt_hole_diameter":8,"bolt_circle_diameter":70})),
    ] { out.push((text.into(),args)); }
    out
}

pub fn near_misses() -> Vec<String> {
    let grid = "plate 180 x 120 x 10, 4 x 3 grid of 8 mm holes at 30 mm pitch";
    let mut out: Vec<_> = ["not ", "maybe ", "without "]
        .iter()
        .map(|s| format!("{s}{grid}"))
        .collect();
    for tail in [
        " or 40 mm pitch",
        " and 5 mm thick",
        " offset 10 mm",
        " on front plane and XY plane",
        " no holes",
        " except one hole",
        " and a centre bore",
        " pitch 40 mm",
        " on back face",
    ] {
        out.push(format!("{grid}{tail}"));
    }
    out.push("plate 18 cm x 120 x 10 mm, 4 x 3 grid of 8 mm holes at 30 mm pitch".into());
    out.push(
        "4in square plate 1/4in thick, 5 holes 1/4in on a 3in bolt circle or 4in bolt circle"
            .into(),
    );
    out
}
