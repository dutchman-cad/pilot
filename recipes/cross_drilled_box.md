---
{
  "id":"cross_drilled_box",
  "description":"Box or cube cross-drilled with centred through holes along one, two or three world axes; holes through all six faces. No sketches or construction planes.",
  "examples":["Make a cube with holes through all six faces", "Build a cross-drilled box", "A box with a hole through each face", "Create a cube with holes along X, Y and Z", "A block with a through hole along Y"],
  "expander":"cross_drilled_box",
  "schema":{
    "type":"object", "additionalProperties":false,
    "x-pilot-expander":"cross_drilled_box",
    "$defs":{
      "Length":{"type":"number","exclusiveMinimum":0,"maximum":1000000},
      "Point3":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-1000000,"maximum":1000000}}
    },
    "properties":{
      "length":{"$ref":"#/$defs/Length"},
      "width":{"$ref":"#/$defs/Length"},
      "height":{"$ref":"#/$defs/Length"},
      "corner":{"$ref":"#/$defs/Point3"},
      "holes":{"type":"array","minItems":1,"maxItems":3,"items":{
        "type":"object","additionalProperties":false,
        "properties":{"axis":{"type":"string","enum":["X","Y","Z"]},"diameter":{"$ref":"#/$defs/Length"},"position":{"$ref":"#/$defs/Point3"}},
        "required":["axis","diameter"]
      }}
    },
    "required":["length","width","height","holes"]
  }
}
---

The corner defaults to the world origin. Each distinct axis gets one solid.hole
using a world face position and an inward direction, through all by default.
Positions default to face centres; explicit positions must lie on the selected
axis face and the whole opening must fit strictly inside it. No auxiliary features.
Equal centred diameters have an exact volume using cylinder intersection volumes;
unequal or offset holes require measured volume.
