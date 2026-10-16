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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_projection_preserves_blank_lines_and_bare_carriage_returns() {
        let attachment = "/home/test-user/.prodex/profiles/.prodex-overlay-123/attachments/a.png";
        assert_eq!(
            transcript_visible_message_text(&format!("before\n\n{attachment}\nafter")),
            Some("before\n\nafter".to_string()),
        );
        for (input, expected) in [
            ("before\rafter", "before\rafter"),
            ("before\r\nafter", "before\nafter"),
            ("before\n\r\nafter\r", "before\n\nafter"),
        ] {
            assert_eq!(
                transcript_visible_message_text(input).as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn tool_projection_preserves_attachment_text_and_binary_sample_limit() {
        let attachment = "/home/test-user/.prodex/profiles/.prodex-overlay-123/attachments/a.png";
        assert_eq!(
            transcript_visible_tool_output(attachment).as_deref(),
            Some(attachment)
        );
        let text = format!("{}\u{0}tail", "a".repeat(4096));
        assert_eq!(transcript_visible_tool_output(&text), Some(text));
    }
}
