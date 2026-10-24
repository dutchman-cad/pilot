# Use Pilot with SolidCraft's Assistant

Start your model server, then Pilot, then add an API agent in SolidCraft's
Assistant. Pilot is the agent's base URL; the real model endpoint is Pilot's
upstream. Select the same model id that the upstream serves.

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

## Check a turn

Ask for a simple dimensioned part, such as “make a 20 mm cube”. Pilot can suggest
a deterministic plan to the model and repair explicit JSON calls in its reply;
SolidCraft receives ordinary tool calls and applies its own validation. A reply
that only describes an action does not execute it.

Rewrites go to the selected JSONL file. Without --log, the log and profiles live
in the platform data directory under dutchman-pilot. Summarize them with:

```sh
dutchman pilot profile qwen2.5:7b
dutchman pilot log-summary rewrites.jsonl
```

Streaming replies are buffered for one assistant turn. Empty turns get one retry;
guards get at most two nudges. Use --no-plan, --no-how, and --no-hints for paired
measurements, or --prompt-help for extra bounded command guidance. See the
[flags](flags.md), [behavior](details.md), and [security note](../SECURITY.md).
Keep the listener on loopback and do not expose it to a network.
