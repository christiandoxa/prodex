//! Compiled DeepSeek/Kiro tool-shape issue wording and strict C ABI.
//! The host supplies provider label and detail, never the validation message.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_deepseek_tool_shape_issue_message_v1(
        abi_version: i64,
        issue: i64,
        provider_address: u64,
        provider_length: i64,
        detail_address: u64,
        detail_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Renders a tool-shape error selected by the compiled request-policy kernel.
pub fn deepseek_tool_shape_issue_message(
    issue: i64,
    provider_label: &str,
    detail: &str,
) -> Result<String, MojoError> {
    let capacity = provider_label
        .len()
        .checked_add(detail.len())
        .and_then(|n| n.checked_add(256))
        .ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_deepseek_tool_shape_issue_message_v1(
            1,
            issue,
            provider_label.as_ptr() as usize as u64,
            i64::try_from(provider_label.len()).map_err(|_| MojoError::InvalidInput)?,
            detail.as_ptr() as usize as u64,
            i64::try_from(detail.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_tool_shape_messages_cover_all_issue_codes_and_provider_labels() {
        let cases: &[(i64, &str)] = &[
            (1, " tools must be an array"),
            (2, " tools entries must be objects"),
            (3, " tool type `` is not supported"),
            (4, " tool description must be a string"),
            (5, " function description must be a string"),
            (6, " function strict must be a boolean"),
            (7, " tool strict must be a boolean"),
            (8, "  tools require a name"),
            (
                9,
                " custom tools cannot preserve strict=true through the function wrapper",
            ),
            (10, " custom tool format must be an object"),
            (11, " custom tool format.type must be a string"),
            (12, " namespace tools require a name"),
            (13, " namespace tool `` requires a tools array"),
            (14, " namespace tool `` requires at least one tool"),
            (15, " namespace tool `` entries must be objects"),
            (16, " namespace function description must be a string"),
            (17, " namespace function strict must be a boolean"),
            (18, " namespace tool `` entries must be function tools"),
            (19, " namespace tool `` function entries require a name"),
            (20, " MCP function tools require a name"),
            (21, " MCP function tool `` requires a schema"),
            (22, " MCP toolsets require a server name"),
            (
                23,
                " MCP toolset `` requires allowed_tools or enabled configs",
            ),
            (24, " tools shape validation returned an unknown result"),
        ];
        for provider in ["DeepSeek", "Kiro", "Provider🔥"] {
            for &(issue, suffix) in cases {
                assert_eq!(
                    deepseek_tool_shape_issue_message(issue, provider, "").unwrap(),
                    format!("{provider}{suffix}"),
                    "{provider:?}/{issue}",
                );
            }
        }
        for (issue, provider, detail, expected) in [
            (
                3,
                "DeepSeek",
                "file_search",
                "DeepSeek tool type `file_search` is not supported",
            ),
            (8, "Kiro", "mcp", "Kiro mcp tools require a name"),
            (
                13,
                "Kiro",
                "ns 東京🧭",
                "Kiro namespace tool `ns 東京🧭` requires a tools array",
            ),
            (
                14,
                "Kiro",
                "ns",
                "Kiro namespace tool `ns` requires at least one tool",
            ),
            (
                15,
                "Kiro",
                "ns",
                "Kiro namespace tool `ns` entries must be objects",
            ),
            (
                18,
                "Kiro",
                "ns",
                "Kiro namespace tool `ns` entries must be function tools",
            ),
            (
                19,
                "Kiro",
                "ns",
                "Kiro namespace tool `ns` function entries require a name",
            ),
            (
                21,
                "DeepSeek",
                "tool",
                "DeepSeek MCP function tool `tool` requires a schema",
            ),
            (
                23,
                "DeepSeek",
                "server",
                "DeepSeek MCP toolset `server` requires allowed_tools or enabled configs",
            ),
        ] {
            assert_eq!(
                deepseek_tool_shape_issue_message(issue, provider, detail).unwrap(),
                expected,
                "{issue}/{detail:?}",
            );
        }
    }

    #[test]
    fn compiled_tool_shape_message_abi_rejects_malformed_inputs_without_writes() {
        let provider = b"Kiro";
        let detail = b"server";
        let provider_ptr = provider.as_ptr() as usize as u64;
        let detail_ptr = detail.as_ptr() as usize as u64;
        let mut buffer = [57_u8; 150];
        let mut written = 89_i64;
        let target = buffer.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        let call = |version,
                    issue,
                    provider_ptr,
                    provider_length,
                    detail_ptr,
                    detail_length,
                    output,
                    capacity,
                    written_ptr| unsafe {
            prodex_deepseek_tool_shape_issue_message_v1(
                version,
                issue,
                provider_ptr,
                provider_length,
                detail_ptr,
                detail_length,
                output,
                capacity,
                written_ptr,
            )
        };
        assert_eq!(
            call(2, 23, provider_ptr, 4, detail_ptr, 6, target, 150, count),
            4
        );
        assert_eq!(
            call(1, 0, provider_ptr, 4, detail_ptr, 6, target, 150, count),
            1
        );
        assert_eq!(call(1, 23, 0, 4, detail_ptr, 6, target, 150, count), 1);
        assert_eq!(
            call(1, 23, provider_ptr, -1, detail_ptr, 6, target, 150, count),
            1
        );
        assert_eq!(call(1, 23, provider_ptr, 4, 0, 6, target, 150, count), 1);
        assert_eq!(
            call(1, 23, provider_ptr, 4, detail_ptr, -1, target, 150, count),
            1
        );
        assert_eq!(
            call(1, 23, provider_ptr, 4, detail_ptr, 6, 0, 150, count),
            1
        );
        assert_eq!(
            call(1, 23, provider_ptr, 4, detail_ptr, 6, target, 150, 0),
            1
        );
        assert_eq!(
            call(1, 23, provider_ptr, 4, detail_ptr, 6, target, 1, count),
            3
        );
        let invalid = [0xff_u8];
        assert_eq!(
            call(
                1,
                23,
                invalid.as_ptr() as usize as u64,
                1,
                detail_ptr,
                6,
                target,
                150,
                count
            ),
            1
        );
        assert_eq!(buffer, [57; 150]);
        assert_eq!(written, 89);
        assert_eq!(
            deepseek_tool_shape_issue_message(0, "Kiro", ""),
            Err(MojoError::InvalidInput)
        );
    }
}
