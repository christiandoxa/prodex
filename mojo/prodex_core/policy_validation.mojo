from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime POLICY_NUMERIC_NON_ZERO: Int64 = 0
comptime POLICY_NUMERIC_RANGE: Int64 = 1
comptime POLICY_NUMERIC_RELATION_LE: Int64 = 2
comptime UINT64_MAX: UInt64 = 18446744073709551615

comptime POLICY_TEXT_OBSERVABILITY_SCHEMA: Int64 = 1
comptime POLICY_TEXT_STATE_BACKEND: Int64 = 2
comptime POLICY_TEXT_ADMIN_ROLE: Int64 = 3
comptime POLICY_TEXT_WEBHOOK_PHASE: Int64 = 4
comptime POLICY_TEXT_HTTP_ENDPOINT: Int64 = 5


def policy_text_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def policy_text_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def policy_text_equals(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    if view.len != UInt(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if policy_text_ascii_lower(policy_text_byte(view, index)) != policy_text_ascii_lower(right[unsafe_offset=index]):
            return False
    return True


def policy_text_equals_exact(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    if view.len != UInt(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if policy_text_byte(view, index) != right[unsafe_offset=index]:
            return False
    return True


def policy_text_has_ascii_whitespace(view: ProdexRichStringView) -> Bool:
    for index in range(Int64(view.len)):
        var value = policy_text_byte(view, index)
        if value == 9 or value >= 10 and value <= 13 or value == 32:
            return True
    return False


def policy_text_token_allowed(view: ProdexRichStringView, kind: Int64) -> Bool:
    if view.len == 0 or policy_text_has_ascii_whitespace(view):
        return False
    if kind == POLICY_TEXT_OBSERVABILITY_SCHEMA:
        for literal in [
            StringSlice("generic"), StringSlice("otel"), StringSlice("otlp"),
            StringSlice("opentelemetry"), StringSlice("datadog"), StringSlice("langfuse"),
        ]:
            if policy_text_equals(view, literal):
                return True
        return False
    if kind == POLICY_TEXT_STATE_BACKEND:
        for literal in [StringSlice("file"), StringSlice("sqlite"), StringSlice("postgres"), StringSlice("redis")]:
            if policy_text_equals(view, literal):
                return True
        return False
    if kind == POLICY_TEXT_ADMIN_ROLE:
        for literal in [
            StringSlice("admin"), StringSlice("write"), StringSlice("writer"),
            StringSlice("viewer"), StringSlice("read"), StringSlice("readonly"), StringSlice("read-only"),
        ]:
            if policy_text_equals(view, literal):
                return True
        return False
    for literal in [StringSlice("pre"), StringSlice("request"), StringSlice("post"), StringSlice("response")]:
        if policy_text_equals(view, literal):
            return True
    return False


def policy_text_http_endpoint_valid(view: ProdexRichStringView) -> Bool:
    if view.len < 3:
        return False
    var separator: Int64 = -1
    for index in range(Int64(view.len) - 2):
        if policy_text_byte(view, index) == 58 and policy_text_byte(view, index + 1) == 47 and policy_text_byte(view, index + 2) == 47:
            separator = index
            break
    if separator < 0:
        return False
    var scheme = ProdexRichStringView(view.ptr, UInt(separator))
    if not (policy_text_equals_exact(scheme, StringSlice("http")) or policy_text_equals_exact(scheme, StringSlice("https"))):
        return False
    var host_start = separator + 3
    var host_end = Int64(view.len)
    for index in range(host_start, Int64(view.len)):
        var value = policy_text_byte(view, index)
        if value == 47 or value == 63 or value == 35:
            host_end = index
            break
    var host_view = ProdexRichStringView(
        view.ptr + UInt(host_start), UInt(host_end - host_start)
    )
    var host_bounds = rich_trim_bounds(host_view)
    host_start += host_bounds[0]
    host_end = host_start + host_bounds[1] - host_bounds[0]
    if host_start == host_end:
        return False
    for index in range(host_start, host_end):
        if policy_text_byte(view, index) == 64:
            return False
    return True


@export("prodex_runtime_policy_validate_text")
def prodex_runtime_policy_validate_text(
    abi_version: Int64,
    value_address: UInt,
    kind: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 6 or value_address == 0 or output_address == 0:
        return 4
    var value = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(value_address))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var view = value[].copy()
    if not rich_view_valid(view, 4_194_304):
        return 2
    if kind == POLICY_TEXT_HTTP_ENDPOINT:
        output[] = Int64(policy_text_http_endpoint_valid(view))
        return 0
    if kind < POLICY_TEXT_OBSERVABILITY_SCHEMA or kind > POLICY_TEXT_WEBHOOK_PHASE:
        return 1
    output[] = Int64(policy_text_token_allowed(view, kind))
    return 0

@export("prodex_runtime_policy_validate_numeric")
def prodex_runtime_policy_validate_numeric(
    values: Pointer[mut=False, UInt64, _],
    kinds: Pointer[mut=False, Int64, _],
    minimums: Pointer[mut=False, UInt64, _],
    maximums: Pointer[mut=False, UInt64, _],
    related_values: Pointer[mut=False, UInt64, _],
    failed_rules: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0:
        return 1

    for index in range(count):
        var kind = kinds[unsafe_offset=index]
        var value = values[unsafe_offset=index]
        var invalid = False
        if kind == POLICY_NUMERIC_NON_ZERO:
            invalid = value == 0
        elif kind == POLICY_NUMERIC_RANGE:
            invalid = value < minimums[unsafe_offset=index] or value > maximums[unsafe_offset=index]
        elif kind == POLICY_NUMERIC_RELATION_LE:
            invalid = value > related_values[unsafe_offset=index]
        else:
            return 2

        if invalid:
            failed_rules[unsafe_offset=index] = 1
        else:
            failed_rules[unsafe_offset=index] = 0
    return 0


comptime ACCOUNTING_USAGE_ADD: Int64 = 0
comptime ACCOUNTING_USAGE_SATURATING_SUB: Int64 = 1
comptime ACCOUNTING_USAGE_EXCEEDS: Int64 = 2
comptime ACCOUNTING_SNAPSHOT_AVAILABLE: Int64 = 3
comptime ACCOUNTING_RESERVE: Int64 = 4
comptime ACCOUNTING_COMMIT: Int64 = 5


def accounting_value(
    values_address: UInt, index: Int64
) -> UInt64:
    var values = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    return values[unsafe_offset=index]


def accounting_checked_add(left: UInt64, right: UInt64) -> InlineArray[UInt64, 2]:
    var result = InlineArray[UInt64, 2](fill=0)
    if left > UINT64_MAX - right:
        result[1] = 1
    else:
        result[0] = left + right
    return result^


@export("prodex_domain_accounting_arithmetic_v1")
def prodex_domain_accounting_arithmetic_v1(
    abi_version: Int64,
    operation: Int64,
    values_address: UInt,
    value_count: Int64,
    output_address: UInt,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != 6 or value_count < 0 or output_address == 0 or result_address == 0:
        return 4
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    result[] = 0
    var required: Int64 = 4
    if operation == ACCOUNTING_SNAPSHOT_AVAILABLE:
        required = 6
    elif operation == ACCOUNTING_RESERVE or operation == ACCOUNTING_COMMIT:
        required = 8
    if operation < ACCOUNTING_USAGE_ADD or operation > ACCOUNTING_COMMIT or value_count < required:
        return 1
    if required > 0 and values_address == 0:
        return 1

    if operation == ACCOUNTING_USAGE_ADD:
        var tokens = accounting_checked_add(accounting_value(values_address, 0), accounting_value(values_address, 2))
        var cost = accounting_checked_add(accounting_value(values_address, 1), accounting_value(values_address, 3))
        if tokens[1] == 1 or cost[1] == 1:
            result[] = 1
        else:
            output[unsafe_offset=0] = tokens[0]
            output[unsafe_offset=1] = cost[0]
        return 0
    if operation == ACCOUNTING_USAGE_SATURATING_SUB:
        output[unsafe_offset=0] = accounting_value(values_address, 0) - accounting_value(values_address, 2) if accounting_value(values_address, 0) > accounting_value(values_address, 2) else 0
        output[unsafe_offset=1] = accounting_value(values_address, 1) - accounting_value(values_address, 3) if accounting_value(values_address, 1) > accounting_value(values_address, 3) else 0
        return 0
    if operation == ACCOUNTING_USAGE_EXCEEDS:
        output[unsafe_offset=0] = UInt64(accounting_value(values_address, 0) > accounting_value(values_address, 2) or accounting_value(values_address, 1) > accounting_value(values_address, 3))
        return 0
    if operation == ACCOUNTING_SNAPSHOT_AVAILABLE:
        var held_tokens = accounting_checked_add(accounting_value(values_address, 0), accounting_value(values_address, 2))
        var held_cost = accounting_checked_add(accounting_value(values_address, 1), accounting_value(values_address, 3))
        if held_tokens[1] == 1 or held_cost[1] == 1:
            result[] = 1
        else:
            output[unsafe_offset=0] = accounting_value(values_address, 4) - held_tokens[0] if accounting_value(values_address, 4) > held_tokens[0] else 0
            output[unsafe_offset=1] = accounting_value(values_address, 5) - held_cost[0] if accounting_value(values_address, 5) > held_cost[0] else 0
        return 0
    if operation == ACCOUNTING_RESERVE:
        var request_tokens = accounting_value(values_address, 6)
        var request_cost = accounting_value(values_address, 7)
        if request_tokens == 0 and request_cost == 0:
            result[] = 2
            return 0
        var held_tokens = accounting_checked_add(accounting_value(values_address, 0), accounting_value(values_address, 2))
        var held_cost = accounting_checked_add(accounting_value(values_address, 1), accounting_value(values_address, 3))
        var next_tokens = accounting_checked_add(held_tokens[0], request_tokens)
        var next_cost = accounting_checked_add(held_cost[0], request_cost)
        if held_tokens[1] == 1 or held_cost[1] == 1 or next_tokens[1] == 1 or next_cost[1] == 1:
            result[] = 1
        elif next_tokens[0] > accounting_value(values_address, 4):
            result[] = 3
        elif next_cost[0] > accounting_value(values_address, 5):
            result[] = 4
        else:
            var reserved_tokens = accounting_checked_add(accounting_value(values_address, 0), request_tokens)
            var reserved_cost = accounting_checked_add(accounting_value(values_address, 1), request_cost)
            if reserved_tokens[1] == 1 or reserved_cost[1] == 1:
                result[] = 1
            else:
                output[unsafe_offset=0] = reserved_tokens[0]
                output[unsafe_offset=1] = reserved_cost[0]
                output[unsafe_offset=2] = accounting_value(values_address, 2)
                output[unsafe_offset=3] = accounting_value(values_address, 3)
        return 0
    if accounting_value(values_address, 6) == 0 and accounting_value(values_address, 7) == 0:
        result[] = 1
        return 0
    if accounting_value(values_address, 6) > accounting_value(values_address, 4) or accounting_value(values_address, 7) > accounting_value(values_address, 5):
        result[] = 2
        return 0
    if accounting_value(values_address, 4) > accounting_value(values_address, 0) or accounting_value(values_address, 5) > accounting_value(values_address, 1):
        result[] = 3
        return 0
    var committed_tokens = accounting_checked_add(accounting_value(values_address, 2), accounting_value(values_address, 6))
    var committed_cost = accounting_checked_add(accounting_value(values_address, 3), accounting_value(values_address, 7))
    if committed_tokens[1] == 1 or committed_cost[1] == 1:
        result[] = 4
        return 0
    output[unsafe_offset=0] = accounting_value(values_address, 0) - accounting_value(values_address, 4) if accounting_value(values_address, 0) > accounting_value(values_address, 4) else 0
    output[unsafe_offset=1] = accounting_value(values_address, 1) - accounting_value(values_address, 5) if accounting_value(values_address, 1) > accounting_value(values_address, 5) else 0
    output[unsafe_offset=2] = committed_tokens[0]
    output[unsafe_offset=3] = committed_cost[0]
    return 0
