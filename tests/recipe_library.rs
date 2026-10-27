#![cfg(feature = "core")]
#![allow(clippy::unwrap_used, clippy::panic)]
use dutchman_pilot::{catalog::Catalog, recipe_library::*};
use serde_json::{json, Value};

#[test]
fn recipe_text_accepts_windows_line_endings_and_one_leading_bom() {
    let text = include_str!("../recipes/box.md");
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let expected = Recipe::parse(&text).unwrap().markdown().unwrap();
    for ending in ["\n", "\r\n", "\r"] {
        for bom in ["", "\u{feff}"] {
            let input = format!("{bom}{}", text.replace('\n', ending));
            let recipe = Recipe::parse(&input).unwrap();
            assert_eq!(recipe.markdown().unwrap(), expected, "{ending:?}, {bom:?}");
            assert_eq!(Library::parse([input.as_str()]).unwrap().recipes.len(), 1);
        }
    }
    assert!(Recipe::parse(&format!("\u{feff}\u{feff}{text}")).is_err());
    // The cap applies to original bytes, even when normalization shrinks them.
    let oversized = format!("{text}{}", "\r\n".repeat(32768));
    assert_eq!(
        Recipe::parse(&oversized).unwrap_err(),
        "recipe exceeds 64 KiB"
    );
}

