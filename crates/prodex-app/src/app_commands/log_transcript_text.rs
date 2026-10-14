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
    let stats = transcript_text_stats(text);
    stats.total > 0
        && (stats.replacement > 0
            || stats.control > 0
            || transcript_line_looks_like_mojibake(&stats)
            || stats.suspicious.saturating_mul(5) >= stats.total
            || stats.suspicious >= 6)
}

fn transcript_text_looks_binary(text: &str) -> bool {
    let stats = transcript_text_stats(text);
    stats.total > 0
        && (stats.replacement >= 3
            || stats.control > 0
            || stats.suspicious.saturating_mul(4) > stats.total
            || stats.suspicious >= 8)
}

#[derive(Default)]
struct TranscriptTextStats {
    total: usize,
    suspicious: usize,
    replacement: usize,
    control: usize,
    readable_ascii: usize,
    non_ascii: usize,
    whitespace: usize,
}

fn transcript_text_stats(text: &str) -> TranscriptTextStats {
    let mut stats = TranscriptTextStats::default();
    for ch in text.chars().take(4096) {
        let char_stats = transcript_char_stats(ch);
        stats.total += char_stats.total;
        stats.suspicious += char_stats.suspicious;
        stats.replacement += char_stats.replacement;
        stats.control += char_stats.control;
        stats.readable_ascii += char_stats.readable_ascii;
        stats.non_ascii += char_stats.non_ascii;
        stats.whitespace += char_stats.whitespace;
    }
    stats
}

fn transcript_char_stats(ch: char) -> TranscriptTextStats {
    let whitespace = ch.is_whitespace();
    let replacement = ch == '\u{fffd}';
    let control = ch.is_control() && !matches!(ch, '\n' | '\r' | '\t');
    let readable_ascii =
        ch.is_ascii() && (ch.is_alphanumeric() || whitespace || is_common_punctuation(ch));
    let suspicious = replacement
        || control
        || (!ch.is_ascii() && !ch.is_alphanumeric() && !whitespace && !is_common_punctuation(ch));
    TranscriptTextStats {
        total: 1,
        suspicious: usize::from(suspicious),
        replacement: usize::from(replacement),
        control: usize::from(control),
        readable_ascii: usize::from(readable_ascii),
        non_ascii: usize::from(!ch.is_ascii()),
        whitespace: usize::from(whitespace),
    }
}

fn transcript_line_looks_like_mojibake(stats: &TranscriptTextStats) -> bool {
    stats.total >= 24
        && stats.non_ascii >= 8
        && stats.non_ascii.saturating_mul(100) >= stats.total.saturating_mul(20)
        && stats.readable_ascii.saturating_mul(100) <= stats.total.saturating_mul(55)
        && stats.whitespace.saturating_mul(6) <= stats.total
}

fn is_common_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | ':'
            | ';'
            | '!'
            | '?'
            | '-'
            | '_'
            | '/'
            | '\\'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '"'
            | '\''
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '+'
            | '='
            | '|'
            | '~'
            | '`'
    )
}
