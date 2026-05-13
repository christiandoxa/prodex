use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LANE_PRESSURE_LOG: &[u8] = include_bytes!("../fixtures/runtime_doctor/lane_pressure.log");
const ACTIVE_REQUEST_PRESSURE_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/active_request_pressure.log");
const PERSISTENCE_BACKPRESSURE_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/persistence_backpressure.log");
const PREVIOUS_RESPONSE_FAIL_CLOSED_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/previous_response_fail_closed.log");
const PROFILE_INFLIGHT_SATURATION_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/profile_inflight_saturation.log");
const ROUTE_SCOPED_PROFILE_HEALTH_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/route_scoped_profile_health.log");
const WEBSOCKET_CONNECT_OVERFLOW_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/websocket_connect_overflow.log");
const PROFILE_AUTH_RECOVERY_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/profile_auth_recovery.log");
const REQUEST_TIMELINE_LOG: &[u8] =
    include_bytes!("../fixtures/runtime_doctor/request_timeline.log");

fn json_object_keys(value: &serde_json::Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("runtime doctor JSON should be an object")
        .keys()
        .cloned()
        .collect()
}

fn runtime_doctor_fixture_summary(log: &[u8]) -> RuntimeDoctorSummary {
    let mut summary = summarize_runtime_log_tail(log);
    summary.pointer_exists = true;
    summary.log_exists = true;
    summary.log_path = Some(PathBuf::from("/tmp/prodex-runtime-fixture.log"));
    diagnosis::runtime_doctor_finalize_summary(&mut summary);
    summary
}

fn runtime_doctor_fixture_fields(summary: &RuntimeDoctorSummary) -> BTreeMap<String, String> {
    runtime_doctor_fields_for_summary(summary, Path::new("/tmp/prodex-runtime-latest.path"))
        .into_iter()
        .collect()
}

fn runtime_doctor_json_string<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} should be a JSON string"))
}

fn runtime_doctor_json_incident<'a>(
    value: &'a serde_json::Value,
    id: &str,
) -> &'a serde_json::Value {
    value["incident_explainer"]
        .as_array()
        .expect("incident_explainer should be an array")
        .iter()
        .find(|incident| incident["id"] == id)
        .unwrap_or_else(|| panic!("missing incident {id}: {value:#?}"))
}

