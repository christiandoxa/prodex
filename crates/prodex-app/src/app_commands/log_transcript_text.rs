pub(super) fn transcript_text_from_content(content: &serde_json::Value) -> Option<String> {
    let values = content.as_array()?;
    let parts = values
        .iter()
        .filter_map(|value| {
            value
                .get("text")
                .or_else(|| value.get("content"))
                .and_then(serde_json::Value::as_str)
                .and_then(transcript_visible_message_text)
        })
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| {
        let parts = parts.iter().map(String::as_str).collect::<Vec<_>>();
        prodex_mojo_core::log::join_transcript_user_parts(&parts)
            .expect("compiled-in Mojo transcript part joiner failed")
    })
}

pub(super) fn transcript_visible_message_text(text: &str) -> Option<String> {
    prodex_mojo_core::log::normalize_transcript_visible_text(text, false)
        .expect("compiled-in Mojo transcript message projection failed")
}

pub(super) fn transcript_visible_tool_output(output: &str) -> Option<String> {
    let output = prodex_mojo_core::log::strip_transcript_ansi_codes(output)
        .expect("compiled-in Mojo transcript normalizer failed");
    prodex_mojo_core::log::normalize_transcript_visible_text(&output, true)
        .expect("compiled-in Mojo transcript tool projection failed")
}
