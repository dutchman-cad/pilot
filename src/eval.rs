//! Bounded, model-only corpus evaluation. No proposed command is executed.
use crate::{catalog::Catalog, repair};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod cache;
#[cfg(test)]
mod tests;

const MAX_FILE: usize = 128 * 1_048_576;
const MAX_REPLY: usize = 64 * 1024;
const MAX_CASES: usize = 512;
const JUDGE: u32 = 1;
const PROMPT: &str = "You help with SolidCraft. Use execute for requested design actions and for facts about the current design. Command lengths are millimetres unless explicitly expressed with units. Ask which when a target is ambiguous. A name alone requests a lookup. Answer general questions in words. Refuse unsupported requests honestly. Do not claim an action happened without calling a tool. Propose only the first required command. State below describes the current screen, not a request.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Create,
    Edit,
    Query,
    Workspace,
    Refuse,
}
const BUCKETS: [Bucket; 5] = [
    Bucket::Create,
    Bucket::Edit,
    Bucket::Query,
    Bucket::Workspace,
    Bucket::Refuse,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Claim {
    Command { id: String, args: Value },
    AsksWhich,
    AnswersInWords,
    Refuses,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub utterance: String,
    pub state: String,
    pub bucket: Bucket,
    pub claim: Claim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Floors {
    pub floors: BTreeMap<Bucket, f64>,
}
impl Floors {
    fn validate(&self) -> Result<(), String> {
        if self.floors.len() != BUCKETS.len()
            || BUCKETS.iter().any(|b| !self.floors.contains_key(b))
        {
            return Err("floor config must specify all five buckets".into());
        }
        if self
            .floors
            .values()
            .any(|f| !f.is_finite() || !(0.0..=1.0).contains(f))
        {
            return Err("bucket floors must be numbers between 0 and 1".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Broken,
    Noise,
    Throttled,
    NoVerdict,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPath {
    NativeCall,
    TextCall,
    Prose,
    None,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub verdict: Verdict,
    pub path: TurnPath,
    pub command_correct: Option<bool>,
    pub arguments_correct: Option<bool>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub status: u16,
    pub raw: String,
    pub error: Option<String>,
    pub latency_ms: u64,
    pub tokens: Option<Value>,
    pub hand_backs: Option<u64>,
    pub retry_after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub recording: Recording,
    pub score: Score,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case: Case,
    pub cached: bool,
    pub classification: Verdict,
    pub turns: Vec<Turn>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BucketResult {
    pub cases: usize,
    pub passes: usize,
    pub broken: usize,
    pub noise: usize,
    pub no_verdict: usize,
    pub pass_rate: Option<f64>,
    pub flakiness: Option<f64>,
    pub floor: f64,
    pub floor_met: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub judge_version: u32,
    pub model: String,
    pub base_url: String,
    pub repetitions: usize,
    pub generated_at: u64,
    pub buckets: BTreeMap<Bucket, BucketResult>,
    pub results: Vec<CaseResult>,
    pub floors_met: bool,
}

pub fn read_bounded(path: &Path, cap: usize) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if bytes.len() > cap {
        return Err(format!("{} exceeds {cap} bytes", path.display()));
    }
    String::from_utf8(bytes).map_err(|e| format!("{} is not UTF-8: {e}", path.display()))
}
pub fn load_corpus(path: &Path, catalog: &Catalog) -> Result<Vec<Case>, String> {
    let input = read_bounded(path, 2 * 1_048_576)?;
    let mut cases = Vec::new();
    let mut ids = BTreeSet::new();
    for (line, text) in input.lines().enumerate() {
        if text.len() > 8192 {
            return Err(format!("corpus line {} exceeds 8192 bytes", line + 1));
        }
        let case: Case =
            serde_json::from_str(text).map_err(|e| format!("corpus line {}: {e}", line + 1))?;
        if cases.len() >= MAX_CASES {
            return Err(format!("corpus exceeds {MAX_CASES} cases"));
        }
        if case.id.is_empty()
            || case.id.len() > 128
            || !ids.insert(case.id.clone())
            || case.utterance.trim().is_empty()
            || case.state.trim().is_empty()
        {
            return Err(format!(
                "corpus line {}: empty fields or duplicate/oversized id",
                line + 1
            ));
        }
        if let Claim::Command { id, args } = &case.claim {
            if catalog.get(id).is_none() || !args.is_object() {
                return Err(format!(
                    "{}: unknown command or args not an object",
                    case.id
                ));
            }
            repair::check_value(args)?;
            // Parameter docs include unions such as "radius | diameter" that
            // the conservative schema intentionally omits.
            let docs = catalog.get(id).map_or("", |c| c.params.as_str());
            if args.as_object().is_some_and(|a| {
                a.keys().any(|k| {
                    !docs
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .any(|word| word == k)
                })
            }) {
                return Err(format!(
                    "{}: claim has a key absent from catalogue parameter docs",
                    case.id
                ));
            }
        }
        cases.push(case);
    }
    if cases.is_empty() {
        return Err("corpus has no cases".into());
    }
    Ok(cases)
}

fn tools(catalog: &Catalog) -> Value {
    let docs = catalog
        .commands()
        .iter()
        .map(|c| format!("{}: {}. {}", c.id, c.label, c.params))
        .collect::<Vec<_>>()
        .join("\n");
    json!([{"type":"function","function":{"name":"execute","description":format!("Run one SolidCraft command. Parameter reference (availability depends on the current screen):\n{docs}"),"parameters":{"type":"object","properties":{"command":{"type":"string","enum":catalog.commands().iter().map(|c| &c.id).collect::<Vec<_>>()},"params":{"type":"object"}},"required":["command","params"],"additionalProperties":false}}}])
}
fn request(model: &str, case: &Case, tools: &Value) -> Value {
    json!({"model":model,"stream":false,"temperature":0.1,"seed":42,"max_tokens":512,"messages":[{"role":"system","content":format!("{PROMPT}\nCurrent screen: {}",case.state)},{"role":"user","content":case.utterance}],"tools":tools,"tool_choice":"auto"})
}

fn subset(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => a
            .iter()
            .all(|(k, v)| b.get(k).is_some_and(|w| subset(v, w))),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(v, w)| subset(v, w))
        }
        (Value::Number(a), Value::Number(b)) => a == b || a.as_f64() == b.as_f64(),
        _ => expected == actual,
    }
}
fn decode_args(value: &Value) -> Result<Value, String> {
    let args = if let Some(s) = value.as_str() {
        serde_json::from_str(s).map_err(|e| format!("invalid tool arguments: {e}"))?
    } else {
        value.clone()
    };
    if !args.is_object() {
        return Err("tool arguments must be an object".into());
    }
    Ok(args)
}
fn call(value: &Value) -> Result<(String, Value), String> {
    let function = value.get("function").unwrap_or(value);
    let (name, args) = if let Some(name) = function
        .get("name")
        .or_else(|| function.get("tool"))
        .and_then(Value::as_str)
    {
        (
            name.to_string(),
            decode_args(
                function
                    .get("arguments")
                    .or_else(|| function.get("args"))
                    .unwrap_or(&json!({})),
            )?,
        )
    } else if let Some(o) = function.as_object().filter(|o| o.len() == 1) {
        let (name, args) = o.iter().next().ok_or("empty text call")?;
        (name.clone(), decode_args(args)?)
    } else {
        return Err("explicit object does not identify a tool".into());
    };
    if name == "execute" {
        let id = args
            .get("command")
            .and_then(Value::as_str)
            .ok_or("execute missing command")?
            .to_string();
        let params = args.get("params").cloned().unwrap_or(json!({}));
        if !params.is_object() {
            return Err("execute params must be an object".into());
        }
        Ok((id, params))
    } else {
        Ok((name, args))
    }
}
fn score(verdict: Verdict, path: TurnPath, reason: &str) -> Score {
    Score {
        verdict,
        path,
        command_correct: None,
        arguments_correct: None,
        reason: reason.into(),
    }
}
/// The judge is deliberately conservative and versioned; prose checks are lexical,
/// not a semantic evaluator. It never repairs native calls or executes anything.
pub fn judge(record: &Recording, claim: &Claim) -> Score {
    if record.status == 429 || (record.status == 503 && record.retry_after.is_some()) {
        return score(Verdict::Throttled, TurnPath::None, "throttled; no verdict");
    }
    if record.error.is_some() || !(200..300).contains(&record.status) {
        return score(
            Verdict::NoVerdict,
            TurnPath::None,
            "endpoint/transport failure; no verdict",
        );
    }
    let Ok(value) = serde_json::from_str::<Value>(&record.raw) else {
        return score(Verdict::Broken, TurnPath::None, "malformed response JSON");
    };
    let Some(message) = value
        .pointer("/choices/0/message")
        .filter(|v| v.is_object())
    else {
        return score(
            Verdict::Broken,
            TurnPath::None,
            "missing first assistant message",
        );
    };
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let (path, proposed) = if let Some(calls) = message.get("tool_calls").filter(|v| !v.is_null()) {
        let Some(calls) = calls.as_array() else {
            return score(
                Verdict::Broken,
                TurnPath::NativeCall,
                "tool_calls is not an array",
            );
        };
        if calls.is_empty() {
            (TurnPath::Prose, None)
        } else if let [first] = calls.as_slice() {
            (TurnPath::NativeCall, Some(call(first)))
        } else {
            return score(
                Verdict::Broken,
                TurnPath::NativeCall,
                "expected one first-step call",
            );
        }
    } else {
        (TurnPath::Prose, None)
    };
    let (path, proposed) = if proposed.is_none() && text.contains('{') {
        match repair::extract_balanced_json(text) {
            Ok(Some(v)) => (TurnPath::TextCall, Some(call(&v))),
            _ => {
                return score(
                    Verdict::Broken,
                    TurnPath::TextCall,
                    "malformed explicit text object",
                )
            }
        }
    } else {
        (path, proposed)
    };
    if let Some(proposed) = proposed {
        return match (proposed, claim) {
            (
                Ok((id, args)),
                Claim::Command {
                    id: wanted,
                    args: expected,
                },
            ) => {
                let command_correct = id == *wanted;
                let arguments_correct = subset(expected, &args);
                Score {
                    verdict: if command_correct && arguments_correct {
                        Verdict::Pass
                    } else {
                        Verdict::Broken
                    },
                    path,
                    command_correct: Some(command_correct),
                    arguments_correct: Some(arguments_correct),
                    reason: if !command_correct {
                        "wrong command"
                    } else if !arguments_correct {
                        "key arguments differ"
                    } else {
                        "command and key arguments match"
                    }
                    .into(),
                }
            }
            (Err(e), _) => score(Verdict::Broken, path, &e),
            _ => score(Verdict::Broken, path, "unexpected tool call"),
        };
    }
    let lower = text.to_lowercase();
    let refusal = [
        "cannot",
        "can't",
        "unable",
        "not supported",
        "unsupported",
        "won't",
        "will not",
        "no command",
        "do not support",
        "don't support",
    ]
    .iter()
    .any(|s| lower.contains(s));
    let asks = text.contains('?')
        && ["which", "what name", "specify", "clarify", "do you mean"]
            .iter()
            .any(|s| lower.contains(s));
    let pass = match claim {
        Claim::Command { .. } => false,
        Claim::AnswersInWords => !text.is_empty() && !refusal && !asks,
        Claim::AsksWhich => asks,
        Claim::Refuses => {
            refusal
                || message
                    .get("refusal")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty())
        }
    };
    score(
        if pass { Verdict::Pass } else { Verdict::Broken },
        path,
        if pass {
            "prose matches lexical claim rule"
        } else {
            "prose does not satisfy claim"
        },
    )
}
fn classify(turns: &[Turn]) -> Verdict {
    // An incomplete repetition set cannot be a cached pass or a floor verdict.
    if turns
        .iter()
        .any(|t| matches!(t.score.verdict, Verdict::Throttled | Verdict::NoVerdict))
    {
        return Verdict::NoVerdict;
    }
    let passes = turns
        .iter()
        .filter(|t| t.score.verdict == Verdict::Pass)
        .count();
    if passes == turns.len() && !turns.is_empty() {
        Verdict::Pass
    } else if passes == 0 {
        Verdict::Broken
    } else {
        Verdict::Noise
    }
}

fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| format!("system clock before epoch: {e}"))
}
fn fetch(agent: &ureq::Agent, endpoint: &str, body: &str, key: Option<&str>) -> Recording {
    let start = Instant::now();
    let mut req = agent.post(endpoint).set("Content-Type", "application/json");
    if let Some(key) = key {
        req = req.set("Authorization", &format!("Bearer {key}"));
    }
    let response = req.send_string(body);
    record_response(response, start)
}
fn record_response(response: Result<ureq::Response, ureq::Error>, start: Instant) -> Recording {
    let (status, raw, error, hand_backs, retry_after) = match response {
        Ok(r) | Err(ureq::Error::Status(_, r)) => {
            let status = r.status();
            let retry_after = r
                .header("retry-after")
                .map(|s| s.chars().take(256).collect());
            let hand_backs = r
                .header(crate::identity::HAND_BACKS_HEADER)
                .and_then(|s| s.parse().ok());
            let mut bytes = Vec::new();
            let result = r
                .into_reader()
                .take(MAX_REPLY as u64 + 1)
                .read_to_end(&mut bytes);
            let error = if bytes.len() > MAX_REPLY {
                bytes.truncate(MAX_REPLY);
                Some(format!("response exceeds {MAX_REPLY} bytes"))
            } else {
                result.err().map(|e| format!("cannot read response: {e}"))
            };
            (
                status,
                String::from_utf8_lossy(&bytes).into_owned(),
                error,
                hand_backs,
                retry_after,
            )
        }
        Err(e) => (
            0,
            String::new(),
            Some(format!("endpoint transport failure: {e}")),
            None,
            None,
        ),
    };
    let tokens = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v.get("usage").cloned());
    Recording {
        status,
        raw,
        error,
        latency_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
        tokens,
        hand_backs,
        retry_after,
    }
}

