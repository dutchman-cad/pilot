---
{
  "id": "mirror",
  "description": "Mirror an existing body across XY XZ or YZ, keeping or discarding the original.",
  "examples": [
    "Mirror this body across the XY plane",
    "Reflect the part across YZ",
    "Make a mirrored copy of the solid",
    "Mirror the block and keep the original",
    "Reflect the body across the XZ plane"
  ],
  "schema": {
    "x-pilot-expander": "mirror",
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
      "body": {
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "keep_original": {
        "type": "boolean"
      },
      "plane": {
        "$ref": "#/$defs/Plane"
      }
    },
    "required": [
      "body",
      "plane",
      "keep_original"
    ],
    "type": "object"
  },
  "expander": "mirror"
}
---

Reflect a selected body across a principal plane. Built by the `mirror` expander.
