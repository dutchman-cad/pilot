//! Test-only MCP binary; compiled by the integration test, never part of the product.
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
const CATALOG: &str = r#"[{"id":"solid.box","label":"Box","params":"length, width, height: number; operation?: new"},{"id":"solid.fillet","label":"Fillet","params":"edges: [[x,y,z]]; radius: number"},{"id":"sketch.finish","label":"Finish sketch","params":""}]"#;
fn main() {
    if std::env::args().nth(1).as_deref() == Some("commands") {
        println!("{CATALOG}"); return;
    }
    assert_eq!(std::env::args().nth(1).as_deref(), Some("mcp"));
    assert!(std::env::args().any(|s| s == "--test-arg"));
    let mut changes = 0;
    let mut inspections = 0;
    let fallback = std::env::args().any(|s| s == "--fallback");
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        let id = &v["id"];
        if v.get("method").is_none() {
            assert_eq!(v["result"], "client answer");
            println!("{{\"jsonrpc\":\"2.0\",\"id\":77,\"result\":\"reverse completed\"}}");
            continue;
        }
        if id.is_null() { continue; }
        if v["method"] == "raw" {
            // Echo the client's raw JSON string to prove its whitespace was preserved.
            println!("{{  \"jsonrpc\": \"2.0\", \"id\": {id}, \"result\": {}  }}", json!(line));
            continue;
        }
        if v["method"] == "reverse" {
            println!("{{ \"jsonrpc\":\"2.0\",\"id\":\"reverse-id\",\"method\":\"sampling/createMessage\",\"params\":{{}} }}");
            io::stdout().flush().unwrap(); continue;
        }
        if v["params"]["name"] == "failed_task" {
            println!("{}", json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"request failed"}}));
            io::stdout().flush().unwrap(); continue;
        }
        if v["params"]["name"] == "execute"
            && v["params"]["arguments"]["command"] == "solid.bxo"
            && std::env::args().any(|s| s == "--jsonrpc-errors")
        {
            let message = v["params"]["arguments"]["params"]["error"]
                .as_str().unwrap_or("unknown command solid.bxo");
            println!("{}", json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":message}}));
            if io::stdout().flush().is_err() { return; }
            continue;
        }
        let mut result = json!({"ok":true});
        if v["method"] == "tools/list" {
            result = json!({"tools":[{"name":"execute"},{"name":"inspect_design"},{"name":"list_commands"}]});
        } else if v["method"] == "tools/call" {
            let args = &v["params"]["arguments"];
            let name = v["params"]["name"].as_str().unwrap();
            let data = match name {
                "list_commands" if fallback => {
                    result = json!({"isError":true,"content":[{"type":"text","text":"unavailable"}]}); Value::Null
                }
                "list_commands" => serde_json::from_str(CATALOG).unwrap(),
                "describe_command" => json!({"input":{"type":"object","properties":{"length":{"type":"number"},"width":{"type":"number"},"height":{"type":"number"}},"required":["length","width","height"]}}),
                "inspect_design" => { inspections += 1; json!({"bodies":[{"name":"Body1","volume_mm3":100}],"sketches":[],"active_sketch":null}) },
                "open_sketch" => json!({"bodies":[],"active_sketch":1,"sketches":[{"name":"Sketch1","dof":2}]}),
                "pages" => json!({"items":[1,2],"total":9,"truncated":true}),
                "unknown_total" => json!({"items":[],"truncated":true}),
                "execute" if args["command"] == "solid.bxo" || args["command"] == "make a 20 mm cube" => {
                    let message = args["params"]["error"].as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("unknown command {}", args["command"]));
                    result = json!({"isError":true,"content":[{"type":"text","text":message}]});
                    Value::Null
                }
                "execute" if args["params"].get("Height").is_some() => {
                    result = json!({"isError":true,"content":[{"type":"text","text":"unknown parameter Height; missing required height"}]});
                    Value::Null
                }
                "execute" if args["command"] == "sketch.dimension" => {
                    result = json!({"isError":true,"content":[{"type":"text","text":"`entities` must list one or two sketch entities"}]});
                    Value::Null
                }
                "execute" if args["command"].as_str().is_some_and(|s| s.starts_with("sketch.constraint.")) => json!({"existing":true}),
                "execute" if args["command"] == "solid.box" && args["params"]["length"].is_number() => {
                    changes += 1;
                    json!({"created_bodies":["Body1"]})
                }
                "batch" if args["commands"][0]["params"]["length"].is_number() => {
                    changes += 1;
                    json!({"ok":true,"results":[{"created_bodies":["Body1"]}]})
                }
                "execute" | "batch" => { changes += 1; json!({"changed":false}) }
                "count" => json!({"executions":changes,"inspections":inspections}),
                _ => json!({"ok":true}),
            };
            if !data.is_null() { result = json!({"content":[{"type":"text","text":data.to_string()}]}); }
        }
        println!("{}", json!({"jsonrpc":"2.0","id":id,"result":result}));
        io::stdout().flush().unwrap();
    }
}
