//! Hand-authored examples verified only against the public CLI.
use serde_json::{json, Value};

pub(super) fn example(task: &str) -> Option<(&'static str, Value, &'static str)> {
    let lower = task.to_ascii_lowercase();
    let words: Vec<_> = lower.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let has = |choices: &[&str]| words.iter().any(|w| choices.contains(w));
    if !has(&["hole", "holes", "counterbore", "counterbores"]) {
        return None;
    }
    if has(&[
        "measure",
        "volume",
        "many",
        "count",
        "edit",
        "delete",
        "remove",
        "resize",
        "change",
        "rename",
        "thread",
        "threaded",
        "tapped",
        "clearance",
        "countersink",
        "countersunk",
    ]) {
        return None;
    }
    let (params, rule) = if has(&["row", "rows"]) {
        return Some(("solid.pattern.rectangular", json!({"tool":"execute","arguments":{"command":"solid.pattern.rectangular","params":{"features":["Hole1"],"dir1":[1,0,0],"count1":4,"spacing1":12}}}),
            "After a side-wall solid.hole named Hole1, pattern its feature (not the body). dir1 runs along the wall; spacing1 is centre spacing in mm; count1 includes the original. Use the returned hole feature name."));
    } else if has(&["counterbore", "counterbores"]) {
        (json!({"position":[20,15,20],"direction":[0,0,-1],"diameter":6,"type":"counterbore","cb_diameter":10,"cb_depth":3}),
        "Example on a 60x40x20 box at the origin: position is on the entry face; direction drills inward. diameter is the bore; cb_diameter and cb_depth size the entry recess (mm). Omit depth for through all; add depth for a blind bore, measured from entry.")
    } else if has(&["underside", "underneath", "bottom"]) {
        (json!({"position":[20,15,0],"direction":[0,0,1],"diameter":6,"depth":7}),
        "Example on a 60x40x20 box at the origin: underside is Z=0, direction +Z drills inward. position is on the entry face, not at the blind end. depth is 7 mm from entry; omit depth for through all.")
    } else if has(&["angled", "sloped", "inclined", "offset", "rotated"]) {
        (json!({"position":[20,15,30],"direction":[0,-0.6,-0.8],"diameter":6,"depth":7}),
        "Example entry [20,15,30] on a face with outward normal [0,0.6,0.8]: direction is its negative. Read model.faces for the body: use point (on the face), not centroid (may be outside); plane_normal gives outward normal. Use world coordinates including offsets; depth is along the drill axis (mm).")
    } else if has(&["side", "wall", "walls"]) {
        (json!({"position":[20,0,10],"direction":[0,1,0],"diameter":6}),
        "Example on a 60x40x20 box at the origin: Y=0 wall drills +Y; Y=40 drills -Y; X=0 drills +X; X=60 drills -X. position lies on the entry face. Omit depth for through all; add depth in mm for blind. For a hollow wall, set depth to wall thickness to avoid drilling the opposite wall.")
    } else {
        (json!({"position":[20,15,20],"direction":[0,0,-1],"diameter":6}),
        "Example on a 60x40x20 box at the origin: position lies on the entry face, direction drills into the body (negative outward normal). Omit depth for through all; add depth in mm for blind. Read model.faces for point and plane_normal on other faces.")
    };
    Some((
        "solid.hole",
        json!({"tool":"execute","arguments":{"command":"solid.hole","params":params}}),
        rule,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::catalog::Catalog;
    #[test]
    fn hole_examples_are_short_catalogue_backed_and_task_specific() {
        let cat = Catalog::bundled().unwrap();
        for (task, fragment) in [
            ("blind hole from the underside", "\"depth\":7"),
            ("hole through a side wall", "\"direction\":[0,1,0]"),
            ("hole on an angled face", "\"direction\":[0,-0.6,-0.8]"),
            ("hole on an offset face", "world coordinates"),
            ("counterbores", "\"cb_depth\":3"),
            ("hole rows along a wall", "solid.pattern.rectangular"),
        ] {
            let text = cat.how_text(task).unwrap();
            assert!(text.contains(fragment), "{text}");
            assert!(text.len() < 900);
            let answer = cat.how(task).unwrap();
            assert_eq!(answer.matches.len(), 1);
            assert!(!answer.matches[0].example.is_null());
        }
        assert!(cat.how_text(&"x".repeat(257)).is_err());
        assert!(Catalog::from_json("[]")
            .unwrap()
            .how("side wall hole")
            .unwrap()
            .matches
            .is_empty());
        assert!(super::example("measure hole volume").is_none());
        assert!(super::example("how many holes").is_none());
        assert!(super::example("angled face").is_none());
    }
}
