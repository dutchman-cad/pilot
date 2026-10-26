---
{
  "id": "cross_drilled_box",
  "synonyms": ["cross drilled cube block crossing bores axes faces"],
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
    "x-pilot-expander": "cross_drilled_box",
    "x-pilot-fill": {
      "groups": [
        {
          "fields": [
            "corner_x",
            "corner_y",
            "corner_z"
          ],
          "required": [
            "corner_x",
            "corner_y",
            "corner_z"
          ]
        },
        {
          "fields": [
            "hole_x_position_x",
            "hole_x_position_y",
            "hole_x_position_z"
          ],
          "required": [
            "hole_x_position_x",
            "hole_x_position_y",
            "hole_x_position_z"
          ]
        },
        {
          "fields": [
            "hole_x_diameter",
            "hole_x_position_x",
            "hole_x_position_y",
            "hole_x_position_z"
          ],
          "required": [
            "hole_x_diameter"
          ]
        },
        {
          "fields": [
            "hole_y_position_x",
            "hole_y_position_y",
            "hole_y_position_z"
          ],
          "required": [
            "hole_y_position_x",
            "hole_y_position_y",
            "hole_y_position_z"
          ]
        },
        {
          "fields": [
            "hole_y_diameter",
            "hole_y_position_x",
            "hole_y_position_y",
            "hole_y_position_z"
          ],
          "required": [
            "hole_y_diameter"
          ]
        },
        {
          "fields": [
            "hole_z_position_x",
            "hole_z_position_y",
            "hole_z_position_z"
          ],
          "required": [
            "hole_z_position_x",
            "hole_z_position_y",
            "hole_z_position_z"
          ]
        },
        {
          "fields": [
            "hole_z_diameter",
            "hole_z_position_x",
            "hole_z_position_y",
            "hole_z_position_z"
          ],
          "required": [
            "hole_z_diameter"
          ]
        }
      ],
      "properties": {
        "corner_x": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/corner/0",
          "x-unit": "mm"
        },
        "corner_y": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/corner/1",
          "x-unit": "mm"
        },
        "corner_z": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/corner/2",
          "x-unit": "mm"
        },
        "height": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/height",
          "x-unit": "mm"
        },
        "hole_x_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_X/diameter",
          "x-unit": "mm"
        },
        "hole_x_position_x": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_X/position/0",
          "x-unit": "mm"
        },
        "hole_x_position_y": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_X/position/1",
          "x-unit": "mm"
        },
        "hole_x_position_z": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_X/position/2",
          "x-unit": "mm"
        },
        "hole_y_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_Y/diameter",
          "x-unit": "mm"
        },
        "hole_y_position_x": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Y/position/0",
          "x-unit": "mm"
        },
        "hole_y_position_y": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Y/position/1",
          "x-unit": "mm"
        },
        "hole_y_position_z": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Y/position/2",
          "x-unit": "mm"
        },
        "hole_z_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_Z/diameter",
          "x-unit": "mm"
        },
        "hole_z_position_x": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Z/position/0",
          "x-unit": "mm"
        },
        "hole_z_position_y": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Z/position/1",
          "x-unit": "mm"
        },
        "hole_z_position_z": {
          "maximum": 1000000,
          "minimum": -1000000,
          "type": "number",
          "x-slot": "/hole_Z/position/2",
          "x-unit": "mm"
        },
        "length": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/length",
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
        "width"
      ]
    }
  },
  "expander": "cross_drilled_box"
}
---

Box or cube cross-drilled with centred through holes along one, two or three world axes; holes through all six faces. No sketches or construction planes. Built by the `cross_drilled_box` expander.
