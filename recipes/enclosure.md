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
      "centred": {
        "default": false,
        "description": "True only for centred/centered/on the origin",
        "type": "boolean"
      },
      "floor": {
        "$ref": "#/$defs/Length",
        "description": "Base thickness; omitted: wall"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "length": {
        "$ref": "#/$defs/Length"
      },
      "lip": {
        "additionalProperties": false,
        "description": "Raised locating ring at opening",
        "properties": {
          "height": {
            "$ref": "#/$defs/Length",
            "description": "Rise above the opening"
          },
          "side": {
            "default": "inner",
            "description": "inner: flush with inner wall face; outer: outer face",
            "enum": [
              "inner",
              "outer"
            ],
            "type": "string"
          },
          "width": {
            "$ref": "#/$defs/Length",
            "description": "Ring width across wall; below wall thickness"
          }
        },
        "required": [
          "width",
          "height"
        ],
        "type": "object"
      },
      "open": {
        "default": "top",
        "description": "Face to remove",
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
      "wall"
    ],
    "type": "object",
    "x-pilot-expander": "enclosure",
    "x-pilot-fill": {
      "groups": [
        {
          "fields": [
            "lip_height",
            "lip_side",
            "lip_width"
          ],
          "required": [
            "lip_height",
            "lip_width"
          ]
        }
      ],
      "properties": {
        "centred": {
          "type": "boolean",
          "x-slot": "/centred"
        },
        "floor": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/floor",
          "x-unit": "mm"
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
        "lip_height": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/lip/height",
          "x-unit": "mm"
        },
        "lip_side": {
          "description": "inner/outer wall face",
          "enum": [
            "inner",
            "outer"
          ],
          "type": "string",
          "x-slot": "/lip/side"
        },
        "lip_width": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/lip/width",
          "x-unit": "mm"
        },
        "open": {
          "description": "Choice",
          "enum": [
            "top",
            "bottom"
          ],
          "type": "string",
          "x-slot": "/open"
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
        }
      },
      "required": [
        "height",
        "length",
        "wall",
        "width"
      ]
    }
  },
  "expander": "enclosure"
}
---

Open enclosure with floor, walls and optional inner or outer lip. Built by the `enclosure` expander.
