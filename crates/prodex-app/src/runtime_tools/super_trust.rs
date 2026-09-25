use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

fn trusted_workspaces_codex_args(
    workspaces: impl IntoIterator<Item = PathBuf>,
    codex_args: &[OsString],
) -> Vec<OsString> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for workspace in workspaces {
        let workspace = workspace.to_string_lossy().into_owned();
        if !seen.insert(workspace.clone()) {
            continue;
        }
        let workspace = serde_json::to_string(&workspace)
            .expect("workspace path should serialize as a TOML-compatible string");
        entries.push(format!("{workspace}={{trust_level=\"trusted\"}}"));
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
                serde_json::to_string(&resumed_workspace.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let launch_workspace = root.join("launch-workspace");
        std::fs::create_dir_all(&launch_workspace).unwrap();
        let args =
            trusted_super_resume_codex_args(&launch_workspace, Some(&session_path), &[]).unwrap();

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
}
