//! Runtime broker registry, health, and metrics transfer models.
//!
//! This crate intentionally stays side-effect-free. The binary crate still owns
//! broker leases, proxy handles, and registry file I/O.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeProfileHealth, RuntimeStateLockWaitMetrics,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerRegistry {
    pub pid: u32,
    pub listen_addr: String,
    pub started_at: i64,
    pub upstream_base_url: String,
    pub include_code_review: bool,
    #[serde(default)]
    pub upstream_no_proxy: bool,
    pub current_profile: String,
    pub instance_token: String,
    pub admin_token: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
    #[serde(default)]
    pub openai_mount_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerHealth {
    pub pid: u32,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub active_requests: usize,
    pub instance_token: String,
    pub persistence_role: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: usize,
    pub limit: usize,
    pub admissions_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationSignalMetrics {
    pub response: usize,
    pub turn_state: usize,
    pub session_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuityFailureReasonMetrics {
    #[serde(default)]
    pub chain_retried_owner: BTreeMap<String, usize>,
    #[serde(default)]
    pub chain_dead_upstream_confirmed: BTreeMap<String, usize>,
    #[serde(default)]
    pub stale_continuation: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerRouteContinuityMetrics {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerPreviousResponseContinuityMetrics {
    pub negative_cache_entries: RuntimeBrokerRouteContinuityMetrics,
    pub negative_cache_failures: RuntimeBrokerRouteContinuityMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: usize,
    pub turn_state_bindings: usize,
    pub session_id_bindings: usize,
    pub warm: usize,
    pub verified: usize,
    pub suspect: usize,
    pub dead: usize,
    #[serde(default)]
    pub failure_counts: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub not_found_streaks: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerMetrics {
    pub health: RuntimeBrokerHealth,
    pub active_request_limit: usize,
    pub local_overload_backoff_remaining_seconds: u64,
    #[serde(default)]
    pub runtime_state_lock_wait: RuntimeStateLockWaitMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, usize>,
    pub retry_backoffs: usize,
    pub transport_backoffs: usize,
    pub route_circuits: usize,
    pub degraded_profiles: usize,
    pub degraded_routes: usize,
    pub continuations: RuntimeBrokerContinuationMetrics,
    #[serde(default)]
    pub previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics,
    #[serde(default)]
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeBrokerContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

pub fn runtime_broker_continuation_metrics(
    statuses: &RuntimeContinuationStatuses,
    now: i64,
    stale_verified_seconds: i64,
) -> RuntimeBrokerContinuationMetrics {
    let mut metrics = RuntimeBrokerContinuationMetrics {
        response_bindings: statuses.response.len(),
        turn_state_bindings: statuses.turn_state.len(),
        session_id_bindings: statuses.session_id.len(),
        warm: 0,
        verified: 0,
        suspect: 0,
        dead: 0,
        failure_counts: RuntimeBrokerContinuationSignalMetrics::default(),
        not_found_streaks: RuntimeBrokerContinuationSignalMetrics::default(),
        stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics::default(),
    };
    for (kind, status) in statuses
        .response
        .values()
        .map(|status| (RuntimeBrokerContinuationBindingKind::Response, status))
        .chain(
            statuses
                .turn_state
                .values()
                .map(|status| (RuntimeBrokerContinuationBindingKind::TurnState, status)),
        )
        .chain(
            statuses
                .session_id
                .values()
                .map(|status| (RuntimeBrokerContinuationBindingKind::SessionId, status)),
        )
    {
        match status.state {
            RuntimeContinuationBindingLifecycle::Warm => metrics.warm += 1,
            RuntimeContinuationBindingLifecycle::Verified => metrics.verified += 1,
            RuntimeContinuationBindingLifecycle::Suspect => metrics.suspect += 1,
            RuntimeContinuationBindingLifecycle::Dead => metrics.dead += 1,
        }
        runtime_broker_add_continuation_signal(
            &mut metrics.failure_counts,
            kind,
            status.failure_count as usize,
        );
        runtime_broker_add_continuation_signal(
            &mut metrics.not_found_streaks,
            kind,
            status.not_found_streak as usize,
        );
        if runtime_broker_continuation_status_is_stale_verified(status, now, stale_verified_seconds)
        {
            runtime_broker_add_continuation_signal(&mut metrics.stale_verified_bindings, kind, 1);
        }
    }
    metrics
}

pub fn runtime_broker_previous_response_continuity_metrics(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    negative_cache_decay_seconds: i64,
) -> RuntimeBrokerPreviousResponseContinuityMetrics {
    const PREFIX: &str = "__previous_response_not_found__:";

    let mut metrics = RuntimeBrokerPreviousResponseContinuityMetrics::default();
    for (key, entry) in profile_health {
        let Some(rest) = key.strip_prefix(PREFIX) else {
            continue;
        };
        let Some((route, _)) = rest.split_once(':') else {
            continue;
        };
        let score = runtime_broker_effective_score(entry, now, negative_cache_decay_seconds);
        if score == 0 {
            continue;
        }
        if runtime_broker_add_route_continuity_signal(&mut metrics.negative_cache_entries, route, 1)
        {
            let _ = runtime_broker_add_route_continuity_signal(
                &mut metrics.negative_cache_failures,
                route,
                score as usize,
            );
        }
    }
    metrics
}

pub fn runtime_broker_add_route_continuity_signal(
    metrics: &mut RuntimeBrokerRouteContinuityMetrics,
    route: &str,
    value: usize,
) -> bool {
    match route {
        "responses" => metrics.responses += value,
        "compact" => metrics.compact += value,
        "websocket" => metrics.websocket += value,
        "standard" => metrics.standard += value,
        _ => return false,
    }
    true
}

pub fn runtime_broker_merge_continuity_failure_reason_metrics(
    metrics: &mut RuntimeBrokerContinuityFailureReasonMetrics,
    delta: RuntimeBrokerContinuityFailureReasonMetrics,
) {
    for (reason, count) in delta.chain_retried_owner {
        *metrics.chain_retried_owner.entry(reason).or_insert(0) += count;
    }
    for (reason, count) in delta.chain_dead_upstream_confirmed {
        *metrics
            .chain_dead_upstream_confirmed
            .entry(reason)
            .or_insert(0) += count;
    }
    for (reason, count) in delta.stale_continuation {
        *metrics.stale_continuation.entry(reason).or_insert(0) += count;
    }
}

pub fn runtime_broker_subtract_continuity_failure_reason_metrics(
    metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    delta: &RuntimeBrokerContinuityFailureReasonMetrics,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: runtime_broker_subtract_reason_metrics(
            metrics.chain_retried_owner,
            &delta.chain_retried_owner,
        ),
        chain_dead_upstream_confirmed: runtime_broker_subtract_reason_metrics(
            metrics.chain_dead_upstream_confirmed,
            &delta.chain_dead_upstream_confirmed,
        ),
        stale_continuation: runtime_broker_subtract_reason_metrics(
            metrics.stale_continuation,
            &delta.stale_continuation,
        ),
    }
}

fn runtime_broker_add_continuation_signal(
    metrics: &mut RuntimeBrokerContinuationSignalMetrics,
    kind: RuntimeBrokerContinuationBindingKind,
    value: usize,
) {
    match kind {
        RuntimeBrokerContinuationBindingKind::Response => metrics.response += value,
        RuntimeBrokerContinuationBindingKind::TurnState => metrics.turn_state += value,
        RuntimeBrokerContinuationBindingKind::SessionId => metrics.session_id += value,
    }
}

fn runtime_broker_continuation_status_last_event_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

fn runtime_broker_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    stale_verified_seconds: i64,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_broker_continuation_status_last_event_at(status)
            .is_some_and(|last| now.saturating_sub(last) >= stale_verified_seconds)
}

fn runtime_broker_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

fn runtime_broker_subtract_reason_metrics(
    metrics: BTreeMap<String, usize>,
    delta: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    metrics
        .into_iter()
        .filter_map(|(reason, count)| {
            let remaining = count.saturating_sub(delta.get(&reason).copied().unwrap_or_default());
            (remaining > 0).then_some((reason, remaining))
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBrokerObservation {
    pub broker_key: String,
    pub listen_addr: String,
    pub metrics: RuntimeBrokerMetrics,
}

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetadata {
    pub broker_key: String,
    pub listen_addr: String,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub instance_token: String,
    pub admin_token: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProdexBinaryIdentity {
    pub prodex_version: Option<String>,
    pub executable_path: Option<PathBuf>,
    pub executable_sha256: Option<String>,
}

impl RuntimeProdexBinaryIdentity {
    pub fn is_present(&self) -> bool {
        self.prodex_version.is_some()
            || self.executable_path.is_some()
            || self.executable_sha256.is_some()
    }
}

pub fn runtime_broker_key_for_binary_identity(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    openai_mount_path: &str,
    binary_identity_key: &str,
) -> String {
    let mut hasher = DefaultHasher::new();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    upstream_no_proxy.hash(&mut hasher);
    openai_mount_path.hash(&mut hasher);
    binary_identity_key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn runtime_prodex_binary_identity_key(identity: &RuntimeProdexBinaryIdentity) -> String {
    match (
        identity.prodex_version.as_deref(),
        identity.executable_sha256.as_deref(),
        identity.executable_path.as_ref(),
    ) {
        (Some(version), Some(sha256), _) => format!("version={version};sha256={sha256}"),
        (Some(version), None, Some(path)) => {
            format!("version={version};path={}", path.display())
        }
        (Some(version), None, None) => format!("version={version}"),
        (None, Some(sha256), _) => format!("sha256={sha256}"),
        (None, None, Some(path)) => format!("path={}", path.display()),
        (None, None, None) => "unknown".to_string(),
    }
}

pub fn runtime_registry_prodex_binary_identity(
    registry: &RuntimeBrokerRegistry,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: registry.prodex_version.clone(),
        executable_path: registry.executable_path.clone().map(PathBuf::from),
        executable_sha256: registry.executable_sha256.clone(),
    }
}

pub fn runtime_health_prodex_binary_identity(
    health: &RuntimeBrokerHealth,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: health.prodex_version.clone(),
        executable_path: health.executable_path.clone().map(PathBuf::from),
        executable_sha256: health.executable_sha256.clone(),
    }
}

pub fn runtime_prodex_binary_identity_matches(
    current: &RuntimeProdexBinaryIdentity,
    other: &RuntimeProdexBinaryIdentity,
) -> bool {
    if let (Some(current_sha256), Some(other_sha256)) = (
        current.executable_sha256.as_deref(),
        other.executable_sha256.as_deref(),
    ) {
        return current_sha256 == other_sha256;
    }
    if let (Some(current_version), Some(other_version)) = (
        current.prodex_version.as_deref(),
        other.prodex_version.as_deref(),
    ) {
        return current_version == other_version;
    }
    false
}

pub fn parse_prodex_version_output(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    let binary_name = parts.next()?;
    let version = parts.next()?;
    if binary_name == "prodex" && !version.is_empty() {
        return Some(version.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_metrics_aggregate_lifecycle_signals_and_staleness() {
        let statuses = RuntimeContinuationStatuses {
            response: BTreeMap::from([
                (
                    "resp-warm".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Warm,
                        failure_count: 2,
                        not_found_streak: 1,
                        ..RuntimeContinuationBindingStatus::default()
                    },
                ),
                (
                    "resp-stale".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        last_verified_at: Some(80),
                        failure_count: 3,
                        ..RuntimeContinuationBindingStatus::default()
                    },
                ),
            ]),
            turn_state: BTreeMap::from([(
                "turn-suspect".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Suspect,
                    not_found_streak: 4,
                    ..RuntimeContinuationBindingStatus::default()
                },
            )]),
            session_id: BTreeMap::from([(
                "session-dead".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Dead,
                    failure_count: 1,
                    ..RuntimeContinuationBindingStatus::default()
                },
            )]),
        };

        let metrics = runtime_broker_continuation_metrics(&statuses, 100, 10);

        assert_eq!(metrics.response_bindings, 2);
        assert_eq!(metrics.turn_state_bindings, 1);
        assert_eq!(metrics.session_id_bindings, 1);
        assert_eq!(metrics.warm, 1);
        assert_eq!(metrics.verified, 1);
        assert_eq!(metrics.suspect, 1);
        assert_eq!(metrics.dead, 1);
        assert_eq!(
            metrics.failure_counts,
            RuntimeBrokerContinuationSignalMetrics {
                response: 5,
                turn_state: 0,
                session_id: 1,
            }
        );
        assert_eq!(
            metrics.not_found_streaks,
            RuntimeBrokerContinuationSignalMetrics {
                response: 1,
                turn_state: 4,
                session_id: 0,
            }
        );
        assert_eq!(
            metrics.stale_verified_bindings,
            RuntimeBrokerContinuationSignalMetrics {
                response: 1,
                turn_state: 0,
                session_id: 0,
            }
        );
    }

    #[test]
    fn previous_response_continuity_metrics_count_active_known_routes_only() {
        let profile_health = BTreeMap::from([
            (
                "__previous_response_not_found__:responses:resp-1".to_string(),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: 90,
                },
            ),
            (
                "__previous_response_not_found__:compact:resp-2".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 99,
                },
            ),
            (
                "__previous_response_not_found__:websocket:resp-3".to_string(),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: 0,
                },
            ),
            (
                "__previous_response_not_found__:unknown:resp-4".to_string(),
                RuntimeProfileHealth {
                    score: 9,
                    updated_at: 99,
                },
            ),
            (
                "main".to_string(),
                RuntimeProfileHealth {
                    score: 9,
                    updated_at: 99,
                },
            ),
        ]);

        let metrics = runtime_broker_previous_response_continuity_metrics(&profile_health, 100, 5);

        assert_eq!(
            metrics.negative_cache_entries,
            RuntimeBrokerRouteContinuityMetrics {
                responses: 1,
                compact: 1,
                websocket: 0,
                standard: 0,
            }
        );
        assert_eq!(
            metrics.negative_cache_failures,
            RuntimeBrokerRouteContinuityMetrics {
                responses: 2,
                compact: 2,
                websocket: 0,
                standard: 0,
            }
        );
    }

    #[test]
    fn continuity_failure_reason_metrics_merge_and_subtract_saturating() {
        let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([
                ("previous_response_not_found".to_string(), 2),
                ("stale".to_string(), 1),
            ]),
            chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
            stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
        };

        runtime_broker_merge_continuity_failure_reason_metrics(
            &mut metrics,
            RuntimeBrokerContinuityFailureReasonMetrics {
                chain_retried_owner: BTreeMap::from([
                    ("previous_response_not_found".to_string(), 4),
                    ("new".to_string(), 1),
                ]),
                chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
                stale_continuation: BTreeMap::from([("watchdog".to_string(), 3)]),
            },
        );

        assert_eq!(
            metrics.chain_retried_owner,
            BTreeMap::from([
                ("new".to_string(), 1),
                ("previous_response_not_found".to_string(), 6),
                ("stale".to_string(), 1),
            ])
        );
        assert_eq!(
            runtime_broker_subtract_continuity_failure_reason_metrics(
                metrics,
                &RuntimeBrokerContinuityFailureReasonMetrics {
                    chain_retried_owner: BTreeMap::from([
                        ("new".to_string(), 2),
                        ("previous_response_not_found".to_string(), 1),
                    ]),
                    chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
                    stale_continuation: BTreeMap::from([("watchdog".to_string(), 10)]),
                },
            ),
            RuntimeBrokerContinuityFailureReasonMetrics {
                chain_retried_owner: BTreeMap::from([
                    ("previous_response_not_found".to_string(), 5),
                    ("stale".to_string(), 1),
                ]),
                chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
                stale_continuation: BTreeMap::new(),
            }
        );
    }

    #[test]
    fn prodex_binary_identity_key_prefers_version_and_sha() {
        let identity = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: Some(PathBuf::from("/tmp/prodex")),
            executable_sha256: Some("abc123".to_string()),
        };

        assert_eq!(
            runtime_prodex_binary_identity_key(&identity),
            "version=0.7.0;sha256=abc123"
        );
    }

    #[test]
    fn prodex_binary_identity_match_prefers_sha_then_version() {
        let current = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };
        let other_same_sha = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.8.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };
        let other_same_version = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: None,
        };

        assert!(runtime_prodex_binary_identity_matches(
            &current,
            &other_same_sha
        ));
        assert!(runtime_prodex_binary_identity_matches(
            &current,
            &other_same_version
        ));
    }

    #[test]
    fn parse_prodex_version_output_requires_prodex_binary_name() {
        assert_eq!(
            parse_prodex_version_output("prodex 0.7.0\n"),
            Some("0.7.0".to_string())
        );
        assert_eq!(parse_prodex_version_output("codex 0.7.0\n"), None);
        assert_eq!(parse_prodex_version_output("prodex\n"), None);
    }
}
