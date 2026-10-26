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
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Session {
    fn new(path: &std::ffi::OsStr) -> Self {
        let mut child = Command::new(path)
            .arg("mcp")
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
        ("XY", true, true, false),
        ("XZ", false, true, false),
        ("XZ", true, false, false),
        ("YZ", false, false, false),
        ("YZ", true, true, false),
    ] {
        let mut args = json!({"length":if fill{150}else{60},"width":if fill{300}else{50},"thickness":3,"size":7,"size_kind":"across_flats","web":1.5,"border":4,"plane":plane,"orientation":if flat{"flat_top"}else{"pointy_top"},"centred":centred});
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
        }
        eprintln!("Honeycomb {plane} flat={flat}: {count} cuts, volume {volume:.9}, measured web {web:.9}, border {border:.9}");
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
