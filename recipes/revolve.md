---
{
  "id": "revolve",
  "description": "Revolve a rectangular sketch about Z through 360 degrees.",
  "examples": [
    "Revolve a rectangle about the Z axis",
    "Spin a rectangular sketch through a full turn",
    "Create a solid by revolving a sketch",
    "Build a full revolution of a rectangle",
    "Sweep a rectangle around Z by 360 degrees"
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
      "height": {
        "$ref": "#/$defs/Length"
      },
      "radius": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "radius",
      "height"
    ],
    "type": "object",
    "x-pilot-expander": "revolve"
  },
  "expander": "revolve"
}
---

Revolve a rectangular sketch about Z through 360 degrees. Built by the `revolve` expander.
