//! Embed the reviewed Markdown library; development overrides are separate.
#![forbid(unsafe_code)]
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=recipes");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("recipes");
    let mut files = Vec::new();
    for (n, entry) in fs::read_dir(&root)?.enumerate() {
        if n >= 128 {
            return Err("recipes directory exceeds 128 entries".into());
        }
        let entry = entry?;
        if entry.path().extension().is_some_and(|s| s == "md") {
            if !entry.file_type()?.is_file() || entry.metadata()?.len() > 65536 {
                return Err("recipe must be a regular file of at most 64 KiB".into());
            }
            files.push(entry.path());
        }
    }
    files.sort();
    let mut source = String::from("const EMBEDDED: &[&str] = &[\n");
    for path in files {
        source.push_str(&format!(
            "include_str!({:?}),\n",
            path.to_str().ok_or("recipe filename must be UTF-8")?
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("recipes.rs"),
        source,
    )?;
    Ok(())
}
