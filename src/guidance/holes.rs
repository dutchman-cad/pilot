//! Bounded face evidence, shared by the model and MCP faces. Never sends reads.
use super::payload;
use serde_json::{json, Value};

pub(crate) const HINT: &str = "solid.hole: position is on the entry face, direction drills inward (negative model.faces plane_normal); use face point, not centroid. Omit depth for through all. Counterbore: type:counterbore, cb_diameter, cb_depth. Wall rows: pattern the returned hole feature with solid.pattern.rectangular.";

#[derive(Default)]
pub(crate) struct Faces {
    faces: Vec<(String, Value)>,
}
fn point(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    let p = [a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?];
    p.iter()
        .all(|n| n.is_finite() && n.abs() <= 1e6)
        .then_some(p)
}
impl Faces {
    pub(crate) fn visit(&mut self, name: &str, args: &Value, result: &Value) -> Vec<String> {
        let data = payload(result);
        if name == "batch" {
            let mut out = Vec::new();
            if let Some((calls, results)) =
                args["commands"].as_array().zip(data["results"].as_array())
            {
                for (call, result) in calls.iter().zip(results).take(128) {
                    for hint in self.visit("execute", call, result) {
                        if out.len() < 4 {
                            out.push(hint);
                        }
                    }
                }
            }
            return out;
        }
        let text = result
            .to_string()
            .chars()
            .take(8192)
            .collect::<String>()
            .to_ascii_lowercase();
        let failed = result["isError"] == true
            || result["result"]["isError"] == true
            || data["isError"] == true
            || data["ok"] == false
            || data.get("error").is_some_and(|e| !e.is_null());
        // A failure can itself contain public face diagnostics.
        self.read(&data);
        if args["command"] == "solid.hole"
            && failed
            && text.contains("position")
            && text.contains("face")
        {
            let Some(p) = point(&args["params"]["position"]) else {
                return Vec::new();
            };
            let mut near = Vec::new();
            for (body, face) in &self.faces {
                if args["params"]["targets"]
                    .as_array()
                    .is_some_and(|a| !a.iter().any(|v| v == body || v == "*"))
                {
                    continue;
                }
                let Some(q) = point(&face["point"]) else {
                    continue;
                };
                let Some(n) = point(&face["plane_normal"]) else {
                    continue;
                };
                let norm = n.iter().map(|v| v * v).sum::<f64>().sqrt();
                if norm < 1e-9 {
                    continue;
                }
                let distance = (0..3).map(|i| (p[i] - q[i]) * n[i]).sum::<f64>().abs() / norm;
                near.push((distance, json!({"body":body,"face":face["name"],"point":q,"outward_normal":n,"plane_distance_mm":(distance*1000.).round()/1000.})));
            }
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let faces: Vec<_> = near.into_iter().take(3).map(|(_, f)| f).collect();
            return vec![if faces.is_empty() {
                "Hole position must be on an entry face. No face evidence available; call model.faces for the target body, use point and drill against plane_normal.".into()
            } else {
                format!("Nearby face planes (plane distance only; check the trimmed face): {}. Use an on-face point and drill against outward_normal.", json!(faces))
            }];
        }
        if failed {
            return Vec::new();
        }
        let command = args["command"].as_str().unwrap_or("");
        if command == "solid.box" {
            self.faces.clear();
            let p = &args["params"];
            if p.get("operation").is_some_and(|v| v != "new") {
                return Vec::new();
            }
            let size = [
                p["length"].as_f64(),
                p["width"].as_f64(),
                p["height"].as_f64(),
            ];
            if let [Some(x), Some(y), Some(z)] = size {
                let size = [x, y, z];
                let corner = if p.get("center").is_some() {
                    point(&p["center"]).map(|c| std::array::from_fn(|i| c[i] - size[i] / 2.))
                } else {
                    p.get("corner").map_or(Some([0.; 3]), point)
                };
                if let Some(lo) =
                    corner.filter(|_| size.iter().all(|v| v.is_finite() && *v > 0. && *v <= 1e6))
                {
                    if let Some(names) = data["created_bodies"].as_array() {
                        for body in names
                            .iter()
                            .take(4)
                            .filter_map(Value::as_str)
                            .filter(|n| n.len() <= 128)
                        {
                            for axis in 0..3 {
                                for sign in [-1., 1.] {
                                    let mut q =
                                        std::array::from_fn::<_, 3, _>(|i| lo[i] + size[i] / 2.);
                                    q[axis] = lo[axis] + if sign > 0. { size[axis] } else { 0. };
                                    let mut n = [0.; 3];
                                    n[axis] = sign;
                                    self.faces.push((body.into(),json!({"name":format!("box {}{}",if sign>0. {"+"} else {"-"},["X","Y","Z"][axis]),"point":q,"plane_normal":n})));
                                }
                            }
                        }
                    }
                }
            }
        } else if command.starts_with("solid.")
            || command.starts_with("timeline.")
            || command == "model.delete"
            || command.starts_with("document.")
            || command.starts_with("component.")
            || command.starts_with("occurrence.")
        {
            self.faces.clear();
        }
        Vec::new()
    }
    fn read(&mut self, data: &Value) {
        if let Some(faces) = data["faces"].as_array() {
            if let Some(body) = data["body"].as_str().filter(|s| s.len() <= 128) {
                self.faces.retain(|(b, _)| b != body);
                for face in faces.iter().take(128) {
                    if self.faces.len() >= 256 {
                        break;
                    }
                    if point(&face["point"]).is_some() && point(&face["plane_normal"]).is_some() {
                        self.faces.push((body.into(),json!({"name":face["name"].as_str().filter(|s| s.len()<=128),"point":face["point"],"plane_normal":face["plane_normal"]})));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn call(position: Value) -> Value {
        json!({"command":"solid.hole","params":{"position":position}})
    }
    fn error() -> Value {
        json!({"error":"position is not on a face of a body"})
    }
    fn box_call() -> Value {
        json!({"command":"solid.box","params":{"length":60,"width":40,"height":20,"corner":[10,20,30]}})
    }
    #[test]
    fn hole_position_error_uses_box_evidence_and_never_changes_calls() {
        let mut s = Faces::default();
        s.visit("execute", &box_call(), &json!({"created_bodies":["Part"]}));
        let args = call(json!([30, 35, 31]));
        let original = args.clone();
        let result = error();
        let original_result = result.clone();
        let hints = s.visit("execute", &args, &result);
        assert_eq!(args, original);
        assert_eq!(result, original_result);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("box -Z"));
        assert!(hints[0].contains("[40.0,40.0,30.0]"));
        assert!(hints[0].contains("\"plane_distance_mm\":1.0"));
        assert!(s
            .visit("execute", &args, &json!({"changed_bodies":["Part"]}))
            .is_empty());
        assert!(s.visit("execute", &args, &error())[0].contains("No face evidence"));
    }
    #[test]
    fn hole_position_error_uses_measured_faces_in_wrappers_and_batches() {
        for wrapped in [false, true] {
            let mut s = Faces::default();
            let data = json!({"body":"Slope","faces":[{"name":"angled","point":[20,15,30],"centroid":[999,999,999],"plane_normal":[0,0.6,0.8]}]});
            let data = if wrapped {
                json!({"result":{"content":[{"type":"text","text":data.to_string()}]}})
            } else {
                data
            };
            s.visit("execute", &json!({"command":"model.faces"}), &data);
            let args = call(json!([20, 15, 29]));
            let result =
                json!({"isError":true,"content":[{"type":"text","text":"position not on a face"}]});
            let h = s.visit(
                "batch",
                &json!({"commands":[args]}),
                &json!({"results":[result]}),
            );
            assert!(h[0].contains("angled"));
            assert!(h[0].contains("[20.0,15.0,30.0]"));
            assert!(!h[0].contains("999"));
            assert!(h[0].contains("0.8"));
        }
    }
    #[test]
    fn hole_position_error_is_scoped_finite_and_invalidated() {
        let mut s = Faces::default();
        s.visit("execute", &box_call(), &json!({"created_bodies":["Part"]}));
        for (command, result, pos) in [
            ("solid.box", error(), json!([1, 2, 3])),
            (
                "solid.hole",
                json!({"error":"diameter must be positive"}),
                json!([1, 2, 3]),
            ),
            ("solid.hole", error(), json!([1, 2])),
            ("solid.hole", error(), json!([1e100, 2, 3])),
            (
                "solid.hole",
                json!({"ok":true,"message":"position face"}),
                json!([1, 2, 3]),
            ),
        ] {
            let args = json!({"command":command,"params":{"position":pos}});
            assert!(s.visit("execute", &args, &result).is_empty());
        }
        s.visit(
            "execute",
            &json!({"command":"solid.move"}),
            &json!({"changed_bodies":["Part"]}),
        );
        assert!(
            s.visit("execute", &call(json!([1, 2, 3])), &error())[0].contains("No face evidence")
        );
    }
}
