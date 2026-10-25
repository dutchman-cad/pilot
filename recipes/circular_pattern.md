---
{
  "id": "circular_pattern",
  "description": "Repeat an existing body or last feature around an axis in a circular pattern with count and full or partial angle.",
  "examples": [
    "put 8 around the centre",
    "circular pattern of this body",
    "repeat the last hole around Z",
    "make 12 copies around the circumference",
    "radial array of six bosses",
    "pattern 5 instances over a partial angle of 120 degrees"
  ],
  "schema": {
    "$defs": {
      "Angle": {
        "description": "Degrees",
        "maximum": 360,
        "minimum": -360,
        "type": "number"
      },
      "Axis": {
        "enum": [
          "X",
          "Y",
          "Z"
        ],
        "type": "string"
      },
      "Count": {
        "maximum": 500,
        "minimum": 2,
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
      "axis": {
        "$ref": "#/$defs/Axis"
      },
      "body": {
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "count": {
        "$ref": "#/$defs/Count"
      },
      "join": {
        "description": "Explicit permission for overlapping joined feature instances; body patterns always create new bodies",
        "type": "boolean"
      },
      "target": {
        "enum": [
          "body",
          "last_feature"
        ],
        "type": "string"
      }
    },
    "required": [
      "target",
      "count",
      "axis",
      "angle_deg"
    ],
    "type": "object",
    "x-pilot-expander": "circular_pattern"
  },
  "expander": "circular_pattern"
}
---

Repeat an existing body or last feature around an axis in a circular pattern with count and full or partial angle. Built by the `circular_pattern` expander.

Counts include the original and total instances are capped at 500. Use target `body` (optionally name `body`) or `last_feature`. Inspect with document.inspect measure:true first. A feature pattern also inspects the last feature definition and inherits its new-body, join, or cut behavior. Body patterns create separate bodies; join is only explicit permission for overlapping joined features, not an invented command parameter. Ambiguous targets, unresolved geometry and possible unrequested overlaps defer. Conservative bounding boxes may require clarification.

Run one solid.pattern.rectangular (linear/grid) or solid.pattern.circular, then document.inspect measure:true. Verify the successful pattern timeline entry, body count and measured volume. Body volume is predicted exactly for disjoint instances; joined/cut feature volume is measured because intersections depend on geometry.

Example selected build: `{"recipe":"circular_pattern","args":{"target":"last_feature","body":"Plate","count":8,"axis":"Z","angle_deg":360}}`. For either an existing body or the last feature, set `target` accordingly; the same typed placement slots apply.
