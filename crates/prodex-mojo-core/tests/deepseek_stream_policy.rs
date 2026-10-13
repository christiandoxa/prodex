use prodex_mojo_core::deepseek_stream_policy::{
    DeepSeekStreamToolArgumentsPlan, stream_tool_arguments_policy,
};

#[test]
fn compiled_mojo_validates_streamed_name_and_arguments() {
    assert_eq!(
        stream_tool_arguments_policy(Some(" \u{2003}shell\u{3000} "), " {\"cmd\":1} ").unwrap(),
        DeepSeekStreamToolArgumentsPlan::Valid
    );
    assert_eq!(
        stream_tool_arguments_policy(Some(" \u{2003} \u{3000}"), "{} ").unwrap(),
        DeepSeekStreamToolArgumentsPlan::MissingName
    );
    assert_eq!(
        stream_tool_arguments_policy(Some("shell"), "{bad").unwrap(),
        DeepSeekStreamToolArgumentsPlan::InvalidArguments
    );
    assert_eq!(
        stream_tool_arguments_policy(Some("shell"), "{\"x\":\"\\uZZZZ\"}").unwrap(),
        DeepSeekStreamToolArgumentsPlan::InvalidArguments
    );
    assert_eq!(
        stream_tool_arguments_policy(Some("shell"), "   ").unwrap(),
        DeepSeekStreamToolArgumentsPlan::Valid
    );
}

#[test]
fn compiled_mojo_rejects_abi_and_pointer_contract_violations() {
    assert_eq!(
        stream_tool_arguments_policy(None, "{}").unwrap(),
        DeepSeekStreamToolArgumentsPlan::MissingName
    );
    assert_eq!(
        stream_tool_arguments_policy(Some("shell"), "null").unwrap(),
        DeepSeekStreamToolArgumentsPlan::Valid
    );
}
