use super::RuntimeSmartContextSymbolRangeStyle;
use crate::runtime_state_shared::{
    RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES, RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES,
    RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES,
};

pub(in crate::runtime_state_shared) fn runtime_smart_context_symbol_range_bounds(
    lines: &[&str],
    declaration_index: usize,
    style: RuntimeSmartContextSymbolRangeStyle,
) -> (usize, usize) {
    let start_index = runtime_smart_context_symbol_prefix_start(lines, declaration_index);
    let end_index = match style {
        RuntimeSmartContextSymbolRangeStyle::Python => {
            runtime_smart_context_python_symbol_end(lines, declaration_index)
        }
        RuntimeSmartContextSymbolRangeStyle::Brace => {
            runtime_smart_context_brace_symbol_end(lines, declaration_index)
        }
    };
    (start_index + 1, end_index + 1)
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_symbol_prefix_start(
    lines: &[&str],
    declaration_index: usize,
) -> usize {
    let mut start = declaration_index;
    let lower_bound =
        declaration_index.saturating_sub(RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES);
    while start > lower_bound {
        let previous = lines[start - 1].trim_start();
        if previous.is_empty()
            || previous.starts_with("#[")
            || previous.starts_with('@')
            || previous.starts_with("//")
        {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_brace_symbol_end(
    lines: &[&str],
    declaration_index: usize,
) -> usize {
    let max_end = (declaration_index + RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES - 1)
        .min(lines.len().saturating_sub(1));
    let mut balance = 0isize;
    let mut saw_open = false;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(declaration_index)
    {
        if runtime_smart_context_scan_brace_line(line, &mut balance, &mut saw_open) {
            return index;
        }
        if runtime_smart_context_brace_signature_ended(line, index, declaration_index, saw_open) {
            return index;
        }
    }
    max_end
}

fn runtime_smart_context_scan_brace_line(
    line: &str,
    balance: &mut isize,
    saw_open: &mut bool,
) -> bool {
    for ch in line.chars() {
        if ch == '{' {
            *saw_open = true;
            *balance += 1;
        } else if ch == '}' && *saw_open {
            *balance -= 1;
        }
    }
    *saw_open && *balance <= 0
}

fn runtime_smart_context_brace_signature_ended(
    line: &str,
    index: usize,
    declaration_index: usize,
    saw_open: bool,
) -> bool {
    if saw_open {
        return false;
    }
    index > declaration_index
        && (index - declaration_index >= RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES
            || line.trim_end().ends_with(';'))
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_python_symbol_end(
    lines: &[&str],
    declaration_index: usize,
) -> usize {
    let base_indent = runtime_smart_context_leading_whitespace(lines[declaration_index]);
    let max_end = (declaration_index + RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES - 1)
        .min(lines.len().saturating_sub(1));
    let mut end = declaration_index;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(declaration_index + 1)
    {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && runtime_smart_context_leading_whitespace(line) <= base_indent
        {
            break;
        }
        end = index;
    }
    end
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_leading_whitespace(
    line: &str,
) -> usize {
    line.chars().take_while(|ch| ch.is_whitespace()).count()
}
