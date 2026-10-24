use std::collections::BTreeSet;

pub fn flags(help: &str) -> BTreeSet<&str> {
    help.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .filter(|token| token.starts_with('-') && token.bytes().any(|c| c.is_ascii_alphabetic()))
        .collect()
}

pub fn reference(main: &str, eval: &str) -> String {
    let mut doc = String::from("# Flags reference\n\nGenerated from the CLI help. Regenerate with `cargo run --locked --example generate_flags`.\nThe test suite compares this page with both binaries' help output.\n\n");
    for (binary, help) in [
        (dutchman_pilot::identity::BINARY, main),
        (dutchman_pilot::identity::EVAL_BINARY, eval),
    ] {
        doc.push_str(&format!(
            "## {binary}\n\n```text\n{help}```\n\nFlags (including aliases):\n\n"
        ));
        for flag in flags(help) {
            doc.push_str(&format!("- `{flag}`\n"));
        }
        doc.push('\n');
    }
    format!("{}\n", doc.trim_end())
}
