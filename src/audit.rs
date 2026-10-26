//! Shared bounded JSONL rewrite records for both proxy faces.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) struct Audit {
    pub(crate) events: Option<Vec<crate::capture::Event>>,
    pub(crate) file: File,
    pub(crate) size: u64,
    pub(crate) model: String,
    pub(crate) count: usize,
    pub(crate) kinds: BTreeMap<String, u64>,
}
impl Audit {
    pub(crate) fn capture(&mut self, kind: &str, after: &Value) {
        if let Some(events) = &mut self.events {
            if events.len() < crate::capture::MAX_EVENTS {
                events.push(crate::capture::audit_event(kind, after));
            }
        }
    }
    pub(crate) fn record(
        &mut self,
        kind: &str,
        before: &Value,
        after: &Value,
    ) -> Result<(), String> {
        self.record_context(kind, before, after, &Value::Null)
    }

    pub(crate) fn record_context(
        &mut self,
        kind: &str,
        before: &Value,
        after: &Value,
        context: &Value,
    ) -> Result<(), String> {
        self.capture(kind, after);
        if self.count >= 4096 {
            return Err("turn exceeds 4096 rewrites".into());
        }
        let mut record =
            json!({"time":now(),"model":self.model,"kind":kind,"before":before,"after":after});
        if let (Some(record), Some(context)) = (record.as_object_mut(), context.as_object()) {
            for key in ["session", "observation", "transport", "dutchman-pilot"] {
                if let Some(value) = context.get(key) {
                    record.insert(key.into(), value.clone());
                }
            }
        }
        let mut bytes =
            serde_json::to_vec(&record).map_err(|e| format!("cannot encode rewrite: {e}"))?;
        bytes.push(b'\n');
        if bytes.len() > 8 * 1_048_576 {
            return Err("rewrite record exceeds 8 MiB".into());
        }
        if self.size.saturating_add(bytes.len() as u64) > 64 * 1_048_576 {
            return Err("rewrite log reached 64 MiB; archive it before continuing".into());
        }
        self.file
            .write_all(&bytes)
            .and_then(|()| self.file.flush())
            .map_err(|e| format!("cannot write rewrite log: {e}"))?;
        self.size += bytes.len() as u64;
        // Size observations are audit records, not request rewrites.
        if kind != "request_size" && kind != "form_request" {
            self.count += 1;
        }
        let kind = kind.split(':').next().unwrap_or(kind);
        let count = self.kinds.entry(kind.to_owned()).or_default();
        *count = count.saturating_add(1);
        Ok(())
    }
}