#[test]
fn cross_drilled_boxes_exact_english_and_typed_validation() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (text, count) in [
        (
            "Build a 24 mm cube with 6 mm holes through all six faces",
            3,
        ),
        ("Make a 30 mm cube with a 4 mm hole through each face", 3),
        (
            "Create a 32 mm cube with 5 mm through holes along X, Y and Z",
            3,
        ),
        ("Build a cross-drilled 28 mm cube with 7 mm holes", 3),
        (
            "Make a box 24 by 30 by 36 mm with 6 mm through holes along X and Z",
            2,
        ),
        ("Create a 26 mm cube with a 3 mm through hole along Y", 1),
        ("Make a 1 inch cube with 1/4 inch holes along X and Y", 2),
    ] {
        let ranked = lib.find(text, &Bm25).unwrap();
        assert_eq!(
            ranked["recipes"][0]["id"], "cross_drilled_box",
            "{text}: {ranked}"
        );
        let p=dutchman_pilot::planner::plan(&json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]}),&cat).unwrap().unwrap_or_else(||panic!("{text}"));
        assert_eq!(p.rule, "cross_drilled_box");
        assert_eq!(
            p.calls
                .iter()
                .filter(|c| c["arguments"]["command"] == "solid.hole")
                .count(),
            count
        );
    }
    let base = json!({"length":24,"width":30,"height":36,"holes":[{"axis":"Y","diameter":4,"position":[9,0,12]}]});
    let build = |args: Value| {
        lib.build(
            &json!({"recipe":"cross_drilled_box","args":args}),
            &json!({"messages":[]}),
            &cat,
        )
    };
    let p = build(base.clone()).unwrap();
    assert_eq!(
        p.calls[1]["arguments"]["params"]["position"],
        json!([9., 0., 12.])
    );
    assert_eq!(
        p.calls[1]["arguments"]["params"]["direction"],
        json!([0., 1., 0.])
    );
    for holes in [
        json!([]),
        json!([{"axis":"Q","diameter":4}]),
        json!([{"axis":"Y","diameter":0}]),
        json!([{"axis":"Y","diameter":4},{"axis":"Y","diameter":4}]),
        json!([{"axis":"Y","diameter":24}]),
        json!([{"axis":"Y","diameter":4,"position":[9,1,12]}]),
    ] {
        let mut args = base.clone();
        args["holes"] = holes;
        assert!(build(args).is_err());
    }
    for text in [
        "Build a 24 mm cube with holes through all six faces",
        "Build a 24 mm cube with 6 mm blind holes along X",
        "Build a 24 mm cube with 6 mm through holes along X then move it",
        "Build a 24 mm cube with 6 mm through holes along X at 4 5 6",
    ] {
        assert!(dutchman_pilot::planner::plan(&json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]}),&cat).unwrap().is_none(),"{text}");
    }
}
#[test]
fn cross_hole_centres_offsets_depths_and_errors() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let build = |args| {
        lib.build(
            &json!({"recipe":"cross_drilled_box","args":args}),
            &json!({"messages":[]}),
            &cat,
        )
    };
    let base = json!({"length":20,"width":20,"height":20,"hole_x_diameter":8,"hole_y_diameter":8,"hole_z_diameter":8});
    let centred = build(base.clone()).unwrap();
    let mut partial = base.clone();
    partial["hole_x_position_y"] = json!(10);
    assert_eq!(build(partial.clone()).unwrap().calls, centred.calls);
    partial["hole_x_position_y"] = json!(0);
    let error = build(partial).unwrap_err();
    assert!(
        error.contains("offset_u") && error.contains("e.g."),
        "{error}"
    );
    let shifted = json!({"length":20,"width":30,"height":40,"corner":[-5,3,7],"hole_x_diameter":4,"hole_x_position_y":20,"hole_x_position_z":24});
    assert_eq!(
        build(shifted).unwrap().calls[1]["arguments"]["params"]["position"],
        json!([15., 20., 24.])
    );
    for value in [0, 10, 20] {
        let mut args = base.clone();
        args["hole_x_position_x"] = json!(value);
        assert_eq!(build(args).unwrap().calls, centred.calls);
    }
    for value in [5, 21] {
        let mut args = base.clone();
        args["hole_x_position_x"] = json!(value);
        let error = build(args).unwrap_err();
        assert!(
            error.contains("offset_u") && error.contains("e.g."),
            "{error}"
        );
    }
    for position in [json!([10, 10, 10]), json!([0, 10, 10]), json!([20, 10, 10])] {
        let mut args = json!({"length":20,"width":20,"height":20,"holes":[]});
        args["holes"] = json!([
            {"axis":"X","diameter":8,"position":position},
            {"axis":"Y","diameter":8,"position":[10,10,10]},
            {"axis":"Z","diameter":8,"position":[10,10,10]}
        ]);
        let p = build(args).unwrap();
        assert_eq!(
            p.expected.as_ref().unwrap().created_volume_mm3,
            centred.expected.as_ref().unwrap().created_volume_mm3
        );
        assert!((p.expected.as_ref().unwrap().created_volume_mm3.unwrap() - 5708.1).abs() < 0.1);
    }
    for (axis, position) in [
        ("x", json!([20., 12., 7.])),
        ("y", json!([12., 20., 7.])),
        ("z", json!([12., 7., 20.])),
    ] {
        let args = json!({"length":20,"width":20,"height":20,format!("hole_{axis}_diameter"):4,format!("hole_{axis}_offset_u"):2,format!("hole_{axis}_offset_v"):-3,format!("hole_{axis}_depth"):5});
        let p = build(args).unwrap();
        let params = &p.calls[1]["arguments"]["params"];
        assert_eq!(params["position"], position);
        assert_eq!(params["depth"], 5.);
        assert_eq!(p.expected.as_ref().unwrap().created_volume_mm3, None);
    }
    for hole in [
        json!({"axis":"X","diameter":8,"position":[9,10,10]}),
        json!({"axis":"Y","diameter":8,"position":[10,0,0]}),
        json!({"axis":"Z","diameter":8,"offset_u":10}),
        json!({"axis":"X","diameter":8,"depth":21}),
    ] {
        let error = build(json!({"length":20,"width":20,"height":20,"holes":[hole]})).unwrap_err();
        assert!(
            error.contains("holes[0]") && (error.contains("e.g.") || error.contains("example")),
            "{error}"
        );
    }
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "cross_drilled_box")
        .unwrap()
        .schema;
    let compact = forms::compact(schema);
    for form in [forms::flat(schema), compact.clone()] {
        assert!(form["properties"]
            .as_object()
            .unwrap()
            .keys()
            .all(|k| !k.contains("_position_")));
        assert_eq!(
            form["description"],
            "Holes are centred unless you give offset_u/offset_v from the face centre."
        );
    }
    for (axis, coordinate, offset, position) in [
        ("x", "y", "offset_u", json!([20., 12., 10.])),
        ("x", "z", "offset_v", json!([20., 10., 12.])),
        ("y", "x", "offset_u", json!([12., 20., 10.])),
        ("y", "z", "offset_v", json!([10., 20., 12.])),
        ("z", "x", "offset_u", json!([12., 10., 20.])),
        ("z", "y", "offset_v", json!([10., 12., 20.])),
    ] {
        let key = format!("hole_{axis}_position_{coordinate}");
        let mut args = json!({"length":20,"width":20,"height":20,format!("hole_{axis}_diameter"):4,key.clone():"12 mm"});
        let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        assert!(!changes.is_empty());
        assert!(args.get(&key).is_none());
        assert_eq!(args[format!("hole_{axis}_{offset}")], 2.);
        assert_eq!(
            build(args.clone()).unwrap().calls[1]["arguments"]["params"]["position"],
            position
        );
        args[key.clone()] = json!(13);
        assert!(build(args).unwrap_err().contains(offset));
        for value in [0, 20, 21] {
            let args = json!({"length":20,"width":20,"height":20,format!("hole_{axis}_diameter"):4,key.clone():value});
            let error = build(args).unwrap_err();
            assert!(error.contains(offset) && error.contains("e.g."), "{error}");
        }
    }
    assert!(compact["properties"].get("hole_x_offset_u").is_some());
    assert_eq!(
        forms::to_slots(&base, schema).unwrap()["holes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

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
        "cross_drilled_box" => {
            json!({"length":24,"width":30,"height":36,"holes":[{"axis":"X","diameter":6},{"axis":"Y","diameter":6},{"axis":"Z","diameter":6}]})
        }
        "honeycomb_pattern" => {
            json!({"length":150,"width":300,"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4})
        }
        "linear_pattern" => json!({"target":"body","count":5,"spacing":10,"direction":"X"}),
        "grid_pattern" => {
            json!({"target":"body","rows":3,"columns":4,"spacing1":10,"spacing2":10,"plane":"XY"})
        }
        "circular_pattern" => json!({"target":"body","count":8,"axis":"Z","angle_deg":360}),
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
        let mut typed_schema = r.schema.clone();
        typed_schema.as_object_mut().unwrap().remove("x-pilot-fill");
        assert_eq!(typed_schema, schema(&r.expander).unwrap());
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
        synonyms: vec![],
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
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("\u{feff}{}", text.replace('\n', "\r\n"))).unwrap();
    let r = Recipe::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(Library::load(Some(&review)).unwrap().recipes.len(), 1);
    assert_eq!(r.examples[0], "Make a block 41 by 23 by 17 mm");
    assert_eq!(r.expander, "sequence");
    assert_eq!(r.schema["properties"].as_object().unwrap().len(), 3);
    assert_eq!(Library::embedded().unwrap().recipes.len(), 19);
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
    for bom in ["", "\u{feff}"] {
        std::fs::write(
            dir.join("sphere.md"),
            format!("{bom}{}", r.markdown().unwrap().replace('\n', "\r\n")),
        )
        .unwrap();
        let loaded = Library::load(Some(&dir)).unwrap();
        assert_eq!(loaded.recipes.len(), 1);
        assert_eq!(loaded.recipes[0].id, "sphere");
    }
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

#[test]
fn pattern_retrieval_independent_requests_and_negatives() {
    let lib = Library::embedded().unwrap();
    for (id, queries) in [
        (
            "linear_pattern",
            vec![
                "make 50 of these in a row",
                "I need 12 copies of this boss 15 mm apart",
                "repeat the hole every 20 mm",
                "linear pattern along Y",
                "duplicate this block 9 times along X",
                "7 instances in a row",
                "repeat the last feature along an edge",
                "pattern this cylinder at 18 mm spacing",
                "make 16 copies spaced 6 mm apart",
                "make 30 of these",
                "array the boss along Z",
            ],
        ),
        (
            "grid_pattern",
            vec![
                "array it 4 by 6",
                "rectangular pattern 3 rows 8 columns",
                "repeat this in a grid",
                "grid of 5 by 9 bodies",
                "pattern 4 rows at 15 mm spacing",
                "copies in 2 rows and 7 columns",
                "array 8 by 3 on YZ",
                "repeat the last feature in columns and rows",
            ],
        ),
        (
            "circular_pattern",
            vec![
                "put 8 around the centre",
                "arrange 11 around Z",
                "circular pattern this body",
                "repeat 7 bosses around the axis",
                "radial array of 13 cylinders",
                "pattern this hole around Y through 180 degrees",
                "12 copies around the circumference",
                "circular array of last feature",
                "repeat 6 instances around X",
            ],
        ),
    ] {
        for query in queries {
            let result = lib.find(query, &Bm25).unwrap();
            assert_eq!(result["recipes"][0]["id"], id, "{query}: {result}");
            assert_eq!(result["hit"], true, "{query}");
        }
    }
    for query in [
        "copy this to the clipboard",
        "make a copy of the file",
        "copy 50 files 15 mm apart",
        "repeat the poem 12 times",
        "make a box 4 by 6 by 8",
        "mirror this body",
        "move this cylinder 20 mm along X",
        "what is the volume of this body",
        "copy this text into a spreadsheet",
    ] {
        let result = lib.find(query, &Bm25).unwrap();
        assert!(
            !result["recipes"][0]["id"]
                .as_str()
                .unwrap()
                .ends_with("_pattern"),
            "{query}: {result}"
        );
    }
}

#[test]
fn pattern_expansion_defers_and_verifies() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let args = json!({"target":"body","count":50,"spacing":10,"direction":"X"});
    let build =
        |a: &Value, r: &Value| lib.build(&json!({"recipe":"linear_pattern","args":a}), r, &cat);
    assert_eq!(
        build(&args, &json!({"messages":[]})).unwrap().calls[0]["arguments"]["command"],
        "document.inspect"
    );
    let data = json!({"bodies":[{"name":"Seed","volume_mm3":24,"bbox":{"min":[0,0,0],"max":[2,3,4]}}],"timeline":[]});
    let req = json!({"messages":[{"role":"tool","content":data.to_string()}]});
    let p = build(&args, &req).unwrap();
    assert_eq!(p.calls.len(), 2);
    assert_eq!(
        p.calls[0]["arguments"]["command"],
        "solid.pattern.rectangular"
    );
    assert_eq!(p.expected.as_ref().unwrap().bodies, Some(50));
    assert_eq!(p.expected.as_ref().unwrap().volume_mm3, Some(1200.));
    assert!(dutchman_pilot::planner::completion(&req, p.expected.as_ref().unwrap()).is_some());
    for changed in [
        json!({"count":501}),
        json!({"spacing":1}),
        json!({"direction":"unknown"}),
        json!({"total_length":100}),
    ] {
        let mut a = args.clone();
        for (k, v) in changed.as_object().unwrap() {
            a[k] = v.clone();
        }
        assert!(build(&a, &req).is_err(), "{a}");
    }
    let mut ambiguous = data.clone();
    ambiguous["bodies"]
        .as_array_mut()
        .unwrap()
        .push(data["bodies"][0].clone());
    assert!(build(
        &args,
        &json!({"messages":[{"role":"tool","content":ambiguous.to_string()}]})
    )
    .is_err());
    let mut total = args.clone();
    total.as_object_mut().unwrap().remove("spacing");
    total["total_length"] = json!(490);
    assert_eq!(
        build(&total, &req).unwrap().calls[0]["arguments"]["params"]["spacing1"],
        10.
    );
}

#[test]
fn repeated_creation_hint_uses_sizes_and_step_in_current_turn() {
    use dutchman_pilot::planner::repetition_hint;
    let calls:Vec<_>=(0..3).map(|i|json!({"function":{"name":"execute","arguments":json!({"command":"solid.box","params":{"length":2,"width":3,"height":4,"corner":[i*10,0,0]}}).to_string()}})).collect();
    let req = json!({"messages":[{"role":"user","content":"make a row"},{"role":"assistant","tool_calls":calls}]});
    assert!(repetition_hint(&req)
        .unwrap()
        .contains("solid.pattern.rectangular"));
    let mut changed = req.clone();
    changed["messages"][1]["tool_calls"][2]["function"]["arguments"] =
        json!({"command":"solid.box","params":{"length":5,"width":3,"height":4,"corner":[20,0,0]}})
            .to_string()
            .into();
    assert!(repetition_hint(&changed).is_none());
    changed["messages"][1]["tool_calls"][2]["function"]["arguments"] =
        json!({"command":"solid.box","params":{"length":2,"width":3,"height":4,"corner":[27,0,0]}})
            .to_string()
            .into();
    assert!(repetition_hint(&changed).is_none());
    let mut next = req;
    next["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"next turn"}));
    assert!(repetition_hint(&next).is_none());
}

#[test]
fn pattern_hint_for_cylinders_copies_and_copied_sketch_extrusions() {
    use dutchman_pilot::planner::repetition_hint;
    for command in ["solid.cylinder", "occurrence.copy", "timeline.paste"] {
        let calls:Vec<_>=(0..3).map(|i| {
            let params=match command {"solid.cylinder"=>json!({"radius":2,"height":4,"base":[0,i*12,0]}),"occurrence.copy"=>json!({"component":"Part","translate":[0,i*12,0]}),_=>json!({"translate":[0,i*12,0]})};
            json!({"function":{"name":"execute","arguments":json!({"command":command,"params":params}).to_string()}})
        }).collect();
        assert!(repetition_hint(&json!({"messages":[{"role":"user","content":"repeat these"},{"role":"assistant","tool_calls":calls}]})).is_some(),"{command}");
    }
    let mut commands = vec![json!({"command":"sketch.copy","params":{"entities":[1,2,3,4]}})];
    for i in 0..3 {
        commands.push(json!({"command":"sketch.paste","params":{"translate":[i*12,0],"sketch":format!("Copy{i}")}}));
        commands.push(json!({"command":"solid.extrude","params":{"sketch":format!("Copy{i}"),"distance":4,"operation":"new"}}));
    }
    let req = json!({"messages":[{"role":"user","content":"repeat these extrusions"},{"role":"assistant","tool_calls":[{"function":{"name":"batch","arguments":json!({"commands":commands}).to_string()}}]}]});
    assert!(repetition_hint(&req).is_some());
}

#[test]
fn fill_calibration() {
    let lib = Library::embedded().unwrap();
    for (query, id, fill, hit) in [
        (
            "Extrude an L profile on XZ 47 wide 33 tall legs 5 thick depth 22",
            "profile",
            true,
            true,
        ),
        (
            "A mounting plate with a bore and bolt circle holes and rounded corners",
            "plate",
            true,
            true,
        ),
        (
            "An open enclosure with walls and an inner lip",
            "enclosure",
            true,
            true,
        ),
        (
            "Write a story about an enclosure with a lip",
            "enclosure",
            false,
            true,
        ),
        ("a cuboid please", "box", false, true),
        ("translate French poetry", "move", false, false),
        ("write a sonnet", "box", false, false),
    ] {
        let found = lib.find(query, &Bm25).unwrap();
        let score = found["recipes"][0]["score"].as_f64().unwrap();
        assert_eq!(found["recipes"][0]["id"], id, "{query}");
        assert_eq!(
            lib.form_band(query).unwrap()["band"] == "fill",
            fill,
            "{query}: {score}"
        );
        assert_eq!(found["hit"], hit, "{query}");
    }
}

#[test]
fn fill_forms_are_short_and_documented() {
    use dutchman_pilot::recipe_library::forms::{compact, line};
    fn documented(v: &Value) {
        match v {
            Value::Object(o) => {
                if o.contains_key("enum") {
                    assert!(
                        o.get("description")
                            .and_then(Value::as_str)
                            .is_some_and(|d| !d.is_empty()),
                        "{v}"
                    );
                }
                if let Some(d) = o.get("description").and_then(Value::as_str) {
                    assert!(d.chars().count() <= 80, "{d}");
                }
                for child in o.values() {
                    documented(child);
                }
            }
            Value::Array(a) => {
                for child in a {
                    documented(child);
                }
            }
            _ => {}
        }
    }
    for r in Library::embedded().unwrap().recipes {
        documented(&r.schema);
        let mut form = compact(&r.schema);
        form["properties"]["decline"] = json!({"type":"boolean"});
        let size = form.to_string().chars().count() + line(&r.id).chars().count();
        eprintln!("{}: {size} characters", r.id);
        // All catalogue joint/relation types need up to five motion limit pairs.
        let cap = if r.id == "revolute_joint" { 3000 } else { 1200 };
        assert!(size <= cap, "{}: {size}", r.id);
    }
    documented(&dutchman_pilot::planner::shortcut_tool());
}

#[test]
fn probe_plate_error_and_omitted_defaults() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[]});
    let mut plate = json!({"length":80,"width":80,"thickness":8,"centred":true,"bore_diameter":30,"hole_diameter":8,"bolt_circle":{"count":6,"diameter":56},"corners":{"kind":"fillet","size":10,"edges":"all"}});
    let error = lib
        .build(&json!({"recipe":"plate","args":plate}), &req, &cat)
        .unwrap_err();
    assert_eq!(error, "corners.size 10 with edges \"all\" must be below 4 (half the 8 mm thickness); for the four corner edges through the thickness use edges \"vertical\"");
    plate["corners"].as_object_mut().unwrap().remove("edges");
    let defaults = lib
        .build(&json!({"recipe":"plate","args":plate}), &req, &cat)
        .unwrap();
    plate["plane"] = json!("XY");
    plate["corners"]["edges"] = json!("vertical");
    assert_eq!(
        defaults.calls,
        lib.build(&json!({"recipe":"plate","args":plate}), &req, &cat)
            .unwrap()
            .calls
    );
    let mut enclosure =
        json!({"length":80,"width":50,"height":30,"wall":2,"lip":{"width":1,"height":2}});
    let defaults = lib
        .build(&json!({"recipe":"enclosure","args":enclosure}), &req, &cat)
        .unwrap();
    enclosure["lip"]["side"] = json!("inner");
    enclosure["centred"] = json!(false);
    enclosure["open"] = json!("top");
    assert_eq!(
        defaults.calls,
        lib.build(&json!({"recipe":"enclosure","args":enclosure}), &req, &cat)
            .unwrap()
            .calls
    );
    let mut plate = json!({"length":12,"width":14,"thickness":2});
    let defaults = lib
        .build(&json!({"recipe":"plate","args":plate}), &req, &cat)
        .unwrap();
    plate["plane"] = json!("XY");
    plate["centred"] = json!(false);
    assert_eq!(
        defaults.calls,
        lib.build(&json!({"recipe":"plate","args":plate}), &req, &cat)
            .unwrap()
            .calls
    );
}

