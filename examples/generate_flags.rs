#[path = "support/flags.rs"]
mod flags;

fn main() -> Result<(), String> {
    let text = flags::reference(&dutchman_pilot::cli::help(), &dutchman_pilot::eval::help());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/flags.md");
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
