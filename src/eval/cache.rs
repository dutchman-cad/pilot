use super::*;
const TTL: u64 = 14 * 24 * 60 * 60;
const MAX_ENTRIES: usize = 512;
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Cache {
    entries: BTreeMap<String, Entry>,
}
#[derive(Serialize, Deserialize)]
struct Entry {
    identity: String,
    saved_at: u64,
    result: CaseResult,
}
// Stable local index. Identity is also compared in full, so collisions cannot reuse a pass.
fn fingerprint(identity: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for b in identity.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
impl Cache {
    pub(super) fn load(path: &Path, now: u64) -> Result<Self, String> {
        if !path
            .try_exists()
            .map_err(|e| format!("cannot inspect cache: {e}"))?
        {
            return Ok(Self::default());
        }
        let mut cache: Self = serde_json::from_str(&read_bounded(path, MAX_FILE)?)
            .map_err(|e| format!("invalid cache: {e}"))?;
        if cache.entries.len() > MAX_ENTRIES {
            return Err("cache exceeds 512 entries".into());
        }
        cache
            .entries
            .retain(|_, e| now >= e.saved_at && now - e.saved_at < TTL);
        Ok(cache)
    }
    pub(super) fn get(&self, identity: &str, now: u64) -> Option<CaseResult> {
        let e = self.entries.get(&fingerprint(identity))?;
        (e.identity == identity
            && now >= e.saved_at
            && now - e.saved_at < TTL
            && e.result.classification == Verdict::Pass
            && !e.result.turns.is_empty()
            && e.result.turns.len() <= 10
            && e.result.turns.iter().all(|t| {
                t.recording.raw.len() <= MAX_REPLY
                    && judge(&t.recording, &e.result.case.claim).verdict == Verdict::Pass
            }))
        .then(|| e.result.clone())
    }
    pub(super) fn observe(&mut self, identity: &str, result: &CaseResult, now: u64) {
        let key = fingerprint(identity);
        // Every live miss evicts any older pass before considering this new result.
        self.entries.remove(&key);
        if result.classification != Verdict::Pass {
            return;
        }
        if self.entries.len() >= MAX_ENTRIES {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.saved_at)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            key,
            Entry {
                identity: identity.into(),
                saved_at: now,
                result: result.clone(),
            },
        );
    }
    pub(super) fn save(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let bytes = serde_json::to_vec(self).map_err(|e| format!("cannot serialize cache: {e}"))?;
        if bytes.len() > MAX_FILE {
            return Err("cache exceeds 128 MiB".into());
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| {
                format!(
                    "cannot create cache temporary file {}: {e}",
                    temporary.display()
                )
            })?;
        let result = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = result.and_then(|()| fs::rename(&temporary, path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| format!("cannot save cache {}: {e}", path.display()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_identity_and_live_miss() {
        let mut cache = Cache::default();
        let result = crate::eval::tests::passing_result();
        cache.observe("prompt+tools+model+utterance", &result, 100);
        assert!(cache
            .get("prompt+tools+model+utterance", 100 + TTL - 1)
            .is_some());
        assert!(cache
            .get("prompt+tools+model+utterance", 100 + TTL)
            .is_none());
        assert!(cache.get("different tools", 101).is_none());
        assert!(cache.get("prompt+tools+model+utterance", 99).is_none());
        let mut failed = result;
        failed.classification = Verdict::Broken;
        cache.observe("prompt+tools+model+utterance", &failed, 101);
        assert!(cache.get("prompt+tools+model+utterance", 102).is_none());
    }
}
