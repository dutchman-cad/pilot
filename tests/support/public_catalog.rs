//! Read only the configured public binary's catalogue, once per test process.
use super::{Catalog, MAX_CATALOG_BYTES};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub fn catalog() -> Result<Catalog, String> {
    static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
    CATALOG.get_or_init(load).clone()
}

fn load() -> Result<Catalog, String> {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return Catalog::bundled();
    };
    let mut child = Command::new(path)
        .arg("commands")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start public catalogue command: {e}"))?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(|| "public catalogue command has no stdout".to_owned())
        .and_then(|stdout| {
            stdout
                .take(MAX_CATALOG_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("cannot read public catalogue: {e}"))
        });
    if read.is_err() || bytes.len() > MAX_CATALOG_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        read?;
        return Err("public catalogue exceeds 8 MiB".into());
    }
    let status = child
        .wait()
        .map_err(|e| format!("cannot wait for public catalogue: {e}"))?;
    if !status.success() {
        return Err(format!("public catalogue command failed: {status}"));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|e| format!("public catalogue is not UTF-8: {e}"))?;
    Catalog::from_json(text)
}
