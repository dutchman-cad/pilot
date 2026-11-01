//! Geometry-independent checks run even without the optional public binary.
use super::*;

#[test]
fn dev_rectangles_forms_and_exact_parsers_agree() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_rectangles.json")).unwrap();
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
fn dev_rectangles_unclear_requests_preserve_fills_and_defer_exact() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let supplied = json!({"kind":"rectangle","width":40,"height":30,"depth":9,"plane":"XZ"});
    let build = |args, req: &Value| {
        lib.build(&json!({"recipe":"profile","args":args}), req, &cat)
            .unwrap()
    };
    let baseline = build(supplied.clone(), &json!({"messages":[]}));
    for text in [
        "Make a rectangular profile width 40 height 30 then extrude",
        "Make a rectangular profile on XY or YZ width 40 height 30 then extrude 9",
        "Make a rectangular profile on an unknown plane width 40 height 30 then extrude 9",
        "Make a rectangular profile width 40 height 30 then extrude 9 at the corner",
        "Make a rectangular profile width 40 height 30 then extrude 9 and add a hole",
    ] {
        let req = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        assert!(
            dutchman_pilot::planner::plan_direct(&req, &cat)
                .unwrap()
                .is_none(),
            "{text}"
        );
        let mut args = supplied.clone();
        assert!(
            forms::request_defaults(&mut args, schema, &req)
                .unwrap()
                .is_empty(),
            "{text}"
        );
        assert_eq!(args, supplied, "{text}");
        assert_eq!(build(args, &req).calls, baseline.calls, "{text}");
    }
}
