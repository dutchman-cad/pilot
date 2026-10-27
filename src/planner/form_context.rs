//! Compact facts for extraction, derived only from public inspection results.
use super::*;

pub(crate) const MAX_CONTEXT_CHARS: usize = 8192;

fn payloads(request: &Value) -> Vec<Value> {
    let messages = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let start = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .map_or(0, |i| i + 1);
    let mut out = Vec::new();
    for m in messages[start..]
        .iter()
        .take(4096)
        .filter(|m| m["role"] == "tool")
    {
        if let Some(data) = tool_payload(m) {
            // Context reads are emitted separately so topology cannot be truncated in a batch.
            out.push(data);
        }
    }
    out
}
fn document(data: &[Value]) -> Option<&Value> {
    data.iter().rev().find(|d| {
        d["bodies"].as_array().is_some_and(|a| {
            a.len() <= MAX_BODIES
                && a.iter().all(Value::is_object)
                && d.get("body_count")
                    .is_none_or(|n| n.as_u64() == Some(a.len() as u64))
        }) && d["timeline"].is_array()
    })
}
pub(crate) fn empty_design(request: &Value) -> bool {
    document(&payloads(request))
        .is_some_and(|doc| doc["bodies"].as_array().is_some_and(Vec::is_empty))
}
fn active_features(doc: &Value) -> Vec<&Value> {
    doc["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|f| f["suppressed"] != true && f["rolled_back"] != true && f["error"].is_null())
        .take(12)
        .collect()
}
fn detail<'a>(data: &'a [Value], feature: &Value) -> Option<&'a Value> {
    data.iter().rev().find(|d| {
        d["id"] == feature["id"] && d["name"] == feature["name"] && d["definition"].is_object()
    })
}

pub(crate) fn reads(request: &Value, joint: bool, catalog: &Catalog) -> Result<Vec<Value>, String> {
    let data = payloads(request);
    let Some(doc) = document(&data) else {
        return Ok(vec![call(
            catalog,
            "document.inspect",
            json!({"measure":true}),
        )?]);
    };
    let mut calls = Vec::new();
    for feature in active_features(doc) {
        if detail(&data, feature).is_none() {
            if let Some(name) = feature["name"].as_str().filter(|n| n.len() <= 128) {
                calls.push(call(catalog, "timeline.inspect", json!({"feature":name}))?);
            }
        }
    }
    if joint {
        for (key, command) in [("components", "component.list"), ("joints", "joint.list")] {
            if !data.iter().any(|d| d[key].is_array()) {
                calls.push(call(catalog, command, json!({}))?);
            }
        }
        for body in doc["bodies"].as_array().into_iter().flatten().take(12) {
            for (key, command) in [("faces", "model.faces"), ("edges", "model.edges")] {
                if !data
                    .iter()
                    .any(|d| d["body"] == body["name"] && d[key].is_array())
                {
                    calls.push(call(catalog, command, json!({"body":body["name"]}))?);
                }
            }
        }
    }
    Ok(calls)
}
fn fields(value: &Value, keys: &[&str]) -> Value {
    let mut out = json!({});
    for key in keys {
        if let Some(v) = value.get(key) {
            out[*key] = v.clone();
        }
    }
    out
}
fn selection(request: &Value, doc: &Value) -> Value {
    request
        .get("selection")
        .or_else(|| request["metadata"].get("selection"))
        .or_else(|| doc.get("selection"))
        .cloned()
        .unwrap_or(json!([]))
}
fn selected_names(selection: &Value) -> Vec<&str> {
    let items = selection.as_array().map(Vec::as_slice).unwrap_or(&[]);
    items
        .iter()
        .filter_map(|s| {
            s.as_str()
                .or_else(|| s["body"].as_str())
                .or_else(|| s["name"].as_str())
        })
        .collect()
}
fn last_body(data: &[Value], doc: &Value) -> Option<String> {
    let bodies = doc["bodies"].as_array()?;
    for f in active_features(doc) {
        let Some(d) = detail(data, f) else {
            continue;
        };
        if d["definition"]["operation"] == "new_body" {
            let names = d["definition"]["body_names"].as_array()?;
            if names.len() != 1 {
                return None;
            }
            return names[0]
                .as_str()
                .filter(|n| bodies.iter().filter(|b| b["name"] == *n).count() == 1)
                .map(String::from);
        }
    }
    None
}
pub(crate) fn default_body(request: &Value) -> Result<String, String> {
    let data = payloads(request);
    let doc = document(&data).ok_or("args.body requires a fresh complete document inspection")?;
    let bodies = doc["bodies"]
        .as_array()
        .ok_or("args.body requires inspected bodies")?;
    let selection = selection(request, doc);
    let names = selected_names(&selection);
    if selection.as_array().is_some_and(|a| !a.is_empty()) {
        if names.len() == 1 && bodies.iter().filter(|b| b["name"] == names[0]).count() == 1 {
            return Ok(names[0].into());
        }
        return Err("args.body: selection is ambiguous; select exactly one body or supply its exact inspected name".into());
    }
    if let Some(name) = last_body(&data, doc) {
        return Ok(name);
    }
    if let [body] = bodies.as_slice() {
        if let Some(name) = body["name"].as_str() {
            return Ok(name.into());
        }
    }
    Err("args.body: target is ambiguous; select one body or supply its exact inspected name".into())
}

