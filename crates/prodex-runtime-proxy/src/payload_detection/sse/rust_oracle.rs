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

pub(super) fn sse_inspection_step(
    committed: bool,
    quota_blocked: bool,
    rate_limited: bool,
    overloaded: bool,
    previous_response_not_found: bool,
    precommit_hold: bool,
) -> (i64, bool) {
    let action = if !committed && quota_blocked {
        1
    } else if !committed && rate_limited {
        2
    } else if !committed && overloaded {
        3
    } else if !committed && previous_response_not_found {
        4
    } else {
        0
    };
    (action, committed || (action == 0 && !precommit_hold))
}
