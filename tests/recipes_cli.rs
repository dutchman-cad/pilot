//! Optional clean-room contract smoke test against the public SolidCraft MCP binary.
#![cfg(feature = "proxy")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::{catalog::Catalog, planner};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
struct Session {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}

#[test]
fn public_binary_sketch_axes_and_world_holes() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    for (plane, min, max) in [
        ("XY", [9., 2., 0.], [11., 4., 5.]),
        ("XZ", [9., 0., -4.], [11., 5., -2.]),
        ("YZ", [0., 9., 2.], [5., 11., 4.]),
    ] {
        let mut s = Session::new(&path);
        s.execute("sketch.create", json!({"plane":plane}));
        s.execute("sketch.circle.center", json!({"center":[10,3],"radius":1}));
        s.execute("sketch.finish", json!({}));
        s.execute("solid.extrude", json!({"distance":5,"operation":"new"}));
        check_shortcut_body(&mut s, std::f64::consts::PI * 5., min, max, false);
    }
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for axes in [
        vec!["X"],
        vec!["Y"],
        vec!["Z"],
        vec!["X", "Y"],
        vec!["X", "Z"],
        vec!["Y", "Z"],
        vec!["X", "Y", "Z"],
    ] {
        let holes: Vec<_> = axes
            .iter()
            .map(|a| json!({"axis":a,"diameter":8}))
            .collect();
        let p=lib.build(&json!({"recipe":"cross_drilled_box","args":{"length":20,"width":20,"height":20,"holes":holes}}),&json!({"messages":[]}),&cat).unwrap();
        let mut s = Session::new(&path);
        for c in &p.calls {
            s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
        }
        let r: f64 = 4.;
        let overlap = match axes.len() {
            2 => 16. * r.powi(3) / 3.,
            3 => 8. * 2_f64.sqrt() * r.powi(3),
            _ => 0.,
        };
        let want = 8000. - axes.len() as f64 * std::f64::consts::PI * r * r * 20. + overlap;
        check_shortcut_body(&mut s, want, [0.; 3], [20.; 3], false);
        let state = s.execute("document.inspect", json!({"measure":true}));
        let timeline = state["timeline"].as_array().unwrap();
        assert!(
            timeline
                .iter()
                .all(|f| !f["type"].as_str().unwrap().contains("Sketch")
                    && !f["type"].as_str().unwrap().contains("ConstructionPlane")),
            "{state}"
        );
        assert_eq!(
            timeline
                .iter()
                .filter(|f| f["type"] == "HoleFeature")
                .count(),
            axes.len()
        );
        let faces = s.execute("model.faces", json!({"body":"PilotRecipe1"}));
        let faces = faces["faces"].as_array().unwrap();
        let mut round_axes = std::collections::BTreeSet::new();
        for f in faces.iter().filter(|f| f.get("cylinder").is_some()) {
            let c = &f["cylinder"];
            assert!((c["radius"].as_f64().unwrap() - 4.).abs() < 0.001, "{c}");
            let axis = (0..3)
                .find(|i| c["axis"][i].as_f64().unwrap().abs() > 0.99)
                .unwrap();
            for i in (0..3).filter(|i| *i != axis) {
                assert!(
                    (c["origin"][i].as_f64().unwrap() - 10.).abs() < 0.001,
                    "{c}"
                );
            }
            round_axes.insert(axis);
        }
        assert_eq!(round_axes.len(), axes.len());
        // Each planar end face loses one circular opening. Cylindrical patch
        // counts vary with kernel seam splitting, so count the actual openings.
        let openings = faces
            .iter()
            .filter(|f| {
                !f["plane_normal"].is_null() && f["area"].as_f64().is_some_and(|a| a < 399.)
            })
            .count();
        assert_eq!(openings, 2 * axes.len(), "{state}");
    }
}

#[test]
fn public_binary_offset_blind_cross_holes() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for axis in ["x", "y", "z"] {
        let args = json!({"length":20,"width":20,"height":20,"corner_x":-4,"corner_y":5,"corner_z":-6,format!("hole_{axis}_diameter"):4,format!("hole_{axis}_offset_u"):2,format!("hole_{axis}_offset_v"):-3,format!("hole_{axis}_depth"):5});
        let p = lib
            .build(
                &json!({"recipe":"cross_drilled_box","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        s.run(&p);
        check_shortcut_body(
            &mut s,
            8000. - 20. * std::f64::consts::PI,
            [-4., 5., -6.],
            [16., 25., 14.],
            false,
        );
    }
}

#[test]
fn public_binary_construction_plane_result_contract() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let mut s = Session::new(&path);
    let plane = s.execute("construct.plane.offset", json!({"base":"XY","offset":10}));
    assert_eq!(plane["name"], "Plane1");
    let state = s.execute("document.inspect", json!({}));
    assert!(state["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["type"]
            .as_str()
            .is_some_and(|t| t.contains("ConstructionPlane"))));
}

#[test]
fn public_binary_offset_box_with_explicit_side_hole() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let p=dutchman_pilot::recipe_library::Library::embedded().unwrap().build(
        &json!({"recipe":"cross_drilled_box","args":{"length":20,"width":30,"height":40,"corner":[-4,5,-6],"holes":[{"axis":"Y","diameter":6,"position":[3,35,5]}]}}),
        &json!({"messages":[]}),&Catalog::bundled().unwrap()).unwrap();
    let mut s = Session::new(&path);
    for c in &p.calls {
        s.execute(
            c["arguments"]["command"].as_str().unwrap(),
            c["arguments"]["params"].clone(),
        );
    }
    check_shortcut_body(
        &mut s,
        24000. - std::f64::consts::PI * 9. * 30.,
        [-4., 5., -6.],
        [16., 35., 34.],
        false,
    );
}

#[test]
fn public_binary_centred_cross_hole_fills() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (position, partial) in [
        (None, false),
        (None, true),
        (Some([10, 10, 10]), false),
        (Some([0, 10, 10]), false),
        (Some([20, 10, 10]), false),
    ] {
        let mut args = json!({"length":20,"width":20,"height":20,"hole_x_diameter":8,"hole_y_diameter":8,"hole_z_diameter":8});
        if partial {
            args["hole_x_position_y"] = json!(10);
        }
        if let Some(p) = position {
            for (axis, point) in [
                ("x", p),
                ("y", [p[1], p[0], p[2]]),
                ("z", [p[1], p[2], p[0]]),
            ] {
                for (key, n) in ["x", "y", "z"].into_iter().zip(point) {
                    args[format!("hole_{axis}_position_{key}")] = json!(n);
                }
            }
        }
        let p = lib
            .build(
                &json!({"recipe":"cross_drilled_box","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        s.run(&p);
        check_shortcut_body(&mut s, 5708.1, [0.; 3], [20.; 3], false);
        let faces = s.execute("model.faces", json!({"body":"PilotRecipe1"}));
        let mut axes = std::collections::BTreeSet::new();
        for f in faces["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f.get("cylinder").is_some())
        {
            let c = &f["cylinder"];
            assert!((c["radius"].as_f64().unwrap() - 4.).abs() < 0.001);
            axes.insert(
                (0..3)
                    .find(|i| c["axis"][i].as_f64().unwrap().abs() > 0.99)
                    .unwrap(),
            );
        }
        assert_eq!(axes.len(), 3);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Session {
    fn new(path: &std::ffi::OsStr) -> Self {
        Self::with_start(path, None)
    }
    fn with_start(path: &std::ffi::OsStr, start: Option<&std::path::Path>) -> Self {
        let mut command = Command::new(path);
        command.arg("mcp");
        if let Some(start) = start {
            command.arg("--in").arg(start);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            id: 0,
        }
    }
    fn execute(&mut self, command: &str, params: Value) -> Value {
        self.tool("execute", json!({"command":command,"params":params}))
    }
    fn tool(&mut self, command: &str, arguments: Value) -> Value {
        self.id += 1;
        writeln!(self.input,"{}",json!({"jsonrpc":"2.0","id":self.id,"method":"tools/call","params":{"name":command,"arguments":arguments}})).unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        assert!(line.len() < 16 * 1024 * 1024);
        let response: Value = serde_json::from_str(&line).unwrap();
        assert!(response.get("error").is_none(), "{command}: {response}");
        assert_ne!(response["result"]["isError"], true, "{command}: {response}");
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    fn run(&mut self, p: &planner::Plan) {
        if p.calls.len() > 2 {
            let commands: Vec<_> = p.calls[..p.calls.len() - 1]
                .iter()
                .map(|c| c["arguments"].clone())
                .collect();
            self.tool("batch", json!({"commands":commands}));
            self.execute("document.inspect", json!({"measure":true}));
            return;
        }
        for c in &p.calls {
            self.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
        }
    }
}

#[test]
fn public_binary_dev_joint_exact_matches_forms_at_ninety_degrees() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/dev_joint_exact.json")).unwrap();
    let catalog = Catalog::bundled().unwrap();
    let library = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let mut measured = Vec::new();
        for form_path in [false, true] {
            let mut session = Session::new(&path);
            for command in fixture["setup"].as_array().unwrap() {
                session.execute(
                    command["command"].as_str().unwrap(),
                    command["params"].clone(),
                );
            }
            let mut request = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
            let mut completed = false;
            for stage in 0..3 {
                let mut plan = planner::plan_direct(&request, &catalog)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{text}: stage {stage}"));
                if plan.rule == "revolute_joint" && form_path {
                    let form = library
                        .build(
                            &json!({"recipe":"revolute_joint","args":fixture["form_args"]}),
                            &request,
                            &catalog,
                        )
                        .unwrap();
                    assert_eq!(plan.calls, form.calls, "{text}");
                    plan = form;
                }
                let calls: Vec<_> = plan.calls.iter().enumerate().map(|(n,c)| json!({"id":format!("dev_{stage}_{n}"),"function":{"name":"execute","arguments":c["arguments"].to_string()}})).collect();
                request["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"assistant","tool_calls":calls}));
                for (n, call) in plan.calls.iter().enumerate() {
                    let result = session.execute(
                        call["arguments"]["command"].as_str().unwrap(),
                        call["arguments"]["params"].clone(),
                    );
                    request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":format!("dev_{stage}_{n}"),"content":result.to_string()}));
                }
                if plan.rule == "revolute_joint" {
                    assert_eq!(
                        planner::completion(&request, plan.expected.as_ref().unwrap()),
                        None,
                        "{text}: form={form_path}"
                    );
                    completed = true;
                    break;
                }
                assert_eq!(plan.rule, "resolve_joint");
            }
            assert!(completed, "{text}: form={form_path}");
            measured.push((
                session.execute("component.list", json!({})),
                session.execute("joint.list", json!({})),
            ));
        }
        assert_eq!(measured[0], measured[1], "{text}");
    }
}

#[test]
fn public_binary_contact_circle_revolute_joints() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../bench/sets/v2/references/09-revolute-joint.json"
    ))
    .unwrap();
    // Different sizes and an off-origin pivot exercise measured picks and translations.
    for (scale, x, y, angle, offset) in [
        (1., 0., 0., 45., 0.),
        (0.5, 17., -13., 30., 0.),
        (2.54, -23., 31., -60., 0.),
        (1., -17., 10., 25., 0.2),
    ] {
        let mut session = Session::new(&path);
        for c in &reference["commands"].as_array().unwrap()[..8] {
            let mut params = c["params"].clone();
            let moving = params["body_name"] == "Arm" || c["command"] == "solid.hole";
            for key in ["length", "width", "height", "diameter"] {
                if let Some(v) = params[key].as_f64() {
                    params[key] = json!(v * scale);
                }
            }
            for key in ["corner", "base", "position"] {
                if let Some(v) = params[key].as_array().cloned() {
                    params[key] = json!([
                        v[0].as_f64().unwrap() * scale + x + if moving { offset } else { 0. },
                        v[1].as_f64().unwrap() * scale + y,
                        v[2].as_f64().unwrap() * scale
                    ]);
                }
            }
            session.execute(c["command"].as_str().unwrap(), params);
        }
        let before = session.execute("document.inspect", json!({"measure":true}));
        let mut req = json!({"messages":[{"role":"user","content":format!("Connect Base and Arm with a revolute joint and swing Arm {angle} degrees. Keep Base fixed and Arm upright.")}],"tools":[{"function":{"name":"execute"}},{"function":{"name":"batch"}}]});
        let mut action = None;
        for stage in 0..3 {
            let p = planner::plan_direct(&req, &catalog)
                .unwrap()
                .unwrap_or_else(|| panic!("stage {stage}: {req}"));
            let calls:Vec<_>=p.calls.iter().enumerate().map(|(n,c)|json!({"id":format!("stage{stage}_{n}"),"function":{"name":"execute","arguments":c["arguments"].to_string()}})).collect();
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"assistant","tool_calls":calls}));
            for (n, c) in p.calls.iter().enumerate() {
                let result = session.execute(
                    c["arguments"]["command"].as_str().unwrap(),
                    c["arguments"]["params"].clone(),
                );
                req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":format!("stage{stage}_{n}"),"content":result.to_string()}));
            }
            if p.rule == "revolute_joint" {
                action = Some(p);
                break;
            }
            assert_eq!(p.rule, "resolve_joint");
        }
        let p = action.unwrap();
        assert_eq!(
            planner::completion(&req, p.expected.as_ref().unwrap()),
            None,
            "{req}"
        );
        assert!(planner::plan_direct(&req, &catalog).unwrap().is_none());
        let after = session.execute("document.inspect", json!({"measure":true}));
        for body in 0..2 {
            assert!(
                (before["bodies"][body]["volume_mm3"].as_f64().unwrap()
                    - after["bodies"][body]["volume_mm3"].as_f64().unwrap())
                .abs()
                    < 0.01
            );
            assert_eq!(
                before["bodies"][body]["bbox"],
                after["bodies"][body]["bbox"]
            );
            assert_eq!(after["bodies"][body]["face_types"]["cylinder"], 1);
        }
        let types: Vec<_> = after["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["type"].as_str())
            .collect();
        assert!(types.contains(&"CylinderFeature") && types.contains(&"HoleFeature"));
        assert!((p.slots["pin_circle"][2].as_f64().unwrap() - 10. * scale).abs() < 0.001);
    }
}
#[test]
fn public_binary_l_profiles_are_closed_fully_constrained_and_extruded() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    let tasks: Value =
        serde_json::from_str(include_str!("../bench/sets/constraints/tasks.json")).unwrap();
    for task in tasks["tasks"].as_array().unwrap() {
        let mut session = Session::new(&path);
        let req = json!({"messages":[{"role":"user","content":task["prompt"]}],"tools":[{"function":{"name":"execute"}}]});
        let p = planner::plan(&req, &catalog).unwrap().unwrap();
        for call in &p.calls {
            session.execute(
                call["arguments"]["command"].as_str().unwrap(),
                call["arguments"]["params"].clone(),
            );
        }
        let state = session.execute("document.inspect", json!({"measure":true}));
        assert_eq!(state["sketches"][0]["dof"], 0, "{state}");
        assert_eq!(state["sketches"][0]["profiles"], 1, "{state}");
        assert_eq!(state["sketches"][0]["curves"], 6, "{state}");
        assert_eq!(state["params"].as_array().unwrap().len(), 4, "{state}");
        assert_eq!(state["bodies"].as_array().unwrap().len(), 1, "{state}");
        let volume = state["bodies"][0]["volume_mm3"].as_f64().unwrap();
        assert!(
            (volume - task["check"]["volume"].as_f64().unwrap()).abs() < 0.01,
            "{state}"
        );
    }
}

