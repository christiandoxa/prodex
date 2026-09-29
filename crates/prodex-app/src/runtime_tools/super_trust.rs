use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::{AppPaths, AppState, AppStateIoExt};

const SUPER_TRUST_RECENT_SESSION_WORKSPACES: usize = 4_096;
const SUPER_TRUST_CONFIG_MAX_BYTES: usize = 128 * 1024;

#[cfg(windows)]
fn canonical_workspace_alias(path: &Path) -> Option<PathBuf> {
    let path = path.to_string_lossy();
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return Some(PathBuf::from(format!(r"\\{rest}")));
    }
    path.strip_prefix(r"\\?\").map(PathBuf::from)
}

#[cfg(not(windows))]
fn canonical_workspace_alias(_path: &Path) -> Option<PathBuf> {
    None
}

fn lexical_workspace_alias(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut rooted = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
                rooted = true;
            }
            Component::RootDir => {
                normalized.push(component.as_os_str());
                rooted = true;
            }
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::ParentDir) | None if !rooted => normalized.push(".."),
                _ => {}
            },
        }
    }
    normalized
}

fn trusted_workspace_candidates(workspace: PathBuf) -> Vec<PathBuf> {
    let mut candidates = Vec::with_capacity(4);
    candidates.push(workspace.clone());
    let lexical = lexical_workspace_alias(&workspace);
    if lexical != workspace {
        candidates.push(lexical);
    }
    if let Ok(canonical) = workspace.canonicalize() {
        if let Some(alias) = canonical_workspace_alias(&canonical) {
            candidates.push(alias);
        }
        candidates.push(canonical);
    }
    candidates
}

fn trusted_workspaces_codex_args(
    workspaces: impl IntoIterator<Item = PathBuf>,
    codex_args: &[OsString],
) -> Vec<OsString> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    let mut encoded_bytes = "projects={}".len();

    'workspaces: for workspace in workspaces {
        for candidate in trusted_workspace_candidates(workspace) {
            let workspace = candidate.to_string_lossy().into_owned();
            if !seen.insert(workspace.clone()) {
                continue;
            }
            let workspace = serde_json::to_string(&workspace)
                .expect("workspace path should serialize as a TOML-compatible string");
            let entry = format!("{workspace}={{trust_level=\"trusted\"}}");
            let separator_bytes = usize::from(!entries.is_empty());
            let projected = encoded_bytes
                .saturating_add(separator_bytes)
                .saturating_add(entry.len());
            if !entries.is_empty() && projected > SUPER_TRUST_CONFIG_MAX_BYTES {
                break 'workspaces;
            }
            encoded_bytes = projected;
            entries.push(entry);
        }
    }

    let mut args = Vec::with_capacity(codex_args.len() + 2);
    if !entries.is_empty() {
        args.push(OsString::from("-c"));
        args.push(OsString::from(format!(
            "projects={{{}}}",
            entries.join(",")
        )));
    }
    args.extend(codex_args.iter().cloned());
    args
}

pub(crate) fn trusted_workspace_codex_args(
    workspace: &Path,
    codex_args: &[OsString],
) -> Vec<OsString> {
    trusted_workspaces_codex_args([workspace.to_path_buf()], codex_args)
}

fn super_trust_codex_homes(paths: &AppPaths) -> Vec<PathBuf> {
    let mut homes = vec![paths.shared_codex_root.clone()];
    if let Ok(state) = AppState::load(paths) {
        homes.extend(
            state
                .profiles
                .values()
                .map(|profile| profile.codex_home.clone()),
        );
    }
    if let Ok(entries) = std::fs::read_dir(&paths.managed_profiles_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
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
            homes.push(path);
        }
    }
    let mut seen = BTreeSet::new();
    homes.retain(|home| seen.insert(lexical_workspace_alias(home).to_string_lossy().into_owned()));
    homes
}

fn exact_resume_session_path_from_profile_homes(
    paths: &AppPaths,
    session_id: &str,
) -> Option<PathBuf> {
    for home in super_trust_codex_homes(paths) {
        if prodex_core::same_path(&home, &paths.shared_codex_root) {
            continue;
        }
        if let Ok(Some(path)) = prodex_session_store::find_resume_session_path(&home, session_id) {
            return Some(path);
        }
    }
    None
}

fn resume_workspace_from_thread_state(paths: &AppPaths, session_id: &str) -> Option<PathBuf> {
    super_trust_codex_homes(paths).into_iter().find_map(|home| {
        crate::runtime_thread_index::runtime_thread_workspace_for_session(&home, session_id)
    })
}

