---
{
  "id": "linear_pattern",
  "description": "Repeat an existing body or last feature in a linear row with count and spacing or total length; use one pattern, never repeated copies.",
  "examples": [
    "make 50 of these in a row",
    "I need 12 copies of this boss 15 mm apart",
    "repeat the hole every 20 mm",
    "linear pattern of the last feature along X",
    "duplicate this body 10 times along an edge",
    "6 instances over a total length of 100 mm"
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
      "body": {
        "maxLength": 128,
        "minLength": 1,
        "pattern": "^[^\\u0000-\\u001f\\u007f]+$",
        "type": "string"
      },
      "count": {
        "$ref": "#/$defs/Count"
      },
      "direction": {
        "$ref": "#/$defs/Axis"
      },
      "edge": {
        "maxLength": 128,
        "minLength": 1,
        "type": "string"
      },
      "join": {
        "description": "Explicit permission for overlapping joined feature instances; body patterns always create new bodies",
        "type": "boolean"
      },
      "spacing": {
        "$ref": "#/$defs/Length"
      },
      "target": {
        "enum": [
          "body",
          "last_feature"
        ],
        "type": "string"
      },
      "total_length": {
        "$ref": "#/$defs/Length"
      }
    },
    "required": [
      "target",
      "count"
    ],
    "type": "object",
    "x-pilot-expander": "linear_pattern"
  },
  "expander": "linear_pattern"
}
---

Repeat an existing body or last feature in a linear row with count and spacing or total length; use one pattern, never repeated copies. Built by the `linear_pattern` expander.

Counts include the original and total instances are capped at 500. Use target `body` (optionally name `body`) or `last_feature`. Inspect with document.inspect measure:true first. A feature pattern also inspects the last feature definition and inherits its new-body, join, or cut behavior. Body patterns create separate bodies; join is only explicit permission for overlapping joined features, not an invented command parameter. Ambiguous targets, unresolved geometry and possible unrequested overlaps defer. Conservative bounding boxes may require clarification.

Run one solid.pattern.rectangular (linear/grid) or solid.pattern.circular, then document.inspect measure:true. Verify the successful pattern timeline entry, body count and measured volume. Body volume is predicted exactly for disjoint instances; joined/cut feature volume is measured because intersections depend on geometry.

Example selected build: `{"recipe":"linear_pattern","args":{"target":"last_feature","body":"Plate","count":12,"spacing":15,"direction":"X"}}`. For either an existing body or the last feature, set `target` accordingly; the same typed placement slots apply.
