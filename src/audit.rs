//! Shared bounded JSONL rewrite records for both proxy faces.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

const MAX_LOG_BYTES: u64 = 64 * 1_048_576;
static WARNED: AtomicBool = AtomicBool::new(false);

fn warn(error: &io::Error) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("Pilot: cannot save rewrite log; skipping audit records: {error}");
    }
}

pub(crate) struct Audit {
    pub(crate) events: Option<Vec<crate::capture::Event>>,
    file: Option<File>,
    path: PathBuf,
    max_bytes: u64,
    pub(crate) size: u64,
    pub(crate) model: String,
    pub(crate) count: usize,
    pub(crate) kinds: BTreeMap<String, u64>,
}
impl Audit {
    pub(crate) fn open(path: PathBuf, model: String) -> Self {
        Self {
            events: None,
            file: None,
            path,
            max_bytes: MAX_LOG_BYTES,
            size: 0,
            model,
            count: 0,
            kinds: BTreeMap::new(),
        }
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.file.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            self.size = file.metadata()?.len();
            self.file = Some(file);
        }
        if self.size.saturating_add(bytes.len() as u64) > self.max_bytes {
            // Close before renaming so this also works on Windows.
            self.file = None;
            let mut backup = self.path.as_os_str().to_os_string();
            backup.push(".1");
            let backup = PathBuf::from(backup);
            match fs::remove_file(&backup) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            fs::rename(&self.path, &backup)?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            self.size = file.metadata()?.len();
            self.file = Some(file);
        }
        if let Some(file) = &mut self.file {
            file.write_all(bytes)?;
            file.flush()?;
            self.size = self.size.saturating_add(bytes.len() as u64);
        }
        Ok(())
    }

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
        if let Err(error) = self.write(&bytes) {
            warn(&error);
            // Reinspect the current path next time, including after a partial write.
            self.file = None;
        }
        // Wire and size observations are audit records, not request rewrites.
        if !matches!(
            kind,
            "request_size" | "form_request" | "upstream_request" | "upstream_response"
        ) {
            self.count += 1;
        }
        let kind = kind.split(':').next().unwrap_or(kind);
        let count = self.kinds.entry(kind.to_owned()).or_default();
        *count = count.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pilot-audit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rotates_and_replaces_backup_then_reopens_current_path() {
        let dir = Directory::new();
        let path = dir.0.join("rewrites.jsonl");
        let backup = dir.0.join("rewrites.jsonl.1");
        let mut audit = Audit::open(path.clone(), "test".into());
        audit.record("first", &Value::Null, &json!(1)).unwrap();
        let first = fs::read(&path).unwrap();
        audit.max_bytes = audit.size;
        fs::write(&backup, "old backup").unwrap();
        audit.record("second", &Value::Null, &json!(2)).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), first);
        let record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(record["kind"], "second");
        assert_eq!(audit.size, fs::metadata(&path).unwrap().len());
        assert_eq!(audit.count, 2);
        drop(audit);
        let mut next = Audit::open(path.clone(), "test".into());
        next.max_bytes = 1024;
        next.record("third", &Value::Null, &Value::Null).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
        assert_eq!(fs::read(&backup).unwrap(), first);
    }

    #[test]
    fn failed_rotation_skips_record_without_error_and_can_recover() {
        let dir = Directory::new();
        let path = dir.0.join("rewrites.jsonl");
        let backup = dir.0.join("rewrites.jsonl.1");
        let mut audit = Audit::open(path.clone(), "test".into());
        audit.record("first", &Value::Null, &Value::Null).unwrap();
        let first = fs::read(&path).unwrap();
        audit.max_bytes = audit.size;
        // A directory at the backup path deterministically prevents rotation,
        // including when tests run with elevated filesystem privileges.
        fs::create_dir(&backup).unwrap();
        for _ in 0..2 {
            assert!(audit.record("skipped", &Value::Null, &Value::Null).is_ok());
        }
        assert_eq!(fs::read(&path).unwrap(), first);
        assert_eq!(audit.count, 3);
        fs::remove_dir(&backup).unwrap();
        audit
            .record("recovered", &Value::Null, &Value::Null)
            .unwrap();
        assert_eq!(fs::read(&backup).unwrap(), first);
        assert!(fs::read_to_string(&path).unwrap().contains("recovered"));
    }

    #[test]
    fn unavailable_log_does_not_fail_rewrites() {
        let dir = Directory::new();
        let mut audit = Audit::open(dir.0.clone(), "test".into());
        audit.record("rewrite", &Value::Null, &Value::Null).unwrap();
        assert_eq!(audit.count, 1);
        audit.count = 4096;
        assert!(audit.record("rewrite", &Value::Null, &Value::Null).is_err());
        audit.count = 0;
        assert!(audit
            .record("rewrite", &Value::Null, &json!("x".repeat(8 * 1_048_576)))
            .is_err());
    }
}
