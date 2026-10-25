---
{
  "id": "plate",
  "description": "Flat plate with holes: centre bore, bolt circle or rectangular grid and optional corners.",
  "examples": [
    "Make a plate with a grid of holes",
    "I want a mounting plate with a bolt circle",
    "Build a plate with a central bore",
    "Drill six holes in a flat plate",
    "A perforated plate with chamfered corners"
  ],
  "schema": {
    "x-pilot-expander": "plate",
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
      "bolt_circle": {
        "additionalProperties": false,
        "properties": {
          "count": {
            "$ref": "#/$defs/Count"
          },
          "diameter": {
            "$ref": "#/$defs/Length"
          },
          "start_deg": {
            "$ref": "#/$defs/Angle"
          }
        },
        "required": [
          "count",
          "diameter"
        ],
        "type": "object"
      },
      "bore_diameter": {
        "$ref": "#/$defs/Length"
      },
      "centred": {
        "type": "boolean"
      },
      "corners": {
        "additionalProperties": false,
        "properties": {
          "edges": {
            "enum": [
              "vertical",
              "all"
            ],
            "type": "string"
          },
          "kind": {
            "enum": [
              "fillet",
              "chamfer"
            ],
            "type": "string"
          },
          "size": {
            "$ref": "#/$defs/Length"
          }
        },
        "required": [
          "kind",
          "size",
          "edges"
        ],
        "type": "object"
      },
      "grid": {
        "additionalProperties": false,
        "properties": {
          "columns": {
            "$ref": "#/$defs/Count"
          },
          "pitch_x": {
            "$ref": "#/$defs/Length"
          },
          "pitch_y": {
            "$ref": "#/$defs/Length"
          },
          "rows": {
            "$ref": "#/$defs/Count"
          }
        },
        "required": [
          "rows",
          "columns",
          "pitch_x",
          "pitch_y"
        ],
        "type": "object"
      },
      "hole_diameter": {
        "$ref": "#/$defs/Length"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "plane": {
        "$ref": "#/$defs/Plane"
      },
      "thickness": {
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
      "plane",
      "centred"
    ],
    "type": "object"
  },
  "expander": "plate"
}
---

Build a flat plate with optional bores, hole patterns and corner treatment. Built by the `plate` expander.
