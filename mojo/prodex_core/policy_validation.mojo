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

comptime GATEWAY_ADMIN_POLICY_ABI_VERSION: Int64 = 1
comptime GATEWAY_ADMIN_POLICY_STATUS_OK: Int64 = 0
comptime GATEWAY_ADMIN_POLICY_STATUS_INVALID: Int64 = 1
comptime GATEWAY_ADMIN_POLICY_STATUS_ABI: Int64 = 4
comptime RETENTION_DEFAULT_DAYS: UInt64 = 365
comptime RETENTION_MIN_DAYS: UInt64 = 30
comptime RETENTION_MAX_DAYS: UInt64 = 3_650
comptime RETENTION_DEFAULT_LIMIT: UInt64 = 100
comptime RETENTION_MAX_LIMIT: UInt64 = 1_000
comptime RETENTION_MILLIS_PER_DAY: UInt64 = 86_400_000
comptime AUDIT_DECISION_ABI_VERSION: Int64 = 1
comptime AUDIT_TIME_RANGE_CONTAINS: Int64 = 0
comptime AUDIT_COMPARE_POSITIONS: Int64 = 1
comptime AUDIT_RETENTION_CUTOFF: Int64 = 2
comptime AUDIT_EVENT_EXPIRED: Int64 = 3
comptime AUDIT_HOLD_ACTIVE: Int64 = 4


