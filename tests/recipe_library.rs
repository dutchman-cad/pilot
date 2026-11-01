#![cfg(feature = "core")]
#![allow(clippy::unwrap_used, clippy::panic)]
use dutchman_pilot::{catalog::Catalog, recipe_library::*};
use serde_json::{json, Value};
#[path = "support/dev_stem_offset_core.rs"]
mod dev_stem_offset_core;

#[test]
fn pattern_copy_counts_are_totals_and_inspection_is_idempotent() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for recipe in ["linear_pattern", "circular_pattern"] {
        for (text, input, total) in [
            ("12 copies", 12, 13),
            ("12 copies", 13, 13),
            ("12 more", 12, 13),
            ("12 additional copies", 12, 13),
            ("a dozen more", 12, 13),
            ("twelve copies", 12, 13),
            ("12 in total", 12, 12),
            ("a row of 12", 12, 12),
            ("12 copies including the original", 12, 12),
            ("12 copies excluding the original", 12, 13),
            ("12 copies not including the original", 12, 13),
            ("12 copies altogether", 12, 12),
            ("12 slots 1 in apart", 12, 12),
            ("1 more", 1, 2),
            ("499 copies", 499, 500),
        ] {
            let request = json!({"messages":[{"role":"user","content":text}]});
            let mut args = if recipe == "linear_pattern" {
                json!({"target":"body","count":input,"spacing":25.4,"direction":"X"})
            } else {
                json!({"target":"body","count":input,"axis":"Z","angle_deg":360})
            };
            for _ in 0..2 {
                let plan = lib
                    .build(&json!({"recipe":recipe,"args":args}), &request, &cat)
                    .unwrap();
                assert_eq!(plan.slots["count"], total, "{text}");
                args = plan.slots;
            }
        }
        let args = if recipe == "linear_pattern" {
            json!({"target":"body","count":500,"spacing":1,"direction":"X"})
        } else {
            json!({"target":"body","count":500,"axis":"Z","angle_deg":360})
        };
        assert!(lib
            .build(
                &json!({"recipe":recipe,"args":args}),
                &json!({"messages":[{"role":"user","content":"500 more"}]}),
                &cat
            )
            .is_err());
    }
    let request = json!({"messages":[{"role":"user","content":"12 copies around Z"}]});
    let plan = lib.build(&json!({"recipe":"circular_pattern","args":{"target":"body","count":12,"axis":"Z","angle_deg":30}}), &request, &cat).unwrap();
    assert_eq!(plan.slots["count"], 13);
    assert_eq!(plan.slots["angle_deg"], 360.);
}

