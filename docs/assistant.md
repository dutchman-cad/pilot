# Use Pilot with SolidCraft's Assistant

Start your model server, then Pilot, then add an API agent in SolidCraft's
Assistant. Pilot is the agent's base URL; the real model endpoint is Pilot's
upstream. Select the same model id that the upstream serves.

Pilot trims older tool results on the upstream copy of each request by default.
The latest user turn stays whole, and client-visible results and local recipe
readers retain the original history. While a recipe continuation or completion
check is pending, the upstream history also stays whole. Earlier catalogue and
command descriptions become command ids; identical tool/arguments/results keep
their latest full copy and leave a reference in earlier result envelopes.
Other results over `--trim-cap 1500` characters keep JSON keys, scalar fields and
the first/last two array entries, with an explicit recall note. Long strings and
deep nesting are bounded. This is a trigger threshold, not a strict summary size:
preserving keys and scalar fields can require more characters. `--no-trim`
disables history reduction. These flags also apply to `model --dialect solidcraft98`;
the separate `drive98` history limits are unchanged.

Each upstream attempt (including retries) logs a `dutchman-pilot` size field:
client system/tools/user/assistant content, results by tool name, Pilot facts,
hints and local schemas, and final upstream character sizes. These are Unicode
character counts, not tokenizer estimates; JSON totals include envelopes and
escaping. `profile` reports mean/max per attempt and whether trimming is enabled.
Facts already present in client history are reused; missing facts are inserted
once at the latest system position for that request. Pilot never relies on the
client retaining an upstream-only fact.

## Ollama

Pull a model that fits your machine and keep Ollama running. For example:

```sh
ollama pull qwen2.5:7b
dutchman pilot model --upstream http://localhost:11434/v1 --log rewrites.jsonl
```

In SolidCraft's Assistant, add an API agent with these values:

| Field | Value |
| --- | --- |
| Name | Pilot |
| Base URL | `http://127.0.0.1:11500/v1` |
| Model | `qwen2.5:7b` |
| API key | Leave empty if permitted; otherwise a placeholder for a local server that requires none. |

The local [Ollama compatibility endpoint](https://docs.ollama.com/api/openai-compatibility)
uses port 11434 and does not require an API key. Pilot detects Ollama's model
capabilities and enables its conditional fallback when needed. Native tools
remain available.

## LM Studio

Load a model in LM Studio and start its local server. Its
[chat-completions endpoint](https://lmstudio.ai/docs/developer/openai-compat)
can be used as Pilot's upstream:

```sh
dutchman pilot model --upstream http://localhost:1234/v1 --log rewrites.jsonl
```

Add an API agent with the same Pilot base URL, and the exact model id exposed by
LM Studio. If you enabled server authentication, put its key in the agent's API
key field; Pilot forwards the Authorization header in memory. It never saves
keys. Ollama-specific settings do not apply to LM Studio.

## Exact matches without a model

```sh
dutchman pilot model --upstream none --log rewrites.jsonl
```

Add an API agent at `http://127.0.0.1:11500/v1` with model `pilot-recipes`.
Exact matches are on by default. To retain model recovery and other chat, set your
upstream URL. This is recommended for 3–8B models; plan-text recipes remain off by
default. `--no-direct-recipes` sends everything to the model; `--direct-recipes`
is an accepted no-op alias and conflicts with the opt-out. `--upstream none`
rejects the opt-out. Direct answering stays off for `--dialect solidcraft98`.

Keep SolidCraft's tool permission set to **Ask**. Exact matches appear as normal
native `execute` / `batch` tool proposals in its approval UI, with the command
arguments available for review. Pilot does not execute them or bypass approval;
approve them in SolidCraft to run the tools and return results. A declined or
failed tool cannot produce a success claim. The measured final answer starts
`Exact match (no model):` and the response identifies its source as `pilot-recipes`.
Missing results, tool errors, and measurement mismatches go to the configured
model for recovery, or produce a plain failure message when none is configured.
Other no-model requests receive a fixed description of supported recipe kinds.
See [recipes](recipes.md) for required slots, limits and audit counters.

## Check a turn

Ask for a simple dimensioned part, such as “make a 20 mm cube”. Pilot can suggest
a typed recipe plan to the model with `--planner` and repairs explicit JSON calls
by default. With `--planner`, a plain action pledge matching the user recipe's intent and slots
can also become ordinary tool calls. SolidCraft applies its own validation and
the model receives the results before replying. One completion check compares
measured state with the recipe. Plan-text recipe prompts, prose conversion, continuations,
and completion checks are off unless `--planner` is passed. `--no-plan` (alias
`--no-planner`) is accepted as a no-op alias for the default. Argument repair,
text-call pickup, command lookup, hints, and text tool mode stay enabled.
Default direct answering also works with `--planner`; exact matches take precedence.
See [recipes](recipes.md).

Rewrites go to the selected JSONL file. Without --log, the log and profiles live
in the platform data directory under dutchman-pilot. Summarize them with:

```sh
dutchman pilot profile qwen2.5:7b
dutchman pilot log-summary rewrites.jsonl
```

Streaming replies are buffered for one assistant turn. Empty turns get one retry;
guards get at most two nudges. Compare default direct answering with --no-direct-recipes;
test --planner with --no-direct-recipes separately for paired measurements, and
use --no-how or --no-hints to disable those additions.
Use --prompt-help for extra bounded command guidance. See the
[flags](flags.md), [behavior](details.md), and [security note](../SECURITY.md).
Keep the listener on loopback and do not expose it to a network.
