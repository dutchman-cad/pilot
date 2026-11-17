//! Development designs and wording authored for the public CLI; no benchmark tasks.
use super::context_tests::Session;
use super::*;

fn design(s: &mut Session, class: &str) {
    match class {
        "bored" => {
            s.execute(
                "solid.box",
                json!({"length":60,"width":40,"height":20,"body_name":"Plate"}),
            );
            for (name, p) in [
                (
                    "Hole A",
                    json!({"position":[20,20,20],"diameter":6,"depth":15,"type":"counterbore","cb_diameter":11,"cb_depth":4,"targets":["Plate"]}),
                ),
                (
                    "Hole B",
                    json!({"position":[45,20,20],"diameter":5,"depth":12,"type":"countersink","cs_diameter":10,"cs_angle":90,"targets":["Plate"]}),
                ),
            ] {
                let made = s.execute("solid.hole", p);
                s.execute(
                    "timeline.rename",
                    json!({"feature":made["name"],"name":name}),
                );
            }
        }
        "row" | "spread" => {
            s.execute(
                "solid.box",
                json!({"length":90,"width":30,"height":12,"body_name":"Rail"}),
            );
            let places: &[[f64; 2]] = if class == "row" {
                &[[15., 15.], [45., 15.], [75., 15.]]
            } else {
                &[[12., 15.], [40., 8.], [50., 22.]]
            };
            for xy in places {
                s.execute(
                    "solid.hole",
                    json!({"position":[xy[0],xy[1],12],"diameter":5,"depth":8,"targets":["Rail"]}),
                );
            }
        }
        "raised" | "tab" => {
            s.execute(
                "solid.box",
                json!({"length":60,"width":40,"height":10,"body_name":"Base"}),
            );
            s.execute(
                "sketch.create",
                json!({"plane":{"face":[30,20,10]},"project_edges":false}),
            );
            s.execute(
                "sketch.rectangle.two_point",
                json!({"p0":[10,10],"p1":[20,18]}),
            );
            s.execute("sketch.finish", json!({}));
            s.execute(
                "solid.extrude",
                json!({"distance":6,"operation":"join","name":"Pad 1"}),
            );
            if class == "raised" {
                s.execute(
                    "sketch.create",
                    json!({"plane":{"face":[45,25,10]},"project_edges":false}),
                );
                s.execute("sketch.circle.center", json!({"center":[45,25],"radius":4}));
                s.execute("sketch.finish", json!({}));
                s.execute(
                    "solid.extrude",
                    json!({"distance":8,"operation":"join","name":"Boss 1"}),
                );
            }
        }
        _ => {
            s.execute(
                "solid.box",
                json!({"length":60,"width":40,"height":20,"body_name":"Block"}),
            );
            s.execute(
                "sketch.create",
                json!({"plane":{"face":[30,20,20]},"project_edges":false}),
            );
            s.execute(
                "sketch.rectangle.two_point",
                json!({"p0":[10,10],"p1":[40,30]}),
            );
            s.execute("sketch.finish", json!({}));
            s.execute(
                "solid.extrude",
                json!({"distance":6,"direction":"negative","operation":"cut","name":"Step pocket"}),
            );
            s.execute(
                "sketch.create",
                json!({"plane":{"face":[25,20,14]},"project_edges":false}),
            );
            s.execute(
                "sketch.rectangle.two_point",
                json!({"p0":[15,15],"p1":[25,25]}),
            );
            s.execute("sketch.finish", json!({}));
            s.execute(
                "solid.extrude",
                json!({"distance":5,"direction":"negative","operation":"cut"}),
            );
        }
    }
}

fn inspected(s: &mut Session, text: &str, cat: &Catalog) -> Value {
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":text}]);
    for _ in 0..3 {
        for c in planner::named_feature::reads(&req, cat).unwrap() {
            let result = s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":result.to_string()}));
        }
    }
    req
}

