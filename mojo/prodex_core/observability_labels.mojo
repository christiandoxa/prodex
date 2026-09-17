from std.memory import Pointer

comptime OBSERVABILITY_LABEL_ABI_VERSION: Int64 = 1
comptime OBSERVABILITY_LABEL_MAX_BYTES: Int64 = 128
comptime OBSERVABILITY_STATUS_OK: Int64 = 0
comptime OBSERVABILITY_STATUS_INVALID: Int64 = 1
comptime OBSERVABILITY_STATUS_CAPACITY: Int64 = 2
comptime OBSERVABILITY_STATUS_ABI: Int64 = 4

def observability_copy_label(label: StringSlice, output: Pointer[mut=True, UInt8, _], capacity: Int64, output_length: Pointer[mut=True, Int64, _]) -> Int64:
    var length = Int64(label.byte_length())
    output_length[] = length
    if length < 0 or length > OBSERVABILITY_LABEL_MAX_BYTES:
        return OBSERVABILITY_STATUS_INVALID
    if capacity < length:
        return OBSERVABILITY_STATUS_CAPACITY
    var source = label.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    return OBSERVABILITY_STATUS_OK

@export("prodex_mojo_observability_label_v1")
def prodex_mojo_observability_label_v1(
    abi_version: Int64,
    kind: Int64,
    value: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_length_address: UInt,
) abi("C") -> Int64:
    if abi_version != OBSERVABILITY_LABEL_ABI_VERSION:
        return OBSERVABILITY_STATUS_ABI
    if output_address == 0 or output_length_address == 0 or output_capacity < 0:
        return OBSERVABILITY_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var output_length = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_length_address))
    var label = StringSlice("")
    if kind == 0:
        if value == 0:
            label = StringSlice("reservation")
        elif value == 1:
            label = StringSlice("commit")
        elif value == 2:
            label = StringSlice("release")
        elif value == 3:
            label = StringSlice("expire")
        elif value == 4:
            label = StringSlice("reconciliation")
        elif value == 5:
            label = StringSlice("budget_rejection")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 1:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("rejected")
        elif value == 2:
            label = StringSlice("committed")
        elif value == 3:
            label = StringSlice("released")
        elif value == 4:
            label = StringSlice("expired")
        elif value == 5:
            label = StringSlice("reconciled")
        elif value == 6:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 2:
        if value == 0:
            label = StringSlice("reserve_append")
        elif value == 1:
            label = StringSlice("commit_append")
        elif value == 2:
            label = StringSlice("release_append")
        elif value == 3:
            label = StringSlice("reconciliation_append")
        elif value == 4:
            label = StringSlice("query")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 3:
        if value == 0:
            label = StringSlice("written")
        elif value == 1:
            label = StringSlice("read")
        elif value == 2:
            label = StringSlice("skipped")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 4:
        if value == 0:
            label = StringSlice("tenant_budget_exceeded")
        elif value == 1:
            label = StringSlice("virtual_key_budget_exceeded")
        elif value == 2:
            label = StringSlice("rate_limited")
        elif value == 3:
            label = StringSlice("reservation_unavailable")
        elif value == 4:
            label = StringSlice("policy_denied")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 5:
        if value == 0:
            label = StringSlice("reservation_overshoot")
        elif value == 1:
            label = StringSlice("duplicate_charge_prevented")
        elif value == 2:
            label = StringSlice("missing_commit_recovered")
        elif value == 3:
            label = StringSlice("missing_release_recovered")
        elif value == 4:
            label = StringSlice("ledger_mismatch_detected")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 6:
        if value == 0:
            label = StringSlice("allowed")
        elif value == 1:
            label = StringSlice("delayed")
        elif value == 2:
            label = StringSlice("rejected")
        elif value == 3:
            label = StringSlice("unavailable")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 7:
        if value == 0:
            label = StringSlice("tenant")
        elif value == 1:
            label = StringSlice("virtual_key")
        elif value == 2:
            label = StringSlice("principal")
        elif value == 3:
            label = StringSlice("provider")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 8:
        if value == 0:
            label = StringSlice("rate_limit_check")
        elif value == 1:
            label = StringSlice("rate_limit_commit")
        elif value == 2:
            label = StringSlice("recovery_lease_acquire")
        elif value == 3:
            label = StringSlice("recovery_lease_release")
        elif value == 4:
            label = StringSlice("cache_read")
        elif value == 5:
            label = StringSlice("cache_write")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 9:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("limited")
        elif value == 2:
            label = StringSlice("lease_unavailable")
        elif value == 3:
            label = StringSlice("cache_miss")
        elif value == 4:
            label = StringSlice("unavailable")
        elif value == 5:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 10:
        if value == 0:
            label = StringSlice("scan_expired")
        elif value == 1:
            label = StringSlice("acquire_lease")
        elif value == 2:
            label = StringSlice("release_budget")
        elif value == 3:
            label = StringSlice("write_ledger")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 11:
        if value == 0:
            label = StringSlice("recovered")
        elif value == 1:
            label = StringSlice("skipped")
        elif value == 2:
            label = StringSlice("lease_unavailable")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 12:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("global_limit_reached")
        elif value == 2:
            label = StringSlice("route_limit_reached")
        elif value == 3:
            label = StringSlice("queue_full")
        elif value == 4:
            label = StringSlice("draining")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 13:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("rejected_too_large")
        elif value == 2:
            label = StringSlice("unknown_length")
        elif value == 3:
            label = StringSlice("truncated")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 14:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("scim")
        elif value == 3:
            label = StringSlice("upload")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 15:
        if value == 0:
            label = StringSlice("client_disconnect")
        elif value == 1:
            label = StringSlice("timeout_budget")
        elif value == 2:
            label = StringSlice("shutdown_drain")
        elif value == 3:
            label = StringSlice("upstream_abort")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 16:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("provider_stream")
        elif value == 3:
            label = StringSlice("persistence")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 17:
        if value == 0:
            label = StringSlice("compatible")
        elif value == 1:
            label = StringSlice("additive_change")
        elif value == 2:
            label = StringSlice("deprecated_change")
        elif value == 3:
            label = StringSlice("breaking_change")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 18:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("scim")
        elif value == 3:
            label = StringSlice("error_envelope")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 19:
        if value == 0:
            label = StringSlice("notice")
        elif value == 1:
            label = StringSlice("sunset")
        elif value == 2:
            label = StringSlice("rejected")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 20:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("scim")
        elif value == 3:
            label = StringSlice("health")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 21:
        if value == 0:
            label = StringSlice("emitted")
        elif value == 1:
            label = StringSlice("redacted")
        elif value == 2:
            label = StringSlice("validation_failed")
        elif value == 3:
            label = StringSlice("compatibility_rejected")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 22:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("scim")
        elif value == 3:
            label = StringSlice("health")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 23:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("replayed")
        elif value == 2:
            label = StringSlice("conflict")
        elif value == 3:
            label = StringSlice("missing")
        elif value == 4:
            label = StringSlice("invalid")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 24:
        if value == 0:
            label = StringSlice("tenant_mutation")
        elif value == 1:
            label = StringSlice("principal_mutation")
        elif value == 2:
            label = StringSlice("virtual_key_mutation")
        elif value == 3:
            label = StringSlice("policy_mutation")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 25:
        if value == 0:
            label = StringSlice("required")
        elif value == 1:
            label = StringSlice("persisted")
        elif value == 2:
            label = StringSlice("missing")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 26:
        if value == 0:
            label = StringSlice("tenant")
        elif value == 1:
            label = StringSlice("principal")
        elif value == 2:
            label = StringSlice("virtual_key")
        elif value == 3:
            label = StringSlice("policy")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 27:
        if value == 0:
            label = StringSlice("page_returned")
        elif value == 1:
            label = StringSlice("empty_page")
        elif value == 2:
            label = StringSlice("invalid_cursor")
        elif value == 3:
            label = StringSlice("expired_cursor")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 28:
        if value == 0:
            label = StringSlice("control_plane")
        elif value == 1:
            label = StringSlice("scim")
        elif value == 2:
            label = StringSlice("audit_export")
        elif value == 3:
            label = StringSlice("quota")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 29:
        if value == 0:
            label = StringSlice("matched")
        elif value == 1:
            label = StringSlice("missing")
        elif value == 2:
            label = StringSlice("mismatched")
        elif value == 3:
            label = StringSlice("invalid")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 30:
        if value == 0:
            label = StringSlice("tenant")
        elif value == 1:
            label = StringSlice("principal")
        elif value == 2:
            label = StringSlice("virtual_key")
        elif value == 3:
            label = StringSlice("policy")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 31:
        if value == 0:
            label = StringSlice("responses")
        elif value == 1:
            label = StringSlice("compact")
        elif value == 2:
            label = StringSlice("websocket")
        elif value == 3:
            label = StringSlice("control_plane")
        elif value == 4:
            label = StringSlice("health")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 32:
        if value == 0:
            label = StringSlice("request")
        elif value == 1:
            label = StringSlice("response")
        elif value == 2:
            label = StringSlice("openapi")
        elif value == 3:
            label = StringSlice("error_envelope")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 33:
        if value == 0:
            label = StringSlice("valid")
        elif value == 1:
            label = StringSlice("invalid")
        elif value == 2:
            label = StringSlice("missing_schema")
        elif value == 3:
            label = StringSlice("incompatible")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 34:
        if value == 0:
            label = StringSlice("generated")
        elif value == 1:
            label = StringSlice("validated")
        elif value == 2:
            label = StringSlice("published")
        elif value == 3:
            label = StringSlice("rejected")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 35:
        if value == 0:
            label = StringSlice("gateway_openapi")
        elif value == 1:
            label = StringSlice("control_plane_openapi")
        elif value == 2:
            label = StringSlice("scim_schema")
        elif value == 3:
            label = StringSlice("error_envelope")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 36:
        if value == 0:
            label = StringSlice("1xx")
        elif value == 1:
            label = StringSlice("2xx")
        elif value == 2:
            label = StringSlice("3xx")
        elif value == 3:
            label = StringSlice("4xx")
        elif value == 4:
            label = StringSlice("5xx")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 37:
        if value == 0:
            label = StringSlice("ready")
        elif value == 1:
            label = StringSlice("paused")
        elif value == 2:
            label = StringSlice("dropped")
        elif value == 3:
            label = StringSlice("closed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 38:
        if value == 0:
            label = StringSlice("data_plane_stream")
        elif value == 1:
            label = StringSlice("provider_stream")
        elif value == 2:
            label = StringSlice("websocket")
        elif value == 3:
            label = StringSlice("audit_export")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 39:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("expired")
        elif value == 2:
            label = StringSlice("exhausted")
        elif value == 3:
            label = StringSlice("cancelled")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 40:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("provider")
        elif value == 3:
            label = StringSlice("persistence")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 41:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("defaulted")
        elif value == 2:
            label = StringSlice("deprecated")
        elif value == 3:
            label = StringSlice("unsupported")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 42:
        if value == 0:
            label = StringSlice("data_plane")
        elif value == 1:
            label = StringSlice("control_plane")
        elif value == 2:
            label = StringSlice("scim")
        elif value == 3:
            label = StringSlice("health")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 43:
        if value == 0:
            label = StringSlice("postgres")
        elif value == 1:
            label = StringSlice("sqlite")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 44:
        if value == 0:
            label = StringSlice("pending_insert")
        elif value == 1:
            label = StringSlice("complete")
        elif value == 2:
            label = StringSlice("lookup")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 45:
        if value == 0:
            label = StringSlice("recorded")
        elif value == 1:
            label = StringSlice("replayed")
        elif value == 2:
            label = StringSlice("conflict")
        elif value == 3:
            label = StringSlice("not_found")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 46:
        if value == 0:
            label = StringSlice("append")
        elif value == 1:
            label = StringSlice("verify_link")
        elif value == 2:
            label = StringSlice("verify_range")
        elif value == 3:
            label = StringSlice("export_proof")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 47:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("conflict")
        elif value == 2:
            label = StringSlice("digest_invalid")
        elif value == 3:
            label = StringSlice("gap_detected")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 48:
        if value == 0:
            label = StringSlice("emit")
        elif value == 1:
            label = StringSlice("persist")
        elif value == 2:
            label = StringSlice("export")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 49:
        if value == 0:
            label = StringSlice("plan_query")
        elif value == 1:
            label = StringSlice("page_query")
        elif value == 2:
            label = StringSlice("plan_export")
        elif value == 3:
            label = StringSlice("serialize_export")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 50:
        if value == 0:
            label = StringSlice("planned")
        elif value == 1:
            label = StringSlice("page_returned")
        elif value == 2:
            label = StringSlice("empty")
        elif value == 3:
            label = StringSlice("denied")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 51:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failure")
        elif value == 2:
            label = StringSlice("dropped")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 52:
        if value == 0:
            label = StringSlice("select_candidates")
        elif value == 1:
            label = StringSlice("apply_legal_hold")
        elif value == 2:
            label = StringSlice("delete_batch")
        elif value == 3:
            label = StringSlice("verify_chain")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 53:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("protected")
        elif value == 2:
            label = StringSlice("empty")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 54:
        if value == 0:
            label = StringSlice("activated")
        elif value == 1:
            label = StringSlice("rejected")
        elif value == 2:
            label = StringSlice("missing_last_known_good")
        elif value == 3:
            label = StringSlice("invalid_revision")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 55:
        if value == 0:
            label = StringSlice("published_revision")
        elif value == 1:
            label = StringSlice("last_known_good")
        elif value == 2:
            label = StringSlice("rollback")
        elif value == 3:
            label = StringSlice("invalidation_fallback")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 56:
        if value == 0:
            label = StringSlice("invalidated")
        elif value == 1:
            label = StringSlice("reload_scheduled")
        elif value == 2:
            label = StringSlice("not_found")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 57:
        if value == 0:
            label = StringSlice("gateway_policy_cache")
        elif value == 1:
            label = StringSlice("runtime_policy_cache")
        elif value == 2:
            label = StringSlice("redis_policy_cache")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 58:
        if value == 0:
            label = StringSlice("tenant")
        elif value == 1:
            label = StringSlice("principal")
        elif value == 2:
            label = StringSlice("request")
        elif value == 3:
            label = StringSlice("call")
        elif value == 4:
            label = StringSlice("reservation")
        elif value == 5:
            label = StringSlice("virtual_key")
        elif value == 6:
            label = StringSlice("policy_revision")
        elif value == 7:
            label = StringSlice("audit_event")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 59:
        if value == 0:
            label = StringSlice("generated")
        elif value == 1:
            label = StringSlice("parsed")
        elif value == 2:
            label = StringSlice("rejected")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 60:
        if value == 0:
            label = StringSlice("fresh")
        elif value == 1:
            label = StringSlice("refresh_now")
        elif value == 2:
            label = StringSlice("stale_while_revalidate")
        elif value == 3:
            label = StringSlice("last_known_good_backoff")
        elif value == 4:
            label = StringSlice("unavailable")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 61:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failure")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 62:
        if value == 0:
            label = StringSlice("discover_issuer")
        elif value == 1:
            label = StringSlice("fetch_jwks")
        elif value == 2:
            label = StringSlice("validate_snapshot")
        elif value == 3:
            label = StringSlice("write_cache")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 63:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("skipped_fresh")
        elif value == 2:
            label = StringSlice("backoff")
        elif value == 3:
            label = StringSlice("invalid_snapshot")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 64:
        if value == 0:
            label = StringSlice("active")
        elif value == 1:
            label = StringSlice("refresh_async")
        elif value == 2:
            label = StringSlice("last_known_good_refresh")
        elif value == 3:
            label = StringSlice("expired")
        elif value == 4:
            label = StringSlice("invalidated")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 65:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failure")
        elif value == 2:
            label = StringSlice("last_known_good_fallback")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 66:
        if value == 0:
            label = StringSlice("activate_last_known_good")
        elif value == 1:
            label = StringSlice("reject_candidate")
        elif value == 2:
            label = StringSlice("rollback")
        elif value == 3:
            label = StringSlice("verify")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 67:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("blocked")
        elif value == 3:
            label = StringSlice("noop")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 68:
        if value == 0:
            label = StringSlice("backup")
        elif value == 1:
            label = StringSlice("restore")
        elif value == 2:
            label = StringSlice("verify")
        elif value == 3:
            label = StringSlice("drill")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 69:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("partial")
        elif value == 3:
            label = StringSlice("skipped")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 70:
        if value == 0:
            label = StringSlice("apply")
        elif value == 1:
            label = StringSlice("verify")
        elif value == 2:
            label = StringSlice("promote")
        elif value == 3:
            label = StringSlice("rollback")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 71:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("degraded")
        elif value == 3:
            label = StringSlice("skipped")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 72:
        if value == 0:
            label = StringSlice("injected")
        elif value == 1:
            label = StringSlice("recovered")
        elif value == 2:
            label = StringSlice("failed")
        elif value == 3:
            label = StringSlice("skipped")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 73:
        if value == 0:
            label = StringSlice("postgres")
        elif value == 1:
            label = StringSlice("redis")
        elif value == 2:
            label = StringSlice("idp")
        elif value == 3:
            label = StringSlice("provider")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 74:
        if value == 0:
            label = StringSlice("live")
        elif value == 1:
            label = StringSlice("ready")
        elif value == 2:
            label = StringSlice("startup")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 75:
        if value == 0:
            label = StringSlice("passing")
        elif value == 1:
            label = StringSlice("degraded")
        elif value == 2:
            label = StringSlice("failing")
        elif value == 3:
            label = StringSlice("draining")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 76:
        if value == 0:
            label = StringSlice("passed")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("aborted")
        elif value == 3:
            label = StringSlice("threshold_breached")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 77:
        if value == 0:
            label = StringSlice("load")
        elif value == 1:
            label = StringSlice("soak")
        elif value == 2:
            label = StringSlice("spike")
        elif value == 3:
            label = StringSlice("recovery")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 78:
        if value == 0:
            label = StringSlice("status_check")
        elif value == 1:
            label = StringSlice("compatibility_check")
        elif value == 2:
            label = StringSlice("apply")
        elif value == 3:
            label = StringSlice("rollback")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 79:
        if value == 0:
            label = StringSlice("compatible")
        elif value == 1:
            label = StringSlice("applied")
        elif value == 2:
            label = StringSlice("blocked")
        elif value == 3:
            label = StringSlice("failed")
        elif value == 4:
            label = StringSlice("rolled_back")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 80:
        if value == 0:
            label = StringSlice("read")
        elif value == 1:
            label = StringSlice("write")
        elif value == 2:
            label = StringSlice("commit")
        elif value == 3:
            label = StringSlice("rollback")
        elif value == 4:
            label = StringSlice("health_check")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 81:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("conflict")
        elif value == 2:
            label = StringSlice("timeout")
        elif value == 3:
            label = StringSlice("unavailable")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 82:
        if value == 0:
            label = StringSlice("file")
        elif value == 1:
            label = StringSlice("keyring")
        elif value == 2:
            label = StringSlice("external_manager")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 83:
        if value == 0:
            label = StringSlice("read")
        elif value == 1:
            label = StringSlice("write")
        elif value == 2:
            label = StringSlice("delete")
        elif value == 3:
            label = StringSlice("revision_lookup")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 84:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("not_found")
        elif value == 2:
            label = StringSlice("unsupported")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 85:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("skipped")
        elif value == 3:
            label = StringSlice("rollback")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 86:
        if value == 0:
            label = StringSlice("provider_credential")
        elif value == 1:
            label = StringSlice("oidc_client")
        elif value == 2:
            label = StringSlice("signing_key")
        elif value == 3:
            label = StringSlice("storage_credential")
        elif value == 4:
            label = StringSlice("webhook_secret")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 87:
        if value == 0:
            label = StringSlice("signal_received")
        elif value == 1:
            label = StringSlice("draining_started")
        elif value == 2:
            label = StringSlice("readiness_disabled")
        elif value == 3:
            label = StringSlice("inflight_drained")
        elif value == 4:
            label = StringSlice("timeout_elapsed")
        elif value == 5:
            label = StringSlice("completed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 88:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("timeout")
        elif value == 2:
            label = StringSlice("forced")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 89:
        if value == 0:
            label = StringSlice("responses_api")
        elif value == 1:
            label = StringSlice("streaming")
        elif value == 2:
            label = StringSlice("tools")
        elif value == 3:
            label = StringSlice("vision")
        elif value == 4:
            label = StringSlice("json_mode")
        elif value == 5:
            label = StringSlice("remote_compact")
        elif value == 6:
            label = StringSlice("websocket")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 90:
        if value == 0:
            label = StringSlice("compatible")
        elif value == 1:
            label = StringSlice("incompatible")
        elif value == 2:
            label = StringSlice("no_candidate")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 91:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("failure")
        elif value == 2:
            label = StringSlice("probe")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 92:
        if value == 0:
            label = StringSlice("warning")
        elif value == 1:
            label = StringSlice("critical")
        elif value == 2:
            label = StringSlice("recovered")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 93:
        if value == 0:
            label = StringSlice("error_rate")
        elif value == 1:
            label = StringSlice("latency")
        elif value == 2:
            label = StringSlice("overload")
        elif value == 3:
            label = StringSlice("transport")
        elif value == 4:
            label = StringSlice("circuit_open")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 94:
        if value == 0:
            label = StringSlice("openai")
        elif value == 1:
            label = StringSlice("anthropic")
        elif value == 2:
            label = StringSlice("gemini")
        elif value == 3:
            label = StringSlice("local")
        elif value == 4:
            label = StringSlice("other")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 95:
        if value == 0:
            label = StringSlice("success")
        elif value == 1:
            label = StringSlice("rate_limited")
        elif value == 2:
            label = StringSlice("overloaded")
        elif value == 3:
            label = StringSlice("provider_error")
        elif value == 4:
            label = StringSlice("transport_error")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 96:
        if value == 0:
            label = StringSlice("before_dispatch")
        elif value == 1:
            label = StringSlice("before_first_byte")
        elif value == 2:
            label = StringSlice("after_first_byte")
        elif value == 3:
            label = StringSlice("after_cancellation")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 97:
        if value == 0:
            label = StringSlice("allowed")
        elif value == 1:
            label = StringSlice("denied_committed")
        elif value == 2:
            label = StringSlice("denied_budget_exhausted")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 98:
        if value == 0:
            label = StringSlice("selected")
        elif value == 1:
            label = StringSlice("fallback")
        elif value == 2:
            label = StringSlice("rejected")
        elif value == 3:
            label = StringSlice("no_candidate")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 99:
        if value == 0:
            label = StringSlice("responses")
        elif value == 1:
            label = StringSlice("compact")
        elif value == 2:
            label = StringSlice("websocket")
        elif value == 3:
            label = StringSlice("control_plane")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 100:
        if value == 0:
            label = StringSlice("completed")
        elif value == 1:
            label = StringSlice("cancelled")
        elif value == 2:
            label = StringSlice("interrupted")
        elif value == 3:
            label = StringSlice("guardrail_blocked")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 101:
        if value == 0:
            label = StringSlice("responses")
        elif value == 1:
            label = StringSlice("websocket")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 102:
        if value == 0:
            label = StringSlice("postgres")
        elif value == 1:
            label = StringSlice("redis")
        elif value == 2:
            label = StringSlice("provider_http")
        elif value == 3:
            label = StringSlice("oidc_http")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 103:
        if value == 0:
            label = StringSlice("responses")
        elif value == 1:
            label = StringSlice("compact")
        elif value == 2:
            label = StringSlice("websocket")
        elif value == 3:
            label = StringSlice("telemetry")
        elif value == 4:
            label = StringSlice("persistence")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 104:
        if value == 0:
            label = StringSlice("queue_full")
        elif value == 1:
            label = StringSlice("exporter_unavailable")
        elif value == 2:
            label = StringSlice("shutdown")
        elif value == 3:
            label = StringSlice("invalid_payload")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 105:
        if value == 0:
            label = StringSlice("accepted")
        elif value == 1:
            label = StringSlice("malformed")
        elif value == 2:
            label = StringSlice("invalid_signature")
        elif value == 3:
            label = StringSlice("expired")
        elif value == 4:
            label = StringSlice("unknown_key")
        elif value == 5:
            label = StringSlice("missing_tenant")
        elif value == 6:
            label = StringSlice("role_denied")
        elif value == 7:
            label = StringSlice("cache_unavailable")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 106:
        if value == 0:
            label = StringSlice("decode")
        elif value == 1:
            label = StringSlice("signature")
        elif value == 2:
            label = StringSlice("claims")
        elif value == 3:
            label = StringSlice("tenant_claim")
        elif value == 4:
            label = StringSlice("role_claim")
        elif value == 5:
            label = StringSlice("jwks_cache")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 107:
        if value == 0:
            label = StringSlice("data_plane_inference")
        elif value == 1:
            label = StringSlice("data_plane_quota")
        elif value == 2:
            label = StringSlice("control_plane_read")
        elif value == 3:
            label = StringSlice("control_plane_mutation")
        elif value == 4:
            label = StringSlice("control_plane_billing")
        elif value == 5:
            label = StringSlice("break_glass")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 108:
        if value == 0:
            label = StringSlice("allowed")
        elif value == 1:
            label = StringSlice("credential_scope_denied")
        elif value == 2:
            label = StringSlice("role_denied")
        elif value == 3:
            label = StringSlice("tenant_denied")
        elif value == 4:
            label = StringSlice("resource_denied")
        elif value == 5:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 109:
        if value == 0:
            label = StringSlice("request")
        elif value == 1:
            label = StringSlice("approve")
        elif value == 2:
            label = StringSlice("activate")
        elif value == 3:
            label = StringSlice("revoke")
        elif value == 4:
            label = StringSlice("expire")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 110:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("expired")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 111:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 112:
        if value == 0:
            label = StringSlice("rejected")
        elif value == 1:
            label = StringSlice("audited")
        elif value == 2:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 113:
        if value == 0:
            label = StringSlice("consistent")
        elif value == 1:
            label = StringSlice("missing_principal")
        elif value == 2:
            label = StringSlice("missing_tenant")
        elif value == 3:
            label = StringSlice("tenant_mismatch")
        elif value == 4:
            label = StringSlice("correlation_missing")
        elif value == 5:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 114:
        if value == 0:
            label = StringSlice("authentication")
        elif value == 1:
            label = StringSlice("authorization")
        elif value == 2:
            label = StringSlice("audit")
        elif value == 3:
            label = StringSlice("control_plane")
        elif value == 4:
            label = StringSlice("data_plane")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 115:
        if value == 0:
            label = StringSlice("full")
        elif value == 1:
            label = StringSlice("partial")
        elif value == 2:
            label = StringSlice("unsupported")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 116:
        if value == 0:
            label = StringSlice("none")
        elif value == 1:
            label = StringSlice("personal_data")
        elif value == 2:
            label = StringSlice("credential")
        elif value == 3:
            label = StringSlice("financial")
        elif value == 4:
            label = StringSlice("multiple")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 117:
        if value == 0:
            label = StringSlice("none")
        elif value == 1:
            label = StringSlice("masked")
        elif value == 2:
            label = StringSlice("denied")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 118:
        if value == 0:
            label = StringSlice("allowed")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("timeout")
        elif value == 3:
            label = StringSlice("error")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 119:
        if value == 0:
            label = StringSlice("local")
        elif value == 1:
            label = StringSlice("external")
        elif value == 2:
            label = StringSlice("merge")
        elif value == 3:
            label = StringSlice("request_enforcement")
        elif value == 4:
            label = StringSlice("response_enforcement")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 120:
        if value == 0:
            label = StringSlice("create")
        elif value == 1:
            label = StringSlice("update")
        elif value == 2:
            label = StringSlice("publish")
        elif value == 3:
            label = StringSlice("invalidate")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 121:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("published")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 122:
        if value == 0:
            label = StringSlice("applied")
        elif value == 1:
            label = StringSlice("missing")
        elif value == 2:
            label = StringSlice("mismatch_rejected")
        elif value == 3:
            label = StringSlice("rls_denied")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 123:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 124:
        if value == 0:
            label = StringSlice("authentication")
        elif value == 1:
            label = StringSlice("tenant_resolution")
        elif value == 2:
            label = StringSlice("authorization")
        elif value == 3:
            label = StringSlice("credential_scope")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 125:
        if value == 0:
            label = StringSlice("allowed")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("error")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 126:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 127:
        if value == 0:
            label = StringSlice("enforced")
        elif value == 1:
            label = StringSlice("cross_tenant_denied")
        elif value == 2:
            label = StringSlice("missing_tenant_denied")
        elif value == 3:
            label = StringSlice("mismatch_rejected")
        elif value == 4:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 128:
        if value == 0:
            label = StringSlice("authentication")
        elif value == 1:
            label = StringSlice("authorization")
        elif value == 2:
            label = StringSlice("storage_predicate")
        elif value == 3:
            label = StringSlice("cache_key")
        elif value == 4:
            label = StringSlice("audit_query")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 129:
        if value == 0:
            label = StringSlice("create")
        elif value == 1:
            label = StringSlice("update")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 130:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 131:
        if value == 0:
            label = StringSlice("invite")
        elif value == 1:
            label = StringSlice("scim_create")
        elif value == 2:
            label = StringSlice("scim_update")
        elif value == 3:
            label = StringSlice("scim_delete")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 132:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 133:
        if value == 0:
            label = StringSlice("create")
        elif value == 1:
            label = StringSlice("rotate_secret")
        elif value == 2:
            label = StringSlice("persist_reference")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 134:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 135:
        if value == 0:
            label = StringSlice("traceparent")
        elif value == 1:
            label = StringSlice("tracestate")
        elif value == 2:
            label = StringSlice("baggage")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 136:
        if value == 0:
            label = StringSlice("propagated")
        elif value == 1:
            label = StringSlice("rejected")
        elif value == 2:
            label = StringSlice("missing")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 137:
        if value == 0:
            label = StringSlice("closed")
        elif value == 1:
            label = StringSlice("open")
        elif value == 2:
            label = StringSlice("half_open_probe")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 138:
        if value == 0:
            label = StringSlice("gateway_cache_refresh")
        elif value == 1:
            label = StringSlice("runtime_policy_reload")
        elif value == 2:
            label = StringSlice("audit_projection")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 139:
        if value == 0:
            label = StringSlice("delivered")
        elif value == 1:
            label = StringSlice("failed")
        elif value == 2:
            label = StringSlice("skipped")
        elif value == 3:
            label = StringSlice("retry_scheduled")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 140:
        if value == 0:
            label = StringSlice("data_plane_to_control_plane")
        elif value == 1:
            label = StringSlice("control_plane_to_data_plane")
        elif value == 2:
            label = StringSlice("break_glass_to_data_plane")
        elif value == 3:
            label = StringSlice("break_glass_to_control_plane")
        elif value == 4:
            label = StringSlice("missing_credential")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 141:
        if value == 0:
            label = StringSlice("set_context")
        elif value == 1:
            label = StringSlice("verify_context")
        elif value == 2:
            label = StringSlice("apply_rls_policy")
        elif value == 3:
            label = StringSlice("execute_tenant_dml")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 142:
        if value == 0:
            label = StringSlice("create")
        elif value == 1:
            label = StringSlice("rotate_secret")
        elif value == 2:
            label = StringSlice("disable")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 143:
        if value == 0:
            label = StringSlice("grant")
        elif value == 1:
            label = StringSlice("revoke")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 144:
        if value == 0:
            label = StringSlice("rotate")
        elif value == 1:
            label = StringSlice("validate_reference")
        elif value == 2:
            label = StringSlice("persist_reference")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 145:
        if value == 0:
            label = StringSlice("authorized")
        elif value == 1:
            label = StringSlice("denied")
        elif value == 2:
            label = StringSlice("persisted")
        elif value == 3:
            label = StringSlice("failed")
        else:
            return OBSERVABILITY_STATUS_INVALID
    elif kind == 146:
        if value == 0:
            label = StringSlice("update")
        elif value == 1:
            label = StringSlice("validate_scope")
        elif value == 2:
            label = StringSlice("persist_policy")
        else:
            return OBSERVABILITY_STATUS_INVALID
    else:
        return OBSERVABILITY_STATUS_INVALID
    return observability_copy_label(label, output, output_capacity, output_length)


@export("prodex_mojo_observability_metric_name_v1")
def prodex_mojo_observability_metric_name_v1(
    abi_version: Int64,
    plan: Int64,
    slot: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_length_address: UInt,
) abi("C") -> Int64:
    if abi_version != OBSERVABILITY_LABEL_ABI_VERSION:
        return OBSERVABILITY_STATUS_ABI
    if output_address == 0 or output_length_address == 0 or output_capacity < 0:
        return OBSERVABILITY_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var output_length = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_length_address))
    var name = StringSlice("")
    if plan == 0 and slot == 0:
        name = StringSlice("prodex_accounting_events_total")
    elif plan == 1 and slot == 0:
        name = StringSlice("prodex_billing_ledger_events_total")
    elif plan == 2 and slot == 0:
        name = StringSlice("prodex_budget_rejections_total")
    elif plan == 3 and slot == 0:
        name = StringSlice("prodex_quota_correctness_events_total")
    elif plan == 4 and slot == 0:
        name = StringSlice("prodex_rate_limit_decisions_total")
    elif plan == 5 and slot == 0:
        name = StringSlice("prodex_redis_coordination_events_total")
    elif plan == 6 and slot == 0:
        name = StringSlice("prodex_reservation_recovery_events_total")
    elif plan == 7 and slot == 0:
        name = StringSlice("prodex_api_admission_decisions_total")
    elif plan == 8 and slot == 0:
        name = StringSlice("prodex_api_body_limit_events_total")
    elif plan == 9 and slot == 0:
        name = StringSlice("prodex_api_cancellation_events_total")
    elif plan == 10 and slot == 0:
        name = StringSlice("prodex_api_compatibility_events_total")
    elif plan == 11 and slot == 0:
        name = StringSlice("prodex_api_deprecation_events_total")
    elif plan == 12 and slot == 0:
        name = StringSlice("prodex_api_error_envelope_events_total")
    elif plan == 13 and slot == 0:
        name = StringSlice("prodex_api_idempotency_events_total")
    elif plan == 14 and slot == 0:
        name = StringSlice("prodex_api_mutation_audit_events_total")
    elif plan == 15 and slot == 0:
        name = StringSlice("prodex_api_pagination_events_total")
    elif plan == 16 and slot == 0:
        name = StringSlice("prodex_api_precondition_events_total")
    elif plan == 17 and slot == 0:
        name = StringSlice("prodex_api_requests_total")
    elif plan == 17 and slot == 1:
        name = StringSlice("prodex_api_request_duration_ms")
    elif plan == 18 and slot == 0:
        name = StringSlice("prodex_api_schema_validation_total")
    elif plan == 19 and slot == 0:
        name = StringSlice("prodex_api_spec_publication_events_total")
    elif plan == 20 and slot == 0:
        name = StringSlice("prodex_api_stream_backpressure_events_total")
    elif plan == 21 and slot == 0:
        name = StringSlice("prodex_api_timeout_budget_events_total")
    elif plan == 22 and slot == 0:
        name = StringSlice("prodex_api_version_negotiation_events_total")
    elif plan == 23 and slot == 0:
        name = StringSlice("prodex_idempotency_record_events_total")
    elif plan == 24 and slot == 0:
        name = StringSlice("prodex_audit_chain_events_total")
    elif plan == 25 and slot == 0:
        name = StringSlice("prodex_audit_events_total")
    elif plan == 26 and slot == 0:
        name = StringSlice("prodex_audit_query_lifecycle_events_total")
    elif plan == 27 and slot == 0:
        name = StringSlice("prodex_audit_retention_purge_events_total")
    elif plan == 28 and slot == 0:
        name = StringSlice("prodex_config_activation_events_total")
    elif plan == 29 and slot == 0:
        name = StringSlice("prodex_config_cache_invalidation_events_total")
    elif plan == 30 and slot == 0:
        name = StringSlice("prodex_config_publication_delivery_total")
    elif plan == 31 and slot == 0:
        name = StringSlice("prodex_enterprise_id_events_total")
    elif plan == 32 and slot == 0:
        name = StringSlice("prodex_jwks_cache_age_ms")
    elif plan == 33 and slot == 0:
        name = StringSlice("prodex_jwks_refresh_total")
    elif plan == 34 and slot == 0:
        name = StringSlice("prodex_oidc_refresh_events_total")
    elif plan == 35 and slot == 0:
        name = StringSlice("prodex_policy_refresh_total")
    elif plan == 36 and slot == 0:
        name = StringSlice("prodex_policy_rollback_events_total")
    elif plan == 37 and slot == 0:
        name = StringSlice("prodex_backup_restore_events_total")
    elif plan == 38 and slot == 0:
        name = StringSlice("prodex_deployment_rollout_events_total")
    elif plan == 39 and slot == 0:
        name = StringSlice("prodex_fault_injection_events_total")
    elif plan == 40 and slot == 0:
        name = StringSlice("prodex_health_probe_results_total")
    elif plan == 41 and slot == 0:
        name = StringSlice("prodex_load_soak_events_total")
    elif plan == 41 and slot == 1:
        name = StringSlice("prodex_load_soak_duration_ms")
    elif plan == 42 and slot == 0:
        name = StringSlice("prodex_migration_lifecycle_events_total")
    elif plan == 43 and slot == 0:
        name = StringSlice("prodex_persistence_operations_total")
    elif plan == 44 and slot == 0:
        name = StringSlice("prodex_secret_provider_operations_total")
    elif plan == 45 and slot == 0:
        name = StringSlice("prodex_secret_rotation_events_total")
    elif plan == 46 and slot == 0:
        name = StringSlice("prodex_shutdown_lifecycle_total")
    elif plan == 47 and slot == 0:
        name = StringSlice("prodex_provider_capability_negotiation_events_total")
    elif plan == 48 and slot == 0:
        name = StringSlice("prodex_provider_circuit_breaker_events_total")
    elif plan == 49 and slot == 0:
        name = StringSlice("prodex_provider_degradation_events_total")
    elif plan == 50 and slot == 0:
        name = StringSlice("prodex_provider_requests_total")
    elif plan == 50 and slot == 1:
        name = StringSlice("prodex_provider_request_duration_ms")
    elif plan == 51 and slot == 0:
        name = StringSlice("prodex_provider_retry_events_total")
    elif plan == 52 and slot == 0:
        name = StringSlice("prodex_routing_decisions_total")
    elif plan == 53 and slot == 0:
        name = StringSlice("prodex_streaming_lifecycle_total")
    elif plan == 53 and slot == 1:
        name = StringSlice("prodex_streaming_lifecycle_duration_ms")
    elif plan == 54 and slot == 0:
        name = StringSlice("prodex_connection_pool_in_use")
    elif plan == 55 and slot == 0:
        name = StringSlice("prodex_telemetry_dropped_total")
    elif plan == 56 and slot == 0:
        name = StringSlice("prodex_queue_depth")
    elif plan == 57 and slot == 0:
        name = StringSlice("prodex_authn_token_validation_events_total")
    elif plan == 58 and slot == 0:
        name = StringSlice("prodex_authz_decisions_total")
    elif plan == 59 and slot == 0:
        name = StringSlice("prodex_break_glass_lifecycle_events_total")
    elif plan == 60 and slot == 0:
        name = StringSlice("prodex_budget_policy_lifecycle_events_total")
    elif plan == 61 and slot == 0:
        name = StringSlice("prodex_credential_scope_mismatch_events_total")
    elif plan == 62 and slot == 0:
        name = StringSlice("prodex_identity_context_events_total")
    elif plan == 63 and slot == 0:
        name = StringSlice("prodex_inspection_events_total")
    elif plan == 63 and slot == 1:
        name = StringSlice("prodex_inspection_duration_microseconds")
    elif plan == 64 and slot == 0:
        name = StringSlice("prodex_policy_lifecycle_events_total")
    elif plan == 65 and slot == 0:
        name = StringSlice("prodex_postgres_tenant_context_events_total")
    elif plan == 66 and slot == 0:
        name = StringSlice("prodex_provider_credential_lifecycle_events_total")
    elif plan == 67 and slot == 0:
        name = StringSlice("prodex_role_binding_lifecycle_events_total")
    elif plan == 68 and slot == 0:
        name = StringSlice("prodex_security_decisions_total")
    elif plan == 69 and slot == 0:
        name = StringSlice("prodex_service_identity_lifecycle_events_total")
    elif plan == 70 and slot == 0:
        name = StringSlice("prodex_tenant_isolation_events_total")
    elif plan == 71 and slot == 0:
        name = StringSlice("prodex_tenant_lifecycle_events_total")
    elif plan == 72 and slot == 0:
        name = StringSlice("prodex_user_lifecycle_events_total")
    elif plan == 73 and slot == 0:
        name = StringSlice("prodex_virtual_key_lifecycle_events_total")
    elif plan == 74 and slot == 0:
        name = StringSlice("prodex_governance_siem_outbox_pending")
    elif plan == 74 and slot == 1:
        name = StringSlice("prodex_governance_siem_outbox_dead_lettered")
    elif plan == 74 and slot == 2:
        name = StringSlice("prodex_governance_siem_outbox_oldest_pending_lag_milliseconds")
    elif plan == 75 and slot == 0:
        name = StringSlice("prodex_trace_propagation_events_total")
    else:
        return OBSERVABILITY_STATUS_INVALID
    return observability_copy_label(name, output, output_capacity, output_length)


