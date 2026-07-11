use std::path::Path;

use crate::{parse_file_list_entry_line, parse_search_match_line};

pub(super) fn is_lockfile_or_vendor_path(path: &Path) -> bool {
    let rendered = path.display().to_string();
    let normalized = rendered.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("cargo.lock")
        || normalized.ends_with("package-lock.json")
        || normalized.ends_with("pnpm-lock.yaml")
        || normalized.ends_with("yarn.lock")
        || normalized.contains("/node_modules/")
        || normalized.contains("/vendor/")
        || normalized.contains("/target/")
}

pub(super) fn looks_like_base64_blob(line: &str) -> bool {
    let base64_chars = line
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '-' | '_'))
        .count();
    base64_chars.saturating_mul(100) >= line.len().saturating_mul(95)
        && line.chars().any(|ch| ch.is_ascii_digit())
        && line.chars().any(|ch| ch.is_ascii_uppercase())
        && line.chars().any(|ch| ch.is_ascii_lowercase())
}

pub(super) fn looks_like_minified_js_json(line: &str) -> bool {
    let punctuation = line
        .chars()
        .filter(|ch| matches!(ch, '{' | '}' | '[' | ']' | ':' | ',' | ';'))
        .count();
    let spaces = line.chars().filter(|ch| ch.is_whitespace()).count();
    punctuation >= 32 && spaces.saturating_mul(80) < line.len()
}

pub(super) fn repeated_path_flood(lines: &[&str]) -> Option<(usize, usize)> {
    let mut first_path_line = None;
    let mut path_lines = 0usize;
    let mut non_empty = 0usize;
    for (index, line) in lines.iter().enumerate() {
        if !line.trim().is_empty() {
            non_empty += 1;
        }
        if parse_file_list_entry_line(line).is_some() || parse_search_match_line(line).is_some() {
            path_lines += 1;
            first_path_line.get_or_insert(index + 1);
        }
    }

    if path_lines >= 200 && path_lines.saturating_mul(100) >= non_empty.saturating_mul(80) {
        first_path_line.map(|line| (line, path_lines))
    } else {
        None
    }
}
