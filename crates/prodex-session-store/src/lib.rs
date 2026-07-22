use anyhow::{Context, Result};
use prodex_state::AppState;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod repair_transaction;
mod report;
mod resolve_error;
mod session_file;
mod session_meta;
mod state_db_index;

pub use report::{
    SessionReport, apply_session_json_line, apply_session_json_lines, apply_session_value,
    first_i64_value, first_string_value, format_epoch, is_session_metadata_file,
    session_id_from_path, sort_session_reports, timestamp_label_sort_key, value_at_path,
};
pub use resolve_error::*;

use repair_transaction::SessionRepairTransaction;
use session_file::{read_session_file_to_string, visit_session_lines};
use state_db_index::{collect_state_db_rollout_paths, repair_state_db_rollout_path};

const SESSIONS_DIR: &str = "sessions";
const ARCHIVED_SESSIONS_DIR: &str = "archived_sessions";
const SESSION_STORE_FILE_MAX_BYTES: u64 = if cfg!(test) {
    64 * 1024
} else {
    64 * 1024 * 1024
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionReportFilter<'a> {
    pub current_dir: Option<&'a Path>,
    pub profile: Option<&'a str>,
    pub query: Option<&'a str>,
    pub include_subagents: bool,
}

#[derive(Debug, Clone)]
struct SessionRepairCandidate {
    path: PathBuf,
    state_db_match_kind: Option<SessionRepairMatchKind>,
    resolved_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SessionRepairMatchKind {
    Prefix,
    Exact,
}

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
    let mut session_paths = Vec::new();
    collect_session_paths(&shared_codex_root.join(SESSIONS_DIR), &mut session_paths)?;
    collect_session_paths(
        &shared_codex_root.join(ARCHIVED_SESSIONS_DIR),
        &mut session_paths,
    )?;
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

pub fn resolve_session_report_by_id_in_store(
    shared_codex_root: &Path,
    state: &AppState,
    selector: &str,
) -> std::result::Result<SessionReport, SessionResolveError> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(SessionResolveError::Missing {
            selector: selector.to_string(),
        });
    }

    if full_codex_session_id(selector).is_some() {
        let exact_matches =
            resolve_exact_session_reports_by_id_in_store(shared_codex_root, state, selector)?;
        if prodex_debug_resume_repair_enabled() {
            eprintln!(
                "prodex-session-store: resolve exact selector={} matches={}",
                selector,
                exact_matches.len()
            );
        }
        return match exact_matches.len() {
            0 => resolve_session_report_by_id_in_store_slow(shared_codex_root, state, selector),
            1 => Ok(exact_matches
                .into_iter()
                .next()
                .expect("exact match should exist")),
            _ => Err(SessionResolveError::Ambiguous {
                selector: selector.to_string(),
                matches: exact_matches.into_iter().map(|report| report.id).collect(),
            }),
        };
    }

    resolve_session_report_by_id_in_store_slow(shared_codex_root, state, selector)
}

