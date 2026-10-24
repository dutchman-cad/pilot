# SolidCraft 98 benchmark set

Eleven tasks for [SolidCraft 98](https://github.com/raykholo/solidcraft98), requiring
1.2 or later and its remote protocol. The prompts cover a cube, cylinder,
block, rectangle extrusion, centred through hole, edge rounding, moving, body
renaming, calculated volume, two through holes, and an arbitrary shape change.

The Rust checker runs references through Pilot's SolidCraft 98 driver with a
scripted completion client. No model is needed. Each case owns a fresh application
process started only with `--remote --headless`; closing its input stops it, with
a bounded kill and reap of that exact child if necessary. Setup uses the same typed
calls. Before and after execution, the checker reads every feature's parameter
dialog and cancels it without applying edits. Dimensions, positions, plane,
feature count, hole depth, hole centres, and feature status must match.

From the repository root, use new output directories:

```sh
cargo run --locked --example bench_sc98 -- /path/to/sc98 reference results/sc98-reference
cargo run --locked --example bench_sc98 -- /path/to/sc98 nothing results/sc98-nothing
cargo run --locked --example bench_sc98 -- /path/to/sc98 negatives results/sc98-negatives
cargo test --test bench_sc98
```

Each command exits successfully only when all outcomes match expectations.
Infrastructure or command errors always fail validation, including negative cases.
References must pass all 11 tasks. Do-nothing must fail all 11 tasks, including
all eight tasks requiring a shape change. Every negative must fail its check.
The other three tasks require an honest answer: solid-edge rounding and body
renaming are unavailable, and volume must be identified as calculated rather
than measured. An explicit sketch-corner alternative is allowed for rounding;
claiming solid-edge fillets is not. View, selection, clock, and focus changes do
not satisfy the arbitrary-change task.

`tasks.json` uses the benchmark format-1 envelope, exact prompts, checks, relative
reference paths, and reference answers. For this screen protocol, `setup` and
reference `commands` are arrays of explicit screen command strings. Each is capped
at 128 commands; each file is capped at 1 MiB. The runner accepts at most 256 tasks.
A driver run permits 80 turns, 120 seconds, and 131,072 tokens; probes and setup
also consume the session's bounded action and remote-command budgets. Cases run
serially. Records contain initial/final screens, parameter probes, driver
summaries, scores, and audit transcripts. These validation results establish the
checker and references, not model performance.

The earlier paired-run scripts and their scorer regressions are retained under
`bench/sc98/`. They require an explicit executable and external task-file path;
Python is needed only for that existing harness. Its model runs use `drive98`,
with the same prompts and settings in plain and Pilot modes. The Rust set above
is the self-contained public validation entry point.

The checked [validation record](validation.json) contains all 33 outcomes and
SHA-256 hashes of the task definitions, references, and executable used.
References passed 11/11; do-nothing and negatives each failed their task checks
11/11, with zero driver errors. No model performance score is implied.
