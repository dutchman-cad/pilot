//! Serial reference, no-action, and negative validation through Pilot's driver.
mod check;
use dutchman_pilot::solidcraft98::{drive, mcp, transport::Endpoint};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

fn read(path: &Path) -> Result<Value, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("benchmark file exceeds 1 MiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}
fn command(session: &mut mcp::Server, wire: &str) -> Result<String, String> {
    let call = drive::parse_text(&format!("COMMAND: {wire}"), false)?;
    let drive::Call::Tool { name, args } = call else {
        return Err("done is not a setup or probe command".into());
    };
    let result = session.call(&name, &args);
    if result["isError"] == true {
        return Err(format!("{wire}: {result}"));
    }
    result["content"][0]["text"]
        .as_str()
        .map(str::to_owned)
        .ok_or("missing remote text".into())
}
fn snapshot(session: &mut mcp::Server) -> Result<Value, String> {
    let screen = command(session, "look")?;
    let mut fields = serde_json::Map::new();
    let mut probes = Vec::new();
    let mut incomplete = screen.contains("Dialog \"");
    if !incomplete {
        for row in check::rows(&screen)? {
            if !["Sketch", "Extrude", "Revolve", "Hole"].iter().any(|p| {
                row[1]
                    .strip_prefix(p)
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            }) {
                incomplete = true;
                continue;
            }
            let probe = command(session, &format!("doubleclick {}", row[1]))?;
            probes.push(json!({"feature":row[1],"screen":probe}));
            fields.insert(row[1].clone(), check::fields(&probe));
            if probe.contains("Dialog \"") {
                command(session, "key esc")?;
            } else {
                incomplete = true;
            }
        }
    }
    Ok(json!({"screen":screen,"fields":fields,"probes":probes,"incomplete":incomplete}))
}
fn script(value: &Value) -> Result<Vec<String>, String> {
    let values = value.as_array().ok_or("commands must be an array")?;
    if values.len() > 128 {
        return Err("reference exceeds 128 commands".into());
    }
    values
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| s.len() <= 4096)
                .map(str::to_owned)
                .ok_or("command must be a string of at most 4096 bytes".into())
        })
        .collect()
}
fn run(
    program: &Path,
    directory: &Path,
    task: &Value,
    reference: &Value,
    mode: &str,
) -> Result<Value, String> {
    fs::create_dir(directory).map_err(|e| e.to_string())?;
    let endpoint = Endpoint::Program {
        path: program.to_owned(),
        headless: true,
    };
    let log = directory.join("transcript.jsonl");
    let mut session = mcp::Server::new(mcp::Config {
        endpoint: endpoint.clone(),
        log: Some(log.clone()),
    })?;
    for line in script(&task["setup"])? {
        command(&mut session, &line)?;
    }
    let initial = snapshot(&mut session)?;
    if let Some(expected) = task.get("initial_check") {
        let setup_score = check::score(expected, &json!({}), &initial, &Value::Null)?;
        if setup_score["pass"] != true {
            return Err(format!("setup state differs: {setup_score}"));
        }
    }
    let mut commands: VecDeque<_> = if mode == "nothing" {
        Vec::new().into()
    } else {
        script(&reference["commands"])?.into()
    };
    let answer = if mode == "nothing" {
        "No action taken."
    } else {
        task["reference_reply"].as_str().unwrap_or("Done.")
    };
    let summary = drive::run_in_session(
        drive::Config {
            endpoint,
            log: Some(log),
            upstream: "http://127.0.0.1:1/v1".into(),
            model: "reference".into(),
            task: task["prompt"].as_str().ok_or("task needs prompt")?.into(),
            plain: false,
            budget: 80,
            wall: Duration::from_secs(120),
            tokens: 131072,
        },
        &mut session,
        |_, _, _| {
            let command = commands
                .pop_front()
                .unwrap_or_else(|| format!("done {answer}"));
            Ok(
                json!({"choices":[{"message":{"role":"assistant","content":format!("COMMAND: {command}")}}],"usage":{"total_tokens":100}}),
            )
        },
    )?;
    if !commands.is_empty() || summary["stop_reason"] != "done" || summary["errors"] != 0 {
        return Err(format!("reference execution failed: {summary}"));
    }
    let final_state = snapshot(&mut session)?;
    let score = check::score(&task["check"], &summary, &final_state, &initial)?;
    Ok(
        json!({"id":task["id"],"mode":mode,"initial":initial,"final":final_state,"summary":summary,"score":score}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || !matches!(args[1].as_str(), "reference" | "nothing" | "negatives") {
        return Err("usage: cargo run --example bench_sc98 -- PROGRAM reference|nothing|negatives OUTPUT_DIRECTORY".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bench/sets/sc98");
    let mode = &args[1];
    let tasks = read(&root.join(if mode == "negatives" {
        "negatives/tasks.json"
    } else {
        "tasks.json"
    }))?;
    let tasks = tasks["tasks"]
        .as_array()
        .filter(|t| !t.is_empty() && t.len() <= 256)
        .ok_or("expected 1–256 tasks")?;
    let expectations = read(&root.join("do-nothing.json"))?;
    let out = PathBuf::from(&args[2]);
    fs::create_dir(&out).map_err(|e| format!("output directory must be new: {e}"))?;
    let mut failures = 0;
    for (i, task) in tasks.iter().enumerate() {
        let relative = Path::new(task["reference"].as_str().ok_or("missing reference")?);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("reference path must stay in set directory".into());
        }
        let reference = read(&root.join(relative))?;
        let result = run(
            Path::new(&args[0]),
            &out.join(format!("{i:02}")),
            task,
            &reference,
            mode,
        );
        let expected = if mode == "nothing" {
            expectations["expectations"]
                .as_array()
                .and_then(|a| a.iter().find(|e| e["id"] == task["id"]))
                .and_then(|e| e["pass"].as_bool())
                .ok_or("missing do-nothing expectation")?
        } else {
            mode == "reference"
        };
        let (record, matched) = match result {
            Ok(record) => {
                let matched = record["score"]["pass"] == expected;
                (record, matched)
            }
            Err(error) => (json!({"id":task["id"],"error":error}), false),
        };
        if !matched {
            failures += 1;
        }
        println!(
            "{}: {} (expected pass={expected})",
            task["id"],
            if matched { "PASS" } else { "FAIL" }
        );
        fs::write(
            out.join(format!("{i:02}.json")),
            serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    if failures > 0 {
        Err(format!("{failures}/{} unexpected outcomes", tasks.len()))
    } else {
        Ok(())
    }
}