pub fn repair_resume_session_metadata_prefix(
    shared_codex_root: &Path,
    selector: &str,
) -> Result<Option<PathBuf>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(None);
    }
    let selector_is_full = full_codex_session_id(selector).is_some();

    let session_paths = collect_resume_repair_candidate_paths(shared_codex_root, selector)?;
    if prodex_debug_resume_repair_enabled() {
        eprintln!(
            "prodex-session-store: repair selector={} full={} candidates={}",
            selector,
            selector_is_full,
            session_paths.len()
        );
    }

    let mut exact_paths = Vec::new();
    for candidate in &session_paths {
        if candidate.state_db_match_kind == Some(SessionRepairMatchKind::Exact) {
            exact_paths.push(candidate.clone());
            continue;
        }
        if selector_is_full {
            if session_path_id_matches_selector(&candidate.path, selector, true) {
                exact_paths.push(SessionRepairCandidate {
                    path: candidate.path.clone(),
                    state_db_match_kind: None,
                    resolved_session_id: None,
                });
            } else if let Ok(Some(path)) =
                session_file_repair_match(&candidate.path, selector, true)
            {
                exact_paths.push(SessionRepairCandidate {
                    path,
                    state_db_match_kind: None,
                    resolved_session_id: None,
                });
            }
            continue;
        }
        if let Some(path) = session_file_repair_match(&candidate.path, selector, true)? {
            exact_paths.push(SessionRepairCandidate {
                path,
                state_db_match_kind: None,
                resolved_session_id: None,
            });
        }
    }
    for candidate in exact_paths {
        let repair_selector = candidate.resolved_session_id.as_deref().unwrap_or(selector);
        let repaired = repair_session_file_metadata_prefix(
            shared_codex_root,
            &candidate.path,
            repair_selector,
            true,
        )?;
        repair_state_db_rollout_path(shared_codex_root, &candidate.path)?;
        if repaired {
            return Ok(Some(candidate.path));
        }
    }
    if prodex_debug_resume_repair_enabled() {
        eprintln!(
            "prodex-session-store: repair selector={} exact_paths_done full={}",
            selector, selector_is_full
        );
    }
    if selector_is_full {
        return Ok(None);
    }

    let mut prefix_paths = Vec::new();
    for candidate in &session_paths {
        if candidate.state_db_match_kind == Some(SessionRepairMatchKind::Prefix) {
            prefix_paths.push(candidate.clone());
            continue;
        }
        if let Some(path) = session_file_repair_match(&candidate.path, selector, false)? {
            prefix_paths.push(SessionRepairCandidate {
                path,
                state_db_match_kind: None,
                resolved_session_id: None,
            });
        }
    }
    if prefix_paths.len() == 1 {
        let repaired = repair_session_file_metadata_prefix(
            shared_codex_root,
            &prefix_paths[0].path,
            prefix_paths[0]
                .resolved_session_id
                .as_deref()
                .unwrap_or(selector),
            true,
        )?;
        repair_state_db_rollout_path(shared_codex_root, &prefix_paths[0].path)?;
        if repaired {
            return Ok(Some(prefix_paths[0].path.clone()));
        }
    }

    Ok(None)
}

pub fn repair_codex_session_metadata_prefix(path: &Path, _contents: &str) -> Result<bool> {
    let Some(selector) = codex_session_id_from_path(path) else {
        return Ok(false);
    };
    repair_session_file_metadata_prefix(
        repair_transaction::repository_root(path),
        path,
        &selector,
        true,
    )
}

