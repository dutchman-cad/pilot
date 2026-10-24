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
  {binary} model [--listen 127.0.0.1:11500] [--upstream URL|none] [--no-direct-recipes]
  {pad}       [--upstream-timeout SECONDS (default 300, max 3600)]
  {pad}       [--fallback-prompt auto|on|off] [--prompt-help] [--log FILE]
  {pad}       [--profile-dir DIR] [--model-settings [FILE]] [--seed INTEGER]
  {pad}       [--no-report-buffer] [--dialect solidcraft|solidcraft98]
  {pad}       [--no-how] [--no-hints] [--planner] [--no-plan (alias --no-planner)]
  {binary} report [--listen ADDR] [--turn N] [--include-request] [--out FILE]
  {pad}        [--from-request FILE] [--open]
  {binary} profile [model] [--profile-dir DIR] [--json]
  {binary} log-summary <rewrites.jsonl> [--json]
  {binary} mcp [--solidcraft-cli PATH] [--log FILE]
  {pad}     [--no-plan] [--no-hints] [--no-how] -- <args for solidcraft-cli mcp>
  {binary} mcp98 (--connect ADDRESS:9898 | --program PATH [--headless]) [--log FILE]
  {binary} drive98 (--connect ADDRESS:9898 | --program PATH [--headless])
  {pad}         --upstream URL --model MODEL [--plain] [--budget 1..200] [--log FILE] "task"
  {binary} how98 "task"
  {binary} --version (-V)
  {binary} --help (-h)

how uses the bundled public command catalogue and prints explicit JSON example calls.
model --dialect solidcraft98 selects SolidCraft 98 screen tools; catalogue recipes are disabled.
mcp98 serves SolidCraft 98 tools; drive98 runs its bounded model loop; how98 looks up screen help.
model serves /v1/chat/completions and /v1/models; mcp starts the SolidCraft tools proxy.
--planner enables recipe suggestions, matching action prose, and one completion check.
These plan-text features are off by default; model --no-plan is a no-op alias for
the default. The MCP face only suggests plans.
Exact matches are answered without a model by default; --no-direct-recipes sends
everything to the model. --direct-recipes is a no-op alias for the default;
it conflicts with --no-direct-recipes. Direct answering can accompany --planner.
Upstream defaults to Ollama; --upstream none runs without a model and requires
direct answering. Direct replies are marked, audited, and profiled separately.
report prints a redacted local file and issue URL; it never submits anything.
Type /report in the Assistant for a local report and Discord #pilot-help summary.
--include-request opts in to request words; --open opens the browser for review.
The model face keeps five diagnostic turns in memory; --no-report-buffer disables it.
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
    /// Drive SolidCraft 98 through a bounded model/session loop.
    #[command(
        long_about = "Drive SolidCraft 98 through the same validated session as mcp98. One explicit COMMAND: line, complete tool JSON object, or native tool call per turn. Plain mode disables Pilot repairs, name matching, suggestions, hints, and offline help. Default limits: 40 model turns, 900 seconds, 131072 total tokens, 512 output tokens per request, eight history turns / 1 MiB. At most two error hand-backs and one empty retry per turn; every request counts against --budget. done reads a final screen and detaches; it never quits the app. JSON summary on stdout; transcript and all rewrites in --log (default data-directory rewrites.jsonl)."
    )]
    Drive98(Drive98Args),
    /// Offline Pilot help for SolidCraft 98.
    How98 {
        #[arg(value_parser = task)]
        task: String,
    },
    /// Show recorded model profiles.
    Profile {
        model: Option<String>,
        #[arg(long = "profile-dir")]
        dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Write and print a redacted report for review and manual submission.
    Report(crate::report::ReportArgs),
    /// Summarize a rewrite log.
    LogSummary {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Args)]
