use super::*;
use std::collections::BTreeMap;

#[test]
fn cleanup_summary_total_and_merge_count_all_fields() {
    let left = ProdexCleanupSummary {
        duplicate_profiles_removed: 1,
        runtime_logs_removed: 2,
        ..ProdexCleanupSummary::default()
    };
    let right = ProdexCleanupSummary {
        stale_login_dirs_removed: 4,
        dead_runtime_broker_registries_removed: 5,
        ..ProdexCleanupSummary::default()
    };

    let merged = left.merge(right);

    assert_eq!(merged.total_removed(), 12);
    assert_eq!(merged.runtime_logs_removed, 2);
    assert_eq!(merged.stale_login_dirs_removed, 4);
}

#[test]
fn zero_orphan_retention_selects_fresh_orphan_managed_home() {
    let root = std::env::temp_dir().join(format!(
        "prodex-housekeeping-zero-retention-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root,
    };
    fs::create_dir_all(&paths.managed_profiles_root).expect("profiles root should exist");
    let orphan = paths.managed_profiles_root.join("fresh-orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan marker should write");

    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let orphans = collect_orphan_managed_profile_dirs_at(&paths, &state, SystemTime::now(), 0);

    assert_eq!(orphans, vec!["fresh-orphan"]);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}
