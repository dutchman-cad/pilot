//! Bounded, deterministic reports over the model face's profiles and rewrite logs.
use crate::model::{self, Profile};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

const MAX_LOG_BYTES: u64 = 64 * 1_048_576;
const MAX_LINE_BYTES: u64 = 8 * 1_048_576;
const MAX_EVENTS: u64 = 1_000_000;
const MAX_GROUPS: usize = 4096;

/// Event names are shared with the rewrite log; old profiles default to zero.
pub(crate) fn record_events(profile: &mut Profile, kinds: &BTreeMap<String, u64>) {
    for (kind, count) in kinds {
        let target = match kind.as_str() {
            "argument_repair" | "argument_reconcile" | "argument_json" => {
                profile.repairs_by_kind.entry(kind.clone()).or_default()
            }
            "direct_recipe_calls" => &mut profile.direct_recipe_calls,
            "direct_recipe_reply" => &mut profile.direct_recipe_reply,
            "direct_recipe_fallback" => &mut profile.direct_recipe_fallback,
            "provider_error" => &mut profile.provider_errors,
            "ollama 500 recovered" => &mut profile.ollama_500_recoveries,
            "plan" => &mut profile.plans_used,
            "how_answer" => &mut profile.how_answers,
            "next_step_hint" => &mut profile.hints_added,
            _ => continue,
        };
        *target = target.saturating_add(*count);
    }
}

#[derive(Serialize)]
struct Report {
    #[serde(flatten)]
    profile: Profile,
    native_percent: f64,
    text_written_percent: f64,
    prose_percent: f64,
}
impl Report {
    fn new(profile: Profile) -> Self {
        // The proxy observes every choice and retry; client turns are a separate count.
        let observations = profile.native as f64
            + profile.text_written as f64
            + profile.prose as f64
            + profile.empty as f64;
        let percent = |count| {
            if observations == 0.0 {
                0.0
            } else {
                100.0 * count as f64 / observations
            }
        };
        Self {
            native_percent: percent(profile.native),
            text_written_percent: percent(profile.text_written),
            prose_percent: percent(profile.prose),
            profile,
        }
    }
}

pub fn profiles(dir: Option<&Path>, model: Option<&str>, json: bool) -> Result<String, String> {
    let reports: Vec<_> = model::load_profiles(dir, model)?
        .into_iter()
        .map(Report::new)
        .collect();
    if json {
        return serde_json::to_string_pretty(&reports)
            .map_err(|e| format!("cannot format profiles: {e}"));
    }
    if reports.is_empty() {
        return Ok("No matching model profiles.".into());
    }
    let mut text = String::from("Percentages: observed replies, including retries and empty replies.\nRepairs by kind count rewrite events (argument_json may accompany another repair).\n");
    for r in reports {
        let p = r.profile;
        text.push_str(&format!("\n{} @ {}\n", label(&p.model), label(&p.upstream)));
        let cells = [
            ("turns", p.turns.to_string()),
            ("direct-turns (no model)", p.direct_turns.to_string()),
            ("direct-fallbacks", p.direct_recipe_fallback.to_string()),
            ("native%", format!("{:.1}", r.native_percent)),
            ("text-converted%", format!("{:.1}", r.text_written_percent)),
            ("prose-only%", format!("{:.1}", r.prose_percent)),
            ("empty", p.empty.to_string()),
            ("repairs", p.repairs.to_string()),
            ("hand-backs", p.hand_backs.to_string()),
            ("provider-errors", p.provider_errors.to_string()),
            ("ollama-500-recoveries", p.ollama_500_recoveries.to_string()),
            ("plans-used", p.plans_used.to_string()),
            ("how-answers", p.how_answers.to_string()),
            ("hints-added", p.hints_added.to_string()),
        ];
        for header in [true, false] {
            let row = cells
                .iter()
                .map(|(name, value)| {
                    let cell = if header { *name } else { value.as_str() };
                    format!("{cell:>width$}", width = name.len().max(value.len()))
                })
                .collect::<Vec<_>>()
                .join("  ");
            text.push_str(&row);
            text.push('\n');
        }
        text.push_str(&format!(
            "repairs by kind: {}\n",
            counts_text(&p.repairs_by_kind)
        ));
    }
    Ok(text)
}