#[test]
fn field_errors_include_values_numeric_limits_and_fixes() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[]});
    for (id, args, expected) in [
        ("box", json!({"length":1000001,"width":4,"height":3}), "args.length 1000001 must be above 0 and at most 1000000; change args.length to a number in this range"),
        ("box", json!({"width":4,"height":3}), "args.length <missing> requires 1 value; supply length from the request"),
        ("enclosure", json!({"length":80,"width":50,"height":30,"wall":2,"lip":{"width":2,"height":2}}), "lip.width 2 must be below wall 2; reduce lip.width"),
        ("profile", json!({"kind":"L","width":20,"height":30,"thickness":20,"depth":5}), "web_thickness 20 must be below 20 (cut side); reduce thickness or web_thickness"),
        ("tube", json!({"outer_radius":5,"inner_radius":6,"height":4}), "inner_radius 6 must be below outer_radius 5; reduce inner_radius"),
    ] {
        let error = lib.build(&json!({"recipe":id,"args":args}), &req, &cat).unwrap_err();
        assert_eq!(error, expected);
    }
    let error = lib.build(&json!({"recipe":"plate","args":{"length":20,"width":30,"thickness":2,"corners":{"kind":"fillet","size":1,"edges":"corners"}}}), &req, &cat).unwrap_err();
    assert!(error.contains("args.corners.edges \"corners\""));
    assert!(error.contains("vertical"));
    assert!(error.contains("use a listed value"));
}

