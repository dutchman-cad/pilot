#![cfg(feature = "core")]
#![allow(clippy::unwrap_used)]

use dutchman_pilot::{catalog::Catalog, guidance, planner, repair};
use serde_json::json;

#[test]
fn caller_supplied_catalogue_supports_lookup_plan_and_repair() {
    let catalog = Catalog::from_json(
        r#"[{"id":"solid.box","label":"Box","params":"length: expr; width: expr; height: expr"}]"#,
    )
    .unwrap();
    let how = serde_json::to_value(catalog.how("make a cube").unwrap()).unwrap();
    assert_eq!(how["matches"][0]["command"]["id"], "solid.box");
    let request = json!({
        "messages":[{"role":"user","content":"make a 20 mm cube"}],
        "tools":[{"function":{"name":"execute"}}]
    });
    let plan = serde_json::to_value(planner::plan(&request, &catalog).unwrap().unwrap()).unwrap();
    assert_eq!(plan["calls"][0]["arguments"]["command"], "solid.box");
    assert_eq!(plan["calls"][0]["arguments"]["params"]["height"], 20.0);

    let schema = json!({"type":"object","properties":{"points":{"type":"array"}}});
    let mut args = json!({"points":"[[0,0],[20,20]]"});
    let rewrites = repair::repair_arguments("execute", &mut args, &schema).unwrap();
    assert!(!rewrites.is_empty());
    assert_eq!(args["points"], json!([[0, 0], [20, 20]]));
}

#[test]
fn hint_rules_use_explicit_state_and_preserve_partial_results() {
    let empty = json!({"bodies":[],"sketches":[]});
    assert!(guidance::state_hint(None, &empty, false, None)
        .unwrap()
        .contains("Empty design"));
    let sketch = json!({"active_sketch":1,"sketches":[{"name":"S","dof":4}]});
    let hint = guidance::state_hint(None, &sketch, false, None).unwrap();
    assert!(hint.contains("open sketch"));
    assert!(hint.contains("4 free degrees"));
    let body = json!({"bodies":[{"volume_mm3":5}]});
    assert!(guidance::state_hint(Some(&body), &body, true, None)
        .unwrap()
        .contains("design didn't change"));
    let partial = json!({"bodies":[],"sketches":[],"total":12,"truncated":true});
    assert!(guidance::state_hint(Some(&empty), &partial, true, None).is_none());
    assert_eq!(
        guidance::paging(&partial),
        ["shown unknown of 12 (partial)"]
    );
}
