//! Independently authored development examples, using only the public CLI.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{json, Value};
#[path = "support/edit_harness.rs"]
mod edit_harness;

fn start() -> Vec<Value> {
    vec![
        json!({"command":"solid.box","params":{"length":72,"width":44,"height":16}}),
        json!({"command":"solid.hole","params":{"position":[24,22,16],"diameter":8,"depth":7}}),
    ]
}
fn call(id: &str, command: &str, params: Value) -> Value {
    json!({"role":"assistant","content":null,"tool_calls":[{"id":id,"type":"function",
        "function":{"name":"execute","arguments":json!({"command":command,"params":params}).to_string()}}]})
}
fn result(id: &str, value: Value) -> Value {
    json!({"role":"tool","tool_call_id":id,"content":value.to_string()})
}
fn inspected(cli: &std::path::PathBuf, text: &str) -> Vec<Value> {
    let reads = [
        ("document.inspect", json!({"measure":true})),
        ("timeline.inspect", json!({"feature":"Box1"})),
        ("timeline.inspect", json!({"feature":"Hole1"})),
    ];
    let mut script = start();
    for (command, params) in &reads {
        script.push(json!({"command":command,"params":params}));
    }
    let results = edit_harness::run_script(cli, &script);
    let mut messages = vec![json!({"role":"user","content":text})];
    for (i, (command, params)) in reads.into_iter().enumerate() {
        let id = format!("model_read_{i}");
        messages.push(call(&id, command, params));
        messages.push(result(&id, results[start().len() + i].clone()));
    }
    messages
}
fn fact(request: &Value) -> Option<String> {
    edit_harness::systems(request)
        .into_iter()
        .find(|s| s.contains("Design:"))
}