fn definition(s: &mut Session, feature: &str) -> Value {
    s.execute("timeline.inspect", json!({"feature":feature}))["definition"].clone()
}
// Edited fields come back as computed text; compare them to a micrometre.
fn mm(v: &Value) -> f64 {
    let n = v
        .as_f64()
        .unwrap_or_else(|| v.as_str().unwrap().trim_end_matches(" mm").parse().unwrap());
    (n * 1000.).round() / 1000.
}
fn names(req: &Value) -> Value {
    planner::named_feature::form(req)["properties"]["feature"]["enum"].clone()
}
/// Runs the plan through the public CLI and requires its measured verification.
fn verified(s: &mut Session, req: &mut Value, plan: &planner::Plan) {
    assert!(planner::named_feature::completion(req, plan).is_some());
    let calls = direct::calls(req, plan, 0);
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"assistant","tool_calls":calls}));
    for c in &plan.calls {
        let result = s.execute(
            c["arguments"]["command"].as_str().unwrap(),
            c["arguments"]["params"].clone(),
        );
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":result.to_string()}));
    }
    assert_eq!(
        planner::named_feature::completion(req, plan),
        None,
        "{}",
        plan.slots
    );
}
fn exact(s: &mut Session, class: &str, text: &str, cat: &Catalog) -> (Value, planner::Plan) {
    design(s, class);
    let req = inspected(s, text, cat);
    let plan = planner::named_feature::exact(&req, cat)
        .unwrap()
        .unwrap_or_else(|| panic!("{text}: {}", planner::named_feature::trace(&req)));
    (req, plan)
}

#[test]
fn named_feature_counterbored_and_countersunk_holes_edit_the_named_part() {
    let cat = context_tests::public_catalog::catalog().unwrap();
    // The hole's own bore, not its counterbore, when only the hole is named.
    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "bored", "Make Hole A 18 mm deep", &cat);
    assert_eq!(plan.slots["field"], "depth");
    verified(&mut s, &mut req, &plan);
    let d = definition(&mut s, "Hole A");
    assert_eq!((mm(&d["depth"]), mm(&d["hole"]["cb_depth"])), (18., 4.));

    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "bored", "Widen the bore of Hole A to 7 mm", &cat);
    assert_eq!(plan.slots["field"], "diameter");
    verified(&mut s, &mut req, &plan);
    let d = definition(&mut s, "Hole A");
    assert_eq!(
        (mm(&d["diameter"]), mm(&d["hole"]["cb_diameter"])),
        (7., 11.)
    );

    // The counterbore itself stays on the form path, with its own fields.
    let Some(mut s) = Session::new() else { return };
    design(&mut s, "bored");
    let text = "Change the counterbore on Hole A to 5 mm deep";
    let mut req = inspected(&mut s, text, &cat);
    assert!(planner::named_feature::candidate(text));
    assert_eq!(names(&req), json!(["Hole A"]));
    assert!(planner::named_feature::exact(&req, &cat).unwrap().is_none());
    let plan = planner::named_feature::build(
        &json!({"feature":"Hole A","parameter":"depth","mode":"set","value":5}),
        &req,
        &cat,
    )
    .unwrap();
    assert_eq!(plan.slots["field"], "hole.cb_depth");
    verified(&mut s, &mut req, &plan);
    let d = definition(&mut s, "Hole A");
    assert_eq!((mm(&d["depth"]), mm(&d["hole"]["cb_depth"])), (15., 5.));

    // A countersunk hole's bore is measurable; its cone has no public radius.
    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "bored", "Deepen the countersunk hole by 2 mm", &cat);
    assert_eq!(plan.slots["feature"], "Hole B");
    verified(&mut s, &mut req, &plan);
    assert_eq!(mm(&definition(&mut s, "Hole B")["depth"]), 14.);
    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "bored", "Make Hole B 4 mm across", &cat);
    assert_eq!(plan.slots["field"], "diameter");
    verified(&mut s, &mut req, &plan);
    let req = inspected(&mut s, "Make the countersink on Hole B 12 mm across", &cat);
    assert_eq!(names(&req), json!(["Hole B"]));
    assert_eq!(
        planner::named_feature::form(&req)["properties"]["parameter"]["enum"],
        json!([])
    );
}

