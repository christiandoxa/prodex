//! Shared measured ABI for compiled DeepSeek function-tool policy.
use super::{JsonNode, StringView, ffi_nodes, signed, status, transform_json_with_views};
use crate::MojoError;

/// Operations over a canonical Serde context containing input, provider and limit.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum DeepSeekFunctionToolOperation {
    ValidateAndDeduplicate = 0,
    StrictSchema = 1,
    Name = 2,
    IsGeneric = 3,
    ObjectName = 4,
    ValidateName = 5,
    ValidateParameters = 6,
    ValidateChoiceName = 7,
    ValidateChoiceTarget = 8,
}

unsafe extern "C" {
    fn prodex_mojo_deepseek_function_tools_scratch_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        required: u64,
    ) -> i64;
    fn prodex_mojo_deepseek_function_tools_v1(
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

/// Returns a transformed value or the original provider diagnostic as JSON.
/// Rust retains canonical Serde acquisition; Mojo owns all validation and selection.
pub fn deepseek_function_tool_policy(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: DeepSeekFunctionToolOperation,
    strict: bool,
) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    let mut required = -1_i64;
    status(unsafe {
        prodex_mojo_deepseek_function_tools_scratch_v1(
            1,
            operation as i64,
            i64::from(strict),
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            raw.as_ptr() as usize as u64,
            signed(raw.len())?,
            (&mut required as *mut i64) as usize as u64,
        )
    })?;
    let required = usize::try_from(required).map_err(|_| MojoError::InvalidOutput)?;
    if required < input.len().saturating_mul(4) || required > i64::MAX as usize / 16 {
        return Err(MojoError::InvalidOutput);
    }
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(required)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(required, StringView::default());
    transform_json_with_views(
        &input,
        raw,
        operation as i64,
        strict,
        prodex_mojo_deepseek_function_tools_v1,
        &mut scratch,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    #[test]
    fn compiled_function_tool_abi_rejects_version_and_invalid_context_without_output() {
        let mut output = [71_i64; 2];
        let address = output.as_mut_ptr() as usize as u64;
        assert_eq!(
            unsafe {
                prodex_mojo_deepseek_function_tools_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address)
            },
            4
        );
        assert_eq!(
            unsafe { prodex_mojo_deepseek_function_tools_scratch_v1(2, 0, 0, 0, 0, 0, 0, address) },
            4
        );
        assert_eq!(output, [71; 2]);
        let root = JsonNode {
            kind: JsonKind::Object,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 2,
        };
        assert_eq!(
            deepseek_function_tool_policy(
                &[root],
                "{}",
                DeepSeekFunctionToolOperation::Name,
                false
            ),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_function_tool_abi_checks_scratch_capacity_flags_and_output_metadata() {
        let raw = r#"{"input":[],"max_name_bytes":64,"provider_label":"DeepSeek"}"#;
        let root = JsonNode {
            kind: JsonKind::Object,
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: raw.len(),
        };
        let nodes = [
            root,
            JsonNode {
                kind: JsonKind::Array,
                first_child: None,
                next_sibling: Some(2),
                parent: Some(0),
                key: "input",
                text: "",
                raw_start: raw.find("[]").unwrap(),
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: Some(3),
                parent: Some(0),
                key: "max_name_bytes",
                text: "",
                raw_start: raw.find("64").unwrap(),
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(0),
                key: "provider_label",
                text: "DeepSeek",
                raw_start: raw.find("\"DeepSeek\"").unwrap(),
                raw_length: 10,
            },
        ];
        assert_eq!(
            deepseek_function_tool_policy(
                &nodes,
                raw,
                DeepSeekFunctionToolOperation::ValidateAndDeduplicate,
                true
            )
            .unwrap(),
            br#"{"value":[]}"#
        );
        let input = ffi_nodes(&nodes, raw).unwrap();
        let mut scratch = [StringView::default(); 16];
        let mut metadata = [71_i64; 2];
        let mut output = [83_u8; 32];
        for (operation, flag, units, capacity, expected) in [
            (9, 0, 16, 32, 1),
            (0, 2, 16, 32, 1),
            (0, 0, 15, 32, 1),
            (0, 0, 16, 1, 3),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_deepseek_function_tools_v1(
                        1,
                        operation,
                        flag,
                        input.as_ptr() as usize as u64,
                        input.len() as i64,
                        raw.as_ptr() as usize as u64,
                        raw.len() as i64,
                        scratch.as_mut_ptr() as usize as u64,
                        units,
                        0,
                        output.as_mut_ptr() as usize as u64,
                        capacity,
                        metadata.as_mut_ptr() as usize as u64,
                    )
                },
                expected
            );
            assert_eq!(metadata, [71; 2]);
        }
    }
}
