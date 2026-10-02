from std.memory import Pointer

from rich_text import rich_codepoint_width, rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime ROUTE_REASON_ABI_VERSION: Int64 = 1
comptime ROUTE_REASON_INVALID: Int64 = -2
comptime ROUTE_REASON_UNKNOWN: Int64 = -1
comptime ROUTE_REASON_MAX_IDENTIFIER_BYTES: Int64 = 96

def reason_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))

def reason_catalog_literal[label: StaticString](
    mode: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Bool:
    var n = Int64(label.byte_length())
    var literal = label.unsafe_ptr()
    if mode == 0:
        if length != n:
            return False
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(address)
        )
        for index in range(n):
            if source[unsafe_offset=index] != literal[unsafe_offset=index]:
                return False
        return True

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = n
    if output_capacity < n:
        return False
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(n):
        output[unsafe_offset=index] = literal[unsafe_offset=index]
    return True


def reason_catalog(
    kind: Int64,
    mode: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Bool:
    if kind == 0:
        return reason_catalog_literal["auth_failure_backoff"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 1:
        return reason_catalog_literal["selection_backoff"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 2:
        return reason_catalog_literal["route_circuit_open"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 3:
        return reason_catalog_literal["route_circuit_half_open_probe_wait"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 4:
        return reason_catalog_literal["profile_health"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 5:
        return reason_catalog_literal["profile_performance"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 6:
        return reason_catalog_literal["quota_probe_unavailable"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 7:
        return reason_catalog_literal["stale_persisted_quota"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 8:
        return reason_catalog_literal["quota_healthy"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 9:
        return reason_catalog_literal["quota_thin"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 10:
        return reason_catalog_literal["quota_critical"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 11:
        return reason_catalog_literal["quota_exhausted"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 12:
        return reason_catalog_literal["quota_unknown"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 13:
        return reason_catalog_literal["quota_exhausted_before_send"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 14:
        return reason_catalog_literal["quota_windows_unavailable"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 15:
        return reason_catalog_literal["profile_inflight_soft_limit"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 16:
        return reason_catalog_literal["auth_not_quota_compatible"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 17:
        return reason_catalog_literal["prompt_cache_affinity"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 18:
        return reason_catalog_literal["negative_cache"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 19:
        return reason_catalog_literal["excluded"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 20:
        return reason_catalog_literal["affinity_owner_unavailable"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 21:
        return reason_catalog_literal["selection_failed"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 22:
        return reason_catalog_literal["compatible"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 23:
        return reason_catalog_literal["endpoint_unsupported"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 24:
        return reason_catalog_literal["required_capability_missing"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 25:
        return reason_catalog_literal["catalog_entry_unavailable"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 26:
        return reason_catalog_literal["context_window_unknown"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 27:
        return reason_catalog_literal["context_window_exceeded"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 28:
        return reason_catalog_literal["output_limit_unknown"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 29:
        return reason_catalog_literal["requested_output_exceeds_model_limit"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 30:
        return reason_catalog_literal["reasoning_reserve_unsupported"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 31:
        return reason_catalog_literal["reasoning_reserve_excessive"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 32:
        return reason_catalog_literal["malformed_request_limits"](
            mode, address, length, output_address, output_capacity, written_address
        )
    elif kind == 33:
        return reason_catalog_literal["output_limit_clamped"](
            mode, address, length, output_address, output_capacity, written_address
        )
    return False


def reason_kind(address: UInt, length: Int64) -> Int64:
    for kind in range(Int64(34)):
        if reason_catalog(kind, 0, address, length, 0, 0, 0):
            return kind
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


@export("prodex_runtime_route_reason_label_v1")
def prodex_runtime_route_reason_label_v1(
    abi_version: Int64,
    kind: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != ROUTE_REASON_ABI_VERSION
        or kind < 0
        or kind > 33
        or output_capacity < 0
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return ROUTE_REASON_INVALID

    if not reason_catalog(
        kind,
        1,
        0,
        0,
        output_address,
        output_capacity,
        written_address,
    ):
        return -3
    return 0


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
