# Behavior and limits

Pilot is a local, opt-in helper for small language models driving SolidCraft through its
public interfaces. Rust only; no SolidCraft source or CAD library dependency.

Two faces share a library: `model` is a blocking OpenAI-compatible model proxy;
`mcp` is a stdio tools proxy. The library provides bounded JSON repair and
deterministic command discovery.

## Model proxy

Start the opt-in proxy and point your client's base URL at `http://127.0.0.1:11500/v1`:

```sh
cargo run -- model --upstream http://localhost:11434/v1 --log rewrites.jsonl
cargo run -- profile
cargo run -- profile qwen2.5:7b --json
cargo run -- log-summary rewrites.jsonl --json
```

It passes Authorization to the configured upstream without storing it, converts
explicit JSON calls only, repairs stringified containers, and buffers streaming
turns before emitting SSE. Native calls retain their original JSON bytes when no
rewrite is needed. Guards return at most two nudges to the model per turn; empty
replies get one retry. Provider failures retain HTTP status and return a human
message plus `error.code`. It never executes a command itself.

Argument reconciliation preserves meaning: only equivalent spellings (case or
separators) may be renamed. A matching type never justifies
moving a value to another parameter: radius is never diameter, and width is never
height. Catalogue alternatives such as `radius | diameter: number` declare both
names and require either, rather than requiring the last name. Unknown fields
remain for SolidCraft's validation unless the advertised schema forbids them.
Stringified containers must parse completely as the schema's declared container,
including the supported JSON5 subset or fences; nested values are never repaired
by inserting or dropping brackets. Coordinates and units are preserved.

When the catalogue declares `sketch.line`'s `points`, exactly two parameters
`p0` and `p1`, each a 2D point, can be reshaped to `points: [p0, p1]`.
Other endpoint shapes receive the expected-shape hand-back; 3D coordinates are
never flattened. Invalid enum values, including extrude operation `create`, get
the allowed catalogue values instead of a guessed replacement. These rewrites
use the existing audit log and retry limits, and SolidCraft validates every call.
The MCP face likewise suggests only equivalent parameter spellings, never a
same-type semantic substitution, and never re-runs suggestions automatically.

The model face advertises `how` using the same schema and command discovery as
the tools face. Pilot answers it locally and re-asks the model; SolidCraft receives
only the final reply. Up to two local answers share a two-hand-back budget
with guards (guards still get at most two nudges). A further `how` call returns a
bounded proxy error rather than reaching SolidCraft. Mixed replies send only the
local calls into retry history; other calls must be proposed again afterward.
Trailing SolidCraft tool results receive one appended `[dutchman-pilot hint]` line when
their state supports guidance; original result text is preserved. Unchanged-design
hints require comparable before/after states in the result or conversation history.
Use `--no-how` and `--no-hints` independently for A/B measurements.

Before the first model call, requests offering `execute` or `batch` receive a
short box placement fact when the latest user message mentions a box, cube,
block, plate, brick or slab and center, centre, centred, centered, middle or through.
It shares the prompt-help system channel, appending to an existing system message
when available. Explicit `center:[...]` / `centre:[...]` coordinates suppress it;
an existing fact anywhere in the conversation prevents repetition across turns.
MCP has no user conversation, so its tool list carries the same fact once in the
`execute` description (or `batch` when `execute` is absent). Both faces audit
`placement_fact`, add less than 400 bytes, and honour `--no-hints`.

Size fact: when the latest user message states sizes with a unit in inches (`in`,
`inch`, `inches`, `"`, including fractions and mixed numbers such as `1/8"`, `3/8 in`,
`1-1/2"`, `1 1/2 in`) or in cm, and no recipe, form, route or exact match handles the
turn, Pilot adds one system line, for example
`Sizes in mm: 1" = 25.4; 3/8" = 9.525; 1/8" = 3.175.`, listing each stated size in request
order with exact mm (at most 16 sizes, at most 4 decimals, trailing zeros trimmed). Only
sizes written with a unit count: bare numbers, mm, and a fraction with no unit add nothing,
and nothing is inferred. The request text is never changed. It is sent once per
conversation (existing-history deduplication), audited as `unit_fact`, and `--no-hints`
disables it.
The 0.20 binary was checked with a 20 mm box (bbox [0,0,0] to [20,20,20]),
a radius 5, height 20 cylinder (approximately [-5,-5,0] to [5,5,20]), and a
radius 5 sphere ([-5,-5,-5] to [5,5,5]). Thus the sphere's centre defaults to
the origin; the cylinder's bottom centre defaults to the origin.

