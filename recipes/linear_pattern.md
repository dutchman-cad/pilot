---
{
  "id": "linear_pattern",
  "synonyms": ["array row line copies repeat spacing along"],
  "description": "Repeat an existing body or last feature in a linear row with count and spacing or total length; use one pattern, never repeated copies.",
  "examples": [
    "make 50 of these in a row",
    "I need 12 copies of this boss 15 mm apart",
    "repeat the hole every 20 mm",
    "linear pattern of the last feature along X",
    "duplicate this body 10 times along an edge",
    "6 instances over a total length of 100 mm"
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
      "count": {
        "$ref": "#/$defs/Count"
      },
      "direction": {
        "$ref": "#/$defs/Axis",
        "description": "World axis"
      },
      "edge": {
        "description": "Inspected straight edge; use instead of direction",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "join": {
        "default": false,
        "description": "Allow overlap of joined last features only",
        "type": "boolean"
      },
      "spacing": {
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
      },
      "total_length": {
        "$ref": "#/$defs/Length",
        "description": "First to last centre; use instead of spacing"
      }
    },
    "required": [
      "target",
      "count"
    ],
    "type": "object",
    "x-pilot-expander": "linear_pattern",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
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
        "direction": {
          "description": "Choice",
          "enum": [
            "X",
            "Y",
            "Z"
          ],
          "type": "string",
          "x-slot": "/direction"
        },
        "edge": {
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/edge"
        },
        "join": {
          "type": "boolean",
          "x-slot": "/join"
        },
        "spacing": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/spacing",
          "x-unit": "mm"
        },
        "target": {
          "description": "Choice",
          "enum": [
            "body",
            "last_feature"
          ],
          "type": "string",
          "x-slot": "/target"
        },
        "total_length": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/total_length",
          "x-unit": "mm"
        }
      },
      "required": [
        "count",
        "target"
      ]
    }
  },
  "expander": "linear_pattern"
}
---

Repeat an existing body or last feature in a linear row with count and spacing or total length; use one pattern, never repeated copies. Built by the `linear_pattern` expander.
