# Pilot changelog

## Unreleased

- Profile forms keep stated inside gaps and T stem heights as driving sketch dimensions, and support lipped C sections, dimensioned inside fillets on angles, and symmetric extrusion. All remain fully constrained.
- Offer inside gaps, stem heights, lips, symmetric extrusion and inside fillets only when the profile request names them.

## 0.1.0-beta.7 (2026-10-28)

- Joints: one form now covers every SolidCraft joint type (rigid, revolute, slider,
  cylindrical, pin-slot, planar, ball and the relations), at box centres or named origins,
  with aligned axes and motion limits. A plain pin-in-hole joint keeps its short form.
- Plates can carry pegs, bosses and hole rows from one request: rectangular or round
  bases with raised features or holes in a row, a grid or on a circle.
- Profile forms read the section and plane from common wording ("angle", "channel",
  "T-section", "XZ plane"), and a stated web or flange thickness wins over a general one.
- Requests that start with "align" are left to normal tools.

### Details

- Requests that start with "align" are left to normal tools instead of filling a creation form.
- Add a separate general joint form for all SolidCraft joint types, named origins and motion limits, preserving the contact revolute form and existing recipe rankings.
- Plate forms build rectangular or round bases with joined pegs/bosses, a single offset boss, and centred rows, grids or circles of raised features or holes. New-part pattern requests route to plate; existing-part edits retain pattern forms. Grid fields specify X columns, Y rows and centre spacing.
- Profile forms accept common wording for the section (angle, channel, tee, rectangular) and the plane (["X","Z"], "XZ plane"), and a specific web or flange thickness now takes priority over a general thickness instead of being refused.

## 0.1.0-beta.6 (2026-10-27)

- Field tester fixes: a cube with holes through its faces now builds from the form (holes
  are centred unless offsets from the face centre are given, and blind depths work), and a
  centred honeycomb plate no longer fails (Pilot builds it at the origin and moves it).
- Pattern forms (row, grid, and around an axis) accept what small models write: axes as
  words or vectors, the only body in the design, and common target names. The overlap check
  measures the patterned feature, rotates copies for circular patterns, and is skipped
  (logged) when a feature can't be measured.
- SolidCraft 98: the model's action budget counts only its own steps.

### Details

- Check circular pattern bounds by rotation, accept vector strings, and audit skipped checks for unmeasurable features.
- Accept common axis and body descriptions in pattern forms, and check spacing against the patterned feature.
- Build honeycomb plates at the origin before placing them, fixing failed centred patterns.
- Centre cross-drilled holes by default; accept body-centre points, face-centre offsets and blind depths.
- SolidCraft 98: the model's action budget counts only its own steps, a dialog left open is
  filled or cancelled cleanly, and repeated screens are shortened.

## 0.1.0-beta.5 (2026-10-26)

- Recipes with Windows line endings now load.
- Recipe forms: Pilot finds the recipe that fits a request and asks the model only to
  fill in its typed form, in a short separate request. Pilot builds the part from the
  form with checked SolidCraft commands. Small local models now build many common
  parts they could not build before.
- Uncertain requests get one short question: which of a few forms fits, or none.
- New parts: L, T, U, C, and rectangle profiles; plates with bores, bolt circles, hole
  grids, and rounded or chamfered corners; open enclosures with a locating lip; boxes
  with holes through one, two, or three axes; honeycomb hole patterns; and a revolute
  joint on a pin.
- Patterns of an existing body or feature (in a row, a grid, or around a circle), with
  a hint when a model types many equal copies instead.
