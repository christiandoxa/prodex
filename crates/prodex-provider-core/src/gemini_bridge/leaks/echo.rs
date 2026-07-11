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
    gemini_provider_core_normalized_words(&text).join(" ")
}

pub fn gemini_provider_core_text_echoes_internal_instruction(
    text: &str,
    internal_instruction_corpus: &str,
) -> bool {
    if internal_instruction_corpus.is_empty() {
        return false;
    }
    let words = gemini_provider_core_normalized_words(text);
    const ECHO_WORDS: usize = 8;
    if words.len() < ECHO_WORDS {
        return false;
    }
    words.windows(ECHO_WORDS).take(128).any(|window| {
        let needle = window.join(" ");
        internal_instruction_corpus.contains(&needle)
    })
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

fn gemini_provider_core_normalized_words(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .map(str::trim)
        .filter(|word| word.len() >= 2)
        .map(str::to_ascii_lowercase)
        .collect()
}
