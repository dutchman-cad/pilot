#![allow(clippy::unwrap_used)]
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dutchman-pilot"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn discovery_from_the_binary() {
    for (task, expected) in [
        ("round all edges 3 mm", "solid.fillet"),
        ("hollow out the box", "solid.shell"),
    ] {
        let output = run(&["how", task]);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(expected));
        assert!(text.contains("\"tool\":\"execute\""));
        assert!(text.contains("\"params\""));
        assert!(output.stderr.is_empty());
    }
    let output = run(&["how", "make it fly"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("SolidCraft has no command for that"));
    assert!(text.contains("Nearest:"));
    assert!(!text.contains("\"tool\""));
}

#[test]
fn help_version_and_flag_errors() {
    let output = run(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("dutchman-pilot {}\n", env!("CARGO_PKG_VERSION"))
    );
    let output = run(&["--help"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("dutchman-pilot how"));
    let output = run(&["model", "--fallback-prompt", "bad"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("auto, on, or off"));
    for count in ["-1", "4097", "many"] {
        let output = run(&["model", "--trim-keep", count]);
        assert!(!output.status.success());
    }
}

#[test]
fn direct_recipe_configuration_errors_from_binary() {
    for (args, expected) in [
        (
            vec!["model", "--direct-recipes", "--no-direct-recipes"],
            "cannot be used with",
        ),
        (
            vec!["model", "--upstream", "none", "--no-direct-recipes"],
            "--upstream none cannot be combined with --no-direct-recipes",
        ),
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr).unwrap().contains(expected));
    }
}

#[path = "../examples/support/flags.rs"]
mod flags;

#[test]
fn documented_flags_match_binary_help() {
    let main = run(&["--help"]);
    assert!(main.status.success());
    let eval = Command::new(env!("CARGO_BIN_EXE_dutchman-pilot-eval"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(eval.status.success());
    let main = String::from_utf8(main.stdout).unwrap();
    let eval = String::from_utf8(eval.stdout).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/flags.md");
    let doc = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    assert_eq!(
        doc,
        flags::reference(&main, &eval),
        "regenerate docs/flags.md with cargo run --locked --example generate_flags"
    );
    let combined = format!("{main}\n{eval}");
    let documented: std::collections::BTreeSet<_> = doc
        .lines()
        .filter_map(|line| line.strip_prefix("- `").and_then(|s| s.strip_suffix('`')))
        .collect();
    assert_eq!(documented, flags::flags(&combined));
}

#[test]
fn library_accepts_documented_invocations() {
    use clap::Parser;
    use dutchman_pilot::cli::Command as Pilot;
    // README and docs examples, with documentation placeholders made concrete.
    for args in [
        vec!["model", "--upstream", "http://localhost:11434/v1"],
        vec![
            "model",
            "--upstream",
            "http://localhost:11434/v1",
            "--log",
            "pilot-bench.jsonl",
        ],
        vec![
            "model",
            "--upstream",
            "http://localhost:11434/v1",
            "--log",
            "rewrites.jsonl",
        ],
        vec![
            "model",
            "--upstream",
            "http://localhost:1234/v1",
            "--log",
            "rewrites.jsonl",
        ],
        vec![
            "mcp",
            "--solidcraft-cli",
            "/path/to/solidcraft-cli",
            "--",
            "--in",
            "part.solidcraft",
        ],
        vec![
            "mcp",
            "--solidcraft-cli",
            "/path/to/solidcraft-cli",
            "--",
            "--in",
            "/path/to/part.solidcraft",
        ],
        vec![
            "mcp",
            "--solidcraft-cli",
            "/path/to/solidcraft-cli",
            "--",
            "--connect",
            "127.0.0.1:7878",
        ],
        vec!["how", "round all edges 3 mm"],
        vec!["how", "hollow out the box"],
        vec!["profile"],
        vec!["profile", "--json"],
        vec!["profile", "qwen2.5:7b"],
        vec!["profile", "qwen2.5:7b", "--json"],
        vec!["log-summary", "pilot-bench.jsonl", "--json"],
        vec!["log-summary", "rewrites.jsonl"],
        vec!["log-summary", "rewrites.jsonl", "--json"],
    ] {
        assert!(
            Pilot::try_parse_from(std::iter::once("dutchman-pilot").chain(args.clone())).is_ok(),
            "{args:?}"
        );
    }
    assert_eq!(
        dutchman_pilot::cli::run(
            Pilot::try_parse_from(["dutchman-pilot", "how", "round all edges 3 mm"]).unwrap()
        )
        .unwrap(),
        0
    );
}

#[test]
fn embedding_as_a_clap_subcommand() {
    use clap::Parser;
    #[derive(Parser)]
    struct Dutchman {
        #[command(subcommand)]
        command: DutchmanCommand,
    }
    #[derive(clap::Subcommand)]
    enum DutchmanCommand {
        Pilot(dutchman_pilot::cli::Command),
    }
    let DutchmanCommand::Pilot(command) =
        Dutchman::try_parse_from(["dutchman", "pilot", "how", "round all edges 3 mm"])
            .unwrap()
            .command;
    assert_eq!(dutchman_pilot::cli::run(command).unwrap(), 0);
}

#[derive(Debug, clap::Parser)]
#[command(name = "dutchman")]
struct Host {
    #[command(subcommand)]
    action: HostAction,
}

#[derive(Debug, clap::Subcommand)]
enum HostAction {
    #[command(name = "pilot")]
    Pilot(dutchman_pilot::cli::Command),
}

#[test]
fn embedded_help_names_the_parent_command() {
    use clap::CommandFactory;
    let host = Host::command().mut_subcommand("pilot", |c| {
        dutchman_pilot::cli::with_bin_name(c, "dutchman pilot")
    });
    let help = host
        .try_get_matches_from(["dutchman", "pilot", "--help"])
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(help.contains("dutchman pilot how"), "{help}");
    assert!(!help.contains("dutchman-pilot how"), "{help}");
    assert_eq!(
        dutchman_pilot::cli::help_named("dutchman pilot"),
        dutchman_pilot::cli::help().replace("dutchman-pilot", "dutchman pilot")
    );
}

#[test]
fn report_offline_cli_and_request_consent() {
    let path = std::env::temp_dir().join(format!("pilot-offline-report-{}.md", std::process::id()));
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/planner-replay/box.json");
    // A non-loopback listen would fail if the offline command ever tried fetching.
    let output = run(&[
        "report",
        "--from-request",
        input.to_str().unwrap(),
        "--listen",
        "192.0.2.1:1",
        "--out",
        path.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(&file));
    assert!(text.contains("https://github.com/dutchman-cad/pilot/issues/new?title="));
    assert!(!file.contains("Request text (explicitly"));
    assert!(file.contains("Request shape:"));
    let output = run(&[
        "report",
        "--from-request",
        input.to_str().unwrap(),
        "--include-request",
        "--out",
        path.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("Request text (explicitly included; redacted)"));
    std::fs::remove_file(path).unwrap();
    assert!(!run(&["report", "--turn", "0"]).status.success());
    assert!(!run(&[
        "report",
        "--turn",
        "1",
        "--from-request",
        input.to_str().unwrap()
    ])
    .status
    .success());
}