#[test]
fn bolt_circle_radius_and_diameter_must_agree_after_scaling() {
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let args = json!({"bolt_hole_count":4,"bolt_circle_radius":50.8,"bolt_circle_diameter":101.6,"boss_diameter":12.7,"boss_height":50.8});
    assert_eq!(
        forms::to_slots(&args, schema).unwrap()["bolt_circle"]["diameter"],
        101.6
    );
    let mut bad = args;
    bad["bolt_circle_diameter"] = json!(50.8);
    assert!(forms::to_slots(&bad, schema).is_err());
    let mut inches = json!({"bolt_hole_count":4,"bolt_circle_radius":"2 in","boss_diameter":"1/2 in","boss_height":"2 in"});
    forms::coerce(&mut inches, &forms::flat(schema)).unwrap();
    let slots = forms::to_slots(&inches, schema).unwrap();
    assert_eq!(slots["bolt_circle"]["diameter"], 101.6);
    assert_eq!(slots["boss_diameter"], 12.7);
    assert_eq!(slots["boss_height"], 50.8);
}

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
        "joint" => json!({"joint_type":"revolute","axis":"Z"}),
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
        if r.id != "revolute_joint" && r.id != "joint" {
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
                if r.id != "revolute_joint" && r.id != "joint" {
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
    assert_eq!(Library::embedded().unwrap().recipes.len(), 20);
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
                if r.id != "revolute_joint" && r.id != "joint" {
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
        // Plate includes both base outlines and three feature layouts. Corner sides,
        // row offsets and circle radius are offered only when a request names them,
        // so this full form is larger than the one most requests see.
        let cap = match r.id.as_str() {
            "plate" => 2600,
            "joint" => 3000,
            _ => 1200,
        };
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
        if r.id == "joint" {
            // Component inspection and command equivalence are covered by the CLI tests.
            continue;
        }
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
        ("enclosure", json!({"lip_width":1}), "wall"),
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
            // A specific web value wins over the general thickness, which fills the flange.
            redundant["web_thickness"] = json!(7);
            let mut specific = base.clone();
            specific.as_object_mut().unwrap().remove("thickness");
            specific["web_thickness"] = json!(7);
            specific["flange_thickness"] = json!(8);
            assert_eq!(
                build("profile", redundant).unwrap().calls,
                build("profile", specific).unwrap().calls
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
fn audited_first_fills_replay_with_documented_defaults() {
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
                let error = result.unwrap_err();
                assert!(
                    error.contains("open") && error.contains("top") && error.contains("bottom"),
                    "{error}"
                );
                assert!(!error.contains("lip_height missing"));
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
    let mut request = json!({"messages":[{"role":"user","content":"a post at X=30, 6 in total around Z"},{"role":"tool","content":json!({"bodies":[{"name":"Post","volume_mm3":1608.495,"bbox":{"min":[26,-4,0],"max":[34,4,8]}}],"timeline":[]}).to_string()}]});
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
        request["messages"][0]["content"] = json!("a post at X=30, 6 in total around Z");
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
fn profile_fills_accept_kind_and_plane_wording_and_specific_thickness() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "profile")
        .unwrap()
        .schema;
    let flat = forms::flat(schema);
    for (kind, expected) in [
        ("equal angle", "L"),
        ("Angle", "L"),
        ("channel", "C"),
        ("T-bar", "T"),
        ("C channel", "C"),
        ("rectangular", "rectangle"),
        ("T-section", "T"),
        ("U_CHANNEL", "U"),
        ("inverted T", "T"),
        ("flat bar", "rectangle"),
    ] {
        let mut args = json!({"kind":kind});
        forms::coerce(&mut args, &flat).unwrap();
        assert_eq!(args["kind"], expected, "{kind}");
    }
    for plane in [
        json!(["X", "Z"]),
        json!(["XZ"]),
        json!({"plane":"XZ"}),
        json!("XZ plane"),
    ] {
        let mut args = json!({"plane":plane.clone()});
        let changes = forms::coerce(&mut args, &flat).unwrap();
        assert_eq!(args["plane"], "XZ", "{plane}");
        assert_eq!(changes.len(), 1);
    }
    let args =
        json!({"kind":"L","width":25,"height":45,"depth":20,"thickness":5,"flange_thickness":8});
    let slots = forms::to_slots(&args, schema).unwrap();
    assert_eq!(slots.get("thickness"), None);
    assert_eq!(slots["web_thickness"], 5);
    assert_eq!(slots["flange_thickness"], 8);
    let agreeing =
        json!({"kind":"U","width":50,"height":30,"depth":150,"thickness":4,"web_thickness":4});
    let slots = forms::to_slots(&agreeing, schema).unwrap();
    assert_eq!(slots["thickness"], 4);
    assert_eq!(slots.get("web_thickness"), None);
}

#[test]
fn compound_plate_creation_and_existing_pattern_retrieval() {
    let lib = Library::embedded().unwrap();
    for prompt in [
        "60x20x5 base with a row of 4 pegs on top, pegs 6mm dia and 10mm tall, 15mm apart, centred on the base",
        "round plate 70mm dia, 5mm thick, centred at the origin, with 8 pegs (5mm dia, 8mm tall) evenly around a 50mm circle on top",
        "90x90 base plate, 6mm thick, with a 10mm round boss 6mm tall on top, 15mm in from both edges at one corner",
        "pegboard panel 120 x 80 x 3 with a 7 x 5 grid of 6mm holes, 15mm pitch, grid centred on the panel",
        "50x50 base 4mm thick with a 3 by 3 grid of bosses on top, 8mm diameter 5mm tall, 16mm spacing, centred",
        "Make a 60x30x4 base with a pattern of five pegs on top",
        "Add a round plate 70mm across and 5mm thick with eight pegs on a 50mm circle",
    ] {
        let found = lib.find(prompt, &Bm25).unwrap();
        assert_eq!(found["recipes"][0]["id"], "plate", "{prompt}: {found}");
    }
    for (prompt, expected) in [
        ("make that hole into a row of 5 along X, 12mm apart", "linear_pattern"),
        ("pattern the hole on the base plate in a row of 5 along X, 12mm apart", "linear_pattern"),
        ("now copy that hole along the strip so there are 9 holes, 10mm apart", "linear_pattern"),
        ("pattern the small bolt hole around the centre so there are 6 of them evenly spaced", "circular_pattern"),
        ("copy that hole around the middle so I get 4 holes spread over a 90 degree arc, the first one stays where it is", "circular_pattern"),
        ("turn that hole into a grid, 4 across X and 3 along Y, 15mm spacing both ways", "grid_pattern"),
        ("now make that boss a 4x4 grid with 20mm spacing", "grid_pattern"),
    ] {
        let found = lib.find(prompt, &Bm25).unwrap();
        assert_eq!(found["recipes"][0]["id"], expected, "{prompt}: {found}");
    }
}

#[test]
fn plate_grid_has_exact_count_pitch_and_explicit_axes() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (length, width, columns, rows) in [(120, 80, 7, 5), (80, 120, 5, 7)] {
        let p = lib.build(&json!({"recipe":"plate","args":{"length":length,"width":width,"thickness":3,"grid_columns":columns,"grid_rows":rows,"grid_pitch_x":15,"grid_pitch_y":15,"grid_hole_diameter":6}}), &json!({"messages":[]}), &cat).unwrap();
        let holes: Vec<_> = p
            .calls
            .iter()
            .filter(|c| c["arguments"]["command"] == "solid.hole")
            .collect();
        assert_eq!(holes.len(), 35);
        for (i, call) in holes.iter().enumerate() {
            let params = &call["arguments"]["params"];
            assert_eq!(params["diameter"], 6.);
            assert_eq!(
                params["position"],
                json!([
                    length as f64 / 2. + (i % columns as usize) as f64 * 15.
                        - (columns - 1) as f64 * 7.5,
                    width as f64 / 2. + (i / columns as usize) as f64 * 15.
                        - (rows - 1) as f64 * 7.5,
                    3.
                ])
            );
            assert_eq!(params["targets"], json!(["PilotRecipe1"]));
        }
    }
}

#[test]
fn plate_rejects_unbounded_or_ambiguous_features() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let base = json!({"length":60,"width":40,"thickness":5});
    for extra in [
        json!({"diameter":60}),
        json!({"boss_diameter":6}),
        json!({"boss_height":10}),
        json!({"boss_offset_x":3}),
        json!({"boss_diameter":6,"boss_height":10,"boss_offset_x":29}),
        json!({"boss_diameter":6,"boss_height":10,"row_count":4,"row_pitch":5}),
        json!({"boss_diameter":6,"boss_height":10,"row_count":65,"row_pitch":15}),
        json!({"boss_diameter":6,"boss_height":10,"grid_columns":9,"grid_rows":9,"grid_pitch_x":10,"grid_pitch_y":10}),
        json!({"boss_diameter":6,"boss_height":10,"row_count":3,"row_pitch":10,"boss_offset_y":2}),
        json!({"hole_diameter":6,"row_count":3,"row_pitch":10,"grid_columns":2,"grid_rows":2,"grid_pitch_x":10,"grid_pitch_y":10}),
        json!({"boss_diameter":6,"boss_height":10,"bore_diameter":6}),
        json!({"boss_diameter":6,"boss_height":10,"hole_diameter":4,"row_count":3,"row_pitch":10}),
    ] {
        let mut args = base.clone();
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(
            lib.build(
                &json!({"recipe":"plate","args":args}),
                &json!({"messages":[]}),
                &cat
            )
            .is_err(),
            "{args}"
        );
    }
    assert!(lib.build(&json!({"recipe":"plate","args":{"diameter":40,"thickness":3,"hole_diameter":4,"grid":{"rows":2,"columns":2,"pitch_x":28,"pitch_y":28}}}), &json!({"messages":[]}), &cat).is_err());
}

#[test]
fn existing_edge_treatments_do_not_become_plate_creations() {
    let lib = Library::embedded().unwrap();
    for (query, recipe) in [
        ("Apply a fillet where a peg joins its base plate", "fillet"),
        (
            "Please add a chamfer to the boss at the plate surface",
            "chamfer",
        ),
    ] {
        let found = lib.form_band(query).unwrap();
        assert_eq!(found["recipes"][0]["id"], recipe, "{query}: {found}");
    }
}
#[test]
fn every_joint_type_retrieves_one_form_with_ordered_limits() {
    let lib = Library::embedded().unwrap();
    let tasks: Value =
        serde_json::from_str(include_str!("../bench/sets/joints/tasks.json")).unwrap();
    for task in tasks["tasks"].as_array().unwrap() {
        let found = lib.find(task["prompt"].as_str().unwrap(), &Bm25).unwrap();
        assert_eq!(found["recipes"][0]["id"], "joint");
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
            format!("{kind} joint with motion limits"),
            format!("Connect A and B with a {kind} joint at box centres"),
        ] {
            let found = lib.find(&text, &Bm25).unwrap();
            assert_eq!(found["recipes"][0]["id"], "joint", "{text}: {found}");
            assert!(
                found["recipes"][0]["score"].as_f64().unwrap() >= 4.5,
                "{text}: {found}"
            );
        }
    }
    let schema = schema("joint").unwrap();
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
    let recipe = lib.recipes.iter().find(|r| r.id == "joint").unwrap();
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
    let legacy_schema = lib
        .recipes
        .iter()
        .find(|r| r.id == "revolute_joint")
        .unwrap();
    forms::coerce(&mut legacy, &forms::flat(&legacy_schema.schema)).unwrap();
    assert!(legacy.get("axis").is_none());
}

