---
{
  "id": "cross_drilled_box",
  "description": "Box or cube cross-drilled with centred through holes along one, two or three world axes; holes through all six faces. No sketches or construction planes.",
  "examples": [
    "Make a cube with holes through all six faces",
    "Build a cross-drilled box",
    "A box with a hole through each face",
    "Create a cube with holes along X, Y and Z",
    "A block with a through hole along Y"
  ],
  "schema": {
    "$defs": {
      "Length": {
        "exclusiveMinimum": 0,
        "maximum": 1000000,
        "type": "number"
      },
      "Point3": {
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
    "additionalProperties": false,
    "properties": {
      "corner": {
        "$ref": "#/$defs/Point3",
        "description": "Minimum [x,y,z]; omitted: [0,0,0]"
      },
      "height": {
        "$ref": "#/$defs/Length"
      },
      "holes": {
        "description": "One through hole per distinct axis; maximum 3",
        "items": {
          "additionalProperties": false,
          "properties": {
            "axis": {
              "description": "World axis",
              "enum": [
                "X",
                "Y",
                "Z"
              ],
              "type": "string"
            },
            "diameter": {
              "$ref": "#/$defs/Length"
            },
            "position": {
              "$ref": "#/$defs/Point3",
              "description": "World [x,y,z] on entry face; omitted: face centre"
            }
          },
          "required": [
            "axis",
            "diameter"
          ],
          "type": "object"
        },
        "maxItems": 3,
        "minItems": 1,
        "type": "array"
      },
      "length": {
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
      "holes"
    ],
    "type": "object",
    "x-pilot-expander": "cross_drilled_box"
  },
  "expander": "cross_drilled_box"
}
---

Box or cube cross-drilled with centred through holes along one, two or three world axes; holes through all six faces. No sketches or construction planes. Built by the `cross_drilled_box` expander.