Plan-text recipes require `--planner` and are off by default. `--no-plan` (alias
`--no-planner`) is accepted as a no-op alias for the default. Independent direct
answering enables exact-match calls and measured replies by default with normal
client approval and SolidCraft validation; it can accompany `--planner`.
`--no-direct-recipes` sends everything to the model; `--direct-recipes` remains a
no-op alias and conflicts with the opt-out. `--upstream none` requires direct
answering and rejects the opt-out. Direct answering stays off for SolidCraft 98.
See [the recipe library](recipes.md) for
supported slots, geometry, examples, and limits. It adds catalogue-checked calls
as a system message and audits rule, typed slots, expected result, and calls.
In model plan mode, an action pledge parsed as the same recipe and slots can
become native tool calls. The client executes them, SolidCraft validates them,
and the model receives the results before writing its reply. Arbitrary prose
stays prose. After the final reply, a single bounded completion check compares
measured count / volume and available name / placement facts, handing back a
difference even if an earlier tool call was accepted. Requests that fired no
recipe receive no additional completion guard.

Profiles are keyed by upstream URL and model in the user's data directory
(`$XDG_DATA_HOME/dutchman-pilot`, otherwise the platform data directory); override with
`--profile-dir DIR`. `profile [model] [--json]` prints one compact table per
model/upstream pair (or a JSON array), with turns, native/text-converted/prose-only
percentages, empty replies, repairs by kind, hand-backs, provider errors, Ollama-500
recoveries, plans used, how answers, and hints added. Percentages use all observed
replies, including retries, alternative choices, and empty replies; turns count client
requests. New counters default to zero for older profiles; historical repair kinds
cannot be recovered from an old aggregate count. Repair-kind counts are log events:
`argument_json` can accompany `argument_repair` or `argument_reconcile`, so their
sum can exceed the aggregate repair count. Recovery and plan counters consume
`ollama 500 recovered` and `plan` events.
`log-summary <rewrites.jsonl> [--json]` counts exact kinds (including argument paths)
and models, with per-model kind counts. It reads any explicit `--log` file, skips
blank lines, and reports malformed records with a file and line number. Reports
are bounded to 256 profiles/models; logs to 64 MiB, 8 MiB per line, one million
events and 4096 distinct kinds. JSON output uses `total`, `by_kind`, and `by_model`
(each model has `total` and `by_kind`); profile JSON retains raw counters and adds
`native_percent`, `text_written_percent`, and `prose_percent`.

Auto fallback uses Ollama capabilities lacking tools, or text-written calls in at least two of
the last five client turns. Text-call frequency alone keeps native tools enabled. Each client turn
counts once in that window, including its retries; old profiles start with an empty
window. The fallback log entry names the rule that fired.
The model face enters text tool mode when probed capabilities lack `tools`, or a
profile remembers an explicit rejection. It also recognises HTTP 400 containing
`does not support tools` (case-insensitive); this is the complete, deliberately
small pattern list. Other provider failures retain their existing error handling.
On that rejection Pilot persists `text_tool_mode: true` and retries the same turn
once. This retry is separate from existing hand-back and parse-retry budgets.
Capability-based selection works even with `--fallback-prompt off`: that flag
controls the conditional prompt, while text mode makes the chat template usable.

