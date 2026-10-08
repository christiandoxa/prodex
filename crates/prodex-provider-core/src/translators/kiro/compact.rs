//! Kiro remote compact request/response adapters.

use prodex_mojo_core::rich::{
    kiro_semantic_compact_instructions, kiro_semantic_compact_request_json,
    kiro_semantic_compact_summary_json,
};
use serde_json::Value;
use std::sync::OnceLock;

const KIRO_COMPACT_PARSE_ERROR: &str = "failed to parse Kiro compact request JSON";
const KIRO_COMPACT_OBJECT_ERROR: &str = "Kiro compact request must be a JSON object";
const KIRO_COMPACT_INPUT_ERROR: &str = "Kiro compact request must contain an input array";
const KIRO_COMPACT_OUTPUT_ERROR: &str = "Kiro compact response is missing output";
const KIRO_COMPACT_SUMMARY_ERROR: &str = "Kiro compact response returned no summary text";

pub fn kiro_provider_core_semantic_compact_instructions() -> &'static str {
    static INSTRUCTIONS: OnceLock<String> = OnceLock::new();
    INSTRUCTIONS
        .get_or_init(|| {
            let body = kiro_semantic_compact_instructions()
                .unwrap_or_else(|error| panic!("Mojo Kiro compact instructions failed: {error:?}"));
            serde_json::from_slice(&body).unwrap_or_else(|error| {
                panic!("Mojo Kiro compact instructions were invalid: {error}")
            })
        })
        .as_str()
}

pub fn kiro_provider_core_semantic_compact_request_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| format!("{KIRO_COMPACT_PARSE_ERROR}: {error}"))?;
    let input = serde_json::to_string(&value)
        .map_err(|error| format!("failed to serialize Kiro compact request: {error}"))?;
    let output = kiro_semantic_compact_request_json(&input)
        .map_err(|_| KIRO_COMPACT_PARSE_ERROR.to_string())?;
    match output.as_slice() {
        b"Eparse" => Err(KIRO_COMPACT_PARSE_ERROR.to_string()),
        b"Eobject" => Err(KIRO_COMPACT_OBJECT_ERROR.to_string()),
        b"Einput" => Err(KIRO_COMPACT_INPUT_ERROR.to_string()),
        _ => {
            let value = serde_json::from_slice::<Value>(&output)
                .map_err(|_| "Mojo returned invalid Kiro compact request JSON".to_string())?;
            serde_json::to_vec(&value)
                .map_err(|error| format!("failed to serialize Kiro compact request: {error}"))
        }
    }
}

pub fn kiro_provider_core_compact_summary_from_response(
    response: &Value,
) -> Result<String, String> {
    let canonical = serde_json::to_string(response)
        .map_err(|_| "failed to serialize Kiro compact response JSON".to_string())?;
    let output = kiro_semantic_compact_summary_json(&canonical)
        .map_err(|_| KIRO_COMPACT_SUMMARY_ERROR.to_string())?;
    match output.as_slice() {
        b"Eoutput" => Err(KIRO_COMPACT_OUTPUT_ERROR.to_string()),
        b"Esummary" => Err(KIRO_COMPACT_SUMMARY_ERROR.to_string()),
        _ => serde_json::from_slice(&output).map_err(|_| KIRO_COMPACT_SUMMARY_ERROR.to_string()),
    }
}
