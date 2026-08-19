use super::*;

pub(super) fn collect_exact_repair_candidates(
    candidates: &[SessionRepairCandidate],
    selector: &str,
    selector_is_full: bool,
) -> Result<Vec<SessionRepairCandidate>> {
    let mut exact_paths = Vec::new();
    for candidate in candidates {
        if candidate.state_db_match_kind == Some(SessionRepairMatchKind::Exact) {
            exact_paths.push(candidate.clone());
            continue;
        }
        if selector_is_full {
            collect_full_exact_repair_candidate(candidate, selector, &mut exact_paths);
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
    Ok(exact_paths)
}

fn collect_full_exact_repair_candidate(
    candidate: &SessionRepairCandidate,
    selector: &str,
    exact_paths: &mut Vec<SessionRepairCandidate>,
) {
    let path = if session_path_id_matches_selector(&candidate.path, selector, true) {
        Some(candidate.path.clone())
    } else {
        session_file_repair_match(&candidate.path, selector, true)
            .ok()
            .flatten()
    };
    if let Some(path) = path {
        exact_paths.push(SessionRepairCandidate {
            path,
            state_db_match_kind: None,
            resolved_session_id: None,
        });
    }
}

pub(super) fn collect_prefix_repair_candidates(
    candidates: &[SessionRepairCandidate],
    selector: &str,
) -> Result<Vec<SessionRepairCandidate>> {
    let mut prefix_paths = Vec::new();
    for candidate in candidates {
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
    Ok(prefix_paths)
}

pub(super) fn repair_session_candidate(
    shared_codex_root: &Path,
    candidate: &SessionRepairCandidate,
    selector: &str,
) -> Result<bool> {
    let repair_selector = candidate.resolved_session_id.as_deref().unwrap_or(selector);
    let repaired = repair_session_file_metadata_prefix(
        shared_codex_root,
        &candidate.path,
        repair_selector,
        true,
    )?;
    let _ = repair_state_db_rollout_path(shared_codex_root, &candidate.path)?;
    Ok(repaired)
}

pub(super) fn unrepairable_candidate_path(
    candidate: &SessionRepairCandidate,
    selector: &str,
    selector_is_full: bool,
) -> Result<Option<PathBuf>> {
    if candidate.state_db_match_kind == Some(SessionRepairMatchKind::Exact) {
        return Ok(Some(candidate.path.clone()));
    }
    if selector_is_full {
        return Ok(
            session_path_id_matches_selector(&candidate.path, selector, true)
                .then(|| candidate.path.clone()),
        );
    }
    session_file_repair_match(&candidate.path, selector, true)
}
