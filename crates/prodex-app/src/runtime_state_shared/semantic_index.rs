use super::*;

#[derive(Default)]
pub(super) struct RuntimeSmartContextArtifactSemanticLineIndexParts {
    pub(super) file_location_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(super) diff_hunk_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(super) test_failure_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(super) error_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(super) symbol_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(super) complete: bool,
    pub(super) symbol_complete: bool,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSmartContextParsedFileLocation {
    path: String,
    line: usize,
    column: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSmartContextParsedDiffHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

#[derive(Default)]
pub(super) struct RuntimeSmartContextSemanticRangeMetadata {
    label: Option<String>,
    path: Option<String>,
    line: Option<usize>,
    column: Option<usize>,
    old_start: Option<usize>,
    old_count: Option<usize>,
    new_start: Option<usize>,
    new_count: Option<usize>,
    code: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSmartContextParsedSymbolLine {
    label: &'static str,
    symbol: String,
    style: RuntimeSmartContextSymbolRangeStyle,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RuntimeSmartContextSymbolRangeStyle {
    Brace,
    Python,
}

pub(super) fn runtime_smart_context_artifact_semantic_line_index(
    lines: &[&str],
) -> RuntimeSmartContextArtifactSemanticLineIndexParts {
    let mut parts = RuntimeSmartContextArtifactSemanticLineIndexParts {
        complete: true,
        symbol_complete: true,
        ..Default::default()
    };
    let mut remaining = RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES;
    let mut current_diff_path: Option<String> = None;

    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;

        if let Some(path) = runtime_smart_context_parse_diff_file_path(line) {
            current_diff_path = Some(path);
        }

        if let Some(hunk) = runtime_smart_context_parse_diff_hunk(line) {
            let end = runtime_smart_context_diff_hunk_end(lines, index);
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("diff_hunk".to_string()),
                path: current_diff_path.clone(),
                old_start: Some(hunk.old_start),
                old_count: Some(hunk.old_count),
                new_start: Some(hunk.new_start),
                new_count: Some(hunk.new_count),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.diff_hunk_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                end,
                metadata,
            );
        }

        if let Some(location) = runtime_smart_context_parse_file_location(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("file_location".to_string()),
                path: Some(location.path),
                line: Some(location.line),
                column: location.column,
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.file_location_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }

        if runtime_smart_context_is_test_failure_line(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("test_failure".to_string()),
                symbol: runtime_smart_context_parse_test_symbol(line),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.test_failure_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number.saturating_sub(1).max(1),
                (line_number + 1).min(lines.len()),
                metadata,
            );
        }

        if let Some(code) = runtime_smart_context_parse_error_code(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("error".to_string()),
                code: Some(code),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.error_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }
    }

    for (index, _) in lines.iter().enumerate() {
        let Some(symbol) = runtime_smart_context_parse_symbol_line(lines, index) else {
            continue;
        };
        let (start, end) = runtime_smart_context_symbol_range_bounds(lines, index, symbol.style);
        let metadata = RuntimeSmartContextSemanticRangeMetadata {
            label: Some(symbol.label.to_string()),
            line: Some(index + 1),
            symbol: Some(symbol.symbol),
            ..Default::default()
        };
        runtime_smart_context_push_semantic_range(
            &mut parts.symbol_ranges,
            &mut remaining,
            &mut parts.symbol_complete,
            lines,
            start,
            end,
            metadata,
        );
    }

    parts
}

pub(super) fn runtime_smart_context_push_semantic_range(
    target: &mut Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    remaining: &mut usize,
    complete: &mut bool,
    lines: &[&str],
    start: usize,
    end: usize,
    metadata: RuntimeSmartContextSemanticRangeMetadata,
) {
    if *remaining == 0 {
        *complete = false;
        return;
    }
    let Some(text) = runtime_smart_context_line_excerpt(lines, start, end) else {
        *complete = false;
        return;
    };
    if target.iter().any(|range| {
        range.start == start
            && range.end == end
            && range.label == metadata.label
            && range.path == metadata.path
            && range.code == metadata.code
            && range.symbol == metadata.symbol
    }) {
        return;
    }
    let byte_len = text.len();
    target.push(RuntimeSmartContextArtifactSemanticLineRange {
        start,
        end,
        byte_len,
        content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
        text,
        label: metadata.label,
        path: metadata.path,
        line: metadata.line,
        column: metadata.column,
        old_start: metadata.old_start,
        old_count: metadata.old_count,
        new_start: metadata.new_start,
        new_count: metadata.new_count,
        code: metadata.code,
        symbol: metadata.symbol,
    });
    *remaining = remaining.saturating_sub(1);
}

pub(super) fn runtime_smart_context_line_excerpt(
    lines: &[&str],
    start: usize,
    end: usize,
) -> Option<String> {
    if start == 0 || start > lines.len() || end < start {
        return None;
    }
    let end = end.min(lines.len());
    let excerpt = lines[start - 1..end].join("\n");
    (excerpt.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES).then_some(excerpt)
}

pub(super) fn runtime_smart_context_parse_symbol_line(
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

pub(super) fn runtime_smart_context_symbol_range_bounds(
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

pub(super) fn runtime_smart_context_symbol_prefix_start(
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

pub(super) fn runtime_smart_context_brace_symbol_end(
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

pub(super) fn runtime_smart_context_python_symbol_end(
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

pub(super) fn runtime_smart_context_leading_whitespace(line: &str) -> usize {
    line.chars().take_while(|ch| ch.is_whitespace()).count()
}

pub(super) fn runtime_smart_context_has_rust_test_attribute(
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

pub(super) fn runtime_smart_context_is_rust_test_attribute(line: &str) -> bool {
    line == "#[test]"
        || line.starts_with("#[tokio::test")
        || line.starts_with("#[async_std::test")
        || line.starts_with("#[rstest")
}

pub(super) fn runtime_smart_context_parse_python_symbol(line: &str) -> Option<String> {
    let line = line
        .strip_prefix("async def ")
        .or_else(|| line.strip_prefix("def "))?;
    runtime_smart_context_take_identifier(line)
}

pub(super) fn runtime_smart_context_parse_js_function_symbol(line: &str) -> Option<String> {
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

pub(super) fn runtime_smart_context_parse_js_test_symbol(line: &str) -> Option<String> {
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

pub(super) fn runtime_smart_context_identifier_after_keyword(
    line: &str,
    keyword: &str,
) -> Option<String> {
    let (_, rest) = line.split_once(&format!("{keyword} "))?;
    runtime_smart_context_take_identifier(rest.trim_start_matches('*').trim_start())
}

pub(super) fn runtime_smart_context_take_identifier(value: &str) -> Option<String> {
    let identifier = value
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '_' | '$' | '#'))
        .collect::<String>();
    runtime_smart_context_bounded_string(identifier.trim_start_matches("r#"))
}

pub(super) fn runtime_smart_context_parse_file_location(
    line: &str,
) -> Option<RuntimeSmartContextParsedFileLocation> {
    line.split_whitespace()
        .filter_map(runtime_smart_context_parse_file_location_token)
        .next()
}

pub(super) fn runtime_smart_context_parse_file_location_token(
    token: &str,
) -> Option<RuntimeSmartContextParsedFileLocation> {
    let token = token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        })
        .trim_end_matches(':');
    let last_colon = token.rfind(':')?;
    let last_number = token[last_colon + 1..].parse::<usize>().ok()?;
    let prefix = &token[..last_colon];
    let (path, line, column) = if let Some(second_colon) = prefix.rfind(':') {
        if let Ok(line) = prefix[second_colon + 1..].parse::<usize>() {
            (&prefix[..second_colon], line, Some(last_number))
        } else {
            (prefix, last_number, None)
        }
    } else {
        (prefix, last_number, None)
    };
    let path = path
        .trim_start_matches("file://")
        .trim_start_matches("a/")
        .trim_start_matches("b/");
    if !runtime_smart_context_path_looks_like_file(path) {
        return None;
    }
    let path = runtime_smart_context_bounded_string(path)?;
    Some(RuntimeSmartContextParsedFileLocation { path, line, column })
}

pub(super) fn runtime_smart_context_path_looks_like_file(path: &str) -> bool {
    if path.is_empty() || path.len() > RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES {
        return false;
    }
    path.contains('/')
        || path.contains('\\')
        || path.rsplit_once('.').is_some_and(|(_, ext)| {
            matches!(
                ext,
                "rs" | "toml"
                    | "json"
                    | "md"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "swift"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "h"
                    | "hpp"
                    | "css"
                    | "scss"
                    | "html"
                    | "yml"
                    | "yaml"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "sql"
                    | "lock"
            )
        })
}

pub(super) fn runtime_smart_context_parse_diff_file_path(line: &str) -> Option<String> {
    let path = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))?
        .split_whitespace()
        .next()?;
    if path == "/dev/null" {
        return None;
    }
    let path = path
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_matches('"');
    runtime_smart_context_bounded_string(path)
}

pub(super) fn runtime_smart_context_parse_diff_hunk(
    line: &str,
) -> Option<RuntimeSmartContextParsedDiffHunk> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }
    let (old_start, old_count) = runtime_smart_context_parse_diff_span(parts.next()?, '-')?;
    let (new_start, new_count) = runtime_smart_context_parse_diff_span(parts.next()?, '+')?;
    Some(RuntimeSmartContextParsedDiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

pub(super) fn runtime_smart_context_parse_diff_span(
    span: &str,
    prefix: char,
) -> Option<(usize, usize)> {
    let span = span.strip_prefix(prefix)?;
    let mut parts = span.splitn(2, ',');
    let start = parts.next()?.parse::<usize>().ok()?;
    let count = parts
        .next()
        .map(|count| count.parse::<usize>().ok())
        .unwrap_or(Some(1))?;
    Some((start, count))
}

pub(super) fn runtime_smart_context_diff_hunk_end(lines: &[&str], start_index: usize) -> usize {
    let max_end = (start_index + 24).min(lines.len().saturating_sub(1));
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(start_index + 1)
    {
        if line.starts_with("@@ ") || line.starts_with("diff --git ") {
            return index;
        }
        if !(line.starts_with(' ')
            || line.starts_with('+')
            || line.starts_with('-')
            || line.starts_with("\\ No newline"))
        {
            return index;
        }
    }
    max_end + 1
}

pub(super) fn runtime_smart_context_is_test_failure_line(line: &str) -> bool {
    line.contains("test result: FAILED")
        || line == "failures:"
        || line.starts_with("failures:")
        || line.starts_with("FAIL ")
        || line.starts_with("FAILED ")
        || line.contains(" panicked at ")
        || (line.starts_with("---- ") && line.ends_with(" stdout ----"))
}

pub(super) fn runtime_smart_context_parse_test_symbol(line: &str) -> Option<String> {
    if let Some(symbol) = line
        .strip_prefix("---- ")
        .and_then(|line| line.strip_suffix(" stdout ----"))
    {
        return runtime_smart_context_bounded_string(symbol);
    }
    if let Some(rest) = line.strip_prefix("thread '")
        && let Some((symbol, _)) = rest.split_once("' panicked at ")
    {
        return runtime_smart_context_bounded_string(symbol);
    }
    None
}

pub(super) fn runtime_smart_context_parse_error_code(line: &str) -> Option<String> {
    if let Some(code) = runtime_smart_context_parse_bracketed_error_code(line) {
        return Some(code);
    }
    if line.contains("error:") || line.contains("Error:") || line.contains("ERROR") {
        return Some("error".to_string());
    }
    if let Some((_, rest)) = line.split_once("exit code ") {
        let code = rest.split_whitespace().next()?;
        return runtime_smart_context_bounded_string(&format!("exit_code_{code}"));
    }
    if let Some((_, rest)) = line.split_once("status code ") {
        let code = rest.split_whitespace().next()?;
        return runtime_smart_context_bounded_string(&format!("status_code_{code}"));
    }
    None
}

pub(super) fn runtime_smart_context_parse_bracketed_error_code(line: &str) -> Option<String> {
    let start = line.find("error[")? + "error[".len();
    let rest = &line[start..];
    let end = rest.find(']')?;
    runtime_smart_context_bounded_string(&rest[..end])
}

pub(super) fn runtime_smart_context_infer_command_kind(lines: &[&str]) -> Option<String> {
    let mut saw_diff = false;
    let mut saw_cargo_test = false;
    let mut saw_cargo_error = false;
    let mut saw_npm_test = false;
    for line in lines {
        if line.starts_with("diff --git ") || line.starts_with("@@ ") {
            saw_diff = true;
        }
        if line.contains("test result:") || line.starts_with("running ") && line.ends_with(" tests")
        {
            saw_cargo_test = true;
        }
        if line.contains("error: could not compile") {
            saw_cargo_error = true;
        }
        if line.starts_with("npm ERR!") || line.starts_with("FAIL ") {
            saw_npm_test = true;
        }
        if *line == "Traceback (most recent call last):" {
            return Some("python".to_string());
        }
    }
    if saw_cargo_test {
        Some("cargo-test".to_string())
    } else if saw_npm_test {
        Some("npm-test".to_string())
    } else if saw_cargo_error {
        Some("cargo-build".to_string())
    } else {
        saw_diff.then(|| "diff".to_string())
    }
}

pub(super) fn runtime_smart_context_bounded_string(value: &str) -> Option<String> {
    (!value.is_empty() && value.len() <= RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES)
        .then(|| value.to_string())
}

pub(super) fn runtime_smart_context_semantic_index_complete_default() -> bool {
    true
}

pub(super) fn runtime_smart_context_bool_is_true(value: &bool) -> bool {
    *value
}

pub(super) fn runtime_smart_context_u8_is_zero(value: &u8) -> bool {
    *value == 0
}