#[test]
fn public_binary_recipe_geometry() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let c = Catalog::bundled().unwrap();
    for (text,edit) in [
        ("build a block 17x23x6mm",false),
        ("build a rod diameter 7mm tall 29mm",false),
        ("build a sphere radius 7mm",false),
        ("build a plate 72x48x6mm with a centred through hole diameter 8mm",false),
        ("build a plate 80x60x6mm with 2 through holes diameter 4mm at (12mm,15mm), (65mm,40mm)",false),
        ("build a plate 80x60x6mm with holes diameter 4mm through in a rectangular pattern 2 rows 3 columns pitch 20x20mm centered",false),
        ("build a plate 80x60x6mm with 5 through holes diameter 4mm in a circular pattern radius 18mm centered equally spaced starting at +X",false),
        ("sketch a rectangle 13x21mm on XZ then extrude 6mm",false),
        ("sketch a circle radius 9mm on YZ then extrude 7mm",false),
        ("Sketch a 46 by 28 mm rectangle on the X-Y plane and extrude it 13 mm.",false),
        ("On XY, sketch a circle of diameter 18mm and extrude the sketch 7mm",false),
        ("draw an 18mm circle on XZ and extrude it 7",false),
        ("plate 90x40x6mm with two 6mm through holes one 12mm from each short end halfway across the width",false),
        ("plate 40x90x6mm with two 6mm through holes one 12mm from each short end halfway across the width",false),
        ("plate 90x40x6mm with four 6mm holes right through one near each corner 12mm in from both adjacent edges",false),
        ("shell a box 70x50x30mm wall 2mm open top",false),
        ("revolve a rectangle radius 8mm height 35mm from axis on XZ about Z full turn",false),
        ("move Blank 7mm along X",true), ("rename Blank to Spacer",true),
        ("a hollow tube OD 20mm ID 14mm length 60mm",false),
        ("a tube OD 20mm wall thickness 3mm length 60mm",false),
        ("a plate 84x36x9mm with one centred 6mm through hole",false),
        ("scale Blank about origin by 150%",true),
        ("double Blank about origin",true), ("half Blank about origin",true),
        ("mirror Blank across XZ not keeping the original",true),
        ("mirror Blank about XY",true), ("fillet all edges of Blank radius 2mm",true),
        ("chamfer top edges of Blank distance 2mm",true), ("chamfer all edges of Blank distance 2mm",true),
        ("Drill a hole 3mm in diameter through the body at (5,5,10).",true),
    ] {
        let mut s=Session::new(&path);
        if edit {s.execute("solid.box",json!({"length":10,"width":10,"height":10,"body_name":"Blank","operation":"new"}));}
        let before=s.execute("document.inspect",json!({"measure":true}));
        let mut req=json!({"messages":[{"role":"tool","content":before.to_string()},{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        let p=planner::plan(&req,&c).unwrap().unwrap_or_else(||panic!("{text}"));
        if p.continuation.is_some() {
            let edges=s.execute("model.edges",json!({"body":"Blank"}));
            req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":edges.to_string()}));
            let continued=planner::resume(&req,&p,&c).unwrap().unwrap_or_else(||panic!("cannot continue {text}"));
            s.run(&continued);
        } else {s.run(&p);}
        let after=s.execute("document.inspect",json!({"measure":true}));
        if p.rule == "hole" {
            let actual=after["bodies"][0]["volume_mm3"].as_f64().unwrap();
            let expected=1000.0-std::f64::consts::PI*1.5_f64.powi(2)*10.0;
            assert!((actual-expected).abs()<(expected*0.001).max(0.01),"{text}: {after}");
        }
        req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":after.to_string()}));
        assert_eq!(planner::completion(&req,p.expected.as_ref().unwrap()),None,"{text}: {after}");
    }
}

#[test]
fn public_binary_common_closed_profiles_on_every_plane() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    for plane in ["XY", "XZ", "YZ"] {
        for (shape, sizes, count, area) in [
            ("L", "thickness 3", 4, 201.0),
            (
                "L",
                "horizontal leg 3 thick and vertical leg 4 thick",
                4,
                228.0,
            ),
            ("T", "flange 4 stem 6", 4, 316.0),
            (
                "T",
                "flange 4 stem 6 stem offset 0 from the centre",
                5,
                316.0,
            ),
            (
                "T",
                "flange 4 stem 6 stem offset -3 from the centre",
                5,
                316.0,
            ),
            (
                "T",
                "flange 4 stem 6 stem offset 15 from the left",
                5,
                316.0,
            ),
            (
                "T",
                "flange 4 stem 6 stem offset 3 from the centre",
                5,
                316.0,
            ),
            ("U", "wall 3", 4, 282.0),
            ("U", "web 3 flange 4", 5, 336.0),
            ("U", "web 3 flange 3", 4, 282.0),
            ("U", "wall 3 opening down", 4, 282.0),
            ("U", "wall 3 opening right", 4, 312.0),
            ("C", "wall 3", 4, 312.0),
            ("C", "web 3 flange 4", 5, 386.0),
            ("C", "web 3 flange 3", 4, 312.0),
            ("C", "wall 3 opening left", 4, 312.0),
            ("C", "wall 3 opening +X", 4, 312.0),
            ("C", "wall 3 opening -X", 4, 312.0),
            ("C", "wall 3 opening up", 4, 282.0),
            ("dimensioned rectangle", "", 2, 1200.0),
        ] {
            for centred in [false, true] {
                if centred && shape == "L" {
                    continue;
                }
                let placement = if centred { "centred on the origin" } else { "" };
                let text = format!("Make a {shape} on {plane} {placement} width 40 height 30 {sizes} then extrude 12");
                let req = json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
                let p = planner::plan(&req, &catalog)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{text}"));
                let mut session = Session::new(&path);
                for call in &p.calls {
                    session.execute(
                        call["arguments"]["command"].as_str().unwrap(),
                        call["arguments"]["params"].clone(),
                    );
                }
                let state = session.execute("document.inspect", json!({"measure":true}));
                assert_eq!(state["sketches"][0]["dof"], 0, "{text}: {state}");
                assert_eq!(state["sketches"][0]["profiles"], 1, "{text}: {state}");
                assert_eq!(
                    state["params"].as_array().unwrap().len(),
                    count,
                    "{text}: {state}"
                );
                assert_eq!(
                    state["bodies"].as_array().unwrap().len(),
                    1,
                    "{text}: {state}"
                );
                let body = &state["bodies"][0];
                assert!(
                    (body["volume_mm3"].as_f64().unwrap() - area * 12.0).abs() < 0.01,
                    "{text}: {state}"
                );
                let (x0, y0, x1, y1) = if centred {
                    (-20.0, -15.0, 20.0, 15.0)
                } else {
                    (0.0, 0.0, 40.0, 30.0)
                };
                let (min, max) = match plane {
                    "XY" => ([x0, y0, 0.0], [x1, y1, 12.0]),
                    "XZ" => ([x0, 0.0, -y1], [x1, 12.0, -y0]),
                    _ => ([0.0, x0, y0], [12.0, x1, y1]),
                };
                for axis in 0..3 {
                    assert!(
                        (body["bbox"]["min"][axis].as_f64().unwrap() - min[axis]).abs() < 0.001,
                        "{text}: {state}"
                    );
                    assert!(
                        (body["bbox"]["max"][axis].as_f64().unwrap() - max[axis]).abs() < 0.001,
                        "{text}: {state}"
                    );
                }
            }
        }
    }
}

#[test]
fn public_binary_compound_plates() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    for plane in ["XY", "XZ", "YZ"] {
        for (features,holes,rounds,volume,kind) in [
            ("with a centred 30mm bore through and six 8mm holes evenly spaced on a 56mm bolt circle then round the four vertical corner edges radius 10mm",7,11,80.0*80.0*8.0-std::f64::consts::PI*(225.0+96.0)*8.0-(4.0-std::f64::consts::PI)*100.0*8.0,"FilletFeature"),
            ("with six 8mm holes equally spaced on PCD 56mm then chamfer the four side edges distance 5mm",6,6,80.0*80.0*8.0-std::f64::consts::PI*96.0*8.0-2.0*25.0*8.0,"ChamferFeature"),
            ("with holes diameter 4mm in a centred grid 2 rows 3 columns pitch 20x20mm then fillet the four corner edges radius 5mm",6,10,80.0*80.0*8.0-std::f64::consts::PI*24.0*8.0-(4.0-std::f64::consts::PI)*25.0*8.0,"FilletFeature"),
        ] {
            let text=format!("Make a plate 80x80x8mm centred on the origin on {plane} {features}");
            let req=json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
            let p=planner::plan(&req,&catalog).unwrap().unwrap_or_else(||panic!("{text}"));
            let mut s=Session::new(&path);
            for c in &p.calls {s.execute(c["arguments"]["command"].as_str().unwrap(),c["arguments"]["params"].clone());}
            let state=s.execute("document.inspect",json!({"measure":true}));
            assert_eq!(state["bodies"].as_array().unwrap().len(),1,"{state}");
            assert!((state["bodies"][0]["volume_mm3"].as_f64().unwrap()-volume).abs()<volume*0.001,"{text}: {state}");
            let timeline=state["timeline"].as_array().unwrap();
            assert_eq!(timeline.iter().filter(|f|f["type"]=="HoleFeature").count(),holes,"{state}");
            assert_eq!(timeline.iter().filter(|f|f["type"]==kind).count(),1,"{state}");
            let (min,max)=match plane {"XZ"=>([-40.0,0.0,-40.0],[40.0,8.0,40.0]),"YZ"=>([0.0,-40.0,-40.0],[8.0,40.0,40.0]),_=>([-40.0,-40.0,0.0],[40.0,40.0,8.0])};
            for axis in 0..3 {
                assert!((state["bodies"][0]["bbox"]["min"][axis].as_f64().unwrap()-min[axis]).abs()<0.001,"{state}");
                assert!((state["bodies"][0]["bbox"]["max"][axis].as_f64().unwrap()-max[axis]).abs()<0.001,"{state}");
            }
            let faces=s.execute("model.faces",json!({"body":"PilotRecipe1"}));
            assert_eq!(state["bodies"][0]["face_types"]["cylinder"],rounds,"{state}");
            let blends=faces["faces"].as_array().unwrap().iter().filter(|f|f["cylinder"].is_object() && f["name"].as_str().unwrap().contains(":blend:")).count();
            assert_eq!(blends,if kind=="FilletFeature" {4}else{0},"{faces}");
        }
    }
}

#[test]
fn public_binary_compound_all_edges_and_combined_patterns() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    for (text,count,rounding) in [
        ("Make a plate 80x80x8mm with its corner at the origin with a centred 10mm bore through then fillet all edges radius 1mm",1,true),
        ("Make a plate 80x80x8mm centred on the origin with six 4mm holes equally spaced on a bolt circle radius 20mm then chamfer all edges distance 1mm",6,true),
        ("Make a plate 160x160x10mm centred on the origin with a centred 8mm bore through and six 4mm holes evenly spaced on PCD 40mm and holes diameter 4mm in a centred grid 2 rows 2 columns pitch 80x80mm then round the four vertical edges radius 8mm",11,true),
        ("Make a plate 4x4x1/2 inch centred on the origin with six 1/8 inch holes equally spaced on a 2 inch bolt circle",6,false),
    ] {
        let mut s=Session::new(&path);
        let mut req=json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]});
        let p=planner::plan(&req,&catalog).unwrap().unwrap_or_else(||panic!("{text}"));
        let mut last=Value::Null;
        for c in &p.calls {last=s.execute(c["arguments"]["command"].as_str().unwrap(),c["arguments"]["params"].clone());}
        if p.continuation.is_some() {
            req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":last.to_string()}));
            let next=planner::resume(&req,&p,&catalog).unwrap().unwrap_or_else(||panic!("{text}: {last}"));
            assert_eq!(next.calls[0]["arguments"]["command"],if text.contains("chamfer") {"solid.chamfer"}else{"solid.fillet"});
            for c in &next.calls {s.execute(c["arguments"]["command"].as_str().unwrap(),c["arguments"]["params"].clone());}
        }
        let state=s.execute("document.inspect",json!({"measure":true}));
        let timeline=state["timeline"].as_array().unwrap();
        assert!(timeline.iter().all(|f|f["error"].is_null()),"{text}: {state}");
        assert_eq!(timeline.iter().filter(|f|f["type"]=="HoleFeature").count(),count,"{state}");
        assert_eq!(timeline.iter().filter(|f|f["type"]=="FilletFeature" || f["type"]=="ChamferFeature").count(),usize::from(rounding),"{state}");
        assert_eq!(state["bodies"].as_array().unwrap().len(),1);
        assert!(state["bodies"][0]["volume_mm3"].as_f64().unwrap()>0.0);
        req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","content":state.to_string()}));
        assert_eq!(planner::completion(&req,p.expected.as_ref().unwrap()),None,"{text}: {state}");
    }
}

