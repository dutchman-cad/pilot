---
{
  "id": "grid_pattern",
  "description": "Repeat an existing body or last feature in a rectangular grid: rows, columns, plane and spacing in both directions.",
  "examples": [
    "array it 4 by 6",
    "pattern this body in a rectangular grid",
    "repeat the last feature in 3 rows and 5 columns",
    "make 24 copies in a grid 10 mm apart both ways",
    "rectangular array on XZ",
    "repeat these bosses in rows and columns"
  ],
  "schema": {
    "$defs": {
      "Angle": {
        "description": "Degrees",
        "maximum": 360,
        "minimum": -360,
        "type": "number"
      },
      "Axis": {
        "description": "World axis",
        "enum": [
          "X",
          "Y",
          "Z"
        ],
        "type": "string"
      },
      "Count": {
        "maximum": 500,
        "minimum": 2,
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
      "body": {
        "description": "Exact inspected body name; omit only if unambiguous",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "columns": {
        "$ref": "#/$defs/Count",
        "description": "Instance count; honeycomb requires exact mode"
      },
      "join": {
        "default": false,
        "description": "Allow overlap of joined last features only",
        "type": "boolean"
      },
      "plane": {
        "$ref": "#/$defs/Plane",
        "default": "XY",
        "description": "Sketch plane"
      },
      "rows": {
        "$ref": "#/$defs/Count",
        "description": "Instance count; honeycomb requires exact mode"
      },
      "spacing1": {
        "$ref": "#/$defs/Length",
        "description": "Centre spacing between instances"
      },
      "spacing2": {
        "$ref": "#/$defs/Length",
        "description": "Centre spacing between instances"
      },
      "target": {
        "description": "body: whole solid; last_feature: latest feature",
        "enum": [
          "body",
          "last_feature"
        ],
        "type": "string"
      }
    },
    "required": [
      "target",
      "rows",
      "columns",
      "spacing1",
      "spacing2"
    ],
    "type": "object",
    "x-pilot-expander": "grid_pattern"
  },
  "expander": "grid_pattern"
}
---

Repeat an existing body or last feature in a rectangular grid: rows, columns, plane and spacing in both directions. Built by the `grid_pattern` expander.
