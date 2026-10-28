//! Explicit inputs let rustc track recipes separately for every checkout.
#[rustfmt::skip]
pub(super) const EMBEDDED: &[(&str, &str)] = &[
    ("box.md", include_str!("../../recipes/box.md")),
    ("chamfer.md", include_str!("../../recipes/chamfer.md")),
    ("circular_pattern.md", include_str!("../../recipes/circular_pattern.md")),
    ("cross_drilled_box.md", include_str!("../../recipes/cross_drilled_box.md")),
    ("cylinder.md", include_str!("../../recipes/cylinder.md")),
    ("enclosure.md", include_str!("../../recipes/enclosure.md")),
    ("fillet.md", include_str!("../../recipes/fillet.md")),
    ("grid_pattern.md", include_str!("../../recipes/grid_pattern.md")),
    ("honeycomb_pattern.md", include_str!("../../recipes/honeycomb_pattern.md")),
    ("joint.md", include_str!("../../recipes/joint.md")),
    ("linear_pattern.md", include_str!("../../recipes/linear_pattern.md")),
    ("mirror.md", include_str!("../../recipes/mirror.md")),
    ("move.md", include_str!("../../recipes/move.md")),
    ("plate.md", include_str!("../../recipes/plate.md")),
    ("profile.md", include_str!("../../recipes/profile.md")),
    ("revolute_joint.md", include_str!("../../recipes/revolute_joint.md")),
    ("revolve.md", include_str!("../../recipes/revolve.md")),
    ("shell.md", include_str!("../../recipes/shell.md")),
    ("sphere.md", include_str!("../../recipes/sphere.md")),
    ("tube.md", include_str!("../../recipes/tube.md")),
];

#[cfg(test)]
mod tests {
    use super::EMBEDDED;
    use std::{collections::BTreeSet, fs, path::Path};

    #[test]
    fn embedded_file_list_matches_recipe_directory() -> Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("recipes");
        let mut disk = BTreeSet::new();
        for (n, entry) in fs::read_dir(&root)?.enumerate() {
            assert!(n < 128, "recipes directory exceeds 128 entries");
            let entry = entry?;
            if entry.path().extension().is_some_and(|s| s == "md") {
                assert!(
                    entry.file_type()?.is_file(),
                    "{} must be a regular recipe file",
                    entry.path().display()
                );
                assert!(
                    entry.metadata()?.len() <= 65536,
                    "{} exceeds 64 KiB",
                    entry.path().display()
                );
                disk.insert(
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| "recipe filename must be UTF-8")?,
                );
            }
        }
        let listed: BTreeSet<_> = EMBEDDED
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        assert_eq!(
            listed.len(),
            EMBEDDED.len(),
            "duplicate recipe filename in src/recipe_library/files.rs"
        );
        let mut errors = Vec::new();
        for name in disk.difference(&listed) {
            errors.push(format!("recipes/{name} is missing from src/recipe_library/files.rs; add this line:\n    (\"{name}\", include_str!(\"../../recipes/{name}\")),"));
        }
        for name in listed.difference(&disk) {
            errors.push(format!("recipes/{name} is absent; restore that file or remove this line from src/recipe_library/files.rs:\n    (\"{name}\", include_str!(\"../../recipes/{name}\")),"));
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        Ok(())
    }
}
