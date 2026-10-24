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

## First MCP implementation (2026-10-23)

The MCP tools face is implemented on the private branch. `SPEC.md` remains the
contract. `drive98`, `how98`, and the model-face dialect are deferred. No model,
VM, benchmark loop, or SolidCraft catalogue is involved in this face.

```
dutchman-pilot mcp98 --connect 127.0.0.1:9898 --log /path/to/audit.jsonl
dutchman-pilot mcp98 --program /absolute/path/to/sc98 --headless --log /path/to/audit.jsonl
dutchman-pilot mcp98 --help
```

Exactly one transport is required; `--headless` requires `--program`. TCP currently
requires a numeric socket address (IPv4 or bracketed IPv6), including its port.
The same clap command can be embedded as `dutchman pilot mcp98`. The existing
public help reference is unchanged; this private face has its own subcommand help.
MCP uses newline-delimited JSON-RPC on stdin/stdout, with a 64 KiB input-message
cap. It advertises only the eleven tools in the table above, with closed schemas.
Known double-click/right-click aliases and schema-directed stringified JSON
containers are repaired and audited; prose is rejected. Literal `type.text` is
never repaired. The application's command reader trims outer whitespace.

Core modules implement Windows-1252, framing, screen parsing, normalized and
ambiguous target matching, bounded nearest-target ranking, and an independently
written Pilot help lookup. Proxy modules implement transports and MCP. Every
input action refreshes the screen first, and every application action/help/error
is followed by a `look`; these observations count against the command budget.
Display names are used only for unique normalized matches in the active context.
Unseen menu paths go to the application for validation. Unknown click targets
return at most three unexecuted suggestions. Application corrections remain
authoritative. Help/error offers take priority over Tutor offers, survive looks,
and are invalidated by other actions or transport loss. `yes` never invokes `how`.
Results retain application text and supply structured screen, hint, offer,
suggestions, rewrite count, and errors. Tutor hints are attributed to the app.
Unknown help topics receive no inferred plan. Solid-edge fillets and body
renaming are explicitly unavailable; calculated volume is not a measurement.

Limits are fixed defaults in this cut: 40 action/help attempts (including invalid
calls), 160 command/connect attempts, and 15 minutes per MCP connection. Three
identical action/error/state results stop the session; clock/FPS are excluded.
There is no model history or model retry loop in this face. A new MCP connection
is the explicit reset. Frame, target, coordinate, name, key and wait caps are
those specified above. TCP initial connection/reconnection uses a three-second
deadline and at most two retries (250/500 ms). Write and read share a fifteen-second
command deadline, with one bounded worker/channel per pipe; stderr is continuously
drained with a fixed buffer. After transport failure, cached state is discarded.
TCP attempts a fresh `look`, reports the uncertain outcome, and never resends the
failed command. Lost child processes require a new session and are never respawned.

The process runs with `--remote`, an isolated working directory and child-only
HOME/USERPROFILE under the system temporary directory; its Documents directory
is created there. These directories are retained so saved designs are not removed.
No path is passed through a shell. Closing MCP closes child stdin: headless mode
exits; graphical mode resumes manual operation. A hung owned headless process
gets two seconds to exit before it is killed and reaped. Attached TCP applications
are never killed. Pilot never sends or advertises `quit`.

The existing default JSONL log location and 64 MiB cap are reused. Records include
time, model (`mcp98`), kind, before/after, session, observation and transport.
Syntax/name changes are logged before dispatch; native calls and result/hint
additions are also recorded before delivery. Audit failure prevents further
calls in that session. `log-summary` can summarize these records. There is no
per-model profiling in the MCP face because it does not run a model.

### FreeBASIC build evidence

Source: a temporary `git archive remote freebasic` export from
`/home/ray/Documents/GitHub/solidvb6`, revision `4292235`; the checkout was not
modified. Its FreeBASIC README documents version 1.2 and `--remote`. The documented
`sc98-fb-x86` Podman build could not start in this sandbox: Podman's stored runtime
root `/run/user/1000/containers` is read-only; overriding it caused a database
configuration mismatch. A native fallback successfully built the unchanged
export with the FreeBASIC 1.10.1 compiler archive already present under
`~/vms/fb/img-x86/`:

