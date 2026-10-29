//! Independently authored dev requests; no model or evaluation corpus.
#![cfg(feature = "core")]
#![allow(clippy::unwrap_used)]
use dutchman_pilot::{catalog::Catalog, planner};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/dev_joint_exact.json")).unwrap()
}

fn inspected(text: &str) -> Value {
    let mut messages = vec![json!({"role":"user","content":text})];
    for line in include_str!("fixtures/revolute-public-0.20.jsonl").lines() {
        let response: Value = serde_json::from_str(line).unwrap();
        if (9..=14).contains(&response["id"].as_u64().unwrap()) {
            let content = response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .replace("Base", "Bracket")
                .replace("Arm", "Lever");
            messages.push(json!({"role":"tool","content":content}));
        }
    }
    messages.push(json!({"role":"tool","content":json!({"joints":[],"origins":[],"errors":[],"conflicts":[]}).to_string()}));
    json!({"messages":messages,"tools":[{"function":{"name":"execute"}}]})
}

#[test]
fn exact_joint_dev_prompts_build_the_hand_filled_form() {
    let fixture = fixture();
    let catalog = Catalog::bundled().unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!((6..=10).contains(&cases.len()));
    let mut declined = Vec::new();
    for case in cases {
        let text = case["text"].as_str().unwrap();
        let request = inspected(text);
        let form =
            planner::recipe_expand("revolute_joint", &fixture["form_args"], &request, &catalog)
                .unwrap();
        let exact = planner::plan_direct(&request, &catalog).unwrap();
        if exact.is_none() {
            declined.push(text);
            eprintln!("{text}: exact declined; before: {}", case["before"]);
            continue;
        }
        let exact = exact.unwrap();
        assert_eq!(exact.calls, form.calls, "{text}");
        assert_eq!(exact.slots, form.slots, "{text}");
        assert_eq!(
            serde_json::to_value(exact.expected).unwrap(),
            serde_json::to_value(form.expected).unwrap(),
            "{text}"
        );
        let reference = &fixture["reference"];
        let create = &exact.calls[1]["arguments"]["params"];
        assert_eq!(create["a"]["occurrence"], reference["ground"]);
        assert_eq!(create["b"]["occurrence"], reference["moving"]);
        for (side, key) in [("a", "pin_circle"), ("b", "hole_circle")] {
            let pick = &create[side]["circle"];
            let radius = pick[0].as_f64().unwrap().hypot(pick[1].as_f64().unwrap());
            assert!((radius - reference[key][0].as_f64().unwrap()).abs() < 0.001);
            assert!((pick[2].as_f64().unwrap() - 10.).abs() < 0.001);
        }
        assert_eq!(create["flip"], reference["flip"]);
        assert!(create.get("limits").is_none());
        assert_eq!(
            exact.calls[2]["arguments"]["params"]["value"].as_f64(),
            reference["drive"].as_f64()
        );
        let fresh = json!({"messages":[{"role":"user","content":text}],"tools":request["tools"]});
        assert_eq!(
            planner::plan_direct(&fresh, &catalog)
                .unwrap()
                .unwrap()
                .rule,
            "resolve_joint",
            "{text}"
        );
    }
    assert!(
        declined.is_empty(),
        "exact declined {} dev requests",
        declined.len()
    );
}

#[test]
fn exact_joint_declines_unclear_rotations_and_extra_operations() {
    let catalog = Catalog::bundled().unwrap();
    for text in [
        "revolute joint",
        "Bracket + Lever revolute joint",
        "Hinge Lever on Bracket, turned clockwise to 90 degrees",
        "Hinge Lever on Bracket, turned to about 90 degrees",
        "Hinge Lever on Bracket, turned to 90 or 60 degrees",
        "Hinge Lever on Bracket, turned to 90 radians",
        "Hinge Lever on Bracket, turned",
        "Hinge Lever on Bracket with a limit of 90 degrees",
        "Hinge Lever on Bracket and rotate Bracket 90 degrees",
        "Hinge Lever on Bracket and fillet the hole 2mm",
        "Do not hinge Lever on Bracket at 90 degrees",
    ] {
        assert!(
            planner::plan_direct(&inspected(text), &catalog)
                .unwrap()
                .is_none(),
            "{text}"
        );
    }
}
