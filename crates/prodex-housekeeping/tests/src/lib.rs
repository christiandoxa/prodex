use super::*;
use prodex_state::{ProfileEntry, ProfileProvider};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn test_paths(name: &str) -> AppPaths {
    let root =
        std::env::temp_dir().join(format!("prodex-housekeeping-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root,
    }
}

fn profile(codex_home: PathBuf, managed: bool) -> ProfileEntry {
    ProfileEntry {
        codex_home,
        managed,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

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

#[cfg(unix)]
#[test]
fn runtime_log_discovery_skips_symlink_log_files() {
    let paths = test_paths("runtime-log-symlink");
    fs::create_dir_all(&paths.root).expect("root should exist");
    let regular = paths.root.join("prodex-runtime-1.log");
    let symlink = paths.root.join("prodex-runtime-2.log");
    let target = paths.root.join("secret.txt");
    fs::write(&regular, "runtime\n").expect("regular log should write");
    fs::write(&target, "do not read\n").expect("target should write");
    std::os::unix::fs::symlink(&target, &symlink).expect("symlink should create");

    let logs = prodex_runtime_log_paths_in_dir(&paths.root, "prodex-runtime");

    assert_eq!(logs, vec![regular]);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn zero_orphan_retention_selects_fresh_orphan_managed_home() {
    let paths = test_paths("zero-retention");
    fs::create_dir_all(&paths.managed_profiles_root).expect("profiles root should exist");
    let orphan = paths.managed_profiles_root.join("fresh-orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan marker should write");

    let state = AppState::default();

    let orphans = collect_orphan_managed_profile_dirs_at(&paths, &state, SystemTime::now(), 0);

    assert_eq!(orphans, vec!["fresh-orphan"]);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_for_healthy_state_is_empty() {
    let paths = test_paths("repair-healthy");
    let profile_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&profile_home).expect("profile home should exist");
    fs::write(&paths.state_file, "{}").expect("state should write");

    let mut profiles = BTreeMap::new();
    profiles.insert("main".to_string(), profile(profile_home, true));
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles,
        ..Default::default()
    };

    let actions = plan_prodex_state_repairs_at(
        &paths,
        Some(&state),
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert!(actions.is_empty());
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_uses_recoverable_last_good_for_missing_state() {
    let paths = test_paths("repair-last-good");
    fs::create_dir_all(&paths.root).expect("root should exist");
    let last_good = last_good_file_path(&paths.state_file);
    fs::write(&last_good, r#"{"profiles":{}}"#).expect("last-good should write");

    let actions = plan_prodex_state_repairs_at(
        &paths,
        None,
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0].kind,
        ProdexRepairActionKind::RestoreLastGoodState
    );
    assert_eq!(actions[0].severity, ProdexRepairSeverity::Warning);
    assert_eq!(actions[0].path, paths.state_file);
    assert_eq!(
        actions[0].secondary_path.as_deref(),
        Some(last_good.as_path())
    );
    assert!(actions[0].dry_run_text.contains("would restore"));
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_reports_missing_state_without_last_good() {
    let paths = test_paths("repair-missing-state");
    fs::create_dir_all(&paths.root).expect("root should exist");

    let actions = plan_prodex_state_repairs_at(
        &paths,
        None,
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, ProdexRepairActionKind::MissingStateFile);
    assert_eq!(actions[0].severity, ProdexRepairSeverity::Critical);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_reports_unreadable_state_without_last_good() {
    let paths = test_paths("repair-unreadable-state");
    fs::create_dir_all(&paths.state_file).expect("state directory fixture should exist");

    let actions = plan_prodex_state_repairs_at(
        &paths,
        None,
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, ProdexRepairActionKind::UnreadableStateFile);
    assert_eq!(actions[0].severity, ProdexRepairSeverity::Critical);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_includes_stale_temp_cleanup_without_deleting_file() {
    let paths = test_paths("repair-stale-temp");
    fs::create_dir_all(&paths.root).expect("root should exist");
    fs::write(&paths.state_file, "{}").expect("state should write");
    let stale_temp = paths.root.join("state.json.999999999.1.0.tmp");
    fs::write(&stale_temp, "tmp").expect("temp should write");

    let actions = plan_prodex_state_repairs_at(
        &paths,
        None,
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0].kind,
        ProdexRepairActionKind::RemoveStaleRootTempFile
    );
    assert_eq!(actions[0].severity, ProdexRepairSeverity::Info);
    assert_eq!(actions[0].path, stale_temp);
    assert!(actions[0].path.exists());
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_detects_missing_profile_home() {
    let paths = test_paths("repair-missing-profile-home");
    fs::create_dir_all(&paths.root).expect("root should exist");
    fs::write(&paths.state_file, "{}").expect("state should write");
    let missing_home = paths.managed_profiles_root.join("main");
    let mut profiles = BTreeMap::new();
    profiles.insert("main".to_string(), profile(missing_home.clone(), true));
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles,
        ..Default::default()
    };

    let actions = plan_prodex_state_repairs_at(
        &paths,
        Some(&state),
        SystemTime::now(),
        ProdexRepairPlanOptions::default(),
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0].kind,
        ProdexRepairActionKind::CreateMissingProfileHome
    );
    assert_eq!(actions[0].profile_name.as_deref(), Some("main"));
    assert_eq!(actions[0].path, missing_home);
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_detects_orphaned_managed_home() {
    let paths = test_paths("repair-orphan-home");
    fs::create_dir_all(&paths.managed_profiles_root).expect("profiles root should exist");
    fs::write(&paths.state_file, "{}").expect("state should write");
    let orphan = paths.managed_profiles_root.join("old-orphan");
    fs::create_dir_all(&orphan).expect("orphan should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan marker should write");
    let state = AppState::default();

    let actions = plan_prodex_state_repairs_at(
        &paths,
        Some(&state),
        SystemTime::now(),
        ProdexRepairPlanOptions {
            orphan_managed_profile_retention_seconds: 0,
            ..ProdexRepairPlanOptions::default()
        },
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0].kind,
        ProdexRepairActionKind::RemoveOrphanManagedProfileHome
    );
    assert_eq!(actions[0].profile_name.as_deref(), Some("old-orphan"));
    assert_eq!(actions[0].path, orphan);
    assert!(actions[0].path.exists());
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}

#[test]
fn repair_plan_redacts_paths_in_dry_run_text() {
    let paths = test_paths("repair-redacted");
    fs::create_dir_all(&paths.root).expect("root should exist");
    let last_good = last_good_file_path(&paths.state_file);
    fs::write(&last_good, r#"{"profiles":{}}"#).expect("last-good should write");

    let actions = plan_prodex_state_repairs_at(
        &paths,
        None,
        SystemTime::now(),
        ProdexRepairPlanOptions {
            redact_paths: true,
            ..ProdexRepairPlanOptions::default()
        },
        |_| false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].path, paths.state_file);
    assert!(actions[0].dry_run_text.contains("<prodex-home>/state.json"));
    assert!(
        !actions[0]
            .dry_run_text
            .contains(paths.root.to_str().unwrap())
    );
    fs::remove_dir_all(&paths.root).expect("test root should clean up");
}
