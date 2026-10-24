#![allow(clippy::unwrap_used, clippy::panic)]
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dutchman-pilot-eval"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn binary_offline_rescore_reports_and_exit_codes() {
    let dir = std::env::temp_dir().join(format!("dutchman-pilot-eval-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let floors = dir.join("floors.json");
    fs::write(
        &floors,
        r#"{"floors":{"create":1,"edit":1,"query":1,"workspace":1,"refuse":1}}"#,
    )
    .unwrap();
    let raw=json!({"choices":[{"message":{"role":"assistant","content":"A cube has six square faces."}}]}).to_string();
    let results:Vec<Value>=["create","edit","query","workspace","refuse"].iter().map(|bucket|json!({
        "case":{"id":bucket,"utterance":"What is a cube?","state":"Empty design","bucket":bucket,"claim":{"kind":"answers_in_words"}},
        "cached":false,"classification":"broken","turns":[{"recording":{"status":200,"raw":raw,"error":null,"latency_ms":10,"tokens":null,"hand_backs":null,"retry_after":null},"score":{"verdict":"broken","path":"prose","command_correct":null,"arguments_correct":null,"reason":"old judge"}}]
    })).collect();
    let mut report = json!({"judge_version":0,"model":"fake","base_url":"http://unused.local/v1","repetitions":1,"generated_at":0,"buckets":{},"results":results,"floors_met":false});
    let input = dir.join("recordings.json");
    fs::write(&input, report.to_string()).unwrap();
    let output = dir.join("out");
    let path = |p: &Path| p.to_str().unwrap().to_owned();
    let args = [
        "--rescore".to_string(),
        path(&input),
        "--floors".into(),
        path(&floors),
        "--output".into(),
        path(&output),
    ];
    let invoke = || run(&args.iter().map(String::as_str).collect::<Vec<_>>());
    let success = invoke();
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(output.join("results.json")).unwrap()).unwrap();
    assert_eq!(saved["floors_met"], true);
    assert_eq!(saved["judge_version"], 1);
    assert!(fs::read_to_string(output.join("results.md"))
        .unwrap()
        .contains("| Case |"));
    report["results"][0]["case"]["claim"] = json!({"kind":"refuses"});
    fs::write(&input, report.to_string()).unwrap();
    assert_eq!(invoke().status.code(), Some(1));
    assert_eq!(run(&["--repetitions", "0"]).status.code(), Some(2));
    assert!(run(&["--help"]).status.success());
    fs::remove_dir_all(dir).unwrap();
}
