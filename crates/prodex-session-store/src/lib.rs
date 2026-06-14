use anyhow::{Context, Result};
use prodex_app_reports::{
    SessionReport, apply_session_json_line, apply_session_json_lines, apply_session_value,
    first_string_value, is_session_metadata_file, sort_session_reports,
};
use prodex_state::AppState;
use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionReportFilter<'a> {
    pub current_dir: Option<&'a Path>,
    pub profile: Option<&'a str>,
    pub query: Option<&'a str>,
    pub include_subagents: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionResolveError {
    Missing {
        selector: String,
    },
    Ambiguous {
        selector: String,
        matches: Vec<String>,
    },
}

impl fmt::Display for SessionResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { selector } => {
                write!(formatter, "no session found matching id '{selector}'")
            }
            Self::Ambiguous { selector, matches } => write!(
                formatter,
                "session id '{selector}' is ambiguous; matches: {}",
                matches.join(", ")
            ),
        }
    }
}

impl std::error::Error for SessionResolveError {}

pub fn collect_session_reports(
    shared_codex_root: &Path,
    current_dir: Option<&Path>,
    state: &AppState,
) -> Result<Vec<SessionReport>> {
    collect_session_reports_with_filter(
        shared_codex_root,
        SessionReportFilter {
            current_dir,
            ..SessionReportFilter::default()
        },
        state,
    )
}

pub fn collect_session_reports_with_filter(
    shared_codex_root: &Path,
    filter: SessionReportFilter<'_>,
    state: &AppState,
) -> Result<Vec<SessionReport>> {
    let sessions_root = shared_codex_root.join("sessions");
    let mut session_paths = Vec::new();
    collect_session_paths(&sessions_root, &mut session_paths)?;
    session_paths.sort();

    let mut reports = Vec::new();
    for path in session_paths {
        let Some(report) = read_session_report(&path, state)? else {
            continue;
        };
        if !session_report_matches_filter(&report, &filter) {
            continue;
        }
        reports.push(report);
    }

    sort_session_reports(&mut reports);
    Ok(reports)
}

pub fn session_report_matches_filter(
    report: &SessionReport,
    filter: &SessionReportFilter<'_>,
) -> bool {
    if let Some(current_dir) = filter.current_dir
        && !report.matches_current_dir(current_dir)
    {
        return false;
    }

    if !filter.include_subagents && report.is_subagent() {
        return false;
    }

    if let Some(profile) = filter.profile
        && report.profile.as_deref() != Some(profile)
    {
        return false;
    }

    if let Some(query) = filter
        .query
        .map(str::trim)
        .filter(|query| !query.is_empty())
        && !session_report_matches_query(report, query)
    {
        return false;
    }

    true
}

pub fn session_report_matches_query(report: &SessionReport, query: &str) -> bool {
    let query = query.to_lowercase();
    if query.is_empty() {
        return true;
    }

    [
        Some(report.id.as_str()),
        report.thread_name.as_deref(),
        report.cwd.as_deref(),
        report.profile.as_deref(),
        report.model_provider.as_deref(),
        Some(report.path.as_str()),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_lowercase().contains(&query))
}

pub fn resolve_session_report_by_id<'a>(
    reports: &'a [SessionReport],
    selector: &str,
) -> std::result::Result<&'a SessionReport, SessionResolveError> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(SessionResolveError::Missing {
            selector: selector.to_string(),
        });
    }

    let exact_matches = reports
        .iter()
        .filter(|report| report.id.eq_ignore_ascii_case(selector))
        .collect::<Vec<_>>();
    if exact_matches.len() == 1 {
        return Ok(exact_matches[0]);
    }
    if exact_matches.len() > 1 {
        return Err(SessionResolveError::Ambiguous {
            selector: selector.to_string(),
            matches: session_match_ids(&exact_matches),
        });
    }

    let selector_lower = selector.to_lowercase();
    let prefix_matches = reports
        .iter()
        .filter(|report| report.id.to_lowercase().starts_with(&selector_lower))
        .collect::<Vec<_>>();
    match prefix_matches.len() {
        0 => Err(SessionResolveError::Missing {
            selector: selector.to_string(),
        }),
        1 => Ok(prefix_matches[0]),
        _ => Err(SessionResolveError::Ambiguous {
            selector: selector.to_string(),
            matches: session_match_ids(&prefix_matches),
        }),
    }
}

