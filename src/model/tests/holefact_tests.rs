// Development cases for the hole and circle-cut facts. Cases that need SolidCraft run
// against its public CLI (PILOT_SOLIDCRAFT_CLI) with real tool results; the rest use
// synthetic conversations. Nothing here is a benchmark task.
use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Cli {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: usize,
}
impl Cli {
    fn new() -> Option<Self> {
        let path = std::env::var_os("PILOT_SOLIDCRAFT_CLI")?;
        let mut child = Command::new(path)
            .args(["mcp", "--headless"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Some(Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            id: 0,
        })
    }
    /// The raw result text and whether SolidCraft flagged it as an error.
    fn tool(&mut self, name: &str, arguments: &Value) -> (String, bool) {
        self.id += 1;
        writeln!(self.input,"{}",json!({"jsonrpc":"2.0","id":self.id,"method":"tools/call","params":{"name":name,"arguments":arguments}})).unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        (
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            response["result"]["isError"] == true,
        )
    }
}
impl Drop for Cli {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A growing model-face conversation backed by a live SolidCraft.
struct Run {
    cli: Cli,
    messages: Vec<Value>,
    n: usize,
}
impl Run {
    fn new() -> Option<Self> {
        Some(Self {
            cli: Cli::new()?,
            messages: vec![json!({"role":"user","content":"make a block and drill it"})],
            n: 0,
        })
    }
    /// Run one command as the model would (execute) and return Pilot's added facts.
    fn exec(&mut self, command: &str, params: Value) -> Vec<String> {
        let args = json!({"command":command,"params":params});
        self.call("execute", args)
    }
    fn call(&mut self, tool: &str, args: Value) -> Vec<String> {
        let (text, _) = self.cli.tool(tool, &args);
        self.push(tool, args, text)
    }
    fn push(&mut self, tool: &str, args: Value, text: String) -> Vec<String> {
        self.n += 1;
        let id = format!("c{}", self.n);
        self.messages.push(json!({"role":"assistant","tool_calls":[{"id":id,"type":"function","function":{"name":tool,"arguments":args.to_string()}}]}));
        self.messages
            .push(json!({"role":"tool","tool_call_id":id,"content":text}));
        facts_of(&self.messages)
    }
    fn volume(&mut self) -> f64 {
        let (text, _) = self.cli.tool(
            "execute",
            &json!({"command":"document.inspect","params":{"measure":true}}),
        );
        let v: Value = serde_json::from_str(&text).unwrap();
        v["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["volume_mm3"].as_f64().unwrap())
            .sum()
    }
    fn block(&mut self) {
        self.exec("solid.box", json!({"length":40,"width":30,"height":12}));
    }
    /// A circle cut. Returns the extrude's facts.
    fn cut(
        &mut self,
        plane: &str,
        offset: f64,
        centre: [f64; 2],
        size: (&str, f64),
        extra: Value,
    ) -> Vec<String> {
        self.exec("sketch.create", json!({"plane":plane,"offset":offset}));
        let mut circle = json!({"center":centre});
        circle[size.0] = json!(size.1);
        self.exec("sketch.circle.center", circle);
        self.exec("sketch.finish", json!({}));
        let mut p = json!({"operation":"cut"});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        self.exec("solid.extrude", p)
    }
}

fn facts_of(messages: &[Value]) -> Vec<String> {
    let f = Fixture::new(vec![]);
    let mut body = json!({"messages":messages});
    let mut audit = f.audit();
    holefact::append(&mut body, &mut audit).unwrap();
    let last = body["messages"].as_array().unwrap().last().unwrap();
    let original = messages.last().unwrap()["content"].as_str().unwrap();
    let now = last["content"].as_str().unwrap();
    now.strip_prefix(original)
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix(crate::identity::HINT))
        .map(|l| l.trim().to_string())
        .collect()
}

struct Case {
    name: &'static str,
    run: fn(&mut Run) -> Vec<String>,
    has: &'static [&'static str],
}

fn hole(r: &mut Run, p: Value) -> Vec<String> {
    r.block();
    r.exec("solid.hole", p)
}

#[test]
fn twenty_four_development_cases_state_what_the_public_cli_made() {
    let cases: Vec<Case> = vec![
        Case { name: "through all from the top", run: |r| hole(r, json!({"position":[20,15,12],"diameter":8})),
            has: &["Hole made: 8 mm diameter, through all along -Z from (20, 15, 12); it goes through Body1 (12 mm thick)."] },
        Case { name: "blind 10 of 12", run: |r| hole(r, json!({"position":[20,15,12],"diameter":8,"depth":10})),
            has: &["Hole made: 8 mm diameter, 10 mm deep along -Z from (20, 15, 12); it does not go through Body1 (12 mm thick)."] },
        Case { name: "depth equal to the thickness", run: |r| hole(r, json!({"position":[20,15,12],"diameter":5,"depth":12})),
            has: &["12 mm deep along -Z", "goes through Body1"] },
        Case { name: "depth past the body", run: |r| hole(r, json!({"position":[20,15,12],"diameter":5,"depth":50})),
            has: &["50 mm deep along -Z", "goes through Body1 (12 mm thick)"] },
        Case { name: "from the bottom face", run: |r| hole(r, json!({"position":[20,15,0],"diameter":6,"depth":5})),
            has: &["along +Z from (20, 15, 0)", "does not go through"] },
        Case { name: "side face at x = 40", run: |r| hole(r, json!({"position":[40,15,6],"diameter":6,"depth":10})),
            has: &["along -X from (40, 15, 6)", "(40 mm thick)"] },
        Case { name: "side face at y = 0", run: |r| hole(r, json!({"position":[20,0,6],"diameter":4})),
            has: &["along +Y from (20, 0, 6)", "(30 mm thick)"] },
        Case { name: "hole wider than the wall beside it", run: |r| hole(r, json!({"position":[2,15,12],"diameter":8,"depth":5})),
            has: &["the hole reaches past the edge of Body1 (its centre is 2 mm from the edge, its radius is 4 mm)"] },
        Case { name: "radius typed as diameter shows the diameter", run: |r| hole(r, json!({"position":[20,15,12],"diameter":4,"depth":6})),
            has: &["Hole made: 4 mm diameter, 6 mm deep"] },
        Case { name: "box placed by centre", run: |r| {
                r.exec("solid.box", json!({"length":20,"width":20,"height":10,"center":[0,0,0]}));
                r.exec("solid.hole", json!({"position":[0,0,5],"diameter":6}))
            }, has: &["through all along -Z from (0, 0, 5)", "(10 mm thick)"] },
        Case { name: "box placed by corner", run: |r| {
                r.exec("solid.box", json!({"length":20,"width":20,"height":10,"corner":[10,10,0]}));
                r.exec("solid.hole", json!({"position":[20,20,10],"diameter":6,"depth":3}))
            }, has: &["3 mm deep along -Z from (20, 20, 10)", "does not go through"] },
        Case { name: "second hole in the same block", run: |r| {
                hole(r, json!({"position":[10,15,12],"diameter":4,"depth":4}));
                r.exec("solid.hole", json!({"position":[30,15,12],"diameter":4,"depth":20}))
            }, has: &["20 mm deep along -Z", "goes through Body1 (12 mm thick)"] },
        Case { name: "hole in an extruded plate after an inspect", run: |r| {
                r.exec("sketch.create", json!({"plane":"XY"}));
                r.exec("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[30,20]}));
                r.exec("sketch.finish", json!({}));
                r.exec("solid.extrude", json!({"distance":6}));
                r.exec("document.inspect", json!({"measure":true}));
                r.exec("solid.hole", json!({"position":[15,10,6],"diameter":5,"depth":2}))
            }, has: &["2 mm deep along -Z from (15, 10, 6)", "does not go through Body1 (6 mm thick)"] },
        Case { name: "round body: direction but no thickness claim", run: |r| {
                r.exec("sketch.create", json!({"plane":"XY"}));
                r.exec("sketch.circle.center", json!({"center":[0,0],"diameter":30}));
                r.exec("sketch.finish", json!({}));
                r.exec("solid.extrude", json!({"distance":8}));
                r.exec("document.inspect", json!({"measure":true}));
                r.exec("solid.hole", json!({"position":[0,0,8],"diameter":4,"depth":3}))
            }, has: &["Hole made: 4 mm diameter, 3 mm deep along -Z from (0, 0, 8)."] },
        Case { name: "unmeasured body: only what was asked", run: |r| {
                r.exec("sketch.create", json!({"plane":"XY"}));
                r.exec("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[30,20]}));
                r.exec("sketch.finish", json!({}));
                r.exec("solid.extrude", json!({"distance":6}));
                r.exec("solid.hole", json!({"position":[15,10,6],"diameter":5,"depth":2}))
            }, has: &["Hole made: 5 mm diameter, 2 mm deep at (15, 10, 6)."] },
        Case { name: "cut: blind from the top, downwards", run: |r| { r.block(); r.cut("XY", 12.0, [20.0, 15.0], ("diameter", 8.0), json!({"distance":5,"direction":"negative"})) },
            has: &["Cut made: 8 mm diameter circle at (20, 15, 12), 5 mm deep along -Z; it does not go through Body1 (12 mm thick)."] },
        Case { name: "cut: extruded away from the body", run: |r| { r.block(); r.cut("XY", 12.0, [20.0, 15.0], ("diameter", 8.0), json!({"distance":5})) },
            has: &["This cut removed no material: the cut covers Z 12..17, which does not reach Body1 (Z 0..12); the other direction (negative) would reach it."] },
        Case { name: "cut: circle beside the body", run: |r| { r.block(); r.cut("XY", 12.0, [80.0, 15.0], ("diameter", 8.0), json!({"distance":5,"direction":"negative"})) },
            has: &["This cut removed no material: the circle at (80, 15, 12) is outside Body1 (X 0..40, Y 0..30)."] },
        Case { name: "cut: through all, downwards", run: |r| { r.block(); r.cut("XY", 12.0, [20.0, 15.0], ("diameter", 6.0), json!({"through_all":true,"direction":"negative"})) },
            has: &["through all along -Z", "goes through Body1 (12 mm thick)"] },
        Case { name: "cut: symmetric across the body", run: |r| { r.block(); r.cut("XY", 6.0, [20.0, 15.0], ("diameter", 6.0), json!({"distance":50,"direction":"symmetric"})) },
            has: &["both ways along Z", "goes through Body1"] },
        Case { name: "cut: radius typed where a diameter belongs", run: |r| { r.block(); r.cut("XY", 0.0, [20.0, 15.0], ("radius", 4.0), json!({"distance":5})) },
            has: &["Cut made: 8 mm diameter circle at (20, 15, 0), 5 mm deep along +Z"] },
        Case { name: "cut: XZ plane puts sketch y on -Z", run: |r| { r.block(); r.cut("XZ", 0.0, [20.0, 6.0], ("diameter", 8.0), json!({"distance":5})) },
            has: &["This cut removed no material: the circle at (20, 0, -6) is outside Body1 (X 0..40, Z 0..12)."] },
        Case { name: "cut: XZ plane hits with negative sketch y", run: |r| { r.block(); r.cut("XZ", 0.0, [20.0, -6.0], ("diameter", 8.0), json!({"distance":30})) },
            has: &["Cut made: 8 mm diameter circle at (20, 0, 6), 30 mm deep along +Y", "goes through Body1 (30 mm thick)"] },
        Case { name: "cut: YZ plane on the far face, default direction", run: |r| { r.block(); r.cut("YZ", 40.0, [15.0, 6.0], ("diameter", 8.0), json!({"distance":5})) },
            has: &["This cut removed no material", "X 40..45", "(negative) would reach it"] },
        Case { name: "cut: YZ plane on the far face, negative", run: |r| { r.block(); r.cut("YZ", 40.0, [15.0, 6.0], ("diameter", 8.0), json!({"distance":5,"direction":"negative"})) },
            has: &["5 mm deep along -X;", "does not go through Body1 (40 mm thick)"] },
    ];
    let mut done = 0;
    for case in cases {
        let Some(mut run) = Run::new() else { return };
        let facts = (case.run)(&mut run);
        let text = facts.join(" ");
        for want in case.has {
            assert!(
                text.contains(want),
                "{}: {want:?} not in {text:?}",
                case.name
            );
        }
        assert!(
            facts.len() <= 1,
            "{}: one fact per command: {facts:?}",
            case.name
        );
        done += 1;
    }
    assert!(done >= 24);
}

#[test]
fn facts_agree_with_measured_volumes() {
    let Some(mut r) = Run::new() else { return };
    r.block();
    let v0 = r.volume();
    let facts = r.exec(
        "solid.hole",
        json!({"position":[20,15,12],"diameter":8,"depth":10}),
    );
    assert!(facts[0].contains("10 mm deep"), "{facts:?}");
    let removed = v0 - r.volume();
    let want = std::f64::consts::PI * 16.0 * 10.0;
    assert!((removed - want).abs() < want * 0.01, "{removed} vs {want}");
    // A through hole removes diameter-circle times thickness, as stated.
    let v1 = r.volume();
    let facts = r.exec("solid.hole", json!({"position":[5,5,12],"diameter":4}));
    assert!(
        facts[0].contains("goes through Body1 (12 mm thick)"),
        "{facts:?}"
    );
    let removed = v1 - r.volume();
    let want = std::f64::consts::PI * 4.0 * 12.0;
    assert!((removed - want).abs() < want * 0.01, "{removed} vs {want}");
    // Every cut claim of "removed no material" matches an unchanged volume.
    let Some(mut r) = Run::new() else { return };
    r.block();
    for (plane, off, c, extra, nothing) in [
        ("XY", 12.0, [20.0, 15.0], json!({"distance":5}), true),
        (
            "XY",
            12.0,
            [20.0, 15.0],
            json!({"distance":5,"direction":"negative"}),
            false,
        ),
        (
            "XY",
            12.0,
            [90.0, 15.0],
            json!({"distance":5,"direction":"negative"}),
            true,
        ),
        ("XZ", 0.0, [20.0, 6.0], json!({"distance":5}), true),
        ("XZ", 0.0, [20.0, -6.0], json!({"distance":5}), false),
        ("YZ", 40.0, [15.0, 6.0], json!({"distance":5}), true),
        (
            "YZ",
            0.0,
            [15.0, 6.0],
            json!({"distance":5,"direction":"negative"}),
            true,
        ),
        (
            "XY",
            6.0,
            [30.0, 25.0],
            json!({"distance":2,"direction":"symmetric"}),
            false,
        ),
    ] {
        let before = r.volume();
        r.messages
            .push(json!({"role":"user","content":"and another cut"}));
        let facts = r.cut(plane, off, c, ("diameter", 4.0), extra.clone());
        let same = (before - r.volume()).abs() < 1e-6;
        assert_eq!(
            facts.iter().any(|f| f.contains("removed no material")),
            nothing,
            "{plane} {off} {c:?} {extra}: {facts:?}"
        );
        assert_eq!(same, nothing, "{plane} {off} {c:?} {extra}: {facts:?}");
    }
}

// ----- Synthetic conversations: where no fact must appear -----

fn pair(tool: &str, args: Value, content: &str, id: &str) -> [Value; 2] {
    [
        json!({"role":"assistant","tool_calls":[{"id":id,"type":"function","function":{"name":tool,"arguments":args.to_string()}}]}),
        json!({"role":"tool","tool_call_id":id,"content":content}),
    ]
}
const BOX_OK: &str = r#"{"bodies":["Body1"],"changed_bodies":[],"created_bodies":["Body1"],"feature":1,"removed_bodies":[]}"#;
const HOLE_OK: &str = r#"{"bodies":["Body1"],"changed_bodies":["Body1"],"created_bodies":[],"feature":2,"removed_bodies":[]}"#;
fn conversation(hole: Value, result: &str) -> Vec<Value> {
    let mut m = vec![json!({"role":"user","content":"drill it"})];
    m.extend(pair(
        "execute",
        json!({"command":"solid.box","params":{"length":40,"width":30,"height":12}}),
        BOX_OK,
        "a",
    ));
    m.extend(pair("execute", hole, result, "b"));
    m
}
fn hole_call(extra: Value) -> Value {
    let mut p = json!({"position":[20,15,12],"diameter":8,"depth":10});
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    json!({"command":"solid.hole","params":p})
}

#[test]
fn synthetic_hole_gets_a_fact_and_ten_cases_get_none() {
    assert_eq!(
        facts_of(&conversation(hole_call(json!({})), HOLE_OK)),
        vec!["Hole made: 8 mm diameter, 10 mm deep along -Z from (20, 15, 12); it does not go through Body1 (12 mm thick).".to_string()]
    );
    let none = |name: &str, messages: Vec<Value>| {
        assert!(facts_of(&messages).is_empty(), "{name}");
    };
    // 1-2: failed commands (error text, rolled-back batch).
    none("error text", conversation(hole_call(json!({})), "invalid parameters for `solid.hole`: `position` [60, 15, 12] is not on a face of a body"));
    none(
        "error object",
        conversation(hole_call(json!({})), r#"{"error":"invalid parameters"}"#),
    );
    let mut batch = conversation(hole_call(json!({})), HOLE_OK);
    batch.truncate(3);
    batch.extend(pair(
        "batch",
        json!({"commands":[hole_call(json!({}))]}),
        r#"{"ok":false,"completed":1,"results":[{"bodies":["Body1"],"changed_bodies":["Body1"]}],"rolled_back":true}"#,
        "c",
    ));
    none("rolled-back batch", batch);
    // Holes Pilot does not model.
    for (name, extra) in [
        ("thread", json!({"thread":"M6"})),
        (
            "counterbore",
            json!({"type":"counterbore","cb_diameter":12,"cb_depth":3}),
        ),
        (
            "sketch points",
            json!({"sketch":"Sketch1","points":["c1.center"]}),
        ),
        (
            "reference distances",
            json!({"references":[{"edge":[0,15,12],"distance":5}]}),
        ),
        ("to object", json!({"to":[20,15,0]})),
        ("no diameter", json!({"diameter":null})),
    ] {
        none(name, conversation(hole_call(extra), HOLE_OK));
    }
    // Not a hole or a cut.
    let mut other = conversation(hole_call(json!({})), HOLE_OK);
    other.truncate(3);
    other.extend(pair(
        "execute",
        json!({"command":"solid.fillet","params":{"radius":1}}),
        HOLE_OK,
        "c",
    ));
    none("fillet", other);
    // Results before the latest assistant message are history, not trailing.
    let mut old = conversation(hole_call(json!({})), HOLE_OK);
    old.push(json!({"role":"assistant","content":"Done."}));
    none("older result", old);
    // A fact already in the result is not repeated.
    let mut again = conversation(hole_call(json!({})), HOLE_OK);
    let first = facts_of(&again);
    again[4]["content"] = json!(format!("{HOLE_OK}\n{} {}", crate::identity::HINT, first[0]));
    none("already present", again);
    // A join extrude is not a cut.
    let mut join = conversation(hole_call(json!({})), HOLE_OK);
    join.truncate(3);
    join.extend(pair(
        "execute",
        json!({"command":"solid.extrude","params":{"distance":5}}),
        HOLE_OK,
        "c",
    ));
    none("join extrude", join);
}

#[test]
fn zero_removal_without_geometry_uses_the_changed_bodies_report() {
    // The sketch is on a face, so Pilot cannot place it; SolidCraft itself reports no change.
    let mut m = conversation(hole_call(json!({})), HOLE_OK);
    m.truncate(3);
    m.extend(pair(
        "execute",
        json!({"command":"sketch.create","params":{"plane":{"face":[20,15,12]}}}),
        r#"{"name":"Sketch1","sketch":2}"#,
        "s",
    ));
    m.extend(pair(
        "execute",
        json!({"command":"sketch.circle.center","params":{"center":[1,1],"diameter":4}}),
        r#"{"curves":["c1"]}"#,
        "t",
    ));
    m.extend(pair(
        "execute",
        json!({"command":"solid.extrude","params":{"distance":5,"operation":"cut"}}),
        r#"{"bodies":["Body1"],"changed_bodies":[],"created_bodies":[],"removed_bodies":[]}"#,
        "u",
    ));
    assert_eq!(
        facts_of(&m),
        vec!["This cut removed no material: SolidCraft reports that no body changed.".to_string()]
    );
    // When something changed and the sketch cannot be placed, nothing is claimed.
    let n = m.len();
    m[n - 1]["content"] =
        json!(r#"{"bodies":["Body1"],"changed_bodies":["Body1"],"created_bodies":[]}"#);
    assert!(facts_of(&m).is_empty());
}

#[test]
fn per_result_and_per_turn_caps() {
    let commands: Vec<Value> = (0..8)
        .map(|i| hole_call(json!({"position":[5 + i * 4, 15, 12]})))
        .collect();
    let results: Vec<Value> = (0..8)
        .map(|_| serde_json::from_str(HOLE_OK).unwrap())
        .collect();
    let mut m = vec![json!({"role":"user","content":"drill eight"})];
    m.extend(pair(
        "execute",
        json!({"command":"solid.box","params":{"length":40,"width":30,"height":12}}),
        BOX_OK,
        "a",
    ));
    m.extend(pair(
        "batch",
        json!({"commands":commands}),
        &json!({"ok":true,"results":results}).to_string(),
        "b",
    ));
    // One batch result: at most four facts.
    assert_eq!(
        facts_of(&m).join(" ").matches("Hole made").count(),
        holefact::MAX_PER_RESULT
    );
    // Across a turn: six in all, however the results are split.
    let mut turn = vec![json!({"role":"user","content":"drill many"})];
    turn.extend(pair(
        "execute",
        json!({"command":"solid.box","params":{"length":40,"width":30,"height":12}}),
        BOX_OK,
        "a",
    ));
    let mut total = 0;
    for i in 0..9 {
        turn.extend(pair(
            "execute",
            hole_call(json!({"position":[5 + i * 3, 15, 12]})),
            HOLE_OK,
            &format!("h{i}"),
        ));
        let n = facts_of(&turn).len();
        // Each request only decorates its trailing results; earlier ones count toward the cap.
        total += n;
    }
    assert_eq!(total, holefact::MAX_PER_TURN);
    // A new user message starts a new count.
    turn.push(json!({"role":"user","content":"and one more"}));
    turn.extend(pair(
        "execute",
        hole_call(json!({"position":[8, 15, 12]})),
        HOLE_OK,
        "z",
    ));
    assert_eq!(facts_of(&turn).len(), 1);
}

// ----- The model face: switches and recipe turns -----

fn drilled_request(user: &str) -> Value {
    let mut req = request();
    req["messages"] = json!(conversation(hole_call(json!({})), HOLE_OK));
    req["messages"][0]["content"] = json!(user);
    req
}
fn sent_has_fact(f: &Fixture) -> bool {
    let script = f.proxy.scripted.as_ref().unwrap().borrow();
    let text = script.requests[0]["messages"].to_string();
    text.contains("Hole made:")
}

#[test]
fn model_face_adds_the_fact_unless_hints_are_off_or_a_recipe_runs() {
    let mut f = Fixture::new(vec![reply("Done.")]);
    f.chat(drilled_request(
        "drill an 8 mm hole 10 mm deep at x 20, y 15 in the block",
    ));
    assert!(sent_has_fact(&f));
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert!(log.contains("\"kind\":\"hole_fact\""));

    let mut f = Fixture::new(vec![reply("Done.")]);
    f.proxy.cfg.no_hints = true;
    f.chat(drilled_request(
        "drill an 8 mm hole 10 mm deep at x 20, y 15 in the block",
    ));
    assert!(!sent_has_fact(&f));
    let log = fs::read_to_string(f.dir.join("log.jsonl")).unwrap();
    assert!(!log.contains("hole_fact"));

    // A turn that matches a recipe carries the recipe's own plan, not this fact.
    let drill = call("execute", hole_call(json!({})));
    let mut f = Fixture::new(vec![drill, reply("Done.")]);
    let mut req = planner::cases::request("create a cube 1/4 inch");
    let response: Value = serde_json::from_slice(&f.chat(req.clone()).body).unwrap();
    let message = response["choices"][0]["message"].clone();
    let id = message["tool_calls"][0]["id"].clone();
    req["messages"].as_array_mut().unwrap().extend([
        message,
        json!({"role":"tool","tool_call_id":id,"content":HOLE_OK}),
    ]);
    f.chat(req);
    let script = f.proxy.scripted.as_ref().unwrap().borrow();
    assert!(!script.requests[1]["messages"]
        .to_string()
        .contains("Hole made:"));
}

#[test]
fn the_fact_never_changes_the_call_or_the_original_result() {
    let messages = conversation(hole_call(json!({})), HOLE_OK);
    let mut body = json!({"messages":messages.clone()});
    let f = Fixture::new(vec![]);
    let mut audit = f.audit();
    holefact::append(&mut body, &mut audit).unwrap();
    let after = body["messages"].as_array().unwrap();
    assert_eq!(after.len(), messages.len());
    for (a, b) in after.iter().zip(&messages) {
        if b["role"] == "tool" && b["tool_call_id"] == "b" {
            assert!(a["content"]
                .as_str()
                .unwrap()
                .starts_with(b["content"].as_str().unwrap()));
        } else {
            assert_eq!(a, b);
        }
    }
}
