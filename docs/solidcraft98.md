# Pilot for SolidCraft 98

Design only; SPEC.md remains the contract. Pilot stays an external, opt-in Rust
adapter. SolidCraft 98 is a separate application with a screen-command protocol,
not SolidCraft's command catalogue or MCP server. No CAD implementation is copied.

## Evidence and compatibility

Read: `solidcraft98-ai/README.md`, `drive.py`, `tasks.json`, and the
`runs/m6/session.txt`, `runs/m8/session.txt`, and `runs/m9/session.txt` transcripts;
`solidvb6` reference `remote`, files `src/modRemote.bas`, `src/modTutor.bas`,
`freebasic/README.md`, and `freebasic/src/remote.bas`; and the separate
`solidcraft98-fb` README, docs listing, and input handling under `src/`.
Use only these authorized references; never inspect SolidCraft or GPL/LGPL CAD source.

The separate FreeBASIC checkout currently describes version 1.1.1, has
`build/sc98`, and does not recognize `--remote` in its input handling.
The `solidvb6` reference's FreeBASIC port documents version 1.2 and implements
`--remote`. Require a verified remote-capable executable before local benchmarks;
do not assume the existing executable has it.

## Transport and lifetime

Use one `RemoteSession` interface with TCP and child-process implementations:

- TCP connects to an explicitly configured address, port 9898 by default, with
  the VB application already running with `/REMOTE`. Pilot does not start a VM.
  `/REMOTE` binds loopback by default; reaching it elsewhere requires deliberate
  application configuration, such as `/REMOTE:9898:ANY`. There is no protocol auth.
- Process transport spawns an explicit executable with `--remote`, optionally
  `--headless`, using argument arrays, never a shell. Use a dedicated data
  directory, child-only home setting, piped stdin/stdout, and drained stderr.
  Keep MCP stdout exclusively for MCP messages.

Both transports send one command line, flush, and read one complete answer before
sending another. Answers terminate at a line containing exactly `.`; a line
beginning `..` loses one leading dot. Accept LF and CRLF. Decode Windows-1252 as
the driver does; reject unrepresentable outbound text instead of silently changing
names or values. Reject embedded CR, LF, and NUL in all command arguments.

Proposed defaults, enforced before allocation or sending: 4 KiB command, 8 KiB
answer line, 256 KiB answer, 4,096 answer lines, 512 parsed targets, and 256 bytes
per matched name. Oversize answers are errors, never partial successful screens.
Connection deadline: 3 seconds; each command's complete write/read deadline:
15 seconds, including `wait` and application settling. `wait.ms` is 0–5,000.
Reader workers use bounded channels and bounded buffers; TCP uses socket deadlines.
One worker per pipe and a single in-flight request avoid unbounded queued reads.

Allow two reconnect attempts with 250 ms and 500 ms delays, within the session
deadline. After EOF, timeout, or framing failure, discard the connection and cached
screen. A command whose write started has an unknown outcome: never replay it.
Reconnect TCP, obtain `look`, and return the uncertainty to the caller. Do not
automatically respawn a lost process into a fresh design; request an explicit restart.

Never send `quit` on completion, error, budget exhaustion, or disconnect. It exits
without saving. Do not advertise it to models. Only an explicit user shutdown
request may enable a separate control operation. Close TCP normally. Close child
stdin on detach: the remote-capable port exits when headless and resumes manual
use when graphical. Document this distinction. A hung Pilot-owned headless child
may be killed and reaped after a 2-second grace period; never kill an attached app.

## Faces and recommendation

**Start with the MCP tools face:** explicit schemas, existing clients, no model
loop, and no `drive.py` changes. This server exposes the remote protocol directly.
Use closed JSON schemas with no arbitrary command escape hatch:

