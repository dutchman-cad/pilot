//! Independently authored drafting requests, with hand-filled millimetre sizes.
use super::*;
use dutchman_pilot::planner;

#[test]
fn dev_l_notation_offers_and_exact_calls_match_forms() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/dev_l_notation.json")).unwrap();
    assert!(cases.len() >= 30);
    for case in cases {
        let prompt = case["prompt"].as_str().unwrap();
        for complete in [false, true] {
            let text = if complete {
                format!("{prompt} length 80 mm")
            } else {
                prompt.to_string()
            };
            let found = lib.find(&text, &Bm25).unwrap();
            assert_eq!(found["hit"], true, "{text}: {found}");
            assert_eq!(found["recipes"][0]["id"], "profile", "{text}: {found}");
            let band = lib.form_band(&text).unwrap();
            assert_eq!(band["band"], "fill", "{text}: {band}");
            assert_eq!(band["recipes"][0]["id"], "profile", "{text}");
            let request = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
            let exact = planner::plan_direct(&request, &cat).unwrap();
            assert_eq!(exact.is_some(), complete, "{text}");
            let args = json!({"kind":"L","plane":case["plane"],"width":case["size"][0],"height":case["size"][1],"thickness":case["size"][2],"depth":80});
            let form = lib
                .build(&json!({"recipe":"profile","args":args}), &request, &cat)
                .unwrap();
            if let Some(exact) = exact {
                assert_eq!(exact.calls, form.calls, "{text}");
            }
        }
    }
}

#[test]
fn dev_l_notation_ambiguous_requests_leave_fills_alone() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for text in [
        "L48x32x4 and T profile length 80 mm",
        "48x32x4 angle and a cylinder length 80 mm",
        "Do not make L48x32x4 length 80 mm",
        "Don't build L-section 48/32/4 length 80 mm",
        "angle 36x36x3 unequal legs length 80 mm",
        "angle 48x32x4 unequal thickness length 80 mm",
        "L-section 48/32 length 80 mm",
        "L-section 48/32/4/2 length 80 mm",
        "L48x32x4 length 80 or 90 mm",
    ] {
        let request = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        assert!(
            planner::plan_direct(&request, &cat).unwrap().is_none(),
            "{text}"
        );
        // A model's complete typed fill is validated on its own merits.
        assert!(lib.build(&json!({"recipe":"profile","args":{"kind":"L","width":48,"height":32,"thickness":4,"depth":80}}), &request, &cat).is_ok(), "{text}");
    }
}
