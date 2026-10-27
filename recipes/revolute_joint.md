---
{
  "id": "revolute_joint",
  "description": "Joint or relation between inspected components, at box centres or named origins, with aligned axes and ordered motion limits. Also supports contact pin-and-hole revolute joints.",
  "examples": [
    "Join the pin and hole with a revolute joint",
    "Create a contact revolute joint",
    "Connect the axle components with a hinge joint",
    "Mate the pin component to the hole component",
    "Add a revolute joint at thirty degrees"
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
      "angle_deg": {
        "$ref": "#/$defs/Angle",
        "default": 0,
        "description": "Rotation in degrees; omitted: 0"
      },
      "axis": {
        "description": "World axis",
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
      "hole_component": {
        "description": "Inspected hole component; omit if unique",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "joint_type": {
        "description": "Omit for contact pin/hole; otherwise select type. Limits: joint.list order.",
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
      "pin_component": {
        "description": "Inspected pin component; omit if unique",
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
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
    "required": [],
    "type": "object",
    "x-pilot-expander": "revolute_joint",
    "x-pilot-fill": {
      "groups": [],
      "properties": {
        "angle_deg": {
          "maximum": 360,
          "minimum": -360,
          "type": "number",
          "x-slot": "/angle_deg"
        },
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
        "hole_component": {
          "maxLength": 128,
          "minLength": 1,
          "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
          "type": "string",
          "x-slot": "/hole_component"
        },
        "joint_type": {
          "description": "Omit for contact pin/hole; otherwise select type. Limits: joint.list order.",
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
        "pin_component": {
          "maxLength": 128,
          "minLength": 1,
          "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
          "type": "string",
          "x-slot": "/pin_component"
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
      "required": []
    }
  },
  "expander": "revolute_joint"
}
---

Joint or relation between inspected components, at box centres or named origins, with aligned axes and ordered motion limits. Also supports contact pin-and-hole revolute joints. Built by the `revolute_joint` expander.
