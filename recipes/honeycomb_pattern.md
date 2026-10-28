---
{
  "id": "honeycomb_pattern",
  "synonyms": [
    "perforated lattice honeycomb hex mesh hexagon cells web border"
  ],
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
        "description": "Sketch plane; extrude along its normal",
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
        "description": "Minimum uncut margin",
        "maximum": 1000000,
        "minimum": 0,
        "type": "number"
      },
      "centred": {
        "default": false,
        "description": "True only for centred/centered/on the origin",
        "type": "boolean"
      },
      "columns": {
        "description": "Instance count; honeycomb requires exact mode",
        "maximum": 5000,
        "minimum": 1,
        "type": "integer"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "mode": {
        "default": "fill",
        "description": "fill: fit field; exact: require rows and columns",
        "enum": [
          "fill",
          "exact"
        ],
        "type": "string"
      },
      "orientation": {
        "description": "Omit: rows along longer side; pointy_top: X rows; flat_top: Y rows",
        "enum": [
          "pointy_top",
          "flat_top"
        ],
        "type": "string"
      },
      "plane": {
        "$ref": "#/$defs/Plane",
        "default": "XY",
        "description": "Sketch plane"
      },
      "rows": {
        "description": "Instance count; honeycomb requires exact mode",
        "maximum": 5000,
        "minimum": 1,
        "type": "integer"
      },
      "size": {
        "$ref": "#/$defs/Length",
        "description": "Fillet radius/chamfer distance; hex span for honeycomb"
      },
      "size_kind": {
        "description": "across_flats: edge to edge; across_corners: vertex to vertex",
        "enum": [
          "across_flats",
          "across_corners"
        ],
        "type": "string"
      },
      "thickness": {
        "$ref": "#/$defs/Length",
        "description": "Plate thickness or uniform profile legs"
      },
      "web": {
        "$ref": "#/$defs/Length",
        "description": "Minimum material between hex holes"
      },
      "width": {
        "$ref": "#/$defs/Length"
      },
      "corner": {
        "$ref": "#/$defs/Point3",
        "description": "Minimum [x,y,z]; omitted: [0,0,0]"
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
    "x-pilot-expander": "honeycomb_pattern",
    "x-pilot-fill": {
      "groups": [
        {
          "fields": [
            "corner_x",
            "corner_y",
            "corner_z"
          ],
          "required": [
            "corner_x",
            "corner_y",
            "corner_z"
          ]
        }
      ],
      "properties": {
        "border": {
          "maximum": 1000000,
          "minimum": 0,
          "type": "number",
          "x-slot": "/border",
          "x-unit": "mm"
        },
        "centred": {
          "type": "boolean",
          "x-slot": "/centred"
        },
        "columns": {
          "maximum": 5000,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/columns"
        },
        "length": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/length",
          "x-unit": "mm"
        },
        "mode": {
          "description": "Choice",
          "enum": [
            "fill",
            "exact"
          ],
          "type": "string",
          "x-slot": "/mode"
        },
        "orientation": {
          "description": "Choice",
          "enum": [
            "pointy_top",
            "flat_top"
          ],
          "type": "string",
          "x-slot": "/orientation"
        },
        "plane": {
          "description": "Choice",
          "enum": [
            "XY",
            "XZ",
            "YZ"
          ],
          "type": "string",
          "x-slot": "/plane"
        },
        "rows": {
          "maximum": 5000,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/rows"
        },
        "size": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/size",
          "x-unit": "mm"
        },
        "size_kind": {
          "description": "Choice",
          "enum": [
            "across_flats",
            "across_corners"
          ],
          "type": "string",
          "x-slot": "/size_kind"
        },
        "thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/thickness",
          "x-unit": "mm"
        },
        "web": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/web",
          "x-unit": "mm"
        },
        "width": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/width",
          "x-unit": "mm"
        },
        "corner_x": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-unit": "mm",
          "x-slot": "/corner/0"
        },
        "corner_y": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-unit": "mm",
          "x-slot": "/corner/1"
        },
        "corner_z": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-unit": "mm",
          "x-slot": "/corner/2"
        }
      },
      "required": [
        "border",
        "length",
        "size",
        "size_kind",
        "thickness",
        "web",
        "width"
      ]
    }
  },
  "expander": "honeycomb_pattern"
}
---

Cut a plate with a staggered honeycomb of hexagonal through holes, bounded web and border; fill or exact rows and columns. Built by the `honeycomb_pattern` expander.

Length stays along sketch X and width along sketch Y. When orientation is omitted,
rows run along the longer side (X on a square). Swapping unequal length and width
rotates the part by 90 degrees with the same hole count and volume. Explicit
`pointy_top` means rows along X; explicit `flat_top` means rows along Y and wins
over the longer-side convention. To rotate an explicit layout, swap these two
orientations as well as the dimensions. This applies to fill and exact mode;
columns count along rows. On XZ/YZ, X and Y here mean the plate's sketch axes.

An optional `corner:[x,y,z]` (flat fields `corner_x/y/z`) sets the final minimum
bounding-box corner. Use either this corner or `centred:true`. Construction always
starts at the origin; a single rigid move places the finished plate. An inline
DXF of hex and row blocks supplies the perforated profile for one extrusion;
zero-border layouts use seed cuts and feature patterns instead.
