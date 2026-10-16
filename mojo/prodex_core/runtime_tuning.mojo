from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

# Fault-injection budgets are a deterministic transition. Rust owns environment
# acquisition and the mutex-protected per-key map; Mojo alone selects the step.
@export("prodex_runtime_fault_budget_step_v1")
def prodex_runtime_fault_budget_step_v1(
    abi_version: Int64,
    configured: UInt64,
    previous_remaining: UInt64,
    configuration_changed: Int64,
    next_remaining: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if abi_version != 1:
        return -3
    if configuration_changed != 0 and configuration_changed != 1:
        return -2

    if configured == 0:
        next_remaining[unsafe_offset=0] = 0
        return 0  # Clear this key.

    var remaining = previous_remaining
    if configuration_changed == 1:
        remaining = configured

    if remaining == 0:
        next_remaining[unsafe_offset=0] = 0
        return 1  # Do not inject.

    next_remaining[unsafe_offset=0] = remaining - 1
    return 2  # Inject once.


comptime RUNTIME_TUNING_INT64_MAX: Int64 = 9223372036854775807

comptime RUNTIME_SCALAR_CONFIG_ABI_VERSION: Int64 = 1
comptime RUNTIME_SCALAR_CONFIG_LOG_FORMAT: Int64 = 0
comptime RUNTIME_SCALAR_CONFIG_PROXY_PRESET: Int64 = 1
comptime RUNTIME_SCALAR_CONFIG_WEB_SEARCH: Int64 = 2
comptime RUNTIME_SCALAR_CONFIG_CLOCK_SOURCE: Int64 = 3
comptime RUNTIME_SCALAR_CONFIG_OPENAI_PROVIDER: Int64 = 4
comptime RUNTIME_SCALAR_CONFIG_LOG_RECORDING: Int64 = 5
comptime RUNTIME_SCALAR_CONFIG_MAX_BYTES: Int64 = 9_223_372_036_854_775_807
comptime RUNTIME_CONFIG_SCALAR_ABI_VERSION: Int64 = 1
comptime RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED: Int64 = 0
comptime RUNTIME_CONFIG_SCALAR_STRICT_UNSIGNED: Int64 = 1
comptime RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_SIGNED: Int64 = 2
comptime RUNTIME_CONFIG_SCALAR_STRICT_BOOL: Int64 = 3
comptime RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED: Int64 = 4
comptime RUNTIME_CONFIG_SCALAR_COMPAT_FLAG: Int64 = 5
comptime RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE: Int64 = 6
comptime RUNTIME_CONFIG_SCALAR_FLAG_TRIM: Int64 = 1
comptime RUNTIME_CONFIG_SCALAR_FLAG_ALLOW_ZERO: Int64 = 2
comptime RUNTIME_CONFIG_SCALAR_FLAG_CLAMP: Int64 = 4
comptime RUNTIME_CONFIG_SCALAR_STATUS_OK: Int64 = 0
comptime RUNTIME_CONFIG_SCALAR_STATUS_MISSING: Int64 = 1
comptime RUNTIME_CONFIG_SCALAR_STATUS_INVALID_UNICODE: Int64 = 2
comptime RUNTIME_CONFIG_SCALAR_STATUS_EMPTY: Int64 = 3
comptime RUNTIME_CONFIG_SCALAR_STATUS_WHITESPACE: Int64 = 4
comptime RUNTIME_CONFIG_SCALAR_STATUS_UNSIGNED_INTEGER: Int64 = 5
comptime RUNTIME_CONFIG_SCALAR_STATUS_NOT_POSITIVE: Int64 = 6
comptime RUNTIME_CONFIG_SCALAR_STATUS_POSITIVE_INTEGER: Int64 = 7
comptime RUNTIME_CONFIG_SCALAR_STATUS_INVALID_BOOL: Int64 = 8
comptime RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT: Int64 = 9
comptime RUNTIME_CONFIG_SCALAR_MAX_U64: UInt64 = 18_446_744_073_709_551_615
comptime RUNTIME_CONFIG_SCALAR_MAX_I64: UInt64 = 9_223_372_036_854_775_807
comptime RUNTIME_CONFIG_SCALAR_MIN_I64_MAGNITUDE: UInt64 = 9_223_372_036_854_775_808
comptime RUNTIME_CONFIG_SCALAR_RECORD_V1_SIZE: Int64 = 136
comptime RUNTIME_CONFIG_SCALAR_OUTPUT_UNSIGNED_OFFSET: Int64 = 120
comptime RUNTIME_CONFIG_SCALAR_OUTPUT_SIGNED_OFFSET: Int64 = 128
comptime RUNTIME_PROXY_PRESET_PLAN_ABI_VERSION: Int64 = 1
comptime RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT: Int64 = 39
comptime RUNTIME_PROXY_PRESET_PLAN_VALID_MASK: UInt64 = 549_755_813_887


@fieldwise_init
struct RuntimeConfigScalarRecordV1(Copyable):
    var operation: Int64
    var value_address: UInt64
    var value_length: Int64
    var value_present: Int64
    var value_valid_utf8: Int64
    var policy_present: Int64
    var policy_unsigned: UInt64
    var default_unsigned: UInt64
    var minimum_unsigned: UInt64
    var maximum_unsigned: UInt64
    var policy_signed: Int64
    var default_signed: Int64
    var minimum_signed: Int64
    var maximum_signed: Int64
    var flags: Int64
    var output_unsigned: UInt64
    var output_signed: Int64



def runtime_scalar_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def runtime_scalar_range_equals[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if (
            runtime_scalar_ascii_lower(source[unsafe_offset=start + index])
            != wanted[unsafe_offset=index]
        ):
            return False
    return True


def runtime_config_scalar_has_whitespace(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var source = rich_view_ptr(view)
    var index = start
    while index < end:
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var codepoint = rich_codepoint(source, index, width)
        if rich_unicode_space(codepoint):
            return True
        index += width
    return False


def runtime_config_scalar_parse_unsigned(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt64, _],
) -> Bool:
    if start >= end:
        return False
    var source = rich_view_ptr(view)
    var index = start
    var negative = False
    if source[unsafe_offset=index] == 43:
        index += 1
    elif source[unsafe_offset=index] == 45:
        negative = True
        index += 1
    if negative or index >= end:
        return False
    var value: UInt64 = 0
    while index < end:
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > (RUNTIME_CONFIG_SCALAR_MAX_U64 - digit) / 10:
            return False
        value = value * 10 + digit
        index += 1
    output[] = value
    return True


def runtime_config_scalar_parse_signed(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Bool:
    if start >= end:
        return False
    var source = rich_view_ptr(view)
    var index = start
    var negative = False
    if source[unsafe_offset=index] == 43:
        index += 1
    elif source[unsafe_offset=index] == 45:
        negative = True
        index += 1
    if index >= end:
        return False
    var limit = RUNTIME_CONFIG_SCALAR_MAX_I64
    if negative:
        limit = RUNTIME_CONFIG_SCALAR_MIN_I64_MAGNITUDE
    var magnitude: UInt64 = 0
    while index < end:
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if magnitude > (limit - digit) / 10:
            return False
        magnitude = magnitude * 10 + digit
        index += 1
    if negative:
        if magnitude == RUNTIME_CONFIG_SCALAR_MIN_I64_MAGNITUDE:
            output[] = -9_223_372_036_854_775_808
        else:
            output[] = -Int64(magnitude)
    else:
        output[] = Int64(magnitude)
    return True


def runtime_config_scalar_choose_unsigned(
    policy_present: Int64,
    policy_value: UInt64,
    default_value: UInt64,
    minimum: UInt64,
    maximum: UInt64,
    allow_zero: Bool,
    output: Pointer[mut=True, UInt64, _],
) -> Bool:
    var value = default_value
    if policy_present == 1 and (allow_zero or policy_value > 0):
        value = policy_value
    if value < minimum:
        value = minimum
    if value > maximum:
        value = maximum
    output[] = value
    return True


def runtime_config_scalar_choose_signed(
    policy_present: Int64,
    policy_value: Int64,
    default_value: Int64,
    minimum: Int64,
    maximum: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Bool:
    var value = default_value
    if policy_present == 1 and policy_value > 0:
        value = policy_value
    if value < minimum:
        value = minimum
    if value > maximum:
        value = maximum
    output[] = value
    return True


@export("prodex_runtime_config_scalar_v1")
def prodex_runtime_config_scalar_v1(
    abi_version: Int64,
    record_address: UInt,
    record_size: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_CONFIG_SCALAR_ABI_VERSION:
        return -3
    if record_address == 0 or record_size != RUNTIME_CONFIG_SCALAR_RECORD_V1_SIZE:
        return -2

    var record_pointer = Pointer[
        mut=False, RuntimeConfigScalarRecordV1, ImmUntrackedOrigin
    ](unsafe_from_address=Int(record_address))
    var record = record_pointer[].copy()
    var operation = record.operation
    var value_address = UInt(record.value_address)
    var value_length = record.value_length
    var value_present = record.value_present
    var value_valid_utf8 = record.value_valid_utf8
    var policy_present = record.policy_present
    var policy_unsigned = record.policy_unsigned
    var default_unsigned = record.default_unsigned
    var minimum_unsigned = record.minimum_unsigned
    var maximum_unsigned = record.maximum_unsigned
    var policy_signed = record.policy_signed
    var default_signed = record.default_signed
    var minimum_signed = record.minimum_signed
    var maximum_signed = record.maximum_signed
    var flags = record.flags

    if operation < RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED or operation > RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE:
        return -2
    if value_present != 0 and value_present != 1 or value_valid_utf8 != 0 and value_valid_utf8 != 1 or policy_present != 0 and policy_present != 1:
        return -2
    if value_length < 0 or value_length > RUNTIME_SCALAR_CONFIG_MAX_BYTES:
        return -2
    if value_present == 0 and value_length != 0:
        return -2
    if value_present == 0 and value_valid_utf8 != 1:
        return -2
    if value_valid_utf8 == 0 and value_present == 1 and (value_address != 0 or value_length != 0):
        return -2
    if value_present == 1 and value_valid_utf8 == 1 and value_length > 0 and value_address == 0:
        return -2
    if value_present == 1 and value_valid_utf8 == 1:
        var input_view = ProdexRichStringView(value_address, UInt(value_length))
        if not rich_view_valid(input_view, RUNTIME_SCALAR_CONFIG_MAX_BYTES):
            return -2
    if minimum_unsigned > maximum_unsigned or minimum_signed > maximum_signed:
        return -2

    if operation == RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED or operation == RUNTIME_CONFIG_SCALAR_STRICT_UNSIGNED or operation == RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED:
        if flags < 0 or flags > RUNTIME_CONFIG_SCALAR_FLAG_TRIM + RUNTIME_CONFIG_SCALAR_FLAG_ALLOW_ZERO + RUNTIME_CONFIG_SCALAR_FLAG_CLAMP:
            return -2
    elif operation == RUNTIME_CONFIG_SCALAR_COMPAT_FLAG or operation == RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE:
        if flags != 0 and flags != RUNTIME_CONFIG_SCALAR_FLAG_TRIM:
            return -2
    elif flags != 0:
        return -2
    if (
        operation != RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED
        and operation != RUNTIME_CONFIG_SCALAR_COMPAT_FLAG
        and operation != RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE
        and (flags & RUNTIME_CONFIG_SCALAR_FLAG_TRIM) != 0
    ):
        return -2
    if (
        operation != RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED
        and operation != RUNTIME_CONFIG_SCALAR_STRICT_UNSIGNED
        and operation != RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED
        and (flags & RUNTIME_CONFIG_SCALAR_FLAG_CLAMP) != 0
    ):
        return -2

    var output_unsigned = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(record_address + UInt(RUNTIME_CONFIG_SCALAR_OUTPUT_UNSIGNED_OFFSET))
    )
    var output_signed = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(record_address + UInt(RUNTIME_CONFIG_SCALAR_OUTPUT_SIGNED_OFFSET))
    )
    var allow_zero = (flags & RUNTIME_CONFIG_SCALAR_FLAG_ALLOW_ZERO) != 0
    var clamp = (flags & RUNTIME_CONFIG_SCALAR_FLAG_CLAMP) != 0

    if operation == RUNTIME_CONFIG_SCALAR_COMPAT_FLAG or operation == RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE:
        var default_bool = operation == RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE
        if value_present == 0:
            output_unsigned[] = UInt64(default_bool)
            return RUNTIME_CONFIG_SCALAR_STATUS_MISSING
        if value_valid_utf8 == 0:
            output_unsigned[] = UInt64(default_bool)
            if default_bool:
                return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
            return RUNTIME_CONFIG_SCALAR_STATUS_OK
        var bool_view = ProdexRichStringView(value_address, UInt(value_length))
        var bool_bounds = rich_trim_bounds(bool_view)
        var enabled = runtime_scalar_range_equals["1"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["true"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["yes"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["on"](bool_view, bool_bounds[0], bool_bounds[1])
        var disabled = runtime_scalar_range_equals["0"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["false"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["no"](bool_view, bool_bounds[0], bool_bounds[1]) or runtime_scalar_range_equals["off"](bool_view, bool_bounds[0], bool_bounds[1])
        if default_bool:
            output_unsigned[] = UInt64(not disabled)
        else:
            output_unsigned[] = UInt64(enabled)
        return RUNTIME_CONFIG_SCALAR_STATUS_OK

    if operation == RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_SIGNED:
        _ = runtime_config_scalar_choose_signed(policy_present, policy_signed, default_signed, minimum_signed, maximum_signed, output_signed)
        if value_present == 0:
            return RUNTIME_CONFIG_SCALAR_STATUS_MISSING
        if value_valid_utf8 == 0:
            return RUNTIME_CONFIG_SCALAR_STATUS_INVALID_UNICODE
        var signed_view = ProdexRichStringView(value_address, UInt(value_length))
        if value_length == 0:
            return RUNTIME_CONFIG_SCALAR_STATUS_EMPTY
        if runtime_config_scalar_has_whitespace(signed_view, 0, value_length):
            return RUNTIME_CONFIG_SCALAR_STATUS_WHITESPACE
        if not runtime_config_scalar_parse_signed(signed_view, 0, value_length, output_signed):
            return RUNTIME_CONFIG_SCALAR_STATUS_POSITIVE_INTEGER
        var parsed_signed = output_signed[]
        if parsed_signed <= 0:
            _ = runtime_config_scalar_choose_signed(policy_present, policy_signed, default_signed, minimum_signed, maximum_signed, output_signed)
            return RUNTIME_CONFIG_SCALAR_STATUS_NOT_POSITIVE
        if parsed_signed < minimum_signed:
            parsed_signed = minimum_signed
        elif parsed_signed > maximum_signed:
            parsed_signed = maximum_signed
        output_signed[] = parsed_signed
        return RUNTIME_CONFIG_SCALAR_STATUS_OK

    _ = runtime_config_scalar_choose_unsigned(policy_present, policy_unsigned, default_unsigned, minimum_unsigned, maximum_unsigned, allow_zero, output_unsigned)
    if operation == RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED:
        if value_present == 0:
            return RUNTIME_CONFIG_SCALAR_STATUS_MISSING
        if value_valid_utf8 == 0:
            return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
        var compatibility_view = ProdexRichStringView(value_address, UInt(value_length))
        var compatibility_bounds = (Int64(0), value_length)
        if (flags & RUNTIME_CONFIG_SCALAR_FLAG_TRIM) != 0:
            compatibility_bounds = rich_trim_bounds(compatibility_view)
        if not runtime_config_scalar_parse_unsigned(compatibility_view, compatibility_bounds[0], compatibility_bounds[1], output_unsigned):
            return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
        var parsed_compatibility = output_unsigned[]
        if not allow_zero and parsed_compatibility == 0:
            _ = runtime_config_scalar_choose_unsigned(0, 0, default_unsigned, minimum_unsigned, maximum_unsigned, allow_zero, output_unsigned)
            return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
        if parsed_compatibility > maximum_unsigned:
            if clamp:
                parsed_compatibility = maximum_unsigned
                output_unsigned[] = parsed_compatibility
                return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
            else:
                _ = runtime_config_scalar_choose_unsigned(0, 0, default_unsigned, minimum_unsigned, maximum_unsigned, allow_zero, output_unsigned)
                return RUNTIME_CONFIG_SCALAR_STATUS_COMPATIBILITY_DEFAULT
        if parsed_compatibility < minimum_unsigned:
            parsed_compatibility = minimum_unsigned
        output_unsigned[] = parsed_compatibility
        return RUNTIME_CONFIG_SCALAR_STATUS_OK

    if value_present == 0:
        return RUNTIME_CONFIG_SCALAR_STATUS_MISSING
    if value_valid_utf8 == 0:
        if operation == RUNTIME_CONFIG_SCALAR_STRICT_BOOL:
            return RUNTIME_CONFIG_SCALAR_STATUS_INVALID_UNICODE
        return RUNTIME_CONFIG_SCALAR_STATUS_INVALID_UNICODE
    var strict_view = ProdexRichStringView(value_address, UInt(value_length))
    if value_length == 0:
        return RUNTIME_CONFIG_SCALAR_STATUS_EMPTY
    if runtime_config_scalar_has_whitespace(strict_view, 0, value_length):
        if operation == RUNTIME_CONFIG_SCALAR_STRICT_BOOL:
            return RUNTIME_CONFIG_SCALAR_STATUS_WHITESPACE
        return RUNTIME_CONFIG_SCALAR_STATUS_WHITESPACE
    if operation == RUNTIME_CONFIG_SCALAR_STRICT_BOOL:
        var strict_enabled = runtime_scalar_range_equals["true"](strict_view, 0, value_length) or runtime_scalar_range_equals["1"](strict_view, 0, value_length) or runtime_scalar_range_equals["yes"](strict_view, 0, value_length) or runtime_scalar_range_equals["on"](strict_view, 0, value_length)
        var strict_disabled = runtime_scalar_range_equals["false"](strict_view, 0, value_length) or runtime_scalar_range_equals["0"](strict_view, 0, value_length) or runtime_scalar_range_equals["no"](strict_view, 0, value_length) or runtime_scalar_range_equals["off"](strict_view, 0, value_length)
        if not strict_enabled and not strict_disabled:
            output_unsigned[] = default_unsigned
            return RUNTIME_CONFIG_SCALAR_STATUS_INVALID_BOOL
        output_unsigned[] = UInt64(strict_enabled)
        return RUNTIME_CONFIG_SCALAR_STATUS_OK
    if not runtime_config_scalar_parse_unsigned(strict_view, 0, value_length, output_unsigned):
        return RUNTIME_CONFIG_SCALAR_STATUS_UNSIGNED_INTEGER
    var parsed_unsigned = output_unsigned[]
    if operation == RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED and parsed_unsigned == 0:
        _ = runtime_config_scalar_choose_unsigned(policy_present, policy_unsigned, default_unsigned, minimum_unsigned, maximum_unsigned, allow_zero, output_unsigned)
        return RUNTIME_CONFIG_SCALAR_STATUS_NOT_POSITIVE
    if parsed_unsigned < minimum_unsigned:
        parsed_unsigned = minimum_unsigned
    elif parsed_unsigned > maximum_unsigned:
        if clamp:
            parsed_unsigned = maximum_unsigned
        else:
            _ = runtime_config_scalar_choose_unsigned(policy_present, policy_unsigned, default_unsigned, minimum_unsigned, maximum_unsigned, allow_zero, output_unsigned)
            return RUNTIME_CONFIG_SCALAR_STATUS_UNSIGNED_INTEGER
    output_unsigned[] = parsed_unsigned
    return RUNTIME_CONFIG_SCALAR_STATUS_OK


@export("prodex_runtime_scalar_config_policy_v1")
def prodex_runtime_scalar_config_policy_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_SCALAR_CONFIG_ABI_VERSION:
        return -3
    if (
        operation < RUNTIME_SCALAR_CONFIG_LOG_FORMAT
        or operation > RUNTIME_SCALAR_CONFIG_LOG_RECORDING
        or length < 0
        or length > RUNTIME_SCALAR_CONFIG_MAX_BYTES
        or (length > 0 and address == 0)
    ):
        return -2
    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, RUNTIME_SCALAR_CONFIG_MAX_BYTES):
        return -2
    var bounds = rich_trim_bounds(view)

    if operation == RUNTIME_SCALAR_CONFIG_LOG_FORMAT:
        if runtime_scalar_range_equals["text"](view, bounds[0], bounds[1]):
            return 0
        if runtime_scalar_range_equals["json"](view, bounds[0], bounds[1]):
            return 1
        return -1

    if operation == RUNTIME_SCALAR_CONFIG_PROXY_PRESET:
        if bounds[0] != 0 or bounds[1] != length:
            return -1
        if runtime_scalar_range_equals["low"](view, 0, length):
            return 0
        if runtime_scalar_range_equals["default"](view, 0, length):
            return 1
        if (
            runtime_scalar_range_equals["many-terminals"](view, 0, length)
            or runtime_scalar_range_equals["many_terminals"](view, 0, length)
        ):
            return 2
        if runtime_scalar_range_equals["aggressive"](view, 0, length):
            return 3
        return -1

    if operation == RUNTIME_SCALAR_CONFIG_WEB_SEARCH:
        if runtime_scalar_range_equals["disabled"](view, 0, length):
            return 0
        if runtime_scalar_range_equals["cached"](view, 0, length):
            return 1
        if runtime_scalar_range_equals["indexed"](view, 0, length):
            return 2
        if runtime_scalar_range_equals["live"](view, 0, length):
            return 3
        return -1

    if operation == RUNTIME_SCALAR_CONFIG_CLOCK_SOURCE:
        if runtime_scalar_range_equals["system"](view, 0, length):
            return 0
        if runtime_scalar_range_equals["external"](view, 0, length):
            return 1
        return -1

    if operation == RUNTIME_SCALAR_CONFIG_OPENAI_PROVIDER:
        return Int64(runtime_scalar_range_equals["openai"](view, 0, length))

    return Int64(
        runtime_scalar_range_equals["1"](view, bounds[0], bounds[1])
        or runtime_scalar_range_equals["true"](view, bounds[0], bounds[1])
        or runtime_scalar_range_equals["yes"](view, bounds[0], bounds[1])
        or runtime_scalar_range_equals["on"](view, bounds[0], bounds[1])
    )


def runtime_tuning_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > RUNTIME_TUNING_INT64_MAX - right:
        return RUNTIME_TUNING_INT64_MAX
    return left + right


def runtime_tuning_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > RUNTIME_TUNING_INT64_MAX / right:
        return RUNTIME_TUNING_INT64_MAX
    return left * right


def runtime_tuning_clamp(value: Int64, minimum: Int64, maximum: Int64) -> Int64:
    if value < minimum:
        return minimum
    if value > maximum:
        return maximum
    return value


@export("prodex_runtime_tuning_defaults")
def prodex_runtime_tuning_defaults(
    parallelism: Int64,
    worker_count: Pointer[mut=True, Int64, _],
    long_lived_worker_count: Pointer[mut=True, Int64, _],
    probe_refresh_worker_count: Pointer[mut=True, Int64, _],
    async_worker_count: Pointer[mut=True, Int64, _],
    log_queue_capacity: Pointer[mut=True, Int64, _],
    websocket_connect_worker_count: Pointer[mut=True, Int64, _],
    websocket_dns_worker_count: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if parallelism < 0:
        return 1
    worker_count[unsafe_offset=0] = runtime_tuning_clamp(parallelism, 4, 12)
    long_lived_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 2),
        8,
        24,
    )
    probe_refresh_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        parallelism, 2, 4
    )
    async_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        parallelism, 2, 4
    )
    log_queue_capacity[unsafe_offset=0] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 256),
        1_024,
        8_192,
    )
    websocket_connect_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        parallelism, 4, 16
    )
    websocket_dns_worker_count[unsafe_offset=0] = runtime_tuning_clamp(
        parallelism, 2, 8
    )
    return 0


def runtime_tuning_lane_limit(
    override: Int64, fallback: Int64, global_limit: Int64
) -> Int64:
    var value = fallback
    if override > 0:
        value = override
    if value < 1:
        value = 1
    if value > global_limit:
        value = global_limit
    return value


def runtime_proxy_preset_defaults(
    preset: Int64,
    output: Pointer[mut=True, UInt64, _],
) -> Int64:
    if preset < 0 or preset > 3:
        return 1
    for index in range(RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT):
        output[unsafe_offset=index] = 0

    if preset == 0:
        output[unsafe_offset=0] = 4
        output[unsafe_offset=1] = 8
        output[unsafe_offset=2] = 2
        output[unsafe_offset=3] = 2
        output[unsafe_offset=4] = 128
        output[unsafe_offset=5] = 48
        output[unsafe_offset=6] = 2
        output[unsafe_offset=7] = 4
        output[unsafe_offset=8] = 36
        output[unsafe_offset=9] = 3
        output[unsafe_offset=10] = 8
        output[unsafe_offset=11] = 2
        output[unsafe_offset=22] = 4
        output[unsafe_offset=23] = 32
        output[unsafe_offset=24] = 64
        output[unsafe_offset=25] = 2
        output[unsafe_offset=26] = 16
        output[unsafe_offset=27] = 32
        output[unsafe_offset=38] = 1
    elif preset == 2:
        output[unsafe_offset=0] = 12
        output[unsafe_offset=1] = 32
        output[unsafe_offset=2] = 4
        output[unsafe_offset=3] = 4
        output[unsafe_offset=4] = 512
        output[unsafe_offset=5] = 160
        output[unsafe_offset=6] = 4
        output[unsafe_offset=7] = 8
        output[unsafe_offset=8] = 120
        output[unsafe_offset=9] = 8
        output[unsafe_offset=10] = 32
        output[unsafe_offset=11] = 8
        output[unsafe_offset=22] = 12
        output[unsafe_offset=23] = 96
        output[unsafe_offset=24] = 384
        output[unsafe_offset=25] = 6
        output[unsafe_offset=26] = 48
        output[unsafe_offset=27] = 96
        output[unsafe_offset=38] = 2
    elif preset == 3:
        output[unsafe_offset=0] = 24
        output[unsafe_offset=1] = 96
        output[unsafe_offset=2] = 8
        output[unsafe_offset=3] = 8
        output[unsafe_offset=4] = 1024
        output[unsafe_offset=5] = 384
        output[unsafe_offset=6] = 8
        output[unsafe_offset=7] = 16
        output[unsafe_offset=8] = 288
        output[unsafe_offset=9] = 16
        output[unsafe_offset=10] = 96
        output[unsafe_offset=11] = 16
        output[unsafe_offset=22] = 16
        output[unsafe_offset=23] = 128
        output[unsafe_offset=24] = 512
        output[unsafe_offset=25] = 8
        output[unsafe_offset=26] = 64
        output[unsafe_offset=27] = 128
        output[unsafe_offset=38] = 3
    return 0


@export("prodex_runtime_proxy_preset_plan_v1")
def prodex_runtime_proxy_preset_plan_v1(
    abi_version: Int64,
    configured_preset: Int64,
    environment_preset: Int64,
    override_presence_mask: UInt64,
    override_values_address: UInt,
    override_critical_floor: Int64,
    output_presence_mask_address: UInt,
    output_values_address: UInt,
    output_effective_preset_address: UInt,
    output_critical_floor_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROXY_PRESET_PLAN_ABI_VERSION:
        return -3
    if (
        configured_preset < -1
        or configured_preset > 3
        or environment_preset < -1
        or environment_preset > 3
        or override_presence_mask > RUNTIME_PROXY_PRESET_PLAN_VALID_MASK
        or override_values_address == 0
        or output_presence_mask_address == 0
        or output_values_address == 0
        or output_effective_preset_address == 0
        or output_critical_floor_address == 0
    ):
        return -2

    var output_presence_mask = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_presence_mask_address))
    var output_values = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_values_address))
    var output_effective_preset = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_effective_preset_address))
    var output_critical_floor = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_critical_floor_address))
    var override_values = Pointer[
        mut=False, UInt64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(override_values_address))

    output_presence_mask[] = 0
    output_effective_preset[] = -1
    output_critical_floor[] = 0
    for index in range(RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT):
        output_values[unsafe_offset=index] = 0

    var selected_preset = environment_preset
    if selected_preset == -1:
        selected_preset = configured_preset
    if selected_preset != -1:
        output_effective_preset[] = selected_preset
        if runtime_proxy_preset_defaults(selected_preset, output_values) != 0:
            return -2
        var default_bit = UInt64(1)
        for index in range(RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT):
            if output_values[unsafe_offset=index] > 0:
                output_presence_mask[] = output_presence_mask[] | default_bit
            default_bit = default_bit * 2

    var override_bit = UInt64(1)
    for index in range(RUNTIME_PROXY_PRESET_PLAN_FIELD_COUNT):
        if override_presence_mask & override_bit != 0:
            output_presence_mask[] = output_presence_mask[] | override_bit
            if index == 37:
                output_critical_floor[] = override_critical_floor
            else:
                output_values[unsafe_offset=index] = override_values[unsafe_offset=index]
        override_bit = override_bit * 2
    return 0


