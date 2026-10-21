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
    // Rust projects OS-aware components without losing non-UTF-8 names.
    // Compiled Mojo alone chooses which original segments survive.
    let components = path.components().collect::<Vec<_>>();
    let kinds = components
        .iter()
        .map(|component| match component {
            Component::Prefix(_) => 0_i64,
            Component::RootDir => 1,
            Component::CurDir => 2,
            Component::Normal(_) => 3,
            Component::ParentDir => 4,
        })
        .collect::<Vec<_>>();
    let indices =
        prodex_mojo_core::runtime_overlay_policy::lexical_workspace_component_indices(&kinds)
            .expect("compiled Mojo Super lexical path planner returned invalid output");
    let mut normalized = PathBuf::new();
    for index in indices {
        normalized.push(components[index].as_os_str());
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
    // The Rust host resolves the original/lexical/canonical OS paths and
    // serializes TOML-compatible entries. Compiled Mojo owns exact path
    // deduplication, first occurrence and bounded config selection.
    let mut candidates = Vec::<(String, String)>::new();
    for workspace in workspaces {
        for candidate in trusted_workspace_candidates(workspace) {
            let workspace = candidate.to_string_lossy().into_owned();
            let serialized = serde_json::to_string(&workspace)
                .expect("workspace path should serialize as a TOML-compatible string");
            let entry = format!("{serialized}={{trust_level=\"trusted\"}}");
            candidates.push((workspace, entry));
        }
    }
    let inputs = candidates
        .iter()
        .map(|(path, entry)| (path.as_str(), entry.len()))
        .collect::<Vec<_>>();
    let selected = prodex_mojo_core::runtime_overlay_policy::workspace_trust_entry_indices(
        &inputs,
        SUPER_TRUST_CONFIG_MAX_BYTES,
    )
    .expect("compiled Mojo Super trust entry planner returned invalid output");
    let entries = selected
        .into_iter()
        .map(|index| candidates[index].1.as_str())
        .collect::<Vec<_>>();

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

fn deduplicated_super_trust_homes(homes: Vec<PathBuf>) -> Vec<PathBuf> {
    // Host computes the platform lexical alias; compiled Mojo chooses the
    // first original home for each exact alias without a Rust dedup fallback.
    let keys = homes
        .iter()
        .map(|home| lexical_workspace_alias(home).to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let records = keys
        .iter()
        .map(|key| (key.as_str(), 0usize))
        .collect::<Vec<_>>();
    let selected = prodex_mojo_core::runtime_overlay_policy::workspace_trust_entry_indices(
        &records,
        usize::MAX,
    )
    .expect("compiled Mojo Super profile-home dedup planner failed");
    let mut slots = homes.into_iter().map(Some).collect::<Vec<_>>();
    selected
        .into_iter()
        .map(|index| {
            slots[index]
                .take()
                .expect("compiled Mojo Super profile-home index repeated")
        })
        .collect()
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
    deduplicated_super_trust_homes(homes)
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
    fn compiled_super_profile_home_dedup_preserves_first_lexical_alias() {
        let source = vec![
            PathBuf::from("/home/test-user/alpha/../beta"),
            PathBuf::from("/home/test-user/beta"),
            PathBuf::from("/home/test-user/Beta"),
            PathBuf::from("/home/test-user/α"),
            PathBuf::from("/home/test-user/α/"),
        ];
        assert_eq!(
            deduplicated_super_trust_homes(source),
            vec![
                PathBuf::from("/home/test-user/alpha/../beta"),
                PathBuf::from("/home/test-user/Beta"),
                PathBuf::from("/home/test-user/α"),
            ]
        );
        assert!(deduplicated_super_trust_homes(vec![]).is_empty());
    }

    #[test]
    fn compiled_lexical_workspace_alias_handles_rooted_and_relative_parent_paths() {
        assert_eq!(
            lexical_workspace_alias(Path::new("/home/test-user/./workspace/../repo")),
            PathBuf::from("/home/test-user/repo")
        );
        assert_eq!(
            lexical_workspace_alias(Path::new("/../../home/test-user/../workspace")),
            PathBuf::from("/home/workspace")
        );
        assert_eq!(
            lexical_workspace_alias(Path::new("../alpha/./beta/../../delta")),
            PathBuf::from("../delta")
        );
        assert_eq!(
            lexical_workspace_alias(Path::new("../../alpha/../")),
            PathBuf::from("../..")
        );
    }

    #[cfg(unix)]
    #[test]
    fn compiled_lexical_workspace_alias_preserves_opaque_non_utf8_components() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let path = PathBuf::from(OsString::from_vec(
            b"/home/test-user/opaque-\xff/../target-\xfe".to_vec(),
        ));
        let normalized = lexical_workspace_alias(&path);
        assert_eq!(
            normalized.as_os_str().as_bytes(),
            b"/home/test-user/target-\xfe"
        );
    }

    #[test]
    fn compiled_super_trust_selection_deduplicates_exact_paths_and_preserves_order() {
        let args = trusted_workspaces_codex_args(
            [
                PathBuf::from("/home/test-user/alpha"),
                PathBuf::from("/home/test-user/alpha"),
                PathBuf::from("/home/test-user/ALPHA"),
                PathBuf::from("/home/test-user/beta"),
            ],
            &[OsString::from("exec"), OsString::from("hello")],
        );
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "-c");
        let config = args[1].to_string_lossy();
        assert_eq!(config.matches("trust_level").count(), 3);
        assert!(
            config.find("/home/test-user/alpha").unwrap()
                < config.find("/home/test-user/ALPHA").unwrap()
        );
        assert!(
            config.find("/home/test-user/ALPHA").unwrap()
                < config.find("/home/test-user/beta").unwrap()
        );
        assert_eq!(args[2], "exec");
        assert_eq!(args[3], "hello");
    }

    #[test]
    fn compiled_super_trust_byte_cap_stops_on_first_overflow() {
        let workspaces = (0..500)
            .map(|index| PathBuf::from(format!("/home/test-user/{index:04}-{}", "abc".repeat(100))))
            .collect::<Vec<_>>();
        let args = trusted_workspaces_codex_args(workspaces, &[]);
        assert_eq!(args.len(), 2);
        let config = args[1].to_string_lossy();
        assert!(config.len() <= SUPER_TRUST_CONFIG_MAX_BYTES);
        assert!(config.contains("/home/test-user/0000-"));
        assert!(!config.contains("/home/test-user/0499-"));

        // Previous contract: a single entry must be retained even if
        // the configured byte budget is exceeded.
        let first = PathBuf::from(format!("/home/test-user/{}", "x".repeat(132_000)));
        let args = trusted_workspaces_codex_args([first], &[]);
        assert!(args[1].to_string_lossy().len() > SUPER_TRUST_CONFIG_MAX_BYTES);
    }

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