pub fn repair_resume_session_metadata_prefix(
    shared_codex_root: &Path,
    selector: &str,
) -> Result<Option<PathBuf>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(None);
    }

    let sessions_root = shared_codex_root.join("sessions");
    let mut session_paths = Vec::new();
    collect_session_paths(&sessions_root, &mut session_paths)?;
    session_paths.sort();

    let mut exact_paths = Vec::new();
    for path in &session_paths {
        if let Some(path) = session_file_repair_match(path, selector, true)? {
            exact_paths.push(path);
        }
    }
    for path in exact_paths {
        if repair_session_file_metadata_prefix(&path, selector, true)? {
            return Ok(Some(path));
        }
    }

    let mut prefix_paths = Vec::new();
    for path in &session_paths {
        if let Some(path) = session_file_repair_match(path, selector, false)? {
            prefix_paths.push(path);
        }
    }
    if prefix_paths.len() == 1
        && repair_session_file_metadata_prefix(&prefix_paths[0], selector, true)?
    {
        return Ok(Some(prefix_paths[0].clone()));
    }

    Ok(None)
}

pub fn find_unrepairable_resume_session(
    shared_codex_root: &Path,
    selector: &str,
) -> Result<Option<PathBuf>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(None);
    }

    let sessions_root = shared_codex_root.join("sessions");
    let mut session_paths = Vec::new();
    collect_session_paths(&sessions_root, &mut session_paths)?;
    session_paths.sort();

    for path in session_paths {
        let Some(path) = session_file_repair_match(&path, selector, true)? else {
            continue;
        };
        if session_file_has_resume_metadata_for_selector(&path, selector)? {
            continue;
        }
        return Ok(Some(path));
    }

    Ok(None)
}

fn session_match_ids(matches: &[&SessionReport]) -> Vec<String> {
    matches.iter().map(|report| report.id.clone()).collect()
}

fn collect_session_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_session_paths(&path, paths)?;
        } else if file_type.is_file() && is_session_metadata_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn read_session_report(path: &Path, state: &AppState) -> Result<Option<SessionReport>> {
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));

    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if !session_value_starts_resume_metadata(&value) {
                return Ok(None);
            }
            apply_session_value(&mut report, &value);
        } else {
            if !session_lines_start_resume_metadata(raw.lines()) {
                return Ok(None);
            }
            apply_session_json_lines(&mut report, raw.lines());
        }
    } else {
        let file = fs::File::open(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let reader = io::BufReader::new(file);
        let mut saw_resume_metadata = false;
        for line in reader.lines() {
            let line =
                line.with_context(|| format!("failed to read line from {}", path.display()))?;
            if !saw_resume_metadata {
                if line.trim().is_empty() {
                    continue;
                }
                if !session_line_starts_resume_metadata(&line) {
                    return Ok(None);
                }
                saw_resume_metadata = true;
            }
            apply_session_json_line(&mut report, &line);
        }
        if !saw_resume_metadata {
            return Ok(None);
        }
    }

    if let Some(binding) = state.session_profile_bindings.get(&report.id) {
        report.set_profile(Some(binding.profile_name.clone()));
    }
    Ok(Some(report))
}

fn session_lines_start_resume_metadata<'a>(lines: impl IntoIterator<Item = &'a str>) -> bool {
    lines
        .into_iter()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(session_line_starts_resume_metadata)
}

fn session_line_starts_resume_metadata(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .is_some_and(|value| session_value_starts_resume_metadata(&value))
}

fn session_value_starts_resume_metadata(value: &serde_json::Value) -> bool {
    if session_value_resume_id(value).is_none() {
        return false;
    }

    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|kind| kind == "session_meta")
}

