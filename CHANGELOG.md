# Pilot changelog

## Unreleased

- Add opt-in `--direct-recipes` with native calls, SSE, measured no-model replies,
  ordinary approval, completion recovery, separate audit/profile counters, and
  `--upstream none`.
- Offer matched leading clauses of compound requests to the model and leave
  remaining work to it; direct mode stays off by default.

- Recognise ordinary recipe sentences with case, sentence punctuation, and
  value-first or label-first lengths, preserving decimals, fractions and names.
- Add round / round off, bevel, drill / bore / cut a hole, shift / slide, and
  call / name forms; require explicit hole positions and unique body references.
- Expand independently authored per-recipe positives and conservative refusals;
  retain missing-slot, ambiguous-body, competing-intent and action-history guards.
- Add bounded chat-completions request-body replay fixtures with exact plan
  assertions and three synthetic long-system-prompt examples, usable in core builds.

- Add model-face text tool mode for capabilities without tools and explicit HTTP
  400 unsupported-tools rejections; persist the rejection and retry once.
- Describe offered tools in a bounded prompt, flatten tool history on the wire,
  and audit conversion to native calls while preserving recipes and guardrails.

- Prepare 0.1.0-beta.2 with a typed recipe library for primitives, centred and
  positioned holes, rectangular / circular hole patterns, sketch extrusion,
  fillet / chamfer, move, rename, mirror, shell, revolve, and existing queries.
- Parse metric / inch lengths, simple fractions, tuple separators, and explicit
  radius / diameter slots; reject missing, conflicting, or multiple intents.
- In model plan mode, convert matching action prose into audited recipe calls;
  keep ordinary tool results and the model's final reply flow.
- Verify measured recipe completion once, including after accepted calls, without
  replaying accepted creation steps or increasing retry budgets.
- Add independent per-recipe phrasings, model-face replays, and an optional
  public-binary geometry contract smoke test. No live-model benchmark claim.

## 0.1.0-beta.1

First public early beta. Results are still being measured; expect rough edges.

- Preserve parameter alternatives and supplied radii; forbid semantic argument
  renames, require exact container parsing, reshape only unambiguous line endpoints,
  hand back invalid enum values, and separate typed templates from concrete examples.

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
