use super::first_string_value;
use std::path::Path;

pub(super) fn session_lines_start_resume_metadata<'a>(
    lines: impl IntoIterator<Item = &'a str>,
) -> bool {
    lines
        .into_iter()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(session_line_starts_resume_metadata)
}

pub(super) fn session_line_starts_resume_metadata(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .is_some_and(|value| session_value_starts_resume_metadata(&value))
}

pub(super) fn session_value_starts_resume_metadata(value: &serde_json::Value) -> bool {
    session_value_resume_id(value).is_some()
        && value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|kind| kind == "session_meta")
}

pub(super) fn session_line_resume_id_matches(line: &str, selector: &str) -> bool {
    session_line_resume_id_matches_mode(line, selector, false)
}

pub(super) fn session_line_is_valid_json(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line).is_ok()
}

pub(super) fn session_line_resume_id_matches_mode(line: &str, selector: &str, exact: bool) -> bool {
    session_line_resume_id_matching_mode(line, selector, exact).is_some()
}

pub(super) fn session_line_resume_id_matching_mode(
    line: &str,
    selector: &str,
    exact: bool,
) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| session_value_resume_id(&value))
        .filter(|id| session_id_matches_selector(id, selector, exact))
}

pub(super) fn session_value_resume_id(value: &serde_json::Value) -> Option<String> {
    first_string_value(
        value,
        &[
            &["payload", "id"],
            &["payload", "session_id"],
            &["id"],
            &["session_id"],
        ],
    )
}

pub(super) fn session_path_id_matches_selector(path: &Path, selector: &str, exact: bool) -> bool {
    session_path_id_matching_selector(path, selector, exact).is_some()
}

pub(super) fn session_path_id_matching_selector(
    path: &Path,
    selector: &str,
    exact: bool,
) -> Option<String> {
    let stem = path.file_stem().and_then(|stem| stem.to_str())?;
    if session_id_matches_selector(stem, selector, exact) {
        return Some(stem.to_string());
    }
    stem.split('-')
        .collect::<Vec<_>>()
        .windows(5)
        .map(|parts| parts.join("-"))
        .find(|candidate| session_id_matches_selector(candidate, selector, exact))
}

pub(super) fn session_id_matches_selector(id: &str, selector: &str, exact: bool) -> bool {
    id.eq_ignore_ascii_case(selector)
        || (!exact && id.to_lowercase().starts_with(&selector.to_lowercase()))
}

pub(super) fn full_codex_session_id(selector: &str) -> Option<&str> {
    let bytes = selector.as_bytes();
    let valid = bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    valid.then_some(selector)
}

pub(super) fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem().and_then(|stem| stem.to_str())?;
    if full_codex_session_id(stem).is_some() {
        return Some(stem.to_string());
    }
    stem.split('-')
        .collect::<Vec<_>>()
        .windows(5)
        .map(|parts| parts.join("-"))
        .find(|candidate| full_codex_session_id(candidate).is_some())
}
