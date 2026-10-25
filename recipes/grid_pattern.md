---
{
  "id": "grid_pattern",
  "description": "Repeat an existing body or last feature in a rectangular grid: rows, columns, plane and spacing in both directions.",
  "examples": [
    "array it 4 by 6",
    "pattern this body in a rectangular grid",
    "repeat the last feature in 3 rows and 5 columns",
    "make 24 copies in a grid 10 mm apart both ways",
    "rectangular array on XZ",
    "repeat these bosses in rows and columns"
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
      "columns": {
        "$ref": "#/$defs/Count"
      },
      "join": {
        "description": "Explicit permission for overlapping joined feature instances; body patterns always create new bodies",
        "type": "boolean"
      },
      "plane": {
        "$ref": "#/$defs/Plane"
      },
      "rows": {
        "$ref": "#/$defs/Count"
      },
      "spacing1": {
        "$ref": "#/$defs/Length"
      },
      "spacing2": {
        "$ref": "#/$defs/Length"
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
      "rows",
      "columns",
      "spacing1",
      "spacing2",
      "plane"
    ],
    "type": "object",
    "x-pilot-expander": "grid_pattern"
  },
  "expander": "grid_pattern"
}
---

Repeat an existing body or last feature in a rectangular grid: rows, columns, plane and spacing in both directions. Built by the `grid_pattern` expander.

Counts include the original and total instances are capped at 500. Use target `body` (optionally name `body`) or `last_feature`. Inspect with document.inspect measure:true first. A feature pattern also inspects the last feature definition and inherits its new-body, join, or cut behavior. Body patterns create separate bodies; join is only explicit permission for overlapping joined features, not an invented command parameter. Ambiguous targets, unresolved geometry and possible unrequested overlaps defer. Conservative bounding boxes may require clarification.

Run one solid.pattern.rectangular (linear/grid) or solid.pattern.circular, then document.inspect measure:true. Verify the successful pattern timeline entry, body count and measured volume. Body volume is predicted exactly for disjoint instances; joined/cut feature volume is measured because intersections depend on geometry.

Example selected build: `{"recipe":"grid_pattern","args":{"target":"body","rows":4,"columns":6,"spacing1":15,"spacing2":20,"plane":"XY"}}`. For either an existing body or the last feature, set `target` accordingly; the same typed placement slots apply.
