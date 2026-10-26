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
    "type": "object",
    "x-pilot-expander": "tube",
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
        "inner_radius": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/inner_radius",
          "x-unit": "mm"
        },
        "outer_radius": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/outer_radius",
          "x-unit": "mm"
        }
      },
      "required": [
        "height",
        "inner_radius",
        "outer_radius"
      ]
    }
  },
  "expander": "tube"
}
---

Hollow round tube or pipe with outer radius inner radius and height. Built by the `tube` expander.
