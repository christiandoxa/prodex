use super::{RuntimeSmartContextParsedDiffHunk, RuntimeSmartContextParsedFileLocation};

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
    let plan = prodex_mojo_core::smart_context_markers::parse_file_location_token(token)
        .expect("Mojo smart-context file-location parser returned invalid output")?;
    let path = token.get(plan.path_start..plan.path_end)?.to_string();
    Some(RuntimeSmartContextParsedFileLocation {
        path,
        line: plan.line,
        column: plan.column,
    })
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_diff_file_path(
    line: &str,
) -> Option<String> {
    let token = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))?
        .split_whitespace()
        .next()?;
    let (start, end) =
        prodex_mojo_core::smart_context_markers::normalize_diff_file_path_token(token)
            .expect("Mojo smart-context diff-path parser returned invalid output")?;
    token.get(start..end).map(str::to_string)
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
    let plan = prodex_mojo_core::smart_context_markers::parse_diff_span(span, prefix)
        .expect("Mojo smart-context diff-span parser returned invalid output")?;
    Some((plan.start, plan.count))
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
    prodex_mojo_core::smart_context_markers::is_test_failure_line(line)
        .expect("Mojo smart-context test-failure classifier returned invalid output")
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_test_symbol(
    line: &str,
) -> Option<String> {
    let (start, end) = prodex_mojo_core::smart_context_markers::test_symbol_span(line)
        .expect("Mojo smart-context test-symbol parser returned invalid output")?;
    line.get(start..end).map(str::to_string)
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_parse_error_code(
    line: &str,
) -> Option<String> {
    prodex_mojo_core::smart_context_markers::error_code(line)
        .expect("Mojo smart-context error-code parser returned invalid output")
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_infer_command_kind(
    lines: &[&str],
) -> Option<String> {
    let mut saw_diff = false;
    let mut saw_cargo_test = false;
    let mut saw_cargo_error = false;
    let mut saw_npm_test = false;
    for line in lines {
        match runtime_smart_context_command_line_kind(line) {
            Some("python") => return Some("python".to_string()),
            Some("diff") => saw_diff = true,
            Some("cargo-test") => saw_cargo_test = true,
            Some("cargo-build") => saw_cargo_error = true,
            Some("npm-test") => saw_npm_test = true,
            _ => {}
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

fn runtime_smart_context_command_line_kind(line: &str) -> Option<&'static str> {
    use prodex_mojo_core::smart_context_markers::CommandLineKind;
    match prodex_mojo_core::smart_context_markers::command_line_kind(line)
        .expect("Mojo smart-context command-kind classifier returned invalid output")
    {
        Some(CommandLineKind::Python) => Some("python"),
        Some(CommandLineKind::Diff) => Some("diff"),
        Some(CommandLineKind::CargoTest) => Some("cargo-test"),
        Some(CommandLineKind::CargoBuild) => Some("cargo-build"),
        Some(CommandLineKind::NpmTest) => Some("npm-test"),
        None => None,
    }
}
