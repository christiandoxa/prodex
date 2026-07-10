use std::iter::Peekable;
use std::str::Chars;

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
    let output = strip_transcript_ansi_codes(output);
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
    let mut total = 0usize;
    let mut suspicious = 0usize;
    let mut replacement = 0usize;
    let mut control = 0usize;
    let mut readable_ascii = 0usize;
    let mut non_ascii = 0usize;
    let mut whitespace = 0usize;
    for ch in text.chars().take(4096) {
        total += 1;
        if ch.is_whitespace() {
            whitespace += 1;
        }
        if ch.is_ascii()
            && (ch.is_alphanumeric() || ch.is_whitespace() || is_common_punctuation(ch))
        {
            readable_ascii += 1;
        } else if !ch.is_ascii() {
            non_ascii += 1;
        }
        let is_suspicious = ch == '\u{fffd}'
            || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
            || (!ch.is_ascii()
                && !ch.is_alphanumeric()
                && !ch.is_whitespace()
                && !is_common_punctuation(ch));
        if ch == '\u{fffd}' {
            replacement += 1;
        }
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            control += 1;
        }
        if is_suspicious {
            suspicious += 1;
        }
    }
    TranscriptTextStats {
        total,
        suspicious,
        replacement,
        control,
        readable_ascii,
        non_ascii,
        whitespace,
    }
}

fn transcript_line_looks_like_mojibake(stats: &TranscriptTextStats) -> bool {
    stats.total >= 24
        && stats.non_ascii >= 8
        && stats.non_ascii.saturating_mul(100) >= stats.total.saturating_mul(20)
        && stats.readable_ascii.saturating_mul(100) <= stats.total.saturating_mul(55)
        && stats.whitespace.saturating_mul(6) <= stats.total
}

fn strip_transcript_ansi_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            strip_escape_sequence(&mut chars);
        } else if ch == '\r' {
            if !matches!(chars.peek(), Some('\n')) {
                output.push('\n');
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn strip_escape_sequence(chars: &mut Peekable<Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            let mut previous = '\0';
            for code in chars.by_ref() {
                if code == '\u{7}' || (previous == '\u{1b}' && code == '\\') {
                    break;
                }
                previous = code;
            }
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
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