@export("prodex_mojo_observability_label_key_v1")
def prodex_mojo_observability_label_key_v1(
    abi_version: Int64,
    key: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_length_address: UInt,
) abi("C") -> Int64:
    if abi_version != OBSERVABILITY_LABEL_ABI_VERSION:
        return OBSERVABILITY_STATUS_ABI
    if output_address == 0 or output_length_address == 0 or output_capacity < 0:
        return OBSERVABILITY_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var output_length = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_length_address))
    var name = StringSlice("")
    if key == 0:
        name = StringSlice("account_lifecycle_operation")
    elif key == 1:
        name = StringSlice("account_lifecycle_result")
    elif key == 2:
        name = StringSlice("accounting_operation")
    elif key == 3:
        name = StringSlice("accounting_result")
    elif key == 4:
        name = StringSlice("api_admission_result")
    elif key == 5:
        name = StringSlice("api_admission_route")
    elif key == 6:
        name = StringSlice("api_body_limit_result")
    elif key == 7:
        name = StringSlice("api_body_limit_surface")
    elif key == 8:
        name = StringSlice("api_cancellation_source")
    elif key == 9:
        name = StringSlice("api_cancellation_surface")
    elif key == 10:
        name = StringSlice("api_compatibility_result")
    elif key == 11:
        name = StringSlice("api_compatibility_surface")
    elif key == 12:
        name = StringSlice("api_deprecation_signal")
    elif key == 13:
        name = StringSlice("api_deprecation_surface")
    elif key == 14:
        name = StringSlice("api_error_envelope_result")
    elif key == 15:
        name = StringSlice("api_error_envelope_surface")
    elif key == 16:
        name = StringSlice("api_idempotency_result")
    elif key == 17:
        name = StringSlice("api_idempotency_surface")
    elif key == 18:
        name = StringSlice("api_mutation_audit_result")
    elif key == 19:
        name = StringSlice("api_mutation_audit_surface")
    elif key == 20:
        name = StringSlice("api_pagination_result")
    elif key == 21:
        name = StringSlice("api_pagination_surface")
    elif key == 22:
        name = StringSlice("api_precondition_result")
    elif key == 23:
        name = StringSlice("api_precondition_surface")
    elif key == 24:
        name = StringSlice("api_route")
    elif key == 25:
        name = StringSlice("api_schema_result")
    elif key == 26:
        name = StringSlice("api_schema_surface")
    elif key == 27:
        name = StringSlice("api_spec_publication_result")
    elif key == 28:
        name = StringSlice("api_spec_surface")
    elif key == 29:
        name = StringSlice("api_stream_backpressure_state")
    elif key == 30:
        name = StringSlice("api_stream_backpressure_surface")
    elif key == 31:
        name = StringSlice("api_timeout_budget_result")
    elif key == 32:
        name = StringSlice("api_timeout_budget_surface")
    elif key == 33:
        name = StringSlice("api_version_result")
    elif key == 34:
        name = StringSlice("api_version_surface")
    elif key == 35:
        name = StringSlice("audit_chain_operation")
    elif key == 36:
        name = StringSlice("audit_chain_result")
    elif key == 37:
        name = StringSlice("audit_operation")
    elif key == 38:
        name = StringSlice("audit_query_operation")
    elif key == 39:
        name = StringSlice("audit_query_result")
    elif key == 40:
        name = StringSlice("audit_result")
    elif key == 41:
        name = StringSlice("audit_retention_operation")
    elif key == 42:
        name = StringSlice("audit_retention_result")
    elif key == 43:
        name = StringSlice("authn_validation_result")
    elif key == 44:
        name = StringSlice("authn_validation_stage")
    elif key == 45:
        name = StringSlice("authz_boundary")
    elif key == 46:
        name = StringSlice("authz_result")
    elif key == 47:
        name = StringSlice("backup_restore_operation")
    elif key == 48:
        name = StringSlice("backup_restore_result")
    elif key == 49:
        name = StringSlice("billing_ledger_operation")
    elif key == 50:
        name = StringSlice("billing_ledger_result")
    elif key == 51:
        name = StringSlice("break_glass_operation")
    elif key == 52:
        name = StringSlice("break_glass_result")
    elif key == 53:
        name = StringSlice("budget_policy_operation")
    elif key == 54:
        name = StringSlice("budget_policy_result")
    elif key == 55:
        name = StringSlice("budget_rejection_reason")
    elif key == 56:
        name = StringSlice("config_activation_result")
    elif key == 57:
        name = StringSlice("config_activation_source")
    elif key == 58:
        name = StringSlice("config_invalidation_result")
    elif key == 59:
        name = StringSlice("config_invalidation_target")
    elif key == 60:
        name = StringSlice("config_publication_result")
    elif key == 61:
        name = StringSlice("config_publication_target")
    elif key == 62:
        name = StringSlice("credential_lifecycle_operation")
    elif key == 63:
        name = StringSlice("credential_lifecycle_result")
    elif key == 64:
        name = StringSlice("credential_scope_direction")
    elif key == 65:
        name = StringSlice("credential_scope_result")
    elif key == 66:
        name = StringSlice("deployment_rollout_operation")
    elif key == 67:
        name = StringSlice("deployment_rollout_result")
    elif key == 68:
        name = StringSlice("enterprise_id_kind")
    elif key == 69:
        name = StringSlice("enterprise_id_result")
    elif key == 70:
        name = StringSlice("fault_injection_result")
    elif key == 71:
        name = StringSlice("fault_injection_target")
    elif key == 72:
        name = StringSlice("health_probe")
    elif key == 73:
        name = StringSlice("health_result")
    elif key == 74:
        name = StringSlice("idempotency_record_backend")
    elif key == 75:
        name = StringSlice("idempotency_record_operation")
    elif key == 76:
        name = StringSlice("idempotency_record_result")
    elif key == 77:
        name = StringSlice("identity_context_result")
    elif key == 78:
        name = StringSlice("identity_context_surface")
    elif key == 79:
        name = StringSlice("inspection_coverage")
    elif key == 80:
        name = StringSlice("inspection_finding_category")
    elif key == 81:
        name = StringSlice("inspection_masking_action")
    elif key == 82:
        name = StringSlice("inspection_outcome")
    elif key == 83:
        name = StringSlice("inspection_stage")
    elif key == 84:
        name = StringSlice("jwks_cache_state")
    elif key == 85:
        name = StringSlice("jwks_refresh_result")
    elif key == 86:
        name = StringSlice("load_soak_result")
    elif key == 87:
        name = StringSlice("load_soak_scenario")
    elif key == 88:
        name = StringSlice("migration_operation")
    elif key == 89:
        name = StringSlice("migration_result")
    elif key == 90:
        name = StringSlice("oidc_refresh_operation")
    elif key == 91:
        name = StringSlice("oidc_refresh_result")
    elif key == 92:
        name = StringSlice("persistence_operation")
    elif key == 93:
        name = StringSlice("persistence_result")
    elif key == 94:
        name = StringSlice("policy_cache_state")
    elif key == 95:
        name = StringSlice("policy_lifecycle_operation")
    elif key == 96:
        name = StringSlice("policy_lifecycle_result")
    elif key == 97:
        name = StringSlice("policy_refresh_result")
    elif key == 98:
        name = StringSlice("policy_rollback_operation")
    elif key == 99:
        name = StringSlice("policy_rollback_result")
    elif key == 100:
        name = StringSlice("pool_kind")
    elif key == 101:
        name = StringSlice("postgres_tenant_context_operation")
    elif key == 102:
        name = StringSlice("postgres_tenant_context_result")
    elif key == 103:
        name = StringSlice("provider")
    elif key == 104:
        name = StringSlice("provider_capability")
    elif key == 105:
        name = StringSlice("provider_capability_result")
    elif key == 106:
        name = StringSlice("provider_circuit_breaker_decision")
    elif key == 107:
        name = StringSlice("provider_circuit_breaker_event")
    elif key == 108:
        name = StringSlice("provider_credential_operation")
    elif key == 109:
        name = StringSlice("provider_credential_result")
    elif key == 110:
        name = StringSlice("provider_degradation_severity")
    elif key == 111:
        name = StringSlice("provider_degradation_signal")
    elif key == 112:
        name = StringSlice("provider_result")
    elif key == 113:
        name = StringSlice("provider_retry_outcome")
    elif key == 114:
        name = StringSlice("provider_retry_stage")
    elif key == 115:
        name = StringSlice("queue_kind")
    elif key == 116:
        name = StringSlice("quota_correctness_event")
    elif key == 117:
        name = StringSlice("rate_limit_decision")
    elif key == 118:
        name = StringSlice("rate_limit_scope")
    elif key == 119:
        name = StringSlice("redis_coordination_operation")
    elif key == 120:
        name = StringSlice("redis_coordination_result")
    elif key == 121:
        name = StringSlice("reservation_recovery_operation")
    elif key == 122:
        name = StringSlice("reservation_recovery_result")
    elif key == 123:
        name = StringSlice("role_binding_operation")
    elif key == 124:
        name = StringSlice("role_binding_result")
    elif key == 125:
        name = StringSlice("routing_lane")
    elif key == 126:
        name = StringSlice("routing_outcome")
    elif key == 127:
        name = StringSlice("secret_backend")
    elif key == 128:
        name = StringSlice("secret_operation")
    elif key == 129:
        name = StringSlice("secret_result")
    elif key == 130:
        name = StringSlice("secret_rotation_result")
    elif key == 131:
        name = StringSlice("secret_scope")
    elif key == 132:
        name = StringSlice("security_decision")
    elif key == 133:
        name = StringSlice("security_result")
    elif key == 134:
        name = StringSlice("service_identity_operation")
    elif key == 135:
        name = StringSlice("service_identity_result")
    elif key == 136:
        name = StringSlice("shutdown_event")
    elif key == 137:
        name = StringSlice("shutdown_result")
    elif key == 138:
        name = StringSlice("siem_outbox_status")
    elif key == 139:
        name = StringSlice("status_class")
    elif key == 140:
        name = StringSlice("stream_outcome")
    elif key == 141:
        name = StringSlice("stream_transport")
    elif key == 142:
        name = StringSlice("telemetry_drop_reason")
    elif key == 143:
        name = StringSlice("tenant_isolation_result")
    elif key == 144:
        name = StringSlice("tenant_isolation_surface")
    elif key == 145:
        name = StringSlice("trace_carrier")
    elif key == 146:
        name = StringSlice("trace_propagation_result")
    elif key == 147:
        name = StringSlice("user_lifecycle_operation")
    elif key == 148:
        name = StringSlice("user_lifecycle_result")
    else:
        return OBSERVABILITY_STATUS_INVALID
    return observability_copy_label(name, output, output_capacity, output_length)