// Escape control characters so model and kind names cannot inject terminal output.
fn label(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}
fn counts_text(counts: &BTreeMap<String, u64>) -> String {
    if counts.is_empty() {
        return "none recorded".into();
    }
    counts
        .iter()
        .map(|(kind, count)| format!("{}={count}", label(kind)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Default, Serialize)]
struct ModelCounts {
    total: u64,
    by_kind: BTreeMap<String, u64>,
}
#[derive(Default, Serialize)]
struct LogSummary {
    total: u64,
    by_kind: BTreeMap<String, u64>,
    by_model: BTreeMap<String, ModelCounts>,
}
#[derive(Deserialize)]
struct Event {
    model: String,
    kind: String,
}
fn increment(counts: &mut BTreeMap<String, u64>, key: &str) -> Result<(), String> {
    if !counts.contains_key(key) && counts.len() >= MAX_GROUPS {
        return Err("rewrite log exceeds 4096 distinct groups".into());
    }
    *counts.entry(key.to_owned()).or_default() += 1;
    Ok(())
}

pub fn log_summary(path: &Path, json: bool) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file.take(MAX_LOG_BYTES + 1));
    let mut summary = LogSummary::default();
    let mut bytes = 0;
    let mut line = Vec::new();
    let mut line_number = 0;
    loop {
        line.clear();
        let n = reader
            .by_ref()
            .take(MAX_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        line_number += 1;
        if line_number > MAX_EVENTS {
            return Err("rewrite log exceeds 1000000 lines".into());
        }
        bytes += n as u64;
        if bytes > MAX_LOG_BYTES || n as u64 > MAX_LINE_BYTES {
            return Err(format!(
                "{}:{line_number}: log exceeds 64 MiB or line exceeds 8 MiB",
                path.display()
            ));
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if summary.total >= MAX_EVENTS {
            return Err("rewrite log exceeds 1000000 events".into());
        }
        let event: Event = serde_json::from_slice(&line)
            .map_err(|e| format!("invalid rewrite log {}:{line_number}: {e}", path.display()))?;
        if event.model.is_empty()
            || event.kind.is_empty()
            || event.model.len() > 4096
            || event.kind.len() > 4096
        {
            return Err(format!(
                "{}:{line_number}: model and kind must contain 1–4096 bytes",
                path.display()
            ));
        }
        if !summary.by_model.contains_key(&event.model) && summary.by_model.len() >= 256 {
            return Err("rewrite log exceeds 256 models".into());
        }
        increment(&mut summary.by_kind, &event.kind)?;
        let model = summary.by_model.entry(event.model).or_default();
        increment(&mut model.by_kind, &event.kind)?;
        model.total += 1;
        summary.total += 1;
    }
    if json {
        return serde_json::to_string_pretty(&summary)
            .map_err(|e| format!("cannot format log summary: {e}"));
    }
    let mut text = format!("events: {}\nkind  count\n", summary.total);
    for (kind, count) in summary.by_kind {
        text.push_str(&format!("{}  {count}\n", label(&kind)));
    }
    text.push_str("\nmodel  count  kinds\n");
    for (model, counts) in summary.by_model {
        text.push_str(&format!(
            "{}  {}  {}\n",
            label(&model),
            counts.total,
            counts_text(&counts.by_kind)
        ));
    }
    Ok(text)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn event_counters_accumulate_without_overflow_or_double_counting_hand_backs() {
        let mut p = Profile {
            hand_backs: 4,
            provider_errors: u64::MAX,
            ..Profile::default()
        };
        let events = [
            ("argument_repair", 2),
            ("argument_json", 1),
            ("argument_reconcile", 3),
            ("provider_error", 1),
            ("ollama 500 recovered", 2),
            ("plan", 3),
            ("how_answer", 4),
            ("next_step_hint", 5),
            ("hand_back", 6),
        ]
        .into_iter()
        .map(|(kind, n)| (kind.into(), n))
        .collect();
        record_events(&mut p, &events);
        assert_eq!(p.provider_errors, u64::MAX);
        assert_eq!(p.hand_backs, 4);
        assert_eq!(p.repairs_by_kind["argument_repair"], 2);
        assert_eq!(p.repairs_by_kind["argument_json"], 1);
        assert_eq!(p.repairs_by_kind["argument_reconcile"], 3);
        assert_eq!(
            (
                p.ollama_500_recoveries,
                p.plans_used,
                p.how_answers,
                p.hints_added
            ),
            (2, 3, 4, 5)
        );
        let r = Report::new(Profile {
            native: u64::MAX,
            text_written: u64::MAX,
            ..Profile::default()
        });
        assert_eq!(r.native_percent, 50.0);
        assert_eq!(r.text_written_percent, 50.0);
    }
}
