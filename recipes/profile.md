---
{
  "id": "profile",
  "synonyms": [
    "bracket angle iron L",
    "tee T section",
    "channel U C",
    "hollow rectangle section"
  ],
  "description": "Extrude L T U C or rectangular profiles with web and flange thickness.",
  "examples": [
    "Extrude an L profile twenty mm deep",
    "I need a T section with a thick stem",
    "Make a U channel profile",
    "Build a C profile on the XZ plane",
    "Extrude a constrained rectangular profile"
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
      "depth": {
        "$ref": "#/$defs/Length",
        "description": "Extrusion distance normal to plane"
      },
      "flange_thickness": {
        "$ref": "#/$defs/Length",
        "description": "Horizontal leg/flange; U sides; omit with thickness"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "kind": {
        "description": "Cross-section outline",
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
        "$ref": "#/$defs/Plane",
        "default": "XY",
        "description": "Sketch plane"
      },
      "stem_offset": {
        "description": "T stem centre from left; omitted: centred",
        "maximum": 1000000,
        "minimum": 0,
        "type": "number"
      },
      "thickness": {
        "$ref": "#/$defs/Length",
        "description": "Plate thickness or uniform profile legs"
      },
      "web_thickness": {
        "$ref": "#/$defs/Length",
        "description": "Vertical leg/stem/web; U bottom; omit with thickness"
      },
      "width": {
        "$ref": "#/$defs/Length"
      },
      "inside_width": {
        "$ref": "#/$defs/Length",
        "description": "U/C gap width; replaces width"
      },
      "inside_height": {
        "$ref": "#/$defs/Length",
        "description": "U/C gap height; replaces height"
      },
      "stem_height": {
        "$ref": "#/$defs/Length",
        "description": "T stem below flange; replaces height"
      },
      "inside_fillet": {
        "$ref": "#/$defs/Length",
        "description": "L inside radius"
      },
      "lip": {
        "$ref": "#/$defs/Length",
        "description": "C lip from outside flange face"
      },
      "symmetric": {
        "type": "boolean",
        "description": "Half depth each side of plane"
      }
    },
    "required": [
      "kind",
      "depth"
    ],
    "type": "object",
    "x-pilot-expander": "profile",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "depth": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/depth",
          "x-unit": "mm"
        },
        "flange_thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/flange_thickness",
          "x-unit": "mm"
        },
        "height": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/height",
          "x-unit": "mm"
        },
        "kind": {
          "description": "Choice",
          "enum": [
            "L",
            "T",
            "U",
            "C",
            "rectangle"
          ],
          "type": "string",
          "x-slot": "/kind"
        },
        "plane": {
          "description": "Choice",
          "enum": [
            "XY",
            "XZ",
            "YZ"
          ],
          "type": "string",
          "x-slot": "/plane"
        },
        "stem_offset": {
          "maximum": 1000000,
          "minimum": 0,
          "type": "number",
          "x-slot": "/stem_offset",
          "x-unit": "mm"
        },
        "thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/thickness",
          "x-unit": "mm"
        },
        "web_thickness": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/web_thickness",
          "x-unit": "mm"
        },
        "width": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/width",
          "x-unit": "mm"
        },
        "inside_width": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-unit": "mm",
          "description": "U/C gap width; replaces width",
          "x-slot": "/inside_width"
        },
        "inside_height": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-unit": "mm",
          "description": "U/C gap height; replaces height",
          "x-slot": "/inside_height"
        },
        "stem_height": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-unit": "mm",
          "description": "T stem below flange; replaces height",
          "x-slot": "/stem_height"
        },
        "inside_fillet": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-unit": "mm",
          "description": "L inside radius",
          "x-slot": "/inside_fillet"
        },
        "lip": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-unit": "mm",
          "description": "C lip from outside flange face",
          "x-slot": "/lip"
        },
        "symmetric": {
          "type": "boolean",
          "description": "Half depth each side of plane",
          "x-slot": "/symmetric"
        }
      },
      "required": [
        "depth",
        "kind"
      ]
    }
  },
  "expander": "profile"
}
---

Extrude L T U C or rectangular profiles with web and flange thickness. Built by the `profile` expander.
