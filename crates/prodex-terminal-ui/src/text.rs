use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn text_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub fn fit_cell(value: &str, width: usize) -> String {
    use prodex_mojo_core::terminal_layout::FitCellPlan;
    match prodex_mojo_core::terminal_layout::fit_cell_plan(text_width(value), width)
        .expect("compiled-in Mojo terminal fit-cell layout policy failed")
    {
        FitCellPlan::Empty => String::new(),
        FitCellPlan::Keep => value.to_string(),
        FitCellPlan::Dots(count) => ".".repeat(count),
        FitCellPlan::Truncate(prefix_cells) => {
            let mut output = take_display_width(value, prefix_cells);
            output.push_str("...");
            output
        }
    }
}

pub fn pad_cell(value: &str, width: usize) -> String {
    let value = fit_cell(value, width);
    let padding = width.saturating_sub(text_width(&value));
    format!("{value}{}", " ".repeat(padding))
}

pub fn chunk_token(token: &str, width: usize) -> Vec<String> {
    // Rust supplies Unicode table cell widths and materializes UTF-8 slices.
    // Mojo chooses every segment boundary, including empty/zero-width cases.
    let mut scalar_cells = Vec::new();
    let mut byte_ends = vec![0usize];
    for (byte_index, ch) in token.char_indices() {
        scalar_cells.push(u64::try_from(ch.width().unwrap_or(0)).expect("Unicode cell width"));
        byte_ends.push(byte_index + ch.len_utf8());
    }
    let boundaries =
        prodex_mojo_core::terminal_layout::token_chunk_boundaries(&scalar_cells, width)
            .expect("compiled-in Mojo terminal token chunk policy failed");
    let mut start = 0usize;
    let mut chunks = Vec::with_capacity(boundaries.len());
    for end_char in boundaries {
        let end = byte_ends[end_char];
        chunks.push(token[start..end].to_string());
        start = end;
    }
    chunks
}

fn take_display_width(value: &str, width: usize) -> String {
    let scalar_cells = value
        .chars()
        .map(|ch| u64::try_from(ch.width().unwrap_or(0)).expect("Unicode cell width"))
        .collect::<Vec<_>>();
    let prefix = prodex_mojo_core::terminal_layout::display_prefix_scalars(&scalar_cells, width)
        .expect("compiled-in Mojo display-width prefix policy failed");
    let end = value
        .char_indices()
        .nth(prefix)
        .map_or(value.len(), |(byte_index, _)| byte_index);
    value[..end].to_string()
}

pub fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        wrap_paragraph(paragraph, width, &mut lines);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn wrap_paragraph(paragraph: &str, width: usize, lines: &mut Vec<String>) {
    if paragraph.trim().is_empty() {
        lines.push(String::new());
        return;
    }

    let mut current = String::new();
    for word in paragraph.split_whitespace() {
        for piece in chunk_token(word, width) {
            append_wrapped_piece(&mut current, lines, &piece, width);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
}

fn append_wrapped_piece(current: &mut String, lines: &mut Vec<String>, piece: &str, width: usize) {
    if current.is_empty() {
        current.push_str(piece);
    } else if text_width(current) + 1 + text_width(piece) <= width {
        current.push(' ');
        current.push_str(piece);
    } else {
        lines.push(std::mem::take(current));
        current.push_str(piece);
    }
}
