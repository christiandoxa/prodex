use super::{
    RuntimeSmartContextParsedDiffHunk, RuntimeSmartContextParsedFileLocation,
    runtime_smart_context_bounded_string,
};
use crate::runtime_state_shared::RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES;

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_file_location(
    line: &str,
) -> Option<RuntimeSmartContextParsedFileLocation> {
    line.split_whitespace()
        .filter_map(runtime_smart_context_parse_file_location_token)
        .next()
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_file_location_token(
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_path_looks_like_file(
    path: &str,
) -> bool {
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_diff_file_path(
    line: &str,
) -> Option<String> {
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_diff_hunk(
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_diff_span(
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_diff_hunk_end(
    lines: &[&str],
    start_index: usize,
) -> usize {
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_is_test_failure_line(
    line: &str,
) -> bool {
    line.contains("test result: FAILED")
        || line == "failures:"
        || line.starts_with("failures:")
        || line.starts_with("FAIL ")
        || line.starts_with("FAILED ")
        || line.contains(" panicked at ")
        || (line.starts_with("---- ") && line.ends_with(" stdout ----"))
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_test_symbol(
    line: &str,
) -> Option<String> {
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_error_code(
    line: &str,
) -> Option<String> {
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

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_bracketed_error_code(
    line: &str,
) -> Option<String> {
    let start = line.find("error[")? + "error[".len();
    let rest = &line[start..];
    let end = rest.find(']')?;
    runtime_smart_context_bounded_string(&rest[..end])
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_infer_command_kind(
    lines: &[&str],
) -> Option<String> {
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
