// Independently authored development requests; no evaluation recordings.
use serde_json::{json, Value};

pub fn cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("peg", "grid_pattern", "Add a grid of pegs to this plate: 3 rows and 4 columns, 12 mm apart both ways."),
        ("peg", "grid_pattern", "Could you repeat the peg already on my plate in a rectangular array, three rows by four columns at 12 mm spacing?"),
        ("peg", "grid_pattern", "Grid this peg, 3 rows 4 columns, XY, pitch 12 mm."),
        ("peg", "grid_pattern", "I have the first peg in place now. Please fill out a grid on the existing plate, 3 rows and 4 columns with 12 mm between centres."),
        ("hole", "grid_pattern", "Repeat this hole in a grid of 3 rows and 4 columns, 12 mm spacing on XY."),
        ("hole", "grid_pattern", "The plate already has its first hole. Can you make that a rectangular grid with 3 rows and 4 columns, spaced 12 mm each way?"),
        ("hole", "grid_pattern", "Hole grid: 3 rows, 4 columns, 12 mm pitch, this plate."),
        ("hole", "grid_pattern", "Please add the remaining holes to the existing plate by repeating the last hole in 3 rows and 4 columns at 12 mm pitch."),
        ("boss", "grid_pattern", "Pattern the boss on this plate in a grid, 3 rows 4 columns 12 mm pitch."),
        ("boss", "grid_pattern", "Thanks, the first boss is there. I'd like a grid of bosses across this plate, 3 rows and 4 columns, 12 mm centre spacing in both directions."),
        ("boss", "grid_pattern", "Boss rectangular array on XY: 3 rows, 4 columns, spacing 12 mm."),
        ("boss", "grid_pattern", "Use the existing boss as the seed and repeat it in 3 rows and 4 columns on this plate with 12 mm spacing."),
        ("peg", "linear_pattern", "Repeat this peg in a row of 4 along X at 12 mm spacing."),
        ("peg", "linear_pattern", "Could you add a row of pegs to the plate using the peg already there, 4 total with 12 mm between centres along X?"),
        ("peg", "linear_pattern", "Peg linear pattern, 4 instances, X, spacing 12 mm."),
        ("peg", "linear_pattern", "Now that the peg is on the plate, please repeat it along X in a single row, four instances at 12 mm pitch."),
        ("hole", "linear_pattern", "Repeat the last hole in a row of 4 along X, 12 mm apart."),
        ("hole", "linear_pattern", "I need the existing hole repeated along the plate in one row of 4, at 12 mm centre spacing along X."),
        ("hole", "linear_pattern", "Linear hole array: 4 instances along X at 12 mm."),
        ("hole", "linear_pattern", "Please take the hole already cut in this plate and pattern it along X, four total at 12 mm spacing."),
        ("boss", "linear_pattern", "Repeat this boss in a row, count 4, X, pitch 12 mm."),
        ("boss", "linear_pattern", "The first boss looks good. Can you repeat it in a row across the existing plate, 4 total along X and 12 mm apart?"),
        ("boss", "linear_pattern", "Linear pattern of this boss: 4 instances, 12 mm, X."),
        ("boss", "linear_pattern", "Use the boss already on the plate for a row of 4 bosses spaced 12 mm along X."),
        ("peg", "circular_pattern", "Circular pattern of this peg, 6 instances around Z, full 360 degrees."),
        ("peg", "circular_pattern", "Could you repeat the peg that's on the plate around Z in a full circle of 6 instances, 360 degrees total?"),
        ("peg", "circular_pattern", "Peg radial array: 6 around Z over 360 degrees."),
        ("peg", "circular_pattern", "I want six pegs around the plate. Repeat the existing peg around the Z axis in a circular pattern covering 360 degrees."),
        ("hole", "circular_pattern", "Repeat this hole in a circular pattern, 6 around Z over 360 degrees."),
        ("hole", "circular_pattern", "Please use the existing hole as the seed for a circular pattern of 6 holes around Z, full 360 degrees."),
        ("hole", "circular_pattern", "Radial hole array, 6 instances around Z, 360 degrees."),
        ("hole", "circular_pattern", "The first hole is ready in this plate; can you repeat it around Z as six holes over a complete 360 degree circle?"),
        ("boss", "circular_pattern", "Circular pattern this boss around Z, 6 instances, 360 degrees."),
        ("boss", "circular_pattern", "I'd like the existing boss repeated around the plate in a circular pattern: 6 instances about Z over 360 degrees."),
        ("boss", "circular_pattern", "Boss radial array about Z: 6 total, 360 degrees."),
        ("boss", "circular_pattern", "Use the boss already on this plate and repeat it around the Z axis, 6 instances over a full 360 degrees."),
    ]
}

pub fn chatty_cases() -> Vec<(&'static str, &'static str, String)> {
    // Deliberately conversational preamble: exercise coverage dilution without
    // changing the CAD instruction, geometry or supported operation.
    let preamble = "Good morning! Yesterday my workshop session ended early because the weather turned miserable. Today feels calmer, so I finally sat down again after breakfast. The coffee went cold while I tidied the desk, found my notebook beneath several receipts, checked the calendar, and settled into a comfortable chair. My neighbour mentioned their weekend hiking trip and sent lovely photographs yesterday evening. Meanwhile the radio played quietly beside the window. Anyway, those distractions are finished, everything feels peaceful, and I am ready to continue the small project we started earlier. I appreciate your patience while I gather my thoughts; writing everything down helps me concentrate. Here is the actual change I would like: ";
    cases()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, (kind, id, text))| (kind, id, format!("{preamble}{text}")))
        .collect()
}

// Public-command setup and corresponding inspection payloads for the offline replay.
pub fn setup(kind: &str) -> Value {
    let hole = kind == "hole";
    let volume = 72_000.0 + std::f64::consts::PI * 4.0 * if hole { -5.0 } else { 3.0 };
    let feature = if hole {
        "HoleFeature"
    } else {
        "CylinderFeature"
    };
    let name = if hole { "Hole1" } else { "Cylinder1" };
    let command = if hole {
        json!({"command":"solid.hole","params":{"position":[20,0,5],"diameter":4}})
    } else {
        json!({"command":"solid.cylinder","params":{"base":[20,0,5],"diameter":4,"height":3,"operation":"join","targets":["Plate"]}})
    };
    let definition = if hole {
        json!({"targets":["Plate"],"position":[20,0,5],"direction":[0,0,-1],"diameter":"4","depth":"5"})
    } else {
        json!({"participants":["Plate"],"base":[20,0,5],"axis":[0,0,1],"diameter":"4","height":"3","operation":"join"})
    };
    json!({
        "commands":[{"command":"solid.box","params":{"body_name":"Plate","corner":[-60,-60,0],"length":120,"width":120,"height":5}},command],
        "document":{"volume_mm3":volume,"bodies":[{"name":"Plate","volume_mm3":volume,"bbox":{"min":[-60,-60,0],"max":[60,60,if hole {5} else {8}]}}],"timeline":[{"id":2,"name":name,"type":feature}]},
        "detail":{"id":2,"name":name,"type":feature,"definition":definition}
    })
}

pub fn request(text: &str, setup: &Value) -> Value {
    json!({"messages":[{"role":"user","content":text},{"role":"tool","content":setup["document"].to_string()},{"role":"tool","content":setup["detail"].to_string()}]})
}
