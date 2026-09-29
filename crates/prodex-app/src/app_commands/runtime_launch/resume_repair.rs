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
    repair_resume_session_for_launch(&paths, codex_args)
}

pub(crate) fn repair_resume_session_for_launch(
    paths: &AppPaths,
    codex_args: &[OsString],
) -> Result<Option<std::path::PathBuf>> {
    let session_file = repair_resume_session_in_shared_home(&paths.shared_codex_root, codex_args)?;
    if let Some(session_file) = session_file.as_deref() {
        prodex_shared_codex_fs::maintain_managed_codex_session_file(paths, session_file)?;
    }
    Ok(session_file)
}

pub(crate) fn repair_super_resume_session_metadata(args: &prodex_cli::SuperArgs) -> Result<()> {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(&args.codex_args);
    let picker_args = [OsString::from("resume")];
    let repair_args = if prodex_runtime_launch::codex_resume_requested(&normalized) {
        normalized.as_slice()
    } else {
        picker_args.as_slice()
    };
    repair_resume_session_metadata_prefix_from_codex_args(repair_args)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resume_repair_rewrites_stale_overlay_attachment_paths_before_launch() {
        let root = std::env::temp_dir().join(format!(
            "prodex-resume-attachment-repair-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join(".codex"),
            legacy_shared_codex_root: root.join("legacy"),
        };
        let session_id = "01900000-0000-7000-8000-000000000434";
        let attachment_id = "34d42e43-d282-44e7-8786-74f086b8e151";
        let stale_overlay = paths.managed_profiles_root.join(".prodex-overlay-dead-0");
        let stale_text = stale_overlay
            .join("attachments")
            .join(attachment_id)
            .join("pasted-text-1.txt");
        let stale_image = stale_overlay
            .join("attachments")
            .join(attachment_id)
            .join("image-1.png");

        let stable_dir = paths
            .shared_codex_root
            .join("attachments")
            .join(attachment_id);
        fs::create_dir_all(&stable_dir).unwrap();
        let stable_text = stable_dir.join("pasted-text-1.txt");
        let stable_image = stable_dir.join("image-1.png");
        fs::write(&stable_text, b"durable pasted text").unwrap();
        fs::write(&stable_image, b"durable image").unwrap();

        let session_file = paths
            .shared_codex_root
            .join("sessions/2026/09/29")
            .join(format!("rollout-2026-09-29T15-00-00-{session_id}.jsonl"));
        fs::create_dir_all(session_file.parent().unwrap()).unwrap();
        fs::write(
            &session_file,
            format!(
                "{{\"timestamp\":\"2026-09-29T08:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"/tmp/workspace\",\"originator\":\"codex-cli\",\"cli_version\":\"0.159.0\"}}}}\n\
                 {{\"timestamp\":\"2026-09-29T08:00:01Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"pasted text file: {}\\nimage file: {}\"}}]}}}}\n",
                stale_text.display(),
                stale_image.display(),
            ),
        )
        .unwrap();

        assert!(
            !stale_overlay.exists(),
            "the old overlay must already be gone"
        );

        let args = [OsString::from("resume"), OsString::from(session_id)];
        let repaired =
            repair_resume_session_for_launch(&paths, &args).expect("resume repair should succeed");
        assert_eq!(repaired.as_deref(), Some(session_file.as_path()));

        let contents = fs::read_to_string(&session_file).unwrap();
        assert!(
            contents.contains(&stable_text.display().to_string()),
            "{contents}"
        );
        assert!(
            contents.contains(&stable_image.display().to_string()),
            "{contents}"
        );
        assert!(
            !contents.contains(&stale_overlay.display().to_string()),
            "{contents}"
        );
        assert_eq!(fs::read(&stable_text).unwrap(), b"durable pasted text");
        assert_eq!(fs::read(&stable_image).unwrap(), b"durable image");

        fs::remove_dir_all(root).unwrap();
    }
}
