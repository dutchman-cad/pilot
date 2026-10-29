//! Geometry-independent checks run even without the optional public binary.
use super::*;

#[test]
fn dev_profile_planes_forms_and_exact_parsers_agree() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_profile_planes.json")).unwrap();
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
            .unwrap();
        assert_eq!(form.calls, exact.calls, "{}", case["prompt"]);
    }
}

#[test]
fn profile_request_defaults_are_bounded_and_preserve_supplied_slots() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let request = |text: &str| json!({"messages":[{"role":"user","content":text}]});
    for (text, plane, offset) in [
        ("T on YZ stem offset -1/8 in from the centre", "YZ", 22.225),
        (
            "T on the right plane stem offset 12 from the left",
            "YZ",
            12.,
        ),
        (
            "T on the front plane stem offset 0 from the center",
            "XZ",
            25.4,
        ),
        (
            "T on the top plane stem offset 1/2 inch from the left",
            "XY",
            12.7,
        ),
    ] {
        let mut args = json!({"kind":"T","width":50.8});
        let changes = forms::request_defaults(&mut args, schema, &request(text)).unwrap();
        assert_eq!(args["plane"], plane);
        assert!((args["stem_offset"].as_f64().unwrap() - offset).abs() < 1e-9);
        assert_eq!(changes.len(), 2);
        assert!(forms::request_defaults(&mut args, schema, &request(text))
            .unwrap()
            .is_empty());
    }
    for text in [
        "T on XY or YZ",
        "T not on XZ",
        "T stem offset 3",
        "T stem offset 3 from left or 4 from centre",
        "T stem offset 1000001 from left",
        "T stem offset -3 from left",
    ] {
        let mut args = json!({"kind":"T","width":40});
        assert!(
            forms::request_defaults(&mut args, schema, &request(text)).is_err(),
            "{text}"
        );
    }
    let mut supplied = json!({"kind":"T","width":40,"plane":"XZ","stem_offset":10});
    let before = supplied.clone();
    forms::request_defaults(
        &mut supplied,
        schema,
        &request("T on YZ stem offset 3 from centre"),
    )
    .unwrap();
    assert_eq!(supplied, before);
    assert!(forms::request_defaults(&mut supplied, schema, &request(&"x".repeat(65_537))).is_err());
}

#[test]
fn profile_openings_do_not_silently_drop_unsupported_or_conflicting_directions() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for text in [
        "U opening forward",
        "U opening left and right",
        "U opening up or down",
        "U opening negative left",
        "U opening down opening right",
    ] {
        let req = json!({"messages":[{"role":"user","content":text}]});
        let result = lib.build(&json!({"recipe":"profile","args":{"kind":"U","width":40,"height":30,"depth":12,"thickness":3}}), &req, &cat);
        assert!(result.unwrap_err().contains("opening"), "{text}");
    }
}
