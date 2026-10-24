//! User-reviewed local reports. No HTTP client, credentials, or issue submission.
use crate::{capture::Turn, catalog::Catalog, redaction::Redactor};
use clap::Args;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    time::{Duration, Instant},
};

const MAX_FETCH: usize = 8 * 1_048_576;
pub const MAX_URL: usize = 7500;
#[derive(Debug, Args)]
pub struct ReportArgs {
    #[arg(long, default_value = "127.0.0.1:11500")]
    pub listen: SocketAddr,
    /// Stable turn id from /pilot/turns; default is last failed turn, else last.
    #[arg(long,value_parser=clap::value_parser!(u64).range(1..))]
    pub turn: Option<u64>,
    #[arg(long)]
    pub include_request: bool,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long, conflicts_with = "turn")]
    pub from_request: Option<PathBuf>,
    /// Open the pre-filled issue page after printing the report for review.
    #[arg(long)]
    pub open: bool,
}
fn bounded_read(reader: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FETCH as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read report input: {e}"))?;
    if bytes.len() > MAX_FETCH {
        return Err("report input exceeds 8 MiB".into());
    }
    Ok(bytes)
}
pub fn offline(body: &Value) -> Result<Turn, String> {
    Turn::from_request(
        body,
        &Catalog::bundled()?,
        json!({"offline_planner_replay":true}),
    )
}
/// Only a literal loopback socket is accepted; no DNS, proxies, redirects or client agent.
fn fetch(listen: SocketAddr) -> Result<Vec<Turn>, String> {
    if !listen.ip().is_loopback() {
        return Err("report --listen must be a loopback address".into());
    }
    let timeout = Duration::from_secs(5);
    let mut socket = TcpStream::connect_timeout(&listen, timeout)
        .map_err(|e| format!("cannot fetch local turns: {e}"))?;
    socket
        .set_read_timeout(Some(timeout))
        .and_then(|()| socket.set_write_timeout(Some(timeout)))
        .map_err(|e| format!("cannot set report timeout: {e}"))?;
    socket
        .write_all(
            format!("GET /pilot/turns HTTP/1.1\r\nHost: {listen}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .map_err(|e| format!("cannot request local turns: {e}"))?;
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("turn fetch timed out")?;
        socket
            .set_read_timeout(Some(remaining.max(Duration::from_millis(1))))
            .map_err(|e| format!("cannot set turn read timeout: {e}"))?;
        let mut chunk = [0u8; 8192];
        let count = socket
            .read(&mut chunk)
            .map_err(|e| format!("cannot read local turns: {e}"))?;
        if count == 0 {
            break;
        }
        if bytes.len().saturating_add(count) > MAX_FETCH {
            return Err("turn response exceeds 8 MiB".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "turn endpoint returned invalid UTF-8")?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .ok_or("turn endpoint returned invalid HTTP")?;
    if !headers.starts_with("HTTP/1.1 200 ") {
        return Err("turn endpoint unavailable; check --listen and --no-report-buffer".into());
    }
    let turns: Vec<Turn> =
        serde_json::from_str(body).map_err(|_| "turn endpoint returned invalid turns")?;
    if turns.len() > crate::capture::MAX_TURNS {
        return Err("turn endpoint exceeded five turns".into());
    }
    Ok(turns)
}
pub fn select(turns: &[Turn], id: Option<u64>) -> Result<&Turn, String> {
    id.map_or_else(
        || {
            turns
                .iter()
                .rev()
                .find(|t| t.failed)
                .or_else(|| turns.last())
        },
        |id| turns.iter().find(|t| t.id == id),
    )
    .ok_or_else(|| {
        "requested turn is unavailable (buffer is disabled, empty, or turn expired)".into()
    })
}
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
        .replace(['\r', '\n'], " ")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn diagnostic_text(value: &Value, depth: usize) -> Option<&str> {
    if depth > 24 {
        return None;
    }
    match value {
        Value::Object(m) => m
            .get("reason")
            .and_then(Value::as_str)
            .or_else(|| m.get("error").and_then(Value::as_str))
            .or_else(|| m.values().find_map(|v| diagnostic_text(v, depth + 1))),
        Value::Array(a) => a
            .iter()
            .take(256)
            .find_map(|v| diagnostic_text(v, depth + 1)),
        _ => None,
    }
}
pub fn render(turn: &Turn, include_request: bool) -> Result<(String, String), String> {
    render_note(turn, include_request, None)
}
fn render_note(
    turn: &Turn,
    include_request: bool,
    note: Option<&str>,
) -> Result<(String, String), String> {
    let catalog = Catalog::bundled()?;
    let mut r = Redactor::new(&turn.names, &catalog);
    // Discover labelled names before rendering so the same name is masked even
    // when its first occurrence is unlabelled (for example in an error title).
    let _ = r.prose(&turn.request);
    if let Some(note) = note {
        let _ = r.prose(note);
    }
    for event in turn.events.iter().take(crate::capture::MAX_EVENTS) {
        let _ = r.value(&event.facts);
    }
    r.counts.clear();
    if !include_request {
        r.omit_request_words(&turn.request);
    }
    let kind = if turn
        .events
        .iter()
        .any(|e| e.kind.contains("error") || e.facts.to_string().contains("\"error\""))
    {
        "Tool or provider error"
    } else if turn.failed {
        "Recipe declined"
    } else {
        "Pilot turn"
    };
    let reason = turn
        .events
        .iter()
        .rev()
        .find_map(|e| diagnostic_text(&e.facts, 0))
        .unwrap_or(if turn.failed {
            "no unambiguous supported plan"
        } else {
            "captured diagnostics"
        });
    let title_reason = r.prose(reason);
    let short: String = title_reason
        .lines()
        .next()
        .unwrap_or(&title_reason)
        .chars()
        .take(100)
        .collect();
    let title = format!("{kind}: {short}");
    let model = r.text(&turn.model);
    let flags = r.value(&turn.flags).to_string();
    let version = r.text(&turn.version);
    // Derive vocabulary only after redaction: a domain or filename may contain
    // otherwise public planner words. Replace old captured diagnostics as well.
    let request = r.prose(&turn.request);
    let mut shape = r.value(&turn.shape);
    if shape["intent"] == "unmatched" {
        shape["decline_tokens"] = json!(crate::planner::report_decline(&request).1);
        shape["token_shape"] = json!(crate::planner::report_tokens(&request));
    }
    let shape = shape.to_string();
    let transitions = turn
        .events
        .iter()
        .map(|e| e.kind.as_str())
        .take(12)
        .collect::<Vec<_>>()
        .join(", ");
    let transitions = r.text(&transitions);
    let mut content=format!("Pilot version: {version}\nModel: {model}\nFlags: `{flags}`\nTurn: {}\n\nWhat happened: Pilot recorded {} events for this turn ({transitions}). {}. The planner intent was `{}`. Tool calls below were offered to the client; capture does not establish that changes were executed.\n\nRequest shape: `{shape}`\nCaptured request bytes: {} of {}.\n",turn.id,turn.events.len(),r.prose(reason),r.text(turn.shape["intent"].as_str().unwrap_or("unknown")),turn.request.len(),turn.request_bytes);
    if include_request {
        content.push_str(&format!(
            "\n## Request text (explicitly included; redacted)\n\n{}\n",
            request
                .lines()
                .map(|l| format!("> {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    } else {
        content.push_str("\nRequest words omitted. Decline tokens identify fixed-vocabulary guards or competing intents; unknown token-shape words appear as [word].\n");
    }
    if turn.truncated_events > 0 {
        content.push_str(&format!(
            "\nCapture omitted {} events at its limit.\n",
            turn.truncated_events
        ));
    }
    content.push_str("\n| Event | Facts |\n| --- | --- |\n");
    for event in turn.events.iter().take(crate::capture::MAX_EVENTS) {
        let kind = r.text(&event.kind);
        let facts = r.value(&event.facts).to_string();
        content.push_str(&format!("| {} | {} |\n", cell(&kind), cell(&facts)));
    }
    if let Some(note) = note.filter(|s| !s.is_empty()) {
        let note = r.prose(note);
        content.push_str("\n## Tester note (redacted)\n\n");
        for line in note.lines() {
            content.push_str(&format!("> {line}\n"));
        }
    }
    let summary = r.summary();
    let report=format!("# {title}\n\n{summary}\n\n{content}\nRedaction summary: {summary}\n\n---\nBy submitting this report, I confirm that I reviewed it before sending. Pilot does not send reports.\n");
    if report.len() > MAX_FETCH {
        return Err("rendered report exceeds 8 MiB".into());
    }
    Ok((title, report))
}
/// Deliberately narrow commands: ordinary CAD requests containing “report” pass through.
pub(crate) fn chat_command(text: &str) -> Option<&str> {
    let text = text.trim();
    if ["/report", "report this", "report that"]
        .iter()
        .any(|command| text.eq_ignore_ascii_case(command))
    {
        return Some("");
    }
    let (command, note) = text.split_once(char::is_whitespace)?;
    let note = note.trim();
    (command.eq_ignore_ascii_case("/report") && note.chars().count() <= 200).then_some(note)
}

fn save_default(report: &str, data_dir: Option<&std::path::Path>) -> Result<PathBuf, String> {
    let dir = crate::model::data_dir(data_dir)?.join("reports");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create reports directory: {e}"))?;
    for suffix in 0..100 {
        let path = dir.join(format!(
            "report-{}-{}-{suffix}.md",
            crate::audit::now(),
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(report.as_bytes())
                    .map_err(|e| format!("cannot write report: {e}"))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot create report: {e}")),
        }
    }
    Err("cannot create report after 100 filename collisions".into())
}

pub(crate) fn chat_reply(
    turns: &std::collections::VecDeque<Turn>,
    disabled: bool,
    note: &str,
    data_dir: Option<&std::path::Path>,
) -> Result<String, String> {
    let eligible: Vec<_> = turns
        .iter()
        .filter(|t| chat_command(&t.request).is_none())
        .cloned()
        .collect();
    if disabled || eligible.is_empty() {
        return Ok(format!("Pilot report: The capture ring is {}; try `dutchman-pilot report --from-request recorded-request.json` in the CLI.", if disabled { "disabled (--no-report-buffer)" } else { "empty" }));
    }
    let turn = select(&eligible, None)?;
    let (title, report) = render_note(turn, false, Some(note))?;
    let path = save_default(&report, data_dir)?;
    let (url, paste) = issue_url(&title, &report);
    let mut lines = Vec::new();
    for prefix in [
        "Pilot version:",
        "Model:",
        "Request shape:",
        "What happened:",
    ] {
        if let Some(line) = report.lines().find(|line| line.starts_with(prefix)) {
            let line = line.replace("What happened:", "Outcome:").replace('`', "'");
            lines.push(
                line.chars()
                    .filter(|c| !c.is_control())
                    .take(1200)
                    .collect::<String>(),
            );
        }
    }
    lines.push(
        if paste {
            "Issue link: title only; review and paste the report file."
        } else {
            "Issue link: prefilled above; review before submitting."
        }
        .into(),
    );
    Ok(format!("Pilot report: Saved a redacted report to {}.\n\n{url}\n\nFor Dutchman CAD Discord #pilot-help:\n```text\n{}\n```\n\nNothing was sent. Open the link to review and submit.", path.display(), lines.join("\n")))
}
pub fn issue_url(title: &str, body: &str) -> (String, bool) {
    let mut url = url::Url::parse("https://github.com/dutchman-cad/pilot/issues/new").ok();
    if let Some(url) = &mut url {
        url.query_pairs_mut()
            .append_pair("title", &title.chars().take(200).collect::<String>())
            .append_pair("body", body);
        if url.as_str().len() < MAX_URL {
            return (url.to_string(), false);
        }
        url.set_query(None);
        url.query_pairs_mut()
            .append_pair("title", &title.chars().take(200).collect::<String>());
        return (url.to_string(), true);
    }
    (
        "https://github.com/dutchman-cad/pilot/issues/new".into(),
        true,
    )
}
pub fn run(args: ReportArgs) -> Result<(), String> {
    // Offline branch deliberately precedes the only socket construction.
    let turns = if let Some(path) = &args.from_request {
        let bytes = bounded_read(
            fs::File::open(path).map_err(|e| format!("cannot open recorded request: {e}"))?,
        )?;
        let body = crate::repair::parse_json(
            std::str::from_utf8(&bytes).map_err(|_| "recorded request must be UTF-8")?,
        )?;
        vec![offline(&body)?]
    } else {
        fetch(args.listen)?
    };
    let turn = select(&turns, args.turn)?;
    let (title, report) = render(turn, args.include_request)?;
    let path = match args.out {
        Some(path) => {
            fs::write(&path, &report).map_err(|e| format!("cannot write report: {e}"))?;
            path
        }
        None => save_default(&report, None)?,
    };
    let (url, paste) = issue_url(&title, &report);
    let mut stdout = std::io::stdout().lock();
    write!(
        stdout,
        "{report}\nReport file: {}\n{}\n{url}\n",
        path.display(),
        if paste {
            "Issue URL too long; paste the file into the issue after reviewing it."
        } else {
            "Review the report before submitting:"
        }
    )
    .map_err(|e| format!("cannot print report: {e}"))?;
    stdout
        .flush()
        .map_err(|e| format!("cannot flush report: {e}"))?;
    if args.open {
        open_browser(&url)?;
    }
    Ok(())
}
fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/D", "/S", "/C"])
        .raw_arg(format!("start \"\" \"{url}\""))
        .status();
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err("cannot open browser; use the printed issue URL".into()),
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn chat_command_exact_forms_and_note_limit() {
        for text in [
            "/report",
            "  /RePoRt \n",
            "report this",
            " REPORT THAT ",
            "/report a short note",
            "/report\tmy note",
        ] {
            assert!(chat_command(text).is_some(), "{text}");
        }
        assert_eq!(
            chat_command(&format!("/report {}", "α".repeat(200)))
                .unwrap()
                .chars()
                .count(),
            200
        );
        for text in [
            "/reporting",
            "please report the volume",
            "report the body count",
            "report this please",
            "report that!",
            "/report-note",
            &format!("/report {}", "α".repeat(201)),
        ] {
            assert!(chat_command(text).is_none(), "{text}");
        }
    }
    #[test]
    fn url_cap() {
        let (url, paste) = issue_url("Recipe declined: α", &"private & text\n".repeat(10000));
        assert!(paste);
        assert!(url.len() < MAX_URL);
        assert!(!url.contains("body="));
        let (url, paste) = issue_url("a & b", "hello\nthere");
        assert!(!paste);
        assert!(url.contains("body=hello%0Athere"));
    }
    #[test]
    fn no_http_client_or_offline_network() {
        let source = include_str!("report.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("ureq"));
        assert!(!production.contains("AgentBuilder"));
        assert!(fetch("192.0.2.1:80".parse().unwrap()).is_err());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod privacy_tests {
    use super::*;
    #[test]
    fn request_shape_cannot_recover_redacted_values_or_their_words() {
        // Each sensitive value contains public recipe vocabulary. Neither the
        // value nor its meaningful constituent words may survive anywhere.
        for (value, fragments, placeholder) in [
            (
                "jane.doe@example.com",
                vec!["jane", "jane.doe", "example", "example.com"],
                "[email]",
            ),
            (
                "/hiddenvault/fillet/cylinder.solidcraft",
                vec!["hiddenvault", "fillet", "cylinder", "solidcraft"],
                "[path]",
            ),
            (
                r"C:\privatevault\chamfer\sphere.solidcraft",
                vec!["privatevault", "chamfer", "sphere", "solidcraft"],
                "[path]",
            ),
            (
                "~/privatevault/mirror/revolve.solidcraft",
                vec!["privatevault", "mirror", "revolve", "solidcraft"],
                "[path]",
            ),
            (
                "https://privatevault.example/fillet",
                vec!["privatevault", "example", "fillet"],
                "[URL]",
            ),
            (
                "sk-privatevault-extrude-cylinder",
                vec!["privatevault", "extrude", "cylinder"],
                "[token]",
            ),
        ] {
            let mut turn = offline(
                &json!({"messages":[{"role":"user","content":format!("please use {value}")}]}),
            )
            .unwrap();
            // Rendering must also repair shape data captured by older versions.
            turn.shape["decline_tokens"] = json!(fragments);
            turn.shape["token_shape"] = json!(fragments);
            for consent in [false, true] {
                let (title, report) = render(&turn, consent).unwrap();
                for text in [title, report] {
                    assert!(!text.contains(value), "{text}");
                    for fragment in &fragments {
                        assert!(
                            !text.to_ascii_lowercase().contains(fragment),
                            "leaked {fragment}: {text}"
                        );
                    }
                }
            }
            let redacted = Redactor::new(&[], &Catalog::bundled().unwrap()).prose(value);
            assert_eq!(redacted, placeholder);
            assert_eq!(crate::planner::report_tokens(&redacted), vec![placeholder]);
            assert!(crate::planner::report_decline(&redacted).1.is_empty());
        }
        assert_eq!(
            crate::planner::report_tokens("cube [IP address] [email] [example cube]"),
            vec!["cube", "[IP address]", "[email]", "[word]"]
        );
    }
    #[test]
    fn consent_names_and_decline_vocabulary() {
        let body = json!({"model":"test-model","messages":[{"role":"user","content":"create a cube 20mm for SecretProject then move 7mm"}],"tools":[{"function":{"name":"execute"}}]});
        let turn = offline(&body).unwrap();
        let (title, text) = render(&turn, false).unwrap();
        assert!(title.contains("multiple intents"), "{title}");
        assert!(!text.contains("SecretProject"));
        assert!(!text.contains("secretproject"));
        assert!(text.contains("[word]"));
        let (_, text) = render(&turn, true).unwrap();
        assert!(text.contains("Request text (explicitly included; redacted)"));
        assert!(text.contains("SecretProject"));
        let body = json!({"model":"test-model","messages":[{"role":"user","content":"rename the body to 'Private Gear'"}],"tools":[{"function":{"name":"execute"}}]});
        let mut turn = offline(&body).unwrap();
        turn.names.push(("Private Gear".into(), "Body".into()));
        let (_, text) = render(&turn, true).unwrap();
        assert!(!text.contains("Private Gear"));
        assert!(text.contains("Body1"));
    }
    #[test]
    fn default_selection_prefers_failure() {
        let mut failed = offline(&json!({})).unwrap();
        failed.id = 1;
        let mut good = failed.clone();
        good.failed = false;
        good.id = 2;
        let turns = vec![failed, good];
        assert_eq!(select(&turns, None).unwrap().id, 1);
        assert_eq!(select(&turns, Some(2)).unwrap().id, 2);
        assert!(select(&turns, Some(99)).is_err());
    }
}
