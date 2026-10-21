use std::fs;
use std::path::PathBuf;

use crate::{AppPaths, AppState, last_good_file_path};

pub(crate) const RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX: &str = "live-";

pub(crate) fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    prodex_state::merge_app_state_for_save(existing, desired)
}

pub(crate) fn runtime_continuations_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuations.json")
}

pub(crate) fn runtime_continuations_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuations_file_path(paths))
}

pub(crate) fn runtime_continuation_journal_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuation-journal.json")
}

pub(crate) fn runtime_continuation_journal_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuation_journal_file_path(paths))
}

pub(crate) fn runtime_broker_registry_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths.root.join(format!("runtime-broker-{broker_key}.json"))
}

pub(crate) fn runtime_broker_registry_last_good_file_path(
    paths: &AppPaths,
    broker_key: &str,
) -> PathBuf {
    last_good_file_path(&runtime_broker_registry_file_path(paths, broker_key))
}

pub(crate) fn runtime_broker_capability_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}.capability"))
}

pub(crate) fn runtime_broker_lease_dir(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-leases"))
}

pub(crate) fn runtime_broker_ensure_lock_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-ensure"))
}

pub(crate) fn runtime_broker_registry_keys(paths: &AppPaths) -> Vec<String> {
    runtime_registry_keys(paths, |key| {
        !key.starts_with(RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX)
    })
}

pub(crate) fn runtime_live_log_source_registry_keys(paths: &AppPaths) -> Vec<String> {
    runtime_registry_keys(paths, |key| {
        key.starts_with(RUNTIME_LIVE_LOG_SOURCE_KEY_PREFIX)
    })
}

fn runtime_registry_keys(paths: &AppPaths, include: impl Fn(&str) -> bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return Vec::new();
    };

    let keys = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_prefix("runtime-broker-")
                .and_then(|suffix| suffix.strip_suffix(".json"))
                .filter(|key| include(key))
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let views = keys.iter().map(String::as_bytes).collect::<Vec<_>>();
    let selected = prodex_mojo_core::core_file_policy::lexical_file_key_order(&views, true)
        .expect("compiled Mojo runtime broker key sort/dedup returned invalid output");
    let mut originals = keys.into_iter().map(Some).collect::<Vec<_>>();
    selected
        .into_iter()
        .map(|index| {
            originals[index]
                .take()
                .expect("validated Mojo broker key indices are unique")
        })
        .collect()
}

#[cfg(test)]
mod compiled_registry_key_order_tests {
    use super::*;

    fn paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "prodex-compiled-registry-sort-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        }
    }

    #[test]
    fn compiled_broker_registry_sort_dedups_and_separates_direct_sources() {
        let paths = paths("unicode");
        for name in [
            "runtime-broker-雪.json",
            "runtime-broker-z.json",
            "runtime-broker-a.json",
            "runtime-broker-é.json",
            "runtime-broker-live-z.json",
            "runtime-broker-live-a.json",
        ] {
            fs::write(paths.root.join(name), b"{}").unwrap();
        }
        // The host's original scanner only filters names; no storage
        // decoding or JSON validation is part of this operation.
        assert_eq!(runtime_broker_registry_keys(&paths), ["a", "z", "é", "雪"]);
        assert_eq!(
            runtime_live_log_source_registry_keys(&paths),
            ["live-a", "live-z"]
        );
        fs::remove_dir_all(&paths.root).unwrap();
    }

    #[test]
    fn compiled_registry_sort_keeps_empty_unicode_and_lexical_suffixes() {
        let paths = paths("mixed");
        for name in [
            "runtime-broker-.json",
            "runtime-broker-0.json",
            "runtime-broker-10.json",
            "runtime-broker-2.json",
            "not-runtime-broker-a.json",
            "runtime-broker-a.json.bak",
        ] {
            fs::write(paths.root.join(name), b"{}").unwrap();
        }
        assert_eq!(runtime_broker_registry_keys(&paths), ["", "0", "10", "2"]);
        assert!(runtime_live_log_source_registry_keys(&paths).is_empty());
        fs::remove_dir_all(&paths.root).unwrap();
    }
}
