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
pointy-top AF lattice has 39 staggered rows of 16 hexes (624 through cuts),
8.5 mm centre pitch, and 7.361215932 mm row pitch. The reference uses two seed
cuts and two rectangular feature patterns, never separate hex bodies. Its
check requires one body, a 150×300×3 mm box, the computed volume
`135000 - 624 × sqrt(3)/2 × 7² × 3`, and 3750 planar faces
(6 plate faces + 6 per hex), enforcing the hex count through the existing face
predicate. `hex_count` records that count as task metadata. The Rust public CLI
test also reconstructs every hole loop from finished B-rep edges, measuring
minimum web 1.5 mm and minimum border 5.625 mm (at least the requested 4 mm).
Doing nothing fails this task.
