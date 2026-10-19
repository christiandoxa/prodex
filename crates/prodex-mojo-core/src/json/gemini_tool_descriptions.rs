//! Measured, caller-owned ABI for Gemini declaration description overrides.
use super::{JsonNode, StringView, ffi_nodes, transform_json_with_views};
use crate::MojoError;

/// Operations over a description catalog, tool-name string, or declaration context.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum GeminiToolDescriptionOperation {
    Catalog = 0,
    DescriptionIndex = 1,
    ApplyOverrides = 2,
}

unsafe extern "C" {
    fn prodex_mojo_gemini_tool_descriptions_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Mojo selects aliases, model eligibility and complete parameter-description edits.
pub fn gemini_tool_description_policy(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiToolDescriptionOperation,
) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    // A decoded name cannot exceed its canonical JSON document's byte length.
    let units = raw.len().div_ceil(16).max(1);
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(units)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(units, StringView::default());
    transform_json_with_views(
        &input,
        raw,
        operation as i64,
        false,
        prodex_mojo_gemini_tool_descriptions_v1,
        &mut scratch,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    #[test]
    fn compiled_description_abi_checks_version_flags_scratch_and_output_capacity() {
        let raw = "\"Read-File\"";
        let nodes = [JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "Read-File",
            raw_start: 0,
            raw_length: raw.len(),
        }];
        assert_eq!(
            gemini_tool_description_policy(
                &nodes,
                raw,
                GeminiToolDescriptionOperation::DescriptionIndex
            ),
            Ok(b"0".to_vec())
        );
        let input = ffi_nodes(&nodes, raw).unwrap();
        let mut scratch = [StringView::default(); 1];
        let mut output = [83_u8; 32];
        let mut metadata = [71_i64; 2];
        for (abi, operation, flag, units, measuring, capacity, expected) in [
            (2, 1, 0, 1, 0, 32, 4),
            (1, 3, 0, 1, 0, 32, 1),
            (1, 1, 1, 1, 0, 32, 1),
            (1, 1, 0, 0, 0, 32, 1),
            (1, 1, 0, 1, 2, 32, 1),
            (1, 1, 0, 1, 0, 0, 3),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_gemini_tool_descriptions_v1(
                        abi,
                        operation,
                        flag,
                        input.as_ptr() as u64,
                        input.len() as i64,
                        raw.as_ptr() as u64,
                        raw.len() as i64,
                        scratch.as_mut_ptr() as u64,
                        units,
                        measuring,
                        output.as_mut_ptr() as u64,
                        capacity,
                        metadata.as_mut_ptr() as u64,
                    )
                },
                expected
            );
            assert_eq!(metadata, [71; 2]);
            assert_eq!(output, [83; 32]);
        }
    }
}
