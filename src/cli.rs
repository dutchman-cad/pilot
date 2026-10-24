//! Synchronous command-line entry point, shared by the binary and embedding applications.
use crate::{catalog::Catalog, mcp, model, profile};
use clap::{Parser, Subcommand};
use std::{
    fmt,
    io::{self, Write},
    net::SocketAddr,
    path::PathBuf,
};

const HELP_TEMPLATE: &str = r#"{display} — a local helper for SolidCraft

Usage:
  {binary} how "<task>"
  {binary} model [--listen 127.0.0.1:11500] [--upstream URL]
  {pad}       [--upstream-timeout SECONDS (default 300, max 3600)]
  {pad}       [--fallback-prompt auto|on|off] [--prompt-help] [--log FILE]
  {pad}       [--profile-dir DIR] [--model-settings [FILE]] [--seed INTEGER]
  {pad}       [--no-how] [--no-hints] [--planner | --no-plan (alias --no-planner)]
  {binary} profile [model] [--profile-dir DIR] [--json]
  {binary} log-summary <rewrites.jsonl> [--json]
  {binary} mcp [--solidcraft-cli PATH] [--log FILE]
  {pad}     [--no-plan] [--no-hints] [--no-how] -- <args for solidcraft-cli mcp>
  {binary} --version (-V)
  {binary} --help (-h)

how uses the bundled public command catalogue and prints explicit JSON example calls.
model serves /v1/chat/completions and /v1/models; mcp starts the SolidCraft tools proxy.
Profiles and the default rewrites.jsonl log live in the user's data dir/{binary}.
--prompt-help adds short rules and at most 15 relevant command purposes.
--model-settings enables Ollama-only temperature 0.1 with tools and num_ctx 8192
when absent; --seed requires --model-settings. All additions are logged.
Optional settings file: JSON keyed by model, with temperature (0–0.2), seed,
num_ctx (8192–131072), and json_retry (boolean, decided JSON retries only).
"#;

/// The same help used by the binary and the reference generator.
pub fn help() -> String {
    help_named(crate::identity::BINARY)
}

/// The help with `bin` (for example `dutchman pilot`) in place of the binary name.
pub fn help_named(bin: &str) -> String {
    HELP_TEMPLATE
        .replace("{binary}", bin)
        .replace("{pad}", &" ".repeat(bin.chars().count()))
        .replace("{display}", crate::identity::DISPLAY_NAME)
}

/// Name the embedded command in its top-level help, for example
/// `host.mut_subcommand("pilot", |c| dutchman_pilot::cli::with_bin_name(c, "dutchman pilot"))`.
/// Subcommand help already uses clap's own usage line with the parent's name.
pub fn with_bin_name(cmd: clap::Command, bin: &str) -> clap::Command {
    cmd.override_help(help_named(bin))
}

