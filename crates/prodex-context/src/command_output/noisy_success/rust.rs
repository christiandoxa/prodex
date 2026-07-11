use super::*;

pub(crate) fn is_rust_success_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("Finished ")
        || trimmed.starts_with("test result: ok")
        || trimmed.starts_with("Summary [") && trimmed.contains(" passed")
}

pub(crate) fn rust_diagnostic_severity(line: &str) -> Option<RustDiagnosticSeverity> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
        Some(RustDiagnosticSeverity::Error)
    } else if trimmed.starts_with("warning[") || trimmed.starts_with("warning:") {
        Some(RustDiagnosticSeverity::Warning)
    } else {
        rust_short_diagnostic_severity(trimmed)
    }
}

pub(crate) fn rust_short_diagnostic_severity(line: &str) -> Option<RustDiagnosticSeverity> {
    let lower = line.to_ascii_lowercase();
    for (needle, severity) in [
        (": error", RustDiagnosticSeverity::Error),
        (": warning", RustDiagnosticSeverity::Warning),
    ] {
        let Some(position) = lower.find(needle) else {
            continue;
        };
        if contains_rust_file_location(&line[..position]) {
            return Some(severity);
        }
    }
    None
}

pub(crate) fn rust_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("test ")?;
    let (name, status) = rest.rsplit_once(" ... ")?;
    (status == "FAILED").then_some(name.trim())
}

pub(crate) fn rust_failure_separator_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("---- ")?.strip_suffix(" ----")?.trim();
    let name = inner
        .strip_suffix(" stdout")
        .or_else(|| inner.strip_suffix(" stderr"))
        .unwrap_or(inner)
        .trim();
    (!name.is_empty()).then_some(name)
}

pub(crate) fn generic_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(name) = trimmed.strip_prefix("FAIL ") {
        return non_empty_prefix(name);
    }
    if let Some(name) = trimmed.strip_prefix("FAILED ") {
        return non_empty_prefix(name);
    }
    if let Some((name, status)) = trimmed.rsplit_once(' ')
        && status == "FAILED"
        && (name.contains("::") || looks_like_location_path(name))
    {
        return non_empty_prefix(name);
    }
    if trimmed.starts_with("Test Suites:") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.starts_with("Tests:") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.contains(" failed, ") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.starts_with("failed ") && !has_zero_only_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    None
}

pub(crate) fn non_empty_prefix(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed
        .split(" - ")
        .next()
        .unwrap_or(trimmed)
        .split(" (")
        .next()
        .unwrap_or(trimmed)
        .trim();
    (!name.is_empty()).then_some(name)
}

pub(crate) fn is_rust_panic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.contains("panicked at ") || trimmed.contains("panicked at:")
}

pub(crate) fn is_rust_backtrace_start(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "stack backtrace:" || trimmed == "Backtrace:"
}

pub(crate) fn is_rust_exit_status_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("exit status")
        || lower.contains("exit code")
        || lower.contains("exit_status")
        || lower.contains("process didn't exit successfully")
}

pub(crate) fn is_rust_location_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("--> ")
        || trimmed.starts_with("::: ")
        || (trimmed.starts_with("at ") && contains_rust_file_location(trimmed))
        || contains_rust_file_location(trimmed)
}

pub(crate) fn contains_rust_file_location(line: &str) -> bool {
    let Some((_, after_rs)) = line.split_once(".rs:") else {
        return false;
    };
    let mut chars = after_rs.chars();
    let mut saw_digit = false;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            continue;
        }
        if ch == ':' {
            return saw_digit && chars.next().is_some_and(|next| next.is_ascii_digit());
        }
        return saw_digit;
    }
    saw_digit
}

pub(crate) fn is_rust_failure_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("test result: FAILED")
        || trimmed == "failures:"
        || trimmed.starts_with("failures:")
        || trimmed.starts_with("error: aborting")
}
