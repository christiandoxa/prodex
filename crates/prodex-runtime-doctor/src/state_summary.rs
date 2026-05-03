use std::collections::{BTreeMap, BTreeSet};

use crate::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary, RuntimeDoctorProfileSummary, RuntimeDoctorRouteSummary,
};

const RUNTIME_DOCTOR_BINDING_SAMPLE_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBindingSourceInput<'a, B> {
    pub response_profile_bindings: &'a BTreeMap<String, B>,
    pub session_profile_bindings: &'a BTreeMap<String, B>,
    pub turn_state_bindings: &'a BTreeMap<String, B>,
    pub session_id_bindings: &'a BTreeMap<String, B>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBindingStateInput<'a, B> {
    pub active_profile: Option<&'a str>,
    pub profile_names: &'a [String],
    pub last_run_selected_profiles: usize,
    pub state: RuntimeDoctorBindingSourceInput<'a, B>,
    pub runtime_continuations: RuntimeDoctorBindingSourceInput<'a, B>,
    pub continuation_journal: RuntimeDoctorBindingSourceInput<'a, B>,
    pub merged_continuations: RuntimeDoctorBindingSourceInput<'a, B>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBackoffMaps<'a> {
    pub retry_backoff_until: &'a BTreeMap<String, i64>,
    pub transport_backoff_until: &'a BTreeMap<String, i64>,
    pub route_circuit_open_until: &'a BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorHealthScore {
    pub score: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorStateSummaryConfig {
    pub health_decay_seconds: i64,
    pub bad_pairing_decay_seconds: i64,
    pub performance_decay_seconds: i64,
    pub usage_snapshot_stale_grace_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorRouteKind {
    Responses,
    Websocket,
    Compact,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeDoctorQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorUsageSnapshot {
    pub checked_at: i64,
    pub five_hour_status: RuntimeDoctorQuotaWindowStatus,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: RuntimeDoctorQuotaWindowStatus,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDoctorBinaryIdentity {
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

impl RuntimeDoctorBinaryIdentity {
    pub fn is_present(&self) -> bool {
        self.prodex_version.is_some()
            || self.executable_path.is_some()
            || self.executable_sha256.is_some()
    }
}

pub fn runtime_doctor_backoff_maps_from_runtime(
    backoffs: &prodex_runtime_state::RuntimeProfileBackoffs,
) -> RuntimeDoctorBackoffMaps<'_> {
    RuntimeDoctorBackoffMaps {
        retry_backoff_until: &backoffs.retry_backoff_until,
        transport_backoff_until: &backoffs.transport_backoff_until,
        route_circuit_open_until: &backoffs.route_circuit_open_until,
    }
}

pub fn runtime_doctor_health_scores_from_runtime(
    scores: &BTreeMap<String, prodex_runtime_state::RuntimeProfileHealth>,
) -> BTreeMap<String, RuntimeDoctorHealthScore> {
    scores
        .iter()
        .map(|(key, health)| {
            (
                key.clone(),
                RuntimeDoctorHealthScore {
                    score: health.score,
                    updated_at: health.updated_at,
                },
            )
        })
        .collect()
}

pub fn runtime_doctor_quota_window_status_from_runtime(
    status: prodex_quota::RuntimeQuotaWindowStatus,
) -> RuntimeDoctorQuotaWindowStatus {
    match status {
        prodex_quota::RuntimeQuotaWindowStatus::Ready => RuntimeDoctorQuotaWindowStatus::Ready,
        prodex_quota::RuntimeQuotaWindowStatus::Thin => RuntimeDoctorQuotaWindowStatus::Thin,
        prodex_quota::RuntimeQuotaWindowStatus::Critical => {
            RuntimeDoctorQuotaWindowStatus::Critical
        }
        prodex_quota::RuntimeQuotaWindowStatus::Exhausted => {
            RuntimeDoctorQuotaWindowStatus::Exhausted
        }
        prodex_quota::RuntimeQuotaWindowStatus::Unknown => RuntimeDoctorQuotaWindowStatus::Unknown,
    }
}

pub fn runtime_doctor_usage_snapshot_from_runtime(
    snapshot: &prodex_runtime_state::RuntimeProfileUsageSnapshot<
        prodex_quota::RuntimeQuotaWindowStatus,
    >,
) -> RuntimeDoctorUsageSnapshot {
    RuntimeDoctorUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_doctor_quota_window_status_from_runtime(
            snapshot.five_hour_status,
        ),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_doctor_quota_window_status_from_runtime(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_doctor_usage_snapshots_from_runtime(
    usage_snapshots: &BTreeMap<
        String,
        prodex_runtime_state::RuntimeProfileUsageSnapshot<prodex_quota::RuntimeQuotaWindowStatus>,
    >,
) -> BTreeMap<String, RuntimeDoctorUsageSnapshot> {
    usage_snapshots
        .iter()
        .map(|(profile_name, snapshot)| {
            (
                profile_name.clone(),
                runtime_doctor_usage_snapshot_from_runtime(snapshot),
            )
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct RuntimeDoctorQuotaWindowSummary {
    status: RuntimeDoctorQuotaWindowStatus,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeDoctorQuotaSummary {
    five_hour: RuntimeDoctorQuotaWindowSummary,
    weekly: RuntimeDoctorQuotaWindowSummary,
    route_band: RuntimeDoctorQuotaPressureBand,
}

pub fn runtime_doctor_route_kind_label(route_kind: RuntimeDoctorRouteKind) -> &'static str {
    match route_kind {
        RuntimeDoctorRouteKind::Responses => "responses",
        RuntimeDoctorRouteKind::Websocket => "websocket",
        RuntimeDoctorRouteKind::Compact => "compact",
        RuntimeDoctorRouteKind::Standard => "standard",
    }
}

fn runtime_doctor_route_key_parts<'a>(key: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

fn runtime_doctor_route_health_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_record_binding_source_kind<B, F, G>(
    source: &mut RuntimeDoctorBindingSourceSummary,
    profile_counts: &mut BTreeMap<String, RuntimeDoctorBindingProfileSummary>,
    known_profiles: &BTreeSet<&str>,
    kind: &str,
    bindings: &BTreeMap<String, B>,
    profile_name: F,
    bound_at: G,
) where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    for (key, binding) in bindings {
        let profile = profile_name(binding);
        let bound_at = bound_at(binding);
        source.oldest_bound_at = Some(
            source
                .oldest_bound_at
                .map(|oldest| oldest.min(bound_at))
                .unwrap_or(bound_at),
        );
        source.newest_bound_at = Some(
            source
                .newest_bound_at
                .map(|newest| newest.max(bound_at))
                .unwrap_or(bound_at),
        );
        if !known_profiles.contains(profile) {
            source.missing_profile_bindings += 1;
            if source.missing_profile_binding_samples.len() < RUNTIME_DOCTOR_BINDING_SAMPLE_LIMIT {
                source
                    .missing_profile_binding_samples
                    .push(format!("{kind}:{key}->{profile}"));
            }
        }
        let profile_entry = profile_counts
            .entry(profile.to_string())
            .or_insert_with(|| RuntimeDoctorBindingProfileSummary {
                profile: profile.to_string(),
                ..RuntimeDoctorBindingProfileSummary::default()
            });
        match kind {
            "response" => profile_entry.response_bindings += 1,
            "session" => profile_entry.session_bindings += 1,
            "turn_state" => profile_entry.turn_state_bindings += 1,
            "session_id" => profile_entry.session_id_bindings += 1,
            _ => {}
        }
        profile_entry.total_bindings += 1;
    }
}

fn runtime_doctor_binding_source_summary<B, F, G>(
    input: RuntimeDoctorBindingSourceInput<'_, B>,
    known_profiles: &BTreeSet<&str>,
    profile_name: F,
    bound_at: G,
) -> RuntimeDoctorBindingSourceSummary
where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    let mut source = RuntimeDoctorBindingSourceSummary {
        response_bindings: input.response_profile_bindings.len(),
        session_bindings: input.session_profile_bindings.len(),
        turn_state_bindings: input.turn_state_bindings.len(),
        session_id_bindings: input.session_id_bindings.len(),
        total_bindings: input.response_profile_bindings.len()
            + input.session_profile_bindings.len()
            + input.turn_state_bindings.len()
            + input.session_id_bindings.len(),
        ..RuntimeDoctorBindingSourceSummary::default()
    };
    let mut profile_counts = BTreeMap::new();
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "response",
        input.response_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "session",
        input.session_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "turn_state",
        input.turn_state_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "session_id",
        input.session_id_bindings,
        profile_name,
        bound_at,
    );
    source.profiles = profile_counts.into_values().collect();
    source.profiles.sort_by(|left, right| {
        right
            .total_bindings
            .cmp(&left.total_bindings)
            .then_with(|| left.profile.cmp(&right.profile))
    });
    source.profile_count = source.profiles.len();
    source
}

pub fn runtime_doctor_binding_state_summary<B, F, G>(
    input: RuntimeDoctorBindingStateInput<'_, B>,
    profile_name: F,
    bound_at: G,
) -> RuntimeDoctorBindingStateSummary
where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    let known_profiles = input
        .profile_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    RuntimeDoctorBindingStateSummary {
        active_profile: input.active_profile.map(str::to_string),
        profile_count: input.profile_names.len(),
        last_run_selected_profiles: input.last_run_selected_profiles,
        state: runtime_doctor_binding_source_summary(
            input.state,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        runtime_continuations: runtime_doctor_binding_source_summary(
            input.runtime_continuations,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        continuation_journal: runtime_doctor_binding_source_summary(
            input.continuation_journal,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        merged_continuations: runtime_doctor_binding_source_summary(
            input.merged_continuations,
            &known_profiles,
            profile_name,
            bound_at,
        ),
    }
}

fn runtime_doctor_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_doctor_route_key_parts(key, "__route_transport_backoff__:")
}

fn runtime_doctor_transport_backoff_profile_name(key: &str) -> &str {
    runtime_doctor_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

fn runtime_doctor_effective_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

fn runtime_doctor_effective_health_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> u32 {
    runtime_doctor_effective_score(entry, now, config.health_decay_seconds)
}

fn runtime_doctor_effective_health_score_from_map(
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    key: &str,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> u32 {
    scores
        .get(key)
        .map(|entry| runtime_doctor_effective_health_score(entry, now, config))
        .unwrap_or(0)
}

fn runtime_doctor_effective_score_from_map(
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    scores
        .get(key)
        .map(|entry| runtime_doctor_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

fn runtime_doctor_push_route_circuits(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
) {
    for (key, until) in backoffs.route_circuit_open_until {
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_circuit__:")
        {
            let state = if *until > now { "open" } else { "half-open" };
            routes.push(format!(
                "{profile_name}/{route} circuit={state} until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_transport_backoffs(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
) {
    for (profile_name, until) in backoffs.transport_backoff_until {
        if let Some((route, profile_name)) =
            runtime_doctor_transport_backoff_key_parts(profile_name)
        {
            routes.push(format!(
                "{profile_name}/{route} transport_backoff until={until}"
            ));
        } else {
            routes.push(format!(
                "{profile_name}/transport transport_backoff until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_retry_backoffs(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
) {
    for (profile_name, until) in backoffs.retry_backoff_until {
        routes.push(format!("{profile_name}/retry retry_backoff until={until}"));
    }
}

fn runtime_doctor_push_score_degradations(
    routes: &mut Vec<String>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) {
    for (key, health) in scores {
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_bad_pairing__:")
        {
            let score =
                runtime_doctor_effective_score(health, now, config.bad_pairing_decay_seconds);
            if score > 0 {
                routes.push(format!("{profile_name}/{route} bad_pairing={score}"));
            }
            continue;
        }
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_health__:")
        {
            let score = runtime_doctor_effective_health_score(health, now, config);
            if score > 0 {
                routes.push(format!("{profile_name}/{route} health={score}"));
            }
        }
    }
}

pub fn runtime_doctor_degraded_routes(
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<String> {
    let mut routes = Vec::new();
    runtime_doctor_push_route_circuits(&mut routes, backoffs, now);
    runtime_doctor_push_transport_backoffs(&mut routes, backoffs);
    runtime_doctor_push_retry_backoffs(&mut routes, backoffs);
    runtime_doctor_push_score_degradations(&mut routes, scores, now, config);
    routes.sort();
    routes.dedup();
    routes.truncate(8);
    routes
}

fn runtime_doctor_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeDoctorQuotaWindowStatus,
    _remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeDoctorQuotaWindowSummary {
    if reset_at != i64::MAX && reset_at <= now {
        return RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Ready,
        };
    }
    RuntimeDoctorQuotaWindowSummary { status }
}

fn runtime_doctor_quota_pressure_band_from_window_status(
    status: RuntimeDoctorQuotaWindowStatus,
) -> RuntimeDoctorQuotaPressureBand {
    match status {
        RuntimeDoctorQuotaWindowStatus::Ready => RuntimeDoctorQuotaPressureBand::Healthy,
        RuntimeDoctorQuotaWindowStatus::Thin => RuntimeDoctorQuotaPressureBand::Thin,
        RuntimeDoctorQuotaWindowStatus::Critical => RuntimeDoctorQuotaPressureBand::Critical,
        RuntimeDoctorQuotaWindowStatus::Exhausted => RuntimeDoctorQuotaPressureBand::Exhausted,
        RuntimeDoctorQuotaWindowStatus::Unknown => RuntimeDoctorQuotaPressureBand::Unknown,
    }
}

fn runtime_doctor_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeDoctorUsageSnapshot,
    route_kind: RuntimeDoctorRouteKind,
    now: i64,
) -> RuntimeDoctorQuotaSummary {
    let five_hour = runtime_doctor_quota_window_summary_from_usage_snapshot_at(
        snapshot.five_hour_status,
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        now,
    );
    let weekly = runtime_doctor_quota_window_summary_from_usage_snapshot_at(
        snapshot.weekly_status,
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
        now,
    );
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeDoctorRouteKind::Responses | RuntimeDoctorRouteKind::Websocket => weekly.status,
            RuntimeDoctorRouteKind::Compact | RuntimeDoctorRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .map(runtime_doctor_quota_pressure_band_from_window_status)
    .fold(
        RuntimeDoctorQuotaPressureBand::Healthy,
        RuntimeDoctorQuotaPressureBand::max,
    );
    RuntimeDoctorQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

fn runtime_doctor_quota_pressure_band_reason(band: RuntimeDoctorQuotaPressureBand) -> &'static str {
    match band {
        RuntimeDoctorQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeDoctorQuotaPressureBand::Thin => "quota_thin",
        RuntimeDoctorQuotaPressureBand::Critical => "quota_critical",
        RuntimeDoctorQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeDoctorQuotaPressureBand::Unknown => "quota_unknown",
    }
}

fn runtime_doctor_quota_window_status_reason(
    status: RuntimeDoctorQuotaWindowStatus,
) -> &'static str {
    match status {
        RuntimeDoctorQuotaWindowStatus::Ready => "ready",
        RuntimeDoctorQuotaWindowStatus::Thin => "thin",
        RuntimeDoctorQuotaWindowStatus::Critical => "critical",
        RuntimeDoctorQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeDoctorQuotaWindowStatus::Unknown => "unknown",
    }
}

fn runtime_doctor_usage_snapshot_hold_active(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeDoctorQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

fn runtime_doctor_usage_snapshot_hold_expired(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeDoctorQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

fn runtime_doctor_usage_snapshot_is_usable(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    if runtime_doctor_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_doctor_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

pub fn runtime_doctor_quota_freshness_label(
    snapshot: Option<&RuntimeDoctorUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> &'static str {
    match snapshot {
        Some(snapshot)
            if runtime_doctor_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds) =>
        {
            "fresh"
        }
        Some(_) => "stale",
        None => "missing",
    }
}

pub fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> &'static str {
    match until {
        Some(until) if until > now => "open",
        Some(_) => "half_open",
        None => "closed",
    }
}

fn runtime_doctor_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_doctor_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

fn runtime_doctor_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_doctor_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

fn runtime_doctor_unknown_quota_summary() -> RuntimeDoctorQuotaSummary {
    RuntimeDoctorQuotaSummary {
        five_hour: RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Unknown,
        },
        weekly: RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Unknown,
        },
        route_band: RuntimeDoctorQuotaPressureBand::Unknown,
    }
}

pub fn runtime_doctor_profile_summaries(
    profile_names: &[String],
    usage_snapshots: &BTreeMap<String, RuntimeDoctorUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<RuntimeDoctorProfileSummary> {
    let mut profiles = Vec::new();
    for profile_name in profile_names {
        let snapshot = usage_snapshots.get(profile_name);
        let quota_age_seconds = snapshot
            .map(|snapshot| now.saturating_sub(snapshot.checked_at))
            .unwrap_or(i64::MAX);
        let routes = [
            RuntimeDoctorRouteKind::Responses,
            RuntimeDoctorRouteKind::Websocket,
            RuntimeDoctorRouteKind::Compact,
            RuntimeDoctorRouteKind::Standard,
        ]
        .into_iter()
        .map(|route_kind| {
            let quota_summary = snapshot
                .map(|snapshot| {
                    runtime_doctor_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now)
                })
                .unwrap_or_else(runtime_doctor_unknown_quota_summary);
            let circuit_key = runtime_doctor_route_circuit_key(profile_name, route_kind);
            RuntimeDoctorRouteSummary {
                route: runtime_doctor_route_kind_label(route_kind).to_string(),
                circuit_state: runtime_doctor_route_circuit_state(
                    backoffs.route_circuit_open_until.get(&circuit_key).copied(),
                    now,
                )
                .to_string(),
                circuit_until: backoffs.route_circuit_open_until.get(&circuit_key).copied(),
                transport_backoff_until: runtime_doctor_transport_backoff_until_from_map(
                    backoffs.transport_backoff_until,
                    profile_name,
                    route_kind,
                    now,
                ),
                health_score: runtime_doctor_effective_health_score_from_map(
                    scores,
                    &runtime_doctor_route_health_key(profile_name, route_kind),
                    now,
                    config,
                ),
                bad_pairing_score: runtime_doctor_effective_score_from_map(
                    scores,
                    &runtime_doctor_route_bad_pairing_key(profile_name, route_kind),
                    now,
                    config.bad_pairing_decay_seconds,
                ),
                performance_score: runtime_doctor_effective_score_from_map(
                    scores,
                    &runtime_doctor_route_performance_key(profile_name, route_kind),
                    now,
                    config.performance_decay_seconds,
                ),
                quota_band: runtime_doctor_quota_pressure_band_reason(quota_summary.route_band)
                    .to_string(),
                five_hour_status: runtime_doctor_quota_window_status_reason(
                    quota_summary.five_hour.status,
                )
                .to_string(),
                weekly_status: runtime_doctor_quota_window_status_reason(
                    quota_summary.weekly.status,
                )
                .to_string(),
            }
        })
        .collect::<Vec<_>>();
        profiles.push(RuntimeDoctorProfileSummary {
            profile: profile_name.clone(),
            quota_freshness: runtime_doctor_quota_freshness_label(
                snapshot,
                now,
                config.usage_snapshot_stale_grace_seconds,
            )
            .to_string(),
            quota_age_seconds,
            retry_backoff_until: backoffs.retry_backoff_until.get(profile_name).copied(),
            transport_backoff_until: runtime_doctor_transport_backoff_max_until(
                backoffs.transport_backoff_until,
                profile_name,
                now,
            ),
            routes,
        });
    }
    profiles
}

pub fn runtime_doctor_runtime_broker_mismatch_reason(
    current: &RuntimeDoctorBinaryIdentity,
    observed: &RuntimeDoctorBinaryIdentity,
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
            _ => "none",
        },
    }
}

#[cfg(test)]
#[path = "../tests/src/state_summary.rs"]
mod tests;
