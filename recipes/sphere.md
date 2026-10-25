---
{
  "id": "sphere",
  "description": "Solid sphere or ball with a radius.",
  "examples": [
    "Make a sphere of radius 9 mm",
    "I need a solid ball",
    "Build a spherical solid",
    "Create a round sphere",
    "A ball twelve millimetres across"
  ],
  "schema": {
    "x-pilot-expander": "sphere",
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
      "radius": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "radius"
    ],
    "type": "object"
  },
  "expander": "sphere"
}
---

Build a solid ball from its radius. Built by the `sphere` expander.
