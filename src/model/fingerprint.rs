//! Content-free wire observations. Components use compact serde JSON, as on the wire;
//! options are every top-level request field except messages and tools. Missing fields
//! hash as JSON null. FNV-1a is a comparison aid, not a cryptographic integrity check.
use crate::audit::Audit;
use serde_json::{json, Value};

pub(super) fn fnv1a64(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn component(value: &Value) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| fnv1a64(&bytes))
        .map_err(|e| format!("cannot encode fingerprint component: {e}"))
}

// Bound metadata independently of upstream-controlled name lengths and counts.
fn names(values: &Value) -> Vec<String> {
    values
        .as_array()
        .into_iter()
        .flatten()
        .take(128)
        .filter_map(|value| value["function"]["name"].as_str())
        .map(|name| name.chars().take(128).collect())
        .collect()
}

pub(super) fn request(
    audit: &mut Audit,
    wire: &[u8],
    body: &Value,
    step: &str,
    attempt: usize,
) -> Result<(), String> {
    let mut options = body.clone();
    if let Some(options) = options.as_object_mut() {
        options.remove("messages");
        options.remove("tools");
    }
    audit.record(
        "upstream_request",
        &Value::Null,
        &json!({
            "body_fnv1a64":fnv1a64(wire),
            "messages_fnv1a64":component(&body["messages"])?,
            "tools_fnv1a64":component(&body["tools"])?,
            "options_fnv1a64":component(&options)?,
            "message_count":body["messages"].as_array().map_or(0, Vec::len),
            "tool_count":body["tools"].as_array().map_or(0, Vec::len),
            "tool_names":names(&body["tools"]),
            "step":step,"attempt":attempt,"retry":attempt > 0
        }),
    )
}

pub(super) fn response(
    audit: &mut Audit,
    wire: &[u8],
    streaming: bool,
    attempt: usize,
) -> Result<(), String> {
    let parsed = if streaming {
        super::collect_sse(wire).ok()
    } else {
        serde_json::from_slice::<Value>(wire).ok()
    };
    let message = parsed
        .as_ref()
        .and_then(|body| body.pointer("/choices/0/message"));
    audit.record(
        "upstream_response",
        &Value::Null,
        &json!({
            "body_fnv1a64":fnv1a64(wire),
            "message_fnv1a64":message.map(component).transpose()?,
            "tool_call_names":names(&message.map_or(Value::Null, |m| m["tool_calls"].clone())),
            "attempt":attempt
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_fnv1a64_vectors() {
        assert_eq!(fnv1a64(b""), "cbf29ce484222325");
        assert_eq!(fnv1a64(b"a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a64(b"foobar"), "85944171f73967e8");
    }

    #[test]
    fn metadata_is_bounded() {
        let tools = json!(vec![json!({"function":{"name":"é".repeat(1000)}}); 200]);
        let bounded = names(&tools);
        assert_eq!(bounded.len(), 128);
        assert!(bounded.iter().all(|name| name.chars().count() == 128));
        assert!(serde_json::to_vec(&bounded).is_ok_and(|bytes| bytes.len() < 100_000));
    }
}
