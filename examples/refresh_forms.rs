//! Regenerate typed contracts and their file-owned scalar fill mappings.
use dutchman_pilot::recipe_library::{schema, Recipe, IDS};
use serde_json::{json, Map, Value};

fn fields(
    s: &Value,
    root: &Value,
    path: &str,
    name: &str,
    props: &mut Map<String, Value>,
    groups: &mut Vec<Value>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let resolved = s["$ref"]
        .as_str()
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|p| root.pointer(p))
        .unwrap_or(s);
    if let Some(children) = resolved["properties"].as_object() {
        let mut members = Vec::new();
        let mut required = Vec::new();
        for (key, child) in children {
            if path.is_empty() && key == "hole_diameter" {
                continue;
            }
            let flat = match (path, key.as_str()) {
                ("/bolt_circle", "count") => "bolt_hole_count".into(),
                ("/bolt_circle", "diameter") => "bolt_circle_diameter".into(),
                ("/bolt_circle", "start_deg") => "bolt_start_deg".into(),
                _ if name.is_empty() => key.clone(),
                _ => format!("{name}_{key}"),
            };
            let names = fields(child, root, &format!("{path}/{key}"), &flat, props, groups)?;
            if resolved["required"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == key))
            {
                required.extend(names.clone());
            }
            members.extend(names);
        }
        if path == "/bolt_circle" || path == "/grid" {
            let flat = if path == "/grid" {
                "grid_hole_diameter"
            } else {
                "bolt_hole_diameter"
            };
            let mut field = json!({"type":"number","x-slot":"/hole_diameter","x-unit":"mm"});
            field["description"] = json!("Pattern holes");
            props.insert(flat.into(), field);
            members.push(flat.into());
            required.push(flat.into());
        }
        groups.push(json!({"fields":members,"required":required}));
        // Only top-level required fields belong to the form's required list.
        return Ok(if path.is_empty() { required } else { members });
    }
    if resolved["type"] == "array" {
        if path == "/holes" {
            let mut members = Vec::new();
            for axis in ["X", "Y", "Z"] {
                let mut item = resolved["items"].clone();
                item["properties"]
                    .as_object_mut()
                    .ok_or("hole properties missing")?
                    .remove("axis");
                item["required"] = json!(["diameter"]);
                members.extend(fields(
                    &item,
                    root,
                    &format!("/hole_{axis}"),
                    &format!("hole_{}", axis.to_ascii_lowercase()),
                    props,
                    groups,
                )?);
            }
            // At least one diameter is enforced by typed holes validation.
            return Ok(members);
        }
        let mut members = Vec::new();
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            members.extend(fields(
                &resolved["items"],
                root,
                &format!("{path}/{i}"),
                &format!("{name}_{axis}"),
                props,
                groups,
            )?);
        }
        groups.push(json!({"fields":members,"required":members}));
        return Ok(members);
    }
    let mut field = resolved.clone();
    if let Some(o) = field.as_object_mut() {
        if !(root["x-pilot-expander"] == "joint"
            && (name.starts_with("limit")
                || name.starts_with("component_")
                || name.starts_with("point_")
                || name == "joint_type"))
        {
            o.remove("description");
        }
        o.remove("default");
    }
    field["x-slot"] = json!(path);
    if field["type"] == "number" && !name.ends_with("_deg") && !name.starts_with("limit") {
        field["x-unit"] = json!("mm");
    }
    if root["x-pilot-expander"] == "joint" && name == "axis" {
        field["default"] = json!("Z");
        field["x-default-when"] = json!("joint_type");
    }
    if field.get("enum").is_some() && !(root["x-pilot-expander"] == "joint" && name == "joint_type")
    {
        field["description"] = json!(match name {
            "corner_kind" => "Round/bevel",
            "corner_edges" => "vertical: corners; all: rims too",
            "lip_side" => "inner/outer wall face",
            _ => "Choice",
        });
    }
    props.insert(name.into(), field);
    Ok(vec![name.into()])
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for id in IDS {
        let path = format!("recipes/{id}.md");
        let text = std::fs::read_to_string(&path)?;
        if text.len() > 65536 {
            return Err("recipe exceeds 65536 bytes".into());
        }
        let front = text
            .strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---\n"))
            .ok_or("missing frontmatter")?
            .0;
        let mut recipe: Recipe = serde_json::from_str(front)?;
        recipe.schema = schema(id)?;
        let mut props = Map::new();
        let mut groups = Vec::new();
        let mut required = fields(
            &recipe.schema,
            &recipe.schema,
            "",
            "",
            &mut props,
            &mut groups,
        )?;
        groups.pop();
        if *id == "cross_drilled_box" {
            required.retain(|s| !s.starts_with("hole_"));
        }
        // User-facing singular corner names.
        let renamed: Vec<_> = props
            .keys()
            .filter(|k| k.starts_with("corners_"))
            .cloned()
            .collect();
        for key in renamed {
            if let Some(value) = props.remove(&key) {
                props.insert(key.replacen("corners_", "corner_", 1), value);
            }
            for group in &mut groups {
                for list in ["fields", "required"] {
                    if let Some(values) = group[list].as_array_mut() {
                        for value in values {
                            if *value == key {
                                *value = json!(key.replacen("corners_", "corner_", 1));
                            }
                        }
                    }
                }
            }
        }
        recipe.schema["x-pilot-fill"] =
            json!({"properties":props,"required":required,"groups":groups});
        std::fs::write(path, recipe.markdown()?)?;
    }
    Ok(())
}