@export("prodex_mojo_policy_refresh_decision_v1")
def prodex_mojo_policy_refresh_decision_v1(
    abi_version: Int64,
    refresh_after_unix_ms: UInt64,
    stale_after_unix_ms: UInt64,
    expires_after_unix_ms: UInt64,
    now_unix_ms: UInt64,
    active_invalidated: Int64,
    last_known_good_present: Int64,
    last_known_good_invalidated: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return GATEWAY_ADMIN_POLICY_STATUS_ABI
    if output_address == 0 or active_invalidated < 0 or active_invalidated > 1 or last_known_good_present < 0 or last_known_good_present > 1 or last_known_good_invalidated < 0 or last_known_good_invalidated > 1 or refresh_after_unix_ms > stale_after_unix_ms or stale_after_unix_ms > expires_after_unix_ms:
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if active_invalidated == 1:
        output[] = 4
    elif now_unix_ms >= expires_after_unix_ms:
        output[] = 3
    elif now_unix_ms >= stale_after_unix_ms and last_known_good_present == 1 and last_known_good_invalidated == 0:
        output[] = 2
    elif now_unix_ms >= refresh_after_unix_ms:
        output[] = 1
    else:
        output[] = 0
    return GATEWAY_ADMIN_POLICY_STATUS_OK


@export("prodex_mojo_migration_step_order_v1")
def prodex_mojo_migration_step_order_v1(
    abi_version: Int64,
    steps_address: UInt,
    step_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return GATEWAY_ADMIN_POLICY_STATUS_ABI
    if step_count < 0 or output_address == 0 or (step_count > 0 and steps_address == 0):
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var steps = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(steps_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    var saw_expand = False
    var saw_backfill = False
    for index in range(step_count):
        var step = steps[unsafe_offset=index]
        if step == 0:
            saw_expand = True
        elif step == 1:
            if not saw_expand:
                output[] = 1
                return GATEWAY_ADMIN_POLICY_STATUS_OK
            saw_backfill = True
        elif step == 2:
            if not saw_backfill:
                output[] = 2
                return GATEWAY_ADMIN_POLICY_STATUS_OK
        elif step == 3:
            if not saw_expand:
                output[] = 3
                return GATEWAY_ADMIN_POLICY_STATUS_OK
        else:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    return GATEWAY_ADMIN_POLICY_STATUS_OK


@export("prodex_mojo_audit_decision_v1")
def prodex_mojo_audit_decision_v1(
    abi_version: Int64,
    operation: Int64,
    values_address: UInt,
    value_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != AUDIT_DECISION_ABI_VERSION:
        return GATEWAY_ADMIN_POLICY_STATUS_ABI
    if values_address == 0 or output_address == 0:
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var values = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if operation == AUDIT_TIME_RANGE_CONTAINS:
        if value_count != 5 or values[0] > 1 or values[2] > 1:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
        output[] = UInt64(
            (values[0] == 0 or values[4] >= values[1])
            and (values[2] == 0 or values[4] <= values[3])
        )
        return GATEWAY_ADMIN_POLICY_STATUS_OK
    if operation == AUDIT_COMPARE_POSITIONS:
        if value_count != 7 or values[6] > 1:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
        if values[0] != values[3]:
            var left_before = values[0] < values[3]
            if values[6] == 1:
                left_before = not left_before
            output[] = 0 if left_before else 2
        elif values[1] != values[4]:
            output[] = 0 if values[1] < values[4] else 2
        elif values[2] != values[5]:
            output[] = 0 if values[2] < values[5] else 2
        else:
            output[] = 1
        return GATEWAY_ADMIN_POLICY_STATUS_OK
    if operation == AUDIT_RETENTION_CUTOFF:
        if value_count != 3 or values[1] > UINT64_MAX // RETENTION_MILLIS_PER_DAY:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
        var retention_ms = values[1] * RETENTION_MILLIS_PER_DAY
        var cutoff = values[0] - retention_ms if values[0] > retention_ms else 0
        output[] = cutoff if cutoff > values[2] else values[2]
        return GATEWAY_ADMIN_POLICY_STATUS_OK
    if operation == AUDIT_EVENT_EXPIRED:
        if value_count != 2:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
        output[] = UInt64(values[0] < values[1])
        return GATEWAY_ADMIN_POLICY_STATUS_OK
    if operation == AUDIT_HOLD_ACTIVE:
        if value_count != 3 or values[0] > 1:
            return GATEWAY_ADMIN_POLICY_STATUS_INVALID
        output[] = UInt64(values[0] == 0 or values[2] <= values[1])
        return GATEWAY_ADMIN_POLICY_STATUS_OK
    return GATEWAY_ADMIN_POLICY_STATUS_INVALID


def gateway_admin_policy_status(abi_version: Int64) -> Int64:
    if abi_version != GATEWAY_ADMIN_POLICY_ABI_VERSION:
        return GATEWAY_ADMIN_POLICY_STATUS_ABI
    return GATEWAY_ADMIN_POLICY_STATUS_OK


def gateway_admin_policy_option_valid(present: Int64) -> Bool:
    return present == 0 or present == 1


def gateway_admin_policy_retention_plan_impl(
    abi_version: Int64,
    retention_days: UInt64,
    retention_present: Int64,
    event_count: Int64,
    normalized_days_address: UInt,
    batch_limit_address: UInt,
) -> Int64:
    var status = gateway_admin_policy_status(abi_version)
    if status != GATEWAY_ADMIN_POLICY_STATUS_OK:
        return status
    if (
        not gateway_admin_policy_option_valid(retention_present)
        or event_count <= 0
        or event_count > Int64(RETENTION_MAX_LIMIT)
        or normalized_days_address == 0
        or batch_limit_address == 0
    ):
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var normalized_days = retention_days if retention_present == 1 else RETENTION_DEFAULT_DAYS
    if normalized_days < RETENTION_MIN_DAYS or normalized_days > RETENTION_MAX_DAYS:
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var normalized_days_ptr = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(normalized_days_address))
    var batch_limit_ptr = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(batch_limit_address))
    normalized_days_ptr[] = normalized_days
    batch_limit_ptr[] = UInt64(event_count)
    return GATEWAY_ADMIN_POLICY_STATUS_OK


def gateway_admin_policy_limit_plan_impl(
    abi_version: Int64,
    requested_limit: UInt64,
    requested_present: Int64,
    normalized_limit_address: UInt,
) -> Int64:
    var status = gateway_admin_policy_status(abi_version)
    if status != GATEWAY_ADMIN_POLICY_STATUS_OK:
        return status
    if (
        not gateway_admin_policy_option_valid(requested_present)
        or normalized_limit_address == 0
    ):
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var normalized_limit = requested_limit if requested_present == 1 else RETENTION_DEFAULT_LIMIT
    if normalized_limit == 0 or normalized_limit > RETENTION_MAX_LIMIT:
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var normalized_limit_ptr = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(normalized_limit_address))
    normalized_limit_ptr[] = normalized_limit
    return GATEWAY_ADMIN_POLICY_STATUS_OK


def gateway_admin_policy_cutoff_impl(
    abi_version: Int64,
    now_unix_ms: UInt64,
    retention_days: UInt64,
    cutoff_address: UInt,
) -> Int64:
    var status = gateway_admin_policy_status(abi_version)
    if status != GATEWAY_ADMIN_POLICY_STATUS_OK:
        return status
    if (
        retention_days < RETENTION_MIN_DAYS
        or retention_days > RETENTION_MAX_DAYS
        or cutoff_address == 0
    ):
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var retention_ms = retention_days * RETENTION_MILLIS_PER_DAY
    var cutoff = now_unix_ms - retention_ms if now_unix_ms > retention_ms else 0
    var cutoff_ptr = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(cutoff_address)
    )
    cutoff_ptr[] = cutoff
    return GATEWAY_ADMIN_POLICY_STATUS_OK


def gateway_admin_policy_purge_result_impl(
    abi_version: Int64,
    requested: UInt64,
    purged: UInt64,
    protected_or_ineligible_address: UInt,
) -> Int64:
    var status = gateway_admin_policy_status(abi_version)
    if status != GATEWAY_ADMIN_POLICY_STATUS_OK:
        return status
    if purged > requested or protected_or_ineligible_address == 0:
        return GATEWAY_ADMIN_POLICY_STATUS_INVALID
    var protected_or_ineligible_ptr = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(protected_or_ineligible_address))
    protected_or_ineligible_ptr[] = requested - purged
    return GATEWAY_ADMIN_POLICY_STATUS_OK