- Sketch planes: a one-line fact on how sketch coordinates map to world axes (on XZ,
  the sketch's second axis is world −Z), and a hint when a profile lands outside the part.
- Fewer input tokens: older tool results are trimmed inside a long task (the newest two
  stay whole), and unused tool schemas are no longer sent. A 24-call task sends 64%
  fewer characters. `--no-trim` and `--trim-keep` control it; `--offer-recipe-tools`
  sends the find and build tools on every call again.
- One retry for an Ollama HTTP 500 that arrives after a full reply.
- Facts and hints are sent once per conversation.
- `--no-fill` keeps the recipe tools but skips the forms; `--no-shortcuts` turns off
  forms, routing, and recipe tools.

### Details

- Offer forms on every fresh user turn, including repeated text across conversations, using history without retaining turn identities.
- Inspect existing geometry before clean fill/route requests, include bounded design context and selection, resolve implicit targets, and shell existing rectangular bodies without rebuilding them.
- Ask for fill and route forms in clean extraction requests, retry off-form/text replies once, and audit each request’s size.
- Discard off-form fill/route calls and prevent repeated form offers after retries, history trimming, or tool execution in the same user turn.
- Restore direct recipe fills at score 4.5, route only scores from 1.5 to below 4.5, and keep declined fills on normal tools.
- Keep recipe tools opt-in with `--offer-recipe-tools`, always send relevant facts/hints once per conversation, and shorten the `how` schema.
- Lay out honeycomb rows along the plate's longer side by default, preserving explicit orientations and matching swapped dimensions by rotation.
- Calibrate recipe retrieval with bounded synonyms, stemming and field hits, and route uncertain requests through one audited recipe choice before filling.
- Embed recipes directly with include_str! so checkouts sharing a build directory each use their own recipe files; test the file list for omissions.
- Trim older tool results within the current user turn, keep the newest two (`--trim-keep`), summarize command docs, and audit per-request `trimmed_chars`.
- Accept consistent redundant fill fields, audit unambiguous corner defaults, explain enum container errors, and allow two bounded fill retries.
- Flatten every recipe fill form into scalar fields with audited coercions and explicit partial-feature errors; preserve nested build arguments.
- Explain fill fields and numeric validation fixes, default unstated placement and inner lips, and cap each recipe form at 1,200 characters.
- Find recipes automatically and ask the model to fill one typed form first, with one validation retry, audited fallback, and `--no-fill`.
- Map sketch planes to world axes with audited hints, flag unused construction planes, and build cross-drilled boxes with world-space through holes.
- Trim older upstream tool history by default, preserve recipe evidence, and audit per-request character sizes with profile mean/max; opt out with `--no-trim`.
- Cut staggered honeycomb plates in one recipe, with across-flats/corners sizing, bounded fill or exact counts, web and border, and measured public CLI verification.
- Build linear, grid and circular body or last-feature patterns through typed recipes, with overlap guards, measured verification and a deduplicated repeated-copy hint.
- Search embedded file recipes with `pilot_find` before `pilot_build`, with audited misses and bounded audit-to-review sequence drafts.
- Keep inspect-first recipes working within the Assistant's tool-message size limit, and accept "fully defined" profile wording.
- Let models build profiles, plates, enclosures and inspected revolute joints through one typed `pilot_build` tool; disable with `--no-shortcuts`.
- Build compound plates with bores, bolt circles, grids and corner rounding, and guide failed edge picks.
- Build open shelled enclosures with optional floor thickness and joined locating lips on either wall edge.
- Connect a hole over a pin with a verified revolute joint and angle drive, with a contact-circle hint for moved or flipped pivots.
- Retry one upstream HTTP 500 per model turn, with audit excerpts and profile counts.
- Build fully constrained L, T, U, C, and rectangle profiles from ordinary wording, with one closed polyline, driving dimensions, and extrusion.
- Add upfront closed sketch guidance to model requests and MCP tool descriptions, respecting --no-hints.
- Add deduplicated hints for dimension entity errors and existing inferred sketch constraints.
- Add one bounded, audited box placement fact before the model's first call for
  centred-feature requests, and to MCP action tool descriptions. `--no-hints`
  disables it; existing conversation facts are not repeated.

## 0.1.0-beta.4 (2026-10-24)

- Exact matches are answered without the model by default; `--no-direct-recipes` opts out.
- Centred holes in cubes and boxes, from a user report, with box placement hints.
- Type `/report` in the Assistant for a local redacted report and issue link.
- A public field-report benchmark set.

### Added

- Type `/report` in the Assistant to save a local redacted report, with a prefilled
  issue link and a paste-ready Discord #pilot-help summary, without calling a model.
- A public field-report benchmark set, starting with a cube with a centred hole,
  checked by volume, with a corner-hole negative.
- After a box is created, both faces add one audited hint with its extent, centre,
  and top face centre, computed from the call's numeric parameters.

### Changed

- Exact-match answers are on by default on the model face, through the same client
  approval and SolidCraft validation. `--no-direct-recipes` sends everything to the
  model; `--direct-recipes` remains a no-op alias and conflicts with the opt-out.
  `--upstream none` no longer needs a flag and rejects the opt-out. SolidCraft 98
  keeps direct answering disabled; `--planner` remains independent.
- Benchmark Pilot launches disable direct recipes to preserve earlier comparisons.
- The centred-hole recipe accepts cubes with one side, boxes and blocks, “draw”,
  “hole through the center”, and an unqualified hole size as its diameter.

## 0.1.0-beta.3 (2026-10-24)

- SolidCraft 98 support through its public remote protocol.
- The shared `dutchman` launcher and installers for Linux, macOS, and Windows.
- A Discord link and a help section in the README.

### Added

- Bundle the shared `dutchman` launcher so a Pilot-only install supports
  `dutchman pilot` alongside `dutchman-pilot`, with the launcher source revision recorded.
- Add Linux, macOS, and Windows installers with custom bin directories, dry runs,
  older-launcher replacement, and preservation of full Dutchman installs.
- Check installer behavior on all three systems and command dispatch in release archives.
- SolidCraft 98 1.2 or later support through its public remote protocol: MCP screen tools,
  a bounded model driver, offline help, and an isolated headless process option.
- SolidCraft 98 model dialect with explicit calls and repairs; catalogue recipes
  remain disabled for this application.
- Eleven public SolidCraft 98 benchmark tasks with references, state checks,
  do-nothing expectations, and deliberate negatives under `bench/sets/sc98`.
- A Dutchman CAD Discord link and a "Help and community" section in the README.

## 0.1.0-beta.2 (2026-10-24)

- Direct exact-match answers with `--direct-recipes`, through SolidCraft's approval.
- Recipe matching for ordinary wording, shape synonyms, and metric or inch lengths.
- Text tool mode for models without native tools; plan-text recipes require `--planner`.
- Benchmark start files for repeatable comparisons.
- Local, redacted bug reports from captured turns or recorded requests.

### Added

- `dutchman-pilot report` writes a redacted local report and issue URL for review
  and submission. Includes five memory-only diagnostic turns, offline recipe replay,
  and optional request text with `--include-request`. `--open` opens the browser;
  `--no-report-buffer` disables capture on the model face.
- Typed recipes for primitives, tubes, centred and positioned holes, rectangular
  and circular hole patterns, sketch extrusions, edge finishes, move, rename,
  mirror, shell, revolve, uniform scale, and measured queries.
- `--direct-recipes` proposes normal tool calls and returns measured replies
  marked "Exact match (no model):" after approval. Supports streaming,
  `--upstream none`, separate logs and counters, and model recovery on failure.
- Direct edits can inspect once to resolve a unique current body before proposing
  changes; edge finishes can request topology before selecting edges.
- Text tool mode describes available tools and adapts tool history when native
  tools are unsupported. Remembers explicit provider rejections and retries once.
- Public v1, v2, and v3 benchmark sets, plus constraint, joint, revision, recovery,
  and unit checks, with reference answers and incorrect-result checks.
- Forty-eight SolidCraft 0.19 starting designs, rebuild support, and migration checks.
- Broader recipe checks covering ordinary requests, refusals, long prompts,
  inspection results, and measured geometry through the public binary.

### Changed

- `--planner` enables recipe suggestions, prose conversion, continuations, and
  completion checks on the model face. `--no-plan` and `--no-planner` remain
  accepted no-op aliases for the default. MCP suggestions are unchanged.
- `--direct-recipes` works independently and can accompany `--planner`. Partial
  requests receive suggested plans only with `--planner`; remaining work stays
  with the model. Argument repair, text-call pickup, command lookup, hints, and
  text tool mode keep their existing defaults.
- Recipes accept polite requests, ordinary shape and action synonyms, reordered
  dimensions, joined units, simple fractions, signed axes, and explicit radius
  or diameter wording. Decimal values and quoted names retain their meaning.
- Hole layouts accept end and corner insets; circle extrusions accept the plane
  first. Mirror requests can keep or remove the original, and scale requests can
  double a body about the origin.
- Completion checks compare measured results once, including after accepted
  calls, without repeating accepted creation steps or increasing retry limits.
- Benchmarks score the resulting design when the model exhausts its request limit.

### Fixed

- Report request shapes use redacted text and treat replacement labels as opaque
  tokens, keeping private vocabulary out of decline and token diagnostics.
- Report redaction classifies file paths before applying token entropy checks,
  including Windows and home-relative paths.
- Direct volume and body-count answers use complete measured results, including
  current-solid references and requested cubic-unit conversions. Incomplete,
  conflicting, partial, or ambiguous measurements return to the model.
- Plane slots accept only XY, XZ, or YZ; name and axis extraction stay bounded.
- Missing dimensions, ambiguous bodies, competing intents, negations, and
  unsupported modifiers leave requests with the model. Edits require valid
  inspection results; hole recipes require explicit positions.
- Partial-request suggestions make clear that nothing has run and that the whole
  request still requires approved calls and returned results.

## 0.1.0-beta.1

First public early beta. Results are still being measured; expect rough edges.

- Preserve parameter alternatives and supplied radii; forbid semantic argument
  renames, require exact container parsing, reshape only unambiguous line endpoints,
  hand back invalid enum values, and separate typed templates from concrete examples.

- Public clap-derived `cli::Command` and synchronous `cli::run` for embedding
  Pilot subcommands with the same flags and explicit exit codes.
- `cli::with_bin_name` and `cli::run_named` so an embedding program's help names
  its own command.
- Local OpenAI-compatible model proxy for SolidCraft, with buffered streaming,
  explicit JSON tool-call conversion, schema-guided argument repair, bounded
  retries, provider error messages, and rewrite logs.
- MCP stdio proxy with command discovery, corrected-call suggestions, state
  hints, truthful paging, and deterministic plans. Suggestions require an
  explicit call from the client; prose never executes a command.
- Catalogue-based planning for common requests, per-model profiles, optional
  prompt guidance, and Ollama model settings.
- Corpus evaluation with repeated trials, bucket floors, recorded replies,
  offline rescoring, and a bounded pass cache for paired measurements.
- Portable Rust core for catalogue search, planning, repair, and hints, including
  a wasm32 build without host I/O.
- Release archives with both command-line binaries, documentation, the starter
  corpus, license notices, and SHA-256 checksums.
