use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn text_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if text_width(value) <= width {
        return value.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut output = take_display_width(value, width - 3);
    output.push_str("...");
    output
}

pub fn pad_cell(value: &str, width: usize) -> String {
    let value = fit_cell(value, width);
    let padding = width.saturating_sub(text_width(&value));
    format!("{value}{}", " ".repeat(padding))
}

pub fn chunk_token(token: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for ch in token.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if current_width > 0 && current_width + ch_width > width {
            chunks.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
        if current_width >= width {
            chunks.push(std::mem::take(&mut current));
            current_width = 0;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn take_display_width(value: &str, width: usize) -> String {
    let mut output = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width > width {
            break;
        }
        output.push(ch);
        used += ch_width;
    }
    output
}

pub fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            for piece in chunk_token(word, width) {
                if current.is_empty() {
                    current.push_str(&piece);
                } else if text_width(&current) + 1 + text_width(&piece) <= width {
                    current.push(' ');
                    current.push_str(&piece);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(&piece);
                }
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
