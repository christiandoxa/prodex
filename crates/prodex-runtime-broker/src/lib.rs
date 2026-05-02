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
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub const RUNTIME_BROKER_HEALTH_PATH: &str = "/__prodex/runtime/health";
pub const RUNTIME_BROKER_METRICS_PATH: &str = "/__prodex/runtime/metrics";
pub const RUNTIME_BROKER_METRICS_PROMETHEUS_PATH: &str = "/__prodex/runtime/metrics/prometheus";
pub const RUNTIME_BROKER_ACTIVATE_PATH: &str = "/__prodex/runtime/activate";
pub const RUNTIME_BROKER_ADMIN_TOKEN_HEADER: &str = "X-Prodex-Admin-Token";

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

impl RuntimeBrokerRegistry {
    pub fn admin_url(&self, route: RuntimeBrokerAdminRoute) -> String {
        runtime_broker_admin_url(&self.listen_addr, route)
    }

    pub fn health_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Health)
    }

    pub fn metrics_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Metrics)
    }

    pub fn metrics_prometheus_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::MetricsPrometheus)
    }

    pub fn activate_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Activate)
    }

    pub fn matches_launch_config(
        &self,
        upstream_base_url: &str,
        include_code_review: bool,
        upstream_no_proxy: bool,
    ) -> bool {
        self.upstream_base_url == upstream_base_url
            && self.include_code_review == include_code_review
            && self.upstream_no_proxy == upstream_no_proxy
    }
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

impl RuntimeBrokerHealth {
    pub fn from_metadata(
        metadata: &RuntimeBrokerMetadata,
        pid: u32,
        active_requests: usize,
        persistence_owner: bool,
    ) -> Self {
        Self {
            pid,
            started_at: metadata.started_at,
            current_profile: metadata.current_profile.clone(),
            include_code_review: metadata.include_code_review,
            active_requests,
            instance_token: metadata.instance_token.clone(),
            persistence_role: if persistence_owner {
                "owner"
            } else {
                "follower"
            }
            .to_string(),
            prodex_version: metadata.prodex_version.clone(),
            executable_path: metadata.executable_path.clone(),
            executable_sha256: metadata.executable_sha256.clone(),
        }
    }

    pub fn matches_registry_instance(&self, registry: &RuntimeBrokerRegistry) -> bool {
        self.instance_token == registry.instance_token
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBrokerAdminRoute {
    Health,
    Metrics,
    MetricsPrometheus,
    Activate,
}

impl RuntimeBrokerAdminRoute {
    pub fn path(self) -> &'static str {
        match self {
            Self::Health => RUNTIME_BROKER_HEALTH_PATH,
            Self::Metrics => RUNTIME_BROKER_METRICS_PATH,
            Self::MetricsPrometheus => RUNTIME_BROKER_METRICS_PROMETHEUS_PATH,
            Self::Activate => RUNTIME_BROKER_ACTIVATE_PATH,
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            RUNTIME_BROKER_HEALTH_PATH => Some(Self::Health),
            RUNTIME_BROKER_METRICS_PATH => Some(Self::Metrics),
            RUNTIME_BROKER_METRICS_PROMETHEUS_PATH => Some(Self::MetricsPrometheus),
            RUNTIME_BROKER_ACTIVATE_PATH => Some(Self::Activate),
            _ => None,
        }
    }
}

pub fn runtime_broker_admin_url(listen_addr: &str, route: RuntimeBrokerAdminRoute) -> String {
    format!("http://{}{}", listen_addr, route.path())
}

pub fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.health_url()
}

pub fn runtime_broker_metrics_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.metrics_url()
}

pub fn runtime_broker_metrics_prometheus_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.metrics_prometheus_url()
}

pub fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.activate_url()
}

pub fn runtime_broker_registry_openai_mount_path(
    registry: &RuntimeBrokerRegistry,
) -> Option<String> {
    registry.openai_mount_path.clone()
}

pub fn runtime_broker_legacy_openai_mount_path(prefix: &str, version: &str) -> String {
    format!("{prefix}{version}")
}

