#![cfg(feature = "core")]
use dutchman_pilot::recipe_library::{Bm25, Library, Ranker};
use serde_json::Value;

// Captured on 5b61d6e before changing the library or ranker.
#[test]
fn existing_routes_match_base_branch() -> Result<(), Box<dyn std::error::Error>> {
    let lib = Library::embedded()?;
    let baseline: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/joint-route-base.json"))?;
    for (source, file) in [
        ("tune", "dev-route-tune.json"),
        ("check", "dev-route-check.json"),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(file);
        if !path.exists() && source == "check" {
            eprintln!("dev-route-check.json was absent from base and reference branches; no check rows available");
            continue;
        }
        let rows: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        for row in rows {
            let text = row["text"].as_str().ok_or("missing route text")?;
            let old = baseline
                .iter()
                .find(|r| r["source"] == source && r["text"] == text)
                .ok_or_else(|| format!("missing base ranking for {source}: {text}"))?;
            if row["recipe"] == "joint" {
                continue;
            }
            let found = lib.form_band(text)?;
            assert_eq!(found["recipes"][0]["id"], old["id"], "{text}");
            assert_eq!(found["band"], old["band"], "{text}");
            // Decimal JSON parsing can round a stored score by one ULP.
            let score = found["recipes"][0]["score"]
                .as_f64()
                .ok_or("missing score")?;
            let previous = old["score"].as_f64().ok_or("missing base score")?;
            assert!((score - previous).abs() < 1e-12, "{text}");
        }
    }
    Ok(())
}

#[test]
fn joint_gate_preserves_all_other_scores_and_stays_out_of_other_forms(
) -> Result<(), Box<dyn std::error::Error>> {
    let lib = Library::embedded()?;
    let old = Library {
        recipes: lib
            .recipes
            .iter()
            .filter(|r| r.id != "joint")
            .cloned()
            .collect(),
    };
    for text in [
        "Add a grid of pegs to this existing part",
        "Repeat the last boss in a circular pattern around Z",
        "Add a revolute joint to the pin and hole",
        "hinge pivot two parts",
        "Create a cylinder at the origin",
        "Move this box to its centre",
        "Make a ball",
        "Set the slider travel to 25 mm",
        "write a sonnet",
        "",
        "Connect A and B with a cylindrical joint with limits",
    ] {
        let scores = Bm25.rank(text, &lib.recipes);
        let joint = scores
            .iter()
            .find(|(i, _)| lib.recipes[*i].id == "joint")
            .ok_or("missing joint score")?;
        assert_eq!(joint.1 > 0., text.contains("cylindrical joint"), "{text}");
        for (i, score) in Bm25.rank(text, &old.recipes) {
            let actual = scores
                .iter()
                .find(|(j, _)| lib.recipes[*j].id == old.recipes[i].id)
                .ok_or("missing recipe score")?;
            assert_eq!(actual.1, score, "{text}: {}", old.recipes[i].id);
        }
        if !text.is_empty() && joint.1 == 0. {
            assert_eq!(lib.form_band(text)?, old.form_band(text)?, "{text}");
        }
    }
    Ok(())
}