/// Help shared by the executable and the generated reference.
pub fn help() -> String {
    format!("{1} — corpus evaluation\n\n{0} --base-url URL/v1 --model MODEL [--corpus corpus/starter.jsonl] [--floors corpus/floors.json] [--repetitions N (alias -n; default 3, range 1–10)] [--timeout SECONDS (default 120, range 1–600)] [--output eval-results] [--cache FILE] [--no-cache | --refresh]\n{0} --rescore results.json [--floors FILE] [--output DIR]\n{0} --help (-h)\nOPENAI_API_KEY supplies optional authorization; it is never recorded.\n", crate::identity::EVAL_BINARY, crate::identity::DISPLAY_NAME)
}

pub struct Options {
    pub base_url: String,
    pub model: String,
    pub corpus: PathBuf,
    pub floors: PathBuf,
    pub output: PathBuf,
    pub cache: PathBuf,
    pub repetitions: usize,
    pub timeout_secs: u64,
    pub no_cache: bool,
    pub refresh: bool,
    pub rescore: Option<PathBuf>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            corpus: "corpus/starter.jsonl".into(),
            floors: "corpus/floors.json".into(),
            output: "eval-results".into(),
            cache: format!(".{}-cache.json", crate::identity::EVAL_BINARY).into(),
            repetitions: 3,
            timeout_secs: 120,
            no_cache: false,
            refresh: false,
            rescore: None,
        }
    }
}
impl Options {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args;
        while let Some(arg) = args.next() {
            if arg == "--no-cache" {
                options.no_cache = true;
                continue;
            }
            if arg == "--refresh" {
                options.refresh = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{arg} requires a value"))?;
            match arg.as_str() {
                "--base-url" => options.base_url = value,
                "--model" => options.model = value,
                "--corpus" => options.corpus = value.into(),
                "--floors" => options.floors = value.into(),
                "--output" => options.output = value.into(),
                "--cache" => options.cache = value.into(),
                "--rescore" => options.rescore = Some(value.into()),
                "--repetitions" | "-n" => {
                    options.repetitions = value
                        .parse()
                        .map_err(|_| "repetitions must be an integer")?
                }
                "--timeout" => {
                    options.timeout_secs =
                        value.parse().map_err(|_| "timeout must be an integer")?
                }
                _ => return Err(format!("unknown option: {arg}")),
            }
        }
        options.validate()?;
        Ok(options)
    }
    fn validate(&self) -> Result<(), String> {
        if !(1..=10).contains(&self.repetitions) || !(1..=600).contains(&self.timeout_secs) {
            return Err("repetitions must be 1–10; timeout must be 1–600 seconds".into());
        }
        if self.rescore.is_none() {
            let url =
                url::Url::parse(&self.base_url).map_err(|e| format!("invalid base URL: {e}"))?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(
                    "base URL must be HTTP(S) without credentials, query or fragment".into(),
                );
            }
            if self.model.is_empty() || self.model.len() > 256 {
                return Err("--model is required (at most 256 bytes)".into());
            }
        }
        Ok(())
    }
}
fn summarize(report: &mut Report, floors: &Floors) {
    report.buckets.clear();
    for bucket in BUCKETS {
        let mut b = BucketResult {
            // validate() requires every bucket; a missing one fails closed.
            floor: floors.floors.get(&bucket).copied().unwrap_or(f64::INFINITY),
            ..BucketResult::default()
        };
        for result in report.results.iter().filter(|r| r.case.bucket == bucket) {
            b.cases += 1;
            match result.classification {
                Verdict::Pass => b.passes += 1,
                Verdict::Broken => b.broken += 1,
                Verdict::Noise => b.noise += 1,
                _ => b.no_verdict += 1,
            }
        }
        let judged = b.passes + b.broken + b.noise;
        if judged > 0 {
            b.pass_rate = Some(b.passes as f64 / judged as f64);
            b.flakiness = Some(b.noise as f64 / judged as f64);
        }
        b.floor_met = b.pass_rate.is_some_and(|rate| rate >= b.floor) && b.no_verdict == 0;
        report.buckets.insert(bucket, b);
    }
    report.floors_met = report.buckets.values().all(|b| b.floor_met);
}
pub fn markdown(report: &Report) -> String {
    let mut text = format!("Model: {} (judge {})\n\n| Bucket | Pass / judged | Pass rate | Flakiness | No verdict | Floor | Result |\n| --- | ---: | ---: | ---: | ---: | ---: | --- |\n", report.model.replace(['|','\n','\r'], " "), report.judge_version);
    for (bucket, b) in &report.buckets {
        let percent = |v: Option<f64>| v.map_or("n/a".into(), |n| format!("{:.1}%", 100.0 * n));
        text.push_str(&format!(
            "| {bucket:?} | {} / {} | {} | {} | {} | {:.1}% | {} |\n",
            b.passes,
            b.passes + b.broken + b.noise,
            percent(b.pass_rate),
            percent(b.flakiness),
            b.no_verdict,
            100.0 * b.floor,
            if b.floor_met { "pass" } else { "fail" }
        ));
    }
    text.push_str("\n| Case | Attempt | Cached | Verdict | Path | Command | Args | Tokens | Latency ms | Hand-backs |\n| --- | ---: | --- | --- | --- | --- | --- | --- | ---: | --- |\n");
    for result in &report.results {
        for (index, turn) in result.turns.iter().enumerate() {
            text.push_str(&format!(
                "| {} | {} | {} | {:?} | {:?} | {:?} | {:?} | {} | {} | {} |\n",
                result.case.id.replace(['|', '\n', '\r'], " "),
                index + 1,
                result.cached,
                turn.score.verdict,
                turn.score.path,
                turn.score.command_correct,
                turn.score.arguments_correct,
                turn.recording
                    .tokens
                    .as_ref()
                    .map_or("unknown".into(), Value::to_string),
                turn.recording.latency_ms,
                turn.recording
                    .hand_backs
                    .map_or("unknown".into(), |n| n.to_string())
            ));
        }
    }
    text
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| format!("cannot serialize {}: {e}", path.display()))?;
    if bytes.len() > MAX_FILE {
        return Err(format!("{} exceeds report/cache limit", path.display()));
    }
    fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
