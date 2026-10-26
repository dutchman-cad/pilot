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
        "default": "pointy_top",
        "description": "pointy_top: vertex up; flat_top: edge up",
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
      "groups": [],
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
