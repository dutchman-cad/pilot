# Native call repairs

For requests without a recipe, Pilot can repair these explicit calls conservatively:

- Invented `solid.cut`, `solid.subtract`, `solid.difference`, `solid.union` and spelling variants become `solid.combine` with `target`, `tools` and `operation`. Both sides must name distinct bodies; Pilot never guesses the target or swaps the operands.
- Extrude operations `new solid` and `remove`, which SolidCraft rejects, become `new` and `cut`. Words SolidCraft accepts itself (`add`, `union`, `subtract`, `new body`) and native operations, including `auto`, stay as written.
- A misplaced sketch origin can be moved into an explicit plane when a principal plane fixes its axes. Explicit axes or face points can be nested when no other plane competes. A normal alone does not fix sketch rotation. Numeric `X=10`, `Y=4`, or `Z=5` planes become `YZ`, `XZ`, or `XY` plus an offset. Top-level `offset` is already valid SolidCraft syntax and is preserved.
- Hole positions `{x,y,z}` become world coordinate arrays; numeric coordinate strings can become numbers. Length expressions stay expressions. A 2D position and a face label still need the missing world coordinate: Pilot never invents it.
- A single move body becomes a one-element `bodies` list. Fillet indices become `{body,index}` only with an explicit body. Edge names need an index or world point from inspection; `Edge1` is never assumed to mean index 0 or 1.
- A circle or rectangle blocked by a failed `sketch.create` in the same batch gets a diagnostic naming that earlier failure and its reason. This also covers steps SolidCraft explicitly reports as not run.

Every rewrite is audited. Ambiguous calls get a short error and a corrected form using the supplied values; angle-bracket placeholders identify information still needed. The model face repairs calls before returning them for client approval and native validation. MCP retains its suggestion-only contract: after a failed call it offers the corrected failing step, never automatically replays it or the batch.

Development coverage uses independently authored examples in `tests/support/callrepair_cases.rs`: 42 repair cases, at least 18 refusals, HTTP and MCP checks, and public CLI replays checking body counts, volumes and placement. No recipe descriptions or form wording are changed.

With complete development geometry, public CLI replay rejected 27 of the 42 original calls; all 42 repaired calls built and met the measured intent. Some shape variants already work in the native CLI, so equivalent normalization does not imply that the original always failed.