#[test]
fn every_flat_form_maps_to_the_same_typed_commands() {
    use dutchman_pilot::recipe_library::forms::{flat, to_slots};
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[{"role":"user","content":"build the selected recipe"}],"tools":[{"function":{"name":"execute"}}]});
    for r in &lib.recipes {
        let mut nested = sample(&r.id);
        if r.id == "revolute_joint" {
            nested = json!({"pin_component":"Base","hole_component":"Arm","angle_deg":45});
        }
        if r.id == "plate" {
            nested = json!({"length":80,"width":80,"thickness":8,"centred":true,"bore_diameter":30,"hole_diameter":8,"bolt_circle":{"count":6,"diameter":56,"start_deg":0},"corners":{"kind":"fillet","size":10,"edges":"vertical"}});
        }
        if r.id == "enclosure" {
            nested["lip"] = json!({"width":1,"height":2,"side":"inner"});
        }
        if r.id == "cross_drilled_box" {
            nested["corner"] = json!([0, 0, 0]);
        }
        let form = flat(&r.schema);
        let mut args = json!({});
        for (key, field) in form["properties"].as_object().unwrap() {
            // Scaled aliases are independently covered by the consistency tests.
            if field.get("x-scale").is_some() {
                continue;
            }
            assert!(
                matches!(
                    field["type"].as_str(),
                    Some("number" | "integer" | "boolean" | "string")
                ),
                "{}.{key}: {field}",
                r.id
            );
            let path = field["x-slot"].as_str().unwrap();
            let value = if let Some(rest) = path.strip_prefix("/hole_").filter(|r| r.contains('/'))
            {
                let (axis, rest) = rest.split_once('/').unwrap();
                nested["holes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|h| h["axis"] == axis)
                    .and_then(|h| h.pointer(&format!("/{rest}")))
            } else {
                nested.pointer(path)
            };
            if let Some(value) = value {
                args[key] = field["x-scale"].as_f64().map_or_else(
                    || value.clone(),
                    |scale| json!(value.as_f64().unwrap() / scale),
                );
            }
        }
        // A plate can contain both patterns with a shared hole diameter. Test
        // one feature at a time to avoid activating an unspecified pattern.
        if r.id == "plate" {
            args.as_object_mut().unwrap().remove("grid_hole_diameter");
        }
        let slots = to_slots(&args, &r.schema).unwrap_or_else(|e| panic!("{}: {e}", r.id));
        assert_eq!(slots, nested, "{}", r.id);
        let build = |a| {
            lib.build(&json!({"recipe":r.id,"args":a}), &req, &cat)
                .unwrap()
        };
        assert_eq!(build(args).calls, build(nested).calls, "{}", r.id);
        let found = lib.find(&r.examples[0], &Bm25).unwrap();
        if let Some(found) = found["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == r.id)
        {
            assert_eq!(found["schema"], form);
        }
    }
}

#[test]
fn flat_coercions_and_partial_features_are_explicit() {
    use dutchman_pilot::recipe_library::forms::{coerce, flat, to_slots};
    let lib = Library::embedded().unwrap();
    let plate = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let mut args = json!({"length":"2 in","width":"56 mm","thickness":"8","centred":"yes","plane":"xy","bolt_hole_count":"6","corner_kind":"ROUND","corner_edges":"side"});
    let changes = coerce(&mut args, &flat(plate)).unwrap();
    assert_eq!(changes.len(), 8);
    assert_eq!(
        args,
        json!({"length":50.8,"width":56.,"thickness":8.,"centred":true,"plane":"XY","bolt_hole_count":6,"corner_kind":"fillet","corner_edges":"vertical"})
    );
    for (id, args, missing) in [
        (
            "plate",
            json!({"bolt_hole_count":6}),
            "bolt_circle_diameter",
        ),
        (
            "plate",
            json!({"bolt_circle_diameter":56,"bolt_hole_count":6}),
            "bolt_hole_diameter",
        ),
        ("plate", json!({"grid_rows":2}), "grid_columns"),
        ("plate", json!({"corner_kind":"fillet"}), "corner_size"),
        ("enclosure", json!({"lip_width":1}), "lip_height"),
        ("move", json!({"delta_x":5}), "delta_y"),
        (
            "cross_drilled_box",
            json!({"hole_x_position_x":0}),
            "length",
        ),
    ] {
        let r = &lib.recipes.iter().find(|r| r.id == id).unwrap().schema;
        let error = to_slots(&args, r).unwrap_err();
        assert!(error.contains(missing), "{error}");
    }
    let error = to_slots(&json!({"bolt_circle":56}), plate).unwrap_err();
    assert!(
        error.contains("bolt_hole_count") && error.contains("bolt_circle_diameter"),
        "{error}"
    );
    for value in ["NaN", "infinity", "", "six", "2 in"] {
        let mut args = json!({"bolt_hole_count":value});
        assert!(coerce(&mut args, &flat(plate)).unwrap().is_empty());
    }
    for (key, value, want) in [
        ("centred", "no", json!(false)),
        ("corner_kind", "bevel", json!("chamfer")),
        ("corner_edges", "corners", json!("vertical")),
    ] {
        let mut args = json!({key:value});
        assert_eq!(coerce(&mut args, &flat(plate)).unwrap().len(), 1);
        assert_eq!(args[key], want);
    }
}

#[test]
fn flat_grid_positions_and_conflicts_preserve_typed_behavior() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[]});
    for (id, flat, nested) in [
        (
            "plate",
            json!({"length":100,"width":80,"thickness":8,"grid_rows":2,"grid_columns":3,"grid_pitch_x":20,"grid_pitch_y":20,"grid_hole_diameter":4,"corner_kind":"chamfer","corner_size":2}),
            json!({"length":100,"width":80,"thickness":8,"grid":{"rows":2,"columns":3,"pitch_x":20,"pitch_y":20},"hole_diameter":4,"corners":{"kind":"chamfer","size":2}}),
        ),
        (
            "cross_drilled_box",
            json!({"length":24,"width":30,"height":36,"hole_z_diameter":4,"hole_z_position_x":12,"hole_z_position_y":15,"hole_z_position_z":0}),
            json!({"length":24,"width":30,"height":36,"holes":[{"axis":"Z","diameter":4,"position":[12,15,36]}]}),
        ),
    ] {
        let build = |args| {
            lib.build(&json!({"recipe":id,"args":args}), &req, &cat)
                .unwrap()
        };
        assert_eq!(build(flat).calls, build(nested).calls);
    }
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let mut args = json!({"bolt_hole_count":6,"bolt_circle_diameter":56,"bolt_hole_diameter":4,"bolt_circle":{"count":5,"diameter":60}});
    assert!(forms::to_slots(&args, schema)
        .unwrap_err()
        .contains("conflicting"));
    args.as_object_mut().unwrap().remove("bolt_circle");
    args["grid_rows"] = json!(2);
    args["grid_columns"] = json!(2);
    args["grid_pitch_x"] = json!(10);
    args["grid_pitch_y"] = json!(10);
    args["grid_hole_diameter"] = json!(4);
    assert_eq!(forms::to_slots(&args, schema).unwrap()["hole_diameter"], 4);
    args["grid_hole_diameter"] = json!(5);
    assert!(forms::to_slots(&args, schema)
        .unwrap_err()
        .contains("conflicting"));
    let mut bounded = schema.clone();
    bounded["x-pilot-fill"]["properties"]["length"]["x-slot"] = json!("/a/b/c/d/e/f/g/h/i");
    assert!(forms::to_slots(&json!({"length":1}), &bounded)
        .unwrap_err()
        .contains("1..8"));
}

