# Use Pilot with SolidCraft's Assistant

Start your model server, then Pilot, then add an API agent in SolidCraft's
Assistant. Pilot is the agent's base URL; the real model endpoint is Pilot's
upstream. Select the same model id that the upstream serves.

Pilot trims older tool results on the upstream copy of each request by default.
The newest two tool results stay whole (`--trim-keep N`, range 0–4096), across
the whole history including the current user turn. The trailing result block
the model is answering always stays whole, including parallel calls and with
`--trim-keep 0`. Client-visible results and local recipe readers retain the
original history. While a recipe, fill, continuation or completion check is
pending, the upstream history also stays whole. Earlier catalogue results become
command ids; command descriptions become ids plus one-line parameter docs
(at most 400 characters). For catalogue results already truncated by the client,
Pilot retains only complete command fields and marks the source incomplete.
Identical tool/arguments/results outside the kept
window leave a reference to the latest occurrence in their result envelopes.
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
escaping. Each request includes `trimmed_chars`, the serialized characters saved
by history trimming before other wire transformations. `profile` reports mean/max
per attempt, whether trimming is enabled, and the configured kept-result window.
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
When the model filled in the recipe's form, the measured answer starts
`Built from the model's form:` instead.
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

Normal model requests omit local `pilot_find` and `pilot_build` schemas. Use
`--offer-recipe-tools` for models that call these tools. Forced fill and route
keep their small replacement tool lists without that flag; `--no-shortcuts`
disables all four paths, and `--no-fill` disables fill and route only.

Facts and hints are offered once per conversation, matched by the client's
retained history (bounded to 256 conversations), and existing facts still deduplicate.
Repeated next-step hints keep at most 256 identities per conversation; further
new next-step hints are omitted at that cap.
Relevant facts and hints always send, regardless of the client's character count;
they are never deferred to meet a size budget. `how` remains available unless
disabled with `--no-how`. Normal requests add 303 fixed characters for `how`
(including its tool-list separator), down from 5,487 characters for the previous
schemas and find-first message. Facts and hints add context-dependent characters
once; explicit prompt modes and recovery retries can add further instructions.
The `how` schema is 302 characters on the model wire, down from 366.
MCP uses its separate tool-list path: it continues advertising `how` and putting
facts in tool descriptions; this model flag and conversation tracking do not apply.
