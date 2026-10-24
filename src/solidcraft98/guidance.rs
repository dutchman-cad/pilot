//! Independently written Pilot lookup. Application help always wins.
use serde_json::{json, Value};
pub fn help(task: &str) -> Value {
    let task = task.to_lowercase();
    if ["fillet", "round", "bevel", "chamfer"]
        .iter()
        .any(|w| task.contains(w))
    {
        json!({"source":"Pilot help","limitation":"SolidCraft 98 cannot fillet solid edges. A sketch corner radius is a limited alternative; this does not round solid edges."})
    } else if task.contains("rename") {
        json!({"source":"Pilot help","limitation":"SolidCraft 98 cannot rename bodies. The design is unchanged by this help lookup."})
    } else if task.contains("volume") {
        json!({"source":"Pilot help","limitation":"SolidCraft 98 does not measure volume. A volume derived from timeline dimensions is a calculation, not an application measurement."})
    } else if ["move", "translate", "position"]
        .iter()
        .any(|w| task.contains(w))
    {
        json!({"source":"Pilot help","examples":[{"name":"doubleclick","arguments":{"name":"Sketch1"}},{"name":"look","arguments":{}}],"limitation":"Move the sketch origin by editing its position fields; dependent features follow. Read current values, preserve dimensions, and change only the requested axis."})
    } else if ["hole", "drill"].iter().any(|w| task.contains(w)) {
        json!({"source":"Pilot help","examples":[{"name":"menu","arguments":{"title":"Sketch","item":"Rectangle..."}},{"name":"menu","arguments":{"title":"Features","item":"Extrude..."}},{"name":"menu","arguments":{"title":"Features","item":"Hole..."}}],"limitation":"Make the plate first. Read each dialog before entering dimensions. Hole coordinates refer to the sketch origin; locate the centre from the plate dimensions. Make separate calls for separate holes; depth zero requests through all. Application help wins conflicts."})
    } else if ["sketch", "rectangle", "cube", "box", "block", "plate"]
        .iter()
        .any(|w| task.contains(w))
    {
        json!({"source":"Pilot help","example":{"name":"menu","arguments":{"title":"Sketch","item":"Rectangle..."}},"limitation":"Start with a sketch, then extrude. Application help is authoritative; use look to read fields before entering dimensions."})
    } else if ["circle", "cylinder"].iter().any(|w| task.contains(w)) {
        json!({"source":"Pilot help","example":{"name":"menu","arguments":{"title":"Sketch","item":"Circle..."}},"limitation":"Read the displayed fields; extrude the sketch separately. Application help wins conflicts."})
    } else if ["extrude", "extrusion"].iter().any(|w| task.contains(w)) {
        json!({"source":"Pilot help","example":{"name":"menu","arguments":{"title":"Features","item":"Extrude..."}},"limitation":"Requires a sketch. Read the displayed fields; Pilot never changes dimensions."})
    } else {
        json!({"source":"Pilot help","limitation":"Pilot has no matching help topic. The application's response above is authoritative; no action was inferred."})
    }
}