#[test]
fn consistent_representations_and_corner_defaults() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let req = json!({"messages":[]});
    let build = |id: &str, args: Value| lib.build(&json!({"recipe":id,"args":args}), &req, &cat);
    for kind in ["L", "T", "U", "C"] {
        let base = json!({"kind":kind,"width":50,"height":40,"depth":30,"thickness":8});
        for fields in [
            json!({"web_thickness":8}),
            json!({"flange_thickness":8}),
            json!({"web_thickness":8,"flange_thickness":8}),
        ] {
            let mut redundant = base.clone();
            redundant
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            assert_eq!(
                build("profile", base.clone()).unwrap().calls,
                build("profile", redundant.clone()).unwrap().calls
            );
            redundant["web_thickness"] = json!(7);
            let error = build("profile", redundant).unwrap_err();
            assert!(
                error.contains("thickness 8") && error.contains("web_thickness 7"),
                "{error}"
            );
        }
    }
    for id in ["cylinder", "sphere", "revolve", "tube"] {
        let mut base = if id == "tube" {
            json!({"outer_radius":10,"inner_radius":8,"height":30})
        } else {
            json!({"radius":10,"height":30})
        };
        if id == "sphere" {
            base.as_object_mut().unwrap().remove("height");
        }
        let mut redundant = base.clone();
        let (radius, diameter) = if id == "tube" {
            ("outer_radius", "outer_diameter")
        } else {
            ("radius", "diameter")
        };
        redundant[diameter] = json!(20);
        assert_eq!(
            build(id, base.clone()).unwrap().calls,
            build(id, redundant.clone()).unwrap().calls
        );
        redundant.as_object_mut().unwrap().remove(radius);
        assert_eq!(
            build(id, base).unwrap().calls,
            build(id, redundant.clone()).unwrap().calls
        );
        redundant[radius] = json!(9);
        let error = build(id, redundant).unwrap_err();
        assert!(
            error.contains(diameter)
                && error.contains("20")
                && error.contains(radius)
                && error.contains('9'),
            "{error}"
        );
    }
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "linear_pattern")
        .unwrap()
        .schema;
    let args = json!({"target":"body","count":6,"spacing":10,"total_length":50,"direction":"X"});
    assert_eq!(
        forms::to_slots(&args, schema).unwrap().get("total_length"),
        None
    );
    let mut bad = args;
    bad["total_length"] = json!(60);
    assert!(forms::to_slots(&bad, schema)
        .unwrap_err()
        .contains("spacing 10"));
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let mut args = json!({"corner_size":10});
    let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|c| c["defaulted"] == true));
    assert_eq!(args["corner_kind"], "fillet");
    assert_eq!(args["corner_edges"], "vertical");
    for value in [json!([1, 2, 3, 4]), json!({"edges":"vertical"})] {
        args["corner_edges"] = value;
        let error = forms::to_slots(&args, schema).unwrap_err();
        assert!(
            error.contains("vertical")
                && error.contains("four corner")
                && error.contains("all")
                && error.contains("rims"),
            "{error}"
        );
    }
}