```
# Export the authorized FreeBASIC port into a writable temporary directory.
git -C /home/ray/Documents/GitHub/solidvb6 archive remote freebasic \
  | tar -x -C /tmp/sc98-pilot-build
# Extract ~/vms/fb/img-x86/FreeBASIC-1.10.1-ubuntu-22.04-x86_64.tar.xz there.
# In /tmp/sc98-pilot-build/lib, provide linker symlinks to the host's
# libX11.so.6, libXext.so.6, libXpm.so.4, libXrandr.so.2, libXrender.so.1,
# libncurses.so.6 and libtinfo.so.6, named lib<name>.so.
CCACHE_DISABLE=1 /tmp/sc98-pilot-build/FreeBASIC-1.10.1-ubuntu-22.04-x86_64/bin/fbc \
  -w all -exx -p /tmp/sc98-pilot-build/lib -l tinfo \
  /tmp/sc98-pilot-build/freebasic/src/sc98.bas -x /tmp/sc98-pilot-build/sc98
```

The compiler emitted existing mixed-boolean warnings and linked successfully.
The executable is installed at
`/home/ray/Documents/GitHub/sc98-remote-bin/sc98`. Note that even `--help` opens
its window and waits, so probe it only through `mcp98`. The ignored integration
test passes with this binary:

```
SC98_PROGRAM=/home/ray/Documents/GitHub/sc98-remote-bin/sc98 cargo test \
  --test solidcraft98 real_freebasic_task_one_through_mcp98 -- --ignored
```

It launches the actual `mcp98` binary, reads `look`, uses menus to create a part
and rectangle sketch, enters 20 by 20 mm, extrudes 20 mm (tasks.json task 1), and
asserts both dimensions and Extrude1 in the final timeline. It uses a fresh
isolated data directory, sends no shutdown command, and detaches through EOF.

### Validation status

Rust tests cover framing/caps/encoding, fake pipe and TCP peers, fragmented CRLF
and dot stuffing, EOF/no replay, missing terminators and deadlines, blocked write
deadlines, stderr floods, typed schemas, dialog precedence, duplicate and stale
targets, authoritative corrections, offers, explicit syntax repair and prose
refusal, audit failure, repeated-state stops and session budgets. The real-build
test is ignored by default and requires `SC98_PROGRAM`.

Both feature sets pass clippy with `-D warnings`; formatting and the wasm32 core
build pass. All suites pass, including the twelve model-proxy integration tests,
the fake-TCP test, and the opt-in real FreeBASIC test.

## Driver, offline help, dialect and paired harness (2026-10-23)

The next private step implements `drive98`, `how98`, and the explicit model
proxy dialect. The historical sections above describe the original design and
MCP milestone. `SPEC.md` remains the contract. No model has been run. No
SolidCraft or GPL/LGPL CAD source was read for this implementation. The external
`solidcraft98-ai` README, driver and eleven task definitions supplied the task
set and setup sequence; application facts were checked with the public remote
binary. The private commands have their own clap help, like `mcp98`; the existing
public top-level help/reference remains unchanged.

```
dutchman-pilot drive98 --program /absolute/path/to/sc98 --headless \
  --upstream http://127.0.0.1:11435/v1 --model MODEL \
  --log /tmp/pilot-drive98.jsonl --budget 40 "Make a 20 mm cube."
dutchman-pilot drive98 --connect 127.0.0.1:9898 \
  --upstream http://127.0.0.1:11435/v1 --model MODEL --plain "Make a 20 mm cube."
dutchman-pilot how98 "move a box along X"
dutchman-pilot model --dialect solidcraft98 --upstream http://127.0.0.1:11435/v1
dutchman-pilot drive98 --help
dutchman-pilot model --help
```

`drive98` calls the MCP face's `Server` session directly, including its transport,
validation, observations, offers, typed commands and audit. There is no second
remote-command implementation in Rust. Both modes send the same eleven native
schemas, common prompt skeleton, task, framing and model settings (temperature
0.1, seed 98, at most 512 output tokens). Both accept one native tool call, one
complete explicit JSON tool object, or exactly one `COMMAND:` line with optional
preceding single-line `THOUGHT:` commentary. Native tool results retain their
call ids in history. A native call cannot be mixed with an explicit text call.
Text JSON supports the four explicit shapes in SPEC.md; surrounding prose,
multiple objects/commands, and unknown verbs are errors, never inferred actions.
`done <answer>` is a COMMAND verb local to the driver, not an application tool.
It obtains a final `look`, logs the summary, and detaches without sending `quit`.
The existing headless-process EOF and graphical-detach behavior still applies.