#[test]
fn public_binary_shell_lips_are_joined_rings() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let c = Catalog::bundled().unwrap();
    for centred in [false, true] {
        for bottom in [false, true] {
            for outer in [false, true] {
                for floor in [1.0, 2.0, 4.0] {
                    let text = format!("Make an enclosure, a box 80x50x30mm wall 2mm floor {floor}mm open {} {} {} lip width 1mm height 2mm", if bottom {"bottom"} else {"top"}, if centred {"centred at the origin"} else {"with one corner at the origin"}, if outer {"outer"} else {"inner"});
                    let p = planner::plan(&json!({"messages":[{"role":"user","content":text}],"tools":[{"function":{"name":"execute"}}]}), &c).unwrap().unwrap_or_else(|| panic!("{text}"));
                    let mut s = Session::new(&path);
                    for call in &p.calls {
                        s.execute(
                            call["arguments"]["command"].as_str().unwrap(),
                            call["arguments"]["params"].clone(),
                        );
                    }
                    let state = s.execute("document.inspect", json!({"measure":true}));
                    assert_eq!(
                        state["bodies"].as_array().unwrap().len(),
                        1,
                        "{text}: {state}"
                    );
                    let body = &state["bodies"][0];
                    assert_eq!(body["kernel"]["shells"], 1, "{text}: {state}");
                    let volume =
                        120000.0 - 76.0 * 46.0 * (30.0 - floor) + if outer { 512.0 } else { 496.0 };
                    assert!(
                        (body["volume_mm3"].as_f64().unwrap() - volume).abs() < 0.01,
                        "{text}: {state}"
                    );
                    let offset = if centred {
                        [-40.0, -25.0, -15.0]
                    } else {
                        [0.0; 3]
                    };
                    let min = [
                        offset[0],
                        offset[1],
                        offset[2] - if bottom { 2.0 } else { 0.0 },
                    ];
                    let max = [
                        offset[0] + 80.0,
                        offset[1] + 50.0,
                        offset[2] + 30.0 + if bottom { 0.0 } else { 2.0 },
                    ];
                    for axis in 0..3 {
                        assert!(
                            (body["bbox"]["min"][axis].as_f64().unwrap() - min[axis]).abs() < 0.001,
                            "{text}: {state}"
                        );
                        assert!(
                            (body["bbox"]["max"][axis].as_f64().unwrap() - max[axis]).abs() < 0.001,
                            "{text}: {state}"
                        );
                    }
                    assert_eq!(
                        body["face_types"]["plane"], body["faces"],
                        "{text}: {state}"
                    );
                    for kind in ["cylinder", "sphere", "cone", "torus"] {
                        assert_eq!(
                            body["face_types"][kind].as_u64().unwrap_or(0),
                            0,
                            "{text}: {state}"
                        );
                    }
                    let timeline = state["timeline"].to_string();
                    assert!(timeline.contains("Shell"), "{state}");
                    assert!(timeline.contains("Extrude"), "{state}");
                }
            }
        }
    }
}

fn check_shortcut_body(
    session: &mut Session,
    volume: f64,
    min: [f64; 3],
    max: [f64; 3],
    profile: bool,
) {
    let state = session.execute("document.inspect", json!({"measure":true}));
    assert_eq!(state["bodies"].as_array().unwrap().len(), 1, "{state}");
    let body = &state["bodies"][0];
    assert!(
        (body["volume_mm3"].as_f64().unwrap() - volume).abs() <= (volume.abs() * 0.001).max(0.01),
        "{state}"
    );
    for axis in 0..3 {
        assert!(
            (body["bbox"]["min"][axis].as_f64().unwrap() - min[axis]).abs() < 0.001,
            "{state}"
        );
        assert!(
            (body["bbox"]["max"][axis].as_f64().unwrap() - max[axis]).abs() < 0.001,
            "{state}"
        );
    }
    if profile {
        assert_eq!(state["sketches"][0]["dof"], 0, "{state}");
    }
}
#[test]
fn public_binary_typed_profile_shortcuts() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    for plane in ["XY", "XZ", "YZ"] {
        for (kind, area) in [
            ("L", 201.0),
            ("T", 201.0),
            ("U", 282.0),
            ("C", 312.0),
            ("rectangle", 1200.0),
        ] {
            let mut args = json!({"shape":"profile","kind":kind,"plane":plane,"width":40,"height":30,"depth":12});
            if kind != "rectangle" {
                args["thickness"] = json!(3);
            }
            let p=planner::shortcut(&args,&json!({"messages":[{"role":"user","content":"Build the specified cross section"}]}),&catalog).unwrap();
            let mut session = Session::new(&path);
            session.run(&p);
            let (min, max) = match plane {
                "XZ" => ([0.0, 0.0, -30.0], [40.0, 12.0, 0.0]),
                "YZ" => ([0.0, 0.0, 0.0], [12.0, 40.0, 30.0]),
                _ => ([0.0; 3], [40.0, 30.0, 12.0]),
            };
            check_shortcut_body(&mut session, area * 12.0, min, max, true);
        }
    }
}
#[test]
fn public_binary_centred_plate_sits_on_sketch_plane() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for features in [
        json!({}),
        json!({"bore_diameter":4}),
        json!({"row_count":3,"row_pitch":10,"hole_diameter":4}),
        json!({"grid_columns":3,"grid_rows":2,"grid_pitch_x":10,"grid_pitch_y":10,"grid_hole_diameter":4}),
        json!({"bolt_hole_count":4,"bolt_circle_diameter":20,"bolt_hole_diameter":4}),
        json!({"diameter":40,"bolt_hole_count":4,"bolt_circle_diameter":20,"bolt_hole_diameter":4}),
    ] {
        let mut args = json!({"length":40,"width":40,"thickness":5,"centred":true});
        args.as_object_mut()
            .unwrap()
            .extend(features.as_object().unwrap().clone());
        if args.get("diameter").is_some() {
            let obj = args.as_object_mut().unwrap();
            obj.remove("length");
            obj.remove("width");
        }
        let p = lib
            .build(
                &json!({"recipe":"plate","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        s.run(&p);
        check_shortcut_body(
            &mut s,
            p.expected.as_ref().unwrap().created_volume_mm3.unwrap(),
            [-20., -20., 0.],
            [20., 20., 5.],
            false,
        );
    }
}

#[test]
fn public_binary_typed_plate_shortcut() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let args = json!({"shape":"plate","length":80,"width":80,"thickness":8,"plane":"XY","centred":true,"bore_diameter":10,"hole_diameter":4,"bolt_circle":{"count":6,"diameter":56,"start_deg":30},"corners":{"kind":"fillet","size":5,"edges":"vertical"}});
    let p =
        planner::shortcut(&args, &json!({"messages":[]}), &Catalog::bundled().unwrap()).unwrap();
    let mut s = Session::new(&path);
    s.run(&p);
    check_shortcut_body(
        &mut s,
        80.0 * 80.0 * 8.0
            - std::f64::consts::PI * 49.0 * 8.0
            - (4.0 - std::f64::consts::PI) * 25.0 * 8.0,
        [-40.0, -40.0, 0.0],
        [40.0, 40.0, 8.0],
        false,
    );
}
#[test]
fn public_binary_typed_enclosure_shortcut() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let args = json!({"shape":"enclosure","length":80,"width":50,"height":30,"wall":2,"floor":4,"open":"bottom","centred":true,"lip":{"width":1,"height":2,"side":"inner"}});
    let p =
        planner::shortcut(&args, &json!({"messages":[]}), &Catalog::bundled().unwrap()).unwrap();
    let mut s = Session::new(&path);
    s.run(&p);
    check_shortcut_body(
        &mut s,
        120000.0 - 76.0 * 46.0 * 26.0 + 496.0,
        [-40.0, -25.0, -17.0],
        [40.0, 25.0, 15.0],
        false,
    );
}
#[test]
fn public_binary_typed_revolute_shortcut() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let catalog = Catalog::bundled().unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../bench/sets/v2/references/09-revolute-joint.json"
    ))
    .unwrap();
    let mut s = Session::new(&path);
    for c in &reference["commands"].as_array().unwrap()[..8] {
        s.execute(c["command"].as_str().unwrap(), c["params"].clone());
    }
    let before = s.execute("document.inspect", json!({"measure":true}));
    let args = json!({"shape":"revolute_joint","pin_component":"Base","hole_component":"Arm","angle_deg":45});
    let mut req =
        json!({"messages":[{"role":"user","content":"Fit these two components together"}]});
    let mut action = None;
    for stage in 0..3 {
        let p = planner::shortcut(&args, &req, &catalog).unwrap();
        let calls:Vec<_>=p.calls.iter().enumerate().map(|(i,c)|json!({"id":format!("typed_{stage}_{i}"),"function":{"name":"execute","arguments":c["arguments"].to_string()}})).collect();
        req["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","tool_calls":calls}));
        for (i, c) in p.calls.iter().enumerate() {
            let result = s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
            req["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":format!("typed_{stage}_{i}"),"content":result.to_string()}));
        }
        if p.rule == "revolute_joint" {
            action = Some(p);
            break;
        }
        assert_eq!(p.rule, "resolve_joint");
    }
    let p = action.unwrap();
    assert_eq!(
        planner::completion(&req, p.expected.as_ref().unwrap()),
        None
    );
    let after = s.execute("document.inspect", json!({"measure":true}));
    for i in 0..2 {
        assert_eq!(before["bodies"][i]["bbox"], after["bodies"][i]["bbox"]);
        assert!(
            (before["bodies"][i]["volume_mm3"].as_f64().unwrap()
                - after["bodies"][i]["volume_mm3"].as_f64().unwrap())
            .abs()
                < 0.01
        );
    }
}

