use anyhow::Result;
use prodex_mojo_core::json::SessionRecentWorkspaceCandidate;
use std::path::{Path, PathBuf};

use super::{
    ARCHIVED_SESSIONS_DIR, SESSIONS_DIR, SessionReport, apply_session_json_line,
    collect_session_paths, file_modified_epoch, read_json_session_report,
    session_line_starts_resume_metadata, visit_session_lines,
};

#[derive(Debug)]
struct RecentWorkspaceCandidate {
    workspace: String,
    modified_epoch: i64,
    absolute: bool,
}

pub fn collect_recent_session_workspaces(
    shared_codex_root: &Path,
    max_workspaces: usize,
) -> Result<Vec<PathBuf>> {
    if max_workspaces == 0 {
        return Ok(Vec::new());
    }

    let candidates = collect_recent_workspace_candidates(shared_codex_root)?;
    select_recent_workspace_candidates(&candidates, max_workspaces)
}

fn select_recent_workspace_candidates(
    candidates: &[RecentWorkspaceCandidate],
    max_workspaces: usize,
) -> Result<Vec<PathBuf>> {
    let candidate_views = candidates
        .iter()
        .map(|candidate| SessionRecentWorkspaceCandidate {
            modified_epoch: candidate.modified_epoch,
            absolute: candidate.absolute,
            workspace: &candidate.workspace,
        })
        .collect::<Vec<_>>();
    let selected =
        prodex_mojo_core::json::session_recent_workspace_plan(&candidate_views, max_workspaces)
            .expect("Mojo recent-session workspace planner returned invalid output");
    Ok(selected
        .into_iter()
        .map(|index| PathBuf::from(&candidates[index].workspace))
        .collect())
}

fn collect_recent_workspace_candidates(
    shared_codex_root: &Path,
) -> Result<Vec<RecentWorkspaceCandidate>> {
    let mut session_paths = Vec::new();
    collect_session_paths(&shared_codex_root.join(SESSIONS_DIR), &mut session_paths)?;
    collect_session_paths(
        &shared_codex_root.join(ARCHIVED_SESSIONS_DIR),
        &mut session_paths,
    )?;
    let mut candidates = Vec::new();
    for path in session_paths {
        let Ok(Some(workspace)) = session_initial_cwd_from_path(&path) else {
            continue;
        };
        let absolute = workspace.is_absolute();
        let Some(workspace) = workspace.to_str() else {
            continue;
        };
        candidates.push(RecentWorkspaceCandidate {
            workspace: workspace.to_owned(),
            modified_epoch: file_modified_epoch(&path).unwrap_or(0),
            absolute,
        });
    }
    Ok(candidates)
}

fn session_initial_cwd_from_path(path: &Path) -> Result<Option<PathBuf>> {
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));
    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        if !read_json_session_report(path, &mut report)? {
            return Ok(None);
        }
        return Ok(report.cwd.map(PathBuf::from));
    }

    let mut saw_resume_metadata = false;
    visit_session_lines(path, |line| {
        if line.trim().is_empty() {
            return true;
        }
        if !session_line_starts_resume_metadata(line) {
            return false;
        }
        apply_session_json_line(&mut report, line);
        saw_resume_metadata = true;
        false
    })?;
    if !saw_resume_metadata {
        return Ok(None);
    }
    Ok(report.cwd.map(PathBuf::from))
}