@export("prodex_runtime_tuning_capacity_defaults")
def prodex_runtime_tuning_capacity_defaults(
    parallelism: Int64,
    global_limit: Int64,
    worker_count: Int64,
    long_lived_worker_count: Int64,
    responses_override: Int64,
    compact_override: Int64,
    websocket_override: Int64,
    standard_override: Int64,
    websocket_connect_queue_override: Int64,
    websocket_dns_queue_override: Int64,
    long_lived_queue_capacity: Pointer[mut=True, Int64, _],
    active_request_limit: Pointer[mut=True, Int64, _],
    log_queue_capacity: Pointer[mut=True, Int64, _],
    websocket_connect_queue_capacity: Pointer[mut=True, Int64, _],
    websocket_connect_overflow_capacity: Pointer[mut=True, Int64, _],
    websocket_dns_queue_capacity: Pointer[mut=True, Int64, _],
    websocket_dns_overflow_capacity: Pointer[mut=True, Int64, _],
    responses_lane_limit: Pointer[mut=True, Int64, _],
    compact_lane_limit: Pointer[mut=True, Int64, _],
    websocket_lane_limit: Pointer[mut=True, Int64, _],
    standard_lane_limit: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if parallelism < 0 or global_limit < 0 or worker_count < 0 or long_lived_worker_count < 0:
        return 1
    if responses_override < 0 or compact_override < 0 or websocket_override < 0 or standard_override < 0 or websocket_connect_queue_override < 0 or websocket_dns_queue_override < 0:
        return 1
    var bounded_global = global_limit
    if bounded_global < 1:
        bounded_global = 1

    long_lived_queue_capacity[] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(long_lived_worker_count, 8),
        128,
        1_024,
    )
    active_request_limit[] = runtime_tuning_clamp(
        runtime_tuning_saturating_add(
            worker_count,
            runtime_tuning_saturating_mul(long_lived_worker_count, 3),
        ),
        64,
        512,
    )
    log_queue_capacity[] = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 256),
        1_024,
        8_192,
    )

    var websocket_connect_workers = runtime_tuning_clamp(parallelism, 4, 16)
    var websocket_connect_queue = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(websocket_connect_workers, 8), 32, 128
    )
    if websocket_connect_queue_override > 0:
        websocket_connect_queue = websocket_connect_queue_override
    websocket_connect_queue_capacity[] = websocket_connect_queue
    var websocket_connect_overflow = runtime_tuning_saturating_mul(
        websocket_connect_queue, 4
    )
    if websocket_connect_overflow < websocket_connect_workers:
        websocket_connect_overflow = websocket_connect_workers
    websocket_connect_overflow_capacity[] = runtime_tuning_clamp(
        websocket_connect_overflow, 32, 512
    )

    var websocket_dns_queue = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(parallelism, 4), 16, 64
    )
    if websocket_dns_queue_override > 0:
        websocket_dns_queue = websocket_dns_queue_override
    websocket_dns_queue_capacity[] = websocket_dns_queue
    var websocket_dns_overflow = runtime_tuning_saturating_mul(
        websocket_dns_queue, 2
    )
    if websocket_dns_overflow < parallelism:
        websocket_dns_overflow = parallelism
    websocket_dns_overflow_capacity[] = runtime_tuning_clamp(
        websocket_dns_overflow, 16, 128
    )

    var responses_fallback = runtime_tuning_saturating_mul(bounded_global, 3) / 4
    responses_fallback = runtime_tuning_clamp(responses_fallback, 4, bounded_global)
    responses_lane_limit[] = runtime_tuning_lane_limit(
        responses_override, responses_fallback, bounded_global
    )

    var compact_fallback = runtime_tuning_clamp(bounded_global / 4, 2, 6)
    if compact_fallback > bounded_global:
        compact_fallback = bounded_global
    compact_lane_limit[] = runtime_tuning_lane_limit(
        compact_override, compact_fallback, bounded_global
    )
    var websocket_fallback = long_lived_worker_count
    if websocket_fallback < 2:
        websocket_fallback = 2
    if websocket_fallback > bounded_global:
        websocket_fallback = bounded_global
    websocket_lane_limit[] = runtime_tuning_lane_limit(
        websocket_override, websocket_fallback, bounded_global
    )
    var standard_fallback = runtime_tuning_clamp(
        runtime_tuning_saturating_mul(worker_count, 2), 8, 24
    )
    if standard_fallback > bounded_global:
        standard_fallback = bounded_global
    standard_lane_limit[] = runtime_tuning_lane_limit(
        standard_override, standard_fallback, bounded_global
    )
    return 0