#[test]
fn public_binary_pattern_recipes() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for feature in [false, true] {
        for (id, args, count, kind) in [
            (
                "linear_pattern",
                json!({"count":5,"spacing":10,"direction":"X"}),
                5,
                "RectangularPatternFeature",
            ),
            (
                "grid_pattern",
                json!({"columns":4,"rows":3,"spacing1":10,"spacing2":12,"plane":"XY"}),
                12,
                "RectangularPatternFeature",
            ),
            (
                "circular_pattern",
                json!({"count":8,"axis":"Z","angle_deg":360}),
                8,
                "CircularPatternFeature",
            ),
            (
                "circular_pattern",
                json!({"count":4,"axis":"Z","angle_deg":180}),
                4,
                "CircularPatternFeature",
            ),
        ] {
            let mut session = Session::new(&path);
            session.execute(
                "solid.box",
                json!({"length":2,"width":3,"height":4,"corner":[20,0,0],"body_name":"Seed"}),
            );
            let mut args = args;
            args["target"] = json!(if feature { "last_feature" } else { "body" });
            let mut req = json!({"messages":[{"role":"user","content":"Repeat this seed with the selected pattern"}]});
            let mut expected = None;
            for _ in 0..4 {
                let p = lib
                    .build(&json!({"recipe":id,"args":args}), &req, &cat)
                    .unwrap();
                for c in &p.calls {
                    let result = session.execute(
                        c["arguments"]["command"].as_str().unwrap(),
                        c["arguments"]["params"].clone(),
                    );
                    req["messages"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"role":"tool","content":result.to_string()}));
                }
                if p.expected.is_some() {
                    expected = p.expected;
                    break;
                }
            }
            assert!(expected.is_some());
            assert_eq!(
                planner::completion(&req, expected.as_ref().unwrap()),
                None,
                "{req}"
            );
            let state = session.execute("document.inspect", json!({"measure":true}));
            assert_eq!(state["bodies"].as_array().unwrap().len(), count, "{state}");
            let volume: f64 = state["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b["volume_mm3"].as_f64().unwrap())
                .sum();
            assert!((volume - count as f64 * 24.).abs() < 0.01, "{state}");
            assert_eq!(
                state["timeline"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|f| f["type"] == kind && f["error"].is_null())
                    .count(),
                1,
                "{state}"
            );
        }
    }
}

#[test]
fn public_binary_patterns_inherit_join_and_cut() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for cut in [true, false] {
        for (id, mut args, count) in [
            (
                "linear_pattern",
                json!({"count":3,"spacing":12,"direction":"X"}),
                3,
            ),
            (
                "grid_pattern",
                json!({"columns":3,"rows":2,"spacing1":12,"spacing2":15,"plane":"XY"}),
                6,
            ),
            (
                "circular_pattern",
                json!({"count":8,"axis":"Z","angle_deg":360}),
                8,
            ),
        ] {
            let mut session = Session::new(&path);
            session.execute("solid.box",json!({"length":120,"width":120,"height":5,"corner":[-60,-60,0],"body_name":"Plate"}));
            if cut {
                session.execute(
                    "solid.hole",
                    json!({"position":[20,0,5],"diameter":4,"targets":["Plate"]}),
                );
            } else {
                session.execute("solid.cylinder",json!({"base":[20,0,5],"radius":2,"height":5,"operation":"join","targets":["Plate"]}));
            }
            args["target"] = json!("last_feature");
            args["body"] = json!("Plate");
            let mut req =
                json!({"messages":[{"role":"user","content":"Pattern the last feature"}]});
            let mut expected = None;
            for _ in 0..4 {
                let p = lib
                    .build(&json!({"recipe":id,"args":args}), &req, &cat)
                    .unwrap();
                for c in &p.calls {
                    let result = session.execute(
                        c["arguments"]["command"].as_str().unwrap(),
                        c["arguments"]["params"].clone(),
                    );
                    req["messages"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"role":"tool","content":result.to_string()}));
                }
                if p.expected.is_some() {
                    expected = p.expected;
                    break;
                }
            }
            assert!(expected.is_some());
            assert_eq!(
                planner::completion(&req, expected.as_ref().unwrap()),
                None,
                "{req}"
            );
            let state = session.execute("document.inspect", json!({"measure":true}));
            assert_eq!(state["bodies"].as_array().unwrap().len(), 1, "{state}");
            let want =
                72000. + if cut { -1. } else { 1. } * count as f64 * std::f64::consts::PI * 20.;
            assert!(
                (state["bodies"][0]["volume_mm3"].as_f64().unwrap() - want).abs()
                    < ((want - 72000.).abs() * 0.001).max(0.01),
                "{state}"
            );
            assert!(
                state["timeline"].as_array().unwrap().last().unwrap()["type"]
                    .as_str()
                    .unwrap()
                    .ends_with("PatternFeature")
            );
        }
    }
}

#[test]
fn public_binary_linear_pattern_total_length_and_edge_direction() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let mut s = Session::new(&path);
    s.execute(
        "solid.box",
        json!({"length":2,"width":3,"height":4,"body_name":"Seed"}),
    );
    let state = s.execute("document.inspect", json!({"measure":true}));
    let edges = s.execute("model.edges", json!({"body":"Seed"}));
    let edge = edges["edges"][0]["name"].clone();
    let req = json!({"messages":[{"role":"user","content":"Make fifty in a row along this edge"},{"role":"tool","content":state.to_string()},{"role":"tool","content":edges.to_string()}]});
    let p=lib.build(&json!({"recipe":"linear_pattern","args":{"target":"body","count":50,"total_length":490,"edge":edge}}),&req,&cat).unwrap();
    assert_eq!(p.calls[0]["arguments"]["params"]["spacing1"], 10.);
    for c in &p.calls {
        s.execute(
            c["arguments"]["command"].as_str().unwrap(),
            c["arguments"]["params"].clone(),
        );
    }
    let state = s.execute("document.inspect", json!({"measure":true}));
    assert_eq!(state["bodies"].as_array().unwrap().len(), 50);
    let volume: f64 = state["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["volume_mm3"].as_f64().unwrap())
        .sum();
    assert!((volume - 1200.).abs() < 0.01);
    assert_eq!(
        state["timeline"].as_array().unwrap().last().unwrap()["type"],
        "RectangularPatternFeature"
    );
    let measured = json!({"messages":[{"role":"tool","content":state.to_string()}]});
    assert_eq!(
        planner::completion(&measured, p.expected.as_ref().unwrap()),
        None
    );
}

