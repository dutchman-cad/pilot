//! Clean-room public-feature comparisons; never uses CAD implementation sources.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{
    catalog::Catalog,
    recipe_library::{forms, Library},
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, process::Command};
fn step(command: &str, params: Value) -> Value {
    json!({"command":command,"params":params})
}
fn measure(cli: &std::ffi::OsStr, mut commands: Vec<Value>, id: &str) -> Value {
    assert!(commands.len() <= 32);
    commands[0]["params"]["body_name"] = json!("PilotRecipe1");
    commands.push(step("document.inspect", json!({"measure":true})));
    commands.push(step("model.faces", json!({"body":"PilotRecipe1"})));
    let file =
        std::env::temp_dir().join(format!("pilot-encnative-{}-{id}.json", std::process::id()));
    std::fs::write(&file, serde_json::to_vec(&commands).unwrap()).unwrap();
    let output = Command::new(cli)
        .arg("run")
        .arg(&file)
        .arg("--results")
        .output()
        .unwrap();
    std::fs::remove_file(file).unwrap();
    assert!(
        output.status.success(),
        "{id}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 4 * 1024 * 1024);
    let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(results.len(), commands.len(), "{id}: incomplete script");
    let state = &results[results.len() - 2];
    assert_eq!(
        state["bodies"].as_array().unwrap().len(),
        1,
        "{id}: {state}"
    );
    assert!(
        state["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["error"].is_null()),
        "{id}: {state}"
    );
    json!({"body":state["bodies"][0],"faces":results[results.len()-1]["faces"],"commands":commands})
}
// Group coplanar patches, retaining area and first moments: features may retain different seams.
fn planes(state: &Value) -> BTreeMap<[i64; 4], [f64; 4]> {
    let mut planes = BTreeMap::new();
    for face in state["faces"].as_array().unwrap() {
        let n: [f64; 3] = std::array::from_fn(|i| face["plane_normal"][i].as_f64().unwrap());
        let c: [f64; 3] = std::array::from_fn(|i| face["centroid"][i].as_f64().unwrap());
        let d: f64 = n.iter().zip(c).map(|(a, b)| a * b).sum();
        let key = [
            (n[0] * 1e5).round() as i64,
            (n[1] * 1e5).round() as i64,
            (n[2] * 1e5).round() as i64,
            (d * 1e5).round() as i64,
        ];
        let area = face["area"].as_f64().unwrap();
        let entry = planes.entry(key).or_insert([0.; 4]);
        entry[0] += area;
        for (i, coordinate) in c.iter().enumerate() {
            entry[i + 1] += area * coordinate;
        }
    }
    planes
}
#[test]
fn native_enclosure_conventions_match_hand_filled_forms() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let schema = &lib
        .recipes
        .iter()
        .find(|r| r.id == "enclosure")
        .unwrap()
        .schema;
    let mut records = Vec::new();
    for style in [
        "cover",
        "open top",
        "thick floor",
        "inner lip",
        "unstated lip",
    ] {
        let count = if matches!(style, "inner lip" | "unstated lip") {
            2
        } else {
            8
        };
        for i in 0..count {
            let x = 60. + f64::from(i) * 4.;
            let y = 40. + f64::from(i) * 2.;
            let z = 20. + f64::from(i) * 2.;
            let t = if i % 2 == 0 { 2. } else { 2.5 };
            let floor = if style == "thick floor" {
                t + 2. + f64::from(i % 3)
            } else {
                t
            };
            let w = t / 2.;
            let h = 2. + f64::from(i % 3);
            let bottom = style == "cover";
            let centred = i % 3 == 1;
            let inside = i == 6;
            let lip = matches!(style, "cover" | "inner lip" | "unstated lip");
            let placement = if centred {
                "centred at the origin"
            } else {
                "with one corner at the origin"
            };
            let sizes = format!("{} x {} x {} cm", x / 10., y / 10., z / 10.);
            let prompt = match (style, i % 3) {
                ("cover", 0)=>format!("Make an enclosure cover {sizes}, walls {} cm, outer lip {} cm wide and {} cm high, {placement}.",t/10.,w/10.,h/10.),
                ("cover", 1)=>format!("Build a cover for a rectangular enclosure, {sizes}, wall {} cm, an outside locating lip {} cm wide, {} cm tall, {placement}.",t/10.,w/10.,h/10.),
                ("cover", _)=>format!("A lid {sizes} with {} cm walls and an outer lip of width {} cm and height {} cm, {placement}.",t/10.,w/10.,h/10.),
                ("open top", 0)=>format!("Enclosure {sizes}, wall {} cm, open top, {placement}.",t/10.),
                ("open top", 1)=>format!("Open-top box {sizes}; {} cm walls; {placement}.",t/10.),
                ("open top", _)=>format!("Enclosure {sizes}, top open, {} cm wall, {placement}.",t/10.),
                ("thick floor", 0)=>format!("Make an open top enclosure {sizes}, walls {} cm and a thick floor {} cm, {placement}.",t/10.,floor/10.),
                ("thick floor", 1)=>format!("Build a rectangular housing {sizes}, open top, wall thickness {} cm, base thickness {} cm, {placement}.",t/10.,floor/10.),
                ("thick floor", _)=>format!("An enclosure {sizes} with {} cm walls, {} cm thick floor, top open, {placement}.",t/10.,floor/10.),
                _=>format!("Make an open top enclosure {sizes}, wall {} cm, {} lip {} cm wide and {} cm high, {placement}.",t/10.,if style=="inner lip" {"inner"} else {"locating"},w/10.,h/10.)
            } + if inside {" Sizes are clear inside dimensions."} else {""};
            let mut args = json!({"length":format!("{} cm",x/10.),"width":format!("{} cm",y/10.),"height":format!("{} cm",z/10.),"wall":format!("{} cm",t/10.),"centred":centred});
            if inside {
                args["inside"] = json!(true);
            }
            if style == "thick floor" {
                args["floor"] = json!(format!("{} cm", floor / 10.));
            }
            if style == "open top" {
                args["open"] = json!(true);
            }
            if bottom {
                // A careful hand-fill supplies the cover's bottom opening. Default
                // inference is measured separately so extraction cannot masquerade
                // as a native geometry convention difference.
                args["open"] = json!("bottom");
            }
            if lip {
                args["lip_width"] = json!(format!("{} cm", w / 10.));
                args["lip_height"] = json!(format!("{} cm", h / 10.));
                if style != "unstated lip" {
                    args["lip_side"] = json!(if bottom { "outer" } else { "inner" });
                }
            }
            let supplied = args.clone();
            let request = json!({"messages":[{"role":"user","content":prompt}],"tools":[{"function":{"name":"execute"}}]});
            let mut changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
            changes.extend(forms::request_defaults(&mut args, schema, &request).unwrap());
            let plan = lib
                .build(&json!({"recipe":"enclosure","args":args}), &request, &cat)
                .unwrap();
            let pilot: Vec<_> = plan
                .calls
                .iter()
                .map(|c| c["arguments"].clone())
                .filter(|c| c["command"] != "document.inspect")
                .collect();
            let (a, b, c) = if inside {
                (x + 2. * t, y + 2. * t, z + floor)
            } else {
                (x, y, z)
            };
            let [ox, oy, oz] = if centred {
                [-a / 2., -b / 2., -c / 2.]
            } else {
                [0.; 3]
            };
            let rim = oz + if bottom { 0. } else { c };
            let mut native = vec![
                step(
                    "solid.box",
                    json!({"length":a,"width":b,"height":c,"corner":[ox,oy,oz],"operation":"new"}),
                ),
                step(
                    "solid.shell",
                    json!({"faces":[[ox+a/2.,oy+b/2.,rim]],"thickness":t}),
                ),
            ];
            // Grow the inner floor face into the cavity; independent of Pilot's box join.
            if floor != t {
                native.push(step(
                    "solid.offset_face",
                    json!({"faces":[[ox+a/2.,oy+b/2.,oz+t]],"distance":floor-t}),
                ));
            }
            if lip {
                let mut params = json!({"face":[ox+t/2.,oy+b/2.,rim],"width":w,"height":h});
                if style != "unstated lip" {
                    params["side"] = json!(if bottom { "outside" } else { "inside" });
                }
                native.push(step("plastic.lip", params));
            }
            let id = format!("{}-{i}", style.replace(' ', "-"));
            let actual = measure(&cli, pilot, &format!("{id}-pilot"));
            let reference = measure(&cli, native, &format!("{id}-native"));
            for end in ["min", "max"] {
                for axis in 0..3 {
                    assert!(
                        (actual["body"]["bbox"][end][axis].as_f64().unwrap()
                            - reference["body"]["bbox"][end][axis].as_f64().unwrap())
                        .abs()
                            < 0.001,
                        "{id}: bbox: args={args}; pilot={}; native={}",
                        actual["body"]["bbox"],
                        reference["body"]["bbox"]
                    );
                }
            }
            let dv = actual["body"]["volume_mm3"].as_f64().unwrap()
                - reference["body"]["volume_mm3"].as_f64().unwrap();
            assert!(dv.abs() < 0.01, "{id}: volume difference {dv}");
            let ap = planes(&actual);
            let rp = planes(&reference);
            assert_eq!(
                ap.keys().collect::<Vec<_>>(),
                rp.keys().collect::<Vec<_>>(),
                "{id}: face planes"
            );
            for (key, values) in &ap {
                for (value, other) in values.iter().zip(&rp[key]) {
                    assert!(
                        (value - other).abs() < 0.01,
                        "{id}: face area/first moment {key:?}"
                    );
                }
            }
            records.push(json!({"id":id,"prompt":prompt,"supplied_args":supplied,"coerced_args":args,"changes":changes,"pilot":actual,"native":reference,"volume_difference_mm3":dv,"face_planes_areas_moments_match":true}));
        }
    }
    assert_eq!(records.len(), 28);
}

#[test]
fn native_shell_direction_probes() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let mut records = Vec::new();
    for direction in ["default", "inside", "outside", "both"] {
        let mut shell = json!({"faces":[[30,20,20]],"thickness":2});
        if direction != "default" {
            shell["direction"] = json!(direction);
        }
        let state = measure(
            &cli,
            vec![
                step(
                    "solid.box",
                    json!({"length":60,"width":40,"height":20,"corner":[0,0,0]}),
                ),
                step("solid.shell", shell),
            ],
            &format!("direction-{direction}"),
        );
        records.push(json!({"direction":direction,"state":state}));
    }
    assert_eq!(records[0]["state"]["body"], records[1]["state"]["body"]);
    assert_ne!(
        records[0]["state"]["body"]["bbox"],
        records[2]["state"]["body"]["bbox"]
    );
    assert_ne!(
        records[0]["state"]["body"]["bbox"],
        records[3]["state"]["body"]["bbox"]
    );
}