/// Execute an evaluation or re-score recordings offline. Returns whether floors pass.
pub fn run(options: Options) -> Result<bool, String> {
    run_with(options, fetch)
}

// An endpoint seam lets model-side tests run even on hosts that disallow sockets.
fn run_with(
    options: Options,
    mut endpoint_call: impl FnMut(&ureq::Agent, &str, &str, Option<&str>) -> Recording,
) -> Result<bool, String> {
    options.validate()?;
    let floors: Floors = serde_json::from_str(&read_bounded(&options.floors, 8192)?)
        .map_err(|e| format!("invalid floor config: {e}"))?;
    floors.validate()?;
    let timestamp = now()?;
    let mut report = if let Some(path) = &options.rescore {
        let mut report: Report = serde_json::from_str(&read_bounded(path, MAX_FILE)?)
            .map_err(|e| format!("invalid recording report: {e}"))?;
        if report.results.len() > MAX_CASES
            || !(1..=10).contains(&report.repetitions)
            || report.results.iter().any(|r| {
                r.turns.len() != report.repetitions
                    || r.turns.iter().any(|t| t.recording.raw.len() > MAX_REPLY)
            })
        {
            return Err("recording report exceeds case/turn limits or has incomplete turns".into());
        }
        for result in &mut report.results {
            for turn in &mut result.turns {
                turn.score = judge(&turn.recording, &result.case.claim);
            }
            result.classification = classify(&result.turns);
        }
        report.judge_version = JUDGE;
        report.generated_at = timestamp;
        report
    } else {
        let catalog = Catalog::bundled()?;
        let cases = load_corpus(&options.corpus, &catalog)?;
        let tool_defs = tools(&catalog);
        let endpoint = format!(
            "{}/chat/completions",
            options.base_url.trim_end_matches('/')
        );
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .try_proxy_from_env(false)
            .timeout(Duration::from_secs(options.timeout_secs))
            .build();
        let key = std::env::var("OPENAI_API_KEY").ok();
        let mut cache = if options.no_cache {
            cache::Cache::default()
        } else {
            cache::Cache::load(&options.cache, timestamp)?
        };
        let mut results = Vec::new();
        for case in cases {
            let body = serde_json::to_string(&request(&options.model, &case, &tool_defs))
                .map_err(|e| format!("cannot encode request: {e}"))?;
            if body.len() > 1_048_576 {
                return Err("request exceeds 1 MiB".into());
            }
            // Endpoint, judge, claim, and sample count supplement the required request fingerprint.
            let identity = serde_json::to_string(&json!([
                body,
                options.base_url,
                case.claim,
                options.repetitions,
                JUDGE
            ]))
            .map_err(|e| e.to_string())?;
            let cached = if options.no_cache || options.refresh {
                None
            } else {
                cache.get(&identity, timestamp)
            };
            if let Some(mut result) = cached {
                result.case = case;
                result.cached = true;
                results.push(result);
                continue;
            }
            let mut turns = Vec::new();
            for _ in 0..options.repetitions {
                let recording = endpoint_call(&agent, &endpoint, &body, key.as_deref());
                let score = judge(&recording, &case.claim);
                turns.push(Turn { recording, score });
            }
            let result = CaseResult {
                classification: classify(&turns),
                case,
                turns,
                cached: false,
            };
            if !options.no_cache {
                cache.observe(&identity, &result, timestamp);
                cache.save(&options.cache)?;
            }
            eprintln!("{}: {:?}", result.case.id, result.classification);
            results.push(result);
        }
        Report {
            judge_version: JUDGE,
            model: options.model,
            base_url: options.base_url,
            repetitions: options.repetitions,
            generated_at: timestamp,
            buckets: BTreeMap::new(),
            results,
            floors_met: false,
        }
    };
    summarize(&mut report, &floors);
    fs::create_dir_all(&options.output)
        .map_err(|e| format!("cannot create output directory: {e}"))?;
    write_json(&options.output.join("results.json"), &report)?;
    let table = markdown(&report);
    fs::write(options.output.join("results.md"), &table)
        .map_err(|e| format!("cannot write markdown report: {e}"))?;
    // Keep stdout compact: the saved table includes every turn.
    println!("{}", table.split("\n| Case |").next().unwrap_or(&table));
    Ok(report.floors_met)
}
