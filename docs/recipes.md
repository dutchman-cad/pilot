# Model-face recipes

Plan-text recipes are off by default. `--planner` enables recipe prompts, recipe
prose conversion, continuations, and completion checks. `--no-plan` (alias
`--no-planner`) is accepted as a no-op alias for the default; it does not override
`--planner`. Argument repair, text-call pickup, command lookup, hints, and text
tool mode remain enabled. In plan mode, the model receives SolidCraft's tool
results and writes the final answer. The MCP `plan` tool exposes suggestions only
and never converts prose into actions.

Exact matches are answered directly by default without an upstream model request,
including capability probes. Pilot emits native `tool_calls` using the client's
advertised `execute` and, for multi-step geometry, `batch` shapes; SolidCraft
validates and approves them normally. Streaming requests receive valid SSE.
After the client returns every call's result, Pilot runs the same completion
check and replies with `Exact match (no model):`, the recipe, measured body count,
volume, and available bounds-derived sizes. Length conversions are stated explicitly.
A tool error, missing result, unresolved topology, or completion mismatch hands
back the failure to the model once; direct answering stays disabled for that
turn so the model can recover without replaying accepted changes. With no model,
Pilot states the failure plainly instead. Two-intent requests use the model:
with `--planner`, when a leading clause matches, Pilot offers that computed plan
in context and asks the model to submit calls for the whole request. Offered calls still need
client approval and results; they are never described as already executed.

Use `--upstream none` for a no-model assistant; it rejects `--no-direct-recipes`.
`/v1/models` then lists `pilot-recipes`; other requests receive a fixed
no-model explanation listing supported kinds from the recipe table. Otherwise,
the upstream default remains Ollama. `--no-direct-recipes` sends everything to the
model; `--direct-recipes` is a no-op alias for the default and conflicts with that
opt-out. Direct answering stays disabled for `--dialect solidcraft98`, even with
`--direct-recipes`. Direct answering works independently of `--planner` and accepts
`--no-plan`. It can accompany `--planner`:
exact matches take the direct path, while plan-text suggestions are available
on the model path. Direct mode retains its own continuations and measured checks.

Audit kinds are `direct_recipe_calls`, `direct_recipe_reply`, and
`direct_recipe_fallback`, with `from_model: false` on direct events. Profiles
count `direct_turns` and those events separately; direct replies do not enter
model turns, reply statistics, capability evidence, or recent text-call history.
Recovery requests enter normal model statistics.

