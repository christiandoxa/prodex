use super::{RUNTIME_SSE_LINE_BLANK, RUNTIME_SSE_LINE_DATA};

pub(super) fn sse_line_plan(line: &[u8]) -> (i64, usize, usize) {
    let mut end = line.len();
    while end > 0 && matches!(line.get(end - 1), Some(b'\r' | b'\n')) {
        end -= 1;
    }
    let trimmed = &line[..end];
    if trimmed.is_empty() {
        return (RUNTIME_SSE_LINE_BLANK, end, end);
    }
    if trimmed.starts_with(b":") {
        return (1, end, end);
    }
    let Some(separator) = trimmed.iter().position(|byte| *byte == b':') else {
        return if trimmed == b"data" {
            (RUNTIME_SSE_LINE_DATA, end, end)
        } else {
            (1, end, end)
        };
    };
    if &trimmed[..separator] != b"data" {
        return (1, end, end);
    }
    let mut start = separator + 1;
    if trimmed.get(start) == Some(&b' ') {
        start += 1;
    }
    (RUNTIME_SSE_LINE_DATA, start, end)
}