#[test]
fn runtime_doctor_fixture_lane_pressure_surfaces_doctor_json_and_fields() {
    let summary = runtime_doctor_fixture_summary(LANE_PRESSURE_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(summary.line_count, 2);
    assert_eq!(
        value["marker_counts"]["runtime_proxy_lane_limit_reached"],
        2
    );
    assert_eq!(
        value["marker_context_summary"][0]["marker"],
        "runtime_proxy_lane_limit_reached"
    );
    assert_eq!(value["marker_context_summary"][0]["total"], 2);
    assert_eq!(value["marker_context_summary"][0]["lanes"]["compact"], 2);
    assert_eq!(
        value["marker_last_fields"]["runtime_proxy_lane_limit_reached"]["lane"],
        "compact"
    );
    assert_eq!(
        value["marker_last_fields"]["runtime_proxy_lane_limit_reached"]["active"],
        "7"
    );
    assert_eq!(value["facet_counts"]["lane"]["compact"], 2);
    assert_eq!(value["failure_class_counts"]["admission"], 2);
    assert!(
        runtime_doctor_json_string(&value, "diagnosis")
            .contains("per-lane admission limit was triggered on compact")
    );
    assert_eq!(value["incident_explainer"][0]["id"], "lane_saturation");
    assert!(
        value["incident_explainer"][0]["cause"]
            .as_str()
            .expect("incident cause should be a string")
            .contains("compact lane")
    );
    assert!(
        fields
            .get("Marker context hotspots")
            .expect("marker context hotspots should be rendered")
            .contains(
                "runtime_proxy_lane_limit_reached total=2 route:/responses/compact=2 lane:compact=2"
            )
    );
    assert!(
        fields
            .get("Incident 1")
            .expect("incident explainer should be rendered")
            .contains("runtime_proxy_lane_limit_reached=2")
    );
    assert!(
        fields
            .get("Lane next step")
            .expect("lane next step should be rendered")
            .contains("trim bursty compact traffic")
    );
    assert_eq!(
        fields
            .get("Failure classes")
            .expect("failure classes should be rendered"),
        "admission=2"
    );
}

#[test]
fn runtime_doctor_incident_explainer_classifies_pressure_fixture_logs() {
    let summary = runtime_doctor_fixture_summary(ACTIVE_REQUEST_PRESSURE_LOG);
    let value = runtime_doctor_json_value(&summary);
    let active = runtime_doctor_json_incident(&value, "active_request_pressure");
    assert!(
        active["evidence"]
            .as_array()
            .expect("active evidence should be an array")
            .iter()
            .any(|evidence| evidence == "runtime_proxy_active_limit_reached=2")
    );
    assert!(
        active["next_action"]
            .as_str()
            .expect("active next action should be a string")
            .contains("in-flight requests to drain")
    );

    let summary = runtime_doctor_fixture_summary(PROFILE_INFLIGHT_SATURATION_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);
    let inflight = runtime_doctor_json_incident(&value, "profile_inflight_saturation");
    assert_eq!(inflight["markers"][0], "profile_inflight_saturated");
    assert!(
        inflight["cause"]
            .as_str()
            .expect("inflight cause should be a string")
            .contains("Profile main")
    );
    assert!(
        fields
            .get("Incident 1")
            .expect("incident explainer should be rendered")
            .contains("hard_limit=8")
    );
}

#[test]
fn runtime_doctor_incident_explainer_classifies_transport_and_quota_fixture_logs() {
    let summary = runtime_doctor_fixture_summary(ROUTE_SCOPED_PROFILE_HEALTH_LOG);
    let value = runtime_doctor_json_value(&summary);
    let transport = runtime_doctor_json_incident(&value, "transport_backoff");
    assert!(
        transport["next_action"]
            .as_str()
            .expect("transport next action should be a string")
            .contains("alpha/responses")
    );

    let summary = runtime_doctor_fixture_summary(REQUEST_TIMELINE_LOG);
    let value = runtime_doctor_json_value(&summary);
    let quota = runtime_doctor_json_incident(&value, "quota_pressure");
    assert!(
        quota["cause"]
            .as_str()
            .expect("quota cause should be a string")
            .contains("Quota guard")
    );
    assert!(
        quota["evidence"]
            .as_array()
            .expect("quota evidence should be an array")
            .iter()
            .any(|evidence| evidence == "quota_critical_floor_before_send=1")
    );
}

#[test]
fn runtime_doctor_incident_explainer_classifies_precommit_budget() {
    let mut summary = runtime_doctor_fixture_summary(
        b"[2026-04-24 12:00:00.000 +07:00] request=80 precommit_budget_exhausted route=responses attempts=3 reason=candidate_exhausted\n[2026-04-24 12:00:01.000 +07:00] request=81 compact_precommit_budget_exhausted route=/responses/compact attempts=2 reason=quota\n",
    );
    diagnosis::runtime_doctor_finalize_summary(&mut summary);
    let value = runtime_doctor_json_value(&summary);
    let precommit = runtime_doctor_json_incident(&value, "precommit_budget_exhausted");
    assert!(
        precommit["cause"]
            .as_str()
            .expect("precommit cause should be a string")
            .contains("before any upstream response was committed")
    );
    assert!(
        precommit["next_action"]
            .as_str()
            .expect("precommit next action should be a string")
            .contains("retrying compact")
    );
}

#[test]
fn runtime_doctor_json_with_policy_suggestions_exposes_machine_readable_fields() {
    let summary = runtime_doctor_fixture_summary(LANE_PRESSURE_LOG);
    let value = runtime_doctor_json_value_with_policy_suggestions(
        &summary,
        RuntimeDoctorTuningSnapshot::default(),
    );

    assert_eq!(value["policy_suggestion_count"], 1);
    assert_eq!(value["policy_suggestions"][0]["id"], "lane_pressure");
    assert_eq!(
        value["policy_suggestions"][0]["settings"][0]["section"],
        "runtime_proxy"
    );
    assert_eq!(
        value["policy_suggestions"][0]["settings"][0]["key"],
        "compact_active_limit"
    );
    assert!(
        value["policy_suggestions"][0]["snippet"]
            .as_str()
            .expect("snippet should be a string")
            .contains("[runtime_proxy]")
    );
}

#[test]
fn runtime_doctor_fixture_route_health_stays_profile_route_scoped() {
    let summary = runtime_doctor_fixture_summary(ROUTE_SCOPED_PROFILE_HEALTH_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(value["marker_counts"]["profile_health"], 1);
    assert_eq!(value["marker_counts"]["stream_read_error"], 1);
    assert_eq!(
        value["marker_last_fields"]["profile_health"]["profile"],
        "alpha"
    );
    assert_eq!(
        value["marker_last_fields"]["profile_health"]["route"],
        "responses"
    );
    assert_eq!(value["marker_last_fields"]["profile_health"]["score"], "43");
    assert_eq!(value["route_profile_events"].as_array().unwrap().len(), 2);
    assert_eq!(value["route_health"][0]["profile"], "alpha");
    assert_eq!(value["route_health"][0]["route"], "responses");
    assert_eq!(value["route_health"][0]["event_count"], 2);
    assert_eq!(value["route_health"][0]["health_score"], 43);
    assert_eq!(
        value["route_health"][0]["health_reason"],
        "stream_read_error"
    );
    assert_eq!(value["transport_pressure"], "elevated");
    assert!(runtime_doctor_json_string(&value, "diagnosis").contains("alpha/responses"));
    assert!(
        fields
            .get("Health next step")
            .expect("health next step should be rendered")
            .contains("for alpha/responses")
    );
    assert!(
        fields
            .get("Route health focus")
            .expect("route health focus should be rendered")
            .contains("alpha/responses events=2 health=43")
    );
}

#[test]
fn runtime_doctor_fixture_persistence_backpressure_surfaces_queue_details() {
    let summary = runtime_doctor_fixture_summary(PERSISTENCE_BACKPRESSURE_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(value["marker_counts"]["state_save_queue_backpressure"], 1);
    assert_eq!(
        value["marker_counts"]["continuation_journal_queue_backpressure"],
        1
    );
    assert_eq!(value["state_save_queue_backlog"], 12);
    assert_eq!(value["continuation_journal_save_backlog"], 9);
    assert_eq!(value["continuation_journal_save_lag_ms"], 215);
    assert_eq!(value["persistence_pressure"], "elevated");
    assert_eq!(value["failure_class_counts"]["persistence"], 2);
    assert!(
        runtime_doctor_json_string(&value, "diagnosis")
            .contains("background persistence queue backpressure")
    );
    assert!(
        fields
            .get("Persistence next step")
            .expect("persistence next step should be rendered")
            .contains("Latest backlog: state=12 journal=9")
    );
    assert_eq!(
        fields
            .get("State pressure reason")
            .expect("state pressure reason should be rendered"),
        "channel_full"
    );
}

#[test]
fn runtime_doctor_fixture_previous_response_fail_closed_surfaces_guard() {
    let summary = runtime_doctor_fixture_summary(PREVIOUS_RESPONSE_FAIL_CLOSED_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(value["marker_counts"]["previous_response_not_found"], 1);
    assert_eq!(
        value["marker_counts"]["previous_response_fresh_fallback_blocked"],
        1
    );
    assert_eq!(
        value["previous_response_not_found_by_route"]["responses"],
        1
    );
    assert_eq!(value["previous_response_not_found_by_transport"]["http"], 1);
    assert_eq!(
        value["marker_last_fields"]["previous_response_fresh_fallback_blocked"]["request_shape"],
        "continuation_only"
    );
    assert_eq!(
        value["facet_counts"]["request_shape"]["continuation_only"],
        1
    );
    assert!(
        runtime_doctor_json_string(&value, "diagnosis")
            .contains("context-dependent previous_response_id continuation failed closed")
    );
    assert_eq!(
        fields
            .get("Fail-closed shapes")
            .expect("fail-closed shapes should be rendered"),
        "continuation_only=1"
    );
    assert!(
        fields
            .get("Continuation next step")
            .expect("continuation next step should be rendered")
            .contains("cannot be replayed safely")
    );
}

#[test]
fn runtime_doctor_fixture_websocket_connect_overflow_surfaces_guidance() {
    let summary = runtime_doctor_fixture_summary(WEBSOCKET_CONNECT_OVERFLOW_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(
        value["marker_counts"]["websocket_connect_overflow_enqueue"],
        1
    );
    assert_eq!(
        value["marker_counts"]["websocket_connect_overflow_dispatch"],
        1
    );
    assert_eq!(
        value["marker_counts"]["websocket_connect_overflow_reject"],
        1
    );
    assert_eq!(
        value["marker_counts"]["websocket_connect_overflow_rejected"],
        1
    );
    assert_eq!(value["transport_pressure"], "elevated");
    assert_eq!(value["failure_class_counts"]["admission"], 2);
    assert_eq!(value["failure_class_counts"]["transport"], 2);
    assert!(
        runtime_doctor_json_string(&value, "diagnosis")
            .contains("websocket connect work was rejected")
    );
    assert_eq!(
        fields
            .get("WS overflow pending")
            .expect("overflow pending should be rendered"),
        "3/3"
    );
    assert!(
        fields
            .get("WS overflow next step")
            .expect("overflow next step should be rendered")
            .contains("Reduce concurrent websocket session starts")
    );
}

#[test]
fn runtime_doctor_fixture_profile_auth_recovery_surfaces_guidance() {
    let summary = runtime_doctor_fixture_summary(PROFILE_AUTH_RECOVERY_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(value["marker_counts"]["profile_auth_recovered"], 1);
    assert_eq!(value["marker_counts"]["profile_auth_recovery_failed"], 1);
    assert_eq!(value["failure_class_counts"]["auth"], 1);
    assert!(
        runtime_doctor_json_string(&value, "diagnosis").contains("profile auth recovery failed")
    );
    assert_eq!(
        fields
            .get("Auth recovery profile")
            .expect("auth profile should be rendered"),
        "second"
    );
    assert_eq!(
        fields
            .get("Auth recovery error")
            .expect("auth error should be rendered"),
        "refresh_failed"
    );
    assert!(
        fields
            .get("Auth recovery next step")
            .expect("auth next step should be rendered")
            .contains("prodex login --profile second")
    );
}

#[test]
fn runtime_doctor_fixture_request_timeline_surfaces_latest_request() {
    let summary = runtime_doctor_fixture_summary(REQUEST_TIMELINE_LOG);
    let fields = runtime_doctor_fixture_fields(&summary);
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(summary.latest_request_id.as_deref(), Some("42"));
    assert_eq!(summary.latest_request_timeline.len(), 4);
    assert_eq!(value["latest_request_id"], "42");
    assert_eq!(value["selection_summary"]["picked"], 1);
    assert_eq!(value["selection_summary"]["kept"], 1);
    assert_eq!(value["selection_summary"]["blocked"], 1);
    assert_eq!(value["selection_summary"]["selected_profiles"]["alpha"], 1);
    assert_eq!(value["selection_summary"]["rejected_profiles"]["beta"], 1);
    assert_eq!(
        value["selection_summary"]["rejection_reasons"]["quota_critical_floor_before_send"],
        1
    );
    assert_eq!(value["latest_request_timeline"][0]["phase"], "selection");
    assert_eq!(
        value["latest_request_timeline"][0]["marker"],
        "selection_keep_affinity"
    );
    assert_eq!(value["latest_request_timeline"][1]["phase"], "pre_send");
    assert_eq!(
        value["latest_request_timeline"][2]["marker"],
        "upstream_connect_http"
    );
    assert_eq!(
        value["latest_request_timeline"][2]["timestamp"],
        "2026-04-24T04:00:01.100Z"
    );
    assert_eq!(value["latest_request_timeline"][3]["phase"], "fail");
    assert!(
        value["latest_request_timeline"][2]["detail"]
            .as_str()
            .expect("timeline detail should be a string")
            .contains("transport=http"),
        "timeline should merge JSON message fields with structured fields: {value:#?}"
    );
    assert_eq!(
        fields
            .get("Latest request timeline")
            .expect("latest request timeline should be rendered"),
        "request=42 selection:selection_keep_affinity profile=beta route=responses affinity=previous_response_id -> pre_send:responses_pre_send_skip profile=beta route=responses reason=quota_critical_floor_before_send -> upstream:upstream_connect_http profile=beta route=responses transport=http status=429 -> fail:previous_response_fresh_fallback_blocked profile=beta route=responses reason=previous_response_not_found outcome=blocked_nonreplayable_without_affinity request_shape=continuation_only"
    );
    assert_eq!(
        fields
            .get("Selection decisions")
            .expect("selection decisions should be rendered"),
        "picked=1 kept=1 skipped=0 blocked=1"
    );
}

#[test]
fn runtime_doctor_json_value_keeps_stable_top_level_shape() {
    let mut summary = RuntimeDoctorSummary {
            log_path: Some(PathBuf::from("/tmp/prodex-runtime.log")),
            pointer_exists: true,
            log_exists: true,
            line_count: 7,
            diagnosis: "Runtime broker registry broker-a points to dead pid 123 at 127.0.0.1:1234; run `prodex cleanup` or restart `prodex run` so a fresh broker registry is written."
                .to_string(),
            runtime_broker_identities: vec![
                "broker_key=broker-a pid=123 listen_addr=127.0.0.1:1234 status=dead_pid mismatch=none version=0.1.0 path=/opt/prodex sha256=abc123 source=registry stale_leases=2"
                    .to_string(),
            ],
            ..RuntimeDoctorSummary::default()
        };
    summary
        .marker_counts
        .insert("runtime_proxy_queue_overloaded", 2);
    summary.marker_counts.insert("profile_circuit_open", 1);
    summary.marker_last_fields.insert(
        "runtime_proxy_queue_overloaded",
        BTreeMap::from([
            ("lane".to_string(), "responses".to_string()),
            ("request".to_string(), "9".to_string()),
        ]),
    );
    summary.failure_class_counts =
        BTreeMap::from([("admission".to_string(), 2), ("transport".to_string(), 1)]);

    let value = runtime_doctor_json_value(&summary);
    let keys = json_object_keys(&value);
    let expected_keys = [
        "log_path",
        "pointer_exists",
        "log_exists",
        "line_count",
        "first_timestamp",
        "last_timestamp",
        "compat_warning_count",
        "top_client_family",
        "top_client",
        "top_tool_surface",
        "top_compat_warning",
        "marker_counts",
        "marker_context_summary",
        "marker_last_fields",
        "facet_counts",
        "previous_response_not_found_by_route",
        "previous_response_not_found_by_transport",
        "chain_retried_owner_by_reason",
        "chain_dead_upstream_confirmed_by_reason",
        "stale_continuation_by_reason",
        "latest_chain_event",
        "latest_stale_continuation_reason",
        "latest_request_id",
        "latest_request_timeline",
        "selection_summary",
        "route_profile_events",
        "route_health",
        "last_marker_line",
        "selection_pressure",
        "transport_pressure",
        "persistence_pressure",
        "quota_freshness_pressure",
        "startup_audit_pressure",
        "persisted_retry_backoffs",
        "persisted_transport_backoffs",
        "persisted_route_circuits",
        "persisted_usage_snapshots",
        "persisted_response_bindings",
        "persisted_session_bindings",
        "persisted_turn_state_bindings",
        "persisted_session_id_bindings",
        "persisted_verified_continuations",
        "persisted_warm_continuations",
        "persisted_suspect_continuations",
        "persisted_dead_continuations",
        "persisted_continuation_journal_response_bindings",
        "persisted_continuation_journal_session_bindings",
        "persisted_continuation_journal_turn_state_bindings",
        "persisted_continuation_journal_session_id_bindings",
        "persisted_turn_state_coverage_percent",
        "binding_state",
        "state_save_queue_backlog",
        "state_save_lag_ms",
        "continuation_journal_save_backlog",
        "continuation_journal_save_lag_ms",
        "profile_probe_refresh_backlog",
        "profile_probe_refresh_lag_ms",
        "continuation_journal_saved_at",
        "suspect_continuation_bindings",
        "stale_persisted_usage_snapshots",
        "recovered_state_file",
        "recovered_continuations_file",
        "recovered_continuation_journal_file",
        "recovered_scores_file",
        "recovered_usage_snapshots_file",
        "recovered_backoffs_file",
        "last_good_backups_present",
        "degraded_routes",
        "orphan_managed_dirs",
        "prodex_binary_identities",
        "runtime_broker_identities",
        "runtime_broker_artifacts",
        "prodex_binary_mismatch",
        "runtime_broker_mismatch",
        "failure_class_counts",
        "incident_explainer",
        "profiles",
        "diagnosis",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    assert_eq!(keys, expected_keys);
    assert_eq!(value["log_path"], "/tmp/prodex-runtime.log");
    assert_eq!(value["pointer_exists"], true);
    assert_eq!(value["log_exists"], true);
    assert_eq!(value["line_count"], 7);
    assert_eq!(value["marker_counts"]["runtime_proxy_queue_overloaded"], 2);
    assert_eq!(value["marker_counts"]["profile_circuit_open"], 1);
    assert_eq!(
        value["marker_last_fields"]["runtime_proxy_queue_overloaded"]["lane"],
        "responses"
    );
    assert_eq!(
        value["runtime_broker_artifacts"][0]["broker_key"],
        "broker-a"
    );
    assert_eq!(value["runtime_broker_artifacts"][0]["status"], "dead_pid");
    assert_eq!(value["runtime_broker_artifacts"][0]["stale_leases"], 2);
    assert!(
        value["diagnosis"]
            .as_str()
            .expect("diagnosis should be a string")
            .contains("dead pid 123")
    );
}

#[test]
fn runtime_doctor_json_value_keeps_profile_and_route_shape() {
    let summary = RuntimeDoctorSummary {
        profiles: vec![RuntimeDoctorProfileSummary {
            profile: "alpha".to_string(),
            quota_freshness: "fresh".to_string(),
            quota_age_seconds: 5,
            retry_backoff_until: Some(11),
            transport_backoff_until: Some(13),
            routes: vec![RuntimeDoctorRouteSummary {
                route: "responses".to_string(),
                circuit_state: "closed".to_string(),
                circuit_until: Some(17),
                transport_backoff_until: Some(19),
                health_score: 21,
                bad_pairing_score: 23,
                performance_score: 25,
                quota_band: "healthy".to_string(),
                five_hour_status: "ok".to_string(),
                weekly_status: "ok".to_string(),
            }],
        }],
        ..RuntimeDoctorSummary::default()
    };

    let value = runtime_doctor_json_value(&summary);
    let profile = &value["profiles"][0];
    let route = &profile["routes"][0];

    assert_eq!(
        json_object_keys(profile),
        BTreeSet::from([
            "profile".to_string(),
            "quota_freshness".to_string(),
            "quota_age_seconds".to_string(),
            "retry_backoff_until".to_string(),
            "transport_backoff_until".to_string(),
            "routes".to_string(),
        ])
    );
    assert_eq!(
        json_object_keys(route),
        BTreeSet::from([
            "route".to_string(),
            "circuit_state".to_string(),
            "circuit_until".to_string(),
            "transport_backoff_until".to_string(),
            "health_score".to_string(),
            "bad_pairing_score".to_string(),
            "performance_score".to_string(),
            "quota_band".to_string(),
            "five_hour_status".to_string(),
            "weekly_status".to_string(),
        ])
    );
    assert_eq!(profile["profile"], "alpha");
    assert_eq!(route["route"], "responses");
    assert_eq!(route["health_score"], 21);
}