@export("prodex_mojo_observability_plan_label_spec_v1")
def prodex_mojo_observability_plan_label_spec_v1(
    abi_version: Int64,
    plan: Int64,
    slot: Int64,
    key_address: UInt,
    kind_address: UInt,
) abi("C") -> Int64:
    if abi_version != OBSERVABILITY_LABEL_ABI_VERSION:
        return OBSERVABILITY_STATUS_ABI
    if plan < 0 or slot < 0 or key_address == 0 or kind_address == 0:
        return OBSERVABILITY_STATUS_INVALID
    var key = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(key_address))
    var kind = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(kind_address))
    key[] = -1
    kind[] = -1
    if plan == 0 and slot == 0:
        key[] = 2
        kind[] = 0
    elif plan == 0 and slot == 1:
        key[] = 3
        kind[] = 1
    elif plan == 1 and slot == 0:
        key[] = 49
        kind[] = 2
    elif plan == 1 and slot == 1:
        key[] = 50
        kind[] = 3
    elif plan == 2 and slot == 0:
        key[] = 55
        kind[] = 4
    elif plan == 3 and slot == 0:
        key[] = 116
        kind[] = 5
    elif plan == 4 and slot == 0:
        key[] = 118
        kind[] = 7
    elif plan == 4 and slot == 1:
        key[] = 117
        kind[] = 6
    elif plan == 5 and slot == 0:
        key[] = 119
        kind[] = 8
    elif plan == 5 and slot == 1:
        key[] = 120
        kind[] = 9
    elif plan == 6 and slot == 0:
        key[] = 121
        kind[] = 10
    elif plan == 6 and slot == 1:
        key[] = 122
        kind[] = 11
    elif plan == 7 and slot == 0:
        key[] = 5
        kind[] = 31
    elif plan == 7 and slot == 1:
        key[] = 4
        kind[] = 12
    elif plan == 8 and slot == 0:
        key[] = 7
        kind[] = 14
    elif plan == 8 and slot == 1:
        key[] = 6
        kind[] = 13
    elif plan == 9 and slot == 0:
        key[] = 9
        kind[] = 16
    elif plan == 9 and slot == 1:
        key[] = 8
        kind[] = 15
    elif plan == 10 and slot == 0:
        key[] = 11
        kind[] = 18
    elif plan == 10 and slot == 1:
        key[] = 10
        kind[] = 17
    elif plan == 11 and slot == 0:
        key[] = 13
        kind[] = 20
    elif plan == 11 and slot == 1:
        key[] = 12
        kind[] = 19
    elif plan == 12 and slot == 0:
        key[] = 15
        kind[] = 22
    elif plan == 12 and slot == 1:
        key[] = 14
        kind[] = 21
    elif plan == 13 and slot == 0:
        key[] = 17
        kind[] = 24
    elif plan == 13 and slot == 1:
        key[] = 16
        kind[] = 23
    elif plan == 14 and slot == 0:
        key[] = 19
        kind[] = 26
    elif plan == 14 and slot == 1:
        key[] = 18
        kind[] = 25
    elif plan == 15 and slot == 0:
        key[] = 21
        kind[] = 28
    elif plan == 15 and slot == 1:
        key[] = 20
        kind[] = 27
    elif plan == 16 and slot == 0:
        key[] = 23
        kind[] = 30
    elif plan == 16 and slot == 1:
        key[] = 22
        kind[] = 29
    elif plan == 17 and slot == 0:
        key[] = 24
        kind[] = 31
    elif plan == 17 and slot == 1:
        key[] = 139
        kind[] = 36
    elif plan == 18 and slot == 0:
        key[] = 26
        kind[] = 32
    elif plan == 18 and slot == 1:
        key[] = 25
        kind[] = 33
    elif plan == 19 and slot == 0:
        key[] = 28
        kind[] = 35
    elif plan == 19 and slot == 1:
        key[] = 27
        kind[] = 34
    elif plan == 20 and slot == 0:
        key[] = 30
        kind[] = 38
    elif plan == 20 and slot == 1:
        key[] = 29
        kind[] = 37
    elif plan == 21 and slot == 0:
        key[] = 32
        kind[] = 40
    elif plan == 21 and slot == 1:
        key[] = 31
        kind[] = 39
    elif plan == 22 and slot == 0:
        key[] = 34
        kind[] = 42
    elif plan == 22 and slot == 1:
        key[] = 33
        kind[] = 41
    elif plan == 23 and slot == 0:
        key[] = 74
        kind[] = 43
    elif plan == 23 and slot == 1:
        key[] = 75
        kind[] = 44
    elif plan == 23 and slot == 2:
        key[] = 76
        kind[] = 45
    elif plan == 24 and slot == 0:
        key[] = 35
        kind[] = 46
    elif plan == 24 and slot == 1:
        key[] = 36
        kind[] = 47
    elif plan == 25 and slot == 0:
        key[] = 37
        kind[] = 48
    elif plan == 25 and slot == 1:
        key[] = 40
        kind[] = 51
    elif plan == 26 and slot == 0:
        key[] = 38
        kind[] = 49
    elif plan == 26 and slot == 1:
        key[] = 39
        kind[] = 50
    elif plan == 27 and slot == 0:
        key[] = 41
        kind[] = 52
    elif plan == 27 and slot == 1:
        key[] = 42
        kind[] = 53
    elif plan == 28 and slot == 0:
        key[] = 57
        kind[] = 55
    elif plan == 28 and slot == 1:
        key[] = 56
        kind[] = 54
    elif plan == 29 and slot == 0:
        key[] = 59
        kind[] = 57
    elif plan == 29 and slot == 1:
        key[] = 58
        kind[] = 56
    elif plan == 30 and slot == 0:
        key[] = 61
        kind[] = 138
    elif plan == 30 and slot == 1:
        key[] = 60
        kind[] = 139
    elif plan == 31 and slot == 0:
        key[] = 68
        kind[] = 58
    elif plan == 31 and slot == 1:
        key[] = 69
        kind[] = 59
    elif plan == 32 and slot == 0:
        key[] = 84
        kind[] = 60
    elif plan == 33 and slot == 0:
        key[] = 85
        kind[] = 61
    elif plan == 34 and slot == 0:
        key[] = 90
        kind[] = 62
    elif plan == 34 and slot == 1:
        key[] = 91
        kind[] = 63
    elif plan == 35 and slot == 0:
        key[] = 97
        kind[] = 65
    elif plan == 36 and slot == 0:
        key[] = 98
        kind[] = 66
    elif plan == 36 and slot == 1:
        key[] = 99
        kind[] = 67
    elif plan == 37 and slot == 0:
        key[] = 47
        kind[] = 68
    elif plan == 37 and slot == 1:
        key[] = 48
        kind[] = 69
    elif plan == 38 and slot == 0:
        key[] = 66
        kind[] = 70
    elif plan == 38 and slot == 1:
        key[] = 67
        kind[] = 71
    elif plan == 39 and slot == 0:
        key[] = 71
        kind[] = 73
    elif plan == 39 and slot == 1:
        key[] = 70
        kind[] = 72
    elif plan == 40 and slot == 0:
        key[] = 72
        kind[] = 74
    elif plan == 40 and slot == 1:
        key[] = 73
        kind[] = 75
    elif plan == 41 and slot == 0:
        key[] = 87
        kind[] = 77
    elif plan == 41 and slot == 1:
        key[] = 86
        kind[] = 76
    elif plan == 42 and slot == 0:
        key[] = 88
        kind[] = 78
    elif plan == 42 and slot == 1:
        key[] = 89
        kind[] = 79
    elif plan == 43 and slot == 0:
        key[] = 92
        kind[] = 80
    elif plan == 43 and slot == 1:
        key[] = 93
        kind[] = 81
    elif plan == 44 and slot == 0:
        key[] = 127
        kind[] = 82
    elif plan == 44 and slot == 1:
        key[] = 128
        kind[] = 83
    elif plan == 44 and slot == 2:
        key[] = 129
        kind[] = 84
    elif plan == 45 and slot == 0:
        key[] = 131
        kind[] = 86
    elif plan == 45 and slot == 1:
        key[] = 130
        kind[] = 85
    elif plan == 46 and slot == 0:
        key[] = 136
        kind[] = 87
    elif plan == 46 and slot == 1:
        key[] = 137
        kind[] = 88
    elif plan == 47 and slot == 0:
        key[] = 103
        kind[] = 94
    elif plan == 47 and slot == 1:
        key[] = 104
        kind[] = 89
    elif plan == 47 and slot == 2:
        key[] = 105
        kind[] = 90
    elif plan == 48 and slot == 0:
        key[] = 103
        kind[] = 94
    elif plan == 48 and slot == 1:
        key[] = 106
        kind[] = 137
    elif plan == 48 and slot == 2:
        key[] = 107
        kind[] = 91
    elif plan == 49 and slot == 0:
        key[] = 103
        kind[] = 94
    elif plan == 49 and slot == 1:
        key[] = 111
        kind[] = 93
    elif plan == 49 and slot == 2:
        key[] = 110
        kind[] = 92
    elif plan == 50 and slot == 0:
        key[] = 103
        kind[] = 94
    elif plan == 50 and slot == 1:
        key[] = 112
        kind[] = 95
    elif plan == 51 and slot == 0:
        key[] = 103
        kind[] = 94
    elif plan == 51 and slot == 1:
        key[] = 114
        kind[] = 96
    elif plan == 51 and slot == 2:
        key[] = 113
        kind[] = 97
    elif plan == 52 and slot == 0:
        key[] = 125
        kind[] = 99
    elif plan == 52 and slot == 1:
        key[] = 126
        kind[] = 98
    elif plan == 53 and slot == 0:
        key[] = 141
        kind[] = 101
    elif plan == 53 and slot == 1:
        key[] = 140
        kind[] = 100
    elif plan == 54 and slot == 0:
        key[] = 100
        kind[] = 102
    elif plan == 55 and slot == 0:
        key[] = 142
        kind[] = 104
    elif plan == 56 and slot == 0:
        key[] = 115
        kind[] = 103
    elif plan == 57 and slot == 0:
        key[] = 44
        kind[] = 106
    elif plan == 57 and slot == 1:
        key[] = 43
        kind[] = 105
    elif plan == 58 and slot == 0:
        key[] = 45
        kind[] = 107
    elif plan == 58 and slot == 1:
        key[] = 46
        kind[] = 108
    elif plan == 59 and slot == 0:
        key[] = 51
        kind[] = 109
    elif plan == 59 and slot == 1:
        key[] = 52
        kind[] = 110
    elif plan == 60 and slot == 0:
        key[] = 53
        kind[] = 146
    elif plan == 60 and slot == 1:
        key[] = 54
        kind[] = 111
    elif plan == 61 and slot == 0:
        key[] = 64
        kind[] = 140
    elif plan == 61 and slot == 1:
        key[] = 65
        kind[] = 112
    elif plan == 62 and slot == 0:
        key[] = 78
        kind[] = 114
    elif plan == 62 and slot == 1:
        key[] = 77
        kind[] = 113
    elif plan == 63 and slot == 0:
        key[] = 83
        kind[] = 119
    elif plan == 63 and slot == 1:
        key[] = 79
        kind[] = 115
    elif plan == 63 and slot == 2:
        key[] = 80
        kind[] = 116
    elif plan == 63 and slot == 3:
        key[] = 81
        kind[] = 117
    elif plan == 63 and slot == 4:
        key[] = 82
        kind[] = 118
    elif plan == 64 and slot == 0:
        key[] = 95
        kind[] = 120
    elif plan == 64 and slot == 1:
        key[] = 96
        kind[] = 121
    elif plan == 65 and slot == 0:
        key[] = 101
        kind[] = 141
    elif plan == 65 and slot == 1:
        key[] = 102
        kind[] = 122
    elif plan == 66 and slot == 0:
        key[] = 108
        kind[] = 144
    elif plan == 66 and slot == 1:
        key[] = 109
        kind[] = 145
    elif plan == 67 and slot == 0:
        key[] = 123
        kind[] = 143
    elif plan == 67 and slot == 1:
        key[] = 124
        kind[] = 123
    elif plan == 68 and slot == 0:
        key[] = 132
        kind[] = 124
    elif plan == 68 and slot == 1:
        key[] = 133
        kind[] = 125
    elif plan == 69 and slot == 0:
        key[] = 134
        kind[] = 142
    elif plan == 69 and slot == 1:
        key[] = 135
        kind[] = 126
    elif plan == 70 and slot == 0:
        key[] = 144
        kind[] = 128
    elif plan == 70 and slot == 1:
        key[] = 143
        kind[] = 127
    elif plan == 71 and slot == 0:
        key[] = 0
        kind[] = 129
    elif plan == 71 and slot == 1:
        key[] = 1
        kind[] = 130
    elif plan == 72 and slot == 0:
        key[] = 147
        kind[] = 131
    elif plan == 72 and slot == 1:
        key[] = 148
        kind[] = 132
    elif plan == 73 and slot == 0:
        key[] = 62
        kind[] = 133
    elif plan == 73 and slot == 1:
        key[] = 63
        kind[] = 134
    elif plan == 75 and slot == 0:
        key[] = 145
        kind[] = 135
    elif plan == 75 and slot == 1:
        key[] = 146
        kind[] = 136
    elif plan == 76 and slot == 0:
        key[] = 94
        kind[] = 64
    else:
        return OBSERVABILITY_STATUS_INVALID
    return OBSERVABILITY_STATUS_OK
