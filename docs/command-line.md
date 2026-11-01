# Command-line options

Pilot has two faces. Pick the one that fits your client:

| Client | Start | Connect |
| --- | --- | --- |
| SolidCraft's Assistant | `dutchman pilot model --upstream http://localhost:11434/v1` | Add an API agent at `http://127.0.0.1:11500/v1`. |
| An MCP client | `dutchman pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --in part.solidcraft` | Configure a local stdio server with this command. |

Pilot does not run a model or embed SolidCraft. Install SolidCraft separately, and
use Ollama, LM Studio, or another chat-completions endpoint for the model face.
Every flag is listed in the [flags reference](flags.md).

## What the model face does

- Recipe forms: Pilot finds the recipe that fits a request and asks the model only to
  fill in its typed form, in a short separate request, then builds the part with checked
  calls. See [recipes and their limits](recipes.md).
- Exact matches: a request Pilot can read completely is answered with native tool
  proposals and measured replies, without a model. Their replies start
  **"Exact match (no model):"**, and the proposed calls still go through SolidCraft's
  approval and validation. `--no-direct-recipes` sends everything to the model, and
  `--upstream none` runs recipes alone, with no model.
- Fewer input tokens: older tool results are trimmed inside long tasks (`--no-trim` to
  keep them), and Pilot adds no unused tool schemas.
- Millimetres, metric and inch units, fractions, and deterministic expansions.
- Argument repair, text-call pickup, command lookup, hints, buffered streaming,
  and bounded retries are on by default, with audited rewrites.
- Text tool mode for models without native tools, with automatic retry and saved profiles.
- Opt-in `--planner`: recipe suggestions, matching prose conversion, continuations,
  and one measured completion check. These plan-text features are off by default
  because they reduced reliability with small models; `--no-plan` (alias
  `--no-planner`) is accepted as a no-op.
- Local, redacted [bug reports](report.md) from five memory-only turns or offline
  planner replay: type `/report` in the Assistant. Chat reports omit request words;
  the CLI can include them with `--include-request`. You review and submit.

See [behavior and limits](details.md) for the details of each.

## MCP clients

Configure a local stdio server:

```json
{
  "mcpServers": {
    "solidcraft": {
      "command": "/path/to/dutchman-pilot",
      "args": ["mcp", "--solidcraft-cli", "/path/to/solidcraft-cli", "--", "--in", "/path/to/part.solidcraft"]
    }
  }
}
```

Use absolute paths for the installed binaries and design file. See the
[MCP guide](mcp.md) for client-specific setup and live connections.

## SolidCraft 98

Start `dutchman pilot mcp98 --program /path/to/sc98 --headless` as an MCP server, or
attach with `--connect 127.0.0.1:9898` to an application started with `/REMOTE`
(`/REMOTE:ANY` when Pilot runs on another machine or a virtual machine's host). See the
[SolidCraft 98 quick start](solidcraft98.md) for the model driver and limits.
