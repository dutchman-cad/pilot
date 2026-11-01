//! Geometry-independent checks run even without the optional public binary.
use super::*;

#[test]
fn dev_profile_openings_forms_and_exact_parsers_agree() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_profile_openings.json")).unwrap();
    for case in cases {
        let req = json!({"messages":[{"role":"user","content":case["prompt"]}],"tools":[{"function":{"name":"execute"}}]});
        let mut args = case["args"].clone();
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        forms::request_defaults(&mut args, schema, &req).unwrap();
        let slots = forms::to_slots(&args, schema).unwrap();
        let form = lib
            .build(&json!({"recipe":"profile","args":slots}), &req, &cat)
            .unwrap();
        let exact = dutchman_pilot::planner::plan_direct(&req, &cat)
            .unwrap()
            .unwrap_or_else(|| panic!("{}: no exact plan", case["prompt"]));
        assert_eq!(form.calls, exact.calls, "{}", case["prompt"]);
    }
}

#[test]
fn profile_opening_inference_preserves_fills_when_wording_is_unclear() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for text in [
        "U opening forward",
        "U opening left and right",
        "U opening up or down",
        "U opening negative left",
        "U opening down opening right",
        "U opening not down",
        "U without an opening down",
        "U opening down or toward left",
        "U opening facing",
        "U opening down-facing web",
        "U opening toward the right plane",
        "U with web facing down",
        "U with flange faces right",
        "U facing left",
        "U with web open to the right",
        "U with flange is open to the left",
        "U with flange opening left",
        "U with opening left of the web",
        "U with opening to the right of the origin",
    ] {
        let req = json!({"messages":[{"role":"user","content":text}]});
        let args = json!({"recipe":"profile","args":{"kind":"U","width":40,"height":30,"depth":12,"thickness":3}});
        let default = lib.build(&args, &json!({"messages":[]}), &cat).unwrap();
        let result = lib.build(&args, &req, &cat).unwrap();
        assert_eq!(result.calls, default.calls, "{text}");
        assert_eq!(result.slots, default.slots, "{text}");
    }
    // Even clear request inference cannot invalidate otherwise valid geometry
    // or reinterpret a non-channel fill.
    for (args, text) in [
        (
            json!({"kind":"C","width":40,"height":10,"web_thickness":12,"flange_thickness":3,"depth":12}),
            "C opening down".to_string(),
        ),
        (
            json!({"kind":"rectangle","width":40,"height":30,"depth":12}),
            "rectangle opening left".to_string(),
        ),
    ] {
        let args = json!({"recipe":"profile","args":args});
        let default = lib.build(&args, &json!({"messages":[]}), &cat).unwrap();
        let result = lib
            .build(
                &args,
                &json!({"messages":[{"role":"user","content":text}]}),
                &cat,
            )
            .unwrap();
        assert_eq!(result.calls, default.calls);
    }
}

#[test]
fn profile_openings_exact_parser_declines_unclear_directions() {
    let cat = Catalog::bundled().unwrap();
    for opening in [
        "opening forward",
        "opening left and right",
        "opening up or down",
        "opening negative left",
        "opening down opening right",
        "opening not down",
        "without an opening down",
        "opening down or toward left",
        "opening facing",
        "opening down-facing web",
        "opening toward the right plane",
        "opening left of the web",
        "opening to the right of the origin",
    ] {
        for kind in ["C", "U"] {
            let text = format!("Make a {kind} width 40 mm height 30 mm web 3 mm flange 4 mm then extrude 12 mm {opening}");
            let req = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
            assert!(
                dutchman_pilot::planner::plan_direct(&req, &cat)
                    .unwrap()
                    .is_none(),
                "{text}"
            );
        }
    }
}

#[test]
fn profile_opening_request_is_bounded_and_uses_latest_user() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let args = json!({"recipe":"profile","args":{"kind":"U","width":40,"height":30,"depth":12,"thickness":3}});
    let request = |text: &str| json!({"messages":[{"role":"user","content":text}]});
    let left = lib.build(&args, &request("U opening left"), &cat).unwrap();
    let history = json!({"messages":[
        {"role":"user","content":"U opening down"},
        {"role":"assistant","content":"opening right"},
        {"role":"user","content":"U opening left"},
        {"role":"tool","content":"opening up"}
    ]});
    assert_eq!(lib.build(&args, &history, &cat).unwrap().calls, left.calls);
    let max = "x".repeat(65_536);
    assert!(lib.build(&args, &request(&max), &cat).is_ok());
    let oversized = format!("{max}x");
    assert!(lib
        .build(&args, &request(&oversized), &cat)
        .unwrap_err()
        .contains("request exceeds 65536 bytes"));
}