fn repair_session_file_metadata_prefix(
    path: &Path,
    selector: &str,
    synthesize_missing_metadata: bool,
) -> Result<bool> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    let mut lines = raw.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let Some(first_content_index) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return Ok(false);
    };

    if session_line_resume_id_matches(&lines[first_content_index], selector)
        && session_line_starts_resume_metadata(&lines[first_content_index])
    {
        return Ok(false);
    }

    let metadata_index = lines
        .iter()
        .enumerate()
        .skip(first_content_index + 1)
        .find_map(|(index, line)| {
            (session_line_starts_resume_metadata(line)
                && session_line_resume_id_matches(line, selector))
            .then_some(index)
        });

    let Some(metadata_line) = metadata_index.map(|index| lines.remove(index)).or_else(|| {
        synthesize_missing_metadata
            .then(|| synthetic_session_metadata_line(path, selector, &lines))
            .flatten()
    }) else {
        return Ok(false);
    };

    let mut repaired = String::new();
    repaired.push_str(&metadata_line);
    repaired.push('\n');
    for line in lines {
        repaired.push_str(&line);
        repaired.push('\n');
    }

    let backup_path = path.with_extension(format!(
        "{}.prodex-repair-bak",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("session")
    ));
    if !backup_path.exists() {
        fs::write(&backup_path, &raw)
            .with_context(|| format!("failed to backup session {}", path.display()))?;
    }

    let temp_path = path.with_extension(format!(
        "{}.prodex-repair-tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("session")
    ));
    fs::write(&temp_path, repaired)
        .with_context(|| format!("failed to write repaired session {}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to replace repaired session {}", path.display()))?;

    Ok(true)
}

fn session_file_repair_match(path: &Path, selector: &str, exact: bool) -> Result<Option<PathBuf>> {
    if session_path_id_matches_selector(path, selector, exact) {
        return Ok(Some(path.to_path_buf()));
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    for line in raw.lines() {
        if session_line_resume_id_matches_mode(line, selector, exact) {
            return Ok(Some(path.to_path_buf()));
        }
    }
    Ok(None)
}

fn session_file_has_resume_metadata_for_selector(path: &Path, selector: &str) -> Result<bool> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    Ok(raw.lines().any(|line| {
        session_line_starts_resume_metadata(line) && session_line_resume_id_matches(line, selector)
    }))
}

fn session_line_resume_id_matches(line: &str, selector: &str) -> bool {
    session_line_resume_id_matches_mode(line, selector, false)
}

fn session_line_resume_id_matches_mode(line: &str, selector: &str, exact: bool) -> bool {
    session_line_resume_id_matching_mode(line, selector, exact).is_some()
}

fn session_line_resume_id_matching_mode(line: &str, selector: &str, exact: bool) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| session_value_resume_id(&value))
        .filter(|id| session_id_matches_selector(id, selector, exact))
}

fn session_value_resume_id(value: &serde_json::Value) -> Option<String> {
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

fn session_path_id_matches_selector(path: &Path, selector: &str, exact: bool) -> bool {
    session_path_id_matching_selector(path, selector, exact).is_some()
}

fn session_path_id_matching_selector(path: &Path, selector: &str, exact: bool) -> Option<String> {
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

fn session_id_matches_selector(id: &str, selector: &str, exact: bool) -> bool {
    if id.eq_ignore_ascii_case(selector) {
        return true;
    }
    !exact && id.to_lowercase().starts_with(&selector.to_lowercase())
}

fn synthetic_session_metadata_line(
    path: &Path,
    selector: &str,
    lines: &[String],
) -> Option<String> {
    let session_id = lines
        .iter()
        .find_map(|line| session_line_resume_id_matching_mode(line, selector, false))
        .or_else(|| session_path_id_matching_selector(path, selector, false))?;
    let cwd = lines.iter().find_map(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|value| first_string_value(&value, &[&["payload", "cwd"], &["cwd"]]))
    });
    let mut payload = serde_json::Map::new();
    payload.insert("id".to_string(), serde_json::Value::String(session_id));
    if let Some(cwd) = cwd {
        payload.insert("cwd".to_string(), serde_json::Value::String(cwd));
    }

    Some(
        serde_json::json!({
            "type": "session_meta",
            "payload": payload,
        })
        .to_string(),
    )
}

fn file_modified_epoch(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(prodex_core::system_time_to_unix_seconds)
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
