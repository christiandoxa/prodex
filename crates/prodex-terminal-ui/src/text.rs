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

pub fn middle_ellipsize(value: &str, width: usize) -> String {
    use prodex_mojo_core::terminal_layout::MiddleEllipsisPlan;

    let mut scalar_cells = Vec::new();
    let mut byte_ends = vec![0usize];
    for (byte_index, ch) in value.char_indices() {
        scalar_cells.push(u64::try_from(ch.width().unwrap_or(0)).expect("Unicode cell width"));
        byte_ends.push(byte_index + ch.len_utf8());
    }

    match prodex_mojo_core::terminal_layout::middle_ellipsis_plan(
        &scalar_cells,
        text_width(value),
        width,
    )
    .expect("compiled-in Mojo terminal middle-ellipsis policy failed")
    {
        MiddleEllipsisPlan::Empty => String::new(),
        MiddleEllipsisPlan::Keep => value.to_string(),
        MiddleEllipsisPlan::Dots(count) => ".".repeat(count),
        MiddleEllipsisPlan::Split {
            left_scalars,
            right_scalars,
        } => {
            let left_end = byte_ends[left_scalars];
            let right_start = byte_ends[scalar_cells.len() - right_scalars];
            format!("{}...{}", &value[..left_end], &value[right_start..])
        }
    }
}

pub fn pad_cell(value: &str, width: usize) -> String {
    let value = fit_cell(value, width);
    let padding = prodex_mojo_core::terminal_layout::pad_cell_spaces(text_width(&value), width)
        .expect("compiled-in Mojo terminal padding policy failed");
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
    let mut scalar_cells = Vec::new();
    let mut byte_ends = vec![0usize];
    for (byte_index, ch) in input.char_indices() {
        scalar_cells.push(u64::try_from(ch.width().unwrap_or(0)).expect("Unicode cell width"));
        byte_ends.push(byte_index + ch.len_utf8());
    }
    let plan = prodex_mojo_core::terminal_layout::wrap_text_plan(input, &scalar_cells, width)
        .expect("compiled-in Mojo text-wrap plan failed");
    plan.line_token_ranges
        .into_iter()
        .map(|line| {
            let mut output = String::new();
            for token_index in line.start..line.end {
                if token_index > line.start {
                    output.push(' ');
                }
                let token = plan.token_ranges[token_index];
                output.push_str(&input[byte_ends[token.start]..byte_ends[token.end]]);
            }
            output
        })
        .collect()
}
