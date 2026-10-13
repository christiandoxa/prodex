use std::collections::BTreeMap;

use super::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorRouteKind,
    RuntimeDoctorStateSummaryConfig,
};

pub fn runtime_doctor_route_kind_label(route_kind: RuntimeDoctorRouteKind) -> &'static str {
    match route_kind {
        RuntimeDoctorRouteKind::Responses => "responses",
        RuntimeDoctorRouteKind::Websocket => "websocket",
        RuntimeDoctorRouteKind::Compact => "compact",
        RuntimeDoctorRouteKind::Standard => "standard",
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_route_health_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

// Keep the score edge-case oracle available to the existing state-summary
// tests while production degraded-route selection is fully Mojo-owned.
#[cfg(test)]
fn runtime_doctor_effective_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    prodex_mojo_core::rich::runtime_doctor_state_plan(
        prodex_mojo_core::rich::RuntimeDoctorStatePlanInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_OP_SCORE,
            now,
            score: i64::from(entry.score),
            updated_at: entry.updated_at,
            decay_seconds: decay_seconds.max(1),
            ..prodex_mojo_core::rich::RuntimeDoctorStatePlanInput::default()
        },
    )
    .expect("Mojo runtime-doctor score plan returned invalid output")
    .effective_score as u32
}

#[cfg(test)]
pub(super) fn runtime_doctor_effective_health_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> u32 {
    runtime_doctor_effective_score(entry, now, config.health_decay_seconds)
}

pub fn runtime_doctor_degraded_routes(
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<String> {
    let plan = prodex_mojo_core::rich::runtime_doctor_degraded_routes(
        backoffs
            .retry_backoff_until
            .iter()
            .map(|(key, until)| (key.as_str(), *until)),
        backoffs
            .transport_backoff_until
            .iter()
            .map(|(key, until)| (key.as_str(), *until)),
        backoffs
            .route_circuit_open_until
            .iter()
            .map(|(key, until)| (key.as_str(), *until)),
        scores
            .iter()
            .map(|(key, health)| (key.as_str(), i64::from(health.score), health.updated_at)),
        now,
        config.health_decay_seconds,
        config.bad_pairing_decay_seconds,
    )
    .expect("Mojo runtime-doctor degraded-route plan returned invalid output");
    let keys = plan.keys;
    plan.records
        .into_iter()
        .map(|record| {
            let key = keys[usize::try_from(record.source_index)
                .expect("validated Mojo degraded-route source index")];
            let profile_start = usize::try_from(record.profile_start)
                .expect("validated Mojo degraded-route profile offset");
            let profile_end = usize::try_from(record.profile_start + record.profile_len)
                .expect("validated Mojo degraded-route profile end");
            let profile = &key[profile_start..profile_end];
            let route = if record.route_start < 0 {
                match record.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY => "retry",
                    _ => "transport",
                }
            } else {
                let route_start = usize::try_from(record.route_start)
                    .expect("validated Mojo degraded-route route offset");
                let route_end = usize::try_from(record.route_start + record.route_len)
                    .expect("validated Mojo degraded-route route end");
                &key[route_start..route_end]
            };
            match record.kind {
                prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT => {
                    let state = if record.value > now {
                        "open"
                    } else {
                        "half-open"
                    };
                    format!("{profile}/{route} circuit={state} until={}", record.value)
                }
                prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT => {
                    format!("{profile}/{route} transport_backoff until={}", record.value)
                }
                prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY => {
                    format!("{profile}/{route} retry_backoff until={}", record.value)
                }
                prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_BAD_PAIRING => {
                    format!("{profile}/{route} bad_pairing={}", record.value)
                }
                prodex_mojo_core::rich::RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH => {
                    format!("{profile}/{route} health={}", record.value)
                }
                _ => unreachable!("validated Mojo degraded-route kind"),
            }
        })
        .collect()
}
