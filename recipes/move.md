---
{
  "id": "move",
  "description": "Move or translate an existing body by an XYZ displacement vector.",
  "examples": [
    "Move the body twenty mm along X",
    "Shift this body upwards",
    "Translate the part by an XYZ vector",
    "Move the block to the left",
    "Shift the solid ten millimetres on Y"
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
      "delta": {
        "$ref": "#/$defs/Point3",
        "description": "Signed [x,y,z] displacement in mm"
      }
    },
    "required": [
      "body",
      "delta"
    ],
    "type": "object",
    "x-pilot-expander": "move"
  },
  "expander": "move"
}
---

Move or translate an existing body by an XYZ displacement vector. Built by the `move` expander.
