# Contributing to Pilot

Read [SPEC.md](SPEC.md) before changing behavior. Rust 1.88 or newer is required;
Cargo.toml is the source of truth for the minimum version. Build and check with:

```sh
cargo build --locked --bins
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --no-default-features --features core
cargo clippy --locked --all-targets --no-default-features --features core -- -D warnings
cargo test --locked --all-targets --no-default-features --features core
```

The default features are `core` and `proxy`. The `core` feature alone builds
the library without host I/O; see [plugin preparation](docs/plugin.md).

Use Result with clear errors, no unsafe code, and bounded sizes, counts, and
retries. Production code must not use unwrap, expect, panic, todo, or unreachable.
Tests use synthetic inputs and fake HTTP/MCP peers; no model or SolidCraft is
required. Keep the implementation in Rust.

Use only SolidCraft's public docs and binaries. Never read, copy, or search its
source, or any GPL/LGPL CAD source.

Run the paired benchmark and corpus evaluation as described in
[Measure it](docs/measure.md). Keep the endpoint, model, and task set fixed;
include commands and results with behavior changes. Do not commit keys or logs
containing private design data.

After changing CLI help, regenerate [the flags reference](docs/flags.md):

```sh
cargo run --locked --example generate_flags
```

The display name lives in src/identity.rs. Keep the crate, executable, data-directory,
and protocol names stable when updating product wording.

To release, set the package version, commit, and push the matching `vVERSION`
tag. The four platform archives and checksum files are attached to a draft
GitHub release for review before publication.
