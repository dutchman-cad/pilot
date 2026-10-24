# SolidCraft 98

Pilot supports [SolidCraft 98](https://github.com/raykholo/solidcraft98), a separate
application from SolidCraft. **SolidCraft 98 v1.1.1 or later is required** for the
remote protocol used here. Install the application separately. Pilot uses its
public screen commands; it does not embed a CAD engine.

## Connect

For an application already listening on TCP port 9898, configure your MCP client
to launch this stdio server:

```sh
dutchman-pilot mcp98 --connect 127.0.0.1:9898
```

Start the Visual Basic application with `/REMOTE` to enable remote control.
The TCP address must be numeric, including the port; IPv6 addresses use brackets.
Keep the unauthenticated remote listener on a trusted network.

For the FreeBASIC Linux build, Pilot can start and own a headless process:

```sh
dutchman-pilot mcp98 --program /path/to/sc98 --headless
```

Use exactly one of `--connect` and `--program`. `--headless` requires `--program`.
The process receives `--remote`, an isolated working directory, and its own data
directory under the system temporary directory. Saved designs remain there after
the session. Closing Pilot closes the child's input; a headless child exits, and
a graphical child returns to manual use. Pilot never sends `quit`. A hung owned
headless child is killed and reaped after two seconds; attached applications are
never killed.

## Tools and model driver

The MCP tools are `look`, `click`, `doubleclick`, `rightclick`, `drag`, `menu`,
`key`, `type`, `wait`, `how`, and `yes`. Calls have closed schemas. Read the screen
before choosing a target or changing fields. `type` preserves literal text;
the application's command reader trims outer whitespace. Coordinates must be
inside the last reported screen bounds. `yes` requires a current application offer.

Pilot resolves unique spelling variations of displayed targets, preserves
application corrections, and returns suggestions for ambiguous or missing targets.
It refreshes the screen after actions and errors, attributes Tutor hints to the
application, and supplements `how` with a small offline lookup. Suggestions and
Tutor offers execute only when explicitly called.

For a complete read-screen, model, and action loop:

```sh
dutchman-pilot drive98 --program /path/to/sc98 --headless \
  --upstream http://127.0.0.1:11434/v1 --model MODEL 'Make a 20 mm cube.'
dutchman-pilot how98 'drill a hole'
```

The driver accepts one native tool call, one explicit tool JSON object, or one
`COMMAND:` line per reply. Optional `THOUGHT:` text is commentary. `COMMAND: done
<answer>` finishes locally and reads the final screen. Prose alone never acts.
`--plain` disables Pilot's help, repairs, target matching, and hints for comparisons;
framing, schema validation, and budgets still apply.

An existing chat-completions client can use the model proxy and execute returned
calls through the MCP face:

```sh
dutchman-pilot model --dialect solidcraft98 \
  --upstream http://127.0.0.1:11434/v1
```

This dialect supplies only SolidCraft 98 screen tools. SolidCraft catalogue
recipes, `--direct-recipes`, `--planner`, and catalogue hints are disabled.
Beta.2's plan-text features remain opt-in for the SolidCraft dialect. The HTTP
proxy returns calls for the client to execute; it does not send screen commands.
The commands also work under `dutchman pilot`.

## What works and limits

Rectangle and circle sketches, extrusions, through holes, source-sketch position
edits, and feature parameter edits are supported through application dialogs.
Solid-edge fillets and body renaming are unavailable. Sketch corner rounding is a
limited alternative that must be described as such. A volume computed from
sketch and extrusion dimensions is a calculation, not an application measurement.
The [benchmark set](../bench/sets/sc98/README.md) checks these distinctions.

Commands use Windows-1252. Unrepresentable text, embedded newlines, NUL, oversized
frames, and invalid schemas return errors. Limits include 4 KiB per command,
8 KiB per answer line, 256 KiB per answer, 4,096 answer lines, 512 targets, and
256 bytes per matched name. Screen bounds are at most 16,384 per axis, and waits
are at most five seconds. MCP input messages are limited to 64 KiB.

An MCP session permits 40 action/help attempts, 160 command/connect attempts,
and 15 minutes. The driver defaults to 40 model turns, eight history turns,
1 MiB of history, 131,072 total tokens, and 15 minutes. `--budget` accepts 1–200
turns, with at most 800 remote commands. Three identical action/error/state
results stop the loop. Empty replies have one retry; error hand-backs are bounded.
TCP connection setup has a three-second deadline and at most two retries;
commands share a fifteen-second write/read deadline. A lost connection discards
cached state. An uncertain action is never replayed; a lost child is never
silently restarted into an empty design.

Rewrites and driver transcripts use Pilot's JSONL audit log, capped at 64 MiB.
Use `--log FILE` to choose a location. Audit failure stops rewritten actions.
Transcripts can include prompts, names, paths, and design values; review them
before sharing. `log-summary` summarizes the records. The model proxy also uses
beta.2's memory-only report buffer; `--no-report-buffer` disables it.
