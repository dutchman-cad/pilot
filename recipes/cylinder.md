---
{
  "id": "cylinder",
  "description": "Solid cylinder, round rod or disc with radius and height.",
  "examples": [
    "Make a cylinder 6 mm radius and 24 tall",
    "I need a solid round rod",
    "Build a cylindrical disc",
    "Create a solid cylinder on the Z axis",
    "A circular cylinder forty millimetres high"
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
    "x-pilot-expander": "cylinder",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "height": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/height",
          "x-unit": "mm"
        },
        "radius": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/radius",
          "x-unit": "mm"
        }
      },
      "required": [
        "height",
        "radius"
      ]
    }
  },
  "expander": "cylinder"
}
---

Solid cylinder, round rod or disc with radius and height. Built by the `cylinder` expander.
