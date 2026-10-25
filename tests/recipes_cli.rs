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
        assert!(line.len() < 4 * 1024 * 1024);
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
