//! Development examples authored for the public CLI; no benchmark tasks.
use super::context_tests::{drive, Session};
use super::*;

const CHANGE: &str = "Make the Post boss on the plate a full 5 mm taller.";
const ADD: &str = "add a 6 mm boss 10 mm tall to the plate";

/// A plate with one named boss, plus the earlier turn that made it.
fn design(s: &mut Session) -> Vec<Value> {
    let mut history = vec![
        json!({"role":"user","content":"Model a 60 x 40 x 6 mm plate called Base with an 8 mm post named Post, 10 mm tall"}),
    ];
    for (n, (command, params)) in [
        (
            "solid.box",
            json!({"length":60,"width":40,"height":6,"body_name":"Base"}),
        ),
        (
            "solid.cylinder",
            json!({"base":[20,20,6],"diameter":8,"height":10,"operation":"join","targets":["Base"],"name":"Post"}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let args = json!({"command":command,"params":params});
        let result = s.execute(command, params);
        let id = format!("earlier_{n}");
        history.push(json!({"role":"assistant","content":null,"tool_calls":[{"id":id,"type":"function","function":{"name":"execute","arguments":args.to_string()}}]}));
        history.push(json!({"role":"tool","tool_call_id":id,"content":result.to_string()}));
    }
    history.push(json!({"role":"assistant","content":"Base and Post are made."}));
    history
}
/// Fills the named edit form; declines every creation form.
fn model(wire: &Value) -> Value {
    if wire["tools"][0]["function"]["name"] == "pilot_route" {
        return call("pilot_route", json!({"recipe":"none"}));
    }
    if wire["tools"][0]["function"]["name"] != "pilot_fill" {
        return reply("No change was made.");
    }
    let params = &wire["tools"][0]["function"]["parameters"];
    let Some(names) = params["properties"]["feature"]["enum"].as_array() else {
        return call("pilot_fill", json!({"decline":true}));
    };
    call(
        "pilot_fill",
        json!({"feature":names[0],"parameter":"height","mode":"change","value":5}),
    )
}
fn offered(f: &Fixture) -> Vec<Value> {
    fs::read_to_string(f.dir.join("log.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|r| r["kind"] == "fill_offered" || r["kind"] == "route_offered")
        .map(|r| r["after"].clone())
        .collect()
}

#[test]
fn change_wording_separates_edits_from_additions() {
    for text in [
        CHANGE,
        "deepen the slot by 2 mm",
        "Make the rib height 9 mm.",
        "Set the recess width to 14 mm",
    ] {
        assert!(planner::named_feature::change_wording(text), "{text}");
    }
    for text in [
        ADD,
        "make a boss 6 mm taller than the plate",
        "raise the pocket by 3 mm and widen the hole by 2 mm",
        "make the hole bigger",
        "create another rib 8 mm high",
        "do not raise the pocket by 4 mm",
        "make the plate 60 mm long with a 6 mm boss",
    ] {
        assert!(!planner::named_feature::change_wording(text), "{text}");
    }
}

#[test]
fn design_in_history_needs_an_earlier_body() {
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":CHANGE}]);
    assert!(!planner::named_feature::design_in_history(&req));
    req["messages"] = json!([
        {"role":"user","content":"make a plate"},
        {"role":"assistant","content":null,"tool_calls":[{"id":"a","type":"function","function":{"name":"execute","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"a","content":"{\"id\":1,\"name\":\"Box1\",\"created_bodies\":[\"Base\"]}"},
        {"role":"user","content":CHANGE}
    ]);
    assert!(planner::named_feature::design_in_history(&req));
    // Results after the latest request are this turn's reads, not earlier design.
    req["messages"] = json!([
        {"role":"user","content":CHANGE},
        {"role":"assistant","content":null,"tool_calls":[{"id":"a","type":"function","function":{"name":"execute","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"a","content":"{\"bodies\":[{\"name\":\"Base\"}]}"}
    ]);
    assert!(!planner::named_feature::design_in_history(&req));
}

#[test]
fn existing_feature_change_takes_named_form_not_creation_form() {
    let f = fill_fixture(vec![]);
    // The wording alone does not reach the named form; only inspection does.
    assert!(!planner::named_feature::candidate(CHANGE));
    let band = f.proxy.recipes.form_band(CHANGE).unwrap();
    assert!(
        matches!(band["band"].as_str(), Some("fill" | "route"))
            && !fill::needs_names(&band["recipes"][0]),
        "{band}"
    );
    let Some(mut s) = Session::new() else { return };
    let mut history = design(&mut s);
    history.push(json!({"role":"user","content":CHANGE}));
    let mut f = fill_fixture(vec![]);
    f.proxy.catalog = context_tests::public_catalog::catalog().unwrap();
    f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(model);
    let mut req = request();
    req["messages"] = json!(history);
    let reply = drive(&mut f, &mut s, &mut req);
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert!(log.contains("\"feature_change\""), "{reply}; {log}");
    assert_eq!(
        offered(&f),
        vec![json!({"recipe":"named_feature","score":4.5})],
        "{log}"
    );
    let post = s.execute("timeline.inspect", json!({"feature":"Post"}));
    let height = &post["definition"]["height"];
    let height = height
        .as_f64()
        .or_else(|| height.as_str().and_then(|h| h.parse().ok()));
    assert_eq!(height, Some(15.0), "{post}; {reply}");
}

#[test]
fn adding_a_feature_to_the_same_design_keeps_the_creation_form() {
    let Some(mut s) = Session::new() else { return };
    let mut history = design(&mut s);
    history.push(json!({"role":"user","content":ADD}));
    let mut f = fill_fixture(vec![]);
    f.proxy.catalog = context_tests::public_catalog::catalog().unwrap();
    f.proxy.scripted.as_ref().unwrap().borrow_mut().model = Some(model);
    let creation = f.proxy.recipes.form_band(ADD).unwrap();
    let mut req = request();
    req["messages"] = json!(history);
    drive(&mut f, &mut s, &mut req);
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert!(
        !log.contains("\"feature_change\"") && !log.contains("named_feature"),
        "{log}"
    );
    let first = &offered(&f)[0];
    let id = if first["recipes"].is_array() {
        &first["recipes"][0]["id"]
    } else {
        &first["recipe"]
    };
    assert_eq!(id, &creation["recipes"][0]["id"], "{log}");
}

#[test]
fn empty_design_change_wording_is_unchanged() {
    let mut old = fill_fixture(vec![call("pilot_fill", json!({"decline":true}))]);
    old.proxy
        .recipes
        .recipes
        .retain(|r| r.id != "named_feature");
    let mut new = fill_fixture(vec![call("pilot_fill", json!({"decline":true}))]);
    let mut req = request();
    req["messages"] = json!([{"role":"user","content":CHANGE}]);
    let a = old.chat(req.clone());
    let b = new.chat(req);
    assert_eq!(a.status, b.status);
    let log = fs::read_to_string(new.dir.join("log.jsonl")).unwrap();
    assert!(
        !log.contains("\"feature_change\"") && !log.contains("form_inspect"),
        "{log}"
    );
    let a = old.proxy.scripted.as_ref().unwrap().borrow();
    let b = new.proxy.scripted.as_ref().unwrap().borrow();
    assert_eq!(
        serde_json::to_vec(&a.requests).unwrap(),
        serde_json::to_vec(&b.requests).unwrap()
    );
}
