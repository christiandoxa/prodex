#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};

#[test]
fn stream_choice_delta_omits_only_empty_text_fields_in_mojo() {
    for (source, expected) in [
        (
            r#"{"delta":{"reasoning_content":"","refusal":" ","annotations":[{"type":"citation"}],"content":"","tool_calls":[]}}"#,
            br#"{"refusal":" ","annotations":[{"type":"citation"}],"tool_calls":[]}"#.as_slice(),
        ),
        (
            r#"{"delta":{"reasoning_content":"","refusal":"","content":""}}"#,
            br#"{}"#.as_slice(),
        ),
    ] {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StreamChoiceDelta);
        input.input = Some(source);
        assert_eq!(
            deepseek_kernel(input).expect("DeepSeek Mojo kernel"),
            expected
        );
    }
}
