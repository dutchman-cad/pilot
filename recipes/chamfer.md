---
{
  "id": "chamfer",
  "description": "Chamfer all edges of an existing body by a distance, after topology lookup.",
  "examples": [
    "Bevel all edges of this body",
    "Chamfer every edge by two mm",
    "Apply a chamfer to all edges",
    "I want all the edges beveled",
    "Cut a bevel along every edge"
  ],
  "schema": {
    "x-pilot-expander": "chamfer",
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
      "amount": {
        "$ref": "#/$defs/Length"
      },
      "body": {
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
    "type": "object"
  },
  "expander": "chamfer"
}
---

Look up the body's topology, then bevel all of its edges. Built by the `chamfer` expander.
