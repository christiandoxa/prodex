use super::{
    clear_runtime_policy_cache, load_runtime_policy_cached, reload_runtime_policy_cached,
    runtime_policy_path,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-runtime-policy-{name}-{}-{nanos:x}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn failed_runtime_policy_reload_preserves_last_known_good_entry() {
    clear_runtime_policy_cache();
    let root = temp_root("reload-last-known-good");
    let path = runtime_policy_path(&root);
    fs::write(&path, "version = 1\n\n[runtime_proxy]\nworker_count = 5\n").unwrap();

    let loaded = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(loaded.runtime_proxy.worker_count, Some(5));

    fs::write(&path, "version = [invalid").unwrap();
    assert!(reload_runtime_policy_cached(&root).is_err());

    let last_known_good = load_runtime_policy_cached(&root).unwrap().unwrap();
    assert_eq!(last_known_good.runtime_proxy.worker_count, Some(5));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_runtime_policy_reload_preserves_cached_missing_policy() {
    clear_runtime_policy_cache();
    let root = temp_root("reload-missing-last-known-good");
    let path = runtime_policy_path(&root);

    assert!(load_runtime_policy_cached(&root).unwrap().is_none());
    fs::write(&path, "version = [invalid").unwrap();
    assert!(reload_runtime_policy_cached(&root).is_err());
    assert!(load_runtime_policy_cached(&root).unwrap().is_none());

    let _ = fs::remove_dir_all(root);
}