pub fn format_runtime_broker_metrics_targets(targets: &[String]) -> String {
    match targets {
        [] => "-".to_string(),
        [target] => target.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerLaunchConfig<'a> {
    pub upstream_base_url: &'a str,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
}

impl RuntimeBrokerLaunchConfig<'_> {
    pub fn matches_registry(self, registry: &RuntimeBrokerRegistry) -> bool {
        registry.matches_launch_config(
            self.upstream_base_url,
            self.include_code_review,
            self.upstream_no_proxy,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerRegistryReuseDecision {
    Reuse,
    LaunchConfigMismatch,
    MissingMatchingHealth,
}

pub fn runtime_broker_registry_reuse_decision(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
    launch_config: RuntimeBrokerLaunchConfig<'_>,
) -> RuntimeBrokerRegistryReuseDecision {
    if !launch_config.matches_registry(registry) {
        return RuntimeBrokerRegistryReuseDecision::LaunchConfigMismatch;
    }
    if health.is_some_and(|health| health.matches_registry_instance(registry)) {
        RuntimeBrokerRegistryReuseDecision::Reuse
    } else {
        RuntimeBrokerRegistryReuseDecision::MissingMatchingHealth
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerAdminError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeBrokerAdminError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeBrokerActivationSuccess {
    pub ok: bool,
    pub current_profile: String,
}

pub fn runtime_broker_admin_not_enabled_error() -> RuntimeBrokerAdminError {
    RuntimeBrokerAdminError::new(
        404,
        "not_found",
        "runtime broker admin endpoint is not enabled for this proxy",
    )
}

pub fn runtime_broker_admin_forbidden_error() -> RuntimeBrokerAdminError {
    RuntimeBrokerAdminError::new(
        403,
        "forbidden",
        "missing or invalid runtime broker admin token",
    )
}

pub fn runtime_broker_validate_admin_token(
    provided_token: Option<&str>,
    expected_token: &str,
) -> Result<(), RuntimeBrokerAdminError> {
    if provided_token == Some(expected_token) {
        Ok(())
    } else {
        Err(runtime_broker_admin_forbidden_error())
    }
}

pub fn runtime_broker_validate_activation_method(
    method: &str,
) -> Result<(), RuntimeBrokerAdminError> {
    if method == "POST" {
        Ok(())
    } else {
        Err(RuntimeBrokerAdminError::new(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ))
    }
}

pub fn runtime_broker_validate_activation_profile(
    current_profile: Option<&str>,
) -> Result<String, RuntimeBrokerAdminError> {
    current_profile
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeBrokerAdminError::new(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            )
        })
}

pub fn runtime_broker_activation_success(
    current_profile: impl Into<String>,
) -> RuntimeBrokerActivationSuccess {
    RuntimeBrokerActivationSuccess {
        ok: true,
        current_profile: current_profile.into(),
    }
}

pub fn runtime_broker_activation_profile_from_json(
    body: &[u8],
) -> Result<String, RuntimeBrokerAdminError> {
    let current_profile = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("current_profile")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    runtime_broker_validate_activation_profile(current_profile.as_deref())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: usize,
    pub limit: usize,
    pub admissions_total: u64,
    #[serde(default)]
    pub releases_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
    #[serde(default)]
    pub release_underflows_total: u64,
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
    #[serde(default)]
    pub active_request_release_underflows_total: u64,
    #[serde(default)]
    pub profile_inflight_admissions_total: u64,
    #[serde(default)]
    pub profile_inflight_releases_total: u64,
    #[serde(default)]
    pub profile_inflight_release_underflows_total: u64,
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

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetricsSnapshotInput<'a> {
    pub metadata: &'a RuntimeBrokerMetadata,
    pub pid: u32,
    pub active_requests: usize,
    pub persistence_owner: bool,
    pub active_request_limit: usize,
    pub local_overload_backoff_remaining_seconds: u64,
    pub runtime_state_lock_wait: RuntimeStateLockWaitMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: &'a BTreeMap<String, usize>,
    pub profile_retry_backoff_until: &'a BTreeMap<String, i64>,
    pub profile_transport_backoff_until: &'a BTreeMap<String, i64>,
    pub profile_route_circuit_open_until: &'a BTreeMap<String, i64>,
    pub profile_health: &'a BTreeMap<String, RuntimeProfileHealth>,
    pub continuation_statuses: &'a RuntimeContinuationStatuses,
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
    pub now: i64,
    pub health_decay_seconds: i64,
    pub stale_verified_seconds: i64,
    pub previous_response_negative_cache_seconds: i64,
}

pub fn runtime_broker_metrics_from_snapshot_input(
    input: RuntimeBrokerMetricsSnapshotInput<'_>,
) -> RuntimeBrokerMetrics {
    let degraded_health = runtime_broker_degraded_health_metrics(
        input.profile_health,
        input.now,
        input.health_decay_seconds,
    );

    RuntimeBrokerMetrics {
        health: RuntimeBrokerHealth::from_metadata(
            input.metadata,
            input.pid,
            input.active_requests,
            input.persistence_owner,
        ),
        active_request_limit: input.active_request_limit,
        local_overload_backoff_remaining_seconds: input.local_overload_backoff_remaining_seconds,
        runtime_state_lock_wait: input.runtime_state_lock_wait,
        traffic: input.traffic,
        profile_inflight: input.profile_inflight.clone(),
        active_request_release_underflows_total: 0,
        profile_inflight_admissions_total: 0,
        profile_inflight_releases_total: 0,
        profile_inflight_release_underflows_total: 0,
        retry_backoffs: input
            .profile_retry_backoff_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        transport_backoffs: input
            .profile_transport_backoff_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        route_circuits: input
            .profile_route_circuit_open_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        degraded_profiles: degraded_health.profiles,
        degraded_routes: degraded_health.routes,
        continuations: runtime_broker_continuation_metrics(
            input.continuation_statuses,
            input.now,
            input.stale_verified_seconds,
        ),
        previous_response_continuity: runtime_broker_previous_response_continuity_metrics(
            input.profile_health,
            input.now,
            input.previous_response_negative_cache_seconds,
        ),
        continuity_failure_reasons: input.continuity_failure_reasons,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerMetricsGuardCounters {
    pub active_request_release_underflows_total: u64,
    pub profile_inflight_admissions_total: u64,
    pub profile_inflight_releases_total: u64,
    pub profile_inflight_release_underflows_total: u64,
}

impl RuntimeBrokerMetrics {
    pub fn with_guard_counters(mut self, counters: RuntimeBrokerMetricsGuardCounters) -> Self {
        self.active_request_release_underflows_total =
            counters.active_request_release_underflows_total;
        self.profile_inflight_admissions_total = counters.profile_inflight_admissions_total;
        self.profile_inflight_releases_total = counters.profile_inflight_releases_total;
        self.profile_inflight_release_underflows_total =
            counters.profile_inflight_release_underflows_total;
        self
    }
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

pub fn runtime_broker_continuity_failure_reason_metrics_from_log_bytes(
    log: &[u8],
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let text = String::from_utf8_lossy(log);
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics::default();
    for line in text.lines() {
        let Some(event) = runtime_broker_continuity_failure_event(line) else {
            continue;
        };
        let Some(reason) = runtime_broker_continuity_failure_reason(line) else {
            continue;
        };
        match event {
            "chain_retried_owner" => {
                *metrics.chain_retried_owner.entry(reason).or_insert(0) += 1;
            }
            "chain_dead_upstream_confirmed" => {
                *metrics
                    .chain_dead_upstream_confirmed
                    .entry(reason)
                    .or_insert(0) += 1;
            }
            "stale_continuation" => {
                *metrics.stale_continuation.entry(reason).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    metrics
}

pub fn runtime_broker_continuity_failure_reason_metrics_with_live(
    parsed_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    baseline_metrics: &RuntimeBrokerContinuityFailureReasonMetrics,
    live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let persisted_since_baseline = runtime_broker_subtract_continuity_failure_reason_metrics(
        parsed_metrics.clone(),
        baseline_metrics,
    );
    let pending_live = runtime_broker_subtract_continuity_failure_reason_metrics(
        live_metrics,
        &persisted_since_baseline,
    );
    let mut merged = parsed_metrics;
    runtime_broker_merge_continuity_failure_reason_metrics(&mut merged, pending_live);
    merged
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

fn runtime_broker_continuity_failure_event(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('{')
        && let Some(event) = runtime_broker_json_string_field(trimmed, "event")
        && let Some(event) = runtime_broker_known_continuity_failure_event(&event)
    {
        return Some(event);
    }
    if trimmed.starts_with('{')
        && let Some(message) = runtime_broker_json_string_field(trimmed, "message")
        && let Some(event) = message
            .split_whitespace()
            .find_map(runtime_broker_known_continuity_failure_event)
    {
        return Some(event);
    }
    line.split_whitespace()
        .find_map(runtime_broker_known_continuity_failure_event)
}

fn runtime_broker_continuity_failure_reason(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('{')
        && let Some(reason) = runtime_broker_json_string_field(trimmed, "reason")
    {
        return Some(reason);
    }
    if trimmed.starts_with('{')
        && let Some(message) = runtime_broker_json_string_field(trimmed, "message")
        && let Some(reason) = runtime_broker_log_field_value(&message, "reason")
    {
        return Some(reason);
    }
    runtime_broker_log_field_value(line, "reason")
}

fn runtime_broker_known_continuity_failure_event(token: &str) -> Option<&'static str> {
    match token {
        "chain_retried_owner" => Some("chain_retried_owner"),
        "chain_dead_upstream_confirmed" => Some("chain_dead_upstream_confirmed"),
        "stale_continuation" => Some("stale_continuation"),
        _ => None,
    }
}

fn runtime_broker_json_string_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < line.len() {
        let found = line[index..].find(&needle)?;
        index += found + needle.len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b':') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'"') {
            continue;
        }
        return runtime_broker_parse_json_string(line, index);
    }
    None
}

fn runtime_broker_parse_json_string(line: &str, quote_index: usize) -> Option<String> {
    let bytes = line.as_bytes();
    if bytes.get(quote_index) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut index = quote_index + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(out),
            b'\\' => {
                index += 1;
                match bytes.get(index).copied()? {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex_start = index + 1;
                        let hex_end = hex_start + 4;
                        let value = line.get(hex_start..hex_end)?;
                        if let Ok(codepoint) = u32::from_str_radix(value, 16)
                            && let Some(ch) = char::from_u32(codepoint)
                        {
                            out.push(ch);
                        }
                        index = hex_end - 1;
                    }
                    other => out.push(other as char),
                }
            }
            _ => {
                let rest = line.get(index..)?;
                let ch = rest.chars().next()?;
                out.push(ch);
                index += ch.len_utf8() - 1;
            }
        }
        index += 1;
    }
    None
}

fn runtime_broker_log_field_value(message: &str, target_key: &str) -> Option<String> {
    let bytes = message.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        index = runtime_broker_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let key_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }
        let key = &message[key_start..index];
        index += 1;
        let value_start = index;
        index = runtime_broker_skip_log_field_value(message, index);
        if key == target_key {
            return Some(runtime_broker_parse_log_field_value(
                &message[value_start..index],
            ));
        }
    }
    None
}

fn runtime_broker_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_broker_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if bytes.get(index) == Some(&b'"') {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => return index + 1,
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_broker_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') && raw_value.ends_with('"') && raw_value.len() >= 2 {
        runtime_broker_parse_json_string(raw_value, 0)
            .unwrap_or_else(|| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeBrokerDegradedHealthMetrics {
    pub profiles: usize,
    pub routes: usize,
}

pub fn runtime_broker_degraded_health_metrics(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    health_decay_seconds: i64,
) -> RuntimeBrokerDegradedHealthMetrics {
    let mut metrics = RuntimeBrokerDegradedHealthMetrics::default();
    for (key, entry) in profile_health {
        if runtime_broker_effective_score(entry, now, health_decay_seconds) == 0 {
            continue;
        }
        if key.starts_with("__route_health__:") {
            metrics.routes += 1;
        } else if !key.starts_with("__") {
            metrics.profiles += 1;
        }
    }
    metrics
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerSpawnConfig<'a> {
    pub current_profile: &'a str,
    pub upstream_base_url: &'a str,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub broker_key: &'a str,
    pub instance_token: &'a str,
    pub admin_token: &'a str,
    pub listen_addr: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerProcessCommandPlan {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub prodex_home: PathBuf,
}

pub fn runtime_broker_process_args(config: RuntimeBrokerSpawnConfig<'_>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("__runtime-broker"),
        OsString::from("--current-profile"),
        OsString::from(config.current_profile),
        OsString::from("--upstream-base-url"),
        OsString::from(config.upstream_base_url),
    ];
    if config.include_code_review {
        args.push(OsString::from("--include-code-review"));
    }
    if config.upstream_no_proxy {
        args.push(OsString::from("--upstream-no-proxy"));
    }
    args.extend([
        OsString::from("--broker-key"),
        OsString::from(config.broker_key),
        OsString::from("--instance-token"),
        OsString::from(config.instance_token),
        OsString::from("--admin-token"),
        OsString::from(config.admin_token),
    ]);
    if let Some(listen_addr) = config.listen_addr {
        args.push(OsString::from("--listen-addr"));
        args.push(OsString::from(listen_addr));
    }
    args
}

pub fn runtime_broker_process_command_plan(
    executable: impl Into<PathBuf>,
    prodex_home: impl Into<PathBuf>,
    config: RuntimeBrokerSpawnConfig<'_>,
) -> RuntimeBrokerProcessCommandPlan {
    RuntimeBrokerProcessCommandPlan {
        executable: executable.into(),
        args: runtime_broker_process_args(config),
        prodex_home: prodex_home.into(),
    }
}

pub fn runtime_broker_startup_grace_seconds(ready_timeout_ms: u64, idle_grace_seconds: i64) -> i64 {
    let ready_timeout_seconds = ready_timeout_ms.div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(idle_grace_seconds)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerVersionGuardOutcome {
    Compatible,
    Replaced,
    DeferredActiveRequests,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerVersionGuardDecision {
    pub outcome: RuntimeBrokerVersionGuardOutcome,
    pub current_identity: RuntimeProdexBinaryIdentity,
    pub replacement_reason: Option<&'static str>,
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

pub fn runtime_broker_observed_known_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> Option<RuntimeProdexBinaryIdentity> {
    health
        .filter(|health| health.matches_registry_instance(registry))
        .map(runtime_health_prodex_binary_identity)
        .filter(RuntimeProdexBinaryIdentity::is_present)
        .or_else(|| {
            let identity = runtime_registry_prodex_binary_identity(registry);
            identity.is_present().then_some(identity)
        })
}

pub fn runtime_broker_observed_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
    process_identity: Option<&RuntimeProdexBinaryIdentity>,
) -> RuntimeProdexBinaryIdentity {
    runtime_broker_observed_known_binary_identity(registry, health)
        .or_else(|| {
            process_identity
                .filter(|identity| identity.is_present())
                .cloned()
        })
        .unwrap_or_default()
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

pub fn runtime_broker_replacement_reason(
    current: &RuntimeProdexBinaryIdentity,
    observed: &RuntimeProdexBinaryIdentity,
) -> &'static str {
    match (
        current.executable_sha256.as_deref(),
        observed.executable_sha256.as_deref(),
    ) {
        (Some(current_sha256), Some(observed_sha256)) if current_sha256 != observed_sha256 => {
            "sha256_mismatch"
        }
        _ => match (
            current.prodex_version.as_deref(),
            observed.prodex_version.as_deref(),
        ) {
            (Some(current_version), Some(observed_version))
                if current_version != observed_version =>
            {
                "version_mismatch"
            }
            _ if observed.is_present() => "identity_mismatch",
            _ => "identity_unresolved",
        },
    }
}

pub fn runtime_broker_observed_version_mismatch(
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
) -> bool {
    observed_identity
        .prodex_version
        .as_deref()
        .zip(current_version_identity.prodex_version.as_deref())
        .is_some_and(|(observed, current)| observed != current)
}

pub fn runtime_broker_version_guard_decision(
    process_alive: bool,
    current_binary_identity: &RuntimeProdexBinaryIdentity,
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
    active_requests: usize,
    live_leases: usize,
) -> RuntimeBrokerVersionGuardDecision {
    let current_identity =
        if runtime_broker_observed_version_mismatch(current_version_identity, observed_identity) {
            current_version_identity.clone()
        } else {
            current_binary_identity.clone()
        };

    if !process_alive
        || (observed_identity.is_present()
            && runtime_prodex_binary_identity_matches(&current_identity, observed_identity))
    {
        return RuntimeBrokerVersionGuardDecision {
            outcome: RuntimeBrokerVersionGuardOutcome::Compatible,
            current_identity,
            replacement_reason: None,
        };
    }

    if active_requests > 0 || live_leases > 0 {
        return RuntimeBrokerVersionGuardDecision {
            outcome: RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests,
            current_identity,
            replacement_reason: None,
        };
    }

    let replacement_reason =
        runtime_broker_replacement_reason(&current_identity, observed_identity);
    RuntimeBrokerVersionGuardDecision {
        outcome: RuntimeBrokerVersionGuardOutcome::Replaced,
        current_identity,
        replacement_reason: Some(replacement_reason),
    }
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

    fn test_registry() -> RuntimeBrokerRegistry {
        RuntimeBrokerRegistry {
            pid: 42,
            listen_addr: "127.0.0.1:4567".to_string(),
            started_at: 100,
            upstream_base_url: "https://upstream.example".to_string(),
            include_code_review: true,
            upstream_no_proxy: false,
            current_profile: "work".to_string(),
            instance_token: "broker-token".to_string(),
            admin_token: "admin-token".to_string(),
            prodex_version: Some("0.7.0".to_string()),
            executable_path: Some("/tmp/prodex".to_string()),
            executable_sha256: Some("abc123".to_string()),
            openai_mount_path: Some("/backend-api/prodex".to_string()),
        }
    }

    #[test]
    fn registry_builds_admin_urls_and_matches_launch_config() {
        let registry = test_registry();

        assert_eq!(
            RuntimeBrokerAdminRoute::from_path("/__prodex/runtime/metrics/prometheus"),
            Some(RuntimeBrokerAdminRoute::MetricsPrometheus)
        );
        assert_eq!(
            registry.health_url(),
            "http://127.0.0.1:4567/__prodex/runtime/health"
        );
        assert_eq!(
            registry.metrics_prometheus_url(),
            "http://127.0.0.1:4567/__prodex/runtime/metrics/prometheus"
        );
        assert!(registry.matches_launch_config("https://upstream.example", true, false));
        assert!(!registry.matches_launch_config("https://other.example", true, false));
    }

    #[test]
    fn registry_helpers_format_mount_paths_targets_and_startup_grace() {
        let registry = test_registry();

        assert_eq!(
            runtime_broker_registry_openai_mount_path(&registry).as_deref(),
            Some("/backend-api/prodex")
        );
        assert_eq!(
            runtime_broker_legacy_openai_mount_path("/backend-api/prodex/v", "0.6.0"),
            "/backend-api/prodex/v0.6.0"
        );
        assert_eq!(format_runtime_broker_metrics_targets(&[]), "-");
        assert_eq!(
            format_runtime_broker_metrics_targets(&[
                "http://127.0.0.1:1/metrics".to_string(),
                "http://127.0.0.1:2/metrics".to_string(),
            ]),
            "http://127.0.0.1:1/metrics (+1 more)"
        );
        assert_eq!(runtime_broker_startup_grace_seconds(1_250, 5), 5);
        assert_eq!(runtime_broker_startup_grace_seconds(5_250, 1), 7);
    }

    #[test]
    fn admin_helpers_plan_errors_and_activation_success() {
        assert_eq!(
            runtime_broker_validate_admin_token(Some("secret"), "secret"),
            Ok(())
        );
        assert_eq!(
            runtime_broker_validate_admin_token(None, "secret"),
            Err(runtime_broker_admin_forbidden_error())
        );
        assert_eq!(
            runtime_broker_validate_activation_method("GET"),
            Err(RuntimeBrokerAdminError::new(
                405,
                "method_not_allowed",
                "runtime broker activation requires POST",
            ))
        );
        assert_eq!(
            runtime_broker_validate_activation_profile(Some("  work  ")),
            Ok("work".to_string())
        );
        assert_eq!(
            runtime_broker_activation_profile_from_json(br#"{"current_profile":"  work  "}"#),
            Ok("work".to_string())
        );
        assert_eq!(
            runtime_broker_activation_profile_from_json(br#"{"current_profile":""}"#),
            Err(RuntimeBrokerAdminError::new(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            ))
        );
        assert!(runtime_broker_validate_activation_profile(Some(" ")).is_err());
        assert_eq!(
            runtime_broker_activation_success("work"),
            RuntimeBrokerActivationSuccess {
                ok: true,
                current_profile: "work".to_string(),
            }
        );
    }

    #[test]
    fn health_from_metadata_preserves_identity_fields() {
        let metadata = RuntimeBrokerMetadata {
            broker_key: "key".to_string(),
            listen_addr: "127.0.0.1:4567".to_string(),
            started_at: 100,
            current_profile: "work".to_string(),
            include_code_review: true,
            upstream_no_proxy: false,
            instance_token: "broker-token".to_string(),
            admin_token: "admin-token".to_string(),
            prodex_version: Some("0.7.0".to_string()),
            executable_path: Some("/tmp/prodex".to_string()),
            executable_sha256: Some("abc123".to_string()),
        };

        let health = RuntimeBrokerHealth::from_metadata(&metadata, 42, 3, true);

        assert_eq!(health.pid, 42);
        assert_eq!(health.active_requests, 3);
        assert_eq!(health.persistence_role, "owner");
        assert_eq!(health.current_profile, "work");
        assert_eq!(health.executable_sha256.as_deref(), Some("abc123"));
    }

    #[test]
    fn registry_reuse_decision_requires_launch_match_and_matching_health() {
        let registry = test_registry();
        let launch_config = RuntimeBrokerLaunchConfig {
            upstream_base_url: "https://upstream.example",
            include_code_review: true,
            upstream_no_proxy: false,
        };
        let health = RuntimeBrokerHealth {
            pid: registry.pid,
            started_at: registry.started_at,
            current_profile: registry.current_profile.clone(),
            include_code_review: registry.include_code_review,
            active_requests: 0,
            instance_token: registry.instance_token.clone(),
            persistence_role: "owner".to_string(),
            prodex_version: registry.prodex_version.clone(),
            executable_path: registry.executable_path.clone(),
            executable_sha256: registry.executable_sha256.clone(),
        };

        assert_eq!(
            runtime_broker_registry_reuse_decision(&registry, Some(&health), launch_config),
            RuntimeBrokerRegistryReuseDecision::Reuse
        );
        assert_eq!(
            runtime_broker_registry_reuse_decision(&registry, None, launch_config),
            RuntimeBrokerRegistryReuseDecision::MissingMatchingHealth
        );
        assert_eq!(
            runtime_broker_registry_reuse_decision(
                &registry,
                Some(&health),
                RuntimeBrokerLaunchConfig {
                    upstream_base_url: "https://other.example",
                    include_code_review: true,
                    upstream_no_proxy: false,
                },
            ),
            RuntimeBrokerRegistryReuseDecision::LaunchConfigMismatch
        );
    }

    #[test]
    fn broker_process_args_encode_optional_boolean_switches() {
        let config = RuntimeBrokerSpawnConfig {
            current_profile: "default",
            upstream_base_url: "https://upstream.example",
            include_code_review: true,
            upstream_no_proxy: true,
            broker_key: "key",
            instance_token: "broker-token",
            admin_token: "admin-token",
            listen_addr: Some("127.0.0.1:4567"),
        };
        let args = runtime_broker_process_args(config);

        assert_eq!(args[0], OsString::from("__runtime-broker"));
        assert!(args.contains(&OsString::from("--include-code-review")));
        assert!(args.contains(&OsString::from("--upstream-no-proxy")));
        assert!(args.contains(&OsString::from("--listen-addr")));
        assert!(args.contains(&OsString::from("127.0.0.1:4567")));

        let plan = runtime_broker_process_command_plan("/bin/prodex", "/tmp/prodex-home", config);

        assert_eq!(plan.executable, PathBuf::from("/bin/prodex"));
        assert_eq!(plan.prodex_home, PathBuf::from("/tmp/prodex-home"));
        assert!(plan.args.contains(&OsString::from("--instance-token")));
    }

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
    fn continuity_failure_reason_metrics_parse_text_and_json_logs() {
        let log = br#"[2026-04-22 10:00:00.000 +00:00] request=3 chain_retried_owner profile=second reason=previous_response_not_found_locked_affinity
{"timestamp":"2026-04-22 10:00:01.000 +00:00","event":"stale_continuation","message":"stale_continuation reason=websocket_reuse_watchdog_locked_affinity","fields":{"reason":"websocket_reuse_watchdog_locked_affinity"}}
[2026-04-22 10:00:01.500 +00:00] request=3 ignored_marker reason=ignored
{"timestamp":"2026-04-22 10:00:01.750 +00:00","message":"chain_dead_upstream_confirmed reason=\"json message reason\"","fields":{}}
[2026-04-22 10:00:02.000 +00:00] request=3 chain_dead_upstream_confirmed reason="quoted reason"
"#;

        let metrics = runtime_broker_continuity_failure_reason_metrics_from_log_bytes(log);

        assert_eq!(
            metrics.chain_retried_owner,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
        );
        assert_eq!(
            metrics.stale_continuation,
            BTreeMap::from([("websocket_reuse_watchdog_locked_affinity".to_string(), 1,)])
        );
        assert_eq!(
            metrics.chain_dead_upstream_confirmed,
            BTreeMap::from([
                ("json message reason".to_string(), 1),
                ("quoted reason".to_string(), 1),
            ])
        );
    }

    #[test]
    fn continuity_failure_reason_metrics_merge_live_without_double_counting() {
        let parsed = RuntimeBrokerContinuityFailureReasonMetrics {
            stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 2)]),
            ..RuntimeBrokerContinuityFailureReasonMetrics::default()
        };
        let baseline = RuntimeBrokerContinuityFailureReasonMetrics {
            stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 1)]),
            ..RuntimeBrokerContinuityFailureReasonMetrics::default()
        };
        let live = RuntimeBrokerContinuityFailureReasonMetrics {
            stale_continuation: BTreeMap::from([
                ("previous_response_not_found".to_string(), 1),
                ("watchdog".to_string(), 2),
            ]),
            ..RuntimeBrokerContinuityFailureReasonMetrics::default()
        };

        let metrics =
            runtime_broker_continuity_failure_reason_metrics_with_live(parsed, &baseline, live);

        assert_eq!(
            metrics.stale_continuation,
            BTreeMap::from([
                ("previous_response_not_found".to_string(), 2),
                ("watchdog".to_string(), 2),
            ])
        );
    }

    #[test]
    fn degraded_health_metrics_separates_profiles_and_routes() {
        let profile_health = BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileHealth {
                    score: 3,
                    updated_at: 98,
                },
            ),
            (
                "__route_health__:responses:main".to_string(),
                RuntimeProfileHealth {
                    score: 5,
                    updated_at: 99,
                },
            ),
            (
                "__auth_failure__:main".to_string(),
                RuntimeProfileHealth {
                    score: 5,
                    updated_at: 99,
                },
            ),
            (
                "stale".to_string(),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: 90,
                },
            ),
        ]);

        assert_eq!(
            runtime_broker_degraded_health_metrics(&profile_health, 100, 2),
            RuntimeBrokerDegradedHealthMetrics {
                profiles: 1,
                routes: 1,
            }
        );
    }

    #[test]
    fn metrics_snapshot_input_builds_broker_metrics_dto() {
        let metadata = RuntimeBrokerMetadata {
            broker_key: "key".to_string(),
            listen_addr: "127.0.0.1:4567".to_string(),
            started_at: 100,
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_token: "instance".to_string(),
            admin_token: "admin".to_string(),
            prodex_version: Some("0.7.0".to_string()),
            executable_path: Some("/tmp/prodex".to_string()),
            executable_sha256: Some("abc123".to_string()),
        };
        let lane = RuntimeBrokerLaneMetrics {
            active: 1,
            limit: 4,
            admissions_total: 10,
            releases_total: 9,
            global_limit_rejections_total: 1,
            lane_limit_rejections_total: 2,
            release_underflows_total: 0,
        };
        let traffic = RuntimeBrokerTrafficMetrics {
            responses: lane.clone(),
            compact: lane.clone(),
            websocket: lane.clone(),
            standard: lane,
        };
        let profile_inflight = BTreeMap::from([("main".to_string(), 2)]);
        let profile_health = BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileHealth {
                    score: 3,
                    updated_at: 99,
                },
            ),
            (
                "__route_health__:responses:main".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: 99,
                },
            ),
            (
                "__previous_response_not_found__:responses:resp-1".to_string(),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: 99,
                },
            ),
        ]);
        let continuation_statuses = RuntimeContinuationStatuses {
            response: BTreeMap::from([(
                "resp-1".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    last_verified_at: Some(80),
                    failure_count: 2,
                    ..RuntimeContinuationBindingStatus::default()
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        };

        let metrics =
            runtime_broker_metrics_from_snapshot_input(RuntimeBrokerMetricsSnapshotInput {
                metadata: &metadata,
                pid: 42,
                active_requests: 3,
                persistence_owner: true,
                active_request_limit: 8,
                local_overload_backoff_remaining_seconds: 5,
                runtime_state_lock_wait: RuntimeStateLockWaitMetrics {
                    wait_total_ns: 10,
                    wait_count: 1,
                    wait_max_ns: 10,
                },
                traffic,
                profile_inflight: &profile_inflight,
                profile_retry_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
                profile_transport_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
                profile_route_circuit_open_until: &BTreeMap::from([("main".to_string(), 99)]),
                profile_health: &profile_health,
                continuation_statuses: &continuation_statuses,
                continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics {
                    stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
                    ..RuntimeBrokerContinuityFailureReasonMetrics::default()
                },
                now: 100,
                health_decay_seconds: 10,
                stale_verified_seconds: 10,
                previous_response_negative_cache_seconds: 10,
            })
            .with_guard_counters(RuntimeBrokerMetricsGuardCounters {
                active_request_release_underflows_total: 1,
                profile_inflight_admissions_total: 2,
                profile_inflight_releases_total: 3,
                profile_inflight_release_underflows_total: 4,
            });

        assert_eq!(metrics.health.pid, 42);
        assert_eq!(metrics.health.persistence_role, "owner");
        assert_eq!(metrics.health.active_requests, 3);
        assert_eq!(metrics.profile_inflight.get("main"), Some(&2));
        assert_eq!(metrics.retry_backoffs, 1);
        assert_eq!(metrics.transport_backoffs, 1);
        assert_eq!(metrics.route_circuits, 0);
        assert_eq!(metrics.degraded_profiles, 1);
        assert_eq!(metrics.degraded_routes, 1);
        assert_eq!(metrics.continuations.verified, 1);
        assert_eq!(metrics.continuations.stale_verified_bindings.response, 1);
        assert_eq!(
            metrics
                .previous_response_continuity
                .negative_cache_entries
                .responses,
            1
        );
        assert_eq!(metrics.active_request_release_underflows_total, 1);
        assert_eq!(
            metrics
                .continuity_failure_reasons
                .stale_continuation
                .get("watchdog"),
            Some(&1)
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
    fn observed_binary_identity_prefers_matching_health_then_registry_then_process() {
        let mut registry = test_registry();
        registry.executable_sha256 = None;
        let process_identity = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.9.0".to_string()),
            executable_path: None,
            executable_sha256: Some("process-sha".to_string()),
        };
        let health = RuntimeBrokerHealth {
            pid: 42,
            started_at: 100,
            current_profile: "work".to_string(),
            include_code_review: true,
            active_requests: 0,
            instance_token: "broker-token".to_string(),
            persistence_role: "owner".to_string(),
            prodex_version: Some("0.8.0".to_string()),
            executable_path: None,
            executable_sha256: Some("health-sha".to_string()),
        };

        let observed = runtime_broker_observed_binary_identity(
            &registry,
            Some(&health),
            Some(&process_identity),
        );

        assert_eq!(observed.prodex_version.as_deref(), Some("0.8.0"));
        assert_eq!(observed.executable_sha256.as_deref(), Some("health-sha"));

        let mismatched_health = RuntimeBrokerHealth {
            instance_token: "other-token".to_string(),
            ..health
        };
        let observed = runtime_broker_observed_binary_identity(
            &registry,
            Some(&mismatched_health),
            Some(&process_identity),
        );

        assert_eq!(observed.prodex_version.as_deref(), Some("0.7.0"));
        assert_eq!(observed.executable_path, Some(PathBuf::from("/tmp/prodex")));
    }

    #[test]
    fn replacement_reason_prefers_sha_then_version_then_presence() {
        let current = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };

        assert_eq!(
            runtime_broker_replacement_reason(
                &current,
                &RuntimeProdexBinaryIdentity {
                    executable_sha256: Some("def456".to_string()),
                    ..RuntimeProdexBinaryIdentity::default()
                },
            ),
            "sha256_mismatch"
        );
        assert_eq!(
            runtime_broker_replacement_reason(
                &current,
                &RuntimeProdexBinaryIdentity {
                    prodex_version: Some("0.8.0".to_string()),
                    ..RuntimeProdexBinaryIdentity::default()
                },
            ),
            "version_mismatch"
        );
        assert_eq!(
            runtime_broker_replacement_reason(&current, &RuntimeProdexBinaryIdentity::default()),
            "identity_unresolved"
        );
    }

    #[test]
    fn version_guard_decision_defers_active_then_replaces() {
        let current_binary = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };
        let current_version = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: None,
        };
        let observed = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.8.0".to_string()),
            executable_path: None,
            executable_sha256: Some("def456".to_string()),
        };

        let decision = runtime_broker_version_guard_decision(
            true,
            &current_binary,
            &current_version,
            &observed,
            1,
            0,
        );
        assert_eq!(
            decision.outcome,
            RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests
        );
        assert_eq!(decision.current_identity.executable_sha256, None);

        let decision = runtime_broker_version_guard_decision(
            true,
            &current_binary,
            &current_version,
            &observed,
            0,
            0,
        );
        assert_eq!(decision.outcome, RuntimeBrokerVersionGuardOutcome::Replaced);
        assert_eq!(decision.replacement_reason, Some("version_mismatch"));
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
