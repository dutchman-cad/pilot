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
| Closed profile then extrude / pad | L / angle / angle bracket / L-bracket: width, height, and equal leg thickness or separate bottom / horizontal and upright / vertical thicknesses (4 dimensions). T / tee / T-section / rail: overall or flange / cap width, height, flange thickness, and stem / web thickness (4 dimensions); stem centred under the flange, or an exact stem-centre offset from the left or centre (5 dimensions). U / channel and C-section / C-channel: width, height, common wall thickness (4 dimensions), or separate web and flange / leg thicknesses (5 dimensions). Rectangle / plate / bar: explicit fully constrained or dimensioned sketch, width and height (2 dimensions). Each uses one inferred closed `sketch.line` with stable ids, geometric constraints, driving dimensions, finish, and extrusion. Existing L plans retain their exact commands and `l_profile` audit name; other profiles use `closed_profile`. |
| Sketch circle then extrude | Circle radius or diameter, principal plane before or after the profile, then / and extrude a distance, including “extrude the sketch”. An unqualified “18 mm circle” means diameter 18 mm. |
| Fillet / round / round off all edges | Uniquely resolved current body, all edges (also “every edge” / “all the edges”), and radius, including “a 2mm round”. |
| Chamfer / bevel all / top edges | Uniquely resolved current body, all or top edges, and distance. |
| Move / translate / shift / slide | Uniquely resolved body, one signed distance, and exactly one X / Y / Z axis. “negative X”, “in the negative Y direction”, “along -Y”, and “down Y” also work; conflicting signs defer. |
| Rename / call / name | Uniquely resolved source body and `to NAME`, or `call it NAME` / `name the body NAME`; `rename to NAME` for the unique current body; quote names containing spaces. |
| Mirror | Uniquely resolved body and one principal plane XY / XZ / YZ. Keeps the original by default or with “keeping the original”. “Not keeping the original”, “without keeping the original”, or “do not keep the original” emits mirror then `solid.remove` for the original. Joining is unsupported. |
| Uniform scale | Uniquely resolved body, explicitly about the origin, and a factor: `by 1.5`, `by 150%`, `double` (2), or `half` (0.5). Factor must be positive and at most 1000. |
| Shell box | New box / enclosure / container dimensions, wall thickness, and explicitly open / remove top or bottom. Optional floor / base thickness; corner at origin (default) or centred at origin. An optional lip / rim / locating lip / lid lip / ridge needs width and height: two nested rectangles on the opening rim with a point selecting the ring profile, extruded outward and joined to the new body. The ring sits on the inner edge of the wall by default, or on its outer edge with outer / outside. |
| Revolve rectangle | Rectangle radial `radius` and height, explicitly `from axis on XZ about Z full turn`. Other profiles, axes, offsets, and partial turns defer. |
| Volume query | “What’s the volume”, requests mentioning volume with unit wording, or “how many cubic millimetres”. A complete measured snapshot, optionally one uniquely named body; otherwise one inspect request. A current / existing solid reference requires exactly one body. Replies use mm³ by default, or the requested cm³ / m³ / in³ / ft³ with the conversion stated. Conflicting units defer. |
| Body count query | “How many bodies / solid bodies / parts / solids are here / in the design”, or count bodies. A complete snapshot; otherwise one inspect request. “How do I count” defers. |
| Revolute pin joint | Inspect first (`document.inspect`, `component.list`, `joint.list`, then each body's `model.faces` / `model.edges`). Connect two named components with one cylindrical pin / hole pair on matching upright axes within 0.5 mm; pick the pin root and hole bottom contact circles, ground the pin component, use `flip:true`, optionally drive one exact angle in degrees, then verify both occurrence transforms and joint pivots with `component.list` / `joint.list`. Pin / hole diameter or radius qualifiers accept bounded metric, inch and fractional lengths. Multiple pairs, unnamed components, existing joints, nested or transformed occurrences, unclear angles, unsupported geometry and extra operations defer. |

Lengths default to mm. `mm`, `cm`, `m`, `in`, `inch`, `inches`, and `"` are
accepted, including simple fractions such as `1/4 inch`. Mixed fractions such as
`1 1/4 inch` defer; write `5/4 inch`. Tuple separators are `x`, `by`, and `×`.
A trailing tuple unit applies to its unqualified members. Independently labelled
lengths have independent units. Plans explicitly say that slots have been
converted to mm and display their numeric values. Radius and diameter retain
their mathematical meaning.

Closed profiles accept label-first or value-first sizes (`width 40`, `40 mm wide`),
and width × height pairs (`40x30`, `40 by 30 mm`). Extrusion can be `extrude it 12`,
`extrude 12 mm`, `pad 12`, `12 mm deep`, or `12 long`. The sketch plane defaults
to XY; `on XZ`, `in the XZ plane`, front (XZ), top (XY), and right (YZ) plane
wording also works. Coordinates and opening directions refer to sketch axes;
the public plane transforms determine the world bounds (XZ sketch Y maps to −Z,
and its extrusion normal is +Y).

The default origin is the lower-left bounding-box corner; L uses its outside
corner. A T has its flange at the top and its stem below; placement uses axis
constraints because its bounding-box corner lies outside the material. Symmetric
T, U, C and rectangular profiles can instead be `centred on the origin`, placing
the bounding-box centre there. U opens toward sketch +Y, C toward +X, with
explicit up / down / left / right openings supported. Opposing flange ends and
T shoulders are constrained collinear, and midpoint relations centre the stem.
Named construction points support centred placement and offset dimensions;
there is still only one closed polyline.

This profile recogniser uses a closed vocabulary, including the courtesy words
listed below. Unknown geometry (holes, fillets, arcs, radii, cuts, patterns, and
so on), unknown planes or directions, conflicting labels, extra or unassigned
numbers, unreadable offsets, and zero or overlapping walls defer. Each length is
bounded by the shared 1,000,000 mm limit; profiles have at most 7 input sizes,
8 polyline edges, and 5 driving dimensions. English tests are independently
authored; the optional public-binary tests check zero DOF, one profile, driving
dimension counts, volume and world bounds on XY, XZ and YZ.

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

The revolute recipe uses two bounded read-only prerequisites (`resolve_joint`) in
both model plan mode and direct mode; missing topology never retries a lookup.
`tests/fixtures/revolute-public-0.20.jsonl` records public MCP results from building
the eight setup commands in `bench/sets/v2/references/09-revolute-joint.json`, then
inspecting faces and edges and driving contact-circle picks with `flip:true`.
The CLI test also checks scaled parts and off-origin pivots. A moved pivot or
inverted occurrence gets one short contact-circle hint per user turn, respecting
`--no-hints`.

`tests/fixtures/revolute-assistant-0.20.json` copies the first three public
Assistant request bodies from the permitted v2-09 run. Unlike the CLI's JSON-RPC
`result.content[].text` envelopes, the Assistant sends pretty-printed JSON strings
with plain batch entries and `completed` / `ok` fields. Calls and results remain
in order, but the two lookup stages reused one call id. The second batch string
ends mid-entry after 20,000 characters with `…(cut)`; it cannot provide complete
topology. Direct recipes therefore emit separate topology calls with distinct
ids, accept either result envelope, and correlate reordered results by id. The
regression preserves the invalid original request and uses its three complete
topology entries plus the public CLI's final Arm edge result to exercise the
new continuation. Truncated results still hand back without a mutation.

Closed profiles also accept `fully defined`, `dimension and constrain it`, and
an overall width expressed as `50 mm along the bottom`. Sentence punctuation and
`then extrude it` retain their existing meaning. These forms expand to the same
constraints and dimensions as the corresponding explicitly labelled profile.
`tests/fixtures/constrained-profile-assistant-0.20.json` preserves the first
request body from the permitted v2-11 run for a direct-mode regression.

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

Compound plates combine a centred or corner-origin plate on XY, XZ or YZ with a centred through bore, equally spaced bolt-circle holes (diameter, radius or PCD), a centred rectangular hole grid, and fillets or chamfers on the four vertical corner edges or all edges. Sizes accept mm, cm and inches, including fractions. An unstated bolt-circle start uses +X and is disclosed in the reply. Construction creates the plate, cuts the holes, then rounds the selected edges; four-edge rounding selects only corner midpoints at half thickness. Hole tangency, overlap, plate-edge contact, corner radii at least half the smaller side, uncertain placement or unsupported wording defer to the model; holes near blends are conservatively refused. All-edge rounding includes hole rims and leaves volume verification to measured state. Edge-format and missing-edge errors receive one model-face hint per retained conversation to use model.edges midpoints.

Shell lips require positive explicit sizes and width smaller than the wall. Walls must be smaller than half the smaller planar side; wall and floor must be below the box height. Each number has exactly one role, with at most seven sizes. Unknown wording, nonprincipal directions, conflicting positions, grooves, recesses, and cuts defer. Separate floor thickness adjusts the inward shell floor without changing the specified exterior height. Centred boxes place their three-dimensional bounding-box centre at the origin before adding the lip. Existing uniform top-open shell commands remain unchanged.

## Typed model shortcut

Normal model requests omit `pilot_find` and `pilot_build` by default. Pass
`--offer-recipe-tools` for a model that uses them; they require SolidCraft's native tools.
The profile records `offer_recipe_tools` (older profiles default to false).
There is no upfront “call pilot_find first” fact. The legacy build form accepts a `shape` tag: `profile` (L, T, U, C, rectangle), `plate`,
`enclosure`, or `revolute_joint`. Convert all lengths to mm before calling
(1 in = 25.4 mm). Profiles use `thickness` or both `web_thickness` and
`flange_thickness`; U uses web for its bottom and flange for its two sides.
`stem_offset` is the T stem centre measured from the left edge. Profiles start
at the sketch origin; plates use the selected plane and placement; enclosure
centring includes Z. Hole grids, bolt circles and rows are centred on the plate.
The plate form accepts either `length` and `width` or a round `diameter`, plus
`thickness`. `boss_diameter` and `boss_height` add joined round pegs/bosses on
the top face. Without a layout there is one boss at the plate centre;
`boss_offset_x`/`boss_offset_y` are signed sketch-plane offsets from that centre.
A `row` has `count`, centre-to-centre `pitch` and optional `axis` (X by default,
or Y). The flat equivalents are `row_count`, `row_pitch`, `row_axis`. Circles
use the existing `bolt_hole_count` and `bolt_circle_diameter` fields for either
holes or bosses. Omit hole diameters for raised features. Grid columns run along
sketch X and rows along sketch Y; an unqualified A×B grid puts A along the
longer side unless directed otherwise. Thus a 7×5 grid on a 120×80 plate uses
7 columns, 5 rows, and a stated 15 mm pitch maps to both grid pitches.
Patterns are capped at 64 features and must fit within the base without
overlap. A centre bore can accompany bosses with sufficient clearance.
`bore_diameter` sizes the centre bore; `hole_diameter` sizes pattern holes,
while `bolt_circle.diameter` is the pitch circle diameter. Start angles default
to zero degrees (+X). Joint names (`pin_component`, `hole_component`) are optional;
the joint recipe inspects contact geometry first and applies optional `angle_deg`.

Pilot shares the English recipes' bounds, geometry validation, catalogue checks,
continuations and measured completion checks. The client sees real SolidCraft calls
for approval. Invalid fields return a local tool error and allow two retries, then
Pilot removes the shortcut for that turn and hands back to the model. Audits record
`shortcut` and `shortcut_rejected`. Exact English matches still run first.
`--no-shortcuts` disables both tools independently of `--no-hints` and
`--no-direct-recipes`; the SolidCraft 98 dialect never offers them.

## Find before build

With `--offer-recipe-tools`, the model can call `pilot_find({"request":"a mounting plate with six holes"})`
first, then `pilot_build({"recipe":"plate","args":{...}})`. Search returns three
ids, descriptions, parameter schemas and BM25 scores. Native SolidCraft tools
remain available. `--no-shortcuts` disables both local tools, including text mode;
exact English recipes retain their existing priority.

Each reviewed file in `recipes/` is Markdown with `---` YAML frontmatter written
in YAML's JSON subset (strict JSON syntax, parsed without a YAML dependency).
Required keys are `id`, one-line `description`, `examples` (3–8 ordinary requests),
`schema` (JSON Schema 2020-12), and `expander`. The body explains the recipe and
expander. Each file selects one expander; ids need not equal expander names.
The build script embeds all reviewed `.md` files. No runtime reads are required.
`model --recipes-dir DIR` replaces the set at startup for development; it never
reads nested directories or symlinks. Limits: 128 entries, 64 KiB per recipe,
2,048 query bytes, three results, two searches per turn, 32 schema levels.

Shared `Length` (positive mm, at most 1,000,000), `Angle` (−360…360 degrees),
`Point3` (three signed mm coordinates), `Plane` (XY/XZ/YZ), and `Count` (1…64)
are defined once in Rust and referenced through `$defs` in each returned schema.
Object schemas refuse unknown keys, including nested options. File schemas for
built-in expanders must equal the canonical typed contract: editing a file cannot
silently weaken a validator. Geometry relationships (for example a tube's bore
must be smaller than its outside) use the explicit `x-pilot-expander` schema
extension: Pilot invokes that expander's pure argument validation. Standard JSON
Schema cannot compare arbitrary numeric properties; external schema consumers
must honor this extension to get the full geometry contract. Inspected topology
is additionally checked by the existing expander. A parameter schema alone cannot guarantee a build in
an arbitrary document. The tests exercise schemas and expansion on generated
field mutations and independently authored requests.

Retrieval uses BM25 (k1=1.2, b=0.75), light suffix stemming and aliases for common
editing verbs. Numbers and length units are ignored so dimensions do not dominate
intent. Descriptions and examples are separate small documents; the best document
score selects each recipe, multiplied by query-term coverage in that recipe.
Ties sort by id. `Ranker` permits a future local
embedding implementation; this version uses no embedding or network service.
A best score **below 1.8** is a miss. This fixed initial threshold is calibrated
against the independent 70 positive and 12 off-topic queries in
`tests/recipe_library.rs`; it is not a probability or a benchmark claim.
On a miss the result says “No good recipe hit”, includes closest recipes, and
asks the model to explain supported work or ask one question instead of
improvising CAD. Search never blocks the model's native tools. Audit events are
`find` (query, top ids and scores), `find_hit` and `find_miss`; profile counters
are `finds`, `find_hits`, `find_misses`. Tool descriptions and total tool text
have a measured regression budget in the tests; no recipe catalogue is appended
to the system prompt.

## Draft a recipe from an audit

```
dutchman-pilot recipe new --from-audit rewrites.jsonl --turn 1
```

`--turn` selects the one-based recorded recipe emission (`plan` or
`direct_recipe_calls`), not an individual JSONL line. New audit records include
the request and expanded calls; older logs without a request are rejected with
an explanation. Input is limited to 64 MiB, 1,000,000 records and lines to 8 MiB. The command
prints a new `recipe-review/draft_turn_N.md`, refuses to overwrite a draft,
and never adds it to the embedded or development library. Review every call,
body target, parameter name, bounds and example before moving the file into
`recipes/` and rebuilding. The two generic examples are draft placeholders.

Drafts select the `sequence` expander. Numeric leaves in call parameters become
number parameters (shared Length, Angle or Count when the field and value fit),
with recorded values listed in the body; command names and
body references remain literal. Templates use `{"$param":"parameter_name"}`
as the entire value, preserving its JSON type during substitution. There are
at most 64 `execute` calls, 512 parameters, and 16 inference levels. Expansion
checks commands, documented parameter names, required arguments and available
catalogue types before returning any call. SolidCraft still validates and the
client still approves real calls. Unsupported catalogue schema syntax fails
closed. Sequences make no inferred volume claim.

The search-before-generation pattern is inspired by the public
[Rote README](https://github.com/wdunn001/rote) (MIT); Pilot neither depends on
Rote nor copies its implementation. Shared types and refused unknown keys follow
the public [typed-schema RFC](https://github.com/tangency-labs/solidcraft/issues/2829).

## Pattern recipes

Use `pilot_find` before `pilot_build` for repetition: “make 50 of these in a row”,
“12 copies of this boss 15 mm apart”, “array it 4 by 6”, “repeat the hole every
20 mm”, or “put 8 around the centre”. Copies with a count and spacing mean a
pattern. Retrieval supplies the typed schema; ask for missing count, spacing,
axis, or target instead of issuing primitive calls repeatedly.

| Recipe | Example `pilot_build` arguments |
| --- | --- |
| `linear_pattern` | `{"target":"body","count":50,"spacing":15,"direction":"X"}` |
| `grid_pattern` | `{"target":"body","rows":4,"columns":6,"spacing1":15,"spacing2":20,"plane":"XY"}` |
| `circular_pattern` | `{"target":"last_feature","body":"Plate","count":8,"axis":"Z","angle_deg":360}` |

Wrap these as `{"recipe":"linear_pattern","args":{...}}`. Linear patterns accept
`spacing` or `total_length` (first to last), or both when
`total_length = (count - 1) × spacing`, and exactly one of
`direction` X/Y/Z or `edge` (an inspected straight edge name). Grids use the two
axes of XY/XZ/YZ. Circular axes pass through the origin; partial arcs include both
ends, while full turns do not duplicate the original at 360 degrees. Lengths are
positive millimetres, angles are degrees, and counts include the original.
Each dimension is at least two; the total is capped at 500 instances and extents
at 1,000,000 mm. Missing or ambiguous targets defer.

The expander first requests measured `document.inspect`, then resolves one body
(optionally selected by `body`) or inspects the last feature with `timeline.inspect`.
An edge direction adds a `model.edges` read. It emits one
`solid.pattern.rectangular` or `solid.pattern.circular` plus measured inspection.
Completion checks the successful timeline pattern feature, body count and volume.
Body patterns create separate bodies and have a computed disjoint volume. Feature
patterns inherit their source operation: new bodies, joined bosses, or cuts.
Joined/cut volumes are checked against the measured baseline and conservative
volume bounds, since partial intersections cannot be computed from bounds. Unresolved feature geometry defers; overlap checks use conservative
bounding boxes and may ask for clarification. `join:true` explicitly permits
feature overlaps; it does not change a source operation or invent a pattern
command argument. Body patterns cannot request a join.

When three equal explicit creation/copy calls in one user turn have a constant
position step, the model receives one short `solid.pattern.rectangular` /
`linear_pattern` hint. The hint is audited, deduplicated per retained conversation and disabled by
`--no-hints`; it never executes or replaces the calls.
## Cross-drilled boxes

`pilot_find` retrieves `cross_drilled_box` for a box or cube with through holes
on one, two or three axes, including “holes through all six faces”,
“cross-drilled”, “a hole through each face”, and “holes along X, Y and Z”.
Tidy English forms with a stated box size and hole diameter also match exact
recipes, for example “Build a 24 mm cube with 6 mm holes through all six faces”.
Missing diameters, blind holes, extra operations and ambiguous positions decline.

`pilot_build` accepts `recipe:"cross_drilled_box"` and
`args:{length:24,width:30,height:36,holes:[{axis:"X",diameter:6},{axis:"Z",diameter:4}]}`.
Lengths are millimetres. An optional `corner:[x,y,z]` places the box; otherwise
it starts at the origin. Each hole may supply a world `position:[x,y,z]` on
either face normal to its axis, or the body centre (normalized to the upper
entry face). Without a position, the opening is centred. Optional `offset_u`
and `offset_v` displace it from the face centre: X uses Y/Z, Y uses X/Z, and
Z uses X/Y. Each offset defaults to zero. Optional `depth` makes a blind hole;
omitting it drills through. Interior points other than the body centre are
ambiguous and rejected with an example.
One to three distinct axes are allowed, with positive bounded dimensions and
openings strictly inside their faces. Each axis uses one `solid.hole` with an
explicit world position, inward direction and target body. No sketches or
construction planes are created. Equal centred diameters use exact cylinder
intersection volumes; other positions or diameters use measured volume.

With hints enabled, requests or calls involving XZ/YZ sketches or side holes
receive the shared, audited `SKETCH_AXES` fact once per retained conversation. Sketch (u,v)
maps to XY (X,Y), normal +Z; XZ (X,-Z), normal +Y; YZ (Y,Z), normal +X.
World Z=+10 therefore needs XZ v=-10. Prefer `solid.hole` for holes. A profile
whose XZ centre falls outside every known body's Z range while reversing v
would put it inside gets one sign hint per retained conversation. Unreferenced
construction planes created during the turn get one cleanup hint per retained
conversation before finishing.
`--no-hints` disables the facts and both hints. The public CLI smoke test
checks all mappings and extrusion normals, hole volume/bounds/round faces,
and the absence of auxiliary timeline features.

### Honeycomb through cuts

`pilot_find` retrieves `honeycomb_pattern` for a honeycomb or staggered hexagonal
plate. `pilot_build` takes `length`, `width`, `thickness`, `size`, `size_kind`
(`across_flats` or `across_corners`), `web` and `border`, in mm. Optional slots:
`plane` (XY default, XZ, YZ), `centred` (false), `orientation` (omitted: rows
along the longer side, X for squares; explicit `pointy_top`: X rows or
`flat_top`: Y rows overrides this), and `mode` (`fill` default or `exact`).
An optional `corner:[x,y,z]` sets the final minimum bounding-box corner and
cannot be combined with `centred:true`. Patterns are cut at the origin, then
the finished body is placed with one rigid move.
Length stays along sketch X and width along sketch Y. Exact mode needs
`rows` and `columns`; fill rejects explicit counts. See the
[recipe](../recipes/honeycomb_pattern.md) for the full placement contract.

Tidy English forms put plate dimensions first, then hex size, spacing and border:
“Make a plate 120x180mm thickness 3mm with a honeycomb pattern 8mm AF spacing
2mm border 5mm.” Also accepted: edge to edge, flat to flat, width, point to
point and corner to corner. Add `on XZ`, `centred`, `flat-top`, or
`exact 3 rows 4 columns`. Missing or ambiguous dimensions, extra operations,
nonpositive web, no complete fitting hex or more than 5000 cuts defer.

AF + web is centre pitch; row pitch is (AF + web) × sqrt(3)/2, with half-pitch
odd rows. Pointy-top columns run along local X; flat-top rotates the lattice.
Only complete hexes within the minimum border survive, and the whole field is
centred within the plate footprint. Across-corners converts to AF by sqrt(3)/2.
The footprint corner starts at the origin unless centred; thickness extends
along +Z (XY), -Y (XZ), or +X (YZ). Bounded sketch patterns form the holes in one
explicitly selected outer profile, extruded once. Each independent pattern has
at most 333 hexagons (below the public solver's 4000-coordinate group limit),
and the complete plan has at most 64 commands. This avoids repeating boolean
cuts against an increasingly complex plate and produces one body without
per-hex tool calls. Completion checks measured volume
and bounds. The CLI integration test independently reconstructs finished hex
loops to verify count, minimum web and border.

## Fill a found recipe first

On a fresh user turn that reaches the model, Pilot runs the `pilot_find` BM25 ranker
itself. At **4.5 or higher**, the first request offers exactly one `pilot_fill`
function, with the selected typed schema plus boolean `decline`, and forces that
function with `tool_choice`. Fill and route use separate requests built from scratch:
client model and sampling settings, one short Pilot system message, the latest user
message, and exactly one tool. Client system prompts, tools and conversation history
stay out. The fill instruction requests one tool call, lengths in mm (inches × 25.4,
cm × 10), optional fields only when stated, and `decline: true` for mismatches.
Target recipes append a bounded design summary to
the user message. Accepted arguments use the same validators, expanders, approval
calls and measured verification as `pilot_build`; a form never reaches the client.

Forms use a compact presentation of the full validation schema, with a tested
1,200-character budget including the system line. File schemas retain numeric
limits and field guidance; the compact form keeps the actionable enum meanings
and defaults. Unstated plate/profile planes default to XY, placement to uncentred,
enclosure openings to top, and lip sides to inner (flush with the inner wall
face). Corners default to the four vertical edges through the thickness; `all`
also rounds the rims and limits the radius to below half the thickness. Errors
report the supplied field value, numeric limit, and a suggested change.

An invalid form gets two retries with its latest field error and the same schema (three attempts total). Decline
or a third invalid form restores ordinary tools for the rest of the turn.
Text (including JSON written as text), another tool or mixed calls gets one retry
of the same clean request with a one-line previous-reply problem note; a second
such reply restores ordinary tools, without offering a route question. The rejected proposal is discarded. Normal find/build are
available only with `--offer-recipe-tools`. There is no fill on tool-result
continuations or SolidCraft 98. `--no-fill` skips forced fill and route;
`--no-shortcuts` disables fill, route and both recipe tools even with the offer flag. Exact direct
matches retain priority; use `--no-direct-recipes` to exercise forms for those requests.

The find hit/miss threshold remains **1.8**. Retrieval uses bounded per-recipe
`synonyms` groups (at most 32 strings of 128 bytes), light stemming, and field
names in addition to descriptions and examples. Number and unit tokens do not
affect scores. Scores are lexical relevance, not probabilities. Unsupported
geometry and explanation cues skip forms.

Scores from **1.5** inclusive up to **4.5** exclusive offer only `pilot_route`,
with one required `recipe` enum containing the top four ids plus `none`. Its system note asks for one `pilot_route` call selecting the matching recipe,
or `none` when no form fits. A single valid selection
offers that recipe’s fill form on the next upstream request in the same turn.
`none`, unknown ids or malformed arguments restore normal tools. Text and
mixed/off-form calls get the same single clean-request retry as fills. Scores below **1.5** use normal tools immediately. Both forms are
skipped by `--no-fill`, on continuations and for SolidCraft 98. Routes never
execute geometry. Custom sequence recipes stay on the explicit find/build path.

Tune data is preserved in `tests/fixtures/dev-route-tune.json` (127 independent
requests); `tests/route_calibration.rs` reports precision and recall separately
for direct fill, top-four route coverage, and normal-tool rejection. Thresholds
are selected only on tune data; direct fills on `none` requests must stay at or
below 15%. The earlier 4.5 fill gate satisfies this bound. The separate development
check is read once after implementation; its results belong in the commit body, never recipe files
or test sentences. The ignored `live_gemma_route_tune` sends at most six tune
requests to gemma4:latest on 127.0.0.1:11435 and prints route picks without CAD.

Creation recipes need no existing target. Patterns, mirror, move, fillet/chamfer,
existing-body shell and revolute joint forms inspect before extraction, including
before a route question containing one of these recipes. Inspection calls use the
client's normal approval path. At most three inspection rounds read the measured
document, up to 12 recent active feature definitions, and (for joints) component,
joint and contact topology. Failed, partial or missing results stop with a form
error; inspection never repeats an accepted modification.

The clean request's JSON summary is capped at 8,192 characters and 12 bodies,
with names, bounding boxes, centres, the last created body, last feature definition
and current selection (`selection`, `metadata.selection`, or inspected selection).
Joint summaries also include components, occurrences and circular contact geometry.
Body counts and a partial marker distinguish a shortened list from an empty design.
For "this", "these" or "it", whole-body forms use the selection, else the last
created body whose inspected definition identifies it, else the only body. Multiple
selections or unresolved multiple bodies produce a field error, never a guessed
target. Explicit names and last-feature targets retain shared recipe validation.
An existing rectangular box can be shelled with `body` and `wall`; omit new-box
dimensions. Omitting `body` and supplying length, width, height and wall still
creates a new box. Existing shells preserve other bodies and verify measured volume.
`form_inspect` audits prerequisite calls; `form_request` and `request_size` include
the summary on every extraction attempt.

Existing-body edit fills accept only names from a complete latest inspected
recipe snapshot; absent evidence produces a field error and bounded fallback.
Unknown development sequence state requirements stay on the normal path.
Three verbatim public Assistant requests are replayed from
`tests/fixtures/fill-*.json` with direct recipes disabled, selecting profile,
plate and enclosure. They were not used as recipe examples. The optional ignored
`live_gemma_fill_public_probes` test sends at most three requests to Ollama on
127.0.0.1:11435, with 30-second timeouts, and executes no CAD operations.
The ignored `live_gemma_clean_fill_public_l` sends each of the five public L
prompts once (at most five calls, no retries), reporting valid `pilot_fill` counts.

Audit kinds: `fill_offered` records recipe and score; `fill_accepted` records the
validated envelope and rule; `fill_declined`, `fill_invalid` and `fill_fallback`
record why the normal path resumes. `route_offered`, `route_picked`, and
`route_none` audit routing. A route and its fills share at most four upstream
requests, including provider retries, before normal tools resume. The fill itself
still has at most three attempts, including reply-shape retries. `fill_retry` records
the previous reply's problem. `form_request` records each clean request's serialized
bytes and characters; `request_size` and profile totals count every extraction call.
Existing size and search caps apply.
The wire cap is eight requests, including provider-mode recovery and normal retries.

### Flat fill forms

`pilot_find` returns the same flat scalar schema offered by `pilot_fill`. Supply
numbers in mm, integer counts, booleans, and listed string choices; omit unstated
optional features. Every recipe file owns its field names and slot paths in
`schema.x-pilot-fill`. Pilot maps these fields to the existing typed slots in one
shared conversion. `pilot_build` also accepts the earlier nested arguments.

For a plate use `length`, `width`, `thickness`, `plane`, `centred`,
`bore_diameter`, `bolt_hole_count`, `bolt_hole_diameter`,
`bolt_circle_diameter`, `bolt_start_deg`, `grid_rows`, `grid_columns`,
`grid_pitch_x`, `grid_pitch_y`, `grid_hole_diameter`, `corner_kind`,
`corner_size`, and `corner_edges`. A bolt pattern needs its count, hole diameter,
and circle diameter; a grid needs both counts, both pitches, and its hole diameter.
Corners need size; kind defaults to fillet (round) and edges to vertical when
`corner_size` is supplied. Both defaults are declared in `recipes/plate.md`
and audited individually as `fill_defaulted`. An enclosure lip uses
`lip_width`, `lip_height`, `lip_side` (default inner). A supplied feature field
activates that feature; incomplete features name the missing field in the error.
No missing dimension or count is guessed.

The shared flat mapper accepts agreeing redundant representations across recipes:
uniform profile `thickness` alongside either or both leg thicknesses; equal flat
and nested slots (including shared pattern hole diameters); radius and diameter
aliases for cylinders, spheres, tubes and revolved rectangles; and linear pattern
spacing alongside its first-to-last total length. Diameter aliases map to half
their value via the file's `x-scale`. Conflicts name the supplied fields and values.
Numeric agreement permits relative roundoff of 1e-9; all supplied flat values
retain their individual bounds before redundant fields are removed. Array or object
enum values are refused with allowed choices and their meanings.

`tests/fixtures/fill-audit4.json` preserves only the arguments from the four
`fill_invalid` events in the public audit. The profile now builds; the first plate
builds with corner defaults; its edge-list retry gets the explicit enum error.
The enclosure first fill still needs `lip_height` and uses unsupported `top_left`:
its recorded retry supplies height 2 and open top and builds. Inferring that
missing dimension would violate the contract. The public CLI replay verifies
the profile, the defaulted and corrected plate (42446 mm³), and the enclosure retry.

Vector slots use scalar coordinates: `delta_x/y/z`, `corner_x/y/z`, and
`origin_x/y/z` where offered. Cross-drilled boxes offer `hole_x_diameter`,
`hole_y_diameter`, `hole_z_diameter`, with optional `hole_x_offset_u`,
`hole_x_offset_v`, `hole_x_depth` (and equivalent Y/Z-hole fields).
Offsets are from the face centre in the directions above; omit them for centred
holes and omit depth for through holes. At least one hole diameter is required.
Retired flat `hole_x_position_x/y/z` keys are coerced independently: centre
coordinates and axial entry faces are ignored; other coordinates strictly inside
the face become offsets. Face edges are rejected with an offset example.
Nested `holes[].position` still accepts a complete world point.

Before validating `pilot_fill`, numeric strings such as `"56"`, `"56 mm"`,
and `"2 in"` are converted to numbers (lengths to mm), `"true"`/`"yes"`
and `"false"`/`"no"` become booleans, and enum case and common synonyms
(round/fillet, bevel/chamfer, corners/side/vertical) are normalized. Each changed
field is audited as `fill_coerced`. Counts and angles do not accept length units.
A number supplied under an old container name, such as `bolt_circle: 56`, is
rejected with the scalar field names to use. Each compact fill schema plus its
system line stays below 1,200 characters, including the `decline` field.

The independent public context regressions live in
`src/model/tests/context_tests.rs`. They build a plate and peg, a joined boss and
hole, a rib, two separately created bodies, and a component pair through the
public MCP binary. A scripted extractor reads only the clean wire request; the
original names-only implementation declined the row case because it could not
identify "these". The tests now drive row, grid and circular forms through client
execution and measured completion, exercise every existing-target form, verify
selection precedence and inspection failure, and check extraction-size accounting.
They do not use private evaluation tasks or model-specific prompt examples.

The separate `joint` form supports all public SolidCraft joint and relation types.
It is offered only for joint intent naming a non-revolute type, motion limits,
or centres/origins. Plain contact pin/hole requests retain the unchanged
`revolute_joint` form. General joints use an independent gate and contribute no
BM25 documents; existing recipe scores and their shared statistics stay unchanged.
Only the general joint form receives named origins in its design summary.

Set `joint_type`, `component_a` and `component_b`; names resolve by exact name,
normalized name or an unambiguous substring. Omit names only when candidates are
unique. Hinge/pin, prismatic/slide and ball-and-socket/spherical normalize to
revolute, slider and ball. `axis` accepts X/Y/Z words or unit vectors, defaulting
to Z. Each component's combined box centre supplies its origin;
`point_a`/`point_b` instead select named `joint.list` origins belonging to the
selected occurrence and retain their frames. Pending moves must be applied first.

Supply `limit1_min`, `limit1_max`, etc. in `joint.list` value order. Both ends
are required, finite within ±1,000,000, with min <= max. Omitted pairs remain
unlimited; surplus pairs fail. The general joint form has a 3,000-character
compact schema/system-line budget.

| Type | Ordered motion values and units |
| --- | --- |
| rigid | none |
| revolute | angle (deg) |
| slider | distance (mm) |
| cylindrical | angle (deg), distance (mm) |
| pin_slot | angle (deg), slide (mm) |
| planar | x (mm), y (mm), angle (deg) |
| ball | yaw, pitch, roll (deg) |
| parallel | x, y, z (mm), angle (deg) |
| perpendicular, angle | x, y, z (mm), turn, spin (deg) |
| point_on_plane | x, y (mm), yaw, pitch, roll (deg) |
| point_on_line | slide (mm), yaw, pitch, roll (deg) |
| line_on_plane | x, y (mm), turn, spin (deg) |

For example:
`{"joint_type":"cylindrical","component_a":"Base","component_b":"Link","axis":"Z","limit1_min":-45,"limit1_max":45,"limit2_min":0,"limit2_max":20}`.
