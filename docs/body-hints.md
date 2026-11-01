# Separate-body guidance

For native SolidCraft tools in the model face, Pilot adds a short `Body check`
line to a tool result when public inspection confirms that two separate solids
touch or overlap during a one-part build without a recipe. The line includes an
exact `execute solid.combine` join using the inspected body names. Pilot never
runs the join, changes a valid model call, or refuses it.

This runs in model-only (N) and default modes. `--no-hints` disables it, including
plain-style comparisons. Requests mentioning separate parts, bodies, copies,
components, assemblies or mechanisms suppress it, as do counted solids
("eight boxes") and conversations a recipe built. Recipe descriptions and form
wording are unaffected.

Pilot reuses complete document and contact measurements from tool history.
After a creation, it can request `document.inspect`, then prune pairs by bounds
and confirm overlap with `inspect.interference` or contact with
`inspect.measure`. Bounds alone never establish contact. Reads go through the
client's normal tool approval path. Each user turn permits at most 12 extra
commands, including at most four document inspections; each interference read
checks only two bodies with a three-second budget. Inspection considers at most
24 bodies. Failed reads are not retried for the same unchanged state.

A final answer with multiple bodies receives at most one hand-back per user
turn, including across client continuations. It repeats the measured join fact
when available; otherwise it reports the count and asks the model to verify
contact and finish or explain. It never proposes joining unverified pairs.
Facts are deduplicated within the turn. A later mutation invalidates old geometry
evidence. Errors, partial inspections and ambiguous names cannot supply a join.

Development examples live in `src/model/tests/body_hint_tests.rs`: 20 independent
CLI builds verify the suggested joins leave one closed body with one shell;
15 separation/assembly requests check suppression. Additional focused tests
cover normal valid calls, plain style, bounded reads and the per-turn hand-back.
Run with the public binary configured through Cargo's
`env.PILOT_SOLIDCRAFT_CLI`, using the build queue.
