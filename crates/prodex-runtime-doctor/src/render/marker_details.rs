use terminal_ui::FieldRowsBuilder;

use crate::{RuntimeDoctorSummary, diagnosis};

mod continuation;
mod persistence;
mod pressure;
mod provider;
mod selection;

fn runtime_doctor_format_option<T: ToString>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn marker_field(summary: &RuntimeDoctorSummary, marker: &str, field: &str) -> String {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .cloned()
        .unwrap_or_else(|| "-".to_string())
}

pub(super) fn runtime_doctor_push_marker_detail_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    match marker {
        "runtime_proxy_active_limit_reached" => pressure::active_limit(fields, summary),
        "runtime_proxy_lane_limit_reached" => pressure::lane_limit(fields, summary),
        "profile_inflight_saturated" => pressure::profile_inflight(fields, summary),
        "runtime_proxy_overload_backoff" => pressure::overload_backoff(fields, summary),
        marker @ ("websocket_connect_overflow_rejected"
        | "websocket_connect_overflow_reject"
        | "websocket_connect_overflow_enqueue"
        | "websocket_connect_overflow_dispatch") => {
            pressure::websocket_overflow(fields, summary, marker);
        }
        "profile_health" => pressure::profile_health(fields, summary),
        "local_rewrite_provider_model_fallback" => provider::model_fallback(fields, summary),
        "local_rewrite_provider_auth_failure" => provider::auth_failure(fields, summary),
        marker
        @ ("local_rewrite_gemini_quota_rotate" | "local_rewrite_gemini_rate_limit_retry") => {
            provider::gemini_retry(fields, summary, marker);
        }
        marker @ ("local_rewrite_gemini_invalid_stream_retry"
        | "local_rewrite_gemini_invalid_stream_model_fallback") => {
            provider::gemini_stream_retry(fields, summary, marker);
        }
        marker @ ("local_rewrite_gemini_compact_semantic"
        | "local_rewrite_gemini_compact_fallback") => {
            provider::gemini_compact(fields, summary, marker);
        }
        marker @ ("local_rewrite_gemini_live_error"
        | "local_rewrite_gemini_live_sidecar_error"
        | "local_rewrite_gemini_live_sidecar_session_error") => {
            provider::gemini_live_error(fields, summary, marker);
        }
        marker @ ("profile_auth_recovery_failed" | "profile_auth_recovered") => {
            provider::auth_recovery(fields, summary, marker);
        }
        "previous_response_not_found" => continuation::previous_not_found(fields, summary),
        "previous_response_fresh_fallback" => {
            continuation::previous_fresh_fallback(fields, summary);
        }
        "previous_response_fresh_fallback_blocked" => {
            continuation::previous_fallback_blocked(fields, summary);
        }
        "stale_continuation" => continuation::stale(fields, summary),
        "compact_final_failure" => continuation::compact_final_failure(fields, summary),
        "local_writer_error" => persistence::local_writer_error(fields, summary),
        "state_save_queue_backpressure" => persistence::state_backpressure(fields, summary),
        "continuation_journal_queue_backpressure" => {
            persistence::journal_backpressure(fields, summary);
        }
        "profile_probe_refresh_backpressure" => persistence::probe_backpressure(fields, summary),
        "runtime_proxy_startup_audit" => persistence::startup_audit(fields, summary),
        "selection_skip_sync_probe" => selection::skip_sync_probe(fields, summary),
        "selection_plan" => selection::plan(fields, summary),
        "compat_request_surface" => selection::compat_request_surface(fields, summary),
        _ => {}
    }
}

#[cfg(test)]
#[path = "marker_details/golden.rs"]
mod golden;
