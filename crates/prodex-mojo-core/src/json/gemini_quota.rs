//! Gemini quota and rate-limit semantics over the Serde-acquired JSON tree.
//! Mojo owns traversal and classification; Rust only checks the typed ABI.
use super::{JsonKind, JsonNode, ffi_nodes, signed, status};
use crate::MojoError;

unsafe extern "C" {
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
}