/// Pilot arguments. Embed this as a clap subcommand to retain the standalone flags.
#[derive(Debug, Parser)]
#[command(name = "dutchman-pilot", version, disable_help_subcommand = true, override_help = help())]
pub struct Command {
    #[command(subcommand)]
    action: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Find commands and example calls for a task using the bundled catalogue.
    How {
        #[arg(value_parser = task)]
        task: String,
    },
    /// Serve the OpenAI-compatible model proxy.
    Model(ModelArgs),
    /// Start the MCP tools proxy.
    Mcp(McpArgs),
    /// Serve the MCP tools face for SolidCraft 98.
    Mcp98(Mcp98Args),
    /// Show recorded model profiles.
    Profile {
        model: Option<String>,
        #[arg(long = "profile-dir")]
        dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a rewrite log.
    LogSummary {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Args)]
struct ModelArgs {
    #[arg(long, default_value = "127.0.0.1:11500")]
    listen: SocketAddr,
    #[arg(long, default_value = "http://localhost:11434/v1", value_parser = upstream)]
    upstream: String,
    #[arg(long, default_value = "300", value_parser = clap::value_parser!(u64).range(1..=3600))]
    upstream_timeout: u64,
    #[arg(long, default_value = "auto", value_parser = fallback)]
    fallback_prompt: model::FallbackPrompt,
    #[arg(long)]
    log: Option<PathBuf>,
    #[arg(long)]
    profile_dir: Option<PathBuf>,
    #[arg(long, num_args = 0..=1)]
    model_settings: Option<Option<PathBuf>>,
    #[arg(long, requires = "model_settings", allow_hyphen_values = true)]
    seed: Option<i64>,
    #[arg(long)]
    prompt_help: bool,
    #[arg(long)]
    no_how: bool,
    #[arg(long)]
    no_hints: bool,
    #[arg(long, conflicts_with = "no_plan")]
    planner: bool,
    #[arg(long, alias = "no-planner")]
    no_plan: bool,
}

#[derive(Debug, clap::Args)]
struct Mcp98Args {
    #[arg(long, required_unless_present = "program", conflicts_with = "program")]
    connect: Option<SocketAddr>,
    #[arg(long, required_unless_present = "connect")]
    program: Option<PathBuf>,
    #[arg(long, requires = "program", conflicts_with = "connect")]
    headless: bool,
    #[arg(long)]
    log: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
struct McpArgs {
    #[arg(long, default_value = "solidcraft-cli")]
    solidcraft_cli: PathBuf,
    #[arg(long)]
    log: Option<PathBuf>,
    #[arg(long)]
    no_plan: bool,
    #[arg(long)]
    no_hints: bool,
    #[arg(long)]
    no_how: bool,
    #[arg(last = true)]
    args: Vec<String>,
}

fn task(value: &str) -> Result<String, String> {
    if value.trim().is_empty() || value.len() > crate::catalog::MAX_QUERY_BYTES {
        return Err("how task must contain 1–256 bytes of nonblank text".into());
    }
    Ok(value.into())
}
fn upstream(value: &str) -> Result<String, String> {
    let authority = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .and_then(|s| s.split('/').next())
        .filter(|s| !s.is_empty());
    if authority.is_none() || value.chars().any(char::is_whitespace) {
        return Err("upstream must be an http:// or https:// URL with a host".into());
    }
    Ok(value.into())
}
fn fallback(value: &str) -> Result<model::FallbackPrompt, String> {
    match value {
        "auto" => Ok(model::FallbackPrompt::Auto),
        "on" => Ok(model::FallbackPrompt::On),
        "off" => Ok(model::FallbackPrompt::Off),
        _ => Err("fallback-prompt must be auto, on, or off".into()),
    }
}

/// An error while executing a Pilot command.
#[derive(Debug)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Error {}
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// Execute a parsed command synchronously, returning the process exit status.
///
/// Successful commands return zero. Errors carry a printable diagnostic and do not exit
/// the calling process. Proxy commands block until their server stops.
pub fn run(cmd: Command) -> Result<i32, Error> {
    run_named(cmd, crate::identity::BINARY)
}

/// [`run`] for an embedding program: `bin` names the command in printed help,
/// for example `dutchman pilot`.
pub fn run_named(cmd: Command, bin: &str) -> Result<i32, Error> {
    validate_bounds(&cmd)?;
    let result = match cmd.action {
        None => output(&help_named(bin)),
        Some(Action::Model(args)) => model::serve(model::Config {
            listen: args.listen,
            upstream: args.upstream,
            upstream_timeout: std::time::Duration::from_secs(args.upstream_timeout),
            fallback_prompt: args.fallback_prompt,
            log: args.log,
            profile_dir: args.profile_dir,
            model_settings_enabled: matches!(args.model_settings, Some(None)),
            model_settings: args.model_settings.flatten(),
            seed: args.seed,
            prompt_help: args.prompt_help,
            no_how: args.no_how,
            no_hints: args.no_hints,
            planner: !args.no_plan,
        }),
        Some(Action::Mcp98(args)) => {
            let endpoint = match (args.connect, args.program) {
                (Some(address), None) => crate::solidcraft98::transport::Endpoint::Tcp(address),
                (None, Some(path)) => crate::solidcraft98::transport::Endpoint::Program {
                    path,
                    headless: args.headless,
                },
                _ => {
                    return Err(Error(
                        "exactly one SolidCraft 98 transport is required".into(),
                    ))
                }
            };
            crate::solidcraft98::mcp::serve(crate::solidcraft98::mcp::Config {
                endpoint,
                log: args.log,
            })
        }
        Some(Action::Mcp(args)) => mcp::serve(mcp::Config {
            solidcraft_cli: args.solidcraft_cli,
            args: args.args,
            log: args.log,
            no_plan: args.no_plan,
            no_hints: args.no_hints,
            no_how: args.no_how,
        }),
        Some(Action::Profile { model, dir, json }) => output(&format!(
            "{}\n",
            profile::profiles(dir.as_deref(), model.as_deref(), json)?
        )),
        Some(Action::LogSummary { path, json }) => {
            output(&format!("{}\n", profile::log_summary(&path, json)?))
        }
        Some(Action::How { task }) => {
            output(&format!("{}\n", Catalog::bundled()?.how_text(&task)?))
        }
    };
    result.map_err(Error::from)?;
    Ok(0)
}

fn validate_bounds(cmd: &Command) -> Result<(), Error> {
    let mut bytes = 0usize;
    let mut count = 1usize;
    let mut add = |len: usize| {
        bytes = bytes.saturating_add(len);
        count = count.saturating_add(1);
    };
    match &cmd.action {
        Some(Action::How { task }) => add(task.len()),
        Some(Action::Profile { model, dir, .. }) => {
            if let Some(model) = model {
                add(model.len());
            }
            if let Some(dir) = dir {
                add(dir.as_os_str().len());
            }
        }
        Some(Action::LogSummary { path, .. }) => add(path.as_os_str().len()),
        Some(Action::Model(args)) => {
            add(args.upstream.len());
            for path in args
                .log
                .iter()
                .chain(args.profile_dir.iter())
                .chain(args.model_settings.iter().flatten())
            {
                add(path.as_os_str().len());
            }
        }
        Some(Action::Mcp(args)) => {
            add(args.solidcraft_cli.as_os_str().len());
            if let Some(log) = &args.log {
                add(log.as_os_str().len());
            }
            for arg in &args.args {
                add(arg.len());
            }
        }
        Some(Action::Mcp98(args)) => {
            if let Some(path) = &args.program {
                add(path.as_os_str().len());
            }
            if let Some(path) = &args.log {
                add(path.as_os_str().len());
            }
        }
        None => {}
    }
    if bytes > 65536 || count > 256 {
        return Err(Error(
            "command line exceeds 256 arguments or 65536 bytes".into(),
        ));
    }
    Ok(())
}

fn output(text: &str) -> Result<(), String> {
    io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .map_err(|e| format!("cannot write output: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Command, clap::Error> {
        Command::try_parse_from(std::iter::once("dutchman-pilot").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_and_all_flags() {
        let Some(Action::Model(args)) = parse(&["model"]).unwrap().action else {
            panic!("expected model")
        };
        assert_eq!(args.listen.to_string(), "127.0.0.1:11500");
        assert_eq!(args.upstream, "http://localhost:11434/v1");
        assert_eq!(
            args.upstream_timeout,
            model::DEFAULT_UPSTREAM_TIMEOUT.as_secs()
        );
        assert_eq!(args.fallback_prompt, model::FallbackPrompt::Auto);
        assert!(!args.no_plan);
        assert!(parse(&[
            "model",
            "--listen",
            "[::1]:1234",
            "--upstream-timeout",
            "3600",
            "--fallback-prompt",
            "on",
            "--prompt-help",
            "--no-how",
            "--no-hints",
            "--no-planner",
            "--log",
            "log.jsonl",
            "--profile-dir",
            "profiles",
            "--model-settings",
            "models.json",
            "--seed",
            "-42"
        ])
        .is_ok());
        let Some(Action::Model(args)) = parse(&["model", "--model-settings", "--seed", "42"])
            .unwrap()
            .action
        else {
            panic!("expected model")
        };
        assert_eq!(args.model_settings, Some(None));
        assert_eq!(args.seed, Some(42));
        assert!(parse(&["model", "--planner", "--fallback-prompt", "off"]).is_ok());
        let Some(Action::Mcp(args)) = parse(&[
            "mcp",
            "--no-plan",
            "--no-how",
            "--no-hints",
            "--log",
            "mcp.jsonl",
            "--",
            "--help",
        ])
        .unwrap()
        .action
        else {
            panic!("expected mcp")
        };
        assert_eq!(args.args, ["--help"]);
        assert!(args.no_plan && args.no_how && args.no_hints);
    }

    #[test]
    fn invalid_arguments_and_bounds() {
        for args in [
            vec!["unknown"],
            vec!["how"],
            vec!["how", ""],
            vec!["how", "one", "two"],
            vec!["model", "--upstream"],
            vec!["model", "--upstream", "file:///tmp/x"],
            vec!["model", "--upstream", "http://"],
            vec!["model", "--upstream-timeout", "0"],
            vec!["model", "--upstream-timeout", "3601"],
            vec!["model", "--upstream-timeout", "abc"],
            vec!["model", "--listen", "bad"],
            vec!["model", "--fallback-prompt", "yes"],
            vec!["model", "--seed", "42"],
            vec!["model", "--no-plan", "--no-planner"],
            vec!["model", "--planner", "--no-plan"],
            vec!["model", "--log", "a", "--log", "b"],
            vec!["mcp", "--connect", "x"],
            vec!["mcp", "--solidcraft-cli"],
            vec!["mcp", "--no-how", "--no-how"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(parse(&["how", &"x".repeat(257)]).is_err());
        let long = "x".repeat(65537);
        assert!(run(parse(&["profile", &long]).unwrap()).is_err());
        let many = std::iter::once("dutchman-pilot")
            .chain(["mcp", "--"])
            .chain(std::iter::repeat_n("x", 257));
        assert!(run(Command::try_parse_from(many).unwrap()).is_err());
    }
}
