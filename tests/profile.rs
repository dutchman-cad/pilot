#![allow(clippy::unwrap_used)]
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dutchman-pilot"))
        .args(args)
        .output()
        .unwrap()
}
fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}
fn output_json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "dutchman-pilot-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fixture_profiles_json_and_table() {
    let dir = fixture("profile");
    let records = output_json(&["profile", "--profile-dir", &dir, "--json"]);
    assert_eq!(records.as_array().unwrap().len(), 3);
    let first = &records[0];
    for (key, value) in [
        ("turns", 8),
        ("empty", 1),
        ("repairs", 5),
        ("hand_backs", 2),
        ("provider_errors", 1),
        ("ollama_500_recoveries", 2),
        ("provider_500_retries", 6),
        ("plans_used", 3),
        ("how_answers", 4),
        ("hints_added", 5),
    ] {
        assert_eq!(first[key], value, "{key}");
    }
    assert_eq!(first["native_percent"], 40.0);
    assert_eq!(first["text_written_percent"], 30.0);
    assert_eq!(first["prose_percent"], 20.0);
    assert_eq!(first["repairs_by_kind"]["argument_repair"], 3);
    assert_eq!(records[1]["native_percent"], 0.0);
    assert_eq!(records[2]["provider_errors"], 0);
    assert_eq!(records[2]["provider_500_retries"], 0);
    assert_eq!(records[2]["repairs_by_kind"], json!({}));
    let selected = output_json(&["profile", "alpha", "--json", "--profile-dir", &dir]);
    assert_eq!(selected.as_array().unwrap().len(), 2);
    assert_eq!(
        output_json(&["profile", "missing", "--profile-dir", &dir, "--json"]),
        json!([])
    );
    let output = run(&["profile", "--profile-dir", &dir]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "alpha @ http://a/v1",
        "legacy @ http://old/v1",
        "40.0",
        "30.0",
        "20.0",
        "argument_repair=3",
        "ollama-500-recoveries",
        "provider-500-retries",
        "plans-used",
        "how-answers",
        "hints-added",
        "including retries",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
}

#[test]
fn fixture_log_groups_and_final_line_without_newline() {
    let path = fixture("rewrites.jsonl");
    let summary = output_json(&["log-summary", &path, "--json"]);
    assert_eq!(summary["total"], 5);
    assert_eq!(
        summary["by_kind"],
        json!({"text_tool_call":2,"argument_repair:params":2,"provider_error":1})
    );
    assert_eq!(
        summary["by_model"]["alpha"],
        json!({"total":3,"by_kind":{"text_tool_call":1,"argument_repair:params":1,"provider_error":1}})
    );
    assert_eq!(summary["by_model"]["beta"]["total"], 2);
    let text = String::from_utf8(run(&["log-summary", &path]).stdout).unwrap();
    assert!(text.contains("events: 5"));
    assert!(text.contains("alpha  3"));
    let scratch = Scratch::new();
    let path = scratch.write("one.jsonl", b"{\"model\":\"a\",\"kind\":\"x\"}");
    assert_eq!(
        output_json(&["log-summary", path.to_str().unwrap(), "--json"])["total"],
        1
    );
    let path = scratch.write("empty.jsonl", b" \n\n");
    assert_eq!(
        output_json(&["log-summary", path.to_str().unwrap(), "--json"]),
        json!({"total":0,"by_kind":{},"by_model":{}})
    );
}

#[test]
fn errors_empty_profiles_and_bounded_inputs() {
    for args in [
        vec!["profile", "--json", "--json"],
        vec!["profile", "a", "b"],
        vec!["profile", "--profile-dir"],
        vec!["profile", "--wat"],
        vec!["log-summary"],
        vec!["log-summary", "a", "b"],
        vec!["log-summary", "a", "--json", "--json"],
    ] {
        assert!(!run(&args).status.success(), "{args:?}");
    }
    let scratch = Scratch::new();
    assert_eq!(
        output_json(&[
            "profile",
            "--profile-dir",
            scratch.0.to_str().unwrap(),
            "--json"
        ]),
        json!([])
    );
    for (content, error) in [
        (b"{}\nnot json\n".as_slice(), ":1:"),
        (b"\n{\"model\":\"a\"}\n", ":2:"),
        (b"{\"model\":\"\",\"kind\":\"x\"}", "1–4096 bytes"),
    ] {
        let path = scratch.write("bad.jsonl", content);
        let output = run(&["log-summary", path.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
    }
    let path = scratch.write("huge.jsonl", vec![b' '; 8 * 1_048_576 + 1]);
    assert!(dutchman_pilot::profile::log_summary(&path, false)
        .unwrap_err()
        .contains("8 MiB"));
    scratch.write("profiles.json", b"{");
    assert!(
        dutchman_pilot::profile::profiles(Some(&scratch.0), None, true)
            .unwrap_err()
            .contains("invalid local JSON")
    );
    let profiles: serde_json::Map<String, Value> = (0..257)
        .map(|n| (n.to_string(), json!({"model":"a"})))
        .collect();
    scratch.write("profiles.json", serde_json::to_vec(&profiles).unwrap());
    assert!(
        dutchman_pilot::profile::profiles(Some(&scratch.0), None, true)
            .unwrap_err()
            .contains("256 models")
    );
    scratch.write("profiles.json", vec![b' '; 2 * 1_048_576 + 1]);
    assert!(
        dutchman_pilot::profile::profiles(Some(&scratch.0), None, true)
            .unwrap_err()
            .contains("exceeds")
    );
}
