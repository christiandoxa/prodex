from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime POLICY_NUMERIC_NON_ZERO: Int64 = 0
comptime POLICY_NUMERIC_RANGE: Int64 = 1
comptime POLICY_NUMERIC_RELATION_LE: Int64 = 2
comptime UINT64_MAX: UInt64 = 18446744073709551615

comptime POLICY_TEXT_ROUTE_STRATEGY: Int64 = 0
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
    if kind == POLICY_TEXT_ROUTE_STRATEGY:
        for literal in [
            StringSlice("fallback"), StringSlice("ordered-fallback"), StringSlice("ordered_fallback"),
            StringSlice("round-robin"), StringSlice("round_robin"), StringSlice("rr"), StringSlice("first"),
            StringSlice("first-available"), StringSlice("first_available"), StringSlice("ordered"),
            StringSlice("least-busy"), StringSlice("least_busy"), StringSlice("least-busy-model"),
            StringSlice("least_busy_model"), StringSlice("lowest-cost"), StringSlice("lowest_cost"),
            StringSlice("cost"), StringSlice("cost-optimized"), StringSlice("cost_optimized"),
            StringSlice("lowest-latency"), StringSlice("lowest_latency"), StringSlice("latency"),
            StringSlice("latency-optimized"), StringSlice("latency_optimized"), StringSlice("rpm"),
            StringSlice("rpm-headroom"), StringSlice("rpm_headroom"), StringSlice("tpm"),
            StringSlice("tpm-headroom"), StringSlice("tpm_headroom"),
        ]:
            if policy_text_equals(view, literal):
                return True
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
    if kind < POLICY_TEXT_ROUTE_STRATEGY or kind > POLICY_TEXT_WEBHOOK_PHASE:
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
