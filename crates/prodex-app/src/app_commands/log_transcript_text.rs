pub(super) fn transcript_text_from_content(content: &serde_json::Value) -> Option<String> {
    let values = content.as_array()?;
    let mut parts = Vec::new();
    for value in values {
        if let Some(text) = value
            .get("text")
            .or_else(|| value.get("content"))
            .and_then(serde_json::Value::as_str)
            .and_then(transcript_visible_message_text)
        {
            parts.push(text.to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

pub(super) fn transcript_visible_message_text(text: &str) -> Option<String> {
    let mut visible_lines = Vec::new();
    let mut last_blank = false;
    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            if !last_blank && !visible_lines.is_empty() {
                visible_lines.push(String::new());
            }
            last_blank = true;
            continue;
        }
        if transcript_line_contains_internal_attachment_path(line) {
            continue;
        }
        visible_lines.push(line.to_string());
        last_blank = false;
    }
    while visible_lines.last().is_some_and(|line| line.is_empty()) {
        visible_lines.pop();
    }
    (!visible_lines.is_empty()).then(|| visible_lines.join("\n"))
}

pub(super) fn transcript_visible_tool_output(output: &str) -> Option<String> {
    let output = prodex_mojo_core::log::strip_transcript_ansi_codes(output)
        .expect("compiled-in Mojo transcript normalizer failed");
    let output = output.trim();
    if output.is_empty() {
        return None;
    }
    if !output.contains('\n') && transcript_text_looks_binary(output) {
        return None;
    }

    let mut visible_lines = Vec::new();
    let mut last_blank = false;
    for raw_line in output.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            if !last_blank && !visible_lines.is_empty() {
                visible_lines.push(String::new());
            }
            last_blank = true;
            continue;
        }
        if transcript_line_looks_binary(line) {
            continue;
        }
        visible_lines.push(line.to_string());
        last_blank = false;
    }
    while visible_lines.last().is_some_and(|line| line.is_empty()) {
        visible_lines.pop();
    }
    (!visible_lines.is_empty()).then(|| visible_lines.join("\n"))
}

fn transcript_line_contains_internal_attachment_path(text: &str) -> bool {
    text.contains("/.prodex/profiles/.prodex-overlay-") && text.contains("/attachments/")
}

fn transcript_line_looks_binary(text: &str) -> bool {
    prodex_mojo_core::log::transcript_binary_like(text, true)
        .expect("compiled-in Mojo transcript line classification failed")
}

fn transcript_text_looks_binary(text: &str) -> bool {
    prodex_mojo_core::log::transcript_binary_like(text, false)
        .expect("compiled-in Mojo transcript text classification failed")
}
