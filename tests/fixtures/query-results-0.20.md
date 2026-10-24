Public SolidCraft 0.20 query results (clean-room fixtures)

- `inspect-volume-granite-0.20.json` and `inspect-volume-qwen-0.20.json`
  are the complete `execute document.inspect {measure:true}` result objects
  extracted from the respective `*-19-volume.json` repro-h2 recordings supplied
  for this fix. Their per-body `volume_mm3` is 9520; no measure-style `total`
  or `body_count` fields exist in those inspection results.
- `measure-volume-0.20.json` is the complete MCP result wrapper captured through
  the supplied public 0.20 binary. Calls: `execute solid.box` with
  `{length:31.25,width:22.4,height:13.6,name:"Block"}`, then `measure {}`.
  Measure reports per-body `volume_mm3`, `body_count`, and `total.volume_mm3`.

Tests use independently authored requests, keeping only the public result data
from the recordings. The fixtures exercise raw inspection JSON and MCP text
content wrappers; no SolidCraft source was consulted.