#[test]
fn audited_first_fills_replay_without_inventing_dimensions() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/fill-audit4.json")).unwrap();
    assert_eq!(cases.len(), 4);
    for (index, case) in cases.iter().enumerate() {
        let result = lib.build(case, &json!({"messages":[]}), &cat);
        match index {
            0 | 1 => {
                assert!(result.is_ok(), "{result:?}");
            }
            2 => {
                let error = result.unwrap_err();
                assert!(
                    error.contains("corner_edges")
                        && error.contains("vertical")
                        && error.contains("not a list"),
                    "{error}"
                );
            }
            3 => {
                assert!(result.unwrap_err().contains("lip_height missing"));
            }
            _ => panic!("unexpected audit case {index}"),
        }
    }
}

#[test]
fn enums_reject_containers_and_redundancy_preserves_bounds() {
    let lib = Library::embedded().unwrap();
    for recipe in &lib.recipes {
        for (key, field) in recipe.schema["x-pilot-fill"]["properties"]
            .as_object()
            .unwrap()
        {
            if field.get("enum").is_none() {
                continue;
            }
            for value in [json!([]), json!({})] {
                let error = forms::to_slots(&json!({key:value}), &recipe.schema).unwrap_err();
                for choice in field["enum"].as_array().unwrap() {
                    assert!(error.contains(choice.as_str().unwrap()), "{error}");
                }
                assert!(error.contains("not a list or an object"), "{error}");
            }
        }
    }
    let plate = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let mixed = json!({"corner_size":2,"corners":{"kind":"chamfer","size":2}});
    assert_eq!(
        forms::to_slots(&mixed, plate).unwrap()["corners"],
        json!({"kind":"chamfer","size":2,"edges":"vertical"})
    );
    let moving = &lib.recipes.iter().find(|r| r.id == "move").unwrap().schema;
    assert_eq!(
        forms::to_slots(
            &json!({"delta":[1,2,3],"delta_x":1,"delta_y":2,"delta_z":3}),
            moving
        )
        .unwrap()["delta"],
        json!([1, 2, 3])
    );
    let error = forms::to_slots(
        &json!({"delta":[1,2,3],"delta_x":4,"delta_y":2,"delta_z":3}),
        moving,
    )
    .unwrap_err();
    assert!(
        error.contains("delta_x 4") && error.contains("1 and 4"),
        "{error}"
    );
    let profile = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    assert!(forms::to_slots(
        &json!({"thickness":1000000,"web_thickness":1000000.0001}),
        profile
    )
    .is_err());
    let pattern = &lib
        .recipes
        .iter()
        .find(|r| r.id == "linear_pattern")
        .unwrap()
        .schema;
    assert!(forms::to_slots(
        &json!({"count":3,"spacing":600000,"total_length":1200000}),
        pattern
    )
    .is_err());
    let mut dimensional_default = plate["x-pilot-fill"].clone();
    dimensional_default["properties"]["corner_size"]["default"] = json!(2);
    dimensional_default["properties"]["corner_size"]["x-default-when"] = json!("corner_kind");
    let mut changed = lib
        .recipes
        .iter()
        .find(|r| r.id == "plate")
        .unwrap()
        .clone();
    changed.schema["x-pilot-fill"] = dimensional_default;
    assert!(Recipe::parse(&changed.markdown().unwrap())
        .unwrap_err()
        .contains("never a dimension"));
}

