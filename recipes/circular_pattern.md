---
{
  "id": "circular_pattern",
  "synonyms": ["polar radial array copies around axis"],
  "description": "Repeat an existing body or last feature around an axis in a circular pattern with count and full or partial angle.",
  "examples": [
    "put 8 around the centre",
    "circular pattern of this body",
    "repeat the last hole around Z",
    "make 12 copies around the circumference",
    "radial array of six bosses",
    "pattern 5 instances over a partial angle of 120 degrees"
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
      "angle_deg": {
        "$ref": "#/$defs/Angle",
        "description": "Rotation in degrees"
      },
      "axis": {
        "$ref": "#/$defs/Axis",
        "description": "World axis"
      },
      "body": {
        "description": "Exact inspected body name; omit only if unambiguous",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "count": {
        "$ref": "#/$defs/Count"
      },
      "join": {
        "default": false,
        "description": "Allow overlap of joined last features only",
        "type": "boolean"
      },
      "target": {
        "description": "body: whole part + base; last_feature: latest boss/hole/slot only",
        "enum": [
          "body",
          "last_feature"
        ],
        "type": "string"
      }
    },
    "required": [
      "target",
      "count",
      "axis",
      "angle_deg"
    ],
    "type": "object",
    "x-pilot-expander": "circular_pattern",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "angle_deg": {
          "maximum": 360,
          "minimum": -360,
          "type": "number",
          "x-slot": "/angle_deg"
        },
        "axis": {
          "description": "Choice",
          "enum": [
            "X",
            "Y",
            "Z"
          ],
          "type": "string",
          "x-slot": "/axis"
        },
        "body": {
          "maxLength": 128,
          "minLength": 1,
          "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
          "type": "string",
          "x-slot": "/body"
        },
        "count": {
          "maximum": 500,
          "minimum": 2,
          "type": "integer",
          "x-slot": "/count"
        },
        "join": {
          "type": "boolean",
          "x-slot": "/join"
        },
        "target": {
          "description": "Choice",
          "enum": [
            "body",
            "last_feature"
          ],
          "type": "string",
          "x-slot": "/target"
        }
      },
      "required": [
        "angle_deg",
        "axis",
        "count",
        "target"
      ]
    }
  },
  "expander": "circular_pattern"
}
---

Repeat an existing body or last feature around an axis in a circular pattern with count and full or partial angle. Built by the `circular_pattern` expander.
