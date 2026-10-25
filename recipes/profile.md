---
{
  "id": "profile",
  "description": "Extrude L T U C or rectangular profiles with web and flange thickness.",
  "examples": [
    "Extrude an L profile twenty mm deep",
    "I need a T section with a thick stem",
    "Make a U channel profile",
    "Build a C profile on the XZ plane",
    "Extrude a constrained rectangular profile"
  ],
  "schema": {
    "x-pilot-expander": "profile",
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
      "depth": {
        "$ref": "#/$defs/Length"
      },
      "flange_thickness": {
        "$ref": "#/$defs/Length"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "kind": {
        "enum": [
          "L",
          "T",
          "U",
          "C",
          "rectangle"
        ],
        "type": "string"
      },
      "plane": {
        "$ref": "#/$defs/Plane"
      },
      "stem_offset": {
        "maximum": 1000000,
        "minimum": 0,
        "type": "number"
      },
      "thickness": {
        "$ref": "#/$defs/Length"
      },
      "web_thickness": {
        "$ref": "#/$defs/Length"
      },
      "width": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "kind",
      "plane",
      "width",
      "height",
      "depth"
    ],
    "type": "object"
  },
  "expander": "profile"
}
---

Extrude an L, T, U, C or rectangular section on a principal plane. Built by the `profile` expander.
