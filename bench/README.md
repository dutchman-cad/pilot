<!-- SPDX-License-Identifier: Apache-2.0 -->
# SolidCraft Assistant model benchmarks

Public, reproducible CAD tasks for comparing a model directly with the same model
through Pilot. Python 3.9+ and its standard library are sufficient. Install
SolidCraft 0.19+ separately; pass its `solidcraft-cli` executable directly. No
container, desktop, GPU, or model is needed for reference and checker validation.
The Pilot executable is needed only for paired model runs.

## Run a task by hand

Find the task in `sets/<set>/tasks.json`, open its `start/<id>.solidcraft`
file from that set directory in SolidCraft, and paste the task's `prompt` into
the Assistant. Tasks without a start file begin from an empty design. For
multi-turn tasks, paste each successive turn's prompt after completing the
previous turn.

All 48 starting files were saved with SolidCraft 0.19.0 and open in SolidCraft
0.19 and later; newer versions migrate them on open. No task requires a newer
minimum version. Each set's `start/VERSION` records the version used to save it.

Rebuild with the oldest supported release, SolidCraft 0.19.0:

```sh
python3 bench/make_starts.py --solidcraft /path/to/0.19.0/solidcraft-cli
python3 bench/make_starts.py --solidcraft /path/to/0.19.0/solidcraft-cli \
  --check --open-with /path/to/0.21.0/solidcraft-cli
```

Use `--set v1` to select one set. `--solidcraft PATH` is required; the builder
prints its version and never selects another binary. `--check` writes nothing.
Repeat `--open-with PATH` to test additional releases. Every build and check
compares the saved design with replayed setup: body count, each body's volume
(0.1% tolerance) and bounding box (0.000001 mm tolerance), parameter names and
values, and timeline length and feature types. Additional releases are compared
with the original replay, so migration differences identify the affected task.

| Set | Tasks | Measures |
| --- | ---: | --- |
| field | 1 | Verbatim user field reports; centred cube hole placement |
| v1 | 11 | Primitives, sketch/extrude, holes, fillets, moving, naming, measurement and making a change |
| v2 | 12 | More involved geometry, parameter edits, patterns, selected edges, threads, assemblies, constrained sketches and measurement |
| v3 | 40 | Solid builds/edits, sheet metal and honest refusals; a selected subset, with no claimed difficulty calibration |
| multiturn | 5 | Three successive revisions in one conversation, checking every intermediate design |
| recovery | 5 | Repairing wrong earlier feature settings in place |
| constraints | 5 | Fully defined, dimensioned L profiles across XY, XZ and YZ |
| joints | 5 | Revolute, slider, cylindrical, pin-slot and ball joints; exact endpoints and limits |
| units | 5 | Inch/fraction conversion and asking for missing dimensions before changing a design |

