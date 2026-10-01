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


def audit_policy_view_equals(left: ProdexRichStringView, right: ProdexRichStringView) -> Bool:
    if left.len != right.len:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


@fieldwise_init
struct AuditPolicyWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def audit_policy_put_byte(
    writer: Pointer[mut=True, AuditPolicyWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def audit_policy_put_literal(
    writer: Pointer[mut=True, AuditPolicyWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not audit_policy_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def audit_policy_put_view(
    writer: Pointer[mut=True, AuditPolicyWriter, _], value: ProdexRichStringView
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not audit_policy_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def audit_policy_put_u64(
    writer: Pointer[mut=True, AuditPolicyWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return audit_policy_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not audit_policy_put_byte(
            writer, UInt8(remaining / divisor) + UInt8(48)
        ):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def audit_policy_optional_view(
    address: UInt, length: Int64, present: Int64
) -> Tuple[Bool, ProdexRichStringView]:
    if present != 0 and present != 1:
        return (False, ProdexRichStringView(0, 0))
    if length < 0 or length > AUDIT_LOG_POLICY_MAX_TEXT_BYTES:
        return (False, ProdexRichStringView(0, 0))
    if present == 0:
        if length != 0:
            return (False, ProdexRichStringView(0, 0))
        return (True, ProdexRichStringView(0, 0))
    if length > 0 and address == 0:
        return (False, ProdexRichStringView(0, 0))
    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, AUDIT_LOG_POLICY_MAX_TEXT_BYTES):
        return (False, ProdexRichStringView(0, 0))
    return (True, view.copy())


@export("prodex_audit_query_has_filters_v1")
def prodex_audit_query_has_filters_v1(
    abi_version: Int64,
    component_present: Int64,
    action_present: Int64,
    outcome_present: Int64,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return -4
    if (
        (component_present != 0 and component_present != 1)
        or (action_present != 0 and action_present != 1)
        or (outcome_present != 0 and outcome_present != 1)
    ):
        return -1
    return Int64(
        component_present == 1 or action_present == 1 or outcome_present == 1
    )


@export("prodex_audit_query_matches_v1")
def prodex_audit_query_matches_v1(
    abi_version: Int64,
    component_address: UInt,
    component_length: Int64,
    component_present: Int64,
    action_address: UInt,
    action_length: Int64,
    action_present: Int64,
    outcome_address: UInt,
    outcome_length: Int64,
    outcome_present: Int64,
    event_component_address: UInt,
    event_component_length: Int64,
    event_action_address: UInt,
    event_action_length: Int64,
    event_outcome_address: UInt,
    event_outcome_length: Int64,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return -4
    var component = audit_policy_optional_view(
        component_address, component_length, component_present
    )
    var action = audit_policy_optional_view(
        action_address, action_length, action_present
    )
    var outcome = audit_policy_optional_view(
        outcome_address, outcome_length, outcome_present
    )
    if not component[0] or not action[0] or not outcome[0]:
        return -1
    if (
        event_component_length < 0
        or event_action_length < 0
        or event_outcome_length < 0
        or (event_component_length > 0 and event_component_address == 0)
        or (event_action_length > 0 and event_action_address == 0)
        or (event_outcome_length > 0 and event_outcome_address == 0)
    ):
        return -1
    var event_component = ProdexRichStringView(
        event_component_address, UInt(event_component_length)
    )
    var event_action = ProdexRichStringView(
        event_action_address, UInt(event_action_length)
    )
    var event_outcome = ProdexRichStringView(
        event_outcome_address, UInt(event_outcome_length)
    )
    if (
        not rich_view_valid(event_component, AUDIT_LOG_POLICY_MAX_TEXT_BYTES)
        or not rich_view_valid(event_action, AUDIT_LOG_POLICY_MAX_TEXT_BYTES)
        or not rich_view_valid(event_outcome, AUDIT_LOG_POLICY_MAX_TEXT_BYTES)
    ):
        return -1
    if (
        component_present == 1
        and not audit_policy_view_equals(component[1], event_component)
    ):
        return 0
    if (
        action_present == 1
        and not audit_policy_view_equals(action[1], event_action)
    ):
        return 0
    if (
        outcome_present == 1
        and not audit_policy_view_equals(outcome[1], event_outcome)
    ):
        return 0
    return 1


@export("prodex_audit_query_format_v1")
def prodex_audit_query_format_v1(
    abi_version: Int64,
    component_address: UInt,
    component_length: Int64,
    component_present: Int64,
    action_address: UInt,
    action_length: Int64,
    action_present: Int64,
    outcome_address: UInt,
    outcome_length: Int64,
    outcome_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if output_address == 0 or output_capacity < 0 or written_address == 0:
        return AUDIT_LOG_POLICY_INVALID
    var component = audit_policy_optional_view(
        component_address, component_length, component_present
    )
    var action = audit_policy_optional_view(
        action_address, action_length, action_present
    )
    var outcome = audit_policy_optional_view(
        outcome_address, outcome_length, outcome_present
    )
    if not component[0] or not action[0] or not outcome[0]:
        return AUDIT_LOG_POLICY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = AuditPolicyWriter(output, output_capacity, 0)
    var any = False
    if component_present == 1:
        if (
            not audit_policy_put_literal(
                Pointer(to=writer), StringSlice("component=")
            )
            or not audit_policy_put_view(Pointer(to=writer), component[1])
        ):
            return AUDIT_LOG_POLICY_INVALID
        any = True
    if action_present == 1:
        if any and not audit_policy_put_byte(Pointer(to=writer), UInt8(32)):
            return AUDIT_LOG_POLICY_INVALID
        if (
            not audit_policy_put_literal(
                Pointer(to=writer), StringSlice("action=")
            )
            or not audit_policy_put_view(Pointer(to=writer), action[1])
        ):
            return AUDIT_LOG_POLICY_INVALID
        any = True
    if outcome_present == 1:
        if any and not audit_policy_put_byte(Pointer(to=writer), UInt8(32)):
            return AUDIT_LOG_POLICY_INVALID
        if (
            not audit_policy_put_literal(
                Pointer(to=writer), StringSlice("outcome=")
            )
            or not audit_policy_put_view(Pointer(to=writer), outcome[1])
        ):
            return AUDIT_LOG_POLICY_INVALID
        any = True
    if not any and not audit_policy_put_literal(
        Pointer(to=writer), StringSlice("none")
    ):
        return AUDIT_LOG_POLICY_INVALID

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return AUDIT_LOG_POLICY_OK


@export("prodex_audit_search_scope_format_v1")
def prodex_audit_search_scope_format_v1(
    abi_version: Int64,
    searched_bytes: UInt64,
    log_size_bytes: UInt64,
    search_start_byte: UInt64,
    read_limit_bytes: UInt64,
    limited: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if (
        (limited != 0 and limited != 1)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return AUDIT_LOG_POLICY_INVALID
    var search_end_byte = (
        AUDIT_LOG_POLICY_UINT64_MAX
        if AUDIT_LOG_POLICY_UINT64_MAX - search_start_byte < searched_bytes
        else search_start_byte + searched_bytes
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = AuditPolicyWriter(output, output_capacity, 0)
    if (
        not audit_policy_put_literal(
            Pointer(to=writer), StringSlice("searched ")
        )
        or not audit_policy_put_u64(Pointer(to=writer), searched_bytes)
        or not audit_policy_put_literal(
            Pointer(to=writer), StringSlice(" of ")
        )
        or not audit_policy_put_u64(Pointer(to=writer), log_size_bytes)
        or not audit_policy_put_literal(
            Pointer(to=writer), StringSlice(" bytes (byte range ")
        )
        or not audit_policy_put_u64(Pointer(to=writer), search_start_byte)
        or not audit_policy_put_literal(Pointer(to=writer), StringSlice(".."))
        or not audit_policy_put_u64(Pointer(to=writer), search_end_byte)
        or not audit_policy_put_byte(Pointer(to=writer), UInt8(41))
    ):
        return AUDIT_LOG_POLICY_INVALID
    if limited == 1:
        if (
            not audit_policy_put_literal(
                Pointer(to=writer), StringSlice(" limited to last ")
            )
            or not audit_policy_put_u64(Pointer(to=writer), read_limit_bytes)
            or not audit_policy_put_literal(
                Pointer(to=writer), StringSlice(" bytes")
            )
        ):
            return AUDIT_LOG_POLICY_INVALID
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return AUDIT_LOG_POLICY_OK


@export("prodex_audit_truncate_text_v1")
def prodex_audit_truncate_text_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    max_chars: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_LOG_POLICY_ABI_VERSION:
        return AUDIT_LOG_POLICY_ABI
    if (
        input_length < 0
        or max_chars < 0
        or input_length > AUDIT_LOG_POLICY_MAX_TEXT_BYTES
        or (input_length > 0 and input_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return AUDIT_LOG_POLICY_INVALID
    var input = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(input, AUDIT_LOG_POLICY_MAX_TEXT_BYTES):
        return AUDIT_LOG_POLICY_INVALID
    var source = rich_view_ptr(input)
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor < input_length and count < max_chars:
        cursor += rich_codepoint_width(source[unsafe_offset=cursor])
        count += 1
    var truncated = cursor < input_length
    var required = cursor + (Int64(3) if truncated else Int64(0))
    if required > output_capacity:
        return AUDIT_LOG_POLICY_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(cursor):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    if truncated:
        output[unsafe_offset=cursor] = UInt8(46)
        output[unsafe_offset=cursor + 1] = UInt8(46)
        output[unsafe_offset=cursor + 2] = UInt8(46)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = required
    return AUDIT_LOG_POLICY_OK
