//! Independently authored development requests; analytic geometry is the oracle.
use super::*;

struct Case {
    prompt: &'static str,
    // Sections are [diameter, axial length], ordered from bottom to top.
    outer: Vec<[f64; 2]>,
    bore: Vec<[f64; 2]>,
    replay: bool,
}
fn tube(prompt: &'static str, od: f64, id: f64, h: f64, replay: bool) -> Case {
    Case {
        prompt,
        outer: vec![[od, h]],
        bore: vec![[id, h]],
        replay,
    }
}
fn pin(prompt: &'static str, sections: &[[f64; 2]], replay: bool) -> Case {
    Case {
        prompt,
        outer: sections.to_vec(),
        bore: vec![],
        replay,
    }
}
fn shoulder(prompt: &'static str, od: f64, bore: &[[f64; 2]], replay: bool) -> Case {
    Case {
        prompt,
        outer: vec![[od, bore.iter().map(|s| s[1]).sum()]],
        bore: bore.to_vec(),
        replay,
    }
}
fn cases() -> Vec<Case> {
    vec![
        tube("Make a ring, 38 mm OD, 24 mm ID, 6 mm thick",38.,24.,6.,true),
        tube("Create a sleeve with OD 32 mm and bore 18 mm and length 47 mm",32.,18.,47.,true),
        tube("Build a thrust collar 52 mm outside diameter, 26 mm bore, 9 mm long",52.,26.,9.,true),
        tube("A bushing with 2 inches OD and 1 1/4 inches ID and 3/4 inches length",50.8,31.75,19.05,true),
        tube("washer 1 1/2\" OD, 3/4\" ID, 1/8\" thick",38.1,19.05,3.175,true),
        tube("spacer 3.6 cm OD, 0.4 cm wall thickness, 1.8 cm long",36.,28.,18.,true),
        pin("stepped pin, 20 mm dia for 32 mm then 12 mm dia for 19 mm", &[[20.,32.],[12.,19.]],true),
        pin("Make a shouldered shaft diameter 14 mm for 16 mm then diameter 24 mm for 8 mm", &[[14.,16.],[24.,8.]],true),
        pin("stepped rod 1\" diameter for 1 1/2\" then 1/2\" diameter for 3/4\" then 3/8\" diameter for 1/4\"", &[[25.4,38.1],[12.7,19.05],[9.525,6.35]],true),
        shoulder("shouldered sleeve 40 mm OD, 30 mm long, 12 mm bore through, counterbore 22 mm diameter 7 mm deep from top",40.,&[[12.,23.],[22.,7.]],true),
        shoulder("stepped bore bushing 34 mm OD, 24 mm length, bore 18 mm dia for 9 mm then 10 mm dia for 15 mm from bottom",34.,&[[18.,9.],[10.,15.]],true),
        shoulder("shouldered bore 2\" OD, bore 1\" dia for 1/4\" then 1/2\" dia for 3/4\" from top",50.8,&[[12.7,19.05],[25.4,6.35]],true),
        tube("ring OD 48 mm ID 30 mm length 4 mm",48.,30.,4.,false),
        tube("a ring 4 cm outer diameter 2 cm inner diameter 0.3 cm thickness",40.,20.,3.,false),
        tube("Build a ring, bore diameter 15 mm, length of 5 mm, OD of 29 mm",29.,15.,5.,false),
        tube("sleeve 3/4 in OD 1/2 in ID 2 in long",19.05,12.7,50.8,false),
        tube("sleeve OD 1 3/4 in, ID 1 1/4 in, height 2 1/2 in",44.45,31.75,63.5,false),
        tube("Create a sleeve length 60 mm with wall 3 mm and OD 28 mm",28.,22.,60.,false),
        tube("collar OD 42 mm bore 20 mm length 14 mm",42.,20.,14.,false),
        tube("thrust collar 4.6 cm OD 2.8 cm bore 0.7 cm long",46.,28.,7.,false),
        tube("a collar OD 1 inch ID 1/2 inch thickness 1/4 inch",25.4,12.7,6.35,false),
        tube("bushing 25 mm OD 17 mm ID 39 mm high",25.,17.,39.,false),
        tube("bushing OD 30 mm wall 2 mm length 22 mm",30.,26.,22.,false),
        tube("washer OD 24 mm ID 9 mm thickness 2 mm",24.,9.,2.,false),
        tube("washer 7/8 inches OD 3/8 inches ID 1/16 inches thick",22.225,9.525,1.5875,false),
        tube("spacer 18 mm OD 7 mm bore 16 mm tall",18.,7.,16.,false),
        tube("Please make a spacer, OD 2 cm, ID 1 cm, length 3 cm",20.,10.,30.,false),
        pin("stepped pin diameter 22 mm for 11 mm then diameter 16 mm for 26 mm",&[[22.,11.],[16.,26.]],false),
        pin("stepped pin 8 mm diameter for 9 mm then 14 mm diameter for 5 mm then 6 mm diameter for 12 mm",&[[8.,9.],[14.,5.],[6.,12.]],false),
        pin("build a stepped shaft 2 cm dia for 4 cm then 1 cm dia for 2 cm",&[[20.,40.],[10.,20.]],false),
        pin("a shouldered pin 3/4 inch dia for 1 inch then 1/4 inch dia for 1/2 inch",&[[19.05,25.4],[6.35,12.7]],false),
        pin("create a stepped rod dia 26 mm for 8 mm long and then dia 10 mm for 48 mm long",&[[26.,8.],[10.,48.]],false),
        pin("stepped shaft 1 1/4\" dia for 2\" then 3/4\" dia for 1 1/2\"",&[[31.75,50.8],[19.05,38.1]],false),
        pin("shouldered shaft diameter of 18 mm for 7 mm then diameter of 9 mm for 31 mm",&[[18.,7.],[9.,31.]],false),
        shoulder("shouldered collar OD 48 mm length 16 mm bore 16 mm through counterbore diameter 28 mm 5 mm deep from bottom",48.,&[[28.,5.],[16.,11.]],false),
        shoulder("stepped sleeve OD 32 mm bore 20 mm dia for 6 mm then 12 mm dia for 18 mm from top",32.,&[[12.,18.],[20.,6.]],false),
        shoulder("shouldered bore 28 mm OD 20 mm long bore 16 mm dia for 4 mm then 10 mm dia for 16 mm from bottom",28.,&[[16.,4.],[10.,16.]],false),
        shoulder("stepped bushing OD 3 cm length 2 cm bore 1.8 cm dia for 0.5 cm then 1 cm dia for 1.5 cm from top",30.,&[[10.,15.],[18.,5.]],false),
        shoulder("shouldered sleeve 1 1/2 in OD 1 in long 1/2 in bore through counterbore 3/4 in diameter 1/4 in deep from top",38.1,&[[12.7,19.05],[19.05,6.35]],false),
        shoulder("stepped bore OD 46 mm bore 24 mm dia for 5 mm then 16 mm dia for 10 mm then 10 mm dia for 7 mm from bottom",46.,&[[24.,5.],[16.,10.],[10.,7.]],false),
        shoulder("shouldered spacer OD 26 mm length 15 mm bore 8 mm counterbore 14 mm dia 3 mm deep from the bottom",26.,&[[14.,3.],[8.,12.]],false),
        tube("Can you create a ring with OD 33 mm and ID 19 mm and length 4 mm?",33.,19.,4.,false),
    ]
}
fn request(prompt: &str) -> Value {
    json!({"messages":[{"role":"user","content":prompt}],"tools":[{"function":{"name":"execute"}}]})
}
fn geometry(case: &Case) -> (f64, f64, f64) {
    let volume = |ss: &[[f64; 2]]| {
        ss.iter()
            .map(|s| std::f64::consts::PI * s[0].powi(2) / 4. * s[1])
            .sum::<f64>()
    };
    (
        volume(&case.outer) - volume(&case.bore),
        case.outer.iter().map(|s| s[0] / 2.).fold(0., f64::max),
        case.outer.iter().map(|s| s[1]).sum(),
    )
}
#[test]
fn turned_development_exact_cases() {
    let cat = Catalog::bundled().unwrap();
    assert!(cases().len() >= 40);
    for case in cases() {
        let plan = planner::plan(&request(case.prompt), &cat)
            .unwrap()
            .unwrap_or_else(|| panic!("declined: {}", case.prompt));
        assert_eq!(
            plan.rule,
            if case.outer.len() == 1 && case.bore.len() == 1 {
                "tube"
            } else {
                "turned"
            },
            "{}",
            case.prompt
        );
        let v = plan.expected.as_ref().unwrap().created_volume_mm3.unwrap();
        assert!(
            (v - geometry(&case).0).abs() < 1e-6,
            "{}: {plan:?}",
            case.prompt
        );
        assert!(plan.calls.len() <= 18);
        if plan.rule == "turned" {
            let cylinders: Vec<_> = plan
                .calls
                .iter()
                .filter(|c| c["arguments"]["command"] == "solid.cylinder")
                .collect();
            let mut n = 0;
            for sections in [&case.outer, &case.bore] {
                let mut z = 0.;
                for section in sections {
                    let params = &cylinders[n]["arguments"]["params"];
                    assert!(
                        (params["radius"].as_f64().unwrap() - section[0] / 2.).abs() < 1e-7,
                        "{}",
                        case.prompt
                    );
                    assert!(
                        (params["height"].as_f64().unwrap() - section[1]).abs() < 1e-7,
                        "{}",
                        case.prompt
                    );
                    assert!(
                        (params["base"][2].as_f64().unwrap() - z).abs() < 1e-7,
                        "{}",
                        case.prompt
                    );
                    if n > 0 {
                        assert_eq!(params["targets"], json!(["PilotRecipe1"]));
                    }
                    z += section[1];
                    n += 1;
                }
            }
            assert_eq!(n, cylinders.len());
        }
        // The acted guard must never repeat a completed creation prefix.
        let mut req = request(case.prompt);
        req["messages"].as_array_mut().unwrap().push(json!({"role":"assistant","tool_calls":[{"function":{"name":"execute","arguments":plan.calls[0]["arguments"].to_string()}}]}));
        assert!(planner::plan(&req, &cat).unwrap().is_none());
    }
}
#[test]
fn turned_near_misses_decline() {
    let cat = Catalog::bundled().unwrap();
    for prompt in [
        "ring 30 mm OD 10 mm ID", // missing length
        "ring 30 mm OD 10 mm ID 4 mm long with a notch",
        "Do not make a sleeve 30 mm OD 10 mm bore 8 mm long",
        "collar 30 mm OD 30 mm bore 8 mm long",
        "sleeve 30 mm OD 10 mm ID 2 mm wall 8 mm long",
        "ring 30 mm OD 10 mm ID 8 mm long 9 mm long",
        "stepped pin 20 mm dia for 30 mm then 10 mm dia", // missing second length
        "stepped pin 20 mm dia for 30 mm or 10 mm dia for 10 mm",
        "stepped pin 20 mm dia for -30 mm then 10 mm dia for 10 mm",
        "stepped pin 20 mm dia for 30 mm then 10 mm dia for 10 mm, tapered",
        "shouldered sleeve OD 30 mm length 20 mm bore 16 mm dia for 5 mm then 10 mm dia for 10 mm from top", // inconsistent total
        "shouldered sleeve OD 30 mm bore 16 mm dia for 5 mm then 10 mm dia for 10 mm", // unspecified end
        "shouldered sleeve OD 30 mm length 20 mm bore 8 mm counterbore 16 mm dia 5 mm deep", // unspecified end
        "ring 30 mm OD 10 mm ID 8 mm long on XZ",
        "washer 3/0 in OD 1/4 in ID 1/8 in thick",
        "washer 1 3/2 in OD 1/4 in ID 1/8 in thick",
        "sleeve 30 OD 10 ID 8 long", // units required
        "sleeve OD 30 mm wall 16 mm length 10 mm",
        "shouldered bore OD 30 mm bore 30 mm dia for 5 mm then 10 mm dia for 10 mm from top",
        "shouldered sleeve OD 30 mm length 10 mm bore 8 mm counterbore 16 mm dia 10 mm deep from top",
        "ring 30 mm OD 10 mm ID 8 mm long but no bore",
        "make a ring 30 mm OD 10 mm ID 8 mm long and a pin",
        "stepped pin 12 mm dia for 10 mm then 12 mm dia for 8 mm",
        "stepped pin 20 mm dia for 10 mm then 12 mm dia for 8 mm and",
    ] {
        assert!(planner::plan(&request(prompt), &cat).unwrap().is_none(), "claimed {prompt}");
    }
    let many = format!("stepped pin {}", ["10 mm dia for 10 mm"; 9].join(" then "));
    assert!(planner::plan(&request(&many), &cat).unwrap().is_none());
    assert!(planner::plan(
        &request("stepped pin 10 mm dia for 900000 mm then 5 mm dia for 900000 mm"),
        &cat
    )
    .unwrap()
    .is_none());
}
#[test]
fn turned_public_cli_replays() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = public_catalog::catalog().unwrap();
    let mut count = 0;
    for case in cases().into_iter().filter(|c| c.replay) {
        eprintln!("turned replay: {}", case.prompt);
        let p = planner::plan(&request(case.prompt), &cat).unwrap().unwrap();
        let mut s = Session::new(&path);
        for call in &p.calls {
            s.execute(
                call["arguments"]["command"].as_str().unwrap(),
                call["arguments"]["params"].clone(),
            );
        }
        let (v, r, h) = geometry(&case);
        check_shortcut_body(&mut s, v, [-r, -r, 0.], [r, r, h], false);
        count += 1;
    }
    assert_eq!(count, 12);
}
#[test]
fn turned_existing_forms_and_matches_unchanged() {
    use dutchman_pilot::recipe_library::{forms, Library};
    let cat = Catalog::bundled().unwrap();
    let lib = Library::embedded().unwrap();
    for (prompt, id, args) in [
        (
            "tube 30 mm OD 18 mm ID 24 mm long",
            "tube",
            json!({"outer_radius":15,"inner_radius":9,"height":24}),
        ),
        (
            "tube 30 mm OD 6 mm wall 24 mm long",
            "tube",
            json!({"outer_radius":15,"inner_radius":9,"height":24}),
        ),
        (
            "cylinder 10 mm radius 20 mm high",
            "cylinder",
            json!({"radius":10,"height":20}),
        ),
        (
            "revolve a rectangle radius 10 mm height 20 mm from axis on XZ about Z full turn",
            "revolve",
            json!({"radius":10,"height":20}),
        ),
        (
            "sleeve 30 mm OD 18 mm bore 24 mm long",
            "tube",
            json!({"outer_radius":15,"inner_radius":9,"height":24}),
        ),
    ] {
        let req = request(prompt);
        let exact = planner::plan(&req, &cat).unwrap().unwrap();
        let form = lib
            .build(&json!({"recipe":id,"args":args}), &req, &cat)
            .unwrap();
        assert_eq!(exact.calls, form.calls, "{prompt}");
        assert_eq!(exact.slots, form.slots, "{prompt}");
    }
    let schema = &lib.recipes.iter().find(|r| r.id == "tube").unwrap().schema;
    for prompt in [
        "Maybe a collar, roughly 30 mm OD, or 34 mm?",
        "A sleeve with an unclear bore and shoulder",
    ] {
        let req = request(prompt);
        let mut args = json!({"outer_radius":15,"inner_radius":9,"height":24});
        let supplied = args.clone();
        assert!(forms::request_defaults(&mut args, schema, &req)
            .unwrap()
            .is_empty());
        assert_eq!(args, supplied);
        assert!(lib
            .build(&json!({"recipe":"tube","args":args}), &req, &cat)
            .is_ok());
    }
}