#[test]
fn feature_wording_defaults_aliases_and_edit_context() {
    use dutchman_pilot::recipe_library::forms;
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "enclosure")
        .unwrap()
        .schema;
    for alias in ["centred", "centered", "centre", "center"] {
        let mut args = json!({"length":"3 in","width":"2 in","height":"1 in","wall":"1/8 in","floor":"1/4 in","inside":true,"open":true,"lip":true,alias:false});
        let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        assert_eq!(changes.iter().filter(|c| c["defaulted"] == true).count(), 2);
        assert_eq!(args["lip_height"], 3.175);
        assert_eq!(args["lip_width"], 1.5875);
        assert_eq!(args["open"], "top");
        let plan = lib
            .build(
                &json!({"recipe":"enclosure","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        assert!(
            (plan.calls[0]["arguments"]["params"]["length"]
                .as_f64()
                .unwrap()
                - 82.55)
                .abs()
                < 1e-9
        );
        assert_eq!(plan.calls[0]["arguments"]["params"]["height"], 31.75);
    }
    let mut conflict = json!({"centred":false,"centered":true});
    assert!(forms::coerce(&mut conflict, &forms::flat(schema))
        .unwrap_err()
        .contains("conflicts"));
    let plate = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    for (word, side) in [
        (json!(["top_right", "bottom_right"]), "right"),
        (json!("top-left bottom-left"), "left"),
        (json!("top right and bottom right"), "right"),
        (json!("right (+X)"), "right"),
        (json!("-X"), "left"),
    ] {
        let mut a =
            json!({"length":80,"width":60,"thickness":4,"corner_size":3,"corner_edges":word});
        forms::coerce(&mut a, &forms::flat(plate)).unwrap();
        assert_eq!(a["corner_sides"], side);
        let p = lib
            .build(
                &json!({"recipe":"plate","args":a}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let treatment = p
            .calls
            .iter()
            .find(|c| c["arguments"]["command"] == "solid.fillet")
            .unwrap();
        assert_eq!(
            treatment["arguments"]["params"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    for (prompt,id) in [
        ("Bevel the right edges of the existing plate", "chamfer"),
        ("Hollow out Case from underneath, leave the bottom open. 2mm walls and 2mm closed top. Keep the outside dimensions and position as they are.", "shell"),
        ("Round the upright corners on this plate", "fillet"),
    ] {
        let found=lib.form_band_in_design(prompt, &["Plate".to_owned()]).unwrap();
        assert_eq!(found["recipes"][0]["id"], id, "{found}");
        assert_eq!(found["band"], "fill");
        assert_eq!(lib.form_band_in_design(prompt, &[]).unwrap(), lib.form_band(prompt).unwrap());
    }
    for creation in [
        "Make a plate with a bolt circle and rounded corners",
        "Add a plate with a bolt circle, then round it",
    ] {
        assert_eq!(
            lib.form_band_in_design(creation, &["Plate".to_owned()])
                .unwrap(),
            lib.form_band(creation).unwrap()
        );
    }
    for id in ["fillet", "chamfer"] {
        let request = json!({"messages":[]});
        let conflict = json!({"recipe":id,"args":{"body":"Plate","amount":1,"edges":"all","corner_sides":"right"}});
        assert!(lib
            .build(&conflict, &request, &cat)
            .unwrap_err()
            .contains("requires vertical"));
        let all = json!({"recipe":id,"args":{"body":"Plate","amount":1,"edges":"all","corner_sides":"all"}});
        let legacy = json!({"recipe":id,"args":{"body":"Plate","amount":1}});
        assert_eq!(
            lib.build(&all, &request, &cat).unwrap().calls,
            lib.build(&legacy, &request, &cat).unwrap().calls
        );
    }
}

#[test]
fn request_joint_angle_never_overrides_a_fill_or_chooses_between_angles() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "revolute_joint")
        .unwrap()
        .schema;
    for text in [
        "pivot at 30 degrees or 90 degrees",
        "pivot without an angle",
        "use a quarter turn then 30 degrees",
    ] {
        let mut args = json!({});
        assert!(forms::request_defaults(
            &mut args,
            schema,
            &json!({"messages":[{"role":"user","content":text}]})
        )
        .unwrap()
        .is_empty());
        assert!(args.get("angle_deg").is_none());
    }
    for supplied in [json!(0), json!(45), json!("30°"), Value::Null] {
        let mut args = json!({"angle_deg":supplied});
        assert!(forms::request_defaults(
            &mut args,
            schema,
            &json!({"messages":[{"role":"user","content":"pivot at 90°"}]})
        )
        .unwrap()
        .is_empty());
        assert_eq!(args["angle_deg"], supplied);
    }
    assert!(forms::request_defaults(
        &mut json!(42),
        schema,
        &json!({"messages":[{"role":"user","content":"pivot at 90°"}]})
    )
    .unwrap_err()
    .contains("object"));
}

#[test]
fn flat_plural_variants_use_listed_fields_and_refuse_conflicts() {
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let flat = forms::flat(schema);
    for alias in ["hole_diameter", "holes_diameter", "grid_holes_diameter"] {
        let mut args = json!({"length":90,"width":60,"thickness":4,"grid_columns":3,"grid_rows":2,"grid_pitch_x":20,"grid_pitch_y":30,alias:"5 mm"});
        forms::coerce(&mut args, &flat).unwrap();
        let slots = forms::to_slots(&args, schema).unwrap();
        assert_eq!(slots["hole_diameter"].as_f64(), Some(5.));
    }
    let mut conflict = json!({"holes_diameter":4,"hole_diameter":5});
    assert!(forms::coerce(&mut conflict, &flat)
        .unwrap_err()
        .contains("conflicts"));
    let mut bosses = json!({"bosses_diameter":6});
    forms::coerce(&mut bosses, &flat).unwrap();
    assert_eq!(bosses["boss_diameter"], 6);
    let mut unknown = json!({"unlisted_holes_diameter":4});
    forms::coerce(&mut unknown, &flat).unwrap();
    assert!(forms::to_slots(&unknown, schema)
        .unwrap_err()
        .contains("unknown"));
}

#[test]
fn joint_limit_is_not_inferred_as_current_rotation() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "revolute_joint")
        .unwrap()
        .schema;
    let mut args = json!({});
    let changes = forms::request_defaults(
        &mut args,
        schema,
        &json!({"messages":[{"role":"user","content":"Add a hinge with a limit of 90 degrees"}]}),
    )
    .unwrap();
    assert!(changes.is_empty());
    assert!(args.get("angle_deg").is_none());
}

#[test]
fn profile_centring_cannot_be_discarded_by_the_l_builder() {
    let lib = Library::embedded().unwrap();
    let error = lib.build(&json!({"recipe":"profile","args":{"kind":"L","width":30,"height":20,"thickness":3,"depth":10,"centred":true}}), &json!({"messages":[]}), &Catalog::bundled().unwrap()).unwrap_err();
    assert!(error.contains("centred L profiles"), "{error}");
}

#[test]
fn unused_profile_offsets_and_aliases_preserve_geometry() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = forms::flat(
        &lib.recipes
            .iter()
            .find(|r| r.id == "profile")
            .unwrap()
            .schema,
    );
    for (kind, canonical) in [
        ("Channel", "C"),
        ("channel", "C"),
        ("Tee", "T"),
        ("rectangular", "rectangle"),
        ("rect", "rectangle"),
    ] {
        for (plane, canonical_plane) in [
            ("Top plane", "XY"),
            ("Front plane", "XZ"),
            ("Right plane", "YZ"),
            ("top", "XY"),
            ("R", "YZ"),
        ] {
            let mut args = json!({"kind":kind,"width":40,"height":30,"depth":20,"thickness":4,"plane":plane,"stem_offset":100});
            if canonical == "rectangle" {
                args.as_object_mut().unwrap().remove("thickness");
            }
            let changes = forms::coerce(&mut args, &schema).unwrap();
            assert_eq!(args["kind"], canonical);
            assert_eq!(args["plane"], canonical_plane);
            let build = |a| {
                lib.build(
                    &json!({"recipe":"profile","args":a}),
                    &json!({"messages":[]}),
                    &cat,
                )
            };
            if canonical == "T" {
                assert_eq!(args["stem_offset"], 100);
                assert!(build(args).unwrap_err().contains("stem_offset"));
            } else {
                assert!(args.get("stem_offset").is_none());
                assert!(changes
                    .iter()
                    .any(|c| c["field"] == "stem_offset" && c["after"].is_null()));
                let mut expected = json!({"kind":canonical,"width":40,"height":30,"depth":20,"thickness":4,"plane":canonical_plane});
                if canonical == "rectangle" {
                    expected.as_object_mut().unwrap().remove("thickness");
                }
                assert_eq!(build(args).unwrap().calls, build(expected).unwrap().calls);
            }
        }
    }
    for args in [
        json!({"kind":"T","stem_offset":0}),
        json!({"kind":"unknown","stem_offset":100}),
    ] {
        let mut value = args.clone();
        forms::coerce(&mut value, &schema).unwrap();
        assert_eq!(value["stem_offset"], args["stem_offset"]);
    }
}

#[test]
fn plate_grid_rows_require_one_unambiguous_request_count_pair() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    let base = json!({"length":100,"width":80,"thickness":8,"grid_columns":3,"grid_pitch_x":20,"grid_pitch_y":20,"grid_hole_diameter":4});
    for text in [
        "Make a plate with a 3 x 2 hole grid",
        "Plate with a 3 by 2 grid",
        "Plate with 3x2 holes",
        "Make a box plate with a 3x2 hole grid by the end",
        "Plate with a 3 × 2 grid",
    ] {
        let req = json!({"messages":[{"role":"user","content":text}]});
        let mut args = base.clone();
        let changes = forms::request_defaults(&mut args, schema, &req).unwrap();
        assert_eq!(args["grid_rows"], 2, "{text}");
        assert!(changes
            .iter()
            .any(|c| c["field"] == "grid_rows" && c["after"] == 2));
        let plan = lib
            .build(&json!({"recipe":"plate","args":base}), &req, &cat)
            .unwrap();
        assert_eq!(
            plan.calls,
            lib.build(&json!({"recipe":"plate","args":args}), &req, &cat)
                .unwrap()
                .calls
        );
    }
    for text in [
        "Plate with a 2 by 3 grid",
        "Plate with a 3 by 2 or 3 by 4 grid",
        "Plate with a 3 by 2 mm grid",
        "Plate dimensions 3 by 2",
        "Plate with a 3 by 65 grid",
        "Plate with a 3 by 0 grid",
        "Plate with a 3 by 2.5 grid",
        "Plate without a 3 by 2 grid",
        "Plate grid 3 by 2, 3 rows",
        "Plate grid 3 by 2 by 4",
        "Plate grid 3 by 2 and 5.5 by 4",
    ] {
        let req = json!({"messages":[{"role":"user","content":text}]});
        let mut args = base.clone();
        assert!(
            forms::request_defaults(&mut args, schema, &req)
                .unwrap()
                .is_empty(),
            "{text}"
        );
        assert!(
            lib.build(&json!({"recipe":"plate","args":args}), &req, &cat)
                .unwrap_err()
                .contains("grid_rows"),
            "{text}"
        );
    }
    let mut args = base;
    args["grid_rows"] = json!(4);
    assert!(forms::request_defaults(
        &mut args,
        schema,
        &json!({"messages":[{"role":"user","content":"3 by 2 grid"}]})
    )
    .unwrap()
    .is_empty());
    assert_eq!(args["grid_rows"], 4);
}

