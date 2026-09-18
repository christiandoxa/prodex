from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView

comptime RUNTIME_DOCTOR_MARKER_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_MAX_BYTES: Int64 = 256

def runtime_doctor_marker_known(view: ProdexRichStringView) -> Bool:
    if rich_view_matches_literal["chain_retried_owner"](view, False):
        return True
    if rich_view_matches_literal["chain_dead_upstream_confirmed"](view, False):
        return True
    if rich_view_matches_literal["stale_continuation"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_queue_overloaded"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_active_limit_reached"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_lane_limit_reached"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_overload_backoff"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_admission_wait_started"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_admission_wait_exhausted"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_admission_recovered"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_queue_wait_started"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_queue_wait_exhausted"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_queue_recovered"](view, False):
        return True
    if rich_view_matches_literal["profile_inflight_saturated"](view, False):
        return True
    if rich_view_matches_literal["profile_inflight"](view, False):
        return True
    if rich_view_matches_literal["upstream_connect_timeout"](view, False):
        return True
    if rich_view_matches_literal["upstream_connect_dns_error"](view, False):
        return True
    if rich_view_matches_literal["upstream_tls_handshake_error"](view, False):
        return True
    if rich_view_matches_literal["upstream_connect_error"](view, False):
        return True
    if rich_view_matches_literal["upstream_connect_http"](view, False):
        return True
    if rich_view_matches_literal["upstream_close_before_completed"](view, False):
        return True
    if rich_view_matches_literal["upstream_connection_closed"](view, False):
        return True
    if rich_view_matches_literal["upstream_overload_passthrough"](view, False):
        return True
    if rich_view_matches_literal["upstream_overloaded"](view, False):
        return True
    if rich_view_matches_literal["upstream_read_error"](view, False):
        return True
    if rich_view_matches_literal["upstream_send_error"](view, False):
        return True
    if rich_view_matches_literal["upstream_stream_error"](view, False):
        return True
    if rich_view_matches_literal["precommit_budget_exhausted"](view, False):
        return True
    if rich_view_matches_literal["profile_retry_backoff"](view, False):
        return True
    if rich_view_matches_literal["profile_transport_backoff"](view, False):
        return True
    if rich_view_matches_literal["profile_transport_failure"](view, False):
        return True
    if rich_view_matches_literal["profile_circuit_open"](view, False):
        return True
    if rich_view_matches_literal["profile_circuit_half_open_probe"](view, False):
        return True
    if rich_view_matches_literal["profile_health"](view, False):
        return True
    if rich_view_matches_literal["profile_latency"](view, False):
        return True
    if rich_view_matches_literal["profile_bad_pairing"](view, False):
        return True
    if rich_view_matches_literal["profile_quota_quarantine"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_backoff"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_backoff_cleared"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_proactive_sync"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_proactive_sync_failed"](view, False):
        return True
    if rich_view_matches_literal["previous_response_not_found"](view, False):
        return True
    if rich_view_matches_literal["previous_response_negative_cache"](view, False):
        return True
    if rich_view_matches_literal["previous_response_fresh_fallback"](view, False):
        return True
    if rich_view_matches_literal["previous_response_fresh_fallback_blocked"](view, False):
        return True
    if rich_view_matches_literal["previous_response_binding_cleared"](view, False):
        return True
    if rich_view_matches_literal["previous_response_owner"](view, False):
        return True
    if rich_view_matches_literal["previous_response_release_affinity"](view, False):
        return True
    if rich_view_matches_literal["previous_response_release_deferred"](view, False):
        return True
    if rich_view_matches_literal["previous_response_turn_state_rehydrated"](view, False):
        return True
    if rich_view_matches_literal["compact_committed_owner"](view, False):
        return True
    if rich_view_matches_literal["compact_followup_owner"](view, False):
        return True
    if rich_view_matches_literal["compact_fresh_fallback_blocked"](view, False):
        return True
    if rich_view_matches_literal["compact_pressure_shed"](view, False):
        return True
    if rich_view_matches_literal["compact_lineage_released"](view, False):
        return True
    if rich_view_matches_literal["compact_committed"](view, False):
        return True
    if rich_view_matches_literal["compact_precommit_budget_exhausted"](view, False):
        return True
    if rich_view_matches_literal["compact_candidate_exhausted"](view, False):
        return True
    if rich_view_matches_literal["compact_retryable_failure"](view, False):
        return True
    if rich_view_matches_literal["compact_transport_failure"](view, False):
        return True
    if rich_view_matches_literal["compact_overload_conservative_retry"](view, False):
        return True
    if rich_view_matches_literal["compact_quota_unclassified"](view, False):
        return True
    if rich_view_matches_literal["compact_pre_send_allow_quota_exhausted"](view, False):
        return True
    if rich_view_matches_literal["compact_final_failure"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_committed"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_committed_owner"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_followup_owner"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_fresh_fallback_blocked"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_pressure_shed"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_lineage_released"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_precommit_budget_exhausted"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_candidate_exhausted"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_retryable_failure"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_overload_conservative_retry"](view, False):
        return True
    if rich_view_matches_literal["compact_exit_quota_unclassified"](view, False):
        return True
    if rich_view_matches_literal["selection_keep_affinity"](view, False):
        return True
    if rich_view_matches_literal["selection_keep_current"](view, False):
        return True
    if rich_view_matches_literal["selection_plan"](view, False):
        return True
    if rich_view_matches_literal["selection_pick"](view, False):
        return True
    if rich_view_matches_literal["selection_skip_current"](view, False):
        return True
    if rich_view_matches_literal["selection_skip_affinity"](view, False):
        return True
    if rich_view_matches_literal["local_selection_blocked"](view, False):
        return True
    if rich_view_matches_literal["responses_pre_send_skip"](view, False):
        return True
    if rich_view_matches_literal["websocket_pre_send_skip"](view, False):
        return True
    if rich_view_matches_literal["quota_release_profile_affinity"](view, False):
        return True
    if rich_view_matches_literal["quota_release_affinity"](view, False):
        return True
    if rich_view_matches_literal["quota_blocked"](view, False):
        return True
    if rich_view_matches_literal["quota_critical_floor_before_send"](view, False):
        return True
    if rich_view_matches_literal["upstream_usage_limit_passthrough"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_upstream_start"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_upstream_response"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_request_detail"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_web_search_options_fallback"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_provider_model_fallback"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_provider_auth_failure"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_builtin_tool_fallback"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_quota_rotate"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_rate_limit_retry"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_invalid_stream_retry"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_invalid_stream_model_fallback"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_quota_status_ready"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_quota_status_unavailable"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_compact_semantic"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_compact_fallback"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_synthetic_thought_signature"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_sidecar_started"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_sidecar_error"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_sidecar_accept_error"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_connected"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_error"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_sidecar_connected"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_sidecar_session_error"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_frame"](view, False):
        return True
    if rich_view_matches_literal["local_rewrite_gemini_live_duplex_pump"](view, False):
        return True
    if rich_view_matches_literal["compat_request_surface"](view, False):
        return True
    if rich_view_matches_literal["compat_warning"](view, False):
        return True
    if rich_view_matches_literal["smart_context_autopilot"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_sync_probe_pressure_pause"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_skip_quota_exhausted"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_watchdog"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_watchdog_timeout"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_locked_affinity_owner_fresh_retry"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_nonreplayable_fresh_retry"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_owner_fresh_retry"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_previous_response_blocked"](view, False):
        return True
    if rich_view_matches_literal["websocket_reuse_stale_previous_response_blocked"](view, False):
        return True
    if rich_view_matches_literal["websocket_precommit_frame_timeout"](view, False):
        return True
    if rich_view_matches_literal["websocket_precommit_hold_timeout"](view, False):
        return True
    if rich_view_matches_literal["websocket_dns_resolve_timeout"](view, False):
        return True
    if rich_view_matches_literal["websocket_dns_overflow_enqueue"](view, False):
        return True
    if rich_view_matches_literal["websocket_dns_overflow_dispatch"](view, False):
        return True
    if rich_view_matches_literal["websocket_dns_overflow_reject"](view, False):
        return True
    if rich_view_matches_literal["websocket_connect_local_pressure"](view, False):
        return True
    if rich_view_matches_literal["websocket_connect_overflow_enqueue"](view, False):
        return True
    if rich_view_matches_literal["websocket_connect_overflow_dispatch"](view, False):
        return True
    if rich_view_matches_literal["websocket_connect_overflow_reject"](view, False):
        return True
    if rich_view_matches_literal["websocket_connect_overflow_rejected"](view, False):
        return True
    if rich_view_matches_literal["websocket_proxy_connect_start"](view, False):
        return True
    if rich_view_matches_literal["websocket_proxy_tunnel_ok"](view, False):
        return True
    if rich_view_matches_literal["websocket_proxy_tunnel_failure"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_recovered"](view, False):
        return True
    if rich_view_matches_literal["profile_auth_recovery_failed"](view, False):
        return True
    if rich_view_matches_literal["stream_read_error"](view, False):
        return True
    if rich_view_matches_literal["token_usage"](view, False):
        return True
    if rich_view_matches_literal["local_writer_error"](view, False):
        return True
    if rich_view_matches_literal["first_upstream_chunk"](view, False):
        return True
    if rich_view_matches_literal["first_local_chunk"](view, False):
        return True
    if rich_view_matches_literal["state_save_ok"](view, False):
        return True
    if rich_view_matches_literal["state_save_skipped"](view, False):
        return True
    if rich_view_matches_literal["state_save_error"](view, False):
        return True
    if rich_view_matches_literal["state_save_queued"](view, False):
        return True
    if rich_view_matches_literal["state_save_queue_backpressure"](view, False):
        return True
    if rich_view_matches_literal["continuation_journal_save_ok"](view, False):
        return True
    if rich_view_matches_literal["continuation_journal_save_error"](view, False):
        return True
    if rich_view_matches_literal["continuation_journal_save_queued"](view, False):
        return True
    if rich_view_matches_literal["continuation_journal_queue_backpressure"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_restore_counts"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_startup_audit"](view, False):
        return True
    if rich_view_matches_literal["runtime_proxy_upstream_proxy_mode"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_queued"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_start"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_ok"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_error"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_backpressure"](view, False):
        return True
    if rich_view_matches_literal["profile_probe_refresh_panic"](view, False):
        return True
    if rich_view_matches_literal["selection_skip_sync_probe"](view, False):
        return True
    if rich_view_matches_literal["quota_blocked_affinity_released"](view, False):
        return True
    return False

@export("prodex_mojo_runtime_doctor_marker_known_v1")
def prodex_mojo_runtime_doctor_marker_known_v1(
    abi_version: Int64, marker_address: UInt, known_address: UInt
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MARKER_ABI_VERSION or marker_address == 0 or known_address == 0:
        return 1
    var marker = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(marker_address)
    )[].copy()
    if not rich_view_valid(marker, RUNTIME_DOCTOR_MARKER_MAX_BYTES):
        return 2
    var known = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(known_address))
    known[] = 1 if runtime_doctor_marker_known(marker) else 0
    return 0
