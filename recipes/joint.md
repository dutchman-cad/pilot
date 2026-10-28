---
{
  "id": "joint",
  "description": "Joint or relation between inspected components at box centres or named origins, with aligned axes and ordered motion limits.",
  "examples": [
    "Connect Base and Link with a slider joint at their box centres",
    "Create a revolute joint with motion limits from -30 to 60 degrees",
    "Add a ball joint between named component origins",
    "Connect two components with a rigid joint"
  ],
  "synonyms": [
    "hinge pivot rotational rotating joint axle spindle"
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
      "axis": {
        "description": "Aligned world axes; omitted: Z",
        "enum": [
          "X",
          "Y",
          "Z"
        ],
        "type": "string"
      },
      "component_a": {
        "description": "Inspected component name; omit only for the two remaining candidates",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "component_b": {
        "description": "Inspected component name; omit only for the two remaining candidates",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "joint_type": {
        "description": "Select joint type. Limits: joint.list order.",
        "enum": [
          "rigid",
          "revolute",
          "slider",
          "cylindrical",
          "pin_slot",
          "planar",
          "ball",
          "parallel",
          "perpendicular",
          "angle",
          "point_on_plane",
          "point_on_line",
          "line_on_plane"
        ],
        "type": "string"
      },
      "limit1_max": {
        "description": "Upper bound; units as limit1_min",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit1_min": {
        "description": "Motion 1: deg for revolute/cylindrical/pin_slot/ball; mm otherwise",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit2_max": {
        "description": "Upper bound; units as limit2_min",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit2_min": {
        "description": "Motion 2: deg for ball/point_on_line; otherwise mm",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit3_max": {
        "description": "Upper bound; units as limit3_min",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit3_min": {
        "description": "Motion 3: mm for parallel/perpendicular/angle; otherwise deg",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit4_max": {
        "description": "Upper bound; units as limit4_min",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit4_min": {
        "description": "Motion 4: deg",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit5_max": {
        "description": "Upper bound; units as limit5_min",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "limit5_min": {
        "description": "Motion 5: deg",
        "maximum": 1000000.0,
        "minimum": -1000000.0,
        "type": "number"
      },
      "point_a": {
        "description": "Named joint.list origin; omit for component box centre",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "point_b": {
        "description": "Named joint.list origin; omit for component box centre",
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "joint_type"
    ],
    "type": "object",
    "x-pilot-expander": "joint",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "axis": {
          "default": "Z",
          "description": "Choice",
          "enum": [
            "X",
            "Y",
            "Z"
          ],
          "type": "string",
          "x-default-when": "joint_type",
          "x-slot": "/axis"
        },
        "component_a": {
          "description": "Inspected component name; omit only for the two remaining candidates",
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/component_a"
        },
        "component_b": {
          "description": "Inspected component name; omit only for the two remaining candidates",
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/component_b"
        },
        "joint_type": {
          "description": "Select joint type. Limits: joint.list order.",
          "enum": [
            "rigid",
            "revolute",
            "slider",
            "cylindrical",
            "pin_slot",
            "planar",
            "ball",
            "parallel",
            "perpendicular",
            "angle",
            "point_on_plane",
            "point_on_line",
            "line_on_plane"
          ],
          "type": "string",
          "x-slot": "/joint_type"
        },
        "limit1_max": {
          "description": "Upper bound; units as limit1_min",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit1_max"
        },
        "limit1_min": {
          "description": "Motion 1: deg for revolute/cylindrical/pin_slot/ball; mm otherwise",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit1_min"
        },
        "limit2_max": {
          "description": "Upper bound; units as limit2_min",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit2_max"
        },
        "limit2_min": {
          "description": "Motion 2: deg for ball/point_on_line; otherwise mm",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit2_min"
        },
        "limit3_max": {
          "description": "Upper bound; units as limit3_min",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit3_max"
        },
        "limit3_min": {
          "description": "Motion 3: mm for parallel/perpendicular/angle; otherwise deg",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit3_min"
        },
        "limit4_max": {
          "description": "Upper bound; units as limit4_min",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit4_max"
        },
        "limit4_min": {
          "description": "Motion 4: deg",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit4_min"
        },
        "limit5_max": {
          "description": "Upper bound; units as limit5_min",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit5_max"
        },
        "limit5_min": {
          "description": "Motion 5: deg",
          "maximum": 1000000.0,
          "minimum": -1000000.0,
          "type": "number",
          "x-slot": "/limit5_min"
        },
        "point_a": {
          "description": "Named joint.list origin; omit for component box centre",
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/point_a"
        },
        "point_b": {
          "description": "Named joint.list origin; omit for component box centre",
          "maxLength": 128,
          "minLength": 1,
          "type": "string",
          "x-slot": "/point_b"
        }
      },
      "required": [
        "joint_type"
      ]
    }
  },
  "expander": "joint"
}
---

Joint or relation between inspected components at box centres or named origins, with aligned axes and ordered motion limits. Built by the `joint` expander.