In text mode the upstream receives no `tools`, `tool_choice`, or
`parallel_tool_calls`. A system block lists only the offered tools (including
`how` when enabled), their short purposes and parameter names/types; it does not
expand the command catalogue into fictional tools. The block is capped at 32 KiB
(an oversized block returns a clear error), with 160-byte purposes and schemas
at most three levels deep and 64 properties per level. The model is asked for
exactly one object `{"tool": NAME, "args": {...}}`, or plain text when no action
is needed. Fences and a leading sentence are accepted; several objects remain
text. Converted calls use the existing argument repairs and validation paths.
Recipes and prose guardrails still use the original tools and history.

Only the upstream wire copy flattens assistant tool calls into assistant text and
tool results into user text (`Tool NAME returned: ...`), preserving call-id/name
associations. Each rewritten message is capped at 16 KiB plus a truncation marker.
This also applies to Pilot's local `how` results on retries. Audit kinds are
`text_tool_mode`, `tools_stripped`, `tool_history_flattened`, and
`text_call_converted` (alongside the existing `text_tool_call` repair event).
Native-tool models keep their request format. The MCP face is unchanged.

Every rewrite is recorded in `--log FILE`, or `rewrites.jsonl` beside profiles;
responses include `x-dutchman-pilot: rewrites=N`. Archive the log when it reaches 64 MiB.

`--prompt-help` adds one system block under 1,500 bytes with three design lookup
and disambiguation rules and at most 15 relevant `id: purpose` lines from the
bundled public catalogue. Tool results reporting an open sketch put sketch
commands first. An active conditional fallback shares this block.

`--model-settings` opts into settings for confirmed Ollama upstreams: temperature
0.1 whenever tools are offered and `options.num_ctx` 8192 if absent in the request.
`--seed INTEGER` sets a fixed seed and requires `--model-settings`. Explicit request
context sizes and other options are preserved. Prompt and settings additions are logged.

For per-model overrides, `--model-settings FILE` reads JSON keyed by model:

```json
{"qwen2.5:7b":{"temperature":0.1,"seed":42,"num_ctx":8192,"json_retry":true}}
```

All settings apply only to detected Ollama servers. Temperature applies whenever
tools are offered; seed applies when configured (`--seed` overrides the file),
and `num_ctx` defaults to 8192 (file range 8192–131072). JSON mode is
enabled only on a retry for an already proposed structured call. Omit a model
from this file to preserve its settings.

Limits include 1 MiB bodies, 16 KiB request headers, 256 input messages, 128 tools,
64 calls per choice, eight choices, 256 batch steps, and 256 model profiles.
Requests have a five-second ingress deadline. Upstream calls default to 300 seconds
(`--upstream-timeout`, 1–3600 seconds), with at most four calls plus a three-second
capability probe per client turn. Connections close after one request. Upstream
redirects and environment HTTP proxies are disabled; localhost is the default
listen address. Tests using fake HTTP servers require permission to bind localhost.

## Tools proxy

Run `cargo run -- mcp --solidcraft-cli /path/to/solidcraft-cli -- <child args>`.
The proxy loads the live catalogue after MCP initialization, adds `how` and `plan`, and
appends conservative suggestions, state hints, and truthful paging notices.
Suggestions never execute automatically. Unmodified messages retain their bytes;
rewrites use the same JSONL audit writer as the model face, with `model: "mcp"`.
`dutchman pilot log-summary` can summarize a combined log from both faces. Local answers,
tool-list additions, corrected calls, paging notices, and hints are all recorded.
Messages are capped at 8 MiB, queued requests
at 64, batch steps at 256, read-only probes at 3 seconds, and calls at 120 seconds.

Call `plan` with `{"task":"make a 20 mm cube"}` to obtain `rule`, exact `calls`,
and any computed `facts`. It reads current design state through an inspect probe;
unsupported or ambiguous requests return a no-plan message. On a failed tool call,
Pilot uses an explicit `arguments.task`, or request text in `arguments.command`,
to append the same planner calls as a `suggested` plan. It preserves the original
error and never executes suggestions. Failed batches only get corrections for an
explicitly identified failing step, since earlier steps may have succeeded.

Both faces enable discovery and state hints by default. MCP also enables plan
suggestions by default; model-face plan-text recipes require `--planner`:

