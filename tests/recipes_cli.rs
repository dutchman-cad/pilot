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
