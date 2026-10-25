---
{
  "id": "revolute_joint",
  "description": "Contact revolute joint between a pin component and a hole component, after inspection.",
  "examples": [
    "Join the pin and hole with a revolute joint",
    "Create a contact revolute joint",
    "Connect the axle components with a hinge joint",
    "Mate the pin component to the hole component",
    "Add a revolute joint at thirty degrees"
  ],
  "schema": {
    "x-pilot-expander": "revolute_joint",
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
      "angle_deg": {
        "$ref": "#/$defs/Angle"
      },
      "hole_component": {
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "pin_component": {
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      }
    },
    "required": [],
    "type": "object"
  },
  "expander": "revolute_joint"
}
---

Inspect contact geometry and connect the matching pin and hole components. Built by the `revolute_joint` expander.
