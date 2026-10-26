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
      "radius": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "radius"
    ],
    "type": "object",
    "x-pilot-expander": "sphere",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "radius": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/radius",
          "x-unit": "mm"
        },
        "diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/radius",
          "x-unit": "mm",
          "x-scale": 0.5,
          "description": "Diameter; must equal twice the radius if both supplied"
        }
      },
      "required": [
        "radius"
      ]
    }
  },
  "expander": "sphere"
}
---


Solid sphere or ball with a radius. Built by the `sphere` expander.
