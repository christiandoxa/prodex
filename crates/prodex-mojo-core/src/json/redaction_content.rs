//! One compiled selection plan shared by JSON inspection and masking.
use super::{JsonNode, StringView, ffi_nodes, transform_json_with_views};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_redaction_json_content_plan_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Returns a JSON plan of bounded string slots, sanitized paths, and coverage.
/// The caller validates slot indices against the same borrowed input tree.
pub fn redaction_json_content_plan(nodes: &[JsonNode<'_>]) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, "")?;
    // ABI v1 reserves 64 sixteen-byte units for the bounded display path.
    let mut scratch = [StringView::default(); 64];
    transform_json_with_views(
        &input,
        "",
        0,
        false,
        prodex_redaction_json_content_plan_v1,
        &mut scratch,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    #[test]
    fn compiled_redaction_plan_preserves_empty_and_uninspected_roots() {
        for kind in [
            JsonKind::Null,
            JsonKind::String,
            JsonKind::Array,
            JsonKind::Object,
        ] {
            let root = JsonNode {
                kind,
                first_child: None,
                next_sibling: None,
                parent: None,
                key: "",
                text: "opaque root",
                raw_start: 0,
                raw_length: 0,
            };
            assert_eq!(
                redaction_json_content_plan(&[root]).unwrap(),
                br#"{"slots":[],"coverage":"unsupported","error":0}"#,
            );
        }
    }

    #[test]
    fn compiled_redaction_plan_rejects_invalid_tree_and_abi_before_publishing() {
        assert_eq!(
            redaction_json_content_plan(&[]),
            Err(MojoError::InvalidInput)
        );
        let root = JsonNode {
            kind: JsonKind::Array,
            first_child: Some(0),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 0,
        };
        assert_eq!(
            redaction_json_content_plan(&[root]),
            Err(MojoError::InvalidInput)
        );
        let mut metadata = [71_i64; 2];
        assert_eq!(
            unsafe {
                prodex_redaction_json_content_plan_v1(
                    2,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    metadata.as_mut_ptr() as usize as u64,
                )
            },
            4
        );
        assert_eq!(metadata, [71; 2]);
    }
}
