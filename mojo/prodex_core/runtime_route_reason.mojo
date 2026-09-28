from std.memory import Pointer

from rich_text import rich_codepoint_width, rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime ROUTE_REASON_ABI_VERSION: Int64 = 1
comptime ROUTE_REASON_INVALID: Int64 = -2
comptime ROUTE_REASON_UNKNOWN: Int64 = -1
comptime ROUTE_REASON_MAX_IDENTIFIER_BYTES: Int64 = 96

def reason_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))

def reason_equal[address_literal: StaticString](
    address: UInt, length: Int64
) -> Bool:
    var n = Int64(address_literal.byte_length())
    if length != n:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))
    var target = address_literal.unsafe_ptr()
    for index in range(n):
        if source[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True

def reason_kind(address: UInt, length: Int64) -> Int64:
    if reason_equal["auth_failure_backoff"](address, length): return 0
    if reason_equal["selection_backoff"](address, length): return 1
    if reason_equal["route_circuit_open"](address, length): return 2
    if reason_equal["route_circuit_half_open_probe_wait"](address, length): return 3
    if reason_equal["profile_health"](address, length): return 4
    if reason_equal["profile_performance"](address, length): return 5
    if reason_equal["quota_probe_unavailable"](address, length): return 6
    if reason_equal["stale_persisted_quota"](address, length): return 7
    if reason_equal["quota_healthy"](address, length): return 8
    if reason_equal["quota_thin"](address, length): return 9
    if reason_equal["quota_critical"](address, length): return 10
    if reason_equal["quota_exhausted"](address, length): return 11
    if reason_equal["quota_unknown"](address, length): return 12
    if reason_equal["quota_exhausted_before_send"](address, length): return 13
    if reason_equal["quota_windows_unavailable"](address, length): return 14
    if reason_equal["profile_inflight_soft_limit"](address, length): return 15
    if reason_equal["auth_not_quota_compatible"](address, length): return 16
    if reason_equal["prompt_cache_affinity"](address, length): return 17
    if reason_equal["negative_cache"](address, length): return 18
    if reason_equal["excluded"](address, length): return 19
    if reason_equal["affinity_owner_unavailable"](address, length): return 20
    if reason_equal["selection_failed"](address, length): return 21
    if reason_equal["compatible"](address, length): return 22
    if reason_equal["endpoint_unsupported"](address, length): return 23
    if reason_equal["required_capability_missing"](address, length): return 24
    if reason_equal["catalog_entry_unavailable"](address, length): return 25
    if reason_equal["context_window_unknown"](address, length): return 26
    if reason_equal["context_window_exceeded"](address, length): return 27
    if reason_equal["output_limit_unknown"](address, length): return 28
    if reason_equal["requested_output_exceeds_model_limit"](address, length): return 29
    if reason_equal["reasoning_reserve_unsupported"](address, length): return 30
    if reason_equal["reasoning_reserve_excessive"](address, length): return 31
    if reason_equal["malformed_request_limits"](address, length): return 32
    if reason_equal["output_limit_clamped"](address, length): return 33
    return ROUTE_REASON_UNKNOWN

def reason_stage(kind: Int64) -> Int64:
    if kind == 0 or kind == 16:
        return 5
    if kind == 1 or kind == 2 or kind == 3:
        return 7
    if kind >= 6 and kind <= 14:
        return 6
    if kind == 15:
        return 8
    if kind == 4 or kind == 5 or kind == 17:
        return 9
    if kind == 18 or kind == 19 or kind == 20:
        return 0
    if kind == 21:
        return 10
    if kind == 23 or kind == 24:
        return 2
    if kind == 25:
        return 1
    if kind == 22 or (kind >= 26 and kind <= 33):
        return 3
    return ROUTE_REASON_UNKNOWN

@export("prodex_runtime_route_reason_lookup_v1")
def prodex_runtime_route_reason_lookup_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    stage_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != ROUTE_REASON_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or stage_address == 0
    ):
        return ROUTE_REASON_INVALID
    var view = reason_view(address, length)
    if not rich_view_valid(view, length):
        return ROUTE_REASON_INVALID
    var kind = reason_kind(address, length)
    var stage = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(stage_address)
    )
    stage[] = reason_stage(kind) if kind >= 0 else ROUTE_REASON_UNKNOWN
    return kind


@export("prodex_runtime_route_reason_stage_v1")
def prodex_runtime_route_reason_stage_v1(
    abi_version: Int64,
    kind: Int64,
) abi("C") -> Int64:
    if abi_version != ROUTE_REASON_ABI_VERSION or kind < 0 or kind > 33:
        return ROUTE_REASON_INVALID
    return reason_stage(kind)

@export("prodex_runtime_route_reason_unknown_span_v1")
def prodex_runtime_route_reason_unknown_span_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != ROUTE_REASON_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return ROUTE_REASON_INVALID
    var view = reason_view(address, length)
    if not rich_view_valid(view, length):
        return ROUTE_REASON_INVALID
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    var valid = end > start and end - start <= ROUTE_REASON_MAX_IDENTIFIER_BYTES
    if valid:
        var source = rich_view_ptr(view)
        for index in range(start, end):
            var value = source[unsafe_offset=index]
            if not (
                value >= 97 and value <= 122
                or value >= 48 and value <= 57
                or value == 95
            ):
                valid = False
                break
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = start if valid else -1
    output[1] = end if valid else -1
    return 1 if valid else 0


@export("prodex_runtime_route_identifier_span_v1")
def prodex_runtime_route_identifier_span_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != ROUTE_REASON_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return ROUTE_REASON_INVALID

    var view = reason_view(address, length)
    if not rich_view_valid(view, length):
        return ROUTE_REASON_INVALID

    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var full_end = bounds[1]
    var end = full_end
    var truncated = False

    if full_end - start > ROUTE_REASON_MAX_IDENTIFIER_BYTES:
        var source = rich_view_ptr(view)
        var cursor = start
        var limit = start + ROUTE_REASON_MAX_IDENTIFIER_BYTES
        while cursor < full_end:
            var width = rich_codepoint_width(source[unsafe_offset=cursor])
            if cursor + width > limit:
                break
            cursor += width
        end = cursor
        truncated = end < full_end

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = start
    output[1] = end
    output[2] = Int64(truncated)
    return 0
