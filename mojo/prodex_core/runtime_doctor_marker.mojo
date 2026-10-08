from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime RUNTIME_DOCTOR_MARKER_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION: Int64 = 2
comptime RUNTIME_DOCTOR_PROFILE_PROVIDER_KIND_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_MESSAGE_PARSE_ABI_VERSION: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_MAX_BYTES: Int64 = 256

comptime PROFILE_PROVIDER_OPENAI: Int64 = 0
comptime PROFILE_PROVIDER_GEMINI: Int64 = 1
comptime PROFILE_PROVIDER_EXTERNAL: Int64 = 2
comptime PROFILE_PROVIDER_COPILOT: Int64 = 3

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

comptime RUNTIME_DOCTOR_SMART_CONTEXT_DECISION_ABI_VERSION: Int64 = 1

def runtime_doctor_smart_context_decision_is_fallback(view: ProdexRichStringView) -> Bool:
    if rich_view_matches_literal["rewritten"](view, False):
        return False
    if rich_view_matches_literal["pass_through"](view, False):
        return False
    return True

comptime RUNTIME_DOCTOR_LOG_VALUE_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS: Int64 = 1
comptime RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK: Int64 = 2
comptime RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION: Int64 = 3


def runtime_doctor_log_value_is_ignored(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return True
    return rich_view_matches_literal["-"](view, False)


@export("prodex_mojo_runtime_doctor_log_value_is_ignored_v1")
def prodex_mojo_runtime_doctor_log_value_is_ignored_v1(
    abi_version: Int64, value_address: UInt, ignored_address: UInt
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_DOCTOR_LOG_VALUE_ABI_VERSION
        or value_address == 0
        or ignored_address == 0
    ):
        return 1
    var value = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(value_address)
    )[].copy()
    if not rich_view_valid(value, 0x7FFFFFFFFFFFFFFF):
        return 2
    var ignored = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ignored_address)
    )
    ignored[] = 1 if runtime_doctor_log_value_is_ignored(value) else 0
    return 0


def runtime_doctor_smart_context_fallback_reason_source(
    decision: ProdexRichStringView, has_self_check: Bool, reason_count: Int64
) -> Int64:
    if reason_count > 0:
        return RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS
    if has_self_check and rich_view_matches_literal["self_check_passthrough"](
        decision, False
    ):
        return RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK
    return RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION


@export("prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1")
def prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1(
    abi_version: Int64,
    decision_address: UInt,
    has_self_check: Int64,
    reason_count: Int64,
    source_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_ABI_VERSION
        or decision_address == 0
        or source_address == 0
        or (has_self_check != 0 and has_self_check != 1)
        or reason_count < 0
    ):
        return 1
    var decision = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(decision_address)
    )[].copy()
    if not rich_view_valid(decision, 0x7FFFFFFFFFFFFFFF):
        return 2
    var source = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(source_address)
    )
    source[] = runtime_doctor_smart_context_fallback_reason_source(
        decision, has_self_check == 1, reason_count
    )
    return 0


@export("prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1")
def prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1(
    abi_version: Int64, decision_address: UInt, fallback_address: UInt
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_SMART_CONTEXT_DECISION_ABI_VERSION or decision_address == 0 or fallback_address == 0:
        return 1
    var decision = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(decision_address)
    )[].copy()
    if not rich_view_valid(decision, 0x7FFFFFFFFFFFFFFF):
        return 2
    var fallback = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(fallback_address))
    fallback[] = 1 if runtime_doctor_smart_context_decision_is_fallback(decision) else 0
    return 0

@export("prodex_mojo_runtime_doctor_marker_known_v1")
def prodex_mojo_runtime_doctor_marker_known_v1(
    abi_version: Int64, marker_address: UInt, known_address: UInt
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MARKER_ABI_VERSION or marker_address == 0 or known_address == 0:
        return 1
    var marker = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(marker_address)
    )[].copy()
    if not rich_view_valid(marker, 0x7FFFFFFFFFFFFFFF):
        return 2
    var known = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(known_address))
    # A parser event can be a long unknown token. The catalog bound limits
    # matching work, not valid input length; unknown text must not crash doctor.
    known[] = 1 if marker.len <= UInt(RUNTIME_DOCTOR_MARKER_MAX_BYTES) and runtime_doctor_marker_known(marker) else 0
    return 0


