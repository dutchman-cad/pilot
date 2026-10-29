---
{
  "id": "chamfer",
  "synonyms": [
    "bevel chamfer all edges corners"
  ],
  "description": "Chamfer all edges of an existing body by a distance, after topology lookup.",
  "examples": [
    "Bevel all edges of this body",
    "Chamfer every edge by two mm",
    "Apply a chamfer to all edges",
    "I want all the edges beveled",
    "Cut a bevel along every edge"
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
      },
      "edges": {
        "type": "string",
        "enum": [
          "all",
          "vertical"
        ],
        "default": "all",
        "description": "all: every edge; vertical: upright bounding-box corners"
      },
      "corner_sides": {
        "type": "string",
        "enum": [
          "all",
          "right",
          "left",
          "top",
          "bottom"
        ],
        "description": "Upright corners; right=+X, top=+Y"
      }
    },
    "required": [
      "body",
      "amount"
    ],
    "type": "object",
    "x-pilot-expander": "chamfer",
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
        },
        "edges": {
          "type": "string",
          "enum": [
            "all",
            "vertical"
          ],
          "description": "Optional upright corners of the inspected bounding box; right is +X, top is +Y",
          "x-slot": "/edges"
        },
        "corner_sides": {
          "type": "string",
          "enum": [
            "all",
            "right",
            "left",
            "top",
            "bottom"
          ],
          "description": "Optional upright corners of the inspected bounding box; right is +X, top is +Y",
          "x-slot": "/corner_sides"
        }
      },
      "required": [
        "amount",
        "body"
      ]
    }
  },
  "expander": "chamfer"
}
---

Chamfer all edges of an existing body by a distance, after topology lookup. Built by the `chamfer` expander.
