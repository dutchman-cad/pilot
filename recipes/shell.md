---
{
  "id": "shell",
  "description": "Shell a rectangular box with uniform wall thickness and an open top.",
  "examples": [
    "Shell a box with a two mm wall",
    "Make a shelled rectangular box",
    "Hollow a box leaving uniform walls",
    "Create an open top shell",
    "A shell box with a thin wall"
  ],
  "synonyms": [
    "hollow open top shelled box uniform thickness bin planter"
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
    "anyOf": [
      {
        "required": [
          "body"
        ]
      },
      {
        "required": [
          "length",
          "width",
          "height"
        ]
      }
    ],
    "properties": {
      "body": {
        "description": "Exact inspected body name; omit only if unambiguous",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "wall": {
        "$ref": "#/$defs/Length"
      },
      "width": {
        "$ref": "#/$defs/Length"
      },
      "open": {
        "type": "string",
        "enum": [
          "top",
          "bottom"
        ],
        "default": "top",
        "description": "Face to remove"
      }
    },
    "required": [
      "wall"
    ],
    "type": "object",
    "x-pilot-expander": "shell",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "body": {
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/body"
        },
        "height": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/height",
          "x-unit": "mm"
        },
        "length": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/length",
          "x-unit": "mm"
        },
        "wall": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/wall",
          "x-unit": "mm"
        },
        "width": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/width",
          "x-unit": "mm"
        },
        "open": {
          "type": "string",
          "enum": [
            "top",
            "bottom"
          ],
          "default": "top",
          "description": "Face to remove from the existing box",
          "x-slot": "/open"
        }
      },
      "required": [
        "wall"
      ]
    }
  },
  "expander": "shell"
}
---

Shell a rectangular box with uniform wall thickness and an open top. Built by the `shell` expander.
