use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use crate::{AppPaths, AppState, AppStateIoExt};

pub(crate) fn repair_resume_session_metadata_prefix_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<std::path::PathBuf>> {
    let paths = AppPaths::discover()?;
    repair_resume_session_in_shared_home(&paths.shared_codex_root, codex_args)
}

pub(crate) fn repair_resume_session_in_shared_home(
    codex_home: &Path,
    codex_args: &[OsString],
) -> Result<Option<std::path::PathBuf>> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        if prodex_runtime_launch::codex_resume_requested(codex_args) {
            // Picker resumes have no UUID until after the selection. Repair only indexed overlay
            // rows here; do not scan rollout history or manufacture missing sessions.
            prodex_session_store::repair_stale_overlay_rollout_paths(codex_home)?;
        }
        return Ok(None);
    };

    if let Some(path) = repair_resume_session_home_strict(codex_home, session_id)? {
        return Ok(Some(path));
    }

    if prodex_runtime_launch::codex_resume_requested(codex_args) {
        let _ = prodex_session_store::repair_stale_overlay_rollout_path_for_session(
            codex_home, session_id,
        )?;
        prodex_session_store::repair_stale_overlay_rollout_paths(codex_home)?;
        return repair_resume_session_home_strict(codex_home, session_id);
    }
    Ok(None)
}

pub(super) fn repair_resume_session_in_home(
    codex_home: &Path,
    codex_args: &[OsString],
) -> Result<Option<std::path::PathBuf>> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(None);
    };
    let repaired_path = repair_resume_session_home_strict(codex_home, session_id)?;
    repair_resume_session_in_other_profile_homes(codex_home, session_id);
    Ok(repaired_path)
}

fn repair_resume_session_home_strict(
    codex_home: &Path,
    session_id: &str,
) -> Result<Option<std::path::PathBuf>> {
    if let Some(path) =
        prodex_session_store::repair_resume_session_metadata_prefix(codex_home, session_id)?
    {
        return Ok(Some(path));
    }
    if let Some(path) = prodex_session_store::find_resume_session_path(codex_home, session_id)? {
        return Ok(Some(path));
    }
    if let Some(path) =
        prodex_session_store::find_unrepairable_resume_session(codex_home, session_id)?
    {
        bail!(
            "session '{}' cannot be resumed because {} does not contain session metadata; the file is too incomplete to repair",
            session_id,
            path.display()
        );
    }
    Ok(None)
}

fn repair_resume_session_in_other_profile_homes(primary_home: &Path, session_id: &str) {
    let Ok(paths) = AppPaths::discover() else {
        return;
    };
    let mut repaired_homes = BTreeSet::new();
    let Ok(state) = AppState::load(&paths) else {
        repair_resume_session_in_profile_root_dirs(
            &paths,
            primary_home,
            session_id,
            &mut repaired_homes,
        );
        return;
    };
    for profile in state.profiles.values() {
        repair_resume_session_in_profile_home(
            primary_home,
            &profile.codex_home,
            session_id,
            &mut repaired_homes,
        );
    }
    repair_resume_session_in_profile_root_dirs(
        &paths,
        primary_home,
        session_id,
        &mut repaired_homes,
    );
}

fn repair_resume_session_in_profile_root_dirs(
    paths: &AppPaths,
    primary_home: &Path,
    session_id: &str,
    repaired_homes: &mut BTreeSet<String>,
) {
    let Ok(entries) = fs::read_dir(&paths.managed_profiles_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        repair_resume_session_in_profile_home(primary_home, &path, session_id, repaired_homes);
    }
}

fn repair_resume_session_in_profile_home(
    primary_home: &Path,
    profile_home: &Path,
    session_id: &str,
    repaired_homes: &mut BTreeSet<String>,
) {
    if prodex_core::same_path(primary_home, profile_home) {
        return;
    }
    let key = profile_home.display().to_string();
    if !repaired_homes.insert(key) {
        return;
    }
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(profile_home, session_id);
}

pub(super) fn goal_resume_line_has_usage_limit(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    runtime_proxy_crate::runtime_usage_limit_text_message(line)
        || lower.contains("usage limit")
        || lower.contains("insufficient_quota")
        || lower.contains("rate_limit_exceeded")
        || lower.contains("usage_limit_reached")
}