pub(crate) fn trusted_super_resume_codex_args(
    workspace: &Path,
    resume_session_path: Option<&Path>,
    paths: &AppPaths,
    codex_args: &[OsString],
) -> Result<Vec<OsString>> {
    let mut workspaces = vec![workspace.to_path_buf()];
    let exact_profile_resume_path = resume_session_path
        .is_none()
        .then(|| {
            prodex_runtime_launch::codex_resume_session_id(codex_args).and_then(|session_id| {
                exact_resume_session_path_from_profile_homes(paths, session_id)
            })
        })
        .flatten();
    let resume_session_path = resume_session_path.or(exact_profile_resume_path.as_deref());
    if let Some(session_path) = resume_session_path
        && let Some(resume_workspace) = prodex_session_store::session_cwd_from_path(session_path)
            .with_context(|| {
                format!(
                    "failed to resolve resumed Super workspace from {}",
                    session_path.display()
                )
            })?
        && resume_workspace.is_absolute()
    {
        workspaces.push(resume_workspace);
    }
    if let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args)
        && let Some(resume_workspace) = resume_workspace_from_thread_state(paths, session_id)
    {
        workspaces.push(resume_workspace);
    }

    if let Ok(recent_workspaces) = prodex_session_store::collect_recent_session_workspaces(
        &paths.shared_codex_root,
        SUPER_TRUST_RECENT_SESSION_WORKSPACES,
    ) {
        workspaces.extend(recent_workspaces);
    }

    Ok(trusted_workspaces_codex_args(workspaces, codex_args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_super_resume_trusts_persisted_session_workspace() {
        let root = crate::test_temp_root()
            .join(format!("prodex-super-resume-trust-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let session_path =
            root.join("rollout-2026-09-25T08-45-58-01900000-0000-7000-8000-000000000157.jsonl");
        let resumed_workspace = root.join("resumed-workspace");
        std::fs::create_dir_all(&resumed_workspace).unwrap();
        std::fs::write(
            &session_path,
            format!(
                "{{\"timestamp\":\"2026-09-25T01:45:58Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"01900000-0000-7000-8000-000000000157\",\"cwd\":{},\"originator\":\"codex-tui\",\"cli_version\":\"0.157.0\"}}}}\n",
                serde_json::to_string(
                    &resumed_workspace
                        .join("..")
                        .join("resumed-workspace")
                        .to_string_lossy()
                )
                .unwrap()
            ),
        )
        .unwrap();

        let launch_workspace = root.join("launch-workspace");
        std::fs::create_dir_all(&launch_workspace).unwrap();
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.clone(),
            legacy_shared_codex_root: root.join("legacy"),
        };
        let args =
            trusted_super_resume_codex_args(&launch_workspace, Some(&session_path), &paths, &[])
                .unwrap();

        assert_eq!(args.first(), Some(&OsString::from("-c")));
        let config: toml::Value =
            toml::from_str(args[1].to_str().expect("config override should be UTF-8")).unwrap();
        for workspace in [&launch_workspace, &resumed_workspace] {
            assert_eq!(
                config["projects"][workspace.to_string_lossy().as_ref()]["trust_level"].as_str(),
                Some("trusted")
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bare_super_uuid_pretrusts_workspace_from_managed_profile_session() {
        let root = crate::test_temp_root().join(format!(
            "prodex-super-profile-resume-trust-{}",
            std::process::id()
        ));
        let managed_profiles_root = root.join("profiles");
        let profile_home = managed_profiles_root.join("profile-a");
        let sessions = profile_home.join("sessions/2026/09/29");
        let shared_codex_root = root.join("shared");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::create_dir_all(&shared_codex_root).unwrap();

        let session_id = "01900000-0000-7000-8000-000000000257";
        let resumed_workspace = root.join("ts-exp-py-1");
        let launch_workspace = root.join("launcher");
        std::fs::create_dir_all(&resumed_workspace).unwrap();
        std::fs::create_dir_all(&launch_workspace).unwrap();
        let session_path = sessions.join(format!("rollout-2026-09-29T08-12-51-{session_id}.jsonl"));
        std::fs::write(
            &session_path,
            format!(
                "{{\"timestamp\":\"2026-09-29T01:12:51Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":{},\"timestamp\":\"2026-09-29T01:12:51Z\",\"cwd\":{},\"originator\":\"codex-tui\",\"cli_version\":\"0.158.0\"}}}}\n",
                serde_json::to_string(session_id).unwrap(),
                serde_json::to_string(&resumed_workspace.to_string_lossy()).unwrap(),
            ),
        )
        .unwrap();

        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root,
            shared_codex_root,
            legacy_shared_codex_root: root.join("legacy"),
        };
        let normalized =
            prodex_runtime_launch::normalize_run_codex_args(&[OsString::from(session_id)]);
        assert_eq!(
            prodex_runtime_launch::codex_resume_session_id(&normalized),
            Some(session_id)
        );
        let args =
            trusted_super_resume_codex_args(&launch_workspace, None, &paths, &normalized).unwrap();
        let config: toml::Value =
            toml::from_str(args[1].to_str().expect("config override should be UTF-8")).unwrap();

        for workspace in [&launch_workspace, &resumed_workspace] {
            assert_eq!(
                config["projects"][workspace.to_string_lossy().as_ref()]["trust_level"].as_str(),
                Some("trusted"),
                "workspace={}",
                workspace.display(),
            );
        }

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_super_resume_trusts_managed_profile_state_db_workspace_without_rollout_file() {
        let root = crate::test_temp_root().join(format!(
            "prodex-super-resume-state-db-trust-{}",
            std::process::id()
        ));
        let launch_workspace = root.join("launch-workspace");
        let resumed_workspace = root.join("resumed-workspace");
        let managed_profiles_root = root.join("profiles");
        let profile_home = managed_profiles_root.join("profile-a");
        let shared_codex_root = root.join("shared");
        std::fs::create_dir_all(&launch_workspace).unwrap();
        std::fs::create_dir_all(&resumed_workspace).unwrap();
        std::fs::create_dir_all(&profile_home).unwrap();
        std::fs::create_dir_all(&shared_codex_root).unwrap();

        let session_id = "01900000-0000-7000-8000-000000000271";
        let database = profile_home.join("state_5.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute("CREATE TABLE threads (id TEXT PRIMARY KEY, cwd TEXT)", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, cwd) VALUES (?1, ?2)",
                rusqlite::params![session_id, resumed_workspace.display().to_string()],
            )
            .unwrap();
        drop(connection);

        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root,
            shared_codex_root,
            legacy_shared_codex_root: root.join("legacy"),
        };
        let normalized =
            prodex_runtime_launch::normalize_run_codex_args(&[OsString::from(session_id)]);
        let args =
            trusted_super_resume_codex_args(&launch_workspace, None, &paths, &normalized).unwrap();
        let config: toml::Value =
            toml::from_str(args[1].to_str().expect("config override should be UTF-8")).unwrap();

        for workspace in [&launch_workspace, &resumed_workspace] {
            assert_eq!(
                config["projects"][workspace.to_string_lossy().as_ref()]["trust_level"].as_str(),
                Some("trusted"),
                "workspace={}",
                workspace.display(),
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn super_tui_pretrusts_recent_session_workspaces_for_slash_resume() {
        let root = crate::test_temp_root().join(format!(
            "prodex-super-slash-resume-trust-{}",
            std::process::id()
        ));
        let shared_codex_root = root.join("shared");
        let sessions = shared_codex_root.join("sessions/2026/09/28");
        std::fs::create_dir_all(&sessions).unwrap();

        let workspace_a = root.join("workspace-a");
        let workspace_b = root.join("workspace-b");
        let launch_workspace = root.join("launch-workspace");
        for workspace in [&workspace_a, &workspace_b, &launch_workspace] {
            std::fs::create_dir_all(workspace).unwrap();
        }

        for (name, id, timestamp, workspace) in [
            (
                "rollout-a.jsonl",
                "01900000-0000-7000-8000-000000000201",
                "2026-09-28T01:00:00Z",
                &workspace_a,
            ),
            (
                "rollout-b.jsonl",
                "01900000-0000-7000-8000-000000000202",
                "2026-09-28T02:00:00Z",
                &workspace_b,
            ),
        ] {
            std::fs::write(
                sessions.join(name),
                format!(
                    "{{\"timestamp\":{timestamp},\"type\":\"session_meta\",\"payload\":{{\"id\":{id},\"cwd\":{cwd},\"originator\":\"codex-tui\",\"cli_version\":\"0.157.0\"}}}}\n",
                    timestamp = serde_json::to_string(timestamp).unwrap(),
                    id = serde_json::to_string(id).unwrap(),
                    cwd = serde_json::to_string(&workspace.to_string_lossy()).unwrap(),
                ),
            )
            .unwrap();
        }

        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: shared_codex_root.clone(),
            legacy_shared_codex_root: root.join("legacy"),
        };
        let args = trusted_super_resume_codex_args(&launch_workspace, None, &paths, &[]).unwrap();
        let config: toml::Value =
            toml::from_str(args[1].to_str().expect("config override should be UTF-8")).unwrap();

        for workspace in [&launch_workspace, &workspace_a, &workspace_b] {
            assert_eq!(
                config["projects"][workspace.to_string_lossy().as_ref()]["trust_level"].as_str(),
                Some("trusted"),
                "workspace={}",
                workspace.display(),
            );
        }

        std::fs::remove_dir_all(root).unwrap();
    }
}