struct ModelArgs {
    #[arg(long, default_value = "solidcraft", value_parser = ["solidcraft", "solidcraft98"], help = "SolidCraft 98 uses only screen tools and repairs; disables SolidCraft catalogue plans, how, and hints")]
    dialect: String,
    #[arg(long, default_value = "127.0.0.1:11500")]
    listen: SocketAddr,
    #[arg(long, value_parser = upstream)]
    upstream: Option<String>,
    /// Accepted no-op alias for default exact answers with normal approval and validation.
    #[arg(long, conflicts_with = "no_direct_recipes")]
    direct_recipes: bool,
    /// Send everything to the model instead of answering exact recipes directly.
    #[arg(long)]
    no_direct_recipes: bool,
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
    /// Enable plan-text recipe suggestions, prose conversion, and completion checks.
    #[arg(long)]
    planner: bool,
    /// Accepted no-op alias for the default; use --planner to enable plan-text recipes.
    #[arg(long, alias = "no-planner")]
    no_plan: bool,
    #[arg(long)]
    no_report_buffer: bool,
}

impl ModelArgs {
    fn direct_recipes_enabled(&self) -> Result<bool, String> {
        let enabled = !self.no_direct_recipes && self.dialect != "solidcraft98";
        if self.upstream.as_deref() == Some("none") && !enabled {
            return Err(if self.no_direct_recipes {
                "--upstream none cannot be combined with --no-direct-recipes; enable direct answering or provide an upstream URL"
            } else {
                "--upstream none requires direct answering, which is disabled for --dialect solidcraft98"
            }
            .into());
        }
        Ok(enabled)
    }
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
struct Drive98Args {
    #[command(flatten)]
    remote: Mcp98Args,
    #[arg(long, value_parser = upstream)]
    upstream: String,
    #[arg(long)]
    model: String,
    #[arg(long)]
    plain: bool,
    /// Maximum model turns (includes format retries); wall 900s, total tokens 131072.
    #[arg(long, default_value = "40", value_parser = clap::value_parser!(u16).range(1..=200))]
    budget: u16,
    task: String,
}
fn endpoint(args: &Mcp98Args) -> Result<crate::solidcraft98::transport::Endpoint, String> {
    use crate::solidcraft98::transport::Endpoint;
    match (&args.connect, &args.program) {
        (Some(a), None) => Ok(Endpoint::Tcp(*a)),
        (None, Some(path)) => Ok(Endpoint::Program {
            path: path.clone(),
            headless: args.headless,
        }),
        _ => Err("exactly one SolidCraft 98 transport required".into()),
    }
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
    if value == "none" {
        return Ok(value.into());
    }
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
        Some(Action::Model(args)) => {
            let direct_recipes = args.direct_recipes_enabled()?;
            model::serve(model::Config {
                solidcraft98: args.dialect == "solidcraft98",
                listen: args.listen,
                no_report_buffer: args.no_report_buffer,
                // No-model operation is explicit: --upstream none.
                upstream: args
                    .upstream
                    .unwrap_or_else(|| "http://localhost:11434/v1".into()),
                direct_recipes,
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
                planner: args.planner,
            })
        }
        Some(Action::How98 { task }) => {
            output(&format!("{}\n", crate::solidcraft98::guidance::help(&task)))
        }
        Some(Action::Drive98(args)) => {
            let summary = crate::solidcraft98::drive::run(crate::solidcraft98::drive::Config {
                endpoint: endpoint(&args.remote)?,
                upstream: args.upstream,
                model: args.model,
                task: args.task,
                plain: args.plain,
                log: args.remote.log,
                budget: usize::from(args.budget),
                wall: std::time::Duration::from_secs(900),
                tokens: 131072,
            })?;
            output(&format!("{summary}\n"))
        }
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
        Some(Action::Report(args)) => crate::report::run(args),
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
        Some(Action::How { task } | Action::How98 { task }) => add(task.len()),
        Some(Action::Drive98(args)) => {
            add(args.task.len());
            add(args.upstream.len());
            add(args.model.len());
            if let Some(p) = &args.remote.program {
                add(p.as_os_str().len());
            }
            if let Some(p) = &args.remote.log {
                add(p.as_os_str().len());
            }
        }
        Some(Action::Profile { model, dir, .. }) => {
            if let Some(model) = model {
                add(model.len());
            }
            if let Some(dir) = dir {
                add(dir.as_os_str().len());
            }
        }
        Some(Action::Report(args)) => {
            for path in args.out.iter().chain(args.from_request.iter()) {
                add(path.as_os_str().len());
            }
        }
        Some(Action::LogSummary { path, .. }) => add(path.as_os_str().len()),
        Some(Action::Model(args)) => {
            add(args.upstream.as_ref().map_or(0, String::len));
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
        assert_eq!(args.upstream, None);
        assert_eq!(
            args.upstream_timeout,
            model::DEFAULT_UPSTREAM_TIMEOUT.as_secs()
        );
        assert_eq!(args.fallback_prompt, model::FallbackPrompt::Auto);
        assert!(!args.planner && !args.no_plan && !args.no_direct_recipes);
        assert!(args.direct_recipes_enabled().unwrap());
        assert!(parse(&["model", "--direct-recipes", "--upstream", "none"]).is_ok());
        assert!(parse(&["model", "--direct-recipes", "--no-plan"]).is_ok());
        assert!(parse(&["model", "--direct-recipes", "--planner"]).is_ok());
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
    fn no_plan_alias_preserves_recipe_options() {
        for alias in ["--no-plan", "--no-planner"] {
            for enabled in [false, true] {
                let mut flags = vec!["model", "--direct-recipes", alias];
                if enabled {
                    flags.push("--planner");
                }
                let Some(Action::Model(args)) = parse(&flags).unwrap().action else {
                    panic!("expected model")
                };
                assert_eq!(args.planner, enabled);
                assert!(args.direct_recipes && args.no_plan);
                assert!(!args.no_how && !args.no_hints);
                assert_eq!(args.fallback_prompt, model::FallbackPrompt::Auto);
            }
        }
    }

    #[test]
    fn direct_recipe_defaults_opt_out_alias_and_dialect() {
        for (flags, expected) in [
            (vec![], true),
            (vec!["--no-direct-recipes"], false),
            (vec!["--direct-recipes"], true),
            (vec!["--planner"], true),
            (vec!["--planner", "--no-direct-recipes"], false),
            (vec!["--dialect", "solidcraft98"], false),
            (vec!["--dialect", "solidcraft98", "--direct-recipes"], false),
            (
                vec!["--dialect", "solidcraft98", "--no-direct-recipes"],
                false,
            ),
            (vec!["--upstream", "none"], true),
            (vec!["--upstream", "none", "--direct-recipes"], true),
        ] {
            let mut command = vec!["model"];
            command.extend(flags);
            let Some(Action::Model(args)) = parse(&command).unwrap().action else {
                panic!("expected model")
            };
            assert_eq!(
                args.direct_recipes_enabled().unwrap(),
                expected,
                "{command:?}"
            );
        }
        for flags in [
            vec!["model", "--upstream", "none", "--no-direct-recipes"],
            vec!["model", "--upstream", "none", "--dialect", "solidcraft98"],
        ] {
            let Some(Action::Model(args)) = parse(&flags).unwrap().action else {
                panic!("expected model")
            };
            assert!(args
                .direct_recipes_enabled()
                .unwrap_err()
                .contains("--upstream none"));
        }
        for flags in [
            ["model", "--direct-recipes", "--no-direct-recipes"],
            ["model", "--no-direct-recipes", "--direct-recipes"],
        ] {
            let error = parse(&flags).unwrap_err().to_string();
            assert!(error.contains("--direct-recipes") && error.contains("--no-direct-recipes"));
            assert!(error.contains("cannot be used with"));
        }
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