@export("prodex_runtime_tuning_capacity_default_v1")
def prodex_runtime_tuning_capacity_default_v1(
    version: Int64, operation: Int64, value: Int64, secondary: Int64
) abi("C") -> Int64:
    if version != 1:
        return -3
    if operation < 0 or operation > 6 or value < 0 or secondary < 0:
        return -2
    if operation == 0:
        return runtime_tuning_clamp(runtime_tuning_saturating_mul(value, 8), 128, 1_024)
    if operation == 1:
        return runtime_tuning_clamp(runtime_tuning_saturating_add(value, runtime_tuning_saturating_mul(secondary, 3)), 64, 512)
    if operation == 2:
        return runtime_tuning_clamp(runtime_tuning_saturating_mul(value, 256), 1_024, 8_192)
    if operation == 3:
        return runtime_tuning_clamp(runtime_tuning_saturating_mul(runtime_tuning_clamp(value, 4, 16), 8), 32, 128)
    if operation == 5:
        return runtime_tuning_clamp(runtime_tuning_saturating_mul(value, 4), 16, 64)
    var queue = secondary
    if queue == 0:
        queue = prodex_runtime_tuning_capacity_default_v1(1, operation - 1, value, 0)
    if operation == 4:
        return runtime_tuning_clamp(max(runtime_tuning_saturating_mul(queue, 4), runtime_tuning_clamp(value, 4, 16)), 32, 512)
    return runtime_tuning_clamp(max(runtime_tuning_saturating_mul(queue, 2), value), 16, 128)