| Tools | Typed arguments and wire mapping |
|---|---|
| `look`, `yes` | Empty object; same remote verb |
| `click`, `doubleclick`, `rightclick` | Exactly one of `name: string` or `point: {x: integer, y: integer}` |
| `drag` | Integer `x1`, `y1`, `x2`, `y2`, and `button: left\|right`; optional wire `right` |
| `menu` | `title: string`, optional `item: string`; `menu title [> item]` |
| `key` | `keys: string`; validate supported chords, at most 16 per call |
| `type` | `text: string`; no content repair; document the app's outer whitespace trimming |
| `wait` | `ms: integer`; bounded delay |
| `how` | `task: string`; `how task`, plus separately labeled Pilot help |

Coordinates must be nonnegative and inside the last reported screen bounds;
reject bounds above 16,384 per axis. Refresh absent bounds before coordinate input.
Reject `>` inside menu components. Tool results preserve the application's text
and add structured `screen`, `hint`, `suggested`, `rewrites`, and error fields.
Use MCP `isError` for application or adapter failures. A successful action is
followed by `look`, giving the caller current state and Tutor guidance.

The model face still helps: a client supplies these same chat-completions tool
schemas to `dutchman-pilot model`; Pilot repairs explicit JSON calls, applies
fallback prompts and guards, and returns calls for the client to execute via MCP.
The HTTP proxy never sends screen commands. Select a SolidCraft 98 dialect explicitly
so SolidCraft catalogue rewrites and plans cannot invent its features.

Later, `drive98` can own the read-screen/model/execute loop for text-command users.
It calls a configured OpenAI-compatible endpoint, optionally the model face, and
maps tools through the same session. A text compatibility mode accepts exactly
one `COMMAND:` line containing an allowlisted command, or one explicit tool JSON
object. `THOUGHT:` stays commentary; never scrape a last line as `drive.py` does.
Multiple commands, unknown verbs, and prose are returned as format errors.
`done <message>` ends the driver locally, obtains a final `look`, and never quits
the application. Existing `drive.py` calls Ollama `/api/chat` or Anthropic directly;
pointing its URL at `/v1` alone is insufficient. Adapt its client explicitly or
use the Rust driver; do not change the supplied Python code as part of Pilot.

## What Pilot adds

Cache the last complete `look`, with an observation number, active window,
dialog, focus, selection, menu, targets, timeline, Tutor line, and offer.
Refresh before an action when the cache is absent or invalidated. Restrict name
matching to the active context; never pick an underlying menu through a dialog.
Compare names by case, collapsed whitespace, mnemonic markers, terminal colons,
and ellipses. A unique normalized match can rewrite spelling to the displayed
name. Preserve numbers and literal `type` text. Ambiguous matches require choice.
For other misses, rank at most three closest names with bounded edit distance
and word overlap; return an unexecuted `suggested` call, not a speculative click.
For an unseen menu item, let the app validate the explicit path and then read it.
Never invent a target from a menu that has not been read.

If the app says “Did you mean”, preserve it as authoritative, suppress Pilot's
competing nearest-name guess, and expose its offered correction without rerunning.
Menu errors can open menus, so refresh after errors too. A suggestion is not
permission to execute. `yes` is a real action only when explicitly called and a
current application offer exists. Remember help/error offers ahead of Tutor
offers, as the app does; invalidate them after other actions or transport loss.
Do not issue an unrelated `how` while preparing `yes`, since it can replace its offer.

Turn the Tutor line into one short, attributed next-step hint, preserving its
“Say \"yes\" to do” offer. Never execute the hint. Missing or unfamiliar Tutor
text means no inferred action. `how` always passes through; Pilot supplements
it with a small independently written lookup indexed by words and synonyms,
returning explicit example calls and limitations, not SolidCraft catalogue ids.
Application help wins conflicts. Solid-edge fillets and body renaming are unavailable.
A sketch corner radius is a limited alternative; volume from timeline dimensions is a calculation,
not an application measurement. Mark incomplete read-outs as incomplete.

