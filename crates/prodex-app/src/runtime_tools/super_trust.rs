use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

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

pub(crate) fn trusted_super_resume_codex_args(
    workspace: &Path,
    resume_session_path: Option<&Path>,
    shared_codex_root: &Path,
    codex_args: &[OsString],
) -> Result<Vec<OsString>> {
    let mut workspaces = vec![workspace.to_path_buf()];
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

    if let Ok(recent_workspaces) = prodex_session_store::collect_recent_session_workspaces(
        shared_codex_root,
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
        let args =
            trusted_super_resume_codex_args(&launch_workspace, Some(&session_path), &root, &[])
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

        let args =
            trusted_super_resume_codex_args(&launch_workspace, None, &shared_codex_root, &[])
                .unwrap();
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