| Flag | `model` | `mcp` |
| --- | --- | --- |
| `--no-plan` | Accepted no-op alias for the default (`--no-planner` alias). | Disable the local `plan` tool and planner suggestions. |
| `--planner` | Enable plan-text recipe suggestions, prose conversion, continuations, and completion checks. | Not available. |
| `--direct-recipes` | No-op alias for default exact answers with normal approval; accepts `--planner`. | Not available. |
| `--no-direct-recipes` | Send everything to the model; conflicts with `--direct-recipes` and `--upstream none`. | Not available. |
| `--no-hints` | Disable state hints and the upfront placement fact. | Disable state hints, their inspect probes, and the placement fact in tool descriptions. |
| `--no-how` | Disable local command discovery. | Disable the local `how` tool. |
| `--log FILE` | JSONL rewrites, keyed by model. | Same format, keyed by `mcp`. |

Without `--log`, both use `rewrites.jsonl` in the platform data directory under
`dutchman-pilot` (the model face also supports `--profile-dir`). Logs stop at 64 MiB;
individual records are capped at 8 MiB. MCP's disabled local tools are neither
advertised nor intercepted; any upstream tools with those names pass through.
`--no-hints` leaves error corrections and truthful paging enabled.

## Command discovery

Try command discovery without a model or a running SolidCraft instance:

```sh
cargo run -- how "round all edges 3 mm"
cargo run -- how "hollow out the box"
cargo run -- --help
```

`how` searches the bundled snapshot of `solidcraft-cli 0.20 commands` in
`tests/fixtures/commands.json`. It leads with a concrete example when available
(otherwise `null`), followed by a separately labelled, non-executable typed
template, and trims trailing parameter docs to 400 characters. Placeholders such
as `<number mm>` and `<body id from model.bodies>` belong only to templates.
Examples with unresolved references or enum choices are omitted; an enum choice
is shown only when the task explicitly names it (for example, revolute).
Geometry selectors in examples must match the current design; SolidCraft still
validates the filled call. The catalogue loader also accepts live CLI JSON and MCP
`list_commands` results. Unsupported tasks return a no-command message and up to
three nearest alternatives.

Check with `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`.

## Corpus evaluation (no SolidCraft required)

`dutchman-pilot-eval` proposes commands to a model and scores its first response; it
never executes commands. Point it at an OpenAI-compatible `/v1` base URL, or at
Pilot for a comparison using the same model:

```sh
cargo run --bin dutchman-pilot-eval -- --base-url http://localhost:11434/v1 --model qwen2.5:7b
cargo run --bin dutchman-pilot-eval -- --base-url http://localhost:11500/v1 --model qwen2.5:7b --output eval-pilot
cargo run --bin dutchman-pilot-eval -- --rescore eval-results/results.json --output eval-rescored
```

`OPENAI_API_KEY` supplies optional authorization and is never written to disk.
The only network destination is the supplied base URL: redirects and environment
HTTP proxies are disabled. There are no provider retries or command executions.
The runner uses the bundled public command catalogue, independent of SolidCraft.
An `execute` function describes the command ids and their parameter docs; the
minimal system prompt contains the case's current screen. Each request starts a
fresh conversation. Temperature is 0.1 and seed is 42; providers may ignore these.

The starter [corpus](../corpus/starter.jsonl) has 150 independent JSONL cases: 40
create, 35 edit, 35 query, 25 workspace, and 15 refuse. Each line has a unique
`id`, English `utterance`, `state`, `bucket`, and one tagged `claim`:

```json
{"id":"cube","utterance":"Make a 20 mm cube.","state":"Empty design; display units mm.","bucket":"create","claim":{"kind":"command","id":"solid.box","args":{"length":20,"width":20,"height":20}}}
```

