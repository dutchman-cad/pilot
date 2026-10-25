<!-- SPDX-License-Identifier: Apache-2.0 -->
# Field reports

This set holds prompts from user field reports, preserved verbatim.

`cube-six-face-holes` is the Discord report of three crossing 8 mm holes in
a 20 mm cube. Its exact remaining volume is
`8000 - 960π + 512√2 = 5708.148396488824 mm³` (0.1% kernel tolerance).
Three distinct cylindrical axes give six round face openings; the existing
`round` checker merges the two ends of each hole, so its count is three.
Requiring one Box and three Hole features within four total features leaves
no room for Sketch or ConstructionPlane features. The public-binary Rust test
also checks the timeline directly.

`cube-centre-hole` starts empty and asks for a 20 mm cube with a centred
5 mm diameter through hole. The check requires one body, a 20×20×20 mm
bounding box, and volume 7607.30 mm³ within 0.5 mm³ (`tol` is relative).
The reference cuts at the top-face centre (10, 10, 20). The negative cuts
at the origin corner, leaving about 7901.8 mm³, and must fail. Doing nothing
must also fail. References and recorded fixtures use only public CLI/MCP calls.

Run the reference, negative, and do-nothing modes documented in
[the benchmark guide](../../README.md) with `--set field`.