#[test]
fn joined_boss_pattern_spacing_uses_tool_extent() {
    let library = Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let snapshot = json!({"bodies":[{"name":"Base","volume_mm3":49071.16,"bbox":{"min":[0,0,0],"max":[90,90,12]}}],"timeline":[{"id":1,"name":"Cylinder1","type":"CylinderFeature","error":null}]});
    let detail = json!({"id":1,"name":"Cylinder1","type":"CylinderFeature","error":null,"definition":{"operation":"join","base":[15,15,6],"axis":[0,0,1],"radius":"5","height":"6"}});
    let request = json!({"messages":[{"role":"user","content":"Pattern that boss"},{"role":"tool","content":snapshot.to_string()},{"role":"tool","content":detail.to_string()}]});
    let mut args = json!({"target":"boss","body":null,"columns":4,"rows":4,"spacing1":20,"spacing2":20,"plane":"XY"});
    let plan = library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &request,
            &catalog,
        )
        .unwrap();
    assert_eq!(
        plan.calls[0]["arguments"]["params"]["features"],
        json!(["Cylinder1"])
    );
    assert_eq!(plan.expected.as_ref().unwrap().bodies, Some(1));
    let mut expression_request = request.clone();
    let mut expression_detail = detail.clone();
    expression_detail["definition"]["radius"] = json!("(10 mm) / 2");
    expression_detail["inputs"] = json!([{"label":"Radius","param":"d4"}]);
    expression_request["messages"][2]["content"] = json!(expression_detail.to_string());
    let lookup = library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &expression_request,
            &catalog,
        )
        .unwrap();
    assert_eq!(lookup.calls[0]["arguments"]["command"], "parameters.list");
    expression_request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":json!({"parameters":[{"feature":"Cylinder1","name":"d4","unit":"mm","expression":"(10 mm) / 2","value":5}]}).to_string()}));
    let resolved = library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &expression_request,
            &catalog,
        )
        .unwrap();
    assert_eq!(resolved.calls[0], plan.calls[0]);
    expression_request["messages"][3]["content"] = json!(json!({"parameters":[{"feature":"Cylinder1","name":"d4","unit":"mm","expression":"stale","value":5}]}).to_string());
    assert!(library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &expression_request,
            &catalog
        )
        .unwrap_err()
        .contains("current evaluated input"));
    args["spacing1"] = json!(8);
    let error = library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &request,
            &catalog,
        )
        .unwrap_err();
    assert!(error.contains("overlapping instance bounds"), "{error}");
    let mut wrong = request.clone();
    wrong["messages"][1]["content"] = json!(json!({"bodies":snapshot["bodies"],"timeline":[{"id":1,"name":"Hole1","type":"HoleFeature","error":null}]}).to_string());
    assert!(library
        .build(
            &json!({"recipe":"grid_pattern","args":args}),
            &wrong,
            &catalog
        )
        .unwrap_err()
        .contains("does not match"));
}

#[test]
fn circular_post_bounds_rotate_and_step_angle_is_not_a_short_arc() {
    let library = Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let mut request = json!({"messages":[{"role":"user","content":"a post at X=30, 6 copies around Z"},{"role":"tool","content":json!({"bodies":[{"name":"Post","volume_mm3":1608.495,"bbox":{"min":[26,-4,0],"max":[34,4,8]}}],"timeline":[]}).to_string()}]});
    for axis in ["0, 0, 1", "[0, 0, 1]", "(0 0 1)"] {
        let envelope = json!({"recipe":"circular_pattern","args":{"target":"body","body":"post","axis":axis,"count":6,"angle_deg":60}});
        let plan = library.build(&envelope, &request, &catalog).unwrap();
        assert_eq!(plan.calls[0]["arguments"]["params"]["angle"], 360.);
        assert_eq!(plan.calls[0]["arguments"]["params"]["axis"], "Z");
        assert_eq!(plan.expected.as_ref().unwrap().bodies, Some(6));
        request["messages"][0]["content"] =
            json!("pattern this post over a partial arc of 60 degrees around Z");
        let error = library.build(&envelope, &request, &catalog).unwrap_err();
        assert!(
            error.contains("overlapping rotated instance bounds"),
            "{error}"
        );
        assert!(!error.contains("spacing [0,0]"), "{error}");
        request["messages"][0]["content"] = json!("a post at X=30, 6 copies around Z");
    }
}