Default mode supplements the first prompt and error feedback with independently
written Pilot help, and enables the existing session's syntax/alias/name repairs,
unexecuted suggestions and attributed Tutor hints. Offline help now covers
movement and plate/hole sequences as well as sketches, extrusion and limitations.
It never changes dimensions or replaces an application's pending offer. Plain
mode disables all of these additions, including relaxed JSON and fenced-object
repair. The application can still supply its own Tutor/help/corrections in the
raw screen; those are the same for both modes. Schema validation, coordinate
bounds, explicit-call rules, safety checks and format-error feedback are common.
Every added prompt/help, conversion, syntax/name rewrite, suggestion and result
hint is audited before dispatch/delivery. The transcript shares the capped JSONL
log, with request, response, action/result and summary records, model, session,
observation and transport. A failed audit prevents further actions.

`--budget` counts all model requests, including retries and `done`: default 40,
range 1–200. Wall time is capped at 900 seconds, each upstream request at the
remaining wall time or 120 seconds, whichever is less. The remote session shares
the driver's wall deadline. There are at most two error hand-backs and one empty
retry before a successful action resets that turn's allowance; all consume steps.
Upstream failures stop without replaying an action. Total reported tokens are
capped at 131072; absent usage, input/output byte counts are a conservative
fallback. Requests reserve an input-byte upper bound (including tool schemas)
and 512 output tokens before dispatch. History holds eight complete turns and
at most 1 MiB, dropping oldest complete groups; message content is capped at
1 MiB before dispatch. Responses are capped at 1 MiB with existing JSON
nesting/node limits. Remote commands/reconnects are capped at
`clamp(4 * budget + 2, 160, 800)`, including initial/final observations. Three
identical action/error/semantic-state results stop; commentary, observation
numbers, FPS and clock do not distinguish states. A final observation is always
attempted; failure is explicit and an old screen is not presented as final.
The stdout JSON includes task/model/plain, steps/actions/errors/rewrites,
tokens, elapsed seconds (`latency`), stop reason, done answer, final screen,
error, and native/text/repair/retry/hand-back counts. Logs can be grouped by model
and session for statistics; this driver does not modify the model proxy's profile.

`model --dialect solidcraft98` replaces advertised tools with the eleven screen
schemas and uses an empty SolidCraft catalogue. It disables catalogue plans,
local catalogue `how`, catalogue result hints and catalogue prompt rails even
if their ordinary options were supplied. It retains explicit JSON conversion,
known click aliases, schema-directed argument repair, conditional fallback,
empty-reply retry, streaming buffering, auditing and existing bounded guards.
The HTTP proxy executes no application commands; screen `how` goes back to its
MCP client. SolidCraft command ids cannot become `execute` calls in this dialect.
Default `model` behavior is unchanged.

### Paired benchmark (Python 3 standard library only)

The private `bench98/` scripts run the eleven external task definitions with
fresh isolated HOME/USERPROFILE, working directory and Documents per run. The
only application invocation is `--remote --headless`. A loopback bridge exposes
the same already-set-up process to `drive98 --connect`; no model performs setup.
Tasks 6–9 and 11 receive their checked rectangle/extrusion box. Each setup is
verified from timeline and feature-dialog values before the model is contacted.
The harness holds `/tmp/solidcraft-pilot-c.lock` with `flock` for its whole matrix,
including setup and scoring; lock waits stop after 900 seconds. Runs are serial,
A/P order alternates across reps, and directories must be new. Each driver run
has a 930-second outer timeout. Limits: 32 models, 20 reps, 200 steps per run,
1000 setup/driver/probe commands per process, and bounded frames and files.

```
python3 bench98/run.py --pilot "$CARGO_TARGET_DIR/debug/dutchman-pilot" \
  --program /home/ray/Documents/GitHub/sc98-remote-bin/sc98 \
  --upstream http://127.0.0.1:11435/v1 \
  --models MODEL1 MODEL2 --reps 3 --out /tmp/solidcraft98-paired
python3 bench98/report.py /tmp/solidcraft98-paired
python3 bench98/report.py /tmp/solidcraft98-paired --rescore
```

