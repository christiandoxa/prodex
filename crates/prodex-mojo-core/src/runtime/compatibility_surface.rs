use crate::MojoError;

const COMPATIBILITY_SURFACE_ABI_VERSION: i64 = 1;
const COMPATIBILITY_SURFACE_MAX_TOOL_LABELS: usize = 1_024;

#[repr(C)]
#[derive(Clone, Copy)]
struct CompatibilityStringView {
    ptr: u64,
    len: u64,
}

const _: () = {
    assert!(std::mem::size_of::<CompatibilityStringView>() == 16);
    assert!(std::mem::align_of::<CompatibilityStringView>() == 8);
};

unsafe extern "C" {
    fn prodex_runtime_compatibility_text_v1(
        abi_version: i64,
        operation: i64,
        flags: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_compatibility_surface_plan_v1(
        abi_version: i64,
        route_kind: i64,
        transport_websocket: i64,
        codex_headers: i64,
        subagent_header: i64,
        internal_origin: i64,
        explicit_stream: i64,
        previous_response_present: i64,
        turn_state_present: i64,
        session_present: i64,
        tools_present: i64,
        user_agent_address: u64,
        user_agent_length: i64,
        tool_views_address: u64,
        tool_count: i64,
        output_address: u64,
    ) -> i64;
}

pub fn compatibility_surface_plan(
    facts: [i64; 10],
    user_agent: &str,
    tool_labels: &[&str],
) -> Result<[i64; 7], MojoError> {
    if tool_labels.len() > COMPATIBILITY_SURFACE_MAX_TOOL_LABELS {
        return Err(MojoError::InvalidInput);
    }
    let tool_views = tool_labels
        .iter()
        .map(|value| CompatibilityStringView {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut output = [0_i64; 7];
    let status = unsafe {
        prodex_runtime_compatibility_surface_plan_v1(
            COMPATIBILITY_SURFACE_ABI_VERSION,
            facts[0],
            facts[1],
            facts[2],
            facts[3],
            facts[4],
            facts[5],
            facts[6],
            facts[7],
            facts[8],
            facts[9],
            user_agent.as_ptr() as usize as u64,
            i64::try_from(user_agent.len()).map_err(|_| MojoError::InvalidInput)?,
            tool_views.as_ptr() as usize as u64,
            i64::try_from(tool_views.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[repr(i64)]
#[derive(Clone, Copy)]
enum CompatibilityTextOperation {
    Tools = 0,
    Continuation = 1,
    LogSafe = 2,
    UserAgent = 3,
}

fn compatibility_text(
    operation: CompatibilityTextOperation,
    flags: i64,
    value: &str,
) -> Result<String, MojoError> {
    // Fixed label vocabulary is shorter than 128 bytes, while both text
    // transforms never expand an input scalar into more than its byte length.
    let capacity = value.len().checked_add(128).ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_compatibility_text_v1(
            COMPATIBILITY_SURFACE_ABI_VERSION,
            operation as i64,
            flags,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
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

/// Mojo owns the ordered tool-surface labels, plus signs and empty marker.
pub fn compatibility_tool_surface(flags: i64) -> Result<String, MojoError> {
    compatibility_text(CompatibilityTextOperation::Tools, flags, "")
}

/// Mojo owns the ordered continuation labels and their empty representation.
pub fn compatibility_continuation_surface(flags: i64) -> Result<String, MojoError> {
    compatibility_text(CompatibilityTextOperation::Continuation, flags, "")
}

/// Mojo replaces each unsafe Unicode scalar with a single underscore; Rust
/// retains only log I/O and the fail-closed, UTF-8-checked ABI transport.
pub fn compatibility_log_safe_value(value: &str) -> Result<String, MojoError> {
    compatibility_text(CompatibilityTextOperation::LogSafe, 0, value)
}

/// Use the same compiled Unicode whitespace owner for the visible user agent.
pub fn compatibility_user_agent_display(value: Option<&str>) -> Result<String, MojoError> {
    compatibility_text(
        CompatibilityTextOperation::UserAgent,
        0,
        value.unwrap_or(""),
    )
}

#[cfg(test)]
mod diagnostic_text_tests {
    use super::*;

    #[test]
    fn compiled_compatibility_field_rendering_preserves_order_and_unicode() {
        for (flags, expected) in [
            (0, "none"),
            (1, "tools"),
            (2, "web"),
            (4, "mcp"),
            (8, "computer"),
            (16, "shell"),
            (32, "approval"),
            (1 | 2 | 16, "shell+tools+web"),
            (63, "approval+computer+mcp+shell+tools+web"),
        ] {
            assert_eq!(compatibility_tool_surface(flags).unwrap(), expected);
        }
        for (flags, expected) in [
            (0, "none"),
            (1, "previous_response"),
            (2, "turn_state"),
            (4, "session"),
            (7, "previous_response+session+turn_state"),
        ] {
            assert_eq!(compatibility_continuation_surface(flags).unwrap(), expected);
        }
        for (value, expected) in [
            ("", "-"),
            ("codex-CLI/v0.5:yes", "codex-CLI/v0.5:yes"),
            ("a b", "a_b"),
            ("a\r\nb", "a__b"),
            ("日本語🧭", "____"),
            ("e\u{301}", "e_"),
            ("a\u{2003}b", "a_b"),
            ("å", "_"),
            ("a\0b", "a_b"),
        ] {
            assert_eq!(
                compatibility_log_safe_value(value).unwrap(),
                expected,
                "{value:?}"
            );
        }
        for (value, expected) in [
            (None, "-"),
            (Some(""), "-"),
            (Some(" \t\u{3000} "), "-"),
            (Some("\u{2003}Codex/1.2\u{3000}"), "Codex/1.2"),
            (Some("Codex Agent 1.2"), "Codex Agent 1.2"),
            (Some(" X \u{2003} Y "), "X \u{2003} Y"),
        ] {
            assert_eq!(
                compatibility_user_agent_display(value).unwrap(),
                expected,
                "{value:?}"
            );
        }
    }

    #[test]
    fn compiled_compatibility_text_abi_rejects_malformed_values_before_writes() {
        let valid = b"hello";
        let input = valid.as_ptr() as usize as u64;
        let mut output = [42_u8; 128];
        let mut written = 34_i64;
        let out = output.as_mut_ptr() as usize as u64;
        let w = (&mut written as *mut i64) as usize as u64;
        let invalid_utf8 = [0xff_u8];
        unsafe {
            assert_eq!(
                prodex_runtime_compatibility_text_v1(2, 0, 0, input, 5, out, 128, w),
                4
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, -1, 0, input, 5, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 0, 64, input, 5, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 1, 8, input, 5, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 2, 1, input, 5, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 2, 0, input, -1, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 2, 0, 0, 5, out, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 2, 0, input, 5, 0, 128, w),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(1, 2, 0, input, 5, out, 128, 0),
                1
            );
            assert_eq!(
                prodex_runtime_compatibility_text_v1(
                    1,
                    2,
                    0,
                    invalid_utf8.as_ptr() as usize as u64,
                    1,
                    out,
                    128,
                    w
                ),
                1
            );
        }
        assert_eq!(output, [42; 128]);
        assert_eq!(written, 34);
    }

    #[test]
    fn compiled_compatibility_text_abi_bounded_output_fails_explicitly() {
        let input = "🌐foo";
        let mut output = [0_u8; 1];
        let mut written = 98_i64;
        assert_eq!(
            unsafe {
                prodex_runtime_compatibility_text_v1(
                    1,
                    2,
                    0,
                    input.as_ptr() as usize as u64,
                    input.len() as i64,
                    output.as_mut_ptr() as usize as u64,
                    1,
                    (&mut written as *mut i64) as usize as u64,
                )
            },
            3
        );
    }
}