#[test]
fn joined_sketch_extrusions_skip_unmeasurable_overlap_checks() {
    let library = Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let request = json!({"messages":[{"role":"user","content":"Pattern the existing boss"},{"role":"tool","content":json!({"bodies":[{"name":"Base","volume_mm3":10251.3,"bbox":{"min":[0,0,0],"max":[50,50,9]}}],"timeline":[{"id":3,"name":"Extrude1","type":"ExtrudeFeature"}]}).to_string()},{"role":"tool","content":json!({"id":3,"name":"Extrude1","type":"ExtrudeFeature","definition":{"operation":"join","participants":["Base"],"distance":"5 mm"}}).to_string()}]});
    for (id, args) in [
        (
            "grid_pattern",
            json!({"target":"last_feature","rows":3,"columns":3,"spacing1":16,"spacing2":16,"plane":"XY"}),
        ),
        (
            "circular_pattern",
            json!({"target":"last_feature","axis":"[0, 0, 1]","count":6,"angle_deg":360}),
        ),
        (
            "linear_pattern",
            json!({"target":"last_feature","direction":"X","count":3,"spacing":16}),
        ),
    ] {
        let plan = library
            .build(&json!({"recipe":id,"args":args}), &request, &catalog)
            .unwrap();
        assert_eq!(plan.slots["overlap_precheck"]["status"], "skipped");
        assert_eq!(
            plan.calls[0]["arguments"]["params"]["features"],
            json!(["Extrude1"])
        );
        let expected = plan.expected.unwrap();
        assert_eq!(expected.bodies, Some(1));
        assert!(expected.volume_mm3.is_none());
        assert!(expected.volume_range_mm3.is_some());
    }
}

#[test]
fn every_joint_type_retrieves_one_form_with_ordered_limits() {
    let lib = Library::embedded().unwrap();
    let tasks: Value =
        serde_json::from_str(include_str!("../bench/sets/joints/tasks.json")).unwrap();
    for task in tasks["tasks"].as_array().unwrap() {
        let found = lib.find(task["prompt"].as_str().unwrap(), &Bm25).unwrap();
        assert_eq!(found["recipes"][0]["id"], "revolute_joint");
        assert!(found["recipes"][0]["score"].as_f64().unwrap() >= 4.5);
    }
    for kind in [
        "revolute",
        "hinge",
        "pin",
        "slider",
        "slide",
        "prismatic",
        "cylindrical",
        "pin_slot",
        "pin slot",
        "ball",
        "ball and socket",
        "spherical",
        "planar",
        "rigid",
        "parallel",
        "perpendicular",
        "angle",
        "point_on_plane",
        "point_on_line",
        "line_on_plane",
    ] {
        for text in [
            format!("{kind} joint"),
            format!("Connect A and B with a {kind} joint"),
        ] {
            let found = lib.find(&text, &Bm25).unwrap();
            assert_eq!(
                found["recipes"][0]["id"], "revolute_joint",
                "{text}: {found}"
            );
            assert!(
                found["recipes"][0]["score"].as_f64().unwrap() >= 4.5,
                "{text}: {found}"
            );
        }
    }
    let schema = schema("revolute_joint").unwrap();
    for args in [
        json!({"joint_type":"ball","limit1_min":20,"limit1_max":-20}),
        json!({"joint_type":"slider","limit2_min":0,"limit2_max":1}),
        json!({"joint_type":"rigid","limit1_min":0,"limit1_max":1}),
        json!({"joint_type":"ball","limit1_max":20}),
        json!({"joint_type":"slider","limit1_min":-1000001,"limit1_max":0}),
    ] {
        assert!(validate(&args, &schema).is_err(), "{args}");
    }
    for (kind, count) in [
        ("rigid", 0),
        ("revolute", 1),
        ("slider", 1),
        ("cylindrical", 2),
        ("pin_slot", 2),
        ("planar", 3),
        ("ball", 3),
        ("parallel", 4),
        ("perpendicular", 5),
        ("angle", 5),
        ("point_on_plane", 5),
        ("point_on_line", 4),
        ("line_on_plane", 4),
    ] {
        let mut args = json!({"joint_type":kind});
        for i in 1..=count {
            args[format!("limit{i}_min")] = json!(-10);
            args[format!("limit{i}_max")] = json!(10);
        }
        validate(&args, &schema).unwrap();
    }
}

#[test]
fn joint_form_preserves_limit_units_and_audits_axis_default() {
    let lib = Library::embedded().unwrap();
    let recipe = lib
        .recipes
        .iter()
        .find(|r| r.id == "revolute_joint")
        .unwrap();
    let flat = forms::flat(&recipe.schema);
    let compact = forms::compact(&recipe.schema);
    assert!(compact["properties"]["joint_type"]["description"]
        .as_str()
        .unwrap()
        .contains("joint.list order"));
    let mut args = json!({"joint_type":"spherical","limit1_min":"-15","limit1_max":"15"});
    let changes = forms::coerce(&mut args, &flat).unwrap();
    assert_eq!(args["joint_type"], "ball");
    assert_eq!(args["axis"], "Z");
    assert_eq!(args["limit1_min"].as_f64(), Some(-15.));
    assert!(changes
        .iter()
        .any(|c| c["field"] == "axis" && c["defaulted"] == true));
    for i in 1..=5 {
        assert!(flat["properties"][format!("limit{i}_min")]
            .get("x-unit")
            .is_none());
    }
    let mut legacy = json!({"pin_component":"Base","hole_component":"Arm","angle_deg":45});
    forms::coerce(&mut legacy, &flat).unwrap();
    assert!(legacy.get("axis").is_none());
}
