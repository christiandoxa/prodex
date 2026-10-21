use prodex_runtime_state::{RuntimeProbeCacheFreshness, runtime_probe_cache_freshness};
use prodex_shared_types::RuntimeProfileProbeCacheEntry;

pub fn runtime_profile_usage_cache_is_fresh(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
    fresh_seconds: i64,
) -> bool {
    matches!(
        runtime_profile_probe_cache_freshness(entry, now, fresh_seconds, fresh_seconds),
        RuntimeProbeCacheFreshness::Fresh
    )
}

pub fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
    fresh_seconds: i64,
    stale_grace_seconds: i64,
) -> RuntimeProbeCacheFreshness {
    runtime_probe_cache_freshness(entry.checked_at, now, fresh_seconds, stale_grace_seconds)
}