pub fn find_unrepairable_resume_session(
    shared_codex_root: &Path,
    selector: &str,
) -> Result<Option<PathBuf>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(None);
    }
    let selector_is_full = full_codex_session_id(selector).is_some();

    let session_paths = collect_resume_repair_candidate_paths(shared_codex_root, selector)?;
    if prodex_debug_resume_repair_enabled() {
        eprintln!(
            "prodex-session-store: unrepairable selector={} full={} candidates={}",
            selector,
            selector_is_full,
            session_paths.len()
        );
    }

    for candidate in session_paths {
        let path = if candidate.state_db_match_kind == Some(SessionRepairMatchKind::Exact) {
            candidate.path
        } else {
            if selector_is_full {
                if !session_path_id_matches_selector(&candidate.path, selector, true) {
                    continue;
                }
                candidate.path
            } else {
                let Some(path) = session_file_repair_match(&candidate.path, selector, true)? else {
                    continue;
                };
                path
            }
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

fn resolve_session_report_by_id_in_store_slow(
    shared_codex_root: &Path,
    state: &AppState,
    selector: &str,
) -> std::result::Result<SessionReport, SessionResolveError> {
    let reports = collect_session_reports(shared_codex_root, None, state).map_err(|_| {
        SessionResolveError::Missing {
            selector: selector.to_string(),
        }
    })?;
    resolve_session_report_by_id(&reports, selector).cloned()
}

fn resolve_exact_session_reports_by_id_in_store(
    shared_codex_root: &Path,
    state: &AppState,
    selector: &str,
) -> std::result::Result<Vec<SessionReport>, SessionResolveError> {
    let mut session_paths = Vec::new();
    collect_exact_session_paths_for_selector(shared_codex_root, selector, &mut session_paths)
        .map_err(|_| SessionResolveError::Missing {
            selector: selector.to_string(),
        })?;
    session_paths.sort();
    session_paths.dedup();

    let mut matches = Vec::new();
    for path in session_paths {
        let Some(report) =
            read_session_report(&path, state).map_err(|_| SessionResolveError::Missing {
                selector: selector.to_string(),
            })?
        else {
            continue;
        };
        if report.id.eq_ignore_ascii_case(selector) {
            matches.push(report);
        }
    }
    Ok(matches)
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

fn collect_exact_session_paths_for_selector(
    root: &Path,
    selector: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    collect_exact_session_paths(&root.join(SESSIONS_DIR), selector, paths)?;
    collect_exact_session_paths(&root.join(ARCHIVED_SESSIONS_DIR), selector, paths)?;
    let mut state_db_paths = Vec::new();
    collect_state_db_rollout_paths(root, selector, &mut state_db_paths);
    paths.extend(state_db_paths.into_iter().map(|candidate| candidate.path));
    Ok(())
}

fn collect_exact_session_paths(
    root: &Path,
    selector: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
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
            collect_exact_session_paths(&path, selector, paths)?;
        } else if file_type.is_file()
            && is_session_metadata_file(&path)
            && session_path_id_matches_selector(&path, selector, true)
        {
            paths.push(path);
        }
    }

    Ok(())
}

fn collect_resume_repair_candidate_paths(
    root: &Path,
    selector: &str,
) -> Result<Vec<SessionRepairCandidate>> {
    let mut paths = Vec::new();
    let mut session_paths = Vec::new();
    let selector_is_full = full_codex_session_id(selector).is_some();
    if selector_is_full {
        collect_exact_session_paths(&root.join(SESSIONS_DIR), selector, &mut session_paths)?;
        collect_exact_session_paths(
            &root.join(ARCHIVED_SESSIONS_DIR),
            selector,
            &mut session_paths,
        )?;
    } else {
        collect_session_paths(&root.join(SESSIONS_DIR), &mut session_paths)?;
        collect_session_paths(&root.join(ARCHIVED_SESSIONS_DIR), &mut session_paths)?;
    }
    paths.extend(
        session_paths
            .into_iter()
            .map(|path| SessionRepairCandidate {
                path,
                state_db_match_kind: None,
                resolved_session_id: None,
            }),
    );
    collect_state_db_rollout_paths(root, selector, &mut paths);
    if selector_is_full && paths.is_empty() {
        let mut fallback_paths = Vec::new();
        collect_session_paths(&root.join(SESSIONS_DIR), &mut fallback_paths)?;
        collect_session_paths(&root.join(ARCHIVED_SESSIONS_DIR), &mut fallback_paths)?;
        paths.extend(
            fallback_paths
                .into_iter()
                .map(|path| SessionRepairCandidate {
                    path,
                    state_db_match_kind: None,
                    resolved_session_id: None,
                }),
        );
    }
    paths.sort_by(|left, right| left.path.cmp(&right.path));

    let mut deduped: Vec<SessionRepairCandidate> = Vec::with_capacity(paths.len());
    for candidate in paths {
        if let Some(existing) = deduped.last_mut()
            && existing.path == candidate.path
        {
            existing.state_db_match_kind = existing
                .state_db_match_kind
                .max(candidate.state_db_match_kind);
            if existing.resolved_session_id.is_none() {
                existing.resolved_session_id = candidate.resolved_session_id;
            }
            continue;
        }
        deduped.push(candidate);
    }
    Ok(deduped)
}

fn read_session_report(path: &Path, state: &AppState) -> Result<Option<SessionReport>> {
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));

    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        let raw = read_session_file_to_string(path)?;
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
        let mut saw_resume_metadata = false;
        let mut valid_resume_metadata = true;
        visit_session_lines(path, |line| {
            if !saw_resume_metadata {
                if line.trim().is_empty() {
                    return true;
                }
                if !session_line_starts_resume_metadata(line) {
                    valid_resume_metadata = false;
                    return false;
                }
                saw_resume_metadata = true;
            }
            apply_session_json_line(&mut report, line);
            true
        })?;
        if !saw_resume_metadata || !valid_resume_metadata {
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
    repository_root: &Path,
    path: &Path,
    selector: &str,
    synthesize_missing_metadata: bool,
) -> Result<bool> {
    if fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect session {}", path.display()))?
        .len()
        > SESSION_STORE_FILE_MAX_BYTES
    {
        let mut first_line_is_matching_codex_metadata = false;
        visit_session_lines(path, |line| {
            if line.trim().is_empty() {
                return true;
            }
            first_line_is_matching_codex_metadata = session_line_resume_id_matches(line, selector)
                && session_line_starts_resume_metadata(line)
                && session_meta::line_starts_codex_rollout_metadata(line);
            false
        })?;
        if first_line_is_matching_codex_metadata {
            return Ok(false);
        }
    }
    let transaction =
        SessionRepairTransaction::begin(repository_root, path, SESSION_STORE_FILE_MAX_BYTES)?;
    let raw = transaction.contents();
    let lines = raw.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let Some(first_content_index) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return Ok(false);
    };

    let first_line_is_matching_metadata =
        session_line_resume_id_matches(&lines[first_content_index], selector)
            && session_line_starts_resume_metadata(&lines[first_content_index]);
    let first_line_is_matching_codex_metadata = first_line_is_matching_metadata
        && session_meta::line_starts_codex_rollout_metadata(&lines[first_content_index]);
    let has_unreadable_lines = lines
        .iter()
        .any(|line| !line.trim().is_empty() && !session_line_is_valid_json(line));

    if first_line_is_matching_codex_metadata && !has_unreadable_lines {
        return Ok(false);
    }

    let metadata_index = lines
        .iter()
        .enumerate()
        .skip(first_content_index + 1)
        .find_map(|(index, line)| {
            (session_meta::line_starts_codex_rollout_metadata(line)
                && session_line_resume_id_matches(line, selector))
            .then_some(index)
        });

    let metadata_line = if first_line_is_matching_codex_metadata {
        lines[first_content_index].clone()
    } else if let Some(index) = metadata_index {
        lines[index].clone()
    } else if synthesize_missing_metadata {
        let Some(line) = session_meta::synthetic_session_metadata_line(path, selector, &lines)
        else {
            return Ok(false);
        };
        line
    } else {
        return Ok(false);
    };

    let mut repaired = String::new();
    repaired.push_str(&metadata_line);
    repaired.push('\n');
    for (index, line) in lines.iter().enumerate() {
        if first_line_is_matching_metadata && index == first_content_index {
            continue;
        }
        if metadata_index == Some(index) {
            continue;
        }
        if line.trim().is_empty() || !session_line_is_valid_json(line) {
            continue;
        }
        if session_line_starts_resume_metadata(line)
            && session_line_resume_id_matches(line, selector)
        {
            continue;
        }
        repaired.push_str(line);
        repaired.push('\n');
    }

    transaction.commit(repaired.as_bytes())?;

    Ok(true)
}