// Reconstruct the cut loops from the public B-rep edges, independently of recipe slots.
fn honeycomb_loops(s: &mut Session, plane: &str, base: f64, radius: f64) -> Vec<Vec<[f64; 2]>> {
    let result = s.execute("model.edges", json!({"body":"HoneycombPlate"}));
    let (u, v, n) = match plane {
        "XZ" => (0, 2, 1),
        "YZ" => (1, 2, 0),
        _ => (0, 1, 2),
    };
    let mut edges: Vec<([f64; 2], [f64; 2])> = result["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| {
            let a = &e["start"];
            let b = &e["end"];
            if (a[n].as_f64()? - base).abs() > 1e-6
                || (b[n].as_f64()? - base).abs() > 1e-6
                || (e["length"].as_f64()? - radius).abs() > 1e-6
            {
                return None;
            }
            Some((
                [a[u].as_f64()?, a[v].as_f64()?],
                [b[u].as_f64()?, b[v].as_f64()?],
            ))
        })
        .collect();
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
    let mut loops = Vec::new();
    while let Some((a, b)) = edges.pop() {
        let mut points = vec![a, b];
        while !close(*points.last().unwrap(), a) {
            let end = *points.last().unwrap();
            let i = edges
                .iter()
                .position(|(a, b)| close(*a, end) || close(*b, end))
                .unwrap();
            let (a, b) = edges.swap_remove(i);
            points.push(if close(a, end) { b } else { a });
            assert!(points.len() <= 7);
        }
        points.pop();
        assert_eq!(points.len(), 6);
        loops.push(points);
    }
    loops
}
fn point_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let t =
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1])).clamp(0., 1.);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}
fn measured_honeycomb_clearances(
    loops: &[Vec<[f64; 2]>],
    length: f64,
    width: f64,
    offset: [f64; 2],
    pitch: f64,
) -> (f64, f64) {
    let border = loops
        .iter()
        .flatten()
        .map(|p| {
            (p[0] - offset[0])
                .min(length + offset[0] - p[0])
                .min(p[1] - offset[1])
                .min(width + offset[1] - p[1])
        })
        .fold(f64::INFINITY, f64::min);
    let centers: Vec<[f64; 2]> = loops
        .iter()
        .map(|p| {
            [
                p.iter().map(|v| v[0]).sum::<f64>() / 6.,
                p.iter().map(|v| v[1]).sum::<f64>() / 6.,
            ]
        })
        .collect();
    let mut web = f64::INFINITY;
    for (i, a) in loops.iter().enumerate() {
        for (j, b) in loops.iter().enumerate().skip(i + 1) {
            if (centers[i][0] - centers[j][0]).hypot(centers[i][1] - centers[j][1]) > pitch * 1.1 {
                continue;
            }
            for k in 0..6 {
                for l in 0..6 {
                    web = web
                        .min(point_segment(a[k], b[l], b[(l + 1) % 6]))
                        .min(point_segment(b[l], a[k], a[(k + 1) % 6]));
                }
            }
        }
    }
    (web, border)
}
#[test]
fn public_binary_honeycomb_swapped_sides_are_quarter_turns() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for (length, width) in [(150., 300.), (45., 67.), (80., 51.)] {
        for kind in ["across_flats", "across_corners"] {
            // Explicit orientation is axis-relative: a quarter turn exchanges
            // pointy_top and flat_top. Omitted orientation needs no such hint.
            for orientation in [None, Some("pointy_top"), Some("flat_top")] {
                let flat = orientation.map_or(width > length, |o| o == "flat_top");
                let mut measurements = Vec::new();
                for swapped in [false, true] {
                    let (l, w) = if swapped {
                        (width, length)
                    } else {
                        (length, width)
                    };
                    let size = if kind == "across_flats" {
                        7.
                    } else {
                        14. / 3_f64.sqrt()
                    };
                    let mut args = json!({"length":l,"width":w,"thickness":3,"size":size,"size_kind":kind,"web":1.5,"border":4});
                    if let Some(o) = orientation {
                        args["orientation"] = json!(if swapped {
                            if o == "flat_top" {
                                "pointy_top"
                            } else {
                                "flat_top"
                            }
                        } else {
                            o
                        });
                    }
                    let plan = lib
                        .build(
                            &json!({"recipe":"honeycomb_pattern","args":args}),
                            &json!({"messages":[]}),
                            &cat,
                        )
                        .unwrap();
                    let mut session = Session::new(&path);
                    session.run(&plan);
                    let state = session.execute("document.inspect", json!({"measure":true}));
                    let bodies = state["bodies"].as_array().unwrap();
                    assert_eq!(bodies.len(), 1);
                    let body = &bodies[0];
                    let volume = body["volume_mm3"].as_f64().unwrap();
                    assert!(
                        (volume - plan.expected.as_ref().unwrap().created_volume_mm3.unwrap())
                            .abs()
                            < 0.01
                    );
                    for (axis, max) in [l, w, 3.].into_iter().enumerate() {
                        assert!(body["bbox"]["min"][axis].as_f64().unwrap().abs() < 1e-6);
                        assert!((body["bbox"]["max"][axis].as_f64().unwrap() - max).abs() < 1e-6);
                    }
                    let radius = 7. / 3_f64.sqrt();
                    let loops = honeycomb_loops(&mut session, "XY", 0., radius);
                    let count = loops.len();
                    assert_eq!(plan.facts.as_ref().unwrap()["hex_count"], count);
                    assert_eq!(body["faces"].as_u64().unwrap(), 6 + 6 * count as u64);
                    let mut points: Vec<[i64; 2]> = loops
                        .into_iter()
                        .flatten()
                        .map(|p| {
                            let p = if swapped {
                                p
                            } else if flat {
                                [width - p[1], p[0]]
                            } else {
                                [p[1], length - p[0]]
                            };
                            p.map(|n| (n * 100_000.).round() as i64)
                        })
                        .collect();
                    points.sort_unstable();
                    measurements.push((count, volume, points));
                }
                assert_eq!(measurements[0].0, measurements[1].0);
                assert!((measurements[0].1 - measurements[1].1).abs() < 0.01);
                assert_eq!(
                    measurements[0].2, measurements[1].2,
                    "{length}x{width} {kind} {orientation:?}"
                );
            }
        }
    }
}

#[test]
fn public_binary_honeycomb_recipe() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for (plane, flat, centred, fill) in [
        ("XY", false, false, true),
        ("XY", false, true, true),
        ("XY", true, true, true),
        ("XY", true, true, false),
        ("XZ", false, true, false),
        ("XZ", true, false, false),
        ("YZ", false, false, false),
        ("YZ", true, true, false),
    ] {
        let mut args = json!({"length":if fill{150}else{60},"width":if fill{300}else{50},"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4,"plane":plane,"orientation":if flat{"flat_top"}else{"pointy_top"},"centred":centred});
        if fill && centred && !flat {
            args["length"] = json!(300);
            args["width"] = json!(150);
        }
        if fill {
            args.as_object_mut().unwrap().remove("orientation");
        }
        if !fill {
            args["mode"] = json!("exact");
            args["rows"] = json!(3);
            args["columns"] = json!(4);
        }
        let p = lib
            .build(
                &json!({"recipe":"honeycomb_pattern","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        assert_eq!(
            p.calls[0]["arguments"]["params"]["corner"],
            json!([0., 0., 0.])
        );
        assert_eq!(
            p.facts.as_ref().unwrap()["hex_count"],
            if fill { 646 } else { 12 }
        );
        s.run(&p);
        let state = s.execute("document.inspect", json!({"measure":true}));
        let count = if fill { 646 } else { 12 };
        let length = args["length"].as_f64().unwrap();
        let width = args["width"].as_f64().unwrap();
        assert_eq!(state["bodies"].as_array().unwrap().len(), 1, "{state}");
        let volume = length * width * 3. - count as f64 * 3_f64.sqrt() / 2. * 49. * 3.;
        assert!(
            (state["bodies"][0]["volume_mm3"].as_f64().unwrap() - volume).abs() < 0.01,
            "{state}"
        );
        assert_eq!(state["bodies"][0]["faces"].as_u64().unwrap(), 6 + 6 * count);
        let measured = json!({"messages":[{"role":"tool","content":state.to_string()}]});
        assert_eq!(
            planner::completion(&measured, p.expected.as_ref().unwrap()),
            None
        );
        let loops = honeycomb_loops(&mut s, plane, 0., 7. / 3_f64.sqrt());
        assert_eq!(loops.len(), count as usize);
        let offset = if centred {
            [-length / 2., -width / 2.]
        } else {
            [0., 0.]
        };
        let (web, border) = measured_honeycomb_clearances(&loops, length, width, offset, 8.5);
        assert!((web - 1.5).abs() < 1e-6, "{plane} {flat}: web {web}");
        assert!(border >= 4. - 1e-6, "border {border}");
        if fill {
            assert!((border - 4.125).abs() < 1e-6);
            assert!((state["bodies"][0]["volume_mm3"].as_f64().unwrap() - 52760.5).abs() < 0.1);
        }
        eprintln!("Honeycomb {plane} flat={flat}: {count} cuts, volume {volume:.9}, measured web {web:.9}, border {border:.9}");
    }
}

#[test]
fn public_binary_honeycomb_given_corner() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for plane in ["XY", "XZ", "YZ"] {
        for orientation in ["pointy_top", "flat_top"] {
            let args = json!({"length":60,"width":50,"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4,"plane":plane,"orientation":orientation,"mode":"exact","rows":3,"columns":4,"corner_x":-37,"corner_y":19,"corner_z":-11});
            let p = lib
                .build(
                    &json!({"recipe":"honeycomb_pattern","args":args}),
                    &json!({"messages":[]}),
                    &cat,
                )
                .unwrap();
            assert_eq!(
                p.calls[0]["arguments"]["params"]["corner"],
                json!([0., 0., 0.])
            );
            assert_eq!(
                p.calls
                    .iter()
                    .filter(|c| c["arguments"]["command"] == "solid.move")
                    .count(),
                1
            );
            let mut s = Session::new(&path);
            s.run(&p);
            let state = s.execute("document.inspect", json!({"measure":true}));
            let measured = json!({"messages":[{"role":"tool","content":state.to_string()}]});
            assert_eq!(
                planner::completion(&measured, p.expected.as_ref().unwrap()),
                None,
                "{state}"
            );
            let dims = match plane {
                "XZ" => [60., 3., 50.],
                "YZ" => [3., 60., 50.],
                _ => [60., 50., 3.],
            };
            check_shortcut_body(
                &mut s,
                9000. - 12. * 3_f64.sqrt() / 2. * 49. * 3.,
                [-37., 19., -11.],
                std::array::from_fn(|i| [-37., 19., -11.][i] + dims[i]),
                false,
            );
        }
    }
}

#[test]
fn public_binary_honeycomb_single_axes_and_fill_clipping() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = Catalog::bundled().unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    for (rows, cols, length, width, count) in [
        (1, 1, 60., 50., 1),
        (1, 4, 60., 50., 4),
        (4, 1, 60., 50., 4),
        (0, 0, 33., 50., 13),
        (0, 0, 15., 50., 3),
    ] {
        let mut a = json!({"length":length,"width":width,"thickness":3,"size":14./3_f64.sqrt(),"size_kind":"across_corners","web":1.5,"border":4,"orientation":"pointy_top"});
        if rows > 0 {
            a["mode"] = json!("exact");
            a["rows"] = json!(rows);
            a["columns"] = json!(cols);
        }
        let p = lib
            .build(
                &json!({"recipe":"honeycomb_pattern","args":a}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        s.run(&p);
        let state = s.execute("document.inspect", json!({"measure":true}));
        assert_eq!(state["bodies"].as_array().unwrap().len(), 1);
        let volume = length * width * 3. - count as f64 * 3_f64.sqrt() / 2. * 49. * 3.;
        assert!(
            (state["bodies"][0]["volume_mm3"].as_f64().unwrap() - volume).abs() < 0.01,
            "{state}"
        );
        assert_eq!(
            honeycomb_loops(&mut s, "XY", 0., 7. / 3_f64.sqrt()).len(),
            count
        );
        assert!(p.calls.len() <= 12);
    }
}

#[test]
fn honeycomb_field_reference_matches_exact_recipe() {
    let tasks: Value =
        serde_json::from_str(include_str!("../bench/sets/field/tasks.json")).unwrap();
    let task = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "honeycomb-plate")
        .unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../bench/sets/field/references/honeycomb-plate.json"
    ))
    .unwrap();
    let req = json!({"messages":[{"role":"user","content":task["prompt"]}],"tools":[{"function":{"name":"execute"}}]});
    let p = planner::plan(&req, &Catalog::bundled().unwrap())
        .unwrap()
        .unwrap();
    let commands: Vec<_> = p.calls[..p.calls.len() - 1]
        .iter()
        .map(|c| c["arguments"].clone())
        .collect();
    // Compare both through the JSON wire decoder, including its decimal rounding.
    let wire: Value = serde_json::from_str(&serde_json::to_string(&commands).unwrap()).unwrap();
    assert_eq!(wire, reference["commands"]);
    assert_eq!(p.facts.as_ref().unwrap()["hex_count"], task["hex_count"]);
    assert!(
        (p.expected.as_ref().unwrap().created_volume_mm3.unwrap()
            - task["check"]["volume"].as_f64().unwrap())
        .abs()
            < 1e-8
    );
    assert_eq!(
        task["check"]["faces"].as_u64().unwrap(),
        6 + task["hex_count"].as_u64().unwrap() * 6
    );
    if let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") {
        let mut session = Session::new(&path);
        for command in reference["commands"].as_array().unwrap() {
            session.execute(
                command["command"].as_str().unwrap(),
                command["params"].clone(),
            );
        }
        let state = session.execute("document.inspect", json!({"measure":true}));
        assert_eq!(state["bodies"].as_array().unwrap().len(), 1);
        let body = &state["bodies"][0];
        assert_eq!(body["faces"], task["check"]["faces"]);
        assert!(
            (body["volume_mm3"].as_f64().unwrap() - task["check"]["volume"].as_f64().unwrap())
                .abs()
                < 0.01
        );
    }
}

#[test]
fn public_binary_probe_fill_plate_and_inner_lip() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (id, args, want) in [
        (
            "profile",
            json!({"depth":30,"flange_thickness":8,"height":40,"kind":"L","thickness":8,"web_thickness":8,"width":50}),
            19680.,
        ),
        (
            "plate",
            json!({"length":80,"width":80,"thickness":8,"centred":true,"bore_diameter":30,"bolt_hole_diameter":8,"bolt_hole_count":6,"bolt_circle_diameter":56,"corner_size":10}),
            42446.,
        ),
        (
            "plate",
            json!({"length":80,"width":80,"thickness":8,"centred":true,"bore_diameter":30,"bolt_hole_diameter":8,"bolt_hole_count":6,"bolt_circle_diameter":56,"corner_kind":"fillet","corner_size":10,"corner_edges":"vertical"}),
            42446.,
        ),
        (
            "enclosure",
            json!({"length":80,"width":50,"height":30,"wall":2,"open":"top","centred":false,"lip_width":1,"lip_height":2}),
            22608.,
        ),
    ] {
        let mut s = Session::new(&path);
        let req: Value = serde_json::from_str(if id == "profile" {
            include_str!("fixtures/fill-constraints-L-1.json")
        } else if id == "plate" {
            include_str!("fixtures/fill-v2-01-bolt-circle-bracket.json")
        } else {
            include_str!("fixtures/fill-v2-03-shell-lip.json")
        })
        .unwrap();
        let plan = lib
            .build(&json!({"recipe":id,"args":args}), &req, &cat)
            .unwrap();
        for c in &plan.calls {
            s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
        }
        let state = s.execute("document.inspect", json!({"measure":true}));
        let bodies = state["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), 1, "{state}");
        let volume = bodies[0]["volume_mm3"].as_f64().unwrap();
        eprintln!("probe {id}: {volume} mm3");
        assert!((volume - want).abs() < 0.5, "{id}: {volume} vs {want}");
        assert!(
            state["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["error"].is_null() || f["error"] == ""),
            "{state}"
        );
    }
}

#[test]
fn development_pattern_fills_replay_reference_measurements() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/dev_pattern_forms.json")).unwrap();
    let library = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    for fixture in fixtures.as_array().unwrap() {
        let mut session = Session::new(&path);
        for command in fixture["commands"].as_array().unwrap() {
            session.execute(
                command["command"].as_str().unwrap(),
                command["params"].clone(),
            );
        }
        let (id, args) = match fixture["id"].as_str().unwrap() {
            "dp-01" => (
                "linear_pattern",
                json!({"last_feature":true,"count":5,"spacing":12,"direction":[1,0,0]}),
            ),
            "dp-05" => (
                "circular_pattern",
                json!({"target":"the hole","count":6,"axis":"Z Axis","angle_deg":360}),
            ),
            "dp-09" => (
                "grid_pattern",
                json!({"target":"hole","columns":4,"rows":3,"spacing1":15,"spacing2":15,"plane":"XY"}),
            ),
            "dp-10" => (
                "grid_pattern",
                json!({"target":"boss","columns":4,"rows":4,"spacing1":20,"spacing2":20,"plane":"XY"}),
            ),
            _ => panic!("unknown development fixture"),
        };
        let mut request =
            json!({"messages":[{"role":"user","content":"Pattern the existing feature"}]});
        let mut built = false;
        for _ in 0..4 {
            let plan = library
                .build(&json!({"recipe":id,"args":args}), &request, &catalog)
                .unwrap();
            for call in &plan.calls {
                let result = session.execute(
                    call["arguments"]["command"].as_str().unwrap(),
                    call["arguments"]["params"].clone(),
                );
                request["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","content":result.to_string()}));
            }
            if plan.rule == id {
                built = true;
                break;
            }
        }
        assert!(built, "{request}");
        let state = session.execute("document.inspect", json!({"measure":true}));
        assert_eq!(
            state["bodies"].as_array().unwrap().len(),
            fixture["check"]["bodies"].as_u64().unwrap() as usize
        );
        let volume: f64 = state["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["volume_mm3"].as_f64().unwrap())
            .sum();
        let expected = fixture["check"]["volume"].as_f64().unwrap();
        assert!((volume - expected).abs() <= expected * 0.001, "{state}");
        let faces = session.execute("model.faces", json!({"body":state["bodies"][0]["name"]}));
        for round in fixture["check"]["round"].as_array().unwrap() {
            let radius = round["d"].as_f64().unwrap() / 2.;
            // The public kernel may split a round wall at its seam. Count
            // distinct cylinder axes, as the task's round checks do.
            let cylinders: std::collections::BTreeSet<[i64; 2]> = faces["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|face| {
                    face["cylinder"]["radius"]
                        .as_f64()
                        .is_some_and(|r| (r - radius).abs() < 0.001)
                })
                .map(|face| {
                    std::array::from_fn(|k| {
                        (face["cylinder"]["origin"][k].as_f64().unwrap() * 1000.).round() as i64
                    })
                })
                .collect();
            let count = cylinders.len();
            assert_eq!(count, round["count"].as_u64().unwrap() as usize, "{faces}");
        }
    }
}