pub(crate) fn summary(request: &Value, joint: bool) -> String {
    let data = payloads(request);
    let Some(doc) = document(&data) else {
        return String::new();
    };
    let bodies = doc["bodies"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut summary = json!({"units":"mm","body_count":bodies.len(),"bodies":[],"selection":selection(request,doc),"last_created_body":last_body(&data,doc),"default_body":default_body(request).ok()});
    let mut shown = Vec::new();
    let preferred = default_body(request).ok();
    for body in bodies
        .iter()
        .filter(|b| preferred.as_ref().is_some_and(|n| b["name"] == *n))
        .chain(
            bodies
                .iter()
                .filter(|b| preferred.as_ref().is_none_or(|n| b["name"] != *n)),
        )
        .take(12)
    {
        let mut b = fields(body, &["name", "bbox", "volume_mm3"]);
        b["centre"] = body["center_of_mass"].clone();
        shown.push(b);
    }
    summary["bodies"] = json!(shown);
    if bodies.len() > 12 {
        summary["partial"] = json!(true);
    }
    if let Some(last) = active_features(doc).first() {
        summary["last_feature"] = fields(
            detail(&data, last).unwrap_or(last),
            &["id", "name", "type", "definition"],
        );
    }
    if joint {
        summary["components"] = json!(data
            .iter()
            .rev()
            .find_map(|d| d["components"].as_array())
            .into_iter()
            .flatten()
            .take(12)
            .map(|c| fields(c, &["id", "name", "bodies", "occurrences"]))
            .collect::<Vec<_>>());
        let mut contacts = Vec::new();
        for faces in data.iter().filter(|d| d["faces"].is_array()).take(12) {
            if let Some(edges) = data
                .iter()
                .rev()
                .find(|d| d["body"] == faces["body"] && d["edges"].is_array())
            {
                contacts.extend(recipes::revolute::contact_summary(faces, edges));
            }
        }
        contacts.truncate(24);
        summary["pins_and_holes"] = json!(contacts);
    }
    // Remove whole records, retaining valid JSON and an explicit truncation marker.
    while summary.to_string().chars().count() > MAX_CONTEXT_CHARS {
        summary["partial"] = json!(true);
        let mut removed = false;
        for key in ["pins_and_holes", "components", "bodies"] {
            if let Some(a) = summary[key].as_array_mut() {
                if a.pop().is_some() {
                    removed = true;
                    break;
                }
            }
        }
        if !removed {
            return json!({"partial":true,"error":"design context exceeds 8192 characters; select one body and inspect it"}).to_string();
        }
    }
    summary.to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"messages":[{"role":"user","content":"move it"},{"role":"tool","content":json!({"bodies":[{"name":"Plate"},{"name":"Peg"}],"timeline":[{"id":1,"name":"First"},{"id":2,"name":"Second"}]}).to_string()},{"role":"tool","content":json!({"id":2,"name":"Second","definition":{"operation":"new_body","body_names":["Peg"]}}).to_string()}]})
    }
    #[test]
    fn defaults_follow_selection_then_last_creation_then_only_body() {
        let mut req = request();
        assert_eq!(default_body(&req).unwrap(), "Peg");
        req["selection"] = json!(["Plate"]);
        assert_eq!(default_body(&req).unwrap(), "Plate");
        req["selection"] = json!(["Plate", "Peg"]);
        assert!(default_body(&req).unwrap_err().contains("ambiguous"));
        req["selection"] = json!(["Unknown"]);
        assert!(default_body(&req).is_err());
        req.as_object_mut().unwrap().remove("selection");
        req["messages"].as_array_mut().unwrap().pop();
        assert!(default_body(&req).unwrap_err().contains("ambiguous"));
        req["messages"][1]["content"] =
            json!(json!({"bodies":[{"name":"Only"}],"timeline":[]}).to_string());
        assert_eq!(default_body(&req).unwrap(), "Only");
    }
    #[test]
    fn summary_has_strict_character_and_body_caps_and_ignores_old_history() {
        let mut req = request();
        let bodies: Vec<_> = (0..30).map(|n| json!({"name":format!("Part{n}"),"bbox":{"min":[0,0,0],"max":[1,2,3]},"center_of_mass":[0.5,1,1.5]})).collect();
        req["messages"][1]["content"] = json!(json!({"bodies":bodies,"timeline":[]}).to_string());
        let s = summary(&req, true);
        assert!(s.chars().count() <= MAX_CONTEXT_CHARS);
        let s: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(s["body_count"], 30);
        assert_eq!(s["bodies"].as_array().unwrap().len(), 12);
        let long_bodies: Vec<_> = (0..30)
            .map(|n| json!({"name":format!("{n}{}","é".repeat(2000))}))
            .collect();
        req["messages"][1]["content"] =
            json!(json!({"bodies":long_bodies,"timeline":[]}).to_string());
        let capped = summary(&req, true);
        assert!(capped.chars().count() <= MAX_CONTEXT_CHARS);
        assert_eq!(
            serde_json::from_str::<Value>(&capped).unwrap()["partial"],
            true
        );
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"next task"}));
        assert!(summary(&req, true).is_empty());
    }
}
