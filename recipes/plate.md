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
      "bolt_circle": {
        "additionalProperties": false,
        "description": "Hole centres on this circle",
        "properties": {
          "count": {
            "$ref": "#/$defs/Count"
          },
          "diameter": {
            "$ref": "#/$defs/Length"
          },
          "start_deg": {
            "$ref": "#/$defs/Angle",
            "default": 0,
            "description": "From +X toward +Y"
          }
        },
        "required": [
          "count",
          "diameter"
        ],
        "type": "object"
      },
      "bore_diameter": {
        "$ref": "#/$defs/Length",
        "description": "Centre through bore"
      },
      "centred": {
        "default": false,
        "description": "True only for centred/centered/on the origin",
        "type": "boolean"
      },
      "corners": {
        "additionalProperties": false,
        "description": "Outside corner treatment",
        "properties": {
          "edges": {
            "default": "vertical",
            "description": "vertical: four corner edges through thickness; all: every rim too",
            "enum": [
              "vertical",
              "all"
            ],
            "type": "string"
          },
          "kind": {
            "description": "fillet: round; chamfer: bevel",
            "enum": [
              "fillet",
              "chamfer"
            ],
            "type": "string"
          },
          "size": {
            "$ref": "#/$defs/Length",
            "description": "Fillet radius/chamfer distance; hex span for honeycomb"
          }
        },
        "required": [
          "kind",
          "size"
        ],
        "type": "object"
      },
      "grid": {
        "additionalProperties": false,
        "description": "Hole centres in rows and columns",
        "properties": {
          "columns": {
            "$ref": "#/$defs/Count",
            "description": "Instance count; honeycomb requires exact mode"
          },
          "pitch_x": {
            "$ref": "#/$defs/Length",
            "description": "Hole centre spacing"
          },
          "pitch_y": {
            "$ref": "#/$defs/Length",
            "description": "Hole centre spacing"
          },
          "rows": {
            "$ref": "#/$defs/Count",
            "description": "Instance count; honeycomb requires exact mode"
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
        "$ref": "#/$defs/Length",
        "description": "Pattern hole diameter; requires bolt_circle or grid"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "plane": {
        "$ref": "#/$defs/Plane",
        "default": "XY",
        "description": "Sketch plane"
      },
      "thickness": {
        "$ref": "#/$defs/Length",
        "description": "Plate thickness or uniform profile legs"
      },
      "width": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "length",
      "width",
      "thickness"
    ],
    "type": "object",
    "x-pilot-expander": "plate",
    "x-pilot-fill": {
      "groups": [
        {
          "fields": [
            "bolt_hole_count",
            "bolt_circle_diameter",
            "bolt_start_deg",
            "bolt_hole_diameter"
          ],
          "required": [
            "bolt_hole_count",
            "bolt_circle_diameter",
            "bolt_hole_diameter"
          ]
        },
        {
          "fields": [
            "corner_edges",
            "corner_kind",
            "corner_size"
          ],
          "required": [
            "corner_kind",
            "corner_size"
          ]
        },
        {
          "fields": [
            "grid_columns",
            "grid_pitch_x",
            "grid_pitch_y",
            "grid_rows",
            "grid_hole_diameter"
          ],
          "required": [
            "grid_columns",
            "grid_pitch_x",
            "grid_pitch_y",
            "grid_rows",
            "grid_hole_diameter"
          ]
        }
      ],
      "properties": {
        "bolt_circle_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/bolt_circle/diameter",
          "x-unit": "mm"
        },
        "bolt_hole_count": {
          "maximum": 64,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/bolt_circle/count"
        },
        "bolt_hole_diameter": {
          "description": "Pattern holes",
          "type": "number",
          "x-slot": "/hole_diameter",
          "x-unit": "mm"
        },
        "bolt_start_deg": {
          "maximum": 360,
          "minimum": -360,
          "type": "number",
          "x-slot": "/bolt_circle/start_deg"
        },
        "bore_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/bore_diameter",
          "x-unit": "mm"
        },
        "centred": {
          "type": "boolean",
          "x-slot": "/centred"
        },
        "corner_edges": {
          "description": "Choice",
          "enum": [
            "vertical",
            "all"
          ],
          "type": "string",
          "x-slot": "/corners/edges"
        },
        "corner_kind": {
          "description": "Choice",
          "enum": [
            "fillet",
            "chamfer"
          ],
          "type": "string",
          "x-slot": "/corners/kind"
        },
        "corner_size": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/corners/size",
          "x-unit": "mm"
        },
        "grid_columns": {
          "maximum": 64,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/grid/columns"
        },
        "grid_hole_diameter": {
          "description": "Pattern holes",
          "type": "number",
          "x-slot": "/hole_diameter",
          "x-unit": "mm"
        },
        "grid_pitch_x": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/grid/pitch_x",
          "x-unit": "mm"
        },
        "grid_pitch_y": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/grid/pitch_y",
          "x-unit": "mm"
        },
        "grid_rows": {
          "maximum": 64,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/grid/rows"
        },
        "length": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/length",
          "x-unit": "mm"
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
        "thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/thickness",
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
        "length",
        "thickness",
        "width"
      ]
    }
  },
  "expander": "plate"
}
---

Flat plate with holes: centre bore, bolt circle or rectangular grid and optional corners. Built by the `plate` expander.