#[test]
fn failed_edits_and_rebuilds_get_one_fact_with_replayable_size_suggestions() {
    let Some(cli) = edit_harness::cli() else {
        return;
    };
    let examples = [
        (
            "Please change Hole1 diameter to 11 mm",
            "Hole1",
            "diameter",
            "11",
        ),
        ("Set the hole diameter to 1.2 cm", "Hole1", "diameter", "12"),
        ("Resize Hole1 depth to 9 mm", "Hole1", "depth", "9"),
        ("Adjust the hole depth to 0.6 cm", "Hole1", "depth", "6"),
        ("Change the box length to 84 mm", "Box1", "length", "84"),
        ("Set the plate width to 51 mm", "Box1", "width", "51"),
        ("Resize the block height to 18 mm", "Box1", "height", "18"),
        ("Make the existing box 90 mm long", "Box1", "length", "90"),
        ("Make the existing plate 48 mm wide", "Box1", "width", "48"),
        ("Make the existing block 20 mm tall", "Box1", "height", "20"),
        ("Update Box1 length to 8 cm", "Box1", "length", "80"),
        ("Modify Box1 width to 4.6 cm", "Box1", "width", "46"),
        ("Edit Box1 height to 1.7 cm", "Box1", "height", "17"),
        (
            "Increase Hole1 diameter to 10 mm",
            "Hole1",
            "diameter",
            "10",
        ),
        ("Reduce the hole depth to 5 mm", "Hole1", "depth", "5"),
        (
            "Adjust the existing part width to 49 mm",
            "Box1",
            "width",
            "49",
        ),
    ];
    assert!(examples.len() >= 15);
    for (direct, rebuild) in [(false, false), (false, true), (true, false), (true, true)] {
        for (text, feature, input, value) in examples {
            let proxy = edit_harness::proxy(direct, false);
            let mut messages = inspected(&cli, text);
            proxy.send(&messages);
            assert!(fact(&proxy.last()).is_none(), "inspection alone: {text}");
            messages.push(if rebuild {
                call(
                    "wrong",
                    "solid.box",
                    json!({"length":84,"width":44,"height":16}),
                )
            } else {
                call(
                    "wrong",
                    "timeline.edit",
                    json!({"feature":"Missing","set":{"diameter":"11"}}),
                )
            });
            messages.push(result(
                "wrong",
                if rebuild {
                    json!({"created_bodies":["Body2"]})
                } else {
                    json!({"isError":true,"error":"feature Missing not found"})
                },
            ));
            let before = messages.clone();
            proxy.send(&messages);
            let upstream = proxy.last();
            let f = fact(&upstream).unwrap_or_else(|| panic!("{text}: {upstream}"));
            assert!(f.chars().count() <= 1240);
            assert!(f.contains("Body1 box 72 x 44 x 16"), "{f}");
            for message in before.iter().filter(|m| m["role"] == "assistant") {
                assert!(upstream["messages"].as_array().unwrap().contains(message));
            }
            let params_text = f
                .split_once("execute timeline.edit ")
                .unwrap()
                .1
                .trim_end_matches('.');
            let params: Value = serde_json::from_str(params_text).unwrap();
            assert_eq!(params["feature"], feature);
            assert_eq!(params["set"][input], value);
            let mut script = start();
            script.push(json!({"command":"timeline.edit","params":params}));
            script.push(json!({"command":"timeline.inspect","params":{"feature":feature}}));
            let replay = edit_harness::run_script(&cli, &script);
            let changed = replay.last().unwrap()["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["label"].as_str().unwrap().eq_ignore_ascii_case(input))
                .unwrap();
            assert_eq!(changed["expression"], value);
            // Same failing continuation and then another mistake: no second fact.
            proxy.send(&messages);
            assert!(fact(&proxy.last()).is_none());
            messages.push(call(
                "wrong_again",
                "timeline.edit",
                json!({"feature":"Missing","set":{"height":"2"}}),
            ));
            messages.push(result("wrong_again", json!({"ok":false,"error":"missing"})));
            proxy.send(&messages);
            assert!(fact(&proxy.last()).is_none());
            let log = std::fs::read_to_string(proxy.dir.join("audit.jsonl")).unwrap();
            assert_eq!(
                log.lines()
                    .filter(|l| serde_json::from_str::<Value>(l).unwrap()["kind"] == "edit_fact")
                    .count(),
                1
            );
            assert!(!log.contains("edit_fact_inspect"));
            if text == examples[0].0 {
                // A new user turn earns its own single fact after fresh model reads.
                messages.extend(inspected(&cli, text));
                messages.push(call(
                    "next_turn",
                    "timeline.edit",
                    json!({"feature":"Missing","set":{"diameter":"11"}}),
                ));
                messages.push(result(
                    "next_turn",
                    json!({"isError":true,"error":"missing"}),
                ));
                proxy.send(&messages);
                assert!(fact(&proxy.last()).is_some());
            }
        }
    }
}

#[test]
fn ten_continuations_must_not_get_a_fact() {
    let Some(cli) = edit_harness::cli() else {
        return;
    };
    for scenario in 0..10 {
        let proxy = edit_harness::proxy(false, scenario == 9);
        let text = match scenario {
            0 => "Create a new 84 mm box",
            1 => "What is the hole diameter?",
            _ => "Change the box length to 84 mm",
        };
        let mut messages = inspected(&cli, text);
        match scenario {
            2 => (), // inspection alone
            3 => {
                messages.truncate(1);
            } // no model action
            4 => {
                // unknown/unmatched result cannot supply evidence
                messages.retain(|m| m["role"] != "assistant");
            }
            5 => {
                // Pilot form reads cannot supply evidence
                for m in &mut messages {
                    if let Some(calls) = m["tool_calls"].as_array_mut() {
                        calls[0]["id"] = json!("pilot_context_0");
                    }
                }
            }
            6 => {
                // empty design
                messages.truncate(1);
                messages.push(call("empty", "document.inspect", json!({"measure":true})));
                messages.push(result("empty", json!({"bodies":[],"timeline":[]})));
            }
            _ => (),
        }
        if !matches!(scenario, 2 | 3) {
            messages.push(call(
                "last",
                if scenario == 7 {
                    "timeline.edit"
                } else if scenario == 8 {
                    "timeline.inspect"
                } else {
                    "solid.box"
                },
                json!({"feature":"Box1","length":84,"width":44,"height":16,"set":{"length":"84"}}),
            ));
            messages.push(result(
                "last",
                if scenario == 8 {
                    json!({"isError":true})
                } else {
                    json!({"ok":true})
                },
            ));
        }
        proxy.send(&messages);
        assert!(fact(&proxy.last()).is_none(), "scenario {scenario}");
    }
}
