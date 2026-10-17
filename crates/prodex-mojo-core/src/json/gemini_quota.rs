//! Gemini quota and rate-limit semantics over the Serde-acquired JSON tree.
//! Mojo owns traversal and classification; Rust only checks the typed ABI.
use super::{JsonKind, JsonNode, ffi_nodes, signed, status};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_gemini_retry_candidates_v1(
        abi_version: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        candidate_address: u64,
        candidate_capacity: i64,
        written_address: u64,
        floor_address: u64,
    ) -> i64;
    fn prodex_gemini_retry_delay_max_v1(
        abi_version: i64,
        candidate_address: u64,
        candidate_count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_gemini_quota_body_plan_v1(
        abi_version: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        scratch_address: u64,
        scratch_count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiQuotaBodyPlan {
    pub terminal_quota: bool,
    pub rate_limit: bool,
    pub quota_message: Option<String>,
}

pub fn gemini_quota_body_plan(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<GeminiQuotaBodyPlan, MojoError> {
    let ffi = ffi_nodes(nodes, raw)?;
    let mut scratch = vec![-1_i64; ffi.len()];
    let mut result = [-3_i64; 3];
    status(unsafe {
        prodex_gemini_quota_body_plan_v1(
            1,
            ffi.as_ptr() as usize as u64,
            signed(ffi.len())?,
            raw.as_ptr() as usize as u64,
            signed(raw.len())?,
            scratch.as_mut_ptr() as usize as u64,
            signed(scratch.len())?,
            result.as_mut_ptr() as usize as u64,
            3,
        )
    })?;
    let bool_value = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let quota_message = match result[2] {
        -2 => None,
        -1 => Some("Gemini account quota was exhausted.".to_string()),
        index if index >= 0 => {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            let node = nodes.get(index).ok_or(MojoError::InvalidOutput)?;
            if !matches!(node.kind, JsonKind::String) {
                return Err(MojoError::InvalidOutput);
            }
            Some(node.text.to_string())
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(GeminiQuotaBodyPlan {
        terminal_quota: bool_value(result[0])?,
        rate_limit: bool_value(result[1])?,
        quota_message,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiRetryCandidateKind {
    ExplicitDuration,
    MessageText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiRetryCandidate<'a> {
    pub kind: GeminiRetryCandidateKind,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiRetryCandidates<'a> {
    pub floor_ms: u64,
    pub fields: Vec<GeminiRetryCandidate<'a>>,
}

/// Mojo owns nested candidate extraction and rate/minute minimums. Only the
/// standard Rust f64 duration grammar remains at the host input boundary.
pub fn gemini_retry_candidates<'a>(
    nodes: &'a [JsonNode<'a>],
    raw: &str,
) -> Result<GeminiRetryCandidates<'a>, MojoError> {
    let ffi = ffi_nodes(nodes, raw)?;
    let capacity = nodes.len().checked_mul(2).ok_or(MojoError::Capacity)?;
    let mut fields = vec![-1_i64; capacity];
    let mut written = -1_i64;
    let mut floor = u64::MAX;
    status(unsafe {
        prodex_gemini_retry_candidates_v1(
            1,
            ffi.as_ptr() as usize as u64,
            signed(ffi.len())?,
            raw.as_ptr() as usize as u64,
            signed(raw.len())?,
            fields.as_mut_ptr() as usize as u64,
            signed(fields.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut floor as *mut u64) as usize as u64,
        )
    })?;
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > nodes.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut result = Vec::with_capacity(count);
    for chunk in fields[..count * 2].chunks_exact(2) {
        let index = usize::try_from(chunk[0]).map_err(|_| MojoError::InvalidOutput)?;
        let node = nodes.get(index).ok_or(MojoError::InvalidOutput)?;
        if !matches!(node.kind, JsonKind::String) {
            return Err(MojoError::InvalidOutput);
        }
        let kind = match chunk[1] {
            0 if node.key == "retryDelay" => GeminiRetryCandidateKind::ExplicitDuration,
            1 if matches!(node.key, "message" | "detail" | "error") => {
                GeminiRetryCandidateKind::MessageText
            }
            _ => return Err(MojoError::InvalidOutput),
        };
        result.push(GeminiRetryCandidate {
            kind,
            text: node.text,
        });
    }
    Ok(GeminiRetryCandidates {
        floor_ms: floor,
        fields: result,
    })
}

/// Mojo chooses the final positive retry-delay maximum across every parsed
/// upstream SSE frame and the compiled quota-code minimum delays.
pub fn gemini_retry_delay_max(candidates_ms: &[u64]) -> Result<Option<u64>, MojoError> {
    let mut best = u64::MAX;
    status(unsafe {
        prodex_gemini_retry_delay_max_v1(
            1,
            candidates_ms.as_ptr() as usize as u64,
            signed(candidates_ms.len())?,
            (&mut best as *mut u64) as usize as u64,
        )
    })?;
    Ok((best > 0).then_some(best))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    fn tree<'a>(code: &'a str, message: &'a str, code_key: &'a str) -> Vec<JsonNode<'a>> {
        vec![
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(1),
                next_sibling: None,
                parent: None,
                key: "",
                text: "",
                raw_start: 0,
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(2),
                next_sibling: None,
                parent: Some(0),
                key: "error",
                text: "",
                raw_start: 0,
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(3),
                parent: Some(1),
                key: code_key,
                text: code,
                raw_start: 0,
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(1),
                key: "message",
                text: message,
                raw_start: 0,
                raw_length: 2,
            },
        ]
    }

    #[test]
    fn mojo_quota_body_classifies_nested_terminal_rate_and_message() {
        assert_eq!(
            gemini_quota_body_plan(&tree(" RESOURCE_EXHAUSTED ", "limited 🧭", "code"), "{}"),
            Ok(GeminiQuotaBodyPlan {
                terminal_quota: true,
                rate_limit: false,
                quota_message: Some("limited 🧭".to_string()),
            }),
        );
        assert_eq!(
            gemini_quota_body_plan(&tree(" Rate_Limit_Exceeded ", "slow", "status"), "{}"),
            Ok(GeminiQuotaBodyPlan {
                terminal_quota: false,
                rate_limit: true,
                quota_message: Some("slow".to_string()),
            }),
        );
        assert_eq!(
            gemini_quota_body_plan(&tree("ok", "hello", "code"), "{}"),
            Ok(GeminiQuotaBodyPlan {
                terminal_quota: false,
                rate_limit: false,
                quota_message: None,
            }),
        );
    }

    #[test]
    fn mojo_quota_body_abi_rejects_bad_views_and_buffers_without_output() {
        let nodes = tree(" RESOURCE_EXHAUSTED ", "hello", "code");
        let ffi = ffi_nodes(&nodes, "{}").unwrap();
        let mut scratch = [-1_i64; 4];
        let mut output = [57_i64; 3];
        let ptr = ffi.as_ptr() as usize as u64;
        let scratch_ptr = scratch.as_mut_ptr() as usize as u64;
        let out = output.as_mut_ptr() as usize as u64;
        let raw = b"{}";
        let call = |version, nodes, count, scratch, scratch_cap, out, cap| unsafe {
            prodex_gemini_quota_body_plan_v1(
                version,
                nodes,
                count,
                raw.as_ptr() as usize as u64,
                2,
                scratch,
                scratch_cap,
                out,
                cap,
            )
        };
        assert_eq!(call(2, ptr, 4, scratch_ptr, 4, out, 3), 4);
        assert_eq!(call(1, 0, 4, scratch_ptr, 4, out, 3), 1);
        assert_eq!(call(1, ptr, 0, scratch_ptr, 4, out, 3), 1);
        assert_eq!(call(1, ptr, 4, 0, 4, out, 3), 1);
        assert_eq!(call(1, ptr, 4, scratch_ptr, 3, out, 3), 1);
        assert_eq!(call(1, ptr, 4, scratch_ptr, 4, 0, 3), 1);
        assert_eq!(call(1, ptr, 4, scratch_ptr, 4, out, 2), 1);
        assert_eq!(output, [57; 3]);
    }

    #[test]
    fn mojo_retry_candidates_extract_nested_message_and_rate_floor() {
        let nodes = tree(" rate_limit_exceeded ", "please retry in 1.5s", "code");
        let plan = gemini_retry_candidates(&nodes, "{}").unwrap();
        assert_eq!(plan.floor_ms, 10_000);
        assert_eq!(
            plan.fields,
            vec![GeminiRetryCandidate {
                kind: GeminiRetryCandidateKind::MessageText,
                text: "please retry in 1.5s",
            },]
        );
        assert_eq!(
            gemini_retry_delay_max(&[0, 1500, plan.floor_ms]),
            Ok(Some(10_000))
        );
        assert_eq!(
            gemini_retry_delay_max(&[60_000, 120_500, 9]),
            Ok(Some(120_500))
        );
        assert_eq!(gemini_retry_delay_max(&[]), Ok(None));
        assert_eq!(gemini_retry_delay_max(&[0, 0]), Ok(None));
        assert_eq!(gemini_retry_delay_max(&[u64::MAX, 2]), Ok(Some(u64::MAX)));
    }

    #[test]
    fn mojo_retry_candidate_and_max_abis_reject_invalid_shapes_before_writes() {
        let nodes = tree("rate_limit_exceeded", "slow", "code");
        let ffi = ffi_nodes(&nodes, "{}").unwrap();
        let ptr = ffi.as_ptr() as usize as u64;
        let raw = b"{}";
        let mut candidates = [97_i64; 8];
        let mut written = 17_i64;
        let mut floor = 23_u64;
        let fields_ptr = candidates.as_mut_ptr() as usize as u64;
        let written_ptr = (&mut written as *mut i64) as usize as u64;
        let floor_ptr = (&mut floor as *mut u64) as usize as u64;
        let call = |version, nodes, count, fields, cap, out, floor| unsafe {
            prodex_gemini_retry_candidates_v1(
                version,
                nodes,
                count,
                raw.as_ptr() as usize as u64,
                2,
                fields,
                cap,
                out,
                floor,
            )
        };
        assert_eq!(call(2, ptr, 4, fields_ptr, 8, written_ptr, floor_ptr), 4);
        assert_eq!(call(1, 0, 4, fields_ptr, 8, written_ptr, floor_ptr), 1);
        assert_eq!(call(1, ptr, 0, fields_ptr, 8, written_ptr, floor_ptr), 1);
        assert_eq!(call(1, ptr, 4, 0, 8, written_ptr, floor_ptr), 1);
        assert_eq!(call(1, ptr, 4, fields_ptr, 7, written_ptr, floor_ptr), 1);
        assert_eq!(call(1, ptr, 4, fields_ptr, 8, 0, floor_ptr), 1);
        assert_eq!(call(1, ptr, 4, fields_ptr, 8, written_ptr, 0), 1);
        assert_eq!(candidates, [97; 8]);
        assert_eq!(written, 17);
        assert_eq!(floor, 23);
        let values = [0_u64, 500];
        let data = values.as_ptr() as usize as u64;
        let mut max = 111_u64;
        let max_ptr = (&mut max as *mut u64) as usize as u64;
        let maximum = |version, ptr, count, out| unsafe {
            prodex_gemini_retry_delay_max_v1(version, ptr, count, out)
        };
        assert_eq!(maximum(2, data, 2, max_ptr), 4);
        assert_eq!(maximum(1, data, -1, max_ptr), 1);
        assert_eq!(maximum(1, 0, 2, max_ptr), 1);
        assert_eq!(maximum(1, data, 2, 0), 1);
        assert_eq!(max, 111);
    }
}
