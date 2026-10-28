---
{
  "description": "Build a rectangular or round plate/base with joined round pegs or bosses, holes in a centred row, grid or bolt circle, and optional corners.",
  "examples": [
    "Make a plate with a grid of holes",
    "I want a mounting plate with a bolt circle",
    "Build a plate with a central bore",
    "A perforated plate with chamfered corners",
    "Build a base with a row of pegs on top",
    "Round plate with bosses around a circle",
    "A pegboard panel with a centred grid of holes",
    "Base plate with a single offset boss"
  ],
  "expander": "plate",
  "id": "plate",
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
        "description": "Feature centres on this circle",
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
      "boss_diameter": {
        "$ref": "#/$defs/Length",
        "description": "Joined round peg/boss diameter; requires boss_height"
      },
      "boss_height": {
        "$ref": "#/$defs/Length",
        "description": "Rise above plate top; requires boss_diameter"
      },
      "boss_offset_x": {
        "description": "Single boss offset from plate centre; omitted: 0",
        "maximum": 1000000,
        "minimum": -1000000,
        "type": "number"
      },
      "boss_offset_y": {
        "description": "Single boss offset from plate centre; omitted: 0",
        "maximum": 1000000,
        "minimum": -1000000,
        "type": "number"
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
      "diameter": {
        "$ref": "#/$defs/Length"
      },
      "grid": {
        "additionalProperties": false,
        "description": "Feature centres: columns along sketch X, rows along sketch Y",
        "properties": {
          "columns": {
            "$ref": "#/$defs/Count",
            "description": "Instance count; honeycomb requires exact mode"
          },
          "pitch_x": {
            "$ref": "#/$defs/Length",
            "description": "Feature centre spacing"
          },
          "pitch_y": {
            "$ref": "#/$defs/Length",
            "description": "Feature centre spacing"
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
        "description": "Pattern hole diameter; requires bolt_circle, grid or row"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "plane": {
        "$ref": "#/$defs/Plane",
        "default": "XY",
        "description": "Sketch plane"
      },
      "row": {
        "additionalProperties": false,
        "description": "Centred feature row; count includes all; pitch is centre spacing",
        "properties": {
          "axis": {
            "description": "World axis",
            "enum": [
              "X",
              "Y"
            ],
            "type": "string"
          },
          "count": {
            "$ref": "#/$defs/Count"
          },
          "pitch": {
            "$ref": "#/$defs/Length"
          }
        },
        "required": [
          "count",
          "pitch"
        ],
        "type": "object"
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
            "bolt_circle_diameter"
          ]
        },
        {
          "fields": [
            "corner_edges",
            "corner_kind",
            "corner_size"
          ],
          "required": [
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
            "grid_rows"
          ]
        },
        {
          "fields": [
            "boss_diameter",
            "boss_height",
            "boss_offset_x",
            "boss_offset_y"
          ],
          "required": [
            "boss_diameter",
            "boss_height"
          ]
        },
        {
          "fields": [
            "row_count",
            "row_pitch",
            "row_axis"
          ],
          "required": [
            "row_count",
            "row_pitch"
          ]
        }
      ],
      "properties": {
        "bolt_circle_diameter": {
          "description": "Diameter of feature-centre circle on plate",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/bolt_circle/diameter",
          "x-unit": "mm"
        },
        "bolt_hole_count": {
          "description": "Feature count on circle, including bosses when boss_diameter is given",
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
        "boss_diameter": {
          "description": "Joined round peg/boss diameter; requires boss_height",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/boss_diameter",
          "x-unit": "mm"
        },
        "boss_height": {
          "description": "Rise above plate top; requires boss_diameter",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/boss_height",
          "x-unit": "mm"
        },
        "boss_offset_x": {
          "description": "Single boss offset from plate centre in sketch coordinates; omitted: 0",
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/boss_offset_x",
          "x-unit": "mm"
        },
        "boss_offset_y": {
          "description": "Single boss offset from plate centre in sketch coordinates; omitted: 0",
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/boss_offset_y",
          "x-unit": "mm"
        },
        "centred": {
          "type": "boolean",
          "x-slot": "/centred"
        },
        "corner_edges": {
          "default": "vertical",
          "description": "vertical: four corner edges through thickness; all: rims too",
          "enum": [
            "vertical",
            "all"
          ],
          "type": "string",
          "x-default-when": "corner_size",
          "x-slot": "/corners/edges"
        },
        "corner_kind": {
          "default": "fillet",
          "description": "fillet: round; chamfer: bevel",
          "enum": [
            "fillet",
            "chamfer"
          ],
          "type": "string",
          "x-default-when": "corner_size",
          "x-slot": "/corners/kind"
        },
        "corner_size": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/corners/size",
          "x-unit": "mm"
        },
        "diameter": {
          "description": "Round plate diameter; omit length and width",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/diameter",
          "x-unit": "mm"
        },
        "grid_columns": {
          "description": "Along sketch X; A x B: A along longer side unless explicitly directed",
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
          "description": "Centre spacing along X; single pitch: use for both axes",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/grid/pitch_x",
          "x-unit": "mm"
        },
        "grid_pitch_y": {
          "description": "Centre spacing along Y; single pitch: use for both axes",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/grid/pitch_y",
          "x-unit": "mm"
        },
        "grid_rows": {
          "description": "Along sketch Y; 7 x 5 on 120 x 80: columns=7, rows=5",
          "maximum": 64,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/grid/rows"
        },
        "hole_diameter": {
          "description": "Hole diameter; without a layout, one hole at the plate centre; omit for bosses",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_diameter",
          "x-unit": "mm"
        },
        "length": {
          "description": "Rectangle X size; supply length and width OR round diameter",
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
        "row_axis": {
          "default": "X",
          "description": "Row along sketch X or Y; omitted: X",
          "enum": [
            "X",
            "Y"
          ],
          "type": "string",
          "x-default-when": "row_count",
          "x-slot": "/row/axis"
        },
        "row_count": {
          "description": "Total number of holes or bosses in centred row",
          "maximum": 64,
          "minimum": 1,
          "type": "integer",
          "x-slot": "/row/count"
        },
        "row_pitch": {
          "description": "Centre-to-centre spacing, not edge gap",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/row/pitch",
          "x-unit": "mm"
        },
        "thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/thickness",
          "x-unit": "mm"
        },
        "width": {
          "description": "Rectangle Y size; omit for round plate",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/width",
          "x-unit": "mm"
        }
      },
      "required": [
        "thickness"
      ]
    }
  },
  "synonyms": [
    "bolt pattern PCD flange mounting plate",
    "perforated panel circular holes grid pitch",
    "base pegs bosses raised features on top row",
    "pegboard panel grid holes pitch"
  ]
}
---

Build the base and features together as one body. Supply length and width OR diameter, plus thickness.
Layouts apply to boss_diameter with boss_height, or hole_diameter, never both. Without a layout, hole_diameter creates one hole at the plate centre. Without a layout a boss is at the plate centre unless boss_offset_x/y are supplied (signed offsets from centre, independent of base placement). A centre bore may accompany bosses. Layouts are centred on the plate regardless of centred base placement.
Columns run along sketch X, rows along sketch Y. For an unstated A x B orientation, put A along the longer plate side (7 x 5 on 120 x 80 means 7 columns, 5 rows); explicit axis/row/column statements win. A single pitch is used for both grid pitches. Counts include every feature. No dimension is defaulted.
A corner_size without corner_kind means fillet; without corner_edges it means vertical. Corners are supported on rectangular plates without bosses.