@export("prodex_mojo_runtime_doctor_profile_provider_kind_v1")
def prodex_mojo_runtime_doctor_profile_provider_kind_v1(
    abi_version: Int64, provider_address: UInt, kind_address: UInt
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_DOCTOR_PROFILE_PROVIDER_KIND_ABI_VERSION
        or provider_address == 0
        or kind_address == 0
    ):
        return 1
    var provider = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(provider_address))[].copy()
    if not rich_view_valid(provider, 0x7FFFFFFFFFFFFFFF):
        return 2
    var kind = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(kind_address)
    )
    if rich_view_matches_literal["openai"](provider, False):
        kind[] = PROFILE_PROVIDER_OPENAI
    elif rich_view_matches_literal["gemini"](provider, False):
        kind[] = PROFILE_PROVIDER_GEMINI
    elif rich_view_matches_literal["copilot"](provider, False):
        kind[] = PROFILE_PROVIDER_COPILOT
    else:
        kind[] = PROFILE_PROVIDER_EXTERNAL
    return 0

comptime RUNTIME_DOCTOR_MARKER_PHASE_NONE: Int64 = 0
comptime RUNTIME_DOCTOR_MARKER_PHASE_SELECTION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_PHASE_PRE_SEND: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_PHASE_UPSTREAM: Int64 = 3
comptime RUNTIME_DOCTOR_MARKER_PHASE_COMMIT: Int64 = 4
comptime RUNTIME_DOCTOR_MARKER_PHASE_FAIL: Int64 = 5

comptime RUNTIME_DOCTOR_MARKER_SELECTION_NONE: Int64 = 0
comptime RUNTIME_DOCTOR_MARKER_SELECTION_PICKED: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_SELECTION_KEPT: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_SELECTION_SKIPPED: Int64 = 3
comptime RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED: Int64 = 4

comptime RUNTIME_DOCTOR_MARKER_ROUTE_NONE: Int64 = 0
comptime RUNTIME_DOCTOR_MARKER_ROUTE_SELECTED: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_ROUTE_SELECTION_SKIP: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_ROUTE_BLOCKED: Int64 = 3
comptime RUNTIME_DOCTOR_MARKER_ROUTE_HEALTH: Int64 = 4
comptime RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_HEALTH: Int64 = 5
comptime RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_FAILURE: Int64 = 6
comptime RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA: Int64 = 7

comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_NONE: Int64 = 0
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_ADMISSION: Int64 = 1
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_AUTH: Int64 = 2
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_CONTINUATION: Int64 = 3
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_PERSISTENCE: Int64 = 4
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_QUOTA: Int64 = 5
comptime RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_TRANSPORT: Int64 = 6

