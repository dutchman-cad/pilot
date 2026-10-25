#![cfg(feature = "core")]
#![allow(clippy::unwrap_used)]
use dutchman_pilot::{catalog::Catalog, recipe_library::*};
use serde_json::{json, Value};
#[test]
fn independent_ranking_and_calibrated_misses() {
    let lib = Library::embedded().unwrap();
    let rows = [
        (
            "box",
            [
                "a cuboid please",
                "make me a cube",
                "a plain block",
                "rectangular solid box",
                "box 73 by 42 by 19 millimetres",
            ],
        ),
        (
            "cylinder",
            [
                "a round rod please",
                "cylinder radius 11 height 23",
                "a circular disc",
                "cylindrical solid",
                "solid rod 15 mm high",
            ],
        ),
        (
            "sphere",
            [
                "sphere radius 15",
                "a spherical ball",
                "solid ball please",
                "a sphere 25 mm across",
                "a round ball",
            ],
        ),
        (
            "tube",
            [
                "a hollow pipe please",
                "tube outside radius 14",
                "a tubular sleeve",
                "pipe with a bore",
                "round tube 75 mm long",
            ],
        ),
        (
            "profile",
            [
                "L section extruded",
                "T profile with a web",
                "U channel please",
                "C profile extrude 75 mm",
                "rectangular constrained profile",
            ],
        ),
        (
            "plate",
            [
                "plate with bolt circle holes",
                "mounting plate and centre bore",
                "flat plate with hole grid",
                "perforated plate please",
                "plate drilled with six holes",
            ],
        ),
        (
            "enclosure",
            [
                "electronics enclosure please",
                "enclosure with an outer lip",
                "lipped enclosure open top",
                "open bottom enclosure",
                "enclosure floor 4 mm walls 3 mm",
            ],
        ),
        (
            "revolute_joint",
            [
                "revolute contact joint please",
                "hinge joint between components",
                "mate pin to hole component",
                "axle components hinge joint",
                "revolute joint angle 42 degrees",
            ],
        ),
        (
            "fillet",
            [
                "fillet all the edges",
                "round every edge",
                "soften edges of body",
                "apply edge fillets",
                "all edges rounded radius 3 mm",
            ],
        ),
        (
            "chamfer",
            [
                "bevel every edge",
                "chamfer all edges please",
                "edges beveled by 3 mm",
                "apply chamfers",
                "bevel the body edges",
            ],
        ),
        (
            "move",
            [
                "move body in X",
                "translate my part",
                "shift the body left",
                "move the solid upwards",
                "XYZ displacement of the body",
            ],
        ),
        (
            "mirror",
            [
                "mirror part in XY",
                "reflect my solid",
                "mirrored copy please",
                "mirror body keeping original",
                "reflect across YZ plane",
            ],
        ),
        (
            "shell",
            [
                "shell a rectangular box",
                "hollow box with uniform walls",
                "open top shell please",
                "shelled box wall 4 mm",
                "box shell with thin wall",
            ],
        ),
        (
            "revolve",
            [
                "revolve sketch rectangle",
                "spin rectangular sketch",
                "full revolution about Z",
                "rectangle swept around Z",
                "revolve about Z through 360 degrees",
            ],
        ),
    ];
    let mut failures = Vec::new();
    for (id, queries) in rows {
        for q in queries {
            let r = lib.find(q, &Bm25).unwrap();
            if r["recipes"][0]["id"] != id || r["hit"] != true {
                failures.push(format!(
                    "{q}: wanted {id}; got {} score {} hit {}",
                    r["recipes"][0]["id"], r["recipes"][0]["score"], r["hit"]
                ));
            }
        }
    }
    for q in [
        "weather tomorrow",
        "write a sonnet",
        "book a flight",
        "sort a spreadsheet",
        "translate French poetry",
        "how much is bitcoin",
        "solve a differential equation",
        "play jazz music",
        "draw a kitten",
        "explain photosynthesis",
        "design an involute gear",
        "generate a helical thread",
    ] {
        let r = lib.find(q, &Bm25).unwrap();
        if r["hit"] != false {
            failures.push(format!(
                "off-topic {q}: got {} score {}",
                r["recipes"][0]["id"], r["recipes"][0]["score"]
            ));
        }
        assert_eq!(r["recipes"].as_array().unwrap().len(), 3);
        if r["hit"] == false {
            assert!(r["message"]
                .as_str()
                .unwrap()
                .contains("No good recipe hit"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
fn sample(id: &str) -> Value {
    match id {
        "profile" => {
            json!({"kind":"L","plane":"XY","width":40,"height":30,"thickness":3,"depth":12})
        }
        "plate" => {
            json!({"length":80,"width":60,"thickness":5,"plane":"XY","centred":true,"bore_diameter":8})
        }
        "enclosure" => {
            json!({"length":80,"width":60,"height":30,"wall":2,"open":"top","centred":false})
        }
        "revolute_joint" => json!({"angle_deg":0}),
        "box" => json!({"length":40,"width":30,"height":20}),
        "cylinder" | "revolve" => json!({"radius":10,"height":20}),
        "sphere" => json!({"radius":10}),
        "tube" => json!({"outer_radius":10,"inner_radius":5,"height":20}),
        "fillet" | "chamfer" => json!({"body":"Box1","amount":2}),
        "move" => json!({"body":"Box1","delta":[10,0,-5]}),
        "mirror" => json!({"body":"Box1","plane":"YZ","keep_original":true}),
        "shell" => json!({"length":40,"width":30,"height":20,"wall":2}),
        _ => Value::Null,
    }
}
#[test]
fn generated_schema_and_expander_cases() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[{"role":"user","content":"build my selected recipe"}]});
    for r in &lib.recipes {
        assert_eq!(r.schema, schema(&r.expander).unwrap());
        let args = sample(&r.id);
        validate(&args, &r.schema).unwrap();
        // Joint discovery depends on contact topology, not the parameter domain.
        if r.id != "revolute_joint" {
            lib.build(&json!({"recipe":r.id,"args":args}), &req, &cat)
                .unwrap();
        }
        let o = args.as_object().unwrap();
        for key in o.keys() {
            for bad in [Value::Null, json!({}), json!([1, 2]), json!("unknown")] {
                let mut changed = args.clone();
                changed[key] = bad;
                let valid = validate(&changed, &r.schema).is_ok();
                let expanded =
                    dutchman_pilot::planner::recipe_expand(&r.expander, &changed, &req, &cat)
                        .is_ok();
                if r.id != "revolute_joint" {
                    assert_eq!(valid, expanded, "{}.{key}", r.id);
                }
            }
        }
        let mut extra = args.clone();
        extra["typo"] = json!(1);
        assert!(validate(&extra, &r.schema).is_err());
        assert!(lib
            .build(&json!({"recipe":r.id,"args":extra}), &req, &cat)
            .is_err());
    }
}
#[test]
fn prompt_budget_includes_descriptions_and_tools() {
    assert!(FIND_DESCRIPTION.chars().count() < 400);
    let total = build_tool().to_string().len() + find_tool().to_string().len();
    eprintln!(
        "recipe tools and descriptions: {total} bytes; find description: {} chars",
        FIND_DESCRIPTION.chars().count()
    );
    assert!(total < 7000, "{total}");
}
#[test]
fn sequence_is_typed_bounded_and_catalogue_checked() {
    let cat = Catalog::bundled().unwrap();
    let mut r = Recipe {
        id: "sequence_test".into(),
        description: "A box sequence".into(),
        examples: vec!["box".into(); 3],
        schema: json!({"type":"object","properties":{"length":{"$ref":"#/$defs/Length"}},"required":["length"],"additionalProperties":false,"$defs":definitions()}),
        expander: "sequence".into(),
        calls: vec![
            json!({"tool":"execute","arguments":{"command":"solid.box","params":{"length":{"$param":"length"},"width":20,"height":10}}}),
        ],
    };
    let req = json!({"messages":[]});
    let envelope = json!({"recipe":"sequence_test","args":{"length":30}});
    let lib = Library::parse([r.markdown().unwrap().as_str()]).unwrap();
    let p = lib.build(&envelope, &req, &cat).unwrap();
    assert_eq!(p.calls[0]["arguments"]["params"]["length"], 30);
    assert!(lib
        .build(
            &json!({"recipe":"sequence_test","args":{"length":0}}),
            &req,
            &cat
        )
        .is_err());
    r.calls = vec![r.calls[0].clone(); 64];
    assert_eq!(
        Library::parse([r.markdown().unwrap().as_str()])
            .unwrap()
            .build(&envelope, &req, &cat)
            .unwrap()
            .calls
            .len(),
        64
    );
    r.calls[0]["arguments"]["command"] = json!("nonexistent.command");
    assert!(Library::parse([r.markdown().unwrap().as_str()])
        .unwrap()
        .build(&envelope, &req, &cat)
        .is_err());
    r.calls = vec![r.calls[0].clone(); 65];
    assert!(Recipe::parse(&r.markdown().unwrap()).is_err());
}
#[cfg(feature = "proxy")]
#[test]
fn audit_draft_requires_review_and_round_trips() {
    let dir = std::env::temp_dir().join(format!("pilot-recipe-draft-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let audit = dir.join("audit.jsonl");
    std::fs::write(&audit,json!({"kind":"direct_recipe_calls","after":{"request":"Make a block 41 by 23 by 17 mm","calls":[{"tool":"execute","arguments":{"command":"solid.box","params":{"length":41,"width":23,"height":17}}}]}}).to_string()).unwrap();
    let review = dir.join("review");
    let path = draft(&audit, 1, &review).unwrap();
    assert!(path.starts_with(&review));
    let r = Recipe::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(r.examples[0], "Make a block 41 by 23 by 17 mm");
    assert_eq!(r.expander, "sequence");
    assert_eq!(r.schema["properties"].as_object().unwrap().len(), 3);
    assert_eq!(Library::embedded().unwrap().recipes.len(), 14);
    assert!(draft(&audit, 1, &review)
        .unwrap_err()
        .contains("cannot create"));
    assert!(draft(&audit, 2, &review).is_err());
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_dutchman-pilot"))
        .current_dir(&dir)
        .args([
            "recipe",
            "new",
            "--from-audit",
            "audit.jsonl",
            "--turn",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let printed = String::from_utf8(result.stdout).unwrap();
    assert!(dir.join(printed.trim()).is_file());
    assert!(!dir.join("recipes").exists());
    std::fs::write(
        &audit,
        json!({"kind":"plan","after":{"request":"malformed audit","calls":[{"arguments":false}]}})
            .to_string(),
    )
    .unwrap();
    assert!(draft(&audit, 1, &review)
        .unwrap_err()
        .contains("params object"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn geometry_schema_extension_and_generated_numeric_boundaries() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[]});
    for r in &lib.recipes {
        let original = sample(&r.id);
        for (key, v) in original.as_object().unwrap() {
            if !v.is_number() {
                continue;
            }
            for n in [-1000001.0, -1.0, 0.0, 0.001, 1.0, 999999.0, 1000001.0] {
                let mut args = original.clone();
                args[key] = json!(n);
                let accepted = validate(&args, &r.schema).is_ok();
                let expanded =
                    dutchman_pilot::planner::recipe_expand(&r.expander, &args, &req, &cat).is_ok();
                if r.id != "revolute_joint" {
                    assert_eq!(accepted, expanded, "{}.{key}={n}", r.id);
                }
            }
        }
    }
    for (id, args) in [
        (
            "tube",
            json!({"inner_radius":20,"outer_radius":10,"height":30}),
        ),
        (
            "shell",
            json!({"length":10,"width":10,"height":10,"wall":6}),
        ),
        (
            "profile",
            json!({"kind":"L","plane":"XY","width":10,"height":10,"thickness":20,"depth":10}),
        ),
        (
            "plate",
            json!({"length":10,"width":10,"thickness":1,"plane":"XY","centred":true,"bore_diameter":30}),
        ),
    ] {
        assert!(validate(&args, &schema(id).unwrap()).is_err(), "{id}");
    }
    let a = lib.find("tube 20mm long", &Bm25).unwrap();
    let b = lib.find("tube 2 inches long", &Bm25).unwrap();
    assert_eq!(a["recipes"], b["recipes"]);
}

#[cfg(feature = "proxy")]
#[test]
fn development_override_is_explicit_and_rejects_duplicates_or_schema_drift() {
    let dir = std::env::temp_dir().join(format!("pilot-recipe-override-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let r = Library::embedded()
        .unwrap()
        .recipes
        .into_iter()
        .find(|r| r.id == "sphere")
        .unwrap();
    std::fs::write(dir.join("sphere.md"), r.markdown().unwrap()).unwrap();
    let lib = Library::load(Some(&dir)).unwrap();
    assert_eq!(lib.recipes.len(), 1);
    assert!(lib
        .build(
            &json!({"recipe":"box","args":{}}),
            &json!({"messages":[]}),
            &Catalog::bundled().unwrap()
        )
        .is_err());
    let mut changed = r.clone();
    changed.schema["additionalProperties"] = json!(true);
    assert!(Recipe::parse(&changed.markdown().unwrap()).is_err());
    std::fs::write(dir.join("duplicate.md"), r.markdown().unwrap()).unwrap();
    assert!(Library::load(Some(&dir)).unwrap_err().contains("duplicate"));
    std::fs::remove_dir_all(dir).unwrap();
}
