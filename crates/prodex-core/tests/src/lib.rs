use super::*;

#[test]
fn root_temp_file_pid_parses_atomic_write_names() {
    assert_eq!(
        root_temp_file_pid("runtime-backoffs.json.999999999.1.0.tmp"),
        Some(999_999_999)
    );
    assert_eq!(root_temp_file_pid("runtime-backoffs.json.tmp"), None);
}

#[test]
fn stale_root_temp_file_selection_respects_live_pid() {
    assert!(owned_root_temp_file_name(
        "runtime-backoffs.json.999999999.1.0.tmp"
    ));
    assert!(should_remove_stale_root_temp_file(
        "runtime-backoffs.json.999999999.1.0.tmp",
        100,
        50,
        false,
    ));
    assert!(!should_remove_stale_root_temp_file(
        "runtime-backoffs.json.999999999.1.0.tmp",
        100,
        50,
        true,
    ));
}

#[test]
fn runtime_log_selection_removes_oldest_excess_first() {
    let selected = select_runtime_log_paths_to_remove(
        vec![
            (PathBuf::from("new.log"), 30),
            (PathBuf::from("old.log"), 10),
            (PathBuf::from("middle.log"), 20),
        ],
        15,
        2,
    );
    assert_eq!(selected, vec![PathBuf::from("old.log")]);
}

#[test]
fn path_root_checks_normalize_missing_dot_dot_segments() {
    let root = Path::new("/tmp/prodex/profiles");

    assert!(path_is_strictly_under_root(
        root,
        Path::new("/tmp/prodex/profiles/main")
    ));
    assert!(!path_is_strictly_under_root(
        root,
        Path::new("/tmp/prodex/profiles/../outside")
    ));
    assert!(!path_is_under_root(
        root,
        Path::new("/tmp/prodex/profiles/../../profiles-escape")
    ));
}

#[cfg(unix)]
#[test]
fn path_root_checks_resolve_existing_symlink_parent_for_missing_child() {
    let root = env::temp_dir().join(format!(
        "prodex-core-path-root-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let profiles = root.join("profiles");
    let outside = root.join("outside");
    let link = profiles.join("link");
    let candidate = link.join("main");

    fs::create_dir_all(&profiles).expect("profiles root should exist");
    fs::create_dir_all(&outside).expect("outside target should exist");
    std::os::unix::fs::symlink(&outside, &link).expect("symlink parent should exist");

    assert!(!path_is_under_root(&profiles, &candidate));
    assert!(!path_is_strictly_under_root(&profiles, &candidate));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_broker_artifact_key_parses_registry_and_lease_names() {
    assert_eq!(
        runtime_broker_artifact_key("runtime-broker-main.json", false),
        Some("main")
    );
    assert_eq!(
        runtime_broker_artifact_key("runtime-broker-main.json.last-good", false),
        Some("main")
    );
    assert_eq!(
        runtime_broker_artifact_key("runtime-broker-main.capability", false),
        Some("main")
    );
    assert_eq!(
        runtime_broker_artifact_key("runtime-broker-main-leases", true),
        Some("main")
    );
    assert_eq!(
        runtime_broker_artifact_key("runtime-broker-main-leases", false),
        None
    );
}

#[test]
fn session_path_date_extracts_valid_date_components() {
    assert_eq!(
        session_path_date(Path::new("/tmp/sessions/2026/02/28/session.jsonl")),
        Some(PathDate {
            year: 2026,
            month: 2,
            day: 28,
        })
    );
    assert_eq!(
        session_path_date(Path::new("/tmp/sessions/2026/02/29/session.jsonl")),
        None
    );
    assert_eq!(
        session_path_date(Path::new("/tmp/sessions/2024/02/29/session.jsonl")),
        Some(PathDate {
            year: 2024,
            month: 2,
            day: 29,
        })
    );
}