#[test]
fn named_feature_position_words_pick_one_clear_feature_or_none() {
    let cat = context_tests::public_catalog::catalog().unwrap();
    for (class, text, expected) in [
        ("row", "Make the left hole 3 mm deeper", "Hole1"),
        ("row", "Make the left one 3 mm deeper", "Hole1"),
        ("row", "Deepen the rightmost hole to 10 mm", "Hole3"),
        ("row", "Make the middle hole 2 mm deeper", "Hole2"),
        ("spread", "Deepen the end hole to 10 mm", "Hole1"),
        ("spread", "Make the front one 2 mm deeper", "Hole2"),
        ("spread", "Make the hole at the back 1 mm deeper", "Hole3"),
    ] {
        let Some(mut s) = Session::new() else { return };
        let (mut req, plan) = exact(&mut s, class, text, &cat);
        assert_eq!(plan.slots["feature"], expected, "{text}");
        verified(&mut s, &mut req, &plan);
    }
    for (class, text, expected) in [
        // Both ends hold a hole, and a direction is not a selection.
        ("row", "Deepen the end hole to 10 mm", json!([])),
        ("row", "Make the back hole 2 mm deeper", json!([])),
        (
            "row",
            "Shift the hole 3 mm to the left",
            json!(["Hole3", "Hole2", "Hole1"]),
        ),
    ] {
        let Some(mut s) = Session::new() else { return };
        design(&mut s, class);
        let req = inspected(&mut s, text, &cat);
        assert_eq!(names(&req), expected, "{text}");
        assert!(planner::named_feature::exact(&req, &cat).unwrap().is_none());
    }
}

#[test]
fn named_feature_raised_features_from_a_face_change_how_far_they_stand() {
    let cat = context_tests::public_catalog::catalog().unwrap();
    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(
        &mut s,
        "tab",
        "Increase the height of the tab to 8 mm",
        &cat,
    );
    assert_eq!(plan.slots["field"], "extent.distance");
    verified(&mut s, &mut req, &plan);
    assert_eq!(mm(&definition(&mut s, "Pad 1")["extent"]["distance"]), 8.);

    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "raised", "Make Pad 1 stand 7 mm tall", &cat);
    verified(&mut s, &mut req, &plan);
    assert_eq!(mm(&definition(&mut s, "Pad 1")["extent"]["distance"]), 7.);

    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "raised", "Make Boss 1 protrude 9 mm", &cat);
    verified(&mut s, &mut req, &plan);
    assert_eq!(mm(&definition(&mut s, "Boss 1")["extent"]["distance"]), 9.);

    // Two raised features answer to "the boss"; the form lists both with height.
    let Some(mut s) = Session::new() else { return };
    design(&mut s, "raised");
    let req = inspected(&mut s, "Raise the boss by 2 mm", &cat);
    let form = planner::named_feature::form(&req);
    assert_eq!(
        form["properties"]["feature"]["enum"],
        json!(["Boss 1", "Pad 1"])
    );
    assert_eq!(form["properties"]["parameter"]["enum"][0], "height");
}

#[test]
fn named_feature_step_pockets_change_one_level() {
    let cat = context_tests::public_catalog::catalog().unwrap();
    // The upper level carries the lower one with it; the volume counts both footprints.
    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(&mut s, "step", "Deepen Step pocket by 2 mm", &cat);
    assert_eq!(plan.slots["feature"], "Step pocket");
    verified(&mut s, &mut req, &plan);
    assert_eq!(
        s.execute("document.inspect", json!({"measure":true}))["bodies"][0]["volume_mm3"],
        json!(42700.0)
    );

    let Some(mut s) = Session::new() else { return };
    let (mut req, plan) = exact(
        &mut s,
        "step",
        "Change the upper step of Step pocket to 8 mm deep",
        &cat,
    );
    assert_eq!(plan.slots["feature"], "Step pocket");
    verified(&mut s, &mut req, &plan);

    for text in [
        "Make the lower level of Step pocket 2 mm deeper",
        "Make the inner step 2 mm deeper",
    ] {
        let Some(mut s) = Session::new() else { return };
        let (mut req, plan) = exact(&mut s, "step", text, &cat);
        assert_ne!(plan.slots["feature"], "Step pocket", "{text}");
        verified(&mut s, &mut req, &plan);
        let d = definition(&mut s, plan.slots["feature"].as_str().unwrap());
        assert_eq!(mm(&d["extent"]["distance"]), 7.);
    }

    // A level word without nested levels selects nothing.
    let Some(mut s) = Session::new() else { return };
    design(&mut s, "row");
    let req = inspected(&mut s, "Make the inner step 2 mm deeper", &cat);
    assert_eq!(names(&req), json!([]));
}
