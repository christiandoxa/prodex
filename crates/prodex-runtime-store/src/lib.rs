//! Runtime store merge and compaction helpers.
//!
//! This crate owns side-effect-free state pruning and merge rules only. File
//! reads, atomic writes, and generation fences stay in the binary crate.

use prodex_runtime_state::{
    RuntimeProfileBackoffs, RuntimeProfileHealth, RuntimeProfileUsageSnapshot,
};
use prodex_state::ProfileEntry;
use std::collections::BTreeMap;

pub const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };

pub fn compact_runtime_usage_snapshots<W>(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let oldest_allowed = now.saturating_sub(RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

pub fn merge_runtime_usage_snapshots<W: Clone>(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, now)
}

pub fn compact_runtime_profile_scores(
    mut scores: BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let oldest_allowed = now.saturating_sub(RUNTIME_SCORE_RETENTION_SECONDS);
    scores.retain(|key, value| {
        profiles.contains_key(runtime_profile_score_profile_name(key))
            && value.updated_at >= oldest_allowed
    });
    scores
}

pub fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let mut merged = existing.clone();
    for (key, value) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| current.updated_at <= value.updated_at);
        if should_replace {
            merged.insert(key.clone(), value.clone());
        }
    }
    compact_runtime_profile_scores(merged, profiles, now)
}

pub fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

pub fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .insert(route_profile_key.clone(), *until);
    }
    compact_runtime_profile_backoffs(merged, profiles, now)
}

fn runtime_profile_route_kind_label_is_valid(label: &str) -> bool {
    matches!(label, "responses" | "compact" | "websocket" | "standard")
}

fn runtime_profile_route_key_parts<'a>(key: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

fn runtime_profile_transport_backoff_key_matches_profiles(
    key: &str,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_profile_route_kind_label_is_valid(route) && profiles.contains_key(profile_name)
        })
        .unwrap_or_else(|| profiles.contains_key(key))
}

fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_runtime_state::{RuntimeProfileUsageSnapshot, RuntimeQuotaWindowStatus};
    use prodex_state::ProfileProvider;
    use std::path::PathBuf;

    fn profile() -> ProfileEntry {
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/profile"),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        }
    }

    #[test]
    fn usage_snapshot_compaction_prunes_missing_and_expired_profiles() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let snapshots = BTreeMap::from([
            (
                "alpha".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: 200,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 100,
                    five_hour_reset_at: 0,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 100,
                    weekly_reset_at: 0,
                },
            ),
            (
                "missing".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: 200,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 100,
                    five_hour_reset_at: 0,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 100,
                    weekly_reset_at: 0,
                },
            ),
        ]);

        let compacted = compact_runtime_usage_snapshots(snapshots, &profiles, 250);

        assert_eq!(compacted.len(), 1);
        assert!(compacted.contains_key("alpha"));
    }

    #[test]
    fn profile_score_compaction_supports_route_scoped_keys() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let scores = BTreeMap::from([
            (
                "__route_health__:responses:alpha".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 200,
                },
            ),
            (
                "__route_health__:responses:missing".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 200,
                },
            ),
        ]);

        let compacted = compact_runtime_profile_scores(scores, &profiles, 250);

        assert_eq!(
            compacted.keys().cloned().collect::<Vec<_>>(),
            vec!["__route_health__:responses:alpha".to_string()]
        );
    }

    #[test]
    fn backoff_compaction_keeps_valid_route_keys_and_future_backoffs() {
        let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
        let backoffs = RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("alpha".to_string(), 300),
                ("expired".to_string(), 100),
            ]),
            transport_backoff_until: BTreeMap::from([
                (
                    "__route_transport_backoff__:responses:alpha".to_string(),
                    300,
                ),
                ("__route_transport_backoff__:unknown:alpha".to_string(), 300),
                ("missing".to_string(), 300),
            ]),
            route_circuit_open_until: BTreeMap::from([
                ("__route_circuit__:responses:alpha".to_string(), 300),
                ("__route_circuit__:responses:missing".to_string(), 300),
            ]),
        };

        let compacted = compact_runtime_profile_backoffs(backoffs, &profiles, 200);

        assert_eq!(
            compacted
                .retry_backoff_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["alpha".to_string()]
        );
        assert_eq!(
            compacted
                .transport_backoff_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["__route_transport_backoff__:responses:alpha".to_string()]
        );
        assert_eq!(
            compacted
                .route_circuit_open_until
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["__route_circuit__:responses:alpha".to_string()]
        );
    }
}
