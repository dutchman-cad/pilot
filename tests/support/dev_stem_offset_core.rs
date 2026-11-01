//! Geometry-independent checks run even without the optional public binary.
use super::*;

#[test]
fn dev_stem_offset_forms_and_exact_parsers_agree() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_stem_offset.json")).unwrap();
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
fn stem_offset_defaults_are_bounded_and_preserve_supplied_slots() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let request = |text: &str| json!({"messages":[{"role":"user","content":text}]});
    for (text, offset) in [
        ("T on YZ stem offset -1/8 in from the centre", 22.225),
        ("T on the right plane stem offset 12 from the left", 12.),
        ("T on the front plane stem offset 0 from the center", 25.4),
        (
            "T on the top plane stem offset 1/2 inch from the left",
            12.7,
        ),
    ] {
        let mut args = json!({"kind":"T","width":50.8});
        let changes = forms::request_defaults(&mut args, schema, &request(text)).unwrap();
        // The plane is left to the builder's default; only the stem offset is filled.
        assert!(args.get("plane").is_none(), "{text}");
        assert!((args["stem_offset"].as_f64().unwrap() - offset).abs() < 1e-9);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["field"], "stem_offset");
        assert_eq!(changes[0]["before"], Value::Null);
        assert_eq!(changes[0]["after"], args["stem_offset"]);
        assert_eq!(changes[0]["defaulted"], true);
        assert_eq!(
            changes[0]["rule"],
            "explicit stem centre offset; converted to distance from left"
        );
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
        "T stem offset 3 from right",
        "T stem offset 3 from left offset 4 from centre",
        "T not stem offset 3 from left",
        "T without stem offset 3 from left",
        "T stem offset either 3 from left",
        "T stem offset 1 ft from left",
        "T stem centred under the flange",
    ] {
        // Unclear wording infers nothing; the fill keeps what the model wrote.
        let mut args = json!({"kind":"T","width":40});
        let changes = forms::request_defaults(&mut args, schema, &request(text)).unwrap();
        assert!(changes.is_empty(), "{text}: {changes:?}");
        assert_eq!(args, json!({"kind":"T","width":40}), "{text}");
        // A completed model fill still builds when request evidence is unclear.
        args["height"] = json!(30);
        args["depth"] = json!(12);
        args["thickness"] = json!(3);
        let slots = forms::to_slots(&args, schema).unwrap();
        lib.build(
            &json!({"recipe":"profile","args":slots}),
            &request(text),
            &Catalog::bundled().unwrap(),
        )
        .unwrap();
    }
    let mut explicit_plane = json!({"kind":"T","width":40,"plane":"XZ"});
    forms::request_defaults(
        &mut explicit_plane,
        schema,
        &request("T on YZ stem offset 12 from left"),
    )
    .unwrap();
    assert_eq!(
        explicit_plane,
        json!({"kind":"T","width":40,"plane":"XZ","stem_offset":12.})
    );
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
