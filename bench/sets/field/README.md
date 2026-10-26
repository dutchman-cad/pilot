<!-- SPDX-License-Identifier: Apache-2.0 -->
# Field reports

This set holds prompts from user field reports, preserved verbatim.

`cube-six-face-holes` is the Discord report of three crossing 8 mm holes in
a 20 mm cube. Its exact remaining volume is
`8000 - 960π + 512√2 = 5708.148396488824 mm³` (0.1% kernel tolerance).
Three distinct cylindrical axes give six round face openings; the existing
`round` checker merges the two ends of each hole, so its count is three.
The check scores the geometry the user asked for, so a correct answer built
from sketches and cuts passes too. Pilot's own recipe test also checks that its
timeline holds no Sketch or ConstructionPlane features.

`cube-centre-hole` starts empty and asks for a 20 mm cube with a centred
5 mm diameter through hole. The check requires one body, a 20×20×20 mm
bounding box, and volume 7607.30 mm³ within 0.5 mm³ (`tol` is relative).
The reference cuts at the top-face centre (10, 10, 20). The negative cuts
at the origin corner, leaving about 7901.8 mm³, and must fail. Doing nothing
must also fail. References and recorded fixtures use only public CLI/MCP calls.

Run the reference, negative, and do-nothing modes documented in
[the benchmark guide](../../README.md) with `--set field`.

`honeycomb-plate` preserves the public rectangle/hexagon report. The default
AF lattice has 19 staggered rows of 34 hexes (646 through cuts) along the
longer side (Y for the given 150×300 plate),
8.5 mm centre pitch, and 7.361215932 mm row pitch. The reference uses two seed
cuts and two rectangular feature patterns, never separate hex bodies. Its
check requires one body, a 150×300×3 mm box, the computed volume
`135000 - 646 × sqrt(3)/2 × 7² × 3`, and 3882 planar faces
(6 plate faces + 6 per hex), enforcing the hex count through the existing face
predicate. `hex_count` records that count as task metadata. The Rust public CLI
test also reconstructs every hole loop from finished B-rep edges, measuring
minimum web 1.5 mm and minimum border 4.125 mm (at least the requested 4 mm).
The reference previously ran rows along the shorter X side; it now follows the
longer-side convention, retaining length along X and width along Y. Explicit
pointy-top/flat-top requests still override this default.
Y-row fields are built with X rows and then rotated through the public move
command, avoiding the 0.20 CLI's planar comparison budget for large Y arrays.
Doing nothing fails this task.
