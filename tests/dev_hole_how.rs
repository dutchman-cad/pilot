//! Independently authored development geometry, using only the public CLI.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use dutchman_pilot::catalog::{Catalog, MAX_CATALOG_BYTES};
use serde_json::{json, Value};
use std::process::Command;
#[path = "support/public_catalog.rs"]
mod public_catalog;
fn step(command: &str, params: Value) -> Value {
    json!({"command":command,"params":params})
}
fn example(cat: &Catalog, task: &str) -> Value {
    let text = cat.how_text(task).unwrap();
    assert!(text.len() < 900, "{text}");
    let call: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(cat.how(task).unwrap().matches[0].example, call);
    call["arguments"].clone()
}
fn replay(cli: &std::ffi::OsStr, commands: &[Value], id: &str) -> Vec<Value> {
    assert!(commands.len() <= 16);
    let path = std::env::temp_dir().join(format!("pilot-holehow-{}-{id}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(commands).unwrap()).unwrap();
    let output = Command::new(cli)
        .arg("run")
        .arg(&path)
        .arg("--results")
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{id}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() < 4 * 1024 * 1024);
    let results: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(results.len(), commands.len());
    results
}
fn vec3(v: &Value) -> [f64; 3] {
    std::array::from_fn(|i| v[i].as_f64().unwrap())
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cylinder(faces: &Value, start: [f64; 3], dir: [f64; 3], diameter: f64, depth: f64) {
    let middle: [f64; 3] = std::array::from_fn(|i| start[i] + dir[i] * depth / 2.);
    let mut count = 0;
    for face in faces.as_array().unwrap() {
        if face["cylinder"].is_null() {
            continue;
        }
        let c = &face["cylinder"];
        let axis = vec3(&c["axis"]);
        if (c["radius"].as_f64().unwrap() * 2. - diameter).abs() > 1e-4
            || dot(axis, dir).abs() < 0.99999
        {
            continue;
        }
        let origin = vec3(&c["origin"]);
        let delta = std::array::from_fn(|i| middle[i] - origin[i]);
        let axial = dot(delta, axis);
        let radial = (0..3)
            .map(|i| (delta[i] - axial * axis[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if radial > 1e-4 {
            continue;
        }
        let centroid = vec3(&face["centroid"]);
        // Public cylinders may have split half faces: their centroids share the axial midpoint.
        assert!(
            dot(std::array::from_fn(|i| centroid[i] - middle[i]), axis).abs() < 1e-4,
            "wrong centre: {face}"
        );
        let length = 2. * dot(std::array::from_fn(|i| origin[i] - centroid[i]), axis).abs();
        assert!(
            (length - depth).abs() < 1e-4,
            "depth {length} != {depth}: {face}"
        );
        count += 1;
    }
    assert!(
        count > 0,
        "missing cylinder d={diameter} depth={depth} centre={middle:?}: {faces}"
    );
}
#[test]
fn how_examples_build_development_holes() {
    let Some(cli) = std::env::var_os("PILOT_SOLIDCRAFT_CLI") else {
        return;
    };
    let cat = public_catalog::catalog().unwrap();
    let mut holes = 0;
    for kind in [
        "underside blind hole",
        "side wall hole",
        "hollow wall hole",
        "angled face hole",
        "offset face hole",
        "counterbore hole",
        "hole row along wall",
    ] {
        for i in 0..4 {
            let mut commands = vec![step(
                "solid.box",
                json!({"length":60,"width":40,"height":20}),
            )];
            let d = 4. + i as f64;
            let depth = 5. + i as f64;
            let mut call = example(&cat, kind);
            let (start, dir, actual_depth) = match kind {
                "underside blind hole" => ([15. + i as f64 * 4., 15., 0.], [0., 0., 1.], depth),
                "side wall hole" => match i {
                    0 => ([10., 0., 10.], [0., 1., 0.], 40.),
                    1 => ([10., 40., 10.], [0., -1., 0.], 40.),
                    2 => ([0., 20., 10.], [1., 0., 0.], 60.),
                    _ => ([60., 20., 10.], [-1., 0., 0.], 60.),
                },
                "hole row along wall" => ([10., 0., 10.], [0., 1., 0.], 40.),
                "hollow wall hole" => {
                    commands.push(step(
                        "solid.shell",
                        json!({"faces":[[30,20,20]],"thickness":3,"body":"Body1"}),
                    ));
                    ([20., 0., 10.], [0., 1., 0.], 3.)
                }
                "angled face hole" => {
                    commands[0]["params"]["center"] = json!([0, 0, 0]);
                    commands.push(step("solid.move",json!({"bodies":["Body1"],"axis":[1,0,0],"angle":-36.86989764584402,"translate":[20,9,22]})));
                    ([20., 15., 30.], [0., -0.6, -0.8], depth)
                }
                "offset face hole" => {
                    commands[0]["params"]["corner"] = json!([-30, 5, 25]);
                    ([-10., 20., 45.], [0., 0., -1.], depth)
                }
                _ => ([20., 15., 20.], [0., 0., -1.], 20.),
            };
            if kind == "hole row along wall" {
                let mut seed = example(&cat, "side wall hole");
                seed["params"]["position"] = json!(start);
                seed["params"]["diameter"] = json!(d);
                commands.push(seed);
                call["params"]["count1"] = json!(3 + i % 2);
                call["params"]["spacing1"] = json!(11);
            } else {
                call["params"]["position"] = json!(start);
                call["params"]["direction"] = json!(dir);
                call["params"]["diameter"] = json!(d);
                if !matches!(kind, "side wall hole" | "counterbore hole") {
                    call["params"]["depth"] = json!(depth);
                }
                if kind == "counterbore hole" && i % 2 == 1 {
                    call["params"]["depth"] = json!(12);
                }
                if kind == "hollow wall hole" {
                    call["params"]["depth"] = json!(3);
                }
            }
            commands.push(call);
            commands.push(step("model.faces", json!({"body":"Body1"})));
            let results = replay(&cli, &commands, &format!("{kind}-{i}"));
            let faces = &results.last().unwrap()["faces"];
            if kind == "hole row along wall" {
                for j in 0..3 + i % 2 {
                    cylinder(
                        faces,
                        [start[0] + 11. * j as f64, start[1], start[2]],
                        dir,
                        d,
                        actual_depth,
                    );
                    holes += 1;
                }
            } else if kind == "counterbore hole" {
                cylinder(faces, start, dir, 10., 3.);
                cylinder(
                    faces,
                    [start[0], start[1], start[2] - 3.],
                    dir,
                    d,
                    if i % 2 == 1 { 9. } else { 17. },
                );
                holes += 1;
            } else {
                cylinder(faces, start, dir, d, actual_depth);
                holes += 1;
            }
        }
    }
    assert!(holes >= 20);
    eprintln!("Verified {holes} development holes by cylinder diameter, axial depth and centre");
}
