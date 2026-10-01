from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime AUDIT_LOG_POLICY_ABI_VERSION: Int64 = 1
comptime AUDIT_LOG_POLICY_OK: Int64 = 0
comptime AUDIT_LOG_POLICY_INVALID: Int64 = 1
comptime AUDIT_LOG_POLICY_ABI: Int64 = 4
comptime AUDIT_LOG_POLICY_MAX_TEXT_BYTES: Int64 = 1_048_576
comptime AUDIT_LOG_POLICY_MAX_ROWS: Int64 = 1_000_000
comptime AUDIT_LOG_POLICY_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


def audit_usage_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if AUDIT_LOG_POLICY_UINT64_MAX - left < right:
        return AUDIT_LOG_POLICY_UINT64_MAX
    return left + right


def audit_usage_allowed_codepoint(codepoint: Int64) -> Bool:
    return (
        codepoint >= 48 and codepoint <= 57
        or codepoint >= 65 and codepoint <= 90
        or codepoint >= 97 and codepoint <= 122
        or codepoint == 45
        or codepoint == 95
        or codepoint == 46
        or codepoint == 58
        or codepoint == 47
    )


def audit_usage_lower_ascii(codepoint: Int64) -> UInt8:
    if codepoint >= 65 and codepoint <= 90:
        return UInt8(codepoint + 32)
    return UInt8(codepoint)


@export("prodex_audit_usage_token_normalize_v1")
def prodex_audit_usage_token_normalize_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    fallback_address: UInt,
    fallback_length: Int64,
    max_chars: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if (
        input_length < 0
        or fallback_length < 0
        or max_chars < 0
        or output_capacity < 0
        or input_length > AUDIT_LOG_POLICY_MAX_TEXT_BYTES
        or fallback_length > AUDIT_LOG_POLICY_MAX_TEXT_BYTES
        or (input_length > 0 and input_address == 0)
        or (fallback_length > 0 and fallback_address == 0)
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return AUDIT_LOG_POLICY_INVALID

    var input = ProdexRichStringView(input_address, UInt(input_length))
    var fallback = ProdexRichStringView(fallback_address, UInt(fallback_length))
    if (
        not rich_view_valid(input, AUDIT_LOG_POLICY_MAX_TEXT_BYTES)
        or not rich_view_valid(fallback, AUDIT_LOG_POLICY_MAX_TEXT_BYTES)
    ):
        return AUDIT_LOG_POLICY_INVALID

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var bounds = rich_trim_bounds(input)
    var source = rich_view_ptr(input)
    var cursor = bounds[0]
    var count: Int64 = 0
    var output_length: Int64 = 0
    while cursor < bounds[1] and count < max_chars:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        if output_length >= output_capacity:
            return AUDIT_LOG_POLICY_INVALID
        output[unsafe_offset=output_length] = (
            audit_usage_lower_ascii(codepoint)
            if audit_usage_allowed_codepoint(codepoint)
            else UInt8(45)
        )
        output_length += 1
        count += 1
        cursor += width

    var start: Int64 = 0
    while start < output_length and output[unsafe_offset=start] == 45:
        start += 1
    var end = output_length
    while end > start and output[unsafe_offset=end - 1] == 45:
        end -= 1

    if start == end:
        if fallback_length > output_capacity:
            return AUDIT_LOG_POLICY_INVALID
        var fallback_ptr = rich_view_ptr(fallback)
        for index in range(fallback_length):
            output[unsafe_offset=index] = fallback_ptr[unsafe_offset=index]
        written[] = fallback_length
        return AUDIT_LOG_POLICY_OK

    var normalized_length = end - start
    for index in range(normalized_length):
        output[unsafe_offset=index] = output[unsafe_offset=start + index]
    written[] = normalized_length
    return AUDIT_LOG_POLICY_OK


@export("prodex_audit_usage_total_v1")
def prodex_audit_usage_total_v1(
    abi_version: Int64,
    current_total: UInt64,
    input_tokens: UInt64,
    output_tokens: UInt64,
    reasoning_tokens: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if output_address == 0:
        return AUDIT_LOG_POLICY_INVALID
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if current_total != 0:
        output[] = current_total
    else:
        output[] = audit_usage_saturating_add(
            audit_usage_saturating_add(input_tokens, output_tokens),
            reasoning_tokens,
        )
    return AUDIT_LOG_POLICY_OK


@export("prodex_audit_usage_summary_v1")
def prodex_audit_usage_summary_v1(
    abi_version: Int64,
    epochs_address: UInt,
    metrics_address: UInt,
    row_count: Int64,
    since_epoch: Int64,
    until_epoch: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if (
        row_count < 0
        or row_count > AUDIT_LOG_POLICY_MAX_ROWS
        or (row_count > 0 and (epochs_address == 0 or metrics_address == 0))
        or output_address == 0
    ):
        return AUDIT_LOG_POLICY_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(7):
        output[unsafe_offset=index] = 0
    if row_count == 0:
        return AUDIT_LOG_POLICY_OK

    var epochs = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(epochs_address)
    )
    var metrics = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(metrics_address)
    )
    for row in range(row_count):
        var epoch = epochs[unsafe_offset=row]
        if epoch < since_epoch or epoch > until_epoch:
            continue
        output[unsafe_offset=0] = audit_usage_saturating_add(
            output[unsafe_offset=0], UInt64(1)
        )
        for metric in range(6):
            output[unsafe_offset=metric + 1] = audit_usage_saturating_add(
                output[unsafe_offset=metric + 1],
                metrics[unsafe_offset=row * 6 + Int64(metric)],
            )
    return AUDIT_LOG_POLICY_OK


@export("prodex_audit_budget_flags_v1")
def prodex_audit_budget_flags_v1(
    abi_version: Int64,
    requests: UInt64,
    total_tokens: UInt64,
    cost_micros: UInt64,
    max_requests_present: Int64,
    max_requests: UInt64,
    max_tokens_present: Int64,
    max_tokens: UInt64,
    max_cost_present: Int64,
    max_cost_micros: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if (
        (max_requests_present != 0 and max_requests_present != 1)
        or (max_tokens_present != 0 and max_tokens_present != 1)
        or (max_cost_present != 0 and max_cost_present != 1)
        or output_address == 0
    ):
        return AUDIT_LOG_POLICY_INVALID

    var flags: UInt64 = 0
    if max_requests_present == 1 and requests >= max_requests:
        flags |= UInt64(1)
    if max_tokens_present == 1 and total_tokens >= max_tokens:
        flags |= UInt64(2)
    if max_cost_present == 1 and cost_micros >= max_cost_micros:
        flags |= UInt64(4)

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = flags
    return AUDIT_LOG_POLICY_OK
