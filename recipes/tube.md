---
{
  "id": "tube",
  "description": "Hollow round tube or pipe with outer radius inner radius and height.",
  "examples": [
    "Make a hollow tube twenty mm long",
    "I need a pipe with a circular bore",
    "Build a tube with inner radius four",
    "Create a hollow round pipe",
    "A tubular sleeve with outer radius eight"
  ],
  "schema": {
    "x-pilot-expander": "tube",
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
      "inner_radius": {
        "$ref": "#/$defs/Length"
      },
      "outer_radius": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "outer_radius",
      "inner_radius",
      "height"
    ],
    "type": "object"
  },
  "expander": "tube"
}
---

Build a circular tube from its inner radius, outer radius and height. Built by the `tube` expander.
