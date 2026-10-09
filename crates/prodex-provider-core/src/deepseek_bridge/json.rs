//! JSON/text helpers shared by DeepSeek bridge compatibility shims.

pub fn deepseek_provider_core_responses_content_text(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let source = serde_json::to_string(value).expect("DeepSeek content serializes");
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::ResponsesContentText,
    );
    input.input = Some(&source);
    let output = prodex_mojo_core::rich::deepseek_kernel(input)
        .expect("Mojo DeepSeek content extraction returned invalid output");
    serde_json::from_slice::<String>(&output)
        .expect("Mojo DeepSeek content extraction returned invalid JSON")
}

pub fn deepseek_provider_core_responses_content_text_value(value: &serde_json::Value) -> String {
    deepseek_provider_core_responses_content_text(Some(value))
}

pub fn deepseek_provider_core_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

pub fn deepseek_provider_core_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in path.iter().skip(1) {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
}
