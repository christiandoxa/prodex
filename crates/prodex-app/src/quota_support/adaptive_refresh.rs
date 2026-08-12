use super::DEFAULT_WATCH_INTERVAL_SECONDS;
use super::{ProviderQuotaSnapshot, QuotaReport};
use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileEntry, RuntimeProfileUsageSnapshot,
    last_good_file_path, load_runtime_usage_snapshots, merge_runtime_usage_snapshots,
    read_versioned_json_file_with_backup, runtime_sidecar_generation_error_is_stale,
    save_runtime_usage_snapshots_for_profiles, save_versioned_json_file_with_fence,
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) const ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS: u64 = DEFAULT_WATCH_INTERVAL_SECONDS;
pub(crate) const ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS: u64 = 10;
pub(crate) const ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS: u64 = 45;
pub(crate) const ALL_QUOTA_WATCH_STABLE_INTERVAL_SECONDS: u64 = 90;
pub(crate) const ALL_QUOTA_WATCH_PROFILE_SCALE_SECONDS: u64 = 2;
pub(crate) const ALL_QUOTA_WATCH_IMMINENT_RESET_SECONDS: i64 = 2 * 60;
pub(crate) const ALL_QUOTA_WATCH_NEAR_RESET_SECONDS: i64 = 15 * 60;
const QUOTA_WATCH_RUNTIME_USAGE_CACHE_FILE: &str = "quota-watch-runtime-usage-cache.json";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum QuotaRefreshSignal {
    Stable,
    Watch,
    Imminent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuotaWatchRuntimeUsageCache {
    updated_at: i64,
    next_refresh_at: i64,
    alive_until: i64,
    snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
}

pub(crate) struct LiveQuotaWatchRuntimeUsageCache {
    pub(crate) snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    next_refresh_at: i64,
}

impl LiveQuotaWatchRuntimeUsageCache {
    pub(crate) fn refresh_interval_at(&self, now: i64) -> Duration {
        Duration::from_secs(
            u64::try_from(self.next_refresh_at.saturating_sub(now).max(1)).unwrap_or(1),
        )
    }
}

pub(crate) fn save_quota_watch_runtime_usage_cache(
    paths: &AppPaths,
    reports: &[QuotaReport],
    refresh_interval: Duration,
) {
    let snapshots = reports
        .iter()
        .filter_map(quota_report_runtime_usage_snapshot)
        .collect::<BTreeMap<_, _>>();
    if let Ok(state) = AppState::load(paths) {
        persist_openai_runtime_usage_snapshots(paths, &state.profiles, &snapshots);
    }
    let now = Local::now().timestamp();
    let refresh_seconds = i64::try_from(refresh_interval.as_secs())
        .unwrap_or(i64::MAX / 2)
        .max(1);
    let cache = QuotaWatchRuntimeUsageCache {
        updated_at: now,
        next_refresh_at: now.saturating_add(refresh_seconds),
        alive_until: now
            .saturating_add(refresh_seconds)
            .saturating_add(ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS as i64),
        snapshots,
    };
    let path = quota_watch_runtime_usage_cache_path(paths);
    let _ = save_versioned_json_file_with_fence(&path, &last_good_file_path(&path), &cache);
}

pub(crate) fn save_openai_quota_runtime_usage_snapshots(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    reports: &[QuotaReport],
) {
    let snapshots = reports
        .iter()
        .filter_map(quota_report_runtime_usage_snapshot)
        .collect::<BTreeMap<_, _>>();
    persist_openai_runtime_usage_snapshots(paths, profiles, &snapshots);
}

pub(crate) fn save_openai_quota_runtime_usage_snapshot(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    profile_name: &str,
    quota: &ProviderQuotaSnapshot,
) {
    let Some(snapshot) = provider_quota_runtime_usage_snapshot(quota, Local::now().timestamp())
    else {
        return;
    };
    persist_openai_runtime_usage_snapshots(
        paths,
        profiles,
        &BTreeMap::from([(profile_name.to_string(), snapshot)]),
    );
}

fn persist_openai_runtime_usage_snapshots(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) {
    if incoming.is_empty() {
        return;
    }
    for _ in 0..2 {
        let Ok(existing) = load_runtime_usage_snapshots(paths, profiles) else {
            return;
        };
        let merged = merge_runtime_usage_snapshots(&existing, incoming, profiles);
        match save_runtime_usage_snapshots_for_profiles(paths, &merged, profiles) {
            Ok(()) => return,
            Err(error) if runtime_sidecar_generation_error_is_stale(&error) => continue,
            Err(_) => return,
        }
    }
}

pub(crate) fn load_live_quota_watch_runtime_usage_cache(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> Option<LiveQuotaWatchRuntimeUsageCache> {
    let path = quota_watch_runtime_usage_cache_path(paths);
    if !path.exists() && !last_good_file_path(&path).exists() {
        return None;
    }
    let cache = read_versioned_json_file_with_backup::<QuotaWatchRuntimeUsageCache>(
        &path,
        &last_good_file_path(&path),
    )
    .ok()?
    .value;
    if cache.alive_until < now {
        return None;
    }
    let snapshots = cache
        .snapshots
        .into_iter()
        .filter(|(name, _)| profiles.contains_key(name))
        .collect::<BTreeMap<_, _>>();
    Some(LiveQuotaWatchRuntimeUsageCache {
        snapshots,
        next_refresh_at: cache.next_refresh_at,
    })
}

fn quota_watch_runtime_usage_cache_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(QUOTA_WATCH_RUNTIME_USAGE_CACHE_FILE)
}

fn quota_report_runtime_usage_snapshot(
    report: &QuotaReport,
) -> Option<(String, RuntimeProfileUsageSnapshot)> {
    Some((
        report.name.clone(),
        provider_quota_runtime_usage_snapshot(report.result.as_ref().ok()?, report.fetched_at)?,
    ))
}

fn provider_quota_runtime_usage_snapshot(
    quota: &ProviderQuotaSnapshot,
    checked_at: i64,
) -> Option<RuntimeProfileUsageSnapshot> {
    let ProviderQuotaSnapshot::OpenAi(usage) = quota else {
        return None;
    };
    let mut snapshot = prodex_runtime_quota::runtime_profile_usage_snapshot_from_usage(usage);
    snapshot.checked_at = checked_at;
    Some(snapshot)
}

pub(crate) fn quota_watch_detail_refresh_interval_for_cached_openai(
    reset_windows: &[i64],
    watch: bool,
    profile_count: usize,
    now: i64,
) -> Duration {
    let signal = if watch {
        QuotaRefreshSignal::Watch
    } else {
        reset_windows
            .iter()
            .copied()
            .map(|reset_at| quota_reset_refresh_signal(reset_at, now))
            .max()
            .unwrap_or(QuotaRefreshSignal::Stable)
    };

    quota_watch_refresh_interval_for_signal(signal, true, profile_count, now)
}

pub(crate) fn quota_watch_refresh_interval_for_signal(
    signal: QuotaRefreshSignal,
    detail: bool,
    profile_count: usize,
    now: i64,
) -> Duration {
    let base = match signal {
        QuotaRefreshSignal::Imminent => ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS,
        QuotaRefreshSignal::Watch => ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS,
        QuotaRefreshSignal::Stable => {
            if detail {
                ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS
            } else {
                ALL_QUOTA_WATCH_STABLE_INTERVAL_SECONDS
            }
        }
    };
    let scaled = base.max(
        u64::try_from(profile_count)
            .unwrap_or(u64::MAX)
            .saturating_mul(ALL_QUOTA_WATCH_PROFILE_SCALE_SECONDS),
    );
    Duration::from_secs(quota_watch_jittered_interval_seconds(scaled, now))
}

pub(crate) fn quota_reset_refresh_signal(reset_at: i64, now: i64) -> QuotaRefreshSignal {
    if reset_at <= now + ALL_QUOTA_WATCH_IMMINENT_RESET_SECONDS {
        QuotaRefreshSignal::Imminent
    } else if reset_at <= now + ALL_QUOTA_WATCH_NEAR_RESET_SECONDS {
        QuotaRefreshSignal::Watch
    } else {
        QuotaRefreshSignal::Stable
    }
}

fn quota_watch_jittered_interval_seconds(seconds: u64, now: i64) -> u64 {
    if seconds <= ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS {
        return seconds;
    }
    let jitter_span = (seconds / 10).max(1);
    let bucket = now.unsigned_abs() % (jitter_span.saturating_mul(2).saturating_add(1));
    let delta = i64::try_from(bucket).unwrap_or(0) - i64::try_from(jitter_span).unwrap_or(0);
    seconds.saturating_add_signed(delta).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileProvider, RuntimeQuotaWindowStatus};
    use prodex_quota::UsageResponse;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_paths() -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-quota-watch-cache-test-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("codex"),
            legacy_shared_codex_root: root.join("shared"),
            root,
        }
    }

    fn profile(paths: &AppPaths) -> ProfileEntry {
        ProfileEntry {
            codex_home: paths.root.join("codex-home"),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        }
    }

    fn snapshot() -> RuntimeProfileUsageSnapshot {
        RuntimeProfileUsageSnapshot {
            checked_at: 0,
            plan_type: None,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 80,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: i64::MAX,
        }
    }

    #[test]
    fn one_shot_openai_quota_updates_runtime_usage_snapshot() {
        let paths = test_paths();
        let profiles = BTreeMap::from([("main".to_string(), profile(&paths))]);
        let checked_at = Local::now().timestamp();
        let quota = ProviderQuotaSnapshot::OpenAi(UsageResponse {
            email: Some("test@example.com".to_string()),
            plan_type: Some("plus".to_string()),
            rate_limit: None,
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        });

        save_openai_quota_runtime_usage_snapshot(&paths, &profiles, "main", &quota);

        let snapshots = load_runtime_usage_snapshots(&paths, &profiles).unwrap();
        let saved = snapshots.get("main").expect("snapshot should be saved");
        assert!(saved.checked_at >= checked_at);
        assert_eq!(saved.plan_type.as_deref(), Some("plus"));
        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn live_quota_watch_cache_uses_writer_schedule() {
        let paths = test_paths();
        let mut snapshots = BTreeMap::new();
        snapshots.insert("main".to_string(), snapshot());
        snapshots.insert("other".to_string(), snapshot());
        let cache = QuotaWatchRuntimeUsageCache {
            updated_at: 0,
            next_refresh_at: 10,
            alive_until: 20,
            snapshots,
        };
        let path = quota_watch_runtime_usage_cache_path(&paths);
        save_versioned_json_file_with_fence(&path, &last_good_file_path(&path), &cache).unwrap();

        let profiles = BTreeMap::from([("main".to_string(), profile(&paths))]);
        let live = load_live_quota_watch_runtime_usage_cache(&paths, &profiles, 5).unwrap();

        assert_eq!(live.refresh_interval_at(5), Duration::from_secs(5));
        assert!(live.snapshots.contains_key("main"));
        assert!(!live.snapshots.contains_key("other"));
        assert!(load_live_quota_watch_runtime_usage_cache(&paths, &profiles, 21).is_none());
    }
}
