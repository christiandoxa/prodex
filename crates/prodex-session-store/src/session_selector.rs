use super::report::session_value_metadata;
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
    let metadata = session_value_metadata(value);
    metadata.resume_id.is_some() && matches!(metadata.type_class, 0 | 1)
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
    session_value_metadata(value).resume_id
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
    prodex_mojo_core::json::session_selector_matches(id, selector, exact)
        .expect("Mojo session selector comparison failed")
}

pub(super) fn full_codex_session_id(selector: &str) -> Option<&str> {
    prodex_mojo_core::json::session_selector_is_full(selector)
        .expect("Mojo session selector validation failed")
        .then_some(selector)
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

#[cfg(test)]
mod tests {
    use super::{full_codex_session_id, session_id_matches_selector};

    #[test]
    fn mojo_selector_keeps_uuid_shape_and_casefold_prefix_contract() {
        let id = "01900000-0000-7000-8000-000000000301";
        assert_eq!(full_codex_session_id(id), Some(id));
        assert_eq!(
            full_codex_session_id(&id.to_uppercase()),
            Some(id.to_uppercase().as_str())
        );
        assert_eq!(
            full_codex_session_id("01900000-0000-7000-8000-00000000030"),
            None
        );
        assert!(session_id_matches_selector(
            id,
            "01900000-0000-7000-8000-0000000003",
            false
        ));
        assert!(!session_id_matches_selector(
            id,
            "01900000-0000-7000-8000-0000000003",
            true
        ));
        assert!(!session_id_matches_selector(
            id,
            "01900000-0000-7000-8000-0000000004",
            false
        ));
    }
}