#[test]
fn turned_public_cli_keeps_existing_overlapping_body() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = public_catalog::catalog().unwrap();
    for case in cases().into_iter().filter(|c| {
        c.prompt.starts_with("stepped pin,") || c.prompt.starts_with("shouldered sleeve 40")
    }) {
        let mut s = Session::new(&path);
        s.execute(
            "solid.cylinder",
            json!({"radius":50,"height":100,"operation":"new","body_name":"PilotRecipe1"}),
        );
        let before = s.execute("document.inspect", json!({"measure":true}));
        let mut req = request(case.prompt);
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool","content":before.to_string()}));
        let p = planner::plan(&req, &cat).unwrap().unwrap();
        assert_eq!(p.expected.as_ref().unwrap().bodies, Some(2));
        assert_eq!(
            p.expected.as_ref().unwrap().name.as_deref(),
            Some("PilotRecipe2")
        );
        for call in &p.calls {
            s.execute(
                call["arguments"]["command"].as_str().unwrap(),
                call["arguments"]["params"].clone(),
            );
        }
        let after = s.execute("document.inspect", json!({"measure":true}));
        let bodies = after["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), 2, "{after}");
        let old = bodies.iter().find(|b| b["name"] == "PilotRecipe1").unwrap();
        let new = bodies.iter().find(|b| b["name"] == "PilotRecipe2").unwrap();
        assert_eq!(old["volume_mm3"], before["bodies"][0]["volume_mm3"]);
        assert_eq!(old["bbox"], before["bodies"][0]["bbox"]);
        let (v, _, _) = geometry(&case);
        assert!((new["volume_mm3"].as_f64().unwrap() - v).abs() < v * 0.001);
    }
}
