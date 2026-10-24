# Pilot changelog

## Unreleased

## 0.1.0-beta.2 (2026-10-24)

- Direct exact-match answers with `--direct-recipes`, through SolidCraft's approval.
- Recipe matching for ordinary wording, shape synonyms, and metric or inch lengths.
- Text tool mode for models without native tool support.
- Plan-text recipes now require `--planner` and are off by default.
- Benchmark start files for repeatable comparisons.

### Added

- Typed recipes for primitives, tubes, centred and positioned holes, rectangular
  and circular hole patterns, sketch extrusions, edge finishes, move, rename,
  mirror, shell, revolve, uniform scale, and measured queries.
- `--direct-recipes` proposes normal tool calls and returns measured replies
  marked "Exact match (no model):" after approval. Supports streaming,
  `--upstream none`, separate logs and counters, and model recovery on failure.
- Direct edits can inspect once to resolve a unique current body before proposing
  changes; edge finishes can request topology before selecting edges.
- Text tool mode describes available tools and adapts tool history when native
  tools are unsupported. Remembers explicit provider rejections and retries once.
- Public v1, v2, and v3 benchmark sets, plus constraint, joint, revision, recovery,
  and unit checks, with reference answers and incorrect-result checks.
- Forty-eight SolidCraft 0.19 starting designs, rebuild support, and migration checks.
- Broader recipe checks covering ordinary requests, refusals, long prompts,
  inspection results, and measured geometry through the public binary.

### Changed

- `--planner` enables recipe suggestions, prose conversion, continuations, and
  completion checks on the model face. `--no-plan` and `--no-planner` remain
  accepted no-op aliases for the default. MCP suggestions are unchanged.
- `--direct-recipes` works independently and can accompany `--planner`. Partial
  requests receive suggested plans only with `--planner`; remaining work stays
  with the model. Argument repair, text-call pickup, command lookup, hints, and
  text tool mode keep their existing defaults.
- Recipes accept polite requests, ordinary shape and action synonyms, reordered
  dimensions, joined units, simple fractions, signed axes, and explicit radius
  or diameter wording. Decimal values and quoted names retain their meaning.
- Hole layouts accept end and corner insets; circle extrusions accept the plane
  first. Mirror requests can keep or remove the original, and scale requests can
  double a body about the origin.
- Completion checks compare measured results once, including after accepted
  calls, without repeating accepted creation steps or increasing retry limits.
- Benchmarks score the resulting design when the model exhausts its request limit.

### Fixed

- Direct volume and body-count answers use complete measured results, including
  current-solid references and requested cubic-unit conversions. Incomplete,
  conflicting, partial, or ambiguous measurements return to the model.
- Plane slots accept only XY, XZ, or YZ; name and axis extraction stay bounded.
- Missing dimensions, ambiguous bodies, competing intents, negations, and
  unsupported modifiers leave requests with the model. Edits require valid
  inspection results; hole recipes require explicit positions.
- Partial-request suggestions make clear that nothing has run and that the whole
  request still requires approved calls and returned results.

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
