//! Regenerate our reviewed file schemas from the typed inputs.
use dutchman_pilot::recipe_library::{schema, Recipe, IDS};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for id in IDS {
        let path = format!("recipes/{id}.md");
        if std::fs::metadata(&path)?.len() > 65536 {
            return Err(format!("{path} exceeds 65536 bytes; shorten the recipe").into());
        }
        let text = std::fs::read_to_string(&path)?;
        let front = text
            .strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---\n"))
            .ok_or("missing frontmatter")?
            .0;
        let mut recipe: Recipe = serde_json::from_str(front)?;
        recipe.schema = schema(id)?;
        std::fs::write(path, recipe.markdown()?)?;
    }
    Ok(())
}