#[test]
fn development_dp08_dp12_fills_after_part_creation() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let library = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    // Development dp-08 and dp-12. Existing-part forms apply only after these
    // public creation calls have established a measurable part and seed feature.
    for (id, args, circular, extrude) in [
        (
            "circular_pattern",
            json!({"target":"body","body":"post","count":6,"axis":"0, 0, 1","angle_deg":60}),
            true,
            false,
        ),
        (
            "grid_pattern",
            json!({"target":"boss","rows":3,"columns":3,"spacing1":16,"spacing2":16,"plane":"XY"}),
            false,
            false,
        ),
        (
            "grid_pattern",
            json!({"target":"boss","rows":3,"columns":3,"spacing1":16,"spacing2":16,"plane":"XY"}),
            false,
            true,
        ),
        (
            "circular_pattern",
            json!({"target":"boss","count":6,"axis":"[0, 0, 1]","angle_deg":360}),
            true,
            true,
        ),
    ] {
        let mut session = Session::new(&path);
        if !circular || extrude {
            session.execute("solid.box", json!({"length":if circular {100} else {50},"width":if circular {100} else {50},"height":4,"corner":if circular {json!([-50,-50,0])} else {json!([0,0,0])},"body_name":"Base"}));
        }
        if extrude {
            session.execute("sketch.create", json!({"plane":"XY","offset":4}));
            session.execute(
                "sketch.circle.center",
                json!({"center":if circular {json!([30,0])} else {json!([9,9])},"radius":4}),
            );
            session.execute("sketch.finish", json!({}));
            session.execute(
                "solid.extrude",
                json!({"distance":5,"operation":"join","targets":["Base"]}),
            );
        } else {
            session.execute("solid.cylinder", if circular {
                json!({"diameter":8,"height":40,"base":[30,0,0],"body_name":"Post"})
            } else {
                json!({"diameter":8,"height":5,"base":[9,9,4],"operation":"join","targets":["Base"]})
            });
        }
        let mut request = json!({"messages":[{"role":"user","content":if circular {"6 copies around Z"} else {"repeat the boss in a centred 3 by 3 grid at 16 mm spacing"}}]});
        let mut expected = None;
        for _ in 0..4 {
            let plan = library
                .build(&json!({"recipe":id,"args":args}), &request, &catalog)
                .unwrap();
            if plan.rule == id && extrude {
                assert_eq!(plan.slots["overlap_precheck"]["status"], "skipped");
            }
            for call in &plan.calls {
                let result = session.execute(
                    call["arguments"]["command"].as_str().unwrap(),
                    call["arguments"]["params"].clone(),
                );
                request["messages"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"role":"tool","content":result.to_string()}));
            }
            if plan.expected.is_some() {
                expected = plan.expected;
                break;
            }
        }
        assert_eq!(
            planner::completion(&request, expected.as_ref().unwrap()),
            None,
            "{request}"
        );
        let state = session.execute("document.inspect", json!({"measure":true}));
        let bodies = state["bodies"].as_array().unwrap();
        assert_eq!(bodies.len(), if circular && !extrude { 6 } else { 1 });
        let volume: f64 = bodies
            .iter()
            .map(|b| b["volume_mm3"].as_f64().unwrap())
            .sum();
        let want = if circular && !extrude {
            12061.71196211915
        } else if circular {
            40000. + 6. * std::f64::consts::PI * 16. * 5.
        } else {
            12261.570992899198
        };
        assert!((volume - want).abs() <= want * 0.001, "{state}");
        let axes: std::collections::BTreeSet<[i64; 2]> = bodies
            .iter()
            .flat_map(|body| {
                let faces = session.execute("model.faces", json!({"body":body["name"]}));
                faces["faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|face| {
                        face["cylinder"]["radius"]
                            .as_f64()
                            .is_some_and(|r| (r - 4.).abs() < 0.001)
                    })
                    .map(|face| {
                        std::array::from_fn(|k| {
                            (face["cylinder"]["origin"][k].as_f64().unwrap() * 1000.).round() as i64
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(axes.len(), if circular { 6 } else { 9 }, "{state}");
        if circular {
            assert!(axes
                .iter()
                .all(|p| ((p[0] as f64).hypot(p[1] as f64) / 1000. - 30.).abs() < 0.01));
        } else {
            assert_eq!(
                axes,
                [9, 25, 41]
                    .into_iter()
                    .flat_map(|x| [9, 25, 41].into_iter().map(move |y| [x * 1000, y * 1000]))
                    .collect()
            );
        }
    }
}

#[test]
fn public_binary_plate_base_and_pattern_forms() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    // Flat scalar fills, matching the development requests. Offsets are from base centre.
    for (args, volume, min, max) in [
        (
            json!({"length":60,"width":20,"thickness":5,"boss_diameter":6,"boss_height":10,"row_count":4,"row_pitch":15}),
            7130.7854964486805,
            [0., 0., 0.],
            [60., 20., 15.],
        ),
        (
            json!({"diameter":70,"thickness":5,"centred":true,"boss_diameter":5,"boss_height":8,"bolt_hole_count":8,"bolt_circle_diameter":50}),
            20498.448327338087,
            [-35., -35., 0.],
            [35., 35., 13.],
        ),
        (
            json!({"length":90,"width":90,"thickness":6,"boss_diameter":10,"boss_height":6,"boss_offset_x":-30,"boss_offset_y":-30}),
            49071.16062352067,
            [0., 0., 0.],
            [90., 90., 12.],
        ),
        (
            json!({"length":120,"width":80,"thickness":3,"grid_columns":7,"grid_rows":5,"grid_pitch_x":15,"grid_pitch_y":15,"grid_hole_diameter":6}),
            25831.688071818775,
            [0., 0., 0.],
            [120., 80., 3.],
        ),
        (
            json!({"length":50,"width":50,"thickness":4,"boss_diameter":8,"boss_height":5,"grid_columns":3,"grid_rows":3,"grid_pitch_x":16,"grid_pitch_y":16}),
            12261.570992899198,
            [0., 0., 0.],
            [50., 50., 9.],
        ),
        // A centred single boss, a Y row of holes, and a round plate with circular holes.
        (
            json!({"length":20,"width":20,"thickness":3,"boss_diameter":4,"boss_height":5}),
            1200. + 20. * std::f64::consts::PI,
            [0., 0., 0.],
            [20., 20., 8.],
        ),
        (
            json!({"length":20,"width":40,"thickness":3,"row_count":3,"row_pitch":10,"row_axis":"Y","hole_diameter":4}),
            2400. - 36. * std::f64::consts::PI,
            [0., 0., 0.],
            [20., 40., 3.],
        ),
        (
            json!({"diameter":40,"thickness":3,"bolt_hole_count":4,"bolt_circle_diameter":20,"bolt_hole_diameter":4}),
            1152. * std::f64::consts::PI,
            [0., 0., 0.],
            [40., 40., 3.],
        ),
    ] {
        let p = lib
            .build(
                &json!({"recipe":"plate","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        assert!(
            (p.expected.as_ref().unwrap().created_volume_mm3.unwrap() - volume).abs()
                < volume * 0.001
        );
        let mut s = Session::new(&path);
        s.run(&p);
        check_shortcut_body(&mut s, volume, min, max, false);
    }
}

#[test]
fn public_binary_plate_boss_planes_and_existing_body_isolation() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (plane, min, max) in [
        ("XY", [-15., -10., 0.], [15., 10., 8.]),
        ("XZ", [-15., 0., -10.], [15., 8., 10.]),
        ("YZ", [0., -15., -10.], [8., 15., 10.]),
    ] {
        let p = lib.build(&json!({"recipe":"plate","args":{"length":30,"width":20,"thickness":3,"centred":true,"plane":plane,"boss_diameter":4,"boss_height":5,"row_count":2,"row_pitch":10}}), &json!({"messages":[]}), &cat).unwrap();
        let mut s = Session::new(&path);
        s.run(&p);
        check_shortcut_body(&mut s, 1800. + 40. * std::f64::consts::PI, min, max, false);
    }
    let mut s = Session::new(&path);
    s.execute(
        "solid.box",
        json!({"length":40,"width":40,"height":20,"body_name":"PilotRecipe1"}),
    );
    let data = s.execute("document.inspect", json!({"measure":true}));
    let p = lib.build(&json!({"recipe":"plate","args":{"length":30,"width":20,"thickness":3,"boss_diameter":4,"boss_height":5}}), &json!({"messages":[{"role":"tool","content":data.to_string()}]}), &cat).unwrap();
    assert_eq!(
        p.calls[0]["arguments"]["params"]["body_name"],
        "PilotRecipe2"
    );
    assert_eq!(
        p.calls[1]["arguments"]["params"]["targets"],
        json!(["PilotRecipe2"])
    );
    s.run(&p);
    let measured = s.execute("document.inspect", json!({"measure":true}));
    let bodies = measured["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 2);
    for (name, want) in [
        ("PilotRecipe1", 32000.),
        ("PilotRecipe2", 1800. + 20. * std::f64::consts::PI),
    ] {
        let b = bodies.iter().find(|b| b["name"] == name).unwrap();
        assert!(
            (b["volume_mm3"].as_f64().unwrap() - want).abs() < want * 0.001,
            "{measured}"
        );
    }
}

#[test]
fn public_binary_joint_forms_replay_public_start_states() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let tasks: Value =
        serde_json::from_str(include_str!("../bench/sets/joints/tasks.json")).unwrap();
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    for (task, word) in tasks["tasks"].as_array().unwrap().iter().zip([
        "hinge",
        "prismatic",
        "Cylindrical joint",
        "pin slot",
        "ball and socket",
    ]) {
        let start = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("bench/sets/joints")
            .join(task["start"].as_str().unwrap());
        let mut s = Session::with_start(&path, Some(&start));
        let doc = s.execute("document.inspect", json!({"measure":true}));
        let components = s.execute("component.list", json!({}));
        let mut req = json!({"messages":[{"role":"user","content":task["prompt"]},{"role":"tool","content":doc.to_string()},{"role":"tool","content":components.to_string()}]});
        let mut args =
            json!({"joint_type":word,"component_a":"base","component_b":"the Link","axis":[0,0,1]});
        // Near names use the same punctuation/case/substring resolution as pattern forms.
        for (i, pair) in task["check"]["joints"][0]["limits"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            args[format!("limit{}_min", i + 1)] = pair[0].clone();
            args[format!("limit{}_max", i + 1)] = pair[1].clone();
        }
        let plan = lib
            .build(&json!({"recipe":"joint","args":args}), &req, &cat)
            .unwrap();
        for c in &plan.calls {
            let result = s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
            req["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":result.to_string()}));
        }
        let listed = s.execute("joint.list", json!({}));
        assert!(listed["errors"].as_array().unwrap().is_empty(), "{listed}");
        assert!(
            listed["conflicts"].as_array().unwrap().is_empty(),
            "{listed}"
        );
        let joints = listed["joints"].as_array().unwrap();
        assert_eq!(joints.len(), 1, "{listed}");
        let joint = &joints[0];
        assert_eq!(joint["type"], task["check"]["joints"][0]["type"]);
        assert_eq!(
            joint["a"],
            components["components"][1]["occurrences"][0]["id"]
        );
        assert_eq!(
            joint["b"],
            components["components"][2]["occurrences"][0]["id"]
        );
        let actual: Vec<_> = joint["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["limits"].clone())
            .collect();
        for (actual, expected) in actual
            .iter()
            .zip(task["check"]["joints"][0]["limits"].as_array().unwrap())
        {
            for end in 0..2 {
                let got = actual[end].as_f64().unwrap();
                let want = expected[end].as_f64().unwrap();
                assert!((got - want).abs() < 1e-8, "{word}: {actual} != {expected}");
            }
        }
        assert_eq!(
            actual.len(),
            task["check"]["joints"][0]["limits"]
                .as_array()
                .unwrap()
                .len()
        );
        assert_eq!(joint["frame_a"]["z"], json!([0., 0., 1.]));
        assert_eq!(joint["frame_b"]["z"], joint["frame_a"]["z"]);
        // The input picks are box centres, before the joint solver moves Link.
        assert_eq!(
            plan.calls[0]["arguments"]["params"]["a"]["point"],
            json!([5., 5., 5.])
        );
        assert_eq!(
            plan.calls[0]["arguments"]["params"]["b"]["point"],
            json!([35., 5., 5.])
        );
        let after = s.execute("document.inspect", json!({"measure":true}));
        assert_eq!(after["bodies"].as_array().unwrap().len(), 3);
        let volume: f64 = after["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["volume_mm3"].as_f64().unwrap())
            .sum();
        assert!((volume - task["check"]["volume"].as_f64().unwrap()).abs() < 0.0001);
        let after_components = s.execute("component.list", json!({}));
        for name in ["Base", "Link", "Spare"] {
            let c = after_components["components"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap();
            assert_eq!(c["bodies"], json!([name]));
        }
        let spare = &after["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == "Spare")
            .unwrap();
        assert_eq!(spare["bbox"], doc["bodies"][2]["bbox"]);
    }
}

#[test]
fn public_binary_remaining_joint_types_and_named_origins() {
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let lib = dutchman_pilot::recipe_library::Library::embedded().unwrap();
    let cat = Catalog::bundled().unwrap();
    let start = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench/sets/joints/start/joints-revolute-limits.solidcraft");
    for (kind, units) in [
        ("rigid", vec![]),
        ("planar", vec!["mm", "mm", "deg"]),
        ("parallel", vec!["mm", "mm", "mm", "deg"]),
        ("perpendicular", vec!["mm", "mm", "mm", "deg", "deg"]),
        ("angle", vec!["mm", "mm", "mm", "deg", "deg"]),
        ("point_on_plane", vec!["mm", "mm", "deg", "deg", "deg"]),
        ("point_on_line", vec!["mm", "deg", "deg", "deg"]),
        ("line_on_plane", vec!["mm", "mm", "deg", "deg"]),
    ] {
        let mut s = Session::with_start(&path, Some(&start));
        for (name, occurrence, point) in [
            ("BaseAnchor", "Base:1", [5, 5, 5]),
            ("LinkAnchor", "Link:1", [35, 5, 5]),
        ] {
            s.execute(
                "joint.origin",
                json!({"name":name,"occurrence":occurrence,"point":point,"z":[1,0,0]}),
            );
        }
        let doc = s.execute("document.inspect", json!({"measure":true}));
        let components = s.execute("component.list", json!({}));
        let origins = s.execute("joint.list", json!({}));
        let req = json!({"messages":[{"role":"user","content":"connect components"},{"role":"tool","content":doc.to_string()},{"role":"tool","content":components.to_string()},{"role":"tool","content":origins.to_string()}]});
        let mut args = json!({"joint_type":kind,"component_a":"Base","component_b":"Link","point_a":"BaseAnchor","point_b":"LinkAnchor","axis":"X axis"});
        for i in 1..=units.len() {
            args[format!("limit{i}_min")] = json!(-100);
            args[format!("limit{i}_max")] = json!(100);
        }
        let plan = lib
            .build(&json!({"recipe":"joint","args":args}), &req, &cat)
            .unwrap();
        assert_eq!(plan.calls[0]["arguments"]["params"]["a"], "BaseAnchor");
        for c in &plan.calls {
            s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
        }
        let listed = s.execute("joint.list", json!({}));
        let values = listed["joints"][0]["values"].as_array().unwrap();
        assert_eq!(values.len(), units.len(), "{kind}: {listed}");
        for (v, unit) in values.iter().zip(&units) {
            assert_eq!(v["unit"], *unit);
            assert_eq!(v["limits"][0].as_f64(), Some(-100.));
            assert_eq!(v["limits"][1].as_f64(), Some(100.));
        }
    }
}

#[test]
fn development_feature_forms_replay_reference_states() {
    replay_development_feature_forms(None, false);
}

#[test]
fn development_plural_hole_fields_replay() {
    replay_development_feature_forms(Some("df-02"), true);
}

#[test]
fn development_existing_plate_chamfer_preserves_twelve_holes() {
    replay_development_feature_forms(Some("df-04"), true);
}

#[test]
fn development_bottom_open_outer_lip_replay() {
    replay_development_feature_forms(Some("df-11"), true);
}

#[test]
fn development_matching_shell_dimensions_replay() {
    replay_development_feature_forms(Some("df-12"), true);
}

#[test]
fn development_omitted_joint_angle_replay() {
    replay_development_feature_forms(Some("df-18"), true);
}

fn replay_development_feature_forms(only: Option<&str>, model_variant: bool) {
    use dutchman_pilot::recipe_library::{forms, Library};
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/dev_feature_forms.json")).unwrap();
    let lib = Library::embedded().unwrap();
    let catalog = Catalog::bundled().unwrap();
    for fixture in fixtures {
        if only.is_some_and(|id| fixture["id"] != id) {
            continue;
        }
        let mut reference = Session::new(&path);
        for c in fixture["commands"].as_array().unwrap() {
            reference.execute(c["command"].as_str().unwrap(), c["params"].clone());
        }
        let want = reference.execute("document.inspect", json!({"measure":true}));
        let (id, mut args) = match fixture["id"].as_str().unwrap() {
            "df-02" => (
                "plate",
                json!({"length":"9 cm","width":"6 cm","thickness":"0.4 cm","centered":true,"grid_columns":3,"grid_rows":2,"grid_pitch_x":"2 cm","grid_pitch_y":"3 cm","grid_hole_diameter":"0.5 cm","corner_kind":"chamfer","corner_size":"0.3 cm","corner_edges":"vertical"}),
            ),
            "df-03" => (
                "plate",
                json!({"length":"4 in","width":"4 in","thickness":"1/4 in","bolt_hole_count":8,"bolt_circle_diameter":"2 1/2 in","bolt_hole_diameter":"1/4 in","corner_size":"3/8 in","corner_edges":"top_rightbottom_right"}),
            ),
            "df-04" => (
                "chamfer",
                json!({"body":"Plate","amount":4,"corner_sides":"right"}),
            ),
            "df-05" => (
                "plate",
                json!({"length":76.2,"width":50.8,"thickness":3.175,"centered":true,"grid_columns":3,"grid_rows":2,"grid_pitch_x":19.05,"grid_pitch_y":25.4,"grid_hole_diameter":4.7625,"corner_size":6.35,"corner_edges":true}),
            ),
            "df-06" => (
                "plate",
                json!({"length":"10cm","width":"8cm","thickness":"0.6cm","bolt_hole_count":4,"bolt_circle_diameter":"5cm","bolt_hole_diameter":"0.6cm","corner_kind":"bevel","corner_size":"0.5cm","corner_sides":"right side"}),
            ),
            "df-09" => (
                "enclosure",
                json!({"length":80,"width":60,"height":30,"wall":3,"centered":true,"open":true,"lip_width":1,"lip_height":2,"lip_side":"inner"}),
            ),
            "df-10" => (
                "enclosure",
                json!({"length":"3 in","width":"2 in","height":"1 in","wall":"1/8 in","floor":"1/4 in","inside":true,"open":true}),
            ),
            "df-11" => (
                "enclosure",
                json!({"length":70,"width":46,"height":24,"wall":3,"floor":5,"open":"bottom","lip_width":1,"lip_height":2,"lip_side":"outer"}),
            ),
            "df-12" => ("shell", json!({"body":"Case","wall":2,"open":"bottom"})),
            "df-13" => (
                "enclosure",
                json!({"length":"6 cm","width":"4 cm","height":"2 cm","wall":"0.25 cm","floor":"0.4 cm","inside":true,"centered":false,"open":true,"lip_width":"0.1 cm","lip_height":"0.15 cm","lip_side":"outer"}),
            ),
            "df-15" => (
                "revolute_joint",
                json!({"pin_component":"Pedestal","hole_component":"Lever","angle_deg":30}),
            ),
            "df-16" => (
                "revolute_joint",
                json!({"pin_component":"Pedestal","hole_component":"Lever","angle_deg":60}),
            ),
            "df-18" => (
                "revolute_joint",
                json!({"pin_component":"Pedestal","hole_component":"Lever","angle_deg":"90°"}),
            ),
            _ => panic!("unknown development feature fixture"),
        };
        let mut session = Session::new(&path);
        if model_variant {
            match fixture["id"].as_str().unwrap() {
                "df-02" => {
                    let diameter = args
                        .as_object_mut()
                        .unwrap()
                        .remove("grid_hole_diameter")
                        .unwrap();
                    args["grid_holes_diameter"] = diameter;
                }
                "df-12" => {
                    args["length"] = json!(64.0001);
                    args["width"] = json!(48);
                    args["height"] = json!(26);
                }
                "df-18" => {
                    args.as_object_mut().unwrap().remove("angle_deg");
                }
                _ => {}
            }
        }
        let commands = fixture["commands"].as_array().unwrap();
        let setup = if fixture["id"] == "df-04" {
            // Hole creation is a separate operation in this multi-intent edit.
            // Replay that public prefix, then fill the corner edit on this body.
            commands
                .iter()
                .position(|c| c["command"] == "solid.chamfer")
                .unwrap()
        } else {
            fixture["setup_steps"].as_u64().unwrap() as usize
        };
        for c in &commands[..setup] {
            session.execute(c["command"].as_str().unwrap(), c["params"].clone());
        }
        let mut request = json!({"messages":[{"role":"user","content":fixture["prompt"]}]});
        let before = session.execute("document.inspect", json!({"measure":true}));
        // Joint forms obtain the full document/component/joint inspection together.
        // A lone document result is not a complete joint inspection context.
        if id != "revolute_joint" {
            request["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","content":before.to_string()}));
        }
        if matches!(id, "chamfer" | "shell") {
            let bodies: Vec<_> = before["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b["name"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(
                lib.form_band_in_design(fixture["prompt"].as_str().unwrap(), &bodies)
                    .unwrap()["recipes"][0]["id"],
                id
            );
        }
        let schema = &lib.recipes.iter().find(|r| r.id == id).unwrap().schema;
        forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        let inferred = forms::request_defaults(&mut args, schema, &request).unwrap();
        if model_variant && fixture["id"] == "df-18" {
            assert_eq!(args["angle_deg"], 90.);
            assert_eq!(inferred.len(), 1);
            assert_eq!(inferred[0]["defaulted"], true);
        }
        if fixture["id"] == "df-04" {
            assert_eq!(before["bodies"][0]["face_types"]["cylinder"], 12);
        }
        if model_variant && fixture["id"] == "df-12" {
            let mut disagreeing = args.clone();
            disagreeing["length"] = json!(65);
            let error = lib
                .build(&json!({"recipe":id,"args":disagreeing}), &request, &catalog)
                .unwrap_err();
            assert!(error.contains("64 x 48 x 26 mm"), "{error}");
        }
        let mut previous: Option<planner::Plan> = None;
        let mut built = false;
        for stage in 0..4 {
            let plan = if let Some(p) = previous.as_ref().filter(|p| p.continuation.is_some()) {
                planner::resume(&request, p, &catalog).unwrap().unwrap()
            } else {
                lib.build(&json!({"recipe":id,"args":args}), &request, &catalog)
                    .unwrap_or_else(|e| panic!("{}: {e}", fixture["id"]))
            };
            let calls: Vec<_> = plan.calls.iter().enumerate().map(|(i,c)| json!({"id":format!("feature_{stage}_{i}"),"function":{"name":"execute","arguments":c["arguments"].to_string()}})).collect();
            if fixture["id"] == "df-04" {
                assert!(plan.calls.iter().all(|c| matches!(
                    c["arguments"]["command"].as_str(),
                    Some("model.edges" | "solid.chamfer" | "document.inspect")
                )));
                for c in plan
                    .calls
                    .iter()
                    .filter(|c| c["arguments"]["command"] == "solid.chamfer")
                {
                    assert_eq!(c["arguments"]["params"]["body"], "Plate");
                    assert_eq!(
                        c["arguments"]["params"]["edges"].as_array().unwrap().len(),
                        2
                    );
                    assert!(c["arguments"]["params"]["edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|e| e["body"] == "Plate" && e["index"].is_u64()));
                }
            }
            request["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"assistant","tool_calls":calls}));
            for (i, c) in plan.calls.iter().enumerate() {
                let result = session.execute(
                    c["arguments"]["command"].as_str().unwrap(),
                    c["arguments"]["params"].clone(),
                );
                request["messages"].as_array_mut().unwrap().push(json!({"role":"tool","tool_call_id":format!("feature_{stage}_{i}"),"content":result.to_string()}));
            }
            let mutation = plan.calls.iter().any(|c| {
                let command = c["arguments"]["command"].as_str().unwrap();
                command.starts_with("solid.") || command == "joint.create"
            });
            if mutation && plan.continuation.is_none() {
                if id == "revolute_joint" {
                    assert!(
                        plan.calls
                            .iter()
                            .any(|c| c["arguments"]["command"] == "joint.drive"
                                && c["arguments"]["params"]["value"].as_f64()
                                    == args["angle_deg"].as_f64()),
                        "{}: missing requested joint drive",
                        fixture["id"]
                    );
                    assert_eq!(
                        planner::completion(&request, plan.expected.as_ref().unwrap()),
                        None
                    );
                }
                built = true;
                break;
            }
            previous = Some(plan);
        }
        assert!(built, "{}", fixture["id"]);
        let actual = session.execute("document.inspect", json!({"measure":true}));
        let a = actual["bodies"].as_array().unwrap();
        if fixture["id"] == "df-04" {
            assert_eq!(a[0]["face_types"]["cylinder"], 12);
            assert!((a[0]["volume_mm3"].as_f64().unwrap() - 31806.143002367506).abs() < 0.01);
        }
        let b = want["bodies"].as_array().unwrap();
        assert_eq!(a.len(), b.len(), "{}", fixture["id"]);
        for (a, b) in a.iter().zip(b) {
            let v = b["volume_mm3"].as_f64().unwrap();
            assert!(
                (a["volume_mm3"].as_f64().unwrap() - v).abs() < v * 0.001,
                "{}: {actual}",
                fixture["id"]
            );
            for key in ["min", "max"] {
                for i in 0..3 {
                    assert!(
                        (a["bbox"][key][i].as_f64().unwrap() - b["bbox"][key][i].as_f64().unwrap())
                            .abs()
                            < 0.001,
                        "{}: {actual}",
                        fixture["id"]
                    );
                }
            }
        }
        if id != "revolute_joint" {
            let actual_edges = session.execute("model.edges", json!({"body":a[0]["name"]}));
            let reference_edges = reference.execute("model.edges", json!({"body":b[0]["name"]}));
            let samples = |v: &Value| -> Vec<[i64; 4]> {
                let mut out: Vec<_> = v["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| {
                        [
                            e["mid"][0].as_f64().unwrap(),
                            e["mid"][1].as_f64().unwrap(),
                            e["mid"][2].as_f64().unwrap(),
                            e["length"].as_f64().unwrap(),
                        ]
                        .map(|n| (n * 1000.).round() as i64)
                    })
                    .collect();
                out.sort();
                out
            };
            assert_eq!(
                samples(&actual_edges),
                samples(&reference_edges),
                "{}",
                fixture["id"]
            );
        }
        assert!(
            actual["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["error"].is_null()),
            "{actual}"
        );
        if id == "revolute_joint" {
            let actual = session.execute("joint.list", json!({}));
            let want = reference.execute("joint.list", json!({}));
            assert_eq!(
                actual["joints"][0]["values"][0]["value"],
                want["joints"][0]["values"][0]["value"]
            );
            let actual = session.execute("component.list", json!({}));
            let want = reference.execute("component.list", json!({}));
            let transforms = |v: &Value| -> Vec<Value> {
                v["components"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|c| c["occurrences"].as_array().into_iter().flatten())
                    .map(|o| o["transform"].clone())
                    .collect()
            };
            let actual = transforms(&actual);
            let want = transforms(&want);
            assert_eq!(actual.len(), want.len());
            for (a, b) in actual.iter().zip(&want) {
                for i in 0..4 {
                    for j in 0..4 {
                        assert!(
                            (a[i][j].as_f64().unwrap() - b[i][j].as_f64().unwrap()).abs() < 1e-6
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn public_binary_wall_derived_lip_defaults() {
    use dutchman_pilot::recipe_library::{forms, Library};
    let Some(path) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
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
    for (request, defaults, width, height) in [
        (json!({"lip":true}), 2, 1., 2.),
        (json!({"lip_width":0.5}), 1, 0.5, 2.),
        (json!({"lip":{"side":"outer"}}), 2, 1., 2.),
    ] {
        let mut args = json!({"length":40,"width":30,"height":20,"wall":2,"open":true});
        args.as_object_mut()
            .unwrap()
            .extend(request.as_object().unwrap().clone());
        let changes = forms::coerce(&mut args, &forms::flat(schema)).unwrap();
        assert_eq!(
            changes.iter().filter(|c| c["defaulted"] == true).count(),
            defaults
        );
        let plan = lib
            .build(
                &json!({"recipe":"enclosure","args":args}),
                &json!({"messages":[]}),
                &cat,
            )
            .unwrap();
        let mut s = Session::new(&path);
        for c in &plan.calls {
            s.execute(
                c["arguments"]["command"].as_str().unwrap(),
                c["arguments"]["params"].clone(),
            );
        }
        let inset = if args["lip"]["side"] == "outer" {
            0.
        } else {
            2. - width
        };
        let ring = ((40. - 2. * inset) * (30. - 2. * inset)
            - (40. - 2. * (inset + width)) * (30. - 2. * (inset + width)))
            * height;
        check_shortcut_body(
            &mut s,
            24000. - 36. * 26. * 18. + ring,
            [0., 0., 0.],
            [40., 30., 20. + height],
            false,
        );
    }
}