`checks.json` records expected dimensions, origins, hole diameters/depths/centres,
body evidence and unsupported-task policy. Scoring reads the final state
independently after the driver disconnects. It probes each feature with
`doubleclick`, `look`, `key esc`, recording the fields and cancelling every
inspection. This matters because the timeline omits hole locations and corner
radii. All feature statuses must be OK and dialogs committed. The screen does
not expose a measured body count: one body is conservatively evidenced by
exactly one solid-producing extrusion, its sketch, and only the requested hole
features; extra solid features fail. Rectangle/circle dimensions, XY ground
plane, source origin and hole attachment are checked. This is screen-state
validation, not a geometric kernel or a body/volume measurement.

Task 6 accepts an honest solid-edge rounding refusal with the unchanged box, or
an explicitly limited 3 mm sketch-corner alternative. Task 8 accepts honest body
rename refusal or a stated part/file alternative, with preserved geometry.
Task 9 requires the preserved 30 x 20 x 10 box and a calculated 6000 mm3 (6 cm3)
answer; a measurement claim fails. Task 11 requires a change in timeline geometry
or numeric feature fields, including deletion to a verified empty timeline;
view, selection, name, clock and uncommitted-dialog changes do not pass. No task
passes from a `done` claim alone. Infrastructure errors remain visible failures.
The offline report prints per-model A/P pass counts, percentage-point difference,
tokens, latency, errors and rewrites plus per-task counts and failure reasons.
Recorded initial/final screens, probes, checks, summaries and transcripts permit
later rescoring without a model.

### Tests and execution constraints

Rust tests include a local HTTP fake upstream and the Rust fake pipe peer from
`tests/support`. They cover strict text/native formats, format hand-backs,
completion, budgets, repeat guards, plain/default differences and audit records.
The same loop has a completion-client embedding entry point; deterministic
replay tests exercise it with the real pipe session without requiring sockets.
The dialect is tested with the existing in-process scripted upstream as well
as an HTTP integration case. The original tests remain in place.

```
cargo test --test drive98
SC98_PROGRAM=/home/ray/Documents/GitHub/sc98-remote-bin/sc98 \
  cargo test --test drive98 real_freebasic_task_one_through_drive98 -- --ignored
python3 -m unittest discover -s bench98 -v
SC98_PROGRAM=/home/ray/Documents/GitHub/sc98-remote-bin/sc98 \
PILOT_BIN="$CARGO_TARGET_DIR/debug/dutchman-pilot" \
  python3 -m unittest discover -s bench98 -v
```

`fake_upstream.py` is a tiny local HTTP replay server; its default script builds
the task-1 cube. The full harness integration test runs it through the actual
Rust CLI and actual FreeBASIC process in both modes and requires a passing state
score, not only a completion message. `PILOT_BIN` and `SC98_PROGRAM` enable it.
There is also a real-build setup/probe/scorer test using pipes alone. Neither
script contacts a model during tests. The fake peer can be run independently
with `python3 bench98/fake_upstream.py`, or supplied a bounded JSON replies file.

This implementation session's sandbox denies localhost sockets (`Operation not
permitted`), affecting existing HTTP/TCP tests too. The real benchmark lock is
also held externally. These constraints do not justify bypassing the lock or
claiming HTTP/full-harness validation passed. Run the commands above on the
unrestricted benchmark machine before measuring models. Git index creation is
read-only in this workspace, so local commits may need to be made there as Ray.

Validation recorded for this step: the 64 Rust library tests, six CLI tests,
two core integration tests, four socket-free driver tests, nine socket-free
SolidCraft 98 tests, existing MCP/eval/profile tests, and the core-only suite
pass. Six Python scorer, report, infrastructure-error and output/timeout-cap tests
pass (two real-build integrations remain opt-in). Both feature sets pass
clippy with `-D warnings`; fmt and the wasm32 core build pass. The 13 model HTTP
integrations, three driver HTTP integrations and existing fake TCP integration
were attempted and fail at socket creation under this sandbox. A direct binary
probe produced the expected task-1 timeline; this is not the full harness pass.
The lock-protected real setup/scorer and full scripted-HTTP task-1 runs remain
unvalidated because the shared lock is held externally and sockets are denied.
No benchmark/model measurements or local commit are claimed.