Repair only explicit call syntax: fences around a JSON object, stringified
containers against schemas, JSON5-style syntax, and known command aliases.
Never turn prose into an action or change dimensions to make a task succeed.
Suggest ambiguous corrections without executing them. Reuse the SPEC guard
allowance of at most two model hand-backs per turn and one empty-reply retry.

Bound a session to 40 action/model turns, 160 total remote commands (including
automatic looks), eight history turns, 1 MiB history, and 15 minutes by default.
Hard maxima are 200 turns, 800 commands, 4 MiB history, and 60 minutes.
Three identical action/error/state triples stop with a useful explanation;
compare focus, menu, dialog, and timeline state, ignoring the clock and FPS.
Count waits, reconnects, help calls, and refreshes against the relevant budgets.
For MCP, budgets belong to a connection; require an explicit new session to reset.

Reuse JSONL audit records: time, model, kind, before, and after; add session,
observation, and transport identifiers without credentials. Log every syntax,
name, prompt, result, and hint rewrite before dispatch or delivery. Audit failure
stops further rewritten actions. Retain the existing 64 MiB log cap, and report
rewrite counts and per-model native, text, repair, retry, and hand-back statistics.

## Code and CLI shape

Add `src/solidcraft98.rs` beside `mcp.rs` and `model.rs`, with `protocol`,
`screen`, `guidance`, `transport`, `mcp`, and later `drive` children. Extract
reusable guard-budget mechanics from `model/recovery.rs`; they are currently
private, not an existing shared API. Reuse `repair`, catalogue matching helpers,
`audit::Audit`, and `profile`; keep SolidCraft 98's help index separate.

Expose pure protocol parsing and matching under `core`; gate transport, MCP,
driver, audit, profiles, and CLI integration under `proxy`. Preserve
`--no-default-features --features core` on wasm32: no sockets, processes, threads,
HTTP dependencies, or filesystem calls enter that build. Keep unsafe forbidden,
the crate's panic-related lints intact, and clear `Result` errors throughout.

Proposed commands: `dutchman-pilot mcp98 --connect ADDRESS:9898` or
`dutchman-pilot mcp98 --program PATH [--headless]`; exactly one transport required.
Later: `dutchman-pilot drive98 --program PATH --upstream URL --model MODEL "task"`;
`--connect` is also available. Add `model --dialect solidcraft98` for schema-only
support and `how98 "task"` for Pilot's offline lookup. Retain `profile` and
`log-summary`; embedded spelling is `dutchman pilot mcp98` or `drive98`.

## Validation and measurement

Rust fake TCP servers and fake child processes test fragmented frames, CRLF,
dot stuffing, Windows-1252, EOF mid-answer, missing terminators, slow reads,
blocked writes, caps, stderr floods, reconnection, unknown outcomes, and no replay.
Test typed schemas, duplicate names, stale screens, dialog precedence, authoritative
corrections, offer lifetime, prose refusal, audit failure, and budget exhaustion.
Check native tests, clippy with `-D warnings`, formatting, and the wasm32 core build.

Later, run all 11 `solidcraft98-ai/tasks.json` tasks locally with a verified
FreeBASIC `--remote --headless` build, with and without Pilot. Use fresh isolated
data directories and identical checked box setups, models, prompts, settings,
seeds, and budgets; repeat each pair three times. Disable all Pilot additions
for the baseline while retaining identical framing and explicit-call rules.
Measure text-driver gains first, then native tools separately to avoid conflating
interface changes. Record final screens, claims, correctness, unsupported-task
honesty, actions, errors, rewrites, tokens, latency, and stops. Score from state
and claimed limitations, never `done` alone. ReactOS with VB TCP is an optional
final parity check. Run neither models nor the VM for this design.

## Questions for Ray

- Which remote-capable FreeBASIC executable should replace the older checkout for measurement?
- Is the MCP tools face first, with the text driver second, the desired order?
- Should benchmark success for unsupported requests require refusal or allow a stated partial alternative?
