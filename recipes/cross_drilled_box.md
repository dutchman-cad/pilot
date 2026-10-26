---
{
  "id": "cross_drilled_box",
  "synonyms": [
    "cross drilled cube block crossing bores axes faces"
  ],
  "description": "Holes are centred unless you give offset_u/offset_v from the face centre.",
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
            },
            "offset_u": {
              "type": "number",
              "minimum": -1000000,
              "maximum": 1000000
            },
            "offset_v": {
              "type": "number",
              "minimum": -1000000,
              "maximum": 1000000
            },
            "depth": {
              "$ref": "#/$defs/Length"
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
            "hole_x_diameter"
          ],
          "required": [
            "hole_x_diameter"
          ]
        },
        {
          "fields": [
            "hole_y_diameter"
          ],
          "required": [
            "hole_y_diameter"
          ]
        },
        {
          "fields": [
            "hole_z_diameter"
          ],
          "required": [
            "hole_z_diameter"
          ]
        },
        {
          "fields": [
            "hole_x_offset_u",
            "hole_x_offset_v",
            "hole_x_depth"
          ],
          "required": [
            "hole_x_diameter"
          ]
        },
        {
          "fields": [
            "hole_y_offset_u",
            "hole_y_offset_v",
            "hole_y_depth"
          ],
          "required": [
            "hole_y_diameter"
          ]
        },
        {
          "fields": [
            "hole_z_offset_u",
            "hole_z_offset_v",
            "hole_z_depth"
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
        "hole_y_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_Y/diameter",
          "x-unit": "mm"
        },
        "hole_z_diameter": {
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "type": "number",
          "x-slot": "/hole_Z/diameter",
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
        },
        "hole_x_offset_u": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_X/offset_u",
          "x-unit": "mm"
        },
        "hole_x_offset_v": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_X/offset_v",
          "x-unit": "mm"
        },
        "hole_x_depth": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-slot": "/hole_X/depth",
          "x-unit": "mm"
        },
        "hole_y_offset_u": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_Y/offset_u",
          "x-unit": "mm"
        },
        "hole_y_offset_v": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_Y/offset_v",
          "x-unit": "mm"
        },
        "hole_y_depth": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-slot": "/hole_Y/depth",
          "x-unit": "mm"
        },
        "hole_z_offset_u": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_Z/offset_u",
          "x-unit": "mm"
        },
        "hole_z_offset_v": {
          "type": "number",
          "minimum": -1000000,
          "maximum": 1000000,
          "x-slot": "/hole_Z/offset_v",
          "x-unit": "mm"
        },
        "hole_z_depth": {
          "type": "number",
          "exclusiveMinimum": 0,
          "maximum": 1000000,
          "x-slot": "/hole_Z/depth",
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

Box or cube with one hole per axis. Optional offset_u/offset_v are measured from the face centre: X uses Y,Z; Y uses X,Z; Z uses X,Y. Omit depth for through holes; a positive depth below the body size makes a blind hole. Omit position for centred holes; legacy world positions accept the body centre or either entry face.