def runtime_doctor_marker_timeline_phase(view: ProdexRichStringView) -> Int64:
    if (
        rich_view_matches_literal["selection_keep_affinity"](view, False)
        or rich_view_matches_literal["selection_keep_current"](view, False)
        or rich_view_matches_literal["selection_plan"](view, False)
        or rich_view_matches_literal["selection_pick"](view, False)
        or rich_view_matches_literal["selection_skip_current"](view, False)
        or rich_view_matches_literal["selection_skip_affinity"](view, False)
        or rich_view_matches_literal["selection_skip_sync_probe"](view, False)
        or rich_view_matches_literal["local_selection_blocked"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_PHASE_SELECTION
    if (
        rich_view_matches_literal["responses_pre_send_skip"](view, False)
        or rich_view_matches_literal["websocket_pre_send_skip"](view, False)
        or rich_view_matches_literal["quota_critical_floor_before_send"](view, False)
        or rich_view_matches_literal["compact_pre_send_allow_quota_exhausted"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_PHASE_PRE_SEND
    if (
        rich_view_matches_literal["upstream_connect_timeout"](view, False)
        or rich_view_matches_literal["upstream_connect_dns_error"](view, False)
        or rich_view_matches_literal["upstream_tls_handshake_error"](view, False)
        or rich_view_matches_literal["upstream_connect_error"](view, False)
        or rich_view_matches_literal["upstream_connect_http"](view, False)
        or rich_view_matches_literal["upstream_overload_passthrough"](view, False)
        or rich_view_matches_literal["upstream_overloaded"](view, False)
        or rich_view_matches_literal["upstream_read_error"](view, False)
        or rich_view_matches_literal["upstream_send_error"](view, False)
        or rich_view_matches_literal["upstream_stream_error"](view, False)
        or rich_view_matches_literal["upstream_close_before_completed"](view, False)
        or rich_view_matches_literal["upstream_connection_closed"](view, False)
        or rich_view_matches_literal["upstream_usage_limit_passthrough"](view, False)
        or rich_view_matches_literal["first_upstream_chunk"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_PHASE_UPSTREAM
    if (
        rich_view_matches_literal["first_local_chunk"](view, False)
        or rich_view_matches_literal["previous_response_owner"](view, False)
        or rich_view_matches_literal["compact_committed"](view, False)
        or rich_view_matches_literal["compact_committed_owner"](view, False)
        or rich_view_matches_literal["compact_followup_owner"](view, False)
        or rich_view_matches_literal["compact_exit_committed"](view, False)
        or rich_view_matches_literal["compact_exit_committed_owner"](view, False)
        or rich_view_matches_literal["compact_exit_followup_owner"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_PHASE_COMMIT
    if (
        rich_view_matches_literal["runtime_proxy_queue_overloaded"](view, False)
        or rich_view_matches_literal["runtime_proxy_active_limit_reached"](view, False)
        or rich_view_matches_literal["runtime_proxy_lane_limit_reached"](view, False)
        or rich_view_matches_literal["runtime_proxy_overload_backoff"](view, False)
        or rich_view_matches_literal["runtime_proxy_admission_wait_exhausted"](view, False)
        or rich_view_matches_literal["runtime_proxy_queue_wait_exhausted"](view, False)
        or rich_view_matches_literal["profile_inflight_saturated"](view, False)
        or rich_view_matches_literal["precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["profile_retry_backoff"](view, False)
        or rich_view_matches_literal["profile_transport_backoff"](view, False)
        or rich_view_matches_literal["profile_transport_failure"](view, False)
        or rich_view_matches_literal["profile_circuit_open"](view, False)
        or rich_view_matches_literal["profile_bad_pairing"](view, False)
        or rich_view_matches_literal["profile_auth_recovery_failed"](view, False)
        or rich_view_matches_literal["previous_response_not_found"](view, False)
        or rich_view_matches_literal["previous_response_negative_cache"](view, False)
        or rich_view_matches_literal["previous_response_fresh_fallback_blocked"](view, False)
        or rich_view_matches_literal["compact_fresh_fallback_blocked"](view, False)
        or rich_view_matches_literal["compact_pressure_shed"](view, False)
        or rich_view_matches_literal["compact_precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["compact_candidate_exhausted"](view, False)
        or rich_view_matches_literal["compact_retryable_failure"](view, False)
        or rich_view_matches_literal["compact_transport_failure"](view, False)
        or rich_view_matches_literal["compact_final_failure"](view, False)
        or rich_view_matches_literal["compact_exit_fresh_fallback_blocked"](view, False)
        or rich_view_matches_literal["compact_exit_pressure_shed"](view, False)
        or rich_view_matches_literal["compact_exit_precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["compact_exit_candidate_exhausted"](view, False)
        or rich_view_matches_literal["compact_exit_retryable_failure"](view, False)
        or rich_view_matches_literal["websocket_precommit_frame_timeout"](view, False)
        or rich_view_matches_literal["websocket_precommit_hold_timeout"](view, False)
        or rich_view_matches_literal["websocket_dns_resolve_timeout"](view, False)
        or rich_view_matches_literal["websocket_dns_overflow_reject"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_reject"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_rejected"](view, False)
        or rich_view_matches_literal["stream_read_error"](view, False)
        or rich_view_matches_literal["local_writer_error"](view, False)
        or rich_view_matches_literal["chain_dead_upstream_confirmed"](view, False)
        or rich_view_matches_literal["stale_continuation"](view, False)
        or rich_view_matches_literal["quota_blocked"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_PHASE_FAIL
    return RUNTIME_DOCTOR_MARKER_PHASE_NONE

def runtime_doctor_marker_selection_bucket(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["selection_pick"](view, False):
        return RUNTIME_DOCTOR_MARKER_SELECTION_PICKED
    if (
        rich_view_matches_literal["selection_keep_affinity"](view, False)
        or rich_view_matches_literal["selection_keep_current"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_SELECTION_KEPT
    if (
        rich_view_matches_literal["selection_skip_current"](view, False)
        or rich_view_matches_literal["selection_skip_affinity"](view, False)
        or rich_view_matches_literal["selection_skip_sync_probe"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_SELECTION_SKIPPED
    if (
        rich_view_matches_literal["local_selection_blocked"](view, False)
        or rich_view_matches_literal["responses_pre_send_skip"](view, False)
        or rich_view_matches_literal["websocket_pre_send_skip"](view, False)
        or rich_view_matches_literal["quota_critical_floor_before_send"](view, False)
        or rich_view_matches_literal["precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["compact_precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["compact_candidate_exhausted"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED
    return RUNTIME_DOCTOR_MARKER_SELECTION_NONE

def runtime_doctor_marker_route_action(view: ProdexRichStringView) -> Int64:
    if (
        rich_view_matches_literal["selection_keep_affinity"](view, False)
        or rich_view_matches_literal["selection_keep_current"](view, False)
        or rich_view_matches_literal["selection_pick"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_SELECTED
    if (
        rich_view_matches_literal["selection_skip_current"](view, False)
        or rich_view_matches_literal["selection_skip_affinity"](view, False)
        or rich_view_matches_literal["selection_skip_sync_probe"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_SELECTION_SKIP
    if (
        rich_view_matches_literal["local_selection_blocked"](view, False)
        or rich_view_matches_literal["responses_pre_send_skip"](view, False)
        or rich_view_matches_literal["websocket_pre_send_skip"](view, False)
        or rich_view_matches_literal["quota_critical_floor_before_send"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_BLOCKED
    if (
        rich_view_matches_literal["profile_health"](view, False)
        or rich_view_matches_literal["profile_latency"](view, False)
        or rich_view_matches_literal["profile_bad_pairing"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_HEALTH
    if (
        rich_view_matches_literal["profile_transport_backoff"](view, False)
        or rich_view_matches_literal["profile_transport_failure"](view, False)
        or rich_view_matches_literal["profile_circuit_open"](view, False)
        or rich_view_matches_literal["profile_circuit_half_open_probe"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_HEALTH
    if (
        rich_view_matches_literal["stream_read_error"](view, False)
        or rich_view_matches_literal["upstream_connect_timeout"](view, False)
        or rich_view_matches_literal["upstream_connect_dns_error"](view, False)
        or rich_view_matches_literal["upstream_tls_handshake_error"](view, False)
        or rich_view_matches_literal["upstream_connect_error"](view, False)
        or rich_view_matches_literal["upstream_connect_http"](view, False)
        or rich_view_matches_literal["upstream_close_before_completed"](view, False)
        or rich_view_matches_literal["upstream_connection_closed"](view, False)
        or rich_view_matches_literal["upstream_read_error"](view, False)
        or rich_view_matches_literal["upstream_send_error"](view, False)
        or rich_view_matches_literal["upstream_stream_error"](view, False)
        or rich_view_matches_literal["compact_transport_failure"](view, False)
        or rich_view_matches_literal["websocket_precommit_frame_timeout"](view, False)
        or rich_view_matches_literal["websocket_precommit_hold_timeout"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_FAILURE
    if (
        rich_view_matches_literal["quota_blocked"](view, False)
        or rich_view_matches_literal["profile_retry_backoff"](view, False)
        or rich_view_matches_literal["profile_quota_quarantine"](view, False)
        or rich_view_matches_literal["compact_retryable_failure"](view, False)
        or rich_view_matches_literal["compact_quota_unclassified"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA
    return RUNTIME_DOCTOR_MARKER_ROUTE_NONE

def runtime_doctor_marker_failure_class(view: ProdexRichStringView) -> Int64:
    if (
        rich_view_matches_literal["runtime_proxy_queue_overloaded"](view, False)
        or rich_view_matches_literal["runtime_proxy_active_limit_reached"](view, False)
        or rich_view_matches_literal["runtime_proxy_lane_limit_reached"](view, False)
        or rich_view_matches_literal["runtime_proxy_overload_backoff"](view, False)
        or rich_view_matches_literal["runtime_proxy_admission_wait_started"](view, False)
        or rich_view_matches_literal["runtime_proxy_admission_wait_exhausted"](view, False)
        or rich_view_matches_literal["runtime_proxy_queue_wait_started"](view, False)
        or rich_view_matches_literal["runtime_proxy_queue_wait_exhausted"](view, False)
        or rich_view_matches_literal["profile_inflight_saturated"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_reject"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_rejected"](view, False)
        or rich_view_matches_literal["compact_precommit_budget_exhausted"](view, False)
        or rich_view_matches_literal["compact_candidate_exhausted"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_ADMISSION
    if (
        rich_view_matches_literal["profile_auth_recovery_failed"](view, False)
        or rich_view_matches_literal["profile_auth_proactive_sync_failed"](view, False)
        or rich_view_matches_literal["local_rewrite_provider_auth_failure"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_AUTH
    if (
        rich_view_matches_literal["previous_response_not_found"](view, False)
        or rich_view_matches_literal["previous_response_negative_cache"](view, False)
        or rich_view_matches_literal["previous_response_fresh_fallback"](view, False)
        or rich_view_matches_literal["chain_retried_owner"](view, False)
        or rich_view_matches_literal["chain_dead_upstream_confirmed"](view, False)
        or rich_view_matches_literal["stale_continuation"](view, False)
        or rich_view_matches_literal["previous_response_fresh_fallback_blocked"](view, False)
        or rich_view_matches_literal["compact_fresh_fallback_blocked"](view, False)
        or rich_view_matches_literal["compact_pressure_shed"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_CONTINUATION
    if (
        rich_view_matches_literal["state_save_error"](view, False)
        or rich_view_matches_literal["state_save_queue_backpressure"](view, False)
        or rich_view_matches_literal["state_save_skipped"](view, False)
        or rich_view_matches_literal["continuation_journal_save_error"](view, False)
        or rich_view_matches_literal["continuation_journal_queue_backpressure"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_PERSISTENCE
    if (
        rich_view_matches_literal["profile_retry_backoff"](view, False)
        or rich_view_matches_literal["profile_transport_backoff"](view, False)
        or rich_view_matches_literal["profile_circuit_open"](view, False)
        or rich_view_matches_literal["profile_circuit_half_open_probe"](view, False)
        or rich_view_matches_literal["profile_health"](view, False)
        or rich_view_matches_literal["profile_latency"](view, False)
        or rich_view_matches_literal["profile_bad_pairing"](view, False)
        or rich_view_matches_literal["profile_quota_quarantine"](view, False)
        or rich_view_matches_literal["profile_auth_backoff"](view, False)
        or rich_view_matches_literal["profile_probe_refresh_error"](view, False)
        or rich_view_matches_literal["profile_probe_refresh_panic"](view, False)
        or rich_view_matches_literal["profile_probe_refresh_backpressure"](view, False)
        or rich_view_matches_literal["selection_skip_sync_probe"](view, False)
        or rich_view_matches_literal["runtime_proxy_sync_probe_pressure_pause"](view, False)
        or rich_view_matches_literal["local_selection_blocked"](view, False)
        or rich_view_matches_literal["responses_pre_send_skip"](view, False)
        or rich_view_matches_literal["websocket_pre_send_skip"](view, False)
        or rich_view_matches_literal["quota_blocked"](view, False)
        or rich_view_matches_literal["quota_critical_floor_before_send"](view, False)
        or rich_view_matches_literal["upstream_usage_limit_passthrough"](view, False)
        or rich_view_matches_literal["upstream_overload_passthrough"](view, False)
        or rich_view_matches_literal["upstream_overloaded"](view, False)
        or rich_view_matches_literal["compact_retryable_failure"](view, False)
        or rich_view_matches_literal["compact_overload_conservative_retry"](view, False)
        or rich_view_matches_literal["compact_quota_unclassified"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_quota_rotate"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_rate_limit_retry"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_quota_status_unavailable"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_QUOTA
    if (
        rich_view_matches_literal["upstream_connect_timeout"](view, False)
        or rich_view_matches_literal["upstream_connect_dns_error"](view, False)
        or rich_view_matches_literal["upstream_tls_handshake_error"](view, False)
        or rich_view_matches_literal["upstream_connect_error"](view, False)
        or rich_view_matches_literal["upstream_connect_http"](view, False)
        or rich_view_matches_literal["upstream_close_before_completed"](view, False)
        or rich_view_matches_literal["upstream_connection_closed"](view, False)
        or rich_view_matches_literal["upstream_read_error"](view, False)
        or rich_view_matches_literal["upstream_send_error"](view, False)
        or rich_view_matches_literal["upstream_stream_error"](view, False)
        or rich_view_matches_literal["profile_transport_failure"](view, False)
        or rich_view_matches_literal["stream_read_error"](view, False)
        or rich_view_matches_literal["local_writer_error"](view, False)
        or rich_view_matches_literal["websocket_precommit_frame_timeout"](view, False)
        or rich_view_matches_literal["websocket_precommit_hold_timeout"](view, False)
        or rich_view_matches_literal["websocket_dns_resolve_timeout"](view, False)
        or rich_view_matches_literal["websocket_dns_overflow_enqueue"](view, False)
        or rich_view_matches_literal["websocket_dns_overflow_dispatch"](view, False)
        or rich_view_matches_literal["websocket_dns_overflow_reject"](view, False)
        or rich_view_matches_literal["websocket_connect_local_pressure"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_enqueue"](view, False)
        or rich_view_matches_literal["websocket_connect_overflow_dispatch"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_invalid_stream_retry"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_invalid_stream_model_fallback"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_live_error"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_live_sidecar_error"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_live_sidecar_accept_error"](view, False)
        or rich_view_matches_literal["local_rewrite_gemini_live_sidecar_session_error"](view, False)
    ):
        return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_TRANSPORT
    return RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_NONE

@export("prodex_mojo_runtime_doctor_marker_semantics_v2")
def prodex_mojo_runtime_doctor_marker_semantics_v2(
    abi_version: Int64,
    marker_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION or marker_address == 0 or output_address == 0:
        return 1
    var marker = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(marker_address)
    )[].copy()
    if not rich_view_valid(marker, 0x7FFFFFFFFFFFFFFF):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if marker.len > UInt(RUNTIME_DOCTOR_MARKER_MAX_BYTES):
        output[unsafe_offset=0] = RUNTIME_DOCTOR_MARKER_PHASE_NONE
        output[unsafe_offset=1] = RUNTIME_DOCTOR_MARKER_SELECTION_NONE
        output[unsafe_offset=2] = RUNTIME_DOCTOR_MARKER_ROUTE_NONE
        output[unsafe_offset=3] = RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_NONE
        return 0
    output[unsafe_offset=0] = runtime_doctor_marker_timeline_phase(marker)
    output[unsafe_offset=1] = runtime_doctor_marker_selection_bucket(marker)
    output[unsafe_offset=2] = runtime_doctor_marker_route_action(marker)
    output[unsafe_offset=3] = runtime_doctor_marker_failure_class(marker)
    return 0

comptime RUNTIME_DOCTOR_MESSAGE_PARSE_MAX_BYTES: Int64 = 4_194_304

def runtime_doctor_message_is_space(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32

def runtime_doctor_message_skip_space(
    view: ProdexRichStringView, index: Int64
) -> Int64:
    var next = index
    var ptr = rich_view_ptr(view)
    while next < Int64(view.len):
        if not runtime_doctor_message_is_space(ptr[unsafe_offset=next]):
            break
        next += 1
    return next

def runtime_doctor_message_skip_key_or_token(
    view: ProdexRichStringView, index: Int64
) -> Int64:
    var next = index
    var ptr = rich_view_ptr(view)
    while next < Int64(view.len):
        var value = ptr[unsafe_offset=next]
        if runtime_doctor_message_is_space(value) or value == 61:
            break
        next += 1
    return next

def runtime_doctor_message_skip_token(
    view: ProdexRichStringView, index: Int64
) -> Int64:
    var next = index
    var ptr = rich_view_ptr(view)
    while next < Int64(view.len):
        if runtime_doctor_message_is_space(ptr[unsafe_offset=next]):
            break
        next += 1
    return next

def runtime_doctor_message_skip_value(
    view: ProdexRichStringView, index: Int64
) -> Int64:
    var next = index
    var length = Int64(view.len)
    if next >= length:
        return next
    var ptr = rich_view_ptr(view)
    if ptr[unsafe_offset=next] == 34:
        next += 1
        var escaped = False
        while next < length:
            var value = ptr[unsafe_offset=next]
            if escaped:
                escaped = False
                next += 1
                continue
            if value == 92:
                escaped = True
                next += 1
                continue
            if value == 34:
                next += 1
                break
            next += 1
        return next
    while next < length:
        if runtime_doctor_message_is_space(ptr[unsafe_offset=next]):
            break
        next += 1
    return next

@export("prodex_mojo_runtime_doctor_parse_message_v2")
def prodex_mojo_runtime_doctor_parse_message_v2(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_MESSAGE_PARSE_ABI_VERSION:
        return 4
    if (
        input_length < 0
        or input_length > RUNTIME_DOCTOR_MESSAGE_PARSE_MAX_BYTES
        or output_capacity < 5
        or output_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, RUNTIME_DOCTOR_MESSAGE_PARSE_MAX_BYTES):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = -1
    output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = -1
    output[unsafe_offset=3] = -1
    output[unsafe_offset=4] = 0
    var index: Int64 = 0
    var field_count: Int64 = 0
    var ptr = rich_view_ptr(view)
    while index < input_length:
        index = runtime_doctor_message_skip_space(view, index)
        if index >= input_length:
            break
        var token_start = index
        index = runtime_doctor_message_skip_key_or_token(view, index)
        if index < input_length and ptr[unsafe_offset=index] == 61:
            var value_start = index + 1
            var value_end = runtime_doctor_message_skip_value(view, value_start)
            var base = 5 + field_count * 4
            if base + 4 > output_capacity:
                return 3
            output[unsafe_offset=base] = token_start
            output[unsafe_offset=base + 1] = index
            output[unsafe_offset=base + 2] = value_start
            output[unsafe_offset=base + 3] = value_end
            field_count += 1
            output[unsafe_offset=4] = field_count
            index = value_end
            continue
        if token_start < index and output[unsafe_offset=0] < 0:
            output[unsafe_offset=0] = token_start
            output[unsafe_offset=1] = index
        index = runtime_doctor_message_skip_token(view, index)
    var marker_bounds = runtime_doctor_message_marker_bounds(view)
    output[unsafe_offset=2] = marker_bounds[0]
    output[unsafe_offset=3] = marker_bounds[1]
    return 0

def runtime_doctor_message_marker_token_byte(value: UInt8) -> Bool:
    return (
        (value >= 48 and value <= 57)
        or (value >= 65 and value <= 90)
        or (value >= 97 and value <= 122)
        or value == 95
    )


def runtime_doctor_message_marker_bounds(
    view: ProdexRichStringView,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    var length = Int64(view.len)
    var ptr = rich_view_ptr(view)

    # Preserve parsed-event precedence: skip leading key=value fields,
    # then prefer the first standalone token when it is a known marker.
    var index: Int64 = 0
    while index < length:
        index = runtime_doctor_message_skip_space(view, index)
        if index >= length:
            break
        var token_start = index
        index = runtime_doctor_message_skip_key_or_token(view, index)
        if index < length and ptr[unsafe_offset=index] == 61:
            index = runtime_doctor_message_skip_value(view, index + 1)
            continue
        if token_start < index:
            var candidate = ProdexRichStringView(
                view.ptr + UInt(token_start), UInt(index - token_start)
            )
            if runtime_doctor_marker_known(candidate):
                result[0] = token_start
                result[1] = index
                return result^
            break
        index = runtime_doctor_message_skip_token(view, index)

    # Match Rust's historical split predicate exactly: fallback marker tokens
    # contain only ASCII alphanumeric bytes and underscore.
    index = 0
    while index < length:
        while index < length and not runtime_doctor_message_marker_token_byte(
            ptr[unsafe_offset=index]
        ):
            index += 1
        var token_start = index
        while index < length and runtime_doctor_message_marker_token_byte(
            ptr[unsafe_offset=index]
        ):
            index += 1
        if token_start < index:
            var candidate = ProdexRichStringView(
                view.ptr + UInt(token_start), UInt(index - token_start)
            )
            if runtime_doctor_marker_known(candidate):
                result[0] = token_start
                result[1] = index
                return result^
    return result^