fn session_file_repair_match(path: &Path, selector: &str, exact: bool) -> Result<Option<PathBuf>> {
    if session_path_id_matches_selector(path, selector, exact) {
        return Ok(Some(path.to_path_buf()));
    }
    let mut matched = false;
    visit_session_lines(path, |line| {
        matched = session_line_resume_id_matches_mode(line, selector, exact);
        !matched
    })?;
    Ok(matched.then(|| path.to_path_buf()))
}

fn session_file_has_resume_metadata_for_selector(path: &Path, selector: &str) -> Result<bool> {
    let mut matched = false;
    visit_session_lines(path, |line| {
        matched = session_meta::line_starts_codex_rollout_metadata(line)
            && session_line_resume_id_matches(line, selector);
        !matched
    })?;
    Ok(matched)
}

fn session_line_resume_id_matches(line: &str, selector: &str) -> bool {
    session_line_resume_id_matches_mode(line, selector, false)
}

fn session_line_is_valid_json(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line).is_ok()
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

fn full_codex_session_id(selector: &str) -> Option<&str> {
    let bytes = selector.as_bytes();
    let valid = bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    valid.then_some(selector)
}

fn codex_session_id_from_path(path: &Path) -> Option<String> {
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

fn prodex_debug_resume_repair_enabled() -> bool {
    env::var_os("PRODEX_DEBUG_RESUME_REPAIR").is_some()
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
