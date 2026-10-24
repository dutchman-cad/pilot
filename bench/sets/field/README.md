<!-- SPDX-License-Identifier: Apache-2.0 -->
# Field reports

This set holds prompts from user field reports, preserved verbatim.

`cube-centre-hole` starts empty and asks for a 20 mm cube with a centred
5 mm diameter through hole. The check requires one body, a 20×20×20 mm
bounding box, and volume 7607.30 mm³ within 0.5 mm³ (`tol` is relative).
The reference cuts at the top-face centre (10, 10, 20). The negative cuts
at the origin corner, leaving about 7901.8 mm³, and must fail. Doing nothing
must also fail. References and recorded fixtures use only public CLI/MCP calls.

Run the reference, negative, and do-nothing modes documented in
[the benchmark guide](../../README.md) with `--set field`.
