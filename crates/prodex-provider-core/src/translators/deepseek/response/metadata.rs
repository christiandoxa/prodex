//! DeepSeek buffered response metadata extraction.

use serde_json::Value;

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

pub(super) fn deepseek_response_metadata(value: &Value) -> Option<Value> {
    let raw = serde_json::to_string(value).expect("DeepSeek response serializes");
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseMetadata);
    input.response = Some(&raw);
    let output = crate::translators::deepseek::deepseek_mojo_value(input);
    (!output.is_null()).then_some(output)
}
