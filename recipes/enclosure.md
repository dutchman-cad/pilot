---
{
  "id": "enclosure",
  "description": "Open enclosure with floor, walls and optional inner or outer lip.",
  "examples": [
    "Make an enclosure with an inner lip",
    "I need an open top electronics enclosure",
    "Build a lipped enclosure with a thick floor",
    "An open bottom enclosure with thin walls",
    "Create an enclosure centred on the origin"
  ],
  "schema": {
    "x-pilot-expander": "enclosure",
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
      "centred": {
        "type": "boolean"
      },
      "floor": {
        "$ref": "#/$defs/Length"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "lip": {
        "additionalProperties": false,
        "properties": {
          "height": {
            "$ref": "#/$defs/Length"
          },
          "side": {
            "enum": [
              "inner",
              "outer"
            ],
            "type": "string"
          },
          "width": {
            "$ref": "#/$defs/Length"
          }
        },
        "required": [
          "width",
          "height",
          "side"
        ],
        "type": "object"
      },
      "open": {
        "enum": [
          "top",
          "bottom"
        ],
        "type": "string"
      },
      "wall": {
        "$ref": "#/$defs/Length"
      },
      "width": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "length",
      "width",
      "height",
      "wall",
      "open",
      "centred"
    ],
    "type": "object"
  },
  "expander": "enclosure"
}
---

Build an open enclosure with a floor and optional locating lip. Built by the `enclosure` expander.