SolidCraft tracks offering these matches in its own command search and chat input
in [tangency-labs/solidcraft#5862](https://github.com/tangency-labs/solidcraft/issues/5862).

Recipes are a bounded table in `src/planner/recipes.rs`, with typed slots and a
shared length parser. Every expanded call checks its command and parameter names
against the supplied catalogue. SolidCraft remains the authoritative validator.
A recipe needs an action or a geometry noun request and all required slots. How-to questions,
negations, explanations, unsupported modifiers, missing slots, and multiple
intents are left to the model. There is no benchmark wording or task-id matching.

| Recipe | Required slots and supported form |
| --- | --- |
| Box / block / brick / bar / plate / slab / sheet / cube | Three dimensions in length × width × height order, or uniquely labelled length, width, and height; a cube can have one side. Two dimensions plus an explicitly labelled thickness also work. |
| Cylinder / rod / disc / disk / puck / coin | Explicit radius or diameter (also `Ø`, “across”, or “wide”), and height / tall / high / thick / deep / length / long. “Round disc” also works. |
| Sphere / ball | Explicit radius or diameter / `Ø` / across / wide. |
| Hollow tube | Outer and inner diameter (`OD` / `ID`), or outer diameter and wall thickness, plus length / height. Inner diameter must be positive and smaller than outer diameter. A new named cylinder and targeted axial through hole leave existing bodies untouched. |
| Plate with centred hole | Box/block/plate dimensions or one cube side, hole size, centre/center/middle or centred/centered, and through (including “hole through the center”). An unqualified “5mm hole” means diameter 5mm, as does “one centred 6mm through hole”. “Draw” works wherever “make” does. |
| Plate with positioned holes | Plate dimensions, integer count, common hole radius or diameter, through, and exactly that many `(x,y)` positions after `at`. Also two holes at a specified inset from each short end, halfway across the width (or each end on the centre line of a nonsquare plate); four holes at a specified common inset from both adjacent edges at each corner. Positions use the plate's origin corner, with Z supplied from its thickness. |
| Plate with rectangular hole pattern | Plate dimensions, hole size, through, centred rectangular pattern, rows, columns, and X × Y pitch. An optional total count must agree. |
| Plate with circular hole pattern | Plate dimensions, hole size, through, centred circular pattern radius or diameter, count, equally spaced, and explicit `starting at +X`. |
| Drill / bore / cut a hole | Uniquely resolved existing body, explicit hole radius or diameter, `through BODY at (x,y,z)`. All three coordinates are required; uses the public hole command's Z direction. |
| Sketch rectangle then extrude | Rectangle width × height and a principal plane, then / and extrude a distance, including “extrude it”. “Raise”, “pull it up”, and “extrude by” also work. |
| L profile then extrude | L, L-shaped, or angle profile / bracket sketch with width, height, both leg thicknesses, and extrusion distance. XY by default, or XY / XZ / YZ explicitly. The outside corner is the origin; the horizontal leg follows sketch X. One inferred closed polyline and four driving dimensions fully constrain it. Holes, edge treatments, other geometry, and thicknesses reaching width or height defer. |
| Sketch circle then extrude | Circle radius or diameter, principal plane before or after the profile, then / and extrude a distance, including “extrude the sketch”. An unqualified “18 mm circle” means diameter 18 mm. |
| Fillet / round / round off all edges | Uniquely resolved current body, all edges (also “every edge” / “all the edges”), and radius, including “a 2mm round”. |
| Chamfer / bevel all / top edges | Uniquely resolved current body, all or top edges, and distance. |
| Move / translate / shift / slide | Uniquely resolved body, one signed distance, and exactly one X / Y / Z axis. “negative X”, “in the negative Y direction”, “along -Y”, and “down Y” also work; conflicting signs defer. |
| Rename / call / name | Uniquely resolved source body and `to NAME`, or `call it NAME` / `name the body NAME`; `rename to NAME` for the unique current body; quote names containing spaces. |
| Mirror | Uniquely resolved body and one principal plane XY / XZ / YZ. Keeps the original by default or with “keeping the original”. “Not keeping the original”, “without keeping the original”, or “do not keep the original” emits mirror then `solid.remove` for the original. Joining is unsupported. |
| Uniform scale | Uniquely resolved body, explicitly about the origin, and a factor: `by 1.5`, `by 150%`, `double` (2), or `half` (0.5). Factor must be positive and at most 1000. |
| Shell box | New box dimensions, wall thickness, and explicitly open / remove top. Shelling inward leaves the bottom and four walls. |
| Revolve rectangle | Rectangle radial `radius` and height, explicitly `from axis on XZ about Z full turn`. Other profiles, axes, offsets, and partial turns defer. |
| Volume query | “What’s the volume”, requests mentioning volume with unit wording, or “how many cubic millimetres”. A complete measured snapshot, optionally one uniquely named body; otherwise one inspect request. A current / existing solid reference requires exactly one body. Replies use mm³ by default, or the requested cm³ / m³ / in³ / ft³ with the conversion stated. Conflicting units defer. |
| Body count query | “How many bodies / solid bodies / parts / solids are here / in the design”, or count bodies. A complete snapshot; otherwise one inspect request. “How do I count” defers. |

Lengths default to mm. `mm`, `cm`, `m`, `in`, `inch`, `inches`, and `"` are
accepted, including simple fractions such as `1/4 inch`. Mixed fractions such as
`1 1/4 inch` defer; write `5/4 inch`. Tuple separators are `x`, `by`, and `×`.
A trailing tuple unit applies to its unqualified members. Independently labelled
lengths have independent units. Plans explicitly say that slots have been
converted to mm and display their numeric values. Radius and diameter retain
their mathematical meaning.

Matching ignores case, sentence punctuation (`. , ; : ! ?`), and quotes or
brackets around words. A question mark still invokes the question guard: action
questions defer, except polite `can you` / `could you` / `would you` requests, including after filler.
Decimal points, fractions, `x` / `×`, and inch marks keep their numeric meaning.
Quoted rename destinations preserve their original case and punctuation; words
inside a complete quoted destination are name data rather than competing actions.

Normalization removes this small filler list anywhere: `please`, `pls`, `thanks`,
`thank`, `you`, `can`, `could`, `I`, `need`, `want`, `give`, `me`, `ok`, `okay`, `so`.
It preserves numeric punctuation, coordinates, and opaque rename destinations.
Unknown conversational words are tolerated. Label colons may precede values, and
numbers may precede their shape noun, including a separate sentence:
`Length: 8.4cm, width: 3.6cm, height: 0.9cm. I need a solid brick.`
A thickness may precede the remaining two dimensions: `a 9mm thick slab, 84 by 36`.

Conflicting action families or primitive families defer; synonyms form one family.
Negation and explanation guards also include `no`, `avoid`, `skip`, `stop`,
`neither`, `nor`, `explanation`, `describe`, `tutorial`, `instructions`, and
common negative contractions (`can’t`, `won’t`, `isn’t`, `shouldn’t`, `wouldn’t`,
`couldn’t`, `mustn’t`, using ASCII apostrophes).
The semantic deny list includes `not`, `don`, `dont`, `never`, `without`, `undo`,
`cannot`, `cant`, `explain`, `would`, `what`, `how`, `why`, `whether`, `example`,
`if`, `or`, `instead`, `except`. Measurement questions are a separate guarded path;
the scoped mirror-original phrases above are the only negation exception.
Known unsupported modifiers also defer: rotate/rotation, taper/tapered,
rounded, thread/threaded, counterbore/countersink, blind, offset, angled/angle,
hollowed, combine/join/union/subtract/intersect/delete, duplicate/copy/copies,
twice, oval/ellipse/cone/torus, flat/vertical/curved, triangle/triangular/polygon,
sweep/loft, text/engrave, color/colour/material, unless/maybe/possibly/either/but.
`hollow` is restricted to tubes; primitive positioning/centering modifiers defer.
An unsupported fillet or chamfer on a new primitive is a conflicting intent.

Labelled lengths accept both orders: `length 31mm, width 22mm, thickness 4mm`
or `31mm long, 22mm wide and 4mm thick`. Height also accepts `high`, `tall`,
`deep`, and `thick`. Radial forms include `14mm in diameter`, `14mm across`,
`a radius of 7mm`, and `with an 8mm radius`. Separate labelled lengths with
`and` or commas; each retains its own unit. Labels must identify every required
slot uniquely. `the box`, `the part`, `the body`, and `it` refer to the current
body only when exactly one body exists, including when a body is named `Box`.

New primitives use `operation: new` and SolidCraft's documented origin / Z-axis
defaults. Rectangle sketches start at the sketch origin; circles are centred
there. Sketch and mirror plane slots are restricted to XY / XZ / YZ, case
insensitive, including “the X-Y plane” and “XY plane”. Named construction planes,
unknown plane text, and the undocumented “top plane” alias defer. Axis slots
produce a typed vector or documented constant; names preserve exactly the quoted
destination or single unquoted token before terminal punctuation. Additional
unquoted sentence text defers. Through holes must be fully enclosed and
non-overlapping. Compound plates and shells receive an available `PilotRecipeN`
body name, and subsequent modifications explicitly target it, including when
older geometry overlaps the new body.

Planner replays live in `tests/fixtures/planner-replay/`. Each request file is an
ordinary chat-completions JSON body with its original `messages` and `tools`.
`cases.json` lists `{"request_file":"example.json","expected":...}` entries.
An expected plan contains `rule`, the complete typed `slots` object, and the
complete `calls` array; `null` asserts that no recipe fires. The loader limits
request size and fixture count, checks JSON bounds, and uses the bundled public
catalogue. Run `cargo test --test planner_replay` with either feature set.
The checked-in request bodies are synthetic, independently authored examples with
a long system prompt and one user message. A further test expands that context
past 16 KiB and verifies that removing the offered tools suppresses planning.

For example:

```text
build a block 17 by 23 by 6 mm
create a rod diameter 1/4 inch tall 2 inches
sketch a circle radius 9mm on XZ then extrude 7mm
build a plate 80x60x6mm with 2 through holes diameter 4mm at (12mm,15mm), (65mm,40mm)
shell a box 70x50x30mm wall 2mm open top
```

With direct answering enabled, an edit whose geometry slots already parse but
whose current body is unknown can first emit one read-only `execute` call to
`document.inspect` with `measure:true` (the public catalogue's inspect equivalent).
No mutation uses a provisional body name. The continuation validates every
returned tool result, resolves the actual body, and only then emits edit calls.
Zero bodies, multiple bodies, partial snapshots, failed results, or missing results defer;
the lookup never repeats. Plan-text mode retains its original acted/body guards.
These prerequisites use the existing direct audit events with rule `resolve_body`.
The six first-turn edit lifecycle cases use recorded public `solidcraft-cli 0.20`
responses in `tests/fixtures/inspect-single-body-0.20.json` and
`edges-single-body-0.20.json`: a new 20 × 12 × 8 mm box (`Body1`), measured with
`document.inspect {measure:true}`, and its `model.edges {body:"Body1"}` result.
Inspect reports the edge count rather than references, so fillet and chamfer
first request that read-only topology lookup before emitting the exact edit.
Each edit also checks that two bodies hand back to the model without a mutation.

A fresh request fires at most one recipe. The existing `acted` check suppresses
new recipes after an `execute` or `batch` call in that request's history, even if
it failed. Edge recipes can first request `model.edges`, then continue that
already selected recipe once with returned indices. Top-edge selection requires
complete straight-edge topology and certified endpoint / midpoint heights;
curved or incomplete topology defers. No accepted creation prefix is replayed.

A model's action pledge (`I will`, `I'll`, `Let me`, or `I am going to`) can become
the selected recipe's ordinary tool calls when parsing that pledge yields the
same intent and slots as the user's request. Different dimensions, another
intent, arbitrary prose, refusal fields, or multiple completion alternatives
never trigger this conversion. Dimensions always come from the user recipe.
Native calls and explicit text JSON still use the existing repair path. When the
client advertises `batch`, multi-step prose recipes use that atomic batch for
geometry and a separate final inspection. Otherwise they return the ordered
`execute` calls; the client must execute them in order and stop on failure.

Plans log `kind: plan`, rule, typed slots, expected result, calls, and prompt.
Prose conversion logs `recipe_prose` with the original reply and resulting calls;
edge continuation logs `recipe_continue` with the resolved indices. Every action
plan ends with a measured `document.inspect` when the catalogue offers it.

After the model's final reply, one completion check compares the latest complete
state against expected body count and total volume where a baseline is known.
Volume tolerance is max(0.01 mm³, 0.1%). Without a baseline, a known created name
or a sole resulting body permits checking the created volume; otherwise Pilot
asks for verification once instead of assuming the design was empty. Rename
checks the resulting name; move also checks translated bounds when baseline
bounds are available. Missing / partial / failed measurements count as unverified.
When a baseline timeline is available, fillet and chamfer also require one more
successful, unsuppressed feature of the requested type. A pending edge lookup is
always unverified; a successful lookup alone does not establish completion.
A difference is handed back once, even after an accepted call, within the existing
two-hand-back / four-attempt request budget. No completion recipe means no new
completion guard. Count and volume cannot prove every geometric property of a
fillet, chamfer, or mirror, and unmeasured movement has only count / volume checks.

Limits: 4096 request characters and messages, shared 1 MiB JSON and nesting caps,
256 bodies / edges inspected, 1,000,000 mm per length, 62 holes (at most 64 calls
including creation and verification), one edge continuation, and one completion
hand-back. Invalid units, non-finite dimensions, and unsupported geometry defer.

`src/planner/cases.rs` contains independently authored phrasings (five or more per
recipe, including separate hole layouts and sketch shapes), inch / fraction
cases, and refusals. They replay through the model face with canned prose.
An optional clean-room geometry smoke test uses only the public MCP binary:

```sh
PILOT_SOLIDCRAFT_CLI=/path/to/solidcraft-cli cargo test --test recipes_cli
```

This test covers 31 geometry cases and checks measured expected results. It is
not a weak-model benchmark and establishes no live-model pass-rate improvement.

`src/planner/robust.rs` adds 228 independently authored exact-slot phrasings
(at least twelve per recipe), over fifty refusal cases, and 288 compositions of
filler, metric unit style, and dimension order across every creation recipe.
Additional tests cover fractions, inch marks, round discs, mirror retention,
scale factors, and the bounded body-resolution prerequisite. Proxy tests exercise
inspection, edit, and measured completion without upstream traffic, plus refusal
on ambiguous, partial, failed, and missing inspections. Existing fixtures retain
their expectations; no held-out benchmark requests are read or embedded.