@export("prodex_mojo_gateway_admin_retention_plan_v1")
def prodex_mojo_gateway_admin_retention_plan_v1(
    abi_version: Int64,
    retention_days: UInt64,
    retention_present: Int64,
    event_count: Int64,
    normalized_days_address: UInt,
    batch_limit_address: UInt,
) abi("C") -> Int64:
    return gateway_admin_policy_retention_plan_impl(
        abi_version,
        retention_days,
        retention_present,
        event_count,
        normalized_days_address,
        batch_limit_address,
    )


@export("prodex_mojo_gateway_admin_limit_plan_v1")
def prodex_mojo_gateway_admin_limit_plan_v1(
    abi_version: Int64,
    requested_limit: UInt64,
    requested_present: Int64,
    normalized_limit_address: UInt,
) abi("C") -> Int64:
    return gateway_admin_policy_limit_plan_impl(
        abi_version,
        requested_limit,
        requested_present,
        normalized_limit_address,
    )


@export("prodex_mojo_gateway_admin_cutoff_v1")
def prodex_mojo_gateway_admin_cutoff_v1(
    abi_version: Int64,
    now_unix_ms: UInt64,
    retention_days: UInt64,
    cutoff_address: UInt,
) abi("C") -> Int64:
    return gateway_admin_policy_cutoff_impl(
        abi_version, now_unix_ms, retention_days, cutoff_address
    )


@export("prodex_mojo_gateway_admin_purge_result_v1")
def prodex_mojo_gateway_admin_purge_result_v1(
    abi_version: Int64,
    requested: UInt64,
    purged: UInt64,
    protected_or_ineligible_address: UInt,
) abi("C") -> Int64:
    return gateway_admin_policy_purge_result_impl(
        abi_version, requested, purged, protected_or_ineligible_address
    )


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
comptime ACCOUNTING_RECORD: Int64 = 6
comptime ACCOUNTING_IS_EXPIRED: Int64 = 7
comptime ACCOUNTING_RELEASE: Int64 = 8
comptime ACCOUNTING_RECONCILE: Int64 = 9


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
    output[unsafe_offset=2] = 0
    output[unsafe_offset=3] = 0
    result[] = 0
    var required: Int64 = 4
    if operation == ACCOUNTING_SNAPSHOT_AVAILABLE:
        required = 6
    elif operation == ACCOUNTING_RESERVE or operation == ACCOUNTING_COMMIT or operation == ACCOUNTING_RECONCILE:
        required = 8
    elif operation == ACCOUNTING_IS_EXPIRED:
        required = 2
    elif operation == ACCOUNTING_RELEASE:
        required = 6
    if operation < ACCOUNTING_USAGE_ADD or operation > ACCOUNTING_RECONCILE or value_count < required:
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
    if operation == ACCOUNTING_RECORD:
        var created_at = accounting_value(values_address, 0)
        var ttl = accounting_value(values_address, 1)
        if ttl == 0:
            result[] = 1
        elif accounting_value(values_address, 2) == 0 and accounting_value(values_address, 3) == 0:
            result[] = 2
        elif created_at > UINT64_MAX - ttl:
            result[] = 3
        else:
            output[unsafe_offset=0] = created_at + ttl
        return 0
    if operation == ACCOUNTING_IS_EXPIRED:
        output[unsafe_offset=0] = UInt64(accounting_value(values_address, 0) >= accounting_value(values_address, 1))
        return 0
    if operation == ACCOUNTING_RELEASE:
        if accounting_value(values_address, 4) < accounting_value(values_address, 5):
            result[] = 1
        elif accounting_value(values_address, 2) > accounting_value(values_address, 0) or accounting_value(values_address, 3) > accounting_value(values_address, 1):
            result[] = 2
        else:
            output[unsafe_offset=0] = accounting_value(values_address, 0) - accounting_value(values_address, 2)
            output[unsafe_offset=1] = accounting_value(values_address, 1) - accounting_value(values_address, 3)
        return 0
    if operation == ACCOUNTING_RECONCILE:
        if accounting_value(values_address, 4) > accounting_value(values_address, 0) or accounting_value(values_address, 5) > accounting_value(values_address, 1):
            result[] = 1
            return 0
        var reconciled_tokens = accounting_checked_add(accounting_value(values_address, 2), accounting_value(values_address, 6))
        var reconciled_cost = accounting_checked_add(accounting_value(values_address, 3), accounting_value(values_address, 7))
        if reconciled_tokens[1] == 1 or reconciled_cost[1] == 1:
            result[] = 2
        else:
            output[unsafe_offset=0] = accounting_value(values_address, 0) - accounting_value(values_address, 4)
            output[unsafe_offset=1] = accounting_value(values_address, 1) - accounting_value(values_address, 5)
            output[unsafe_offset=2] = reconciled_tokens[0]
            output[unsafe_offset=3] = reconciled_cost[0]
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
