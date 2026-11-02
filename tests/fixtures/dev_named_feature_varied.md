These 90 invented development requests and parts were authored for the public
SolidCraft CLI. No benchmark task data is used. Each row includes its complete
construction commands, edit intent, expected body volume and world bounds.

The oracle uses union/subtraction arithmetic: rectangular prisms use L*W*H;
cylinders use pi*r*r*h; counterbores subtract the bore plus the annular recess.
Buried pads count only material above their support, and elevated cuts count
only material below the entry surface. Symmetric extrusions contribute half
their total distance above the support. The second body has its own independent
oracle. Fixture dimensions and body names vary across the examples.

Two-sided extent height and finite depth on a through hole require clarification
and leave the original measured shape unchanged. Lowering a support with a
fixed cap is rejected before mutation. Enlarging that support without moving its
cap must report an unverified edit; the oracle explicitly measures the actual
union, including overlap consumed under the stationary cap. This exercises a
successful timeline.edit that must never be described as a successful request.

Bounds tolerance is 0.001 mm. Analytic cylinder volume tolerance is 0.02% of body
volume because the public CLI's circular boolean tessellation approximates pi.
The runtime verifier separately uses 0.2% of the requested volume *change*, with
a 0.01 mm^3 floor, so a large base cannot mask an incorrect small edit.

Run with PILOT_SOLIDCRAFT_CLI pointing to a public solidcraft-cli binary:
  cargo test --lib named_feature
The existing fixture covers primitive, extrude and hole field encodings,
including nested extent and counterbore edits; this fixture adds geometry and
wording diversity. The tests replay commands through MCP execute, the same
public command surface as the CLI.

Public CLI field encoding audit (SolidCraft 0.29.0):

| Listed feature type | Definition fields exercised through timeline.edit |
| --- | --- |
| BoxFeature | length, width, height |
| CylinderFeature | radius (diameter converted to radius), height |
| ExtrudeFeature | extent: {distance}, extent: {start_offset} |
| HoleFeature, simple | diameter, depth, position: [x,y,z] |
| HoleFeature, counterbore | hole: {cb_diameter}, hole: {cb_depth} |

Nested definition objects preserve sibling fields through the public merge API;
no dotted parameter identifier is sent. Through-all depth and independently
editable sides of a two-sided extent are omitted from the form rather than
advertised as an ordinary finite depth/height. Symmetric and counterbore edits
use the inspected form, as do vague larger/smaller requests.
