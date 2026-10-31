# Development measurement: exact plate patterns

These are independently authored requests in `tests/support/dev_exactgrid.rs`,
not benchmark or held-out prompts. Tests use the public binary catalogue.

Before changing the parser, the first development run claimed:

| Group | Claimed | Form calls identical |
| --- | ---: | ---: |
| Numbers before grid nouns / named face | 0 / 12 | n/a |
| Centimetre grid with chamfer | 0 / 12 | n/a |
| Inch square with bolt circle | 0 / 12 | n/a |
| Explicit-origin controls | 2 / 3 | 2 / 2 |

All 14 original near-misses declined without errors. The third control, with
`one corner at origin` and a circle, declined: the original circle count search
mistook `one` in the placement for the hole count. The original parser remains
the first path; normalization only applies after it declines.

The final fixtures vary the inch counts from five through seven to make all 36
primary prompts distinct. Eight additional cases exercise plane choices, base
centring, explicit mixed units and fractional inch grids. Tests compare complete
call arrays with the frozen plate form, including order, arguments and inspection.
They also check the three origin controls and 28 near-miss/limit cases.

Six CLI replays cover both grid orientations, chamfer before/after the grid, and
two inch circles. Bounds must agree within 0.001 mm; analytic volumes use SPEC.md's
0.1% or 0.01 mm³ tolerance. This accommodates the public binary's small cylindrical
volume approximation (about 1 mm³ on a 210,000 mm³ grid plate). Hole positions and
diameters are also checked by exact call equality; bounds and volume alone cannot
detect a swapped grid orientation.
