# Measure it

Compare the same model and tasks against the direct endpoint and Pilot. Keep
model weights, context, temperature, seed, hardware, and task-set version fixed.
Record the exact commands, binary versions, and settings with the results.

## SolidCraft benchmark

The end-to-end benchmark exercises a design and can check actual geometry.
It requires a SolidCraft build that provides `bench-assistant`; the
0.20 CLI does not advertise this command. Check the
public help of your installed benchmark build before running:

```sh
solidcraft-cli bench-assistant --help
dutchman pilot model --upstream http://localhost:11434/v1 --log pilot-bench.jsonl
```

In another terminal, run the benchmark against each base URL:

```sh
solidcraft-cli bench-assistant --base-url http://localhost:11434/v1
solidcraft-cli bench-assistant --base-url http://127.0.0.1:11500/v1
```

Use that benchmark build's documented model and task-set selectors to repeat
both runs on v1 and v2. Run the same installed models in both arms; the contract
names gemma4:12b, qwen3:14b, qwen2.5:7b, gpt-oss:20b, and muse-glimmer. Model
availability and hardware capacity determine which can be measured locally.
Do not compare different task sets or count throttled turns as failures.
No benchmark results are claimed here.

Inspect Pilot's changes after each run:

```sh
dutchman pilot log-summary pilot-bench.jsonl --json
dutchman pilot profile --json
```

Use separate --profile-dir and --log paths for experiments. For ablations,
repeat with --no-plan, --no-how, or --no-hints; record any --prompt-help,
--fallback-prompt and --model-settings choices.

## dutchman-pilot-eval

The corpus runner measures the first proposed response without executing any
SolidCraft commands. It needs only a model endpoint. From the repository root
(or the extracted release directory containing corpus/):

```sh
dutchman-pilot-eval --base-url http://localhost:11434/v1 --model qwen2.5:7b --output eval-direct --no-cache
dutchman-pilot-eval --base-url http://127.0.0.1:11500/v1 --model qwen2.5:7b --output eval-pilot --no-cache
dutchman-pilot-eval --rescore eval-pilot/results.json --output eval-rescored
```

For a checkout, prefix the runner's arguments with
`cargo run --locked --bin dutchman-pilot-eval --`. OPENAI_API_KEY supplies optional
authorization without being written to a report. Reports still contain design
prompts and model replies; review them before sharing.

The starter corpus has 150 cases in create, edit, query, workspace, and refuse
buckets. Three repetitions are the default: all pass means pass, all fail means
broken, mixed means noise. A transport failure or throttling gives no verdict
and makes the run incomplete. Floors in corpus/floors.json must be met for every
bucket. Exit status is 0 for floors met, 1 for missed floors, or incomplete runs,
and 2 for configuration or I/O errors.

Each output directory receives results.json with recordings and results.md with
bucket rates, flakiness, per-turn paths, token usage, and latency where available.
Paths describe what the endpoint returned; a Pilot-converted call may appear
native. Prose claims use lexical rules, not factual verification; command claims
check one first-step call and a subset of arguments, not geometric equivalence.
This measure complements the end-to-end benchmark.

Passes normally use a 14-day cache keyed by the request, endpoint, claim,
repetitions and judge version. Use --no-cache for a fresh paired comparison,
--refresh to replace cached results, or --rescore for offline judging of existing
recordings. Limits include 512 cases, 1–10 repetitions, 1–600 seconds per request,
and 128 MiB report/cache files. See [flags](flags.md) and
[the evaluation details](details.md#corpus-evaluation-no-solidcraft-required).
