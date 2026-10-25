---
{
  "id": "box",
  "description": "Solid rectangular box, block or cube with length width and height.",
  "examples": [
    "Make a solid box 32 by 24 by 16 mm",
    "I need a rectangular block",
    "Create a cube with sides of 17 mm",
    "Build a plain solid box",
    "A cuboid forty long and twenty wide"
  ],
  "schema": {
    "x-pilot-expander": "box",
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
      "height": {
        "$ref": "#/$defs/Length"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "width": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "length",
      "width",
      "height"
    ],
    "type": "object"
  },
  "expander": "box"
}
---

Build a solid cuboid from its three dimensions. Built by the `box` expander.
