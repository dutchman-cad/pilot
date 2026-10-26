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
      "body": {
        "description": "Exact inspected body name; omit only if unambiguous",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "keep_original": {
        "description": "True: retain source and add mirrored copy",
        "type": "boolean"
      },
      "plane": {
        "$ref": "#/$defs/Plane",
        "description": "Reflection plane"
      }
    },
    "required": [
      "body",
      "plane",
      "keep_original"
    ],
    "type": "object",
    "x-pilot-expander": "mirror"
  },
  "expander": "mirror"
}
---

Mirror an existing body across XY XZ or YZ, keeping or discarding the original. Built by the `mirror` expander.
