use serde_json::{json, Value};

pub fn accepted() -> Vec<(Value, Value)> {
    let mut cases = Vec::new();
    for (id, op) in [
        ("solid.cut", "cut"),
        ("solid.subtract", "cut"),
        ("solid.difference", "cut"),
        ("solid.subtraction", "cut"),
        ("solid.boolean.cut", "cut"),
        ("solid.boolean.subtract", "cut"),
        ("solid.boolean.difference", "cut"),
        ("solid.union", "join"),
        ("solid.unite", "join"),
        ("solid.boolean.union", "join"),
        ("SOLID_UNION", "join"),
        ("solid-difference", "cut"),
    ] {
        cases.push((json!({"command":id,"params":{"target":"Blank","tool":"Drill","keep_tools":false}}),json!({"command":"solid.combine","params":{"target":"Blank","tools":["Drill"],"operation":op,"keep_tools":false}})));
    }
    for (word, op) in [
        ("new solid", "new"),
        ("new body", "new"),
        ("add", "join"),
        ("subtract", "cut"),
        ("remove", "cut"),
        ("union", "join"),
    ] {
        cases.push((
            json!({"command":"solid.extrude","params":{"distance":7,"operation":word}}),
            json!({"command":"solid.extrude","params":{"distance":7,"operation":op}}),
        ));
    }
    for (plane, offset, expected) in [
        ("X=10", 10, "YZ"),
        ("Z=5", 5, "XY"),
        ("Y=4", 4, "XZ"),
        ("Z=-3", -3, "XY"),
        (" x = -2 ", -2, "YZ"),
    ] {
        cases.push((json!({"command":"sketch.create","params":{"plane":plane,"name":"Test plane"}}),json!({"command":"sketch.create","params":{"plane":expected,"offset":offset,"name":"Test plane"}})));
    }
    for z in [2, 5] {
        cases.push((json!({"command":"sketch.create","params":{"plane":"XY","origin":[0,0,z],"normal":[0,0,1]}}),json!({"command":"sketch.create","params":{"plane":{"origin":[0,0,z],"x_dir":[1,0,0],"y_dir":[0,1,0]}}})));
    }
    cases.push((json!({"command":"sketch.create","params":{"plane":{},"origin":[1,2,3],"x_dir":[1,0,0],"y_dir":[0,1,0]}}),json!({"command":"sketch.create","params":{"plane":{"origin":[1,2,3],"x_dir":[1,0,0],"y_dir":[0,1,0]}}})));
    cases.push((
        json!({"command":"sketch.create","params":{"plane":{},"face":[6,7,10]}}),
        json!({"command":"sketch.create","params":{"plane":{"face":[6,7,10]}}}),
    ));
    for (x, y) in [(3, 4), (6, 7), (8, 9), (11, 12)] {
        cases.push((
            json!({"command":"solid.hole","params":{"position":{"x":x,"y":y,"z":10},"diameter":2}}),
            json!({"command":"solid.hole","params":{"position":[x,y,10],"diameter":2}}),
        ));
        cases.push((json!({"command":"solid.hole","params":{"position":[x.to_string(),y.to_string(),"10"],"diameter":"2"}}),json!({"command":"solid.hole","params":{"position":[x,y,10],"diameter":"2"}})));
    }
    for translate in [json!([3, 0, 0]), json!([0, -4, 0]), json!([0, 0, 6])] {
        cases.push((
            json!({"command":"solid.move","params":{"bodies":"Blank","translate":translate}}),
            json!({"command":"solid.move","params":{"bodies":["Blank"],"translate":translate}}),
        ));
    }
    for index in [0, 1, 2, 3] {
        cases.push((json!({"command":"solid.fillet","params":{"edges":[index],"body":"Blank","radius":1}}),json!({"command":"solid.fillet","params":{"edges":[{"body":"Blank","index":index}],"body":"Blank","radius":1}})));
    }
    cases
}

pub fn refused() -> Vec<Value> {
    vec![
        json!({"command":"solid.cut","params":{"target":"Blank"}}),
        json!({"command":"solid.union","params":{"tools":["Drill"]}}),
        json!({"command":"solid.subtract","params":{"target":"Blank","tool":"Drill","tools":["Other"]}}),
        json!({"command":"solid.difference","params":{"target":"Blank","tools":["Drill"],"operation":"join"}}),
        json!({"command":"solid.union","params":{"target":"Blank","tools":["Blank"]}}),
        json!({"command":"solid.cut","params":{"target":"Blank","tools":["*"]}}),
        json!({"command":"solid.cut","params":{"target":"Blank","tools":[0]}}),
        json!({"command":"solid.extrude","params":{"distance":7,"operation":"add or subtract"}}),
        json!({"command":"sketch.create","params":{"plane":"Z=5","offset":4}}),
        json!({"command":"sketch.create","params":{"plane":"X=10","origin":[1,2,3]}}),
        json!({"command":"sketch.create","params":{"plane":{},"normal":[0,0,1],"origin":[0,0,5]}}),
        json!({"command":"sketch.create","params":{"plane":"XY","face":[6,7,10]}}),
        json!({"command":"sketch.create","params":{"plane":{"origin":[0,0,0]},"origin":[1,2,3]}}),
        json!({"command":"solid.hole","params":{"position":[6,7],"face":"top","diameter":3}}),
        json!({"command":"solid.hole","params":{"position":{"x":6,"y":7},"diameter":3}}),
        json!({"command":"solid.fillet","params":{"edges":[0],"radius":1}}),
        json!({"command":"solid.fillet","params":{"edges":["Edge1"],"radius":1}}),
        json!({"command":"solid.fillet","params":{"edges":["Edge1"],"body":"Blank","radius":1}}),
    ]
}
