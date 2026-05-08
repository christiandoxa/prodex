use anyhow::{Context, Result};
use prodex_app_reports::{
    SessionReport, apply_session_json_line, apply_session_json_lines, apply_session_value,
    is_session_metadata_file, sort_session_reports,
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
        let report = read_session_report(&path, state)?;
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

fn read_session_report(path: &Path, state: &AppState) -> Result<SessionReport> {
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));

    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            apply_session_value(&mut report, &value);
        } else {
            apply_session_json_lines(&mut report, raw.lines());
        }
    } else {
        let file = fs::File::open(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            let line =
                line.with_context(|| format!("failed to read line from {}", path.display()))?;
            apply_session_json_line(&mut report, &line);
        }
    }

    if let Some(binding) = state.session_profile_bindings.get(&report.id) {
        report.set_profile(Some(binding.profile_name.clone()));
    }
    Ok(report)
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