#[test]
fn plate_corner_and_plane_aliases_are_bounded_choices() {
    let lib = Library::embedded().unwrap();
    let schema = &lib.recipes.iter().find(|r| r.id == "plate").unwrap().schema;
    for (plane, expected) in [
        ("Top plane", "XY"),
        ("Front plane", "XZ"),
        ("Right plane", "YZ"),
        ("top", "XY"),
        ("R", "YZ"),
    ] {
        for kind in ["radius", "rounded"] {
            let mut args = json!({"plane":plane,"corner_kind":kind,"corner_size":2});
            let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
            assert_eq!(args["plane"], expected);
            assert_eq!(args["corner_kind"], "fillet");
            assert!(changes.iter().any(|c| c["field"] == "corner_kind"));
            forms::to_slots(&args, schema).unwrap();
        }
    }
    for (field, value) in [
        ("plane", "Bottom plane"),
        ("plane", "Front plane offset"),
        ("corner_kind", "radius 5"),
        ("corner_kind", "ellipse"),
    ] {
        let mut args = json!({field:value});
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        assert_eq!(args[field], value);
        assert!(forms::to_slots(&args, schema).is_err());
    }
}

#[test]
fn unneeded_pattern_join_is_removed_without_allowing_overlap() {
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let snapshot = json!({"bodies":[{"name":"Post","volume_mm3":128,"bbox":{"min":[28,-2,0],"max":[32,2,8]}}],"timeline":[{"id":1,"name":"Box1","type":"BoxFeature"}]});
    let detail = json!({"id":1,"name":"Box1","type":"BoxFeature","definition":{"body_names":["Post"],"corner":[28,-2,0],"length":"4","width":"4","height":"8"}});
    let req = json!({"messages":[{"role":"user","content":"Pattern this post around Z"},{"role":"tool","content":snapshot.to_string()},{"role":"tool","content":detail.to_string()}]});
    for target in ["body", "last_feature"] {
        let args = json!({"target":target,"count":6,"angle_deg":360,"axis":"Z","join":true});
        let build =
            |a, r: &Value| lib.build(&json!({"recipe":"circular_pattern","args":a}), r, &cat);
        let plan = build(args.clone(), &req).unwrap();
        let mut without = args.clone();
        without.as_object_mut().unwrap().remove("join");
        assert_eq!(plan.rule, "circular_pattern");
        assert_eq!(plan.calls, build(without, &req).unwrap().calls);
        let mut overlapping = args.clone();
        overlapping["angle_deg"] = json!(10);
        assert!(build(overlapping, &req)
            .unwrap_err()
            .contains("overlapping"));
        let mut explicit = req.clone();
        explicit["messages"][0]["content"] = json!("Join the patterned posts");
        assert!(build(args, &explicit).unwrap_err().contains("join"));
    }
}

#[test]
fn enclosure_opening_follows_the_named_part() {
    let lib = Library::embedded().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "enclosure")
        .unwrap()
        .schema;
    for (text, open) in [
        ("Make a cover with an outer lip", Some("bottom")),
        ("Make an enclosure cover with an outer lip", Some("bottom")),
        ("Build a box lid, 60 by 40 by 10", Some("bottom")),
        ("Make an enclosure with an outer lip for a lid", None),
        ("Make a box without a cover", None),
        ("Make an enclosure 80 by 60 by 30", None),
        ("Make an enclosure cover, open top", Some("top")),
    ] {
        let mut args = json!({"length":60,"width":40,"height":20,"wall":2});
        forms::request_defaults(
            &mut args,
            schema,
            &json!({"messages":[{"role":"user","content":text}]}),
        )
        .unwrap();
        assert_eq!(args["open"].as_str(), open, "{text}");
    }
}
