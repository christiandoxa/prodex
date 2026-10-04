//! Internal-instruction echo detection.

pub fn gemini_provider_core_internal_instruction_corpus(messages: &[serde_json::Value]) -> String {
    let mut text = String::new();
    for message in messages {
        if message.get("role").and_then(serde_json::Value::as_str) != Some("system") {
            continue;
        }
        gemini_provider_core_collect_text_for_echo_detection(message.get("content"), &mut text);
        text.push('\n');
    }
    prodex_mojo_core::gemini_internal_instruction::normalize_corpus(&text)
        .expect("Mojo Gemini instruction corpus normalizer returned invalid output")
}

pub fn gemini_provider_core_text_echoes_internal_instruction(
    text: &str,
    internal_instruction_corpus: &str,
) -> bool {
    prodex_mojo_core::gemini_internal_instruction::text_echoes(text, internal_instruction_corpus)
        .expect("Mojo Gemini instruction echo detector returned invalid output")
}

fn gemini_provider_core_collect_text_for_echo_detection(
    value: Option<&serde_json::Value>,
    output: &mut String,
) {
    match value {
        Some(serde_json::Value::String(text)) => {
            output.push_str(text);
            output.push('\n');
        }
        Some(serde_json::Value::Array(values)) => {
            for value in values {
                gemini_provider_core_collect_text_for_echo_detection(Some(value), output);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            for key in ["text", "content", "input", "output"] {
                gemini_provider_core_collect_text_for_echo_detection(object.get(key), output);
            }
        }
        _ => {}
    }
}
