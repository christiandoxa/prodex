use anyhow::{Context, Result};
use prodex_app_reports::{
    SessionReport, apply_session_json_line, apply_session_json_lines, apply_session_value,
    is_session_metadata_file, sort_session_reports,
};
use prodex_state::AppState;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

pub fn collect_session_reports(
    shared_codex_root: &Path,
    current_dir: Option<&Path>,
    state: &AppState,
) -> Result<Vec<SessionReport>> {
    let sessions_root = shared_codex_root.join("sessions");
    let mut session_paths = Vec::new();
    collect_session_paths(&sessions_root, &mut session_paths)?;
    session_paths.sort();

    let mut reports = Vec::new();
    for path in session_paths {
        let report = read_session_report(&path, state)?;
        if current_dir.is_some_and(|current_dir| !report.matches_current_dir(current_dir)) {
            continue;
        }
        reports.push(report);
    }

    sort_session_reports(&mut reports);
    Ok(reports)
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
#[path = "../../../tests/unit/crates/prodex-session-store/src/lib.rs"]
mod tests;
