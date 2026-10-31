//! Bounded file recipes and a replaceable local retrieval index.
use crate::{catalog::Catalog, planner, repair};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
mod files;
pub mod forms;

pub const FIND_DESCRIPTION: &str = "Search recipes before building. Returns the closest three schemas. On no good hit, do not improvise CAD: tell the user what Pilot can build or ask one question. Then use pilot_build with recipe and args. Lengths are mm.";
pub const BUILD_DESCRIPTION: &str = "Build a recipe found with pilot_find: supply its id and typed args. Lengths are mm. Legacy shape arguments also work. Inspect before selecting existing bodies or a contact joint.";
pub const HIT_THRESHOLD: f64 = 1.8;
/// Offer a direct form at the earlier fill gate; route only below it.
pub const FILL_THRESHOLD: f64 = 4.5;
pub const ROUTE_THRESHOLD: f64 = 1.5;
pub const IDS: &[&str] = &[
    "cross_drilled_box",
    "honeycomb_pattern",
    "linear_pattern",
    "grid_pattern",
    "circular_pattern",
    "profile",
    "plate",
    "enclosure",
    "revolute_joint",
    "joint",
    "box",
    "cylinder",
    "sphere",
    "tube",
    "fillet",
    "chamfer",
    "move",
    "mirror",
    "shell",
    "revolve",
];
const MAX_FILE: usize = 65536;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub description: String,
    pub examples: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synonyms: Vec<String>,
    pub schema: Value,
    pub expander: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<Value>,
}
impl Recipe {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_FILE {
            return Err("recipe exceeds 64 KiB".into());
        }
        // All inputs (embedded, directory files and reviewed drafts) enter here.
        // Check the original byte limit before normalizing the entire document.
        let text = text
            .strip_prefix('\u{feff}')
            .unwrap_or(text)
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let text = text
            .strip_prefix("---\n")
            .ok_or("recipe needs YAML frontmatter (JSON subset)")?;
        let (front, body) = text
            .split_once("\n---\n")
            .ok_or("recipe frontmatter terminator missing")?;
        let recipe: Self =
            serde_json::from_str(front).map_err(|e| format!("invalid recipe frontmatter: {e}"))?;
        repair::check_value(&recipe.schema)?;
        if let Some(fill) = recipe.schema.get("x-pilot-fill") {
            forms::check_definition(fill)?;
        }
        if !identifier(&recipe.id)
            || recipe.synonyms.len() > 32
            || recipe
                .synonyms
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 128)
            || recipe.description.is_empty()
            || recipe.description.len() > 256
            || recipe.description.contains(['\n', '\r'])
            || !(3..=8).contains(&recipe.examples.len())
            || recipe
                .examples
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 512)
            || body.trim().is_empty()
        {
            return Err(
                "recipe needs a bounded id, one-line description, 3..8 examples and a body".into(),
            );
        }
        if recipe.schema["type"] != "object" || recipe.schema["additionalProperties"] != false {
            return Err("recipe schema must be an object with additionalProperties:false".into());
        }
        if recipe.expander == "sequence" {
            if recipe.calls.is_empty() || recipe.calls.len() > 64 {
                return Err("sequence requires 1..64 calls".into());
            }
        } else {
            let mut typed = recipe.schema.clone();
            if let Some(o) = typed.as_object_mut() {
                o.remove("x-pilot-fill");
            }
            if typed != schema(&recipe.expander)? || !recipe.calls.is_empty() {
                return Err(format!("{}: schema differs from typed expander", recipe.id));
            }
        }
        Ok(recipe)
    }
    pub fn markdown(&self) -> Result<String, String> {
        let front =
            serde_json::to_string_pretty(self).map_err(|e| format!("cannot encode recipe: {e}"))?;
        Ok(format!(
            "---\n{front}\n---\n\n{} Built by the `{}` expander.\n",
            self.description, self.expander
        ))
    }
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}
pub fn definitions() -> Value {
    json!({
        "Length":{"type":"number","exclusiveMinimum":0,"maximum":1000000,"description":"Positive millimetres"},
        "Angle":{"type":"number","minimum":-360,"maximum":360,"description":"Degrees"},
        "Point3":{"type":"array","items":{"type":"number","minimum":-1000000,"maximum":1000000},"minItems":3,"maxItems":3,"description":"[x,y,z] millimetres"},
        "Plane":{"type":"string","enum":["XY","XZ","YZ"],"description":"Sketch plane; extrude along its normal"},
        "Count":{"type":"integer","minimum":1,"maximum":64}
    })
}
fn field_type(name: &str, s: &Value) -> Value {
    match name {
        "plane" => json!({"$ref":"#/$defs/Plane"}),
        "count" | "rows" | "columns" => json!({"$ref":"#/$defs/Count"}),
        "angle_deg" | "start_deg" => json!({"$ref":"#/$defs/Angle"}),
        "boss_offset_x" | "boss_offset_y" | "offset_x" | "offset_y" => {
            json!({"type":"number","minimum":-1000000,"maximum":1000000})
        }
        "stem_offset" => json!({"type":"number","minimum":0,"maximum":1000000}),
        "body" | "pin_component" | "hole_component" => {
            json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f]+$"})
        }
        _ if s["type"] == "number" => json!({"$ref":"#/$defs/Length"}),
        _ => s.clone(),
    }
}
fn shared(s: &mut Value) {
    if let Some(props) = s.get_mut("properties").and_then(Value::as_object_mut) {
        for (name, child) in props {
            let description = child.get("description").cloned();
            let default = child.get("default").cloned();
            *child = field_type(name, child);
            if let Some(d) = description {
                child["description"] = d;
            }
            if let Some(d) = default {
                child["default"] = d;
            }
            shared(child);
        }
    }
}
/// Schemas derive from the typed shortcut contract, with shared scalar constraints.
pub fn schema(id: &str) -> Result<Value, String> {
    if id == "joint" {
        let mut s = json!({"type":"object","properties":{},"required":["joint_type"],"additionalProperties":false,
            "$schema":"https://json-schema.org/draft/2020-12/schema","$defs":definitions(),"x-pilot-expander":"joint"});
        planner::joints::fields(&mut s);
        return Ok(s);
    }
    if id == "cross_drilled_box" {
        return Ok(planner::cross_drilled_schema());
    }
    if id == "honeycomb_pattern" {
        return Ok(planner::honeycomb_schema());
    }
    if id.ends_with("_pattern") {
        return planner::pattern_schema(id);
    }
    let old = planner::shortcut_tool();
    let mut s = if let Some(v) = old["function"]["parameters"]["oneOf"]
        .as_array()
        .and_then(|vs| vs.iter().find(|v| v["properties"]["shape"]["const"] == id))
    {
        let mut s = v.clone();
        if let Some(p) = s["properties"].as_object_mut() {
            p.remove("shape");
        }
        if let Some(r) = s["required"].as_array_mut() {
            r.retain(|k| k != "shape");
        }
        s
    } else {
        let fields: &[(&str, &str)] = match id {
            "box" => &[
                ("length", "Length"),
                ("width", "Length"),
                ("height", "Length"),
            ],
            "cylinder" | "revolve" => &[("radius", "Length"), ("height", "Length")],
            "sphere" => &[("radius", "Length")],
            "tube" => &[
                ("outer_radius", "Length"),
                ("inner_radius", "Length"),
                ("height", "Length"),
            ],
            "fillet" | "chamfer" => &[("body", "string"), ("amount", "Length")],
            "move" => &[("body", "string"), ("delta", "Point3")],
            "mirror" => &[
                ("body", "string"),
                ("plane", "Plane"),
                ("keep_original", "boolean"),
            ],
            "shell" => &[
                ("length", "Length"),
                ("width", "Length"),
                ("height", "Length"),
                ("wall", "Length"),
            ],
            _ => return Err(format!("unknown typed expander: {id}")),
        };
        let props: BTreeMap<_, _> = fields
            .iter()
            .map(|(k, t)| {
                (
                    *k,
                    if matches!(*t, "string" | "boolean") {
                        json!({"type":t})
                    } else {
                        json!({"$ref":format!("#/$defs/{t}")})
                    },
                )
            })
            .collect();
        json!({"type":"object","properties":props,"required":fields.iter().map(|(k,_)|k).collect::<Vec<_>>(),"additionalProperties":false})
    };
    shared(&mut s);
    if matches!(id, "fillet" | "chamfer") {
        s["properties"]["edges"] = json!({"type":"string","enum":["all","vertical"]});
        s["properties"]["corner_sides"] =
            json!({"type":"string","enum":["all","right","left","top","bottom"]});
    }
    if id == "shell" {
        s["properties"]["open"] = json!({"type":"string","enum":["top","bottom"],"default":"top"});
        s["properties"]["body"] = json!({"type":"string","minLength":1,"maxLength":128,"description":"Exact inspected rectangular body to shell; omit to create a new box"});
        s["required"] = json!(["wall"]);
        s["anyOf"] = json!([{"required":["body"]},{"required":["length","width","height"]}]);
    }
    s["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    s["$defs"] = definitions();
    s["x-pilot-expander"] = json!(id);
    forms::describe(&mut s);
    Ok(s)
}
/// Validate the small JSON Schema vocabulary emitted by Pilot. Unknown keywords fail closed.
pub fn validate(value: &Value, schema: &Value) -> Result<(), String> {
    validate_structure(value, schema)?;
    // JSON Schema cannot compare two arbitrary numeric properties. This explicit
    // vocabulary hook shares geometry validation with the expander (no I/O).
    if let Some(id) = schema["x-pilot-expander"].as_str() {
        let result =
            planner::recipe_expand(id, value, &json!({"messages":[]}), &Catalog::bundled()?);
        if let Err(error) = result {
            if !matches!(id, "revolute_joint" | "joint")
                || !error.starts_with(&format!("{id}: inspection must resolve"))
            {
                return Err(error);
            }
        }
    }
    Ok(())
}
pub(crate) fn validate_structure(value: &Value, schema: &Value) -> Result<(), String> {
    repair::check_value(value).map_err(|e| format!("args {e}"))?;
    validate_at(value, schema, schema, "args", 0)
}
fn validate_at(v: &Value, s: &Value, root: &Value, path: &str, depth: usize) -> Result<(), String> {
    if depth > 32 {
        return Err("schema nesting exceeds 32".into());
    }
    if let Some(r) = s["$ref"].as_str() {
        let target = r
            .strip_prefix('#')
            .and_then(|p| root.pointer(p))
            .ok_or("schema reference must resolve locally")?;
        return validate_at(v, target, root, path, depth + 1);
    }
    if !s.is_object() {
        return Err("schema must be an object".into());
    }
    if s.get("enum").is_some() && (v.is_array() || v.is_object()) {
        return Err(format!(
            "{path} takes {}; not a list or an object; use one listed string",
            forms::enum_meanings(s)
        ));
    }
    for key in s.as_object().into_iter().flat_map(|m| m.keys()) {
        if ![
            "type",
            "$schema",
            "$defs",
            "$ref",
            "description",
            "default",
            "properties",
            "required",
            "additionalProperties",
            "const",
            "enum",
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "minLength",
            "maxLength",
            "pattern",
            "items",
            "minItems",
            "maxItems",
            "allOf",
            "anyOf",
            "x-pilot-expander",
            "x-pilot-fill",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unsupported schema keyword: {key}"));
        }
    }
    if let Some(types) = s["type"].as_array() {
        if !types.iter().any(|t| {
            let mut branch = s.clone();
            branch["type"] = t.clone();
            validate_at(v, &branch, root, path, depth + 1).is_ok()
        }) {
            return Err(format!(
                "{path} {v} must match 1 of {}; use a listed JSON type",
                s["type"]
            ));
        }
        return Ok(());
    }
    for branch in s["allOf"].as_array().into_iter().flatten() {
        validate_at(v, branch, root, path, depth + 1)?;
    }
    if let Some(branches) = s["anyOf"].as_array() {
        if !branches
            .iter()
            .any(|b| validate_at(v, b, root, path, depth + 1).is_ok())
        {
            return Err(format!("{path} {v} must match at least 1 of {} alternatives; supply the required fields for one alternative", branches.len()));
        }
    }
    let good = match s["type"].as_str() {
        Some("object") => v.is_object(),
        Some("number") => v.as_f64().is_some_and(f64::is_finite),
        Some("integer") => v
            .as_f64()
            .is_some_and(|n| n.is_finite() && n.fract() == 0.0),
        Some("array") => v.is_array(),
        Some("string") => v.is_string(),
        Some("boolean") => v.is_boolean(),
        None => true,
        _ => false,
    };
    if !good
        || s.get("const").is_some_and(|c| c != v)
        || s["enum"].as_array().is_some_and(|e| !e.contains(v))
    {
        return Err(format!("{path} {v} must match 1 value of type {} with choices {}; use a listed value of that type", s["type"], s.get("enum").or_else(|| s.get("const")).unwrap_or(&json!("any"))));
    }
    if let Some(n) = v.as_f64() {
        if s["minimum"].as_f64().is_some_and(|b| n < b)
            || s["maximum"].as_f64().is_some_and(|b| n > b)
            || s["exclusiveMinimum"].as_f64().is_some_and(|b| n <= b)
        {
            return Err(format!(
                "{path} {v} must be {}{} and at most {}; change {path} to a number in this range",
                if s.get("exclusiveMinimum").is_some() {
                    "above "
                } else {
                    "at least "
                },
                s.get("exclusiveMinimum")
                    .or_else(|| s.get("minimum"))
                    .unwrap_or(&json!("unbounded")),
                s.get("maximum").unwrap_or(&json!("unbounded"))
            ));
        }
    }
    if let Some(t) = v.as_str() {
        if s.get("pattern")
            .is_some_and(|p| p != "^[^\\u0000-\\u001f\\u007f]+$")
        {
            return Err("unsupported schema pattern".into());
        }
        if s["minLength"]
            .as_u64()
            .is_some_and(|b| (t.chars().count() as u64) < b)
            || s["maxLength"]
                .as_u64()
                .is_some_and(|b| t.chars().count() as u64 > b)
            || (s.get("pattern").is_some()
                && (t.chars().any(char::is_control) || t.trim().is_empty()))
        {
            return Err(format!(
                "{path} {v} must contain {}..{} characters and 0 control characters; use a short nonempty plain name", s["minLength"].as_u64().unwrap_or(0), s["maxLength"].as_u64().unwrap_or(128)
            ));
        }
    }
    if let Some(a) = v.as_array() {
        if s["minItems"].as_u64().is_some_and(|b| (a.len() as u64) < b)
            || s["maxItems"].as_u64().is_some_and(|b| a.len() as u64 > b)
        {
            return Err(format!(
                "{path} {v} must contain {}..{} items; add or remove items to fit",
                s["minItems"].as_u64().unwrap_or(0),
                s["maxItems"].as_u64().unwrap_or(65536)
            ));
        }
        if let Some(items) = s.get("items") {
            for (i, v) in a.iter().enumerate() {
                validate_at(v, items, root, &format!("{path}[{i}]"), depth + 1)?;
            }
        }
    }
    if let Some(o) = v.as_object() {
        for r in s["required"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !o.contains_key(r) {
                return Err(format!(
                    "{path}.{r} <missing> requires 1 value; supply {r} from the request"
                ));
            }
        }
        for (k, v) in o {
            if let Some(child) = s["properties"].get(k) {
                validate_at(v, child, root, &format!("{path}.{k}"), depth + 1)?;
            } else if s["additionalProperties"] == false {
                return Err(format!(
                    "{path}.{k} {v} allows 0 unknown fields; remove {k} and use a listed field"
                ));
            }
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Library {
    pub recipes: Vec<Recipe>,
}
impl Library {
    pub fn embedded() -> Result<Self, String> {
        Self::parse(files::EMBEDDED.iter().map(|(_, text)| *text))
    }
    pub fn parse<'a>(texts: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut recipes = Vec::new();
        let mut ids = BTreeSet::new();
        for text in texts {
            if recipes.len() >= 128 {
                return Err("recipe library exceeds 128 files".into());
            }
            let r = Recipe::parse(text)?;
            if !ids.insert(r.id.clone()) {
                return Err(format!("duplicate recipe id: {}", r.id));
            }
            recipes.push(r);
        }
        if recipes.is_empty() {
            return Err("recipe library is empty".into());
        }
        Ok(Self { recipes })
    }
    pub fn build(
        &self,
        envelope: &Value,
        request: &Value,
        catalog: &Catalog,
    ) -> Result<planner::Plan, String> {
        let o = envelope.as_object().ok_or_else(|| {
            format!("arguments {envelope} requires 1 object; supply {{recipe,args}}")
        })?;
        if o.len() != 2 || !o.contains_key("args") {
            return Err(format!("arguments {envelope} requires exactly 2 fields recipe and args; supply only those fields"));
        }
        let id = envelope["recipe"].as_str().ok_or_else(|| {
            format!(
                "recipe {} requires 1 string; use an id from pilot_find",
                envelope["recipe"]
            )
        })?;
        let r = self.recipes.iter().find(|r| r.id == id).ok_or_else(|| {
            format!(
                "recipe {id:?} must match 1 of {} loaded ids; call pilot_find and use its id",
                self.recipes.len()
            )
        })?;
        let mut supplied = envelope["args"].clone();
        forms::request_defaults(&mut supplied, &r.schema, request)?;
        if r.expander == "joint" {
            planner::joints::coerce(&mut supplied)?;
        }
        if planner::patterns::is_pattern(&r.expander) {
            planner::patterns::coerce(&mut supplied)?;
            planner::patterns::resolve(&r.expander, &mut supplied, request)?;
        }
        let args = forms::to_slots(&supplied, &r.schema)?;
        let typed_schema = schema(&r.expander).unwrap_or_else(|_| r.schema.clone());
        validate(&args, &typed_schema)?;
        if r.expander == "sequence" {
            return sequence(r, &envelope["args"], catalog);
        }
        planner::recipe_expand(&r.expander, &args, request, catalog)
    }
    pub fn find(&self, request: &str, ranker: &dyn Ranker) -> Result<Value, String> {
        self.find_limit(request, ranker, 3)
    }
    fn find_limit(
        &self,
        request: &str,
        ranker: &dyn Ranker,
        limit: usize,
    ) -> Result<Value, String> {
        if request.trim().is_empty() || request.len() > 2048 {
            return Err("request must contain 1..2048 bytes".into());
        }
        let mut ranked = ranker.rank(request, &self.recipes);
        ranked.retain(|(i, s)| {
            *i < self.recipes.len() && s.is_finite() && (self.recipes[*i].id != "joint" || *s > 0.0)
        });
        ranked.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| self.recipes[a.0].id.cmp(&self.recipes[b.0].id))
        });
        ranked.truncate(limit.min(4));
        let hit = ranked.first().is_some_and(|(_, s)| *s >= HIT_THRESHOLD);
        let recipes: Vec<_> = ranked
            .iter()
            .map(|(i, score)| {
                let r = &self.recipes[*i];
                json!({"id":r.id,"description":r.description,"schema":forms::flat(&r.schema),"score":score})
            })
            .collect();
        Ok(
            json!({"hit":hit,"message":if hit {"Choose a recipe and fill its schema through pilot_build."} else {"No good recipe hit. Do not improvise CAD; explain what Pilot can build or ask one question. Closest recipes follow."},"recipes":recipes}),
        )
    }
    /// Refine an existing-part edit after public inspection, leaving ranking calibration intact.
    pub fn form_band_in_design(&self, request: &str, bodies: &[String]) -> Result<Value, String> {
        let mut band = self.form_band(request)?;
        if !bodies.is_empty() && band["band"] != "low" {
            if let Some(id) = creation_form_edit(&band, request) {
                let recipe = self
                    .recipes
                    .iter()
                    .find(|r| r.id == id)
                    .ok_or("edit recipe is unavailable")?;
                let score = band["recipes"][0]["score"].clone();
                band["band"] = json!("fill");
                band["recipes"] = json!([{"id":id,"score":score,"description":recipe.description,"schema":forms::flat(&recipe.schema)}]);
            }
        }
        Ok(band)
    }
    /// One bounded decision shared by the proxy and development calibration.
    pub fn form_band(&self, request: &str) -> Result<Value, String> {
        if request.trim().is_empty() || request.len() > 2048 {
            return Ok(json!({"band":"low","recipes":[]}));
        }
        let mut found = self.find_limit(request, &Bm25, 4)?;
        if let Some(rs) = found["recipes"].as_array_mut() {
            rs.retain(|r| {
                self.recipes
                    .iter()
                    .any(|source| source.id == r["id"] && source.expander != "sequence")
            });
        }
        let words = tokens(request);
        let lower = request.to_lowercase();
        let blocked = [
            "do not make",
            "do not build",
            "do not create",
            "don't make",
            "don't build",
            "don't create",
        ]
        .iter()
        .any(|s| lower.contains(s))
            || words.iter().any(|w| {
                matches!(
                    w.as_str(),
                    "explain"
                        | "describe"
                        | "poem"
                        | "sonnet"
                        | "illustration"
                        | "story"
                        | "compare"
                        | "loft"
                        | "lofted"
                        | "helical"
                        | "gear"
                        | "twisted"
                        | "sweep"
                        | "swept"
                        | "variable"
                )
            });
        let score = found["recipes"][0]["score"].as_f64().unwrap_or(0.0);
        found["band"] = json!(if blocked || score < ROUTE_THRESHOLD {
            "low"
        } else if score >= FILL_THRESHOLD {
            "fill"
        } else {
            "route"
        });
        Ok(found)
    }
}
/// Only creation forms may be replaced after inspection; patterns and joints retain
/// their complete candidate list, score and band.
pub fn creation_form_edit(band: &Value, request: &str) -> Option<&'static str> {
    match band["recipes"][0]["id"].as_str() {
        Some("plate" | "enclosure" | "box" | "cross_drilled_box") => existing_edit(request),
        _ => None,
    }
}
/// An edit verb plus an existing-target cue warrants inspecting the design.
pub fn existing_edit(request: &str) -> Option<&'static str> {
    if request.len() > 2048 {
        return None;
    }
    let lower = request.to_ascii_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    if ["make", "create", "build"].iter().any(|w| has(w))
        || !(["existing", "already", "this", "these"]
            .iter()
            .any(|w| has(w))
            || words.first().is_some_and(|w| {
                matches!(
                    *w,
                    "hollow" | "shell" | "bevel" | "chamfer" | "fillet" | "round"
                )
            }))
    {
        return None;
    }
    if ["chamfer", "bevel"].iter().any(|w| has(w)) {
        Some("chamfer")
    } else if ["fillet", "round"].iter().any(|w| has(w)) {
        Some("fillet")
    } else if ["shell", "hollow"].iter().any(|w| has(w)) {
        Some("shell")
    } else {
        None
    }
}
pub fn find_tool() -> Value {
    json!({"type":"function","function":{"name":"pilot_find","description":FIND_DESCRIPTION,"parameters":{"type":"object","properties":{"request":{"type":"string","minLength":1,"maxLength":2048}},"required":["request"],"additionalProperties":false}}})
}
pub fn build_tool() -> Value {
    let mut t = planner::shortcut_tool();
    t["function"]["description"] = json!(BUILD_DESCRIPTION);
    if let Some(v) = t["function"]["parameters"]["oneOf"].as_array_mut() {
        v.push(json!({"type":"object","properties":{"recipe":{"type":"string"},"args":{"type":"object"}},"required":["recipe","args"],"additionalProperties":false}));
    }
    t
}
/// A local ranker can later be replaced with embeddings without changing the wire tools.
pub trait Ranker {
    fn rank(&self, query: &str, recipes: &[Recipe]) -> Vec<(usize, f64)>;
}
pub struct Bm25;
fn tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|t| {
            if t.is_empty()
                || t.chars().any(|c| c.is_numeric())
                || t.parse::<f64>().is_ok()
                || matches!(
                    t,
                    "mm" | "millimeter"
                        | "millimeters"
                        | "millimetre"
                        | "millimetres"
                        | "cm"
                        | "centimeter"
                        | "centimeters"
                        | "centimetre"
                        | "centimetres"
                        | "m"
                        | "meter"
                        | "meters"
                        | "metre"
                        | "metres"
                        | "ft"
                        | "foot"
                        | "feet"
                        | "deg"
                        | "degree"
                        | "degrees"
                        | "inch"
                        | "inches"
                        | "in"
                        | "a"
                        | "an"
                        | "the"
                        | "it"
                        | "i"
                        | "my"
                        | "me"
                        | "make"
                        | "build"
                        | "create"
                        | "please"
                        | "can"
                        | "you"
                        | "with"
                        | "and"
                        | "of"
                        | "to"
                        | "for"
                        | "by"
                        | "is"
                        | "on"
                        | "at"
                        | "some"
                        | "need"
                        | "want"
                )
            {
                return None;
            }
            let s = match t {
                "rounded" | "rounding" | "round" => "fillet",
                "bevel" | "beveled" | "bevelled" => "chamfer",
                "spherical" => "sphere",
                "cylindrical" => "cylinder",
                "section" => "profile",
                "extruded" => "extrude",
                "swept" => "sweep",
                "translate" | "shift" => "move",
                "reflected" | "reflect" => "mirror",
                _ => t,
            };
            let s = s
                .strip_suffix("ing")
                .filter(|_| s.len() > 6)
                .or_else(|| s.strip_suffix("ed").filter(|_| s.len() > 5))
                .or_else(|| s.strip_suffix('s').filter(|_| s.len() > 4))
                .unwrap_or(s);
            Some(s.into())
        })
        .collect()
}
impl Ranker for Bm25 {
    fn rank(&self, query: &str, recipes: &[Recipe]) -> Vec<(usize, f64)> {
        // Each description/example is a document. Max pooling avoids dilution by varied examples.
        let docs: Vec<Vec<Vec<String>>> = recipes
            .iter()
            .map(|r| {
                // General joints use an independent gate. They contribute no documents,
                // lengths or term frequencies to the calibrated existing recipe index.
                if r.id == "joint" {
                    return Vec::new();
                }
                std::iter::once(r.description.as_str())
                    .chain(r.examples.iter().map(String::as_str))
                    .chain(r.synonyms.iter().map(String::as_str))
                    .map(tokens)
                    .chain(std::iter::once(tokens(
                        &r.schema["properties"]
                            .as_object()
                            .map(|o| {
                                o.keys()
                                    // New form-only choices must not shift the calibrated BM25 corpus.
                                    .filter(|k| {
                                        !(r.id == "enclosure" && k.as_str() == "inside"
                                            || r.id == "profile" && k.as_str() == "centred"
                                            || r.id == "shell" && k.as_str() == "open"
                                            || matches!(r.id.as_str(), "fillet" | "chamfer")
                                                && matches!(k.as_str(), "edges" | "corner_sides"))
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .unwrap_or_default(),
                    )))
                    .collect()
            })
            .collect();
        let n = docs.iter().map(Vec::len).sum::<usize>() as f64;
        let avg = docs.iter().flatten().map(Vec::len).sum::<usize>() as f64 / n.max(1.0);
        let words = tokens(query);
        let has = |word: &str| words.iter().any(|w| w == word);
        let non_pattern = ["clipboard", "file", "poem", "text", "spreadsheet", "volume"]
            .iter()
            .any(|w| has(w));
        let edge_edit = words
            .first()
            .is_some_and(|w| matches!(w.as_str(), "add" | "apply"))
            && words
                .get(1)
                .is_some_and(|w| matches!(w.as_str(), "fillet" | "chamfer"))
            && query
                .split(|c: char| !c.is_alphabetic())
                .any(|w| w.eq_ignore_ascii_case("fillet") || w.eq_ignore_ascii_case("chamfer"));
        // "Add a grid to the plate" targets a definite existing base, even when
        // polite wording precedes the verb. "Add a plate with a grid" creates one.
        let lower = query.to_ascii_lowercase();
        let phrasing: Vec<_> = lower
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty())
            .collect();
        let add_to_existing = has("add")
            && phrasing.windows(3).any(|w| {
                matches!(w[0], "to" | "on")
                    && w[1] == "the"
                    && matches!(w[2], "plate" | "base" | "panel" | "pegboard")
            })
            && !phrasing.windows(3).any(|w| {
                w[0] == "add"
                    && w[1] == "a"
                    && matches!(w[2], "plate" | "base" | "panel" | "pegboard")
            });
        // New base dimensions plus features belong to the compound plate form.
        // Pronouns and copy/repeat verbs keep edits on existing-part forms.
        // Match the ranker's tokens: "existing" loses its "ing" suffix.
        let existing = [
            "that", "this", "exist", "copy", "copie", "repeat", "these", "those", "selected",
            "current", "last", "already", "into",
        ]
        .iter()
        .any(|w| has(w))
            || words
                .first()
                .is_some_and(|w| matches!(w.as_str(), "pattern" | "array"))
            || edge_edit
            || add_to_existing;
        // An explicit grid added to an existing base needs a pattern form even
        // before its counts are supplied. Do not offer a new base's dimensions.
        let existing_grid = existing
            && has("add")
            && has("grid")
            && !has("chamfer")
            && !(has("fillet") && (has("corner") || has("edge")));
        let plate_creation = !existing
            && ["plate", "base", "panel", "pegboard"]
                .iter()
                .any(|w| has(w))
            && [
                "peg", "pegs", "bos", "boss", "bosse", "bosses", "hole", "holes", "bore", "grid",
                "row", "circle",
            ]
            .iter()
            .any(|w| has(w));
        let creation = plate_creation
            || (!existing
                && (has("plate") || has("enclosure"))
                && !["pattern", "array", "repeat", "copy", "copie", "instance"]
                    .iter()
                    .any(|w| has(w)));
        let pattern = if existing_grid {
            Some("grid_pattern")
        } else if creation && planner::pattern_intent(query) != Some("honeycomb_pattern") {
            None
        } else {
            planner::pattern_intent(query)
        };
        let lipped =
            (has("enclosure") || has("wall")) && has("lip") && (has("open") || has("wall"));
        let section = ["l", "t", "u", "c"].iter().any(|w| has(w))
            && (has("profile")
                || ((has("sketch") || has("extrude"))
                    && ["width", "height", "wide", "tall", "leg", "thickness"]
                        .iter()
                        .any(|w| has(w))));
        let general_joint =
            planner::joints::intent(query) && planner::joints::generic_wanted(query);
        let named: BTreeSet<_> = query
            .to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .map(str::to_owned)
            .collect();
        let query: BTreeSet<_> = tokens(query).into_iter().collect();
        let idfs: BTreeMap<_, _> = query
            .iter()
            .map(|t| {
                let df = docs.iter().flatten().filter(|d| d.contains(t)).count() as f64;
                (t, (1.0 + (n - df + 0.5) / (df + 0.5)).ln())
            })
            .collect();
        docs.iter()
            .enumerate()
            .map(|(i, ds)| {
                if recipes[i].id == "joint" {
                    return (i, if general_joint { 100.0 } else { 0.0 });
                }
                if (edge_edit || existing_grid) && recipes[i].id == "plate" {
                    return (i, 0.0);
                }
                // "Align the lid with the box" moves existing parts; it never creates one.
                if words.first().is_some_and(|w| w == "align") {
                    return (i, 0.0);
                }
                let best = ds
                    .iter()
                    .map(|d| {
                        query
                            .iter()
                            .map(|t| {
                                let tf = d.iter().filter(|w| *w == t).count() as f64;
                                idfs[t] * tf * 2.2
                                    / (tf + 1.2 * (0.25 + 0.75 * d.len() as f64 / avg.max(1.0)))
                            })
                            .sum::<f64>()
                    })
                    .fold(0.0, f64::max);
                // Unsupported words reduce confidence instead of letting one shared
                // verb (e.g. "translate French poetry") manufacture a hit.
                let matched = query
                    .iter()
                    .filter(|t| ds.iter().any(|d| d.contains(t)))
                    .count();
                let coverage = matched as f64 / query.len().max(1) as f64;
                let id_hit = if !recipes[i].id.contains('_') && named.contains(&recipes[i].id) {
                    2.0
                } else {
                    0.0
                };
                (
                    i,
                    id_hit
                        + if recipes[i].id.ends_with("_pattern") {
                            if plate_creation || non_pattern {
                                0.0
                            } else if pattern == Some(recipes[i].id.as_str()) {
                                3.0 + best * coverage
                            } else {
                                best * coverage
                            }
                        } else {
                            best * coverage
                                + if recipes[i].id == "plate" && plate_creation {
                                    8.0
                                } else if recipes[i].id == "profile" && section {
                                    6.0
                                } else if recipes[i].id == "enclosure" && lipped {
                                    3.0
                                } else {
                                    0.0
                                }
                        },
                )
            })
            .collect()
    }
}
fn substitute(v: &Value, args: &Value, depth: usize) -> Result<Value, String> {
    if depth > 32 {
        return Err("sequence nesting exceeds 32".into());
    }
    if let Some(o) = v.as_object() {
        if let Some(p) = o.get("$param") {
            if o.len() != 1 {
                return Err("$param must be the only key".into());
            }
            return args
                .get(p.as_str().ok_or("$param must be a name")?)
                .cloned()
                .ok_or("missing sequence parameter".into());
        }
        return o
            .iter()
            .map(|(k, v)| Ok((k.clone(), substitute(v, args, depth + 1)?)))
            .collect::<Result<serde_json::Map<_, _>, String>>()
            .map(Value::Object);
    }
    if let Some(a) = v.as_array() {
        return a
            .iter()
            .map(|v| substitute(v, args, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    Ok(v.clone())
}
fn sequence(r: &Recipe, args: &Value, catalog: &Catalog) -> Result<planner::Plan, String> {
    if r.calls.is_empty() || r.calls.len() > 64 {
        return Err("sequence requires 1..64 calls".into());
    }
    let mut calls = Vec::new();
    for c in &r.calls {
        let c = substitute(c, args, 0)?;
        if c["tool"] != "execute"
            || c.as_object().is_none_or(|o| o.len() != 2)
            || c["arguments"].as_object().is_none_or(|o| o.len() != 2)
        {
            return Err("sequence calls must be execute with command and params".into());
        }
        let id = c["arguments"]["command"]
            .as_str()
            .ok_or("sequence command must be a string")?;
        if catalog.get(id).is_none() {
            return Err(format!("sequence command missing from catalogue: {id}"));
        }
        validate(&c["arguments"]["params"], &catalog.parameter_schema(id))?;
        let params = c["arguments"]["params"]
            .as_object()
            .ok_or("sequence params must be an object")?;
        let command = catalog.get(id).ok_or("sequence command missing")?;
        let documented: Vec<_> = command
            .params
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .collect();
        if params.keys().any(|k| !documented.contains(&k.as_str())) {
            return Err(format!("sequence has unknown parameters for {id}"));
        }
        calls.push(c);
    }
    Ok(planner::Plan {
        rule: "sequence",
        calls,
        facts: None,
        slots: args.clone(),
        expected: None,
        continuation: None,
    })
}

#[cfg(feature = "proxy")]
fn read_file(path: &std::path::Path, cap: u64) -> Result<String, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if bytes.len() as u64 > cap {
        return Err(format!("{} exceeds {cap} bytes", path.display()));
    }
    String::from_utf8(bytes).map_err(|e| format!("{} is not UTF-8: {e}", path.display()))
}
#[cfg(feature = "proxy")]
impl Library {
    pub fn load(dir: Option<&std::path::Path>) -> Result<Self, String> {
        let Some(dir) = dir else {
            return Self::embedded();
        };
        let mut texts = Vec::new();
        for (n, entry) in std::fs::read_dir(dir)
            .map_err(|e| format!("cannot read recipes directory: {e}"))?
            .enumerate()
        {
            if n >= 128 {
                return Err("recipes directory exceeds 128 entries".into());
            }
            let entry = entry.map_err(|e| format!("cannot read recipe entry: {e}"))?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("recipe filename must be UTF-8")?;
            if !name.ends_with(".md") {
                continue;
            }
            if !entry
                .file_type()
                .map_err(|e| format!("cannot inspect recipe entry: {e}"))?
                .is_file()
            {
                return Err("recipe must be a regular file, not a symlink or directory".into());
            }
            texts.push(read_file(&entry.path(), MAX_FILE as u64)?);
        }
        Self::parse(texts.iter().map(String::as_str))
    }
}
/// Draft the nth recorded recipe emission. Review is required before loading it.
#[cfg(feature = "proxy")]
pub fn draft(
    audit: &std::path::Path,
    turn: u32,
    review: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    if !(1..=4096).contains(&turn) {
        return Err("turn must be 1..4096".into());
    }
    let text = read_file(audit, 64 * 1048576)?;
    let mut index = 0;
    for (line_number, line) in text.lines().enumerate() {
        if line_number >= 1_000_000 {
            return Err("audit exceeds 1000000 records".into());
        }
        if line.len() > 8 * 1048576 {
            return Err("audit line exceeds 8 MiB".into());
        }
        let event: Value =
            serde_json::from_str(line).map_err(|e| format!("invalid audit line: {e}"))?;
        if !matches!(event["kind"].as_str(), Some("direct_recipe_calls" | "plan")) {
            continue;
        }
        index += 1;
        if index != turn {
            continue;
        }
        let request = event["after"]["request"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 512)
            .ok_or(
                "audit recipe lacks a bounded request; record a new turn with this Pilot version",
            )?;
        let mut calls = if let Some(c) = event["after"]["calls"].as_array() {
            c.clone()
        } else {
            event["after"]["message"]["tool_calls"]
                .as_array()
                .ok_or("audit lacks recipe calls")?
                .iter()
                .map(|c| {
                    let raw = &c["function"]["arguments"];
                    let args = if let Some(s) = raw.as_str() {
                        serde_json::from_str(s)
                            .map_err(|e| format!("invalid recorded arguments: {e}"))?
                    } else {
                        raw.clone()
                    };
                    Ok(json!({"tool":c["function"]["name"],"arguments":args}))
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        if calls.is_empty() || calls.len() > 64 {
            return Err("draft requires 1..64 calls".into());
        }
        let mut properties = serde_json::Map::new();
        let mut defaults = serde_json::Map::new();
        for (i, c) in calls.iter_mut().enumerate() {
            let params = c
                .get_mut("arguments")
                .and_then(|a| a.get_mut("params"))
                .filter(|p| p.is_object())
                .ok_or("recorded call must have an arguments object with a params object")?;
            infer(
                params,
                &format!("call_{}", i + 1),
                &mut properties,
                &mut defaults,
                0,
            )?;
        }
        let required: Vec<_> = properties.keys().cloned().collect();
        let schema = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false,"$defs":definitions()});
        let mut description = String::new();
        for ch in request
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
        {
            if description.len() + ch.len_utf8() > 256 {
                break;
            }
            description.push(ch);
        }
        let r = Recipe {
            synonyms: vec![],
            id: format!("draft_turn_{turn}"),
            description,
            examples: vec![
                request.into(),
                "Repeat the recorded CAD sequence with updated dimensions.".into(),
                "Build this recorded part with the specified parameters.".into(),
            ],
            schema,
            expander: "sequence".into(),
            calls,
        };
        sequence(&r, &Value::Object(defaults.clone()), &Catalog::bundled()?)?;
        let mut markdown = r.markdown()?;
        markdown.push_str(&format!("\nDraft only: edit examples, name parameters, and review every call and target.\nRecorded parameter values:\n```json\n{}\n```\n",json!(defaults)));
        Recipe::parse(&markdown)?;
        std::fs::create_dir_all(review)
            .map_err(|e| format!("cannot create review directory: {e}"))?;
        let path = review.join(format!("{}.md", r.id));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        file.write_all(markdown.as_bytes())
            .map_err(|e| format!("cannot write draft: {e}"))?;
        return Ok(path);
    }
    Err(format!("audit has no recorded recipe emission {turn}"))
}
#[cfg(feature = "proxy")]
fn infer(
    v: &mut Value,
    name: &str,
    properties: &mut serde_json::Map<String, Value>,
    defaults: &mut serde_json::Map<String, Value>,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("recorded calls exceed nesting 16".into());
    }
    if properties.len() >= 512 {
        return Err("draft exceeds 512 parameters".into());
    }
    if let Some(n) = v.as_f64() {
        if !n.is_finite() || n.abs() > 1000000.0 {
            return Err("recorded number exceeds 1000000".into());
        }
        let field = name.rsplit('_').next().unwrap_or(name);
        let kind = if (field == "angle" || name.ends_with("_angle_deg"))
            && (-360.0..=360.0).contains(&n)
        {
            Some("Angle")
        } else if matches!(field, "count" | "rows" | "columns")
            && n.fract() == 0.0
            && (1.0..=64.0).contains(&n)
        {
            Some("Count")
        } else if matches!(
            field,
            "length"
                | "width"
                | "height"
                | "depth"
                | "thickness"
                | "radius"
                | "diameter"
                | "wall"
                | "distance"
        ) && n > 0.0
        {
            Some("Length")
        } else {
            None
        };
        properties.insert(
            name.into(),
            kind.map_or_else(
                || json!({"type":"number","minimum":-1000000,"maximum":1000000}),
                |kind| json!({"$ref":format!("#/$defs/{kind}")}),
            ),
        );
        defaults.insert(name.into(), v.clone());
        *v = json!({"$param":name});
    } else if let Some(o) = v.as_object_mut() {
        for (k, v) in o {
            infer(v, &format!("{name}_{k}"), properties, defaults, depth + 1)?;
        }
    } else if let Some(a) = v.as_array_mut() {
        for (i, v) in a.iter_mut().enumerate() {
            infer(v, &format!("{name}_{i}"), properties, defaults, depth + 1)?;
        }
    }
    Ok(())
}