Other claims are `{"kind":"asks_which"}`, `{"kind":"answers_in_words"}`,
and `{"kind":"refuses"}`. Command args recursively match an object subset;
arrays must match in order and length. Numbers compare numerically; unit strings
are not converted by the judge. Claims deliberately choose a representation
(e.g. radius rather than diameter), so equivalent alternative arguments can fail.
Only one first-step call is accepted. Explicit text JSON calls are scored and
identified as text calls; prose is never converted into an action. Native
arguments must be strict JSON. Text calls accept the shared bounded extractor's
JSON5 subset. The prose judge is a versioned lexical check: clarification needs a
question mark plus “which”, “what name”, “specify”, or “clarify”; refusals need an
explicit inability phrase or the provider's refusal field. General answers must
be nonempty prose without those clarification/refusal signals. It does not
establish the factual correctness of general answers.

`--repetitions N` (or `-n N`) defaults to 3. A case is **pass** only when every
sample passes, **broken** when all fail, and **noise** when samples disagree.
HTTP 429, or 503 with Retry-After, is throttling and gets no verdict. Other
transport/provider failures also get no verdict; malformed successful responses
are broken. A case with any missing verdict is excluded from the rate and makes
the run incomplete. A fully judged bucket's pass rate is all-pass cases divided
by judged cases; its flakiness is noise cases divided by judged cases.

`--floors FILE` defaults to [corpus/floors.json](../corpus/floors.json); all five
buckets need a floor from 0 to 1. Missing buckets, incomplete cases, or rates below
a floor fail the run. Exit codes: 0 floors met, 1 floors missed/incomplete, 2
configuration or I/O failure. `--output DIR` receives `results.json` with every
raw response, its score, token usage, latency, and status, and `results.md` with
bucket and per-turn tables. Unknown usage or hand-back counts remain unknown.
Paths describe the response observed at the endpoint: Pilot may already have
converted a text call to native. Cached turns retain original tokens and latency;
these are recorded observations, not costs incurred by the current run. Offline
`--rescore` re-judges all recordings with the current judge and floors without
contacting an endpoint or touching the pass cache.

Passes are cached for 14 days in `.dutchman-pilot-eval-cache.json` (`--cache FILE`). A
stable index fingerprints the full request (prompt, state, tools, model and
utterance), endpoint, claim, repetitions, and judge version. The complete identity
is compared too, preventing hash collisions from reusing a pass. Expired entries
are pruned; failed live evaluations evict their old passes. `--refresh` forces
live evaluation and updates/evicts cache entries. `--no-cache` bypasses all cache
reads and writes. Cache replacement is atomic; concurrent runs should use
separate cache files. Cached replies are re-judged before reuse.

Limits: 512 cases, 8 KiB per corpus line, 2 MiB corpus, 1–10 repetitions,
1–600 seconds per request (`--timeout`, default 120), 1 MiB requests, 64 KiB
responses, 512 cache entries, and 128 MiB reports/cache files. Tests use an
in-process fake endpoint and synthetic HTTP responses so the harness checks run
even when the environment prohibits localhost sockets.

## Embedding

With the default features, `dutchman_pilot::cli::Command` derives `clap::Parser` and
can be nested in a clap subcommand enum as `Pilot(dutchman_pilot::cli::Command)`.
This exposes `dutchman pilot model`, `dutchman pilot mcp`, and the other Pilot
commands with the same flags as `dutchman-pilot`.

To show the parent's name in the top-level help, apply
`dutchman_pilot::cli::with_bin_name` to the nested command, for example
`Host::command().mut_subcommand("pilot", |c| dutchman_pilot::cli::with_bin_name(c, "dutchman pilot"))`,
and call `dutchman_pilot::cli::run_named(command, "dutchman pilot")` so the bare
command prints the same help. Subcommand help already uses clap's usage line.

Call `dutchman_pilot::cli::run(command)` to execute synchronously. It returns
`Result<i32, dutchman_pilot::cli::Error>`: success carries exit code `0`, and errors
implement `Display` and `std::error::Error`. The caller prints errors and chooses
its failure exit code. Proxy commands block until their server stops; `run`
never exits the calling process. Parsing handles help, version, and argument
errors through clap. The API requires `proxy` and is absent with only `core`.