The original prompts and pass rules for v1/v2 are described in
[SolidCraft issue 4851](https://github.com/tangency-labs/solidcraft/issues/4851).
This harness uses the public headless MCP tools and initialization instructions.
It does not reproduce the desktop Assistant's complete prompt, tool selection or
chat loop, so label these results **Pilot headless benchmark format 1**, rather
than claiming they are desktop Assistant scores. Only native tool calls execute;
JSON written as prose stays prose unless Pilot converts it.

## Run

From the repository root, replace the example binary paths and model identifier.
An Ollama root URL, its `/api` URL, or an OpenAI-compatible `/v1` base URL is accepted.
For authenticated endpoints set `BENCH_API_KEY` in the environment; it is forwarded
as a bearer header and never written to the result files.

```sh
python3 bench/run.py --set v1 --model MODEL --endpoint http://127.0.0.1:11434 \
  --solidcraft /path/to/solidcraft-cli --reps 3 --out results/v1-direct
python3 bench/run.py --set v1 --model MODEL --endpoint http://127.0.0.1:11434 \
  --through-pilot /path/to/dutchman-pilot \
  --solidcraft /path/to/solidcraft-cli --reps 3 --out results/v1-pilot
python3 bench/report.py results/v1-direct/results.jsonl results/v1-pilot/results.jsonl
```

Use `--set v2` and distinct output directories for v2. `--through-pilot` starts
`dutchman-pilot model --listen ... --upstream ...` on an available loopback port
and stops it on exit. It uses Pilot's default options; record the Pilot version
when sharing results. The endpoint must already serve the selected model.
The output directory must have no existing `results.jsonl`.

Validate each set before measuring a model:

```sh
python3 bench/run.py --set v1 --solidcraft /path/to/solidcraft-cli \
  --verify-reference --out results/v1-reference
python3 bench/run.py --set v1 --solidcraft /path/to/solidcraft-cli \
  --do-nothing --out results/v1-nothing
python3 bench/run.py --set v1 --solidcraft /path/to/solidcraft-cli \
  --negatives --out results/v1-negatives
python3 -m unittest discover -s bench/tests -v
```

All nine catalogue set names work with every runner mode. For each additional set, run
both paths with the same model and repetition count:

```sh
for SET in v2 v3 multiturn recovery constraints joints units field; do
  python3 bench/run.py --set "$SET" --model MODEL --endpoint http://127.0.0.1:11434 \
    --solidcraft /path/to/solidcraft-cli --reps 3 --out "results/$SET-direct"
  python3 bench/run.py --set "$SET" --model MODEL --endpoint http://127.0.0.1:11434 \
    --through-pilot /path/to/dutchman-pilot \
    --solidcraft /path/to/solidcraft-cli --reps 3 --out "results/$SET-pilot"
  python3 bench/report.py "results/$SET-direct/results.jsonl" "results/$SET-pilot/results.jsonl"
done
```

Repeat the three validation commands with each set name. References must pass;
negatives must fail their checks; do-nothing checks must match `do-nothing.json`.
Validation opens each task's start file and checks it against replayed setup
before scoring. Negative cases with identical setup share the corresponding
start file; other negative cases replay their own setup. Use `--replay-setup`
in any runner mode to force replay instead of opening files.
`--do-nothing` opens only the starting design and supplies an empty reply;
question and refusal tasks therefore also fail their reply predicates. An execution error,
missing measurement or timeout fails validation. Exit status
is 0 only if every expected outcome matches, 1 for an unexpected outcome and 2
for a configuration/startup error. In model mode, a failed task yields status 1
while the remaining tasks still run. A turn that uses all 24 model requests
ends there; its design is scored as it stands and the record says
`"stopped": "request limit"`.

## Format 1

`tasks.json` is an object with top-level `format: 1`, a license identifier and
`tasks`, an array. Task objects retain the original fields:

```json
{
  "format": 1,
  "tasks": [{
    "id": "example-cube",
    "prompt": "Make a 20 mm cube.",
    "reference": "references/example-cube.json",
    "setup_steps": 0,
    "check": {"bodies": 1, "volume": 8000, "bbox_size": [20, 20, 20]}
  }]
}
```

`setup` optionally supplies starting commands explicitly; otherwise the first
`setup_steps` commands of the reference build the start. The reference contains
`commands: [{"command": "solid.box", "params": {...}}, ...]` and a license
identifier. `start` optionally names a saved starting design relative to the set
directory, next to `setup_steps`; those setup commands remain in the
reference. `reference_reply` supplies the answer for reference-only measurement
or refusal checks. It is never sent to a model. `title`, `kind` and `tier` are
optional descriptive fields. Negative task objects use the same format, with
references relative to the set directory. No adapter-specific task fields are
required; a future `bench-assistant` adapter can consume the task array.

### Multi-turn extension

Only multi-turn tasks add `turns`, an ordered array of objects containing `prompt`,
`check` and `reference_end`. The top-level `prompt` equals the first turn prompt;
it is not sent a second time. `reference_end` is an exclusive command index in
the complete reference, including setup. Indices strictly increase and the last
one must consume the entire reference, leaving no unscored trailing commands.
A turn may optionally include `reference_reply` for reference validation.

The runner retains one design and one conversation for all turns. Each turn is
checked against its own starting state, then the top-level `check` is applied to
the final design against the original setup. Success requires every intermediate
check and the final check. Each turn has the same request/time budget. A skipped
middle revision fails even when the final dimensions are right. Recovery tasks
use ordinary setup commands that create the wrong settings; no failed-command
allowance or special recovery envelope is needed.

`sketch_defined.values` requires driving dimension values with multiplicity at
zero degrees of freedom. Joint checks include exact endpoint occurrence names,
limits and joint count. `ask_first` requires exactly one relevant question mark,
no completion claim and a full unchanged-design snapshot. Honest refusals also
require an unchanged snapshot and an explicit inability statement. These are
check predicates, not additional task-format extensions.

## Scoring and recordings

Each task starts a fresh CLI process, design, and conversation. Its start file
is opened through `mcp --in` when present; otherwise setup is replayed before
the exact prompt is sent once. Model settings are temperature 0, seed 1,
and a maximum of 4096 output tokens per request; endpoint implementations may
interpret these differently. A user turn allows at most 24 model requests, 128
tool calls and 600 seconds. There are no harness retries or follow-up nudges.
Pilot may apply its own bounded retries. Limits also include 16 MiB per JSON
message/file, 256 tasks, 1024 reference/setup/batch commands and 1–20 repetitions.

A task passes only when every specified check passes. Volume tolerance defaults
to 1%; a task's `tol` overrides it. `bbox_size` permits axis permutations, with
1% tolerance (minimum scale 1 mm); `bbox_min` has 0.05 mm tolerance. Other checks
cover absolute bounds, feature types/counts/identity, names, parameters,
round-face counts/centres, centre of mass, component membership, joint motion,
and fully constrained sketches. Reply checks use the supplied regular
expressions; they are deterministic checks, not a language-model judge.
`check.py` contains the exact tolerances and semantics.

Scores count passes / attempted task-repetitions. Execution errors count as
failures and are shown separately. Missing task rows are absent, so compare the
denominator against the set count times repetitions. The report pairs identical
set/model/task/repetition keys and reports the change in pass count over those
pairs; unmatched trials are excluded from the paired change. It rejects duplicate
trial keys. Keep endpoint settings, model weights/quantization, context size and
SolidCraft/Pilot versions equal between direct and Pilot runs.

Each per-task JSON records before/final documents, read-only query replies,
per-turn checks, final replies and tool/model transcripts. `results.jsonl` holds
compact summaries. `check.evaluate` can rescore those documents and query replies
without SolidCraft. Recorded fixtures under `tests/` exercise passes and failures.
Logs and transcripts can contain user prompts or model-generated paths; review
and redact results before publishing them.

## Contribute a result

Post a comment on [pilot issue #1](https://github.com/dutchman-cad/pilot/issues/1)
with the set and format version, repository commit, model name/weights/quant,
runtime version, context size, SolidCraft and Pilot versions, hardware class,
repetitions, direct/Pilot table, per-task failures and total elapsed time.
Attach sanitized `results.jsonl` files and relevant failing recordings. Include
reference, do-nothing and negative validation outcomes. Mark changed prompts,
settings or pass rules explicitly; do not mix those scores with unchanged runs.
Do not include credentials, personal paths or identifying computer names.

To add a task, contribute its exact prompt, setup, check, command-only reference,
a deliberate near miss, do-nothing expectation and recorded pass/fail fixtures.
Keep all contributions Apache-2.0 and derive them from public interfaces.

## SolidCraft 98

The [SolidCraft 98 set](sets/sc98/README.md) adds 11 screen-protocol tasks for
[SolidCraft 98](https://github.com/raykholo/solidcraft98) 1.2 or later.
It uses the Rust `bench_sc98` example and Pilot's SolidCraft 98 driver, with a
headless FreeBASIC executable. Run its reference, do-nothing, and negative modes
as documented in the set page. These tasks use screen commands and parameter
dialog probes; the SolidCraft catalogue runner above does not execute this set.
