#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]

use prodex_mojo_core::MojoError;
use prodex_mojo_core::rich::{RICH_ABI_VERSION, kiro_strip_external_tool_controls_json};

unsafe extern "C" {
    fn prodex_mojo_kiro_external_tool_controls_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        sub_agent: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[test]
fn compiled_mojo_preserves_surviving_kiro_request_order_and_unicode() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    let input = r#"{"model":"claude-模型","input":"café 🐈","tools":[{"name":"shell"}],"functions":[{"name":"shell"}],"parallel_tool_calls":true,"tool_choice":"auto","metadata":{"z":1}}"#;
    assert_eq!(
        kiro_strip_external_tool_controls_json(input, false).unwrap(),
        r#"{"model":"claude-模型","input":"café 🐈","parallel_tool_calls":true,"tool_choice":"auto","metadata":{"z":1}}"#
            .as_bytes()
            .to_vec()
    );
}

#[test]
fn compiled_mojo_removes_all_sub_agent_external_tool_controls() {
    let input = r#"{"model":"auto","input":"inspect","metadata":{"scope":"界"},"parallel_tool_calls":true,"tool_choice":"auto","tools":[],"functions":[],"function_call":"auto","web_search_options":{}}"#;
    assert_eq!(
        kiro_strip_external_tool_controls_json(input, true).unwrap(),
        r#"{"model":"auto","input":"inspect","metadata":{"scope":"界"}}"#
            .as_bytes()
            .to_vec()
    );
}

#[test]
fn compiled_mojo_rejects_invalid_kiro_request_controls_input() {
    assert_eq!(
        kiro_strip_external_tool_controls_json("[]", false),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        kiro_strip_external_tool_controls_json(r#"{"input":}"#, false),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_mojo_kiro_request_controls_abi_rejects_bad_contracts() {
    let input = br#"{}"#;
    let mut output = [0_u8; 2];
    let mut written = -7_i64;

    assert_eq!(
        unsafe {
            prodex_mojo_kiro_external_tool_controls_v1(
                RICH_ABI_VERSION + 1,
                input.as_ptr() as u64,
                input.len() as i64,
                0,
                output.as_mut_ptr() as u64,
                output.len() as i64,
                (&mut written as *mut i64) as u64,
            )
        },
        4
    );
    assert_eq!(written, -7);

    assert_eq!(
        unsafe {
            prodex_mojo_kiro_external_tool_controls_v1(
                RICH_ABI_VERSION,
                input.as_ptr() as u64,
                input.len() as i64,
                2,
                output.as_mut_ptr() as u64,
                output.len() as i64,
                (&mut written as *mut i64) as u64,
            )
        },
        1
    );
    assert_eq!(written, -7);

    assert_eq!(
        unsafe {
            prodex_mojo_kiro_external_tool_controls_v1(
                RICH_ABI_VERSION,
                input.as_ptr() as u64,
                input.len() as i64,
                0,
                0,
                output.len() as i64,
                (&mut written as *mut i64) as u64,
            )
        },
        1
    );
    assert_eq!(written, -7);
}
