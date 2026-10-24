# Flags reference

Generated from the CLI help. Regenerate with `cargo run --locked --example generate_flags`.
The test suite compares this page with both binaries' help output.

## dutchman-pilot

```text
Pilot — a local helper for SolidCraft

Usage:
  dutchman-pilot how "<task>"
  dutchman-pilot model [--listen 127.0.0.1:11500] [--upstream URL|none] [--direct-recipes]
                       [--upstream-timeout SECONDS (default 300, max 3600)]
                       [--fallback-prompt auto|on|off] [--prompt-help] [--log FILE]
                       [--profile-dir DIR] [--model-settings [FILE]] [--seed INTEGER]
                       [--no-report-buffer]
                       [--no-how] [--no-hints] [--planner] [--no-plan (alias --no-planner)]
  dutchman-pilot report [--listen ADDR] [--turn N] [--include-request] [--out FILE]
                        [--from-request FILE] [--open]
  dutchman-pilot profile [model] [--profile-dir DIR] [--json]
  dutchman-pilot log-summary <rewrites.jsonl> [--json]
  dutchman-pilot mcp [--solidcraft-cli PATH] [--log FILE]
                     [--no-plan] [--no-hints] [--no-how] -- <args for solidcraft-cli mcp>
  dutchman-pilot --version (-V)
  dutchman-pilot --help (-h)

how uses the bundled public command catalogue and prints explicit JSON example calls.
model serves /v1/chat/completions and /v1/models; mcp starts the SolidCraft tools proxy.
--planner enables recipe suggestions, matching action prose, and one completion check.
These plan-text features are off by default; model --no-plan is a no-op alias for
the default. The MCP face only suggests plans.
--direct-recipes answers exact matches without a model; it can accompany --planner.
Upstream defaults to Ollama; --upstream none (with --direct-recipes) runs without
a model. Direct replies are marked, audited, and profiled separately.
report prints a redacted local file and issue URL; it never submits anything.
--include-request opts in to request words; --open opens the browser for review.
The model face keeps five diagnostic turns in memory; --no-report-buffer disables it.
Profiles and the default rewrites.jsonl log live in the user's data dir/dutchman-pilot.
--prompt-help adds short rules and at most 15 relevant command purposes.
--model-settings enables Ollama-only temperature 0.1 with tools and num_ctx 8192
when absent; --seed requires --model-settings. All additions are logged.
Optional settings file: JSON keyed by model, with temperature (0–0.2), seed,
num_ctx (8192–131072), and json_retry (boolean, decided JSON retries only).
```

Flags (including aliases):

- `--direct-recipes`
- `--fallback-prompt`
- `--from-request`
- `--help`
- `--include-request`
- `--json`
- `--listen`
- `--log`
- `--model-settings`
- `--no-hints`
- `--no-how`
- `--no-plan`
- `--no-planner`
- `--no-report-buffer`
- `--open`
- `--out`
- `--planner`
- `--profile-dir`
- `--prompt-help`
- `--seed`
- `--solidcraft-cli`
- `--turn`
- `--upstream`
- `--upstream-timeout`
- `--version`
- `-V`
- `-h`

## dutchman-pilot-eval

```text
Pilot — corpus evaluation

dutchman-pilot-eval --base-url URL/v1 --model MODEL [--corpus corpus/starter.jsonl] [--floors corpus/floors.json] [--repetitions N (alias -n; default 3, range 1–10)] [--timeout SECONDS (default 120, range 1–600)] [--output eval-results] [--cache FILE] [--no-cache | --refresh]
dutchman-pilot-eval --rescore results.json [--floors FILE] [--output DIR]
dutchman-pilot-eval --help (-h)
OPENAI_API_KEY supplies optional authorization; it is never recorded.
```

Flags (including aliases):

- `--base-url`
- `--cache`
- `--corpus`
- `--floors`
- `--help`
- `--model`
- `--no-cache`
- `--output`
- `--refresh`
- `--repetitions`
- `--rescore`
- `--timeout`
- `-h`
- `-n`
