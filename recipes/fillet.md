---
{
  "id": "fillet",
  "description": "Fillet all edges of an existing body with a radius, after topology lookup.",
  "examples": [
    "Round all edges of this body",
    "Fillet every edge with radius two",
    "Soften the body edges with a fillet",
    "Apply a fillet to all edges",
    "I want all the edges rounded"
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
      "amount": {
        "$ref": "#/$defs/Length",
        "description": "Fillet radius or chamfer distance"
      },
      "body": {
        "description": "Exact inspected body name; omit only if unambiguous",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      }
    },
    "required": [
      "body",
      "amount"
    ],
    "type": "object",
    "x-pilot-expander": "fillet",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "amount": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/amount",
          "x-unit": "mm"
        },
        "body": {
          "maxLength": 128,
          "minLength": 1,
          "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
          "type": "string",
          "x-slot": "/body"
        }
      },
      "required": [
        "amount",
        "body"
      ]
    }
  },
  "expander": "fillet"
}
---

Fillet all edges of an existing body with a radius, after topology lookup. Built by the `fillet` expander.
