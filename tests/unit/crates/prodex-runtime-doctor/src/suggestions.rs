use super::*;

const ACTIVE_REQUEST_PRESSURE_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/active_request_pressure.log");
const LANE_PRESSURE_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/lane_pressure.log");
const PERSISTENCE_BACKPRESSURE_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/persistence_backpressure.log");
const PROFILE_INFLIGHT_SATURATION_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/profile_inflight_saturation.log");
const ROUTE_SCOPED_PROFILE_HEALTH_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/route_scoped_profile_health.log");
const WEBSOCKET_CONNECT_OVERFLOW_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/websocket_connect_overflow.log");
const WEBSOCKET_DNS_OVERFLOW_LOG: &[u8] =
    include_bytes!("../../../../fixtures/runtime_doctor/websocket_dns_overflow.log");

fn runtime_doctor_fixture_suggestions(log: &[u8]) -> Vec<RuntimeDoctorPolicySuggestion> {
    let mut summary = summarize_runtime_log_tail(log);
    summary.pointer_exists = true;
    summary.log_exists = true;
    diagnosis::runtime_doctor_finalize_summary(&mut summary);
    runtime_doctor_policy_suggestions(&summary, RuntimeDoctorTuningSnapshot::default())
}

fn runtime_doctor_suggestion<'a>(
    suggestions: &'a [RuntimeDoctorPolicySuggestion],
    id: &str,
) -> &'a RuntimeDoctorPolicySuggestion {
    suggestions
        .iter()
        .find(|suggestion| suggestion.id == id)
        .unwrap_or_else(|| panic!("missing suggestion {id}: {suggestions:#?}"))
}

fn runtime_doctor_setting<'a>(
    suggestion: &'a RuntimeDoctorPolicySuggestion,
    key: &str,
) -> &'a RuntimeDoctorPolicySettingSuggestion {
    suggestion
        .settings
        .iter()
        .find(|setting| setting.key == key)
        .unwrap_or_else(|| panic!("missing setting {key}: {suggestion:#?}"))
}

#[test]
fn runtime_doctor_policy_suggestions_cover_pressure_fixture_logs() {
    let suggestions = runtime_doctor_fixture_suggestions(LANE_PRESSURE_LOG);
    let lane = runtime_doctor_suggestion(&suggestions, "lane_pressure");
    assert_eq!(lane.markers, vec!["runtime_proxy_lane_limit_reached"]);
    assert!(lane.snippet.contains("compact_active_limit = "));
    assert!(
        runtime_doctor_setting(lane, "compact_active_limit").suggested_value
            > runtime_doctor_setting(lane, "compact_active_limit").current_value
    );

    let suggestions = runtime_doctor_fixture_suggestions(ACTIVE_REQUEST_PRESSURE_LOG);
    let active = runtime_doctor_suggestion(&suggestions, "active_request_pressure");
    assert!(active.snippet.contains("active_request_limit = "));
    assert!(
        runtime_doctor_setting(active, "active_request_limit").suggested_value
            > runtime_doctor_setting(active, "active_request_limit").current_value
    );

    let suggestions = runtime_doctor_fixture_suggestions(PROFILE_INFLIGHT_SATURATION_LOG);
    let inflight = runtime_doctor_suggestion(&suggestions, "profile_inflight_saturation");
    assert!(inflight.snippet.contains("profile_inflight_hard_limit = "));
    assert!(
        runtime_doctor_setting(inflight, "profile_inflight_hard_limit").suggested_value
            > runtime_doctor_setting(inflight, "profile_inflight_hard_limit").current_value
    );
}

#[test]
fn runtime_doctor_policy_suggestions_cover_websocket_and_persistence_fixture_logs() {
    let suggestions = runtime_doctor_fixture_suggestions(WEBSOCKET_CONNECT_OVERFLOW_LOG);
    let connect = runtime_doctor_suggestion(&suggestions, "websocket_connect_overflow");
    assert!(
        connect
            .snippet
            .contains("websocket_connect_worker_count = ")
    );
    assert!(
        connect
            .snippet
            .contains("websocket_connect_queue_capacity = ")
    );
    assert!(
        connect
            .snippet
            .contains("websocket_connect_overflow_capacity = ")
    );

    let suggestions = runtime_doctor_fixture_suggestions(WEBSOCKET_DNS_OVERFLOW_LOG);
    let dns = runtime_doctor_suggestion(&suggestions, "websocket_dns_overflow");
    assert!(dns.snippet.contains("websocket_dns_worker_count = "));
    assert!(dns.snippet.contains("websocket_dns_queue_capacity = "));
    assert!(dns.snippet.contains("websocket_dns_overflow_capacity = "));

    let suggestions = runtime_doctor_fixture_suggestions(PERSISTENCE_BACKPRESSURE_LOG);
    let persistence = runtime_doctor_suggestion(&suggestions, "persistence_backpressure");
    assert!(persistence.snippet.contains("compact_active_limit = "));
    assert!(persistence.snippet.contains("standard_active_limit = "));
    assert!(
        persistence
            .snippet
            .contains("pressure_admission_wait_budget_ms = ")
    );
}

#[test]
fn runtime_doctor_policy_suggestions_cover_route_scoped_health_fixture_log() {
    let suggestions = runtime_doctor_fixture_suggestions(ROUTE_SCOPED_PROFILE_HEALTH_LOG);
    let health = runtime_doctor_suggestion(&suggestions, "route_scoped_profile_health");

    assert!(health.reason.contains("alpha/responses"));
    assert!(health.snippet.contains("profile_inflight_soft_limit = "));
    assert!(health.snippet.contains("profile_inflight_hard_limit = "));
    assert!(
        runtime_doctor_setting(health, "profile_inflight_soft_limit").suggested_value
            <= runtime_doctor_setting(health, "profile_inflight_soft_limit").current_value
    );
}

#[test]
fn runtime_doctor_policy_suggestion_lines_render_snippet() {
    let suggestions = runtime_doctor_fixture_suggestions(LANE_PRESSURE_LOG);
    let lines = runtime_doctor_policy_suggestion_lines(&suggestions);

    assert_eq!(lines[0], "Runtime Policy Suggestions");
    assert!(lines.iter().any(|line| line.contains("Lane pressure")));
    assert!(lines.iter().any(|line| line.trim() == "[runtime_proxy]"));
    assert!(
        lines
            .iter()
            .any(|line| line.trim_start().starts_with("compact_active_limit = "))
    );
}
