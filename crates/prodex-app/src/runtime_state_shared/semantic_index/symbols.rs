use super::{
    RuntimeSmartContextParsedSymbolLine, RuntimeSmartContextSymbolRangeStyle,
    runtime_smart_context_bounded_string,
};
use crate::runtime_state_shared::{
    RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES, RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES,
    RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES,
};

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_symbol_line(
    lines: &[&str],
    index: usize,
) -> Option<RuntimeSmartContextParsedSymbolLine> {
    let line = lines.get(index)?.trim_start();
    if line.is_empty()
        || line.starts_with("//")
        || line.starts_with("/*")
        || line.starts_with('*')
        || line.starts_with("#[")
        || line.starts_with('@')
    {
        return None;
    }

    let rust_test = runtime_smart_context_has_rust_test_attribute(lines, index);
    if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, "fn") {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: if rust_test { "test_symbol" } else { "function" },
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_python_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: if symbol.starts_with("test_") {
                "test_symbol"
            } else {
                "function"
            },
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Python,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_js_test_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: "test_symbol",
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_js_function_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: "function",
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    for keyword in ["struct", "enum", "trait", "impl", "mod", "class"] {
        if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, keyword) {
            return Some(RuntimeSmartContextParsedSymbolLine {
                label: "symbol",
                symbol: if keyword == "impl" {
                    format!("impl {symbol}")
                } else {
                    symbol
                },
                style: RuntimeSmartContextSymbolRangeStyle::Brace,
            });
        }
    }

    None
}

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
        for ch in line.chars() {
            if ch == '{' {
                saw_open = true;
                balance += 1;
            } else if ch == '}' && saw_open {
                balance -= 1;
            }
        }
        if saw_open && balance <= 0 {
            return index;
        }
        if !saw_open
            && index > declaration_index
            && index - declaration_index >= RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES
        {
            return index;
        }
        if !saw_open && line.trim_end().ends_with(';') {
            return index;
        }
    }
    max_end
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_has_rust_test_attribute(
    lines: &[&str],
    declaration_index: usize,
) -> bool {
    let lower_bound =
        declaration_index.saturating_sub(RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES);
    for line in lines[lower_bound..declaration_index].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if runtime_smart_context_is_rust_test_attribute(trimmed) {
            return true;
        }
        if !trimmed.starts_with("#[") {
            break;
        }
    }
    false
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_is_rust_test_attribute(
    line: &str,
) -> bool {
    line == "#[test]"
        || line.starts_with("#[tokio::test")
        || line.starts_with("#[async_std::test")
        || line.starts_with("#[rstest")
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_python_symbol(
    line: &str,
) -> Option<String> {
    let line = line
        .strip_prefix("async def ")
        .or_else(|| line.strip_prefix("def "))?;
    runtime_smart_context_take_identifier(line)
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_js_function_symbol(
    line: &str,
) -> Option<String> {
    if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, "function") {
        return Some(symbol);
    }
    let (left, right) = line.split_once('=')?;
    if !right.contains("=>") && !right.trim_start().starts_with("function") {
        return None;
    }
    let name = left
        .split_whitespace()
        .last()
        .map(|name| name.trim_matches(|ch: char| ch == ':' || ch == '?'))?;
    runtime_smart_context_take_identifier(name)
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_js_test_symbol(
    line: &str,
) -> Option<String> {
    for prefix in ["test(", "it("] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let label = rest
                .trim_start()
                .strip_prefix('"')
                .and_then(|rest| rest.split_once('"').map(|(label, _)| label))
                .or_else(|| {
                    rest.trim_start()
                        .strip_prefix('\'')
                        .and_then(|rest| rest.split_once('\'').map(|(label, _)| label))
                })
                .unwrap_or(prefix.trim_end_matches('('));
            return runtime_smart_context_bounded_string(label);
        }
    }
    None
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_identifier_after_keyword(
    line: &str,
    keyword: &str,
) -> Option<String> {
    let (_, rest) = line.split_once(&format!("{keyword} "))?;
    runtime_smart_context_take_identifier(rest.trim_start_matches('*').trim_start())
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_take_identifier(
    value: &str,
) -> Option<String> {
    let identifier = value
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '_' | '$' | '#'))
        .collect::<String>();
    runtime_smart_context_bounded_string(identifier.trim_start_matches("r#"))
}
