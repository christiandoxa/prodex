use super::final_summary::runtime_doctor_top_facet;
use super::marker_accessors::*;
use super::next_steps::*;
use super::*;

fn runtime_doctor_incident_marker_total(
    summary: &RuntimeDoctorSummary,
    markers: &[&'static str],
) -> usize {
    markers
        .iter()
        .map(|marker| runtime_doctor_marker_count(summary, marker))
        .sum()
}

fn runtime_doctor_incident_marker_evidence(
    summary: &RuntimeDoctorSummary,
    marker: &'static str,
) -> Option<String> {
    let count = runtime_doctor_marker_count(summary, marker);
    (count > 0).then(|| format!("{marker}={count}"))
}

fn runtime_doctor_incident_latest_load(
    summary: &RuntimeDoctorSummary,
    marker: &str,
) -> Option<String> {
    let active = runtime_doctor_marker_last_field(summary, marker, "active")?;
    let limit = runtime_doctor_marker_last_field(summary, marker, "limit")?;
    Some(format!("latest_load={active}/{limit}"))
}

fn runtime_doctor_incident_top_facet_evidence(
    summary: &RuntimeDoctorSummary,
    facet: &str,
) -> Option<String> {
    runtime_doctor_top_facet(summary, facet).map(|value| format!("top_{facet}={value}"))
}

fn runtime_doctor_incident(
    id: &str,
    cause: impl Into<String>,
    evidence: Vec<Option<String>>,
    markers: &[&'static str],
    next_action: impl Into<String>,
) -> RuntimeDoctorIncidentExplanation {
    RuntimeDoctorIncidentExplanation {
        id: id.to_string(),
        cause: cause.into(),
        evidence: evidence.into_iter().flatten().collect(),
        markers: markers.iter().map(|marker| (*marker).to_string()).collect(),
        next_action: next_action.into(),
    }
}

pub fn runtime_doctor_incident_explainer(
    summary: &RuntimeDoctorSummary,
) -> Vec<RuntimeDoctorIncidentExplanation> {
    let mut incidents = Vec::new();

    let lane_markers = ["runtime_proxy_lane_limit_reached"];
    let lane_count = runtime_doctor_incident_marker_total(summary, &lane_markers);
    if lane_count > 0 {
        let lane =
            runtime_doctor_marker_last_field(summary, "runtime_proxy_lane_limit_reached", "lane")
                .unwrap_or("unknown");
        incidents.push(runtime_doctor_incident(
            "lane_saturation",
            format!("Local {lane} lane admission limit blocked fresh work before commit."),
            vec![
                Some(format!("runtime_proxy_lane_limit_reached={lane_count}")),
                runtime_doctor_incident_latest_load(summary, "runtime_proxy_lane_limit_reached"),
                runtime_doctor_incident_top_facet_evidence(summary, "lane"),
                runtime_doctor_incident_top_facet_evidence(summary, "route"),
            ],
            &lane_markers,
            runtime_doctor_lane_pressure_next_step(summary),
        ));
    }

    let active_markers = [
        "runtime_proxy_active_limit_reached",
        "runtime_proxy_queue_overloaded",
    ];
    let active_count = runtime_doctor_incident_marker_total(summary, &active_markers);
    if active_count > 0 {
        incidents.push(runtime_doctor_incident(
            "active_request_pressure",
            "Global proxy admission saturated before upstream commit.",
            vec![
                runtime_doctor_incident_marker_evidence(
                    summary,
                    "runtime_proxy_active_limit_reached",
                ),
                runtime_doctor_incident_marker_evidence(summary, "runtime_proxy_queue_overloaded"),
                runtime_doctor_incident_latest_load(summary, "runtime_proxy_active_limit_reached"),
                runtime_doctor_incident_top_facet_evidence(summary, "transport"),
            ],
            &active_markers,
            runtime_doctor_active_pressure_next_step(summary),
        ));
    }

    let inflight_markers = ["profile_inflight_saturated"];
    let inflight_count = runtime_doctor_incident_marker_total(summary, &inflight_markers);
    if inflight_count > 0 {
        let profile =
            runtime_doctor_marker_last_field(summary, "profile_inflight_saturated", "profile")
                .unwrap_or("selected profile");
        incidents.push(runtime_doctor_incident(
            "profile_inflight_saturation",
            format!("Profile {profile} hit its per-profile in-flight hard cap."),
            vec![
                Some(format!("profile_inflight_saturated={inflight_count}")),
                runtime_doctor_marker_last_field(
                    summary,
                    "profile_inflight_saturated",
                    "hard_limit",
                )
                .map(|limit| format!("hard_limit={limit}")),
                runtime_doctor_incident_top_facet_evidence(summary, "profile"),
                runtime_doctor_incident_top_facet_evidence(summary, "route"),
            ],
            &inflight_markers,
            runtime_doctor_profile_inflight_saturated_next_step(summary),
        ));
    }

    let transport_markers = [
        "profile_transport_backoff",
        "profile_transport_failure",
        "upstream_connect_timeout",
        "upstream_connect_dns_error",
        "upstream_tls_handshake_error",
        "upstream_connect_error",
        "upstream_connect_http",
        "upstream_close_before_completed",
        "upstream_connection_closed",
        "upstream_read_error",
        "upstream_send_error",
        "upstream_stream_error",
        "stream_read_error",
    ];
    let transport_count = runtime_doctor_incident_marker_total(summary, &transport_markers);
    if transport_count > 0 || runtime_doctor_marker_count(summary, "profile_health") > 0 {
        let cause = if runtime_doctor_marker_count(summary, "profile_transport_backoff") > 0 {
            "A profile route is in short transport backoff after upstream transport failures."
        } else if runtime_doctor_marker_count(summary, "stream_read_error") > 0 {
            "Upstream stream failed after commit; Codex likely saw a natural transport failure."
        } else {
            "Upstream transport/connect failures were observed before or during commit."
        };
        incidents.push(runtime_doctor_incident(
            "transport_backoff",
            cause,
            vec![
                Some(format!("transport_markers={transport_count}")),
                runtime_doctor_incident_marker_evidence(summary, "profile_transport_backoff"),
                runtime_doctor_incident_marker_evidence(summary, "stream_read_error"),
                runtime_doctor_incident_marker_evidence(summary, "upstream_connect_timeout"),
                runtime_doctor_incident_marker_evidence(summary, "upstream_connect_error"),
                runtime_doctor_incident_top_facet_evidence(summary, "profile"),
                runtime_doctor_incident_top_facet_evidence(summary, "route"),
                runtime_doctor_incident_top_facet_evidence(summary, "reason"),
            ],
            &transport_markers,
            if runtime_doctor_marker_count(summary, "profile_health") > 0 {
                runtime_doctor_route_health_next_step(summary)
            } else {
                runtime_doctor_transport_backoff_next_step(summary)
            },
        ));
    }

    let quota_markers = [
        "quota_blocked",
        "quota_critical_floor_before_send",
        "responses_pre_send_skip",
        "websocket_pre_send_skip",
        "upstream_usage_limit_passthrough",
        "profile_retry_backoff",
        "profile_quota_quarantine",
        "selection_skip_sync_probe",
        "profile_probe_refresh_backpressure",
        "profile_probe_refresh_error",
    ];
    let quota_count = runtime_doctor_incident_marker_total(summary, &quota_markers);
    if quota_count > 0 || summary.quota_freshness_pressure == "stale_risk" {
        let cause = if runtime_doctor_marker_count(summary, "quota_blocked") > 0
            || runtime_doctor_marker_count(summary, "quota_critical_floor_before_send") > 0
            || runtime_doctor_marker_count(summary, "responses_pre_send_skip") > 0
            || runtime_doctor_marker_count(summary, "websocket_pre_send_skip") > 0
        {
            "Quota guard blocked or skipped near-exhausted sends before commit."
        } else if summary.quota_freshness_pressure == "stale_risk" {
            "Quota snapshots or background probes may be stale under selection pressure."
        } else {
            "Quota-related backoff or upstream usage-limit markers were observed."
        };
        incidents.push(runtime_doctor_incident(
            "quota_pressure",
            cause,
            vec![
                Some(format!("quota_markers={quota_count}")),
                runtime_doctor_incident_marker_evidence(summary, "quota_blocked"),
                runtime_doctor_incident_marker_evidence(
                    summary,
                    "quota_critical_floor_before_send",
                ),
                runtime_doctor_incident_marker_evidence(summary, "responses_pre_send_skip"),
                Some(format!(
                    "quota_freshness={}",
                    summary.quota_freshness_pressure
                )),
                runtime_doctor_incident_top_facet_evidence(summary, "quota_source"),
                runtime_doctor_incident_top_facet_evidence(summary, "quota_band"),
                runtime_doctor_incident_top_facet_evidence(summary, "profile"),
            ],
            &quota_markers,
            runtime_doctor_quota_pressure_next_step(summary),
        ));
    }

    let precommit_markers = [
        "precommit_budget_exhausted",
        "compact_precommit_budget_exhausted",
        "compact_exit_precommit_budget_exhausted",
        "compact_candidate_exhausted",
        "compact_exit_candidate_exhausted",
    ];
    let precommit_count = runtime_doctor_incident_marker_total(summary, &precommit_markers);
    if precommit_count > 0 {
        incidents.push(runtime_doctor_incident(
            "precommit_budget_exhausted",
            "Candidate selection exhausted before any upstream response was committed.",
            vec![
                runtime_doctor_incident_marker_evidence(summary, "precommit_budget_exhausted"),
                runtime_doctor_incident_marker_evidence(
                    summary,
                    "compact_precommit_budget_exhausted",
                ),
                runtime_doctor_incident_marker_evidence(summary, "compact_candidate_exhausted"),
                runtime_doctor_incident_top_facet_evidence(summary, "route"),
                runtime_doctor_incident_top_facet_evidence(summary, "reason"),
            ],
            &precommit_markers,
            runtime_doctor_precommit_budget_next_step(summary),
        ));
    }

    incidents
}
