# Pilot changelog

## Unreleased

- Add 48 SolidCraft 0.19 starting designs, reproducible rebuilds, and migration checks to the public benchmark.

## 0.1.0-beta.1

First public early beta. Results are still being measured; expect rough edges.

- Public clap-derived `cli::Command` and synchronous `cli::run` for embedding
  Pilot subcommands with the same flags and explicit exit codes.
- `cli::with_bin_name` and `cli::run_named` so an embedding program's help names
  its own command.
- Local OpenAI-compatible model proxy for SolidCraft, with buffered streaming,
  explicit JSON tool-call conversion, schema-guided argument repair, bounded
  retries, provider error messages, and rewrite logs.
- MCP stdio proxy with command discovery, corrected-call suggestions, state
  hints, truthful paging, and deterministic plans. Suggestions require an
  explicit call from the client; prose never executes a command.
- Catalogue-based planning for common requests, per-model profiles, optional
  prompt guidance, and Ollama model settings.
- Corpus evaluation with repeated trials, bucket floors, recorded replies,
  offline rescoring, and a bounded pass cache for paired measurements.
- Portable Rust core for catalogue search, planning, repair, and hints, including
  a wasm32 build without host I/O.
- Release archives with both command-line binaries, documentation, the starter
  corpus, license notices, and SHA-256 checksums.
