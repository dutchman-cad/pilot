These 36 invented development edits cover round pads and recesses on Compass
parts. They use the public CLI only and contain no held-out task data.

Each part starts as a rectangular carrier L*W*H. A cylindrical pad adds pi*r*r*h;
a cylindrical or circular extruded recess subtracts pi*r*r*h. The expected
volumes and bounds were computed directly from these construction dimensions,
independently of planner slots, geometry witnesses, or parameter echoes. The
fixtures also specify the expected edited timeline field and its numeric value.

The first 30 requests identify height, depth, distance, or diameter. The last
six require a form because their words do not identify one dimension. Dimensions
and carrier names vary; height/depth edits retain the original radius and round
size edits convert the requested diameter to radius. Circular extruded recesses
exercise the extent.distance encoding and a cylindrical witness for volume.

Run the public CLI replay with PILOT_SOLIDCRAFT_CLI set, using:
  cargo test --lib named_feature

Companion tests run without a CLI. They inject unchanged measurements and masked
tool errors, assert the inverse timeline edit, verify restoration before a
single alternative, and reject model/native-tool handback, including resends.
A public CLI regression also checks restored measurements and the original
feature definition when a later attachment makes the requested edit unverifiable.
