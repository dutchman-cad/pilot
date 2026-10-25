---
{
  "id": "honeycomb_pattern",
  "description": "Cut a plate with a staggered honeycomb of hexagonal through holes, bounded web and border; fill or exact rows and columns.",
  "examples": [
    "plate with honeycomb hexagonal through cuts",
    "staggered hexagon pattern edge to edge spacing border",
    "honeycomb plate flat to flat AF width",
    "hexagonal holes point to point corner to corner",
    "fill a plate with hexes leaving a border"
  ],
  "schema": {
    "$defs": {
      "Angle": {
        "description": "Degrees",
        "maximum": 360,
        "minimum": -360,
        "type": "number"
      },
      "Count": {
        "maximum": 64,
        "minimum": 1,
        "type": "integer"
      },
      "Length": {
        "description": "Positive millimetres",
        "exclusiveMinimum": 0,
        "maximum": 1000000,
        "type": "number"
      },
      "Plane": {
        "enum": [
          "XY",
          "XZ",
          "YZ"
        ],
        "type": "string"
      },
      "Point3": {
        "description": "[x,y,z] millimetres",
        "items": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number"
        },
        "maxItems": 3,
        "minItems": 3,
        "type": "array"
      }
    },
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "additionalProperties": false,
    "properties": {
      "border": {
        "maximum": 1000000,
        "minimum": 0,
        "type": "number"
      },
      "centred": {
        "type": "boolean"
      },
      "columns": {
        "maximum": 5000,
        "minimum": 1,
        "type": "integer"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "mode": {
        "enum": [
          "fill",
          "exact"
        ],
        "type": "string"
      },
      "orientation": {
        "enum": [
          "pointy_top",
          "flat_top"
        ],
        "type": "string"
      },
      "plane": {
        "$ref": "#/$defs/Plane"
      },
      "rows": {
        "maximum": 5000,
        "minimum": 1,
        "type": "integer"
      },
      "size": {
        "$ref": "#/$defs/Length"
      },
      "size_kind": {
        "enum": [
          "across_flats",
          "across_corners"
        ],
        "type": "string"
      },
      "thickness": {
        "$ref": "#/$defs/Length"
      },
      "web": {
        "$ref": "#/$defs/Length"
      },
      "width": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "length",
      "width",
      "thickness",
      "size",
      "size_kind",
      "web",
      "border"
    ],
    "type": "object",
    "x-pilot-expander": "honeycomb_pattern"
  },
  "expander": "honeycomb_pattern"
}
---

Cut a plate with a staggered honeycomb of hexagonal through holes, bounded web and border; fill or exact rows and columns. Built by the `honeycomb_pattern` expander.

Supply `length`, `width`, `thickness`, `size`, `size_kind` (`across_flats` or
`across_corners`), positive `web`, and nonnegative `border`, all in millimetres.
`plane` defaults to XY, `centred` to false, `orientation` to `pointy_top`, and
`mode` to `fill`. `exact` requires both integer `rows` and `columns`; `fill`
accepts neither. Counts include seeds. Defer on nonpositive web, no fitting hex,
more than 5000 cuts, or an exact field that would cross the border.

Pointy-top has vertices along sketch Y: centre pitch along X = AF + web,
row pitch along Y = (AF + web) × sqrt(3)/2. Odd rows move half a centre pitch.
Flat-top rotates the lattice: rows advance along X and columns along Y.
Across-corners size converts to AF = size × sqrt(3)/2. The border is a minimum
edge-to-hex clearance, not a centre inset. Fill keeps all complete hexes within
the border, with potentially shorter odd rows, then centres the entire field's
bounding box in the available rectangle. Exact mode centres the specified field
and rejects any clipping. A narrow fill may have empty odd rows.

The plate footprint starts at the origin, or is centred about it when requested.
In-plane axes are X/Y on XY, X/Z on XZ, and Y/Z on YZ. Thickness extends along
+Z, -Y, and +X respectively. Sketch planes are explicit public command frames.
Two hexagon seed sketches and through cuts, each followed by at most one
rectangular feature pattern, create one plate body in at most 12 commands
including measurement, regardless of cut count. Existing bodies are never cut.

Example: `{"recipe":"honeycomb_pattern","args":{"length":150,"width":300,"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4}}`.
This defaults to 624 cuts with 8.5 mm centre pitch and approximately 7.361216 mm
row pitch. Public B-rep verification measures the hole loops, minimum web,
border, body count, volume and bounds in `tests/recipes_cli.rs`.
