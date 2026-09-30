from std.memory import Pointer

comptime LOG_THROUGHPUT_ABI_VERSION: Int64 = 1
comptime LOG_THROUGHPUT_OK: Int64 = 0
comptime LOG_THROUGHPUT_INVALID: Int64 = 1
comptime LOG_THROUGHPUT_ABI: Int64 = 4
comptime LOG_THROUGHPUT_MIN_SAMPLE_MS: UInt64 = 250


@export("prodex_log_throughput_sample_plan_v1")
def prodex_log_throughput_sample_plan_v1(
    abi_version: Int64,
    previous_present: Int64,
    previous_tokens: UInt64,
    previous_generation_ms: UInt64,
    current_tokens: UInt64,
    current_generation_ms: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if (
        (previous_present != 0 and previous_present != 1)
        or output_address == 0
    ):
        return LOG_THROUGHPUT_INVALID

    var reset = (
        previous_present == 1
        and (
            current_tokens < previous_tokens
            or current_generation_ms < previous_generation_ms
        )
    )
    var append = (
        previous_present == 0
        or reset
        or current_tokens > previous_tokens
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(reset)
    output[unsafe_offset=1] = Int64(append)
    return LOG_THROUGHPUT_OK


def throughput_write_rate(
    valid: Bool,
    rate: Float64,
    valid_address: UInt,
    rate_address: UInt,
) -> Int64:
    if valid_address == 0 or rate_address == 0:
        return LOG_THROUGHPUT_INVALID
    var valid_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(valid_address)
    )
    var rate_output = Pointer[mut=True, Float64, MutUntrackedOrigin](
        unsafe_from_address=Int(rate_address)
    )
    valid_output[] = Int64(valid)
    rate_output[] = rate if valid else 0.0
    return LOG_THROUGHPUT_OK


@export("prodex_log_throughput_completed_rate_v1")
def prodex_log_throughput_completed_rate_v1(
    abi_version: Int64,
    output_tokens: UInt64,
    generation_ms: UInt64,
    valid_address: UInt,
    rate_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if output_tokens == 0 or generation_ms == 0:
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var rate = (
        Float64(output_tokens) * 1000.0 / Float64(generation_ms)
    )
    return throughput_write_rate(
        True, rate, valid_address, rate_address
    )


@export("prodex_log_throughput_stream_rate_v1")
def prodex_log_throughput_stream_rate_v1(
    abi_version: Int64,
    first_tokens: UInt64,
    first_generation_ms: UInt64,
    last_tokens: UInt64,
    last_generation_ms: UInt64,
    valid_address: UInt,
    rate_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if (
        last_generation_ms < first_generation_ms
        or last_tokens < first_tokens
    ):
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var elapsed_ms = last_generation_ms - first_generation_ms
    var tokens = last_tokens - first_tokens
    if (
        elapsed_ms < LOG_THROUGHPUT_MIN_SAMPLE_MS
        or tokens == 0
    ):
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var rate = Float64(tokens) * 1000.0 / Float64(elapsed_ms)
    return throughput_write_rate(
        True, rate, valid_address, rate_address
    )


comptime LOG_RETENTION_POLICY_ABI_VERSION: Int64 = 1
comptime LOG_RETENTION_POLICY_BOUNDED_VALUE: Int64 = 1
comptime LOG_RETENTION_POLICY_ROTATION: Int64 = 2
comptime LOG_RETENTION_POLICY_EXPIRED: Int64 = 3
comptime LOG_RETENTION_POLICY_OVER_BUDGET: Int64 = 4
comptime LOG_RETENTION_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


@export("prodex_log_retention_policy_v1")
def prodex_log_retention_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: UInt64,
    input1: UInt64,
    input2: UInt64,
    input3: UInt64,
    input4: UInt64,
    input5: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_RETENTION_POLICY_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if output_address == 0:
        return LOG_THROUGHPUT_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    if operation == LOG_RETENTION_POLICY_BOUNDED_VALUE:
        # input0 present, input1 value, input2 default, input3 min, input4 max
        if input0 > 1 or input3 > input4:
            return LOG_THROUGHPUT_INVALID
        if input0 == 1 and input1 >= input3 and input1 <= input4:
            output[unsafe_offset=0] = input1
        else:
            output[unsafe_offset=0] = input2
        return LOG_THROUGHPUT_OK

    if operation == LOG_RETENTION_POLICY_ROTATION:
        # current_size, line_len, max_file_bytes
        var current_size = input0
        var line_len = input1
        var max_file_bytes = input2
        var next_size: UInt64
        if LOG_RETENTION_UINT64_MAX - current_size < line_len:
            next_size = LOG_RETENTION_UINT64_MAX
        else:
            next_size = current_size + line_len
        output[unsafe_offset=0] = UInt64(
            current_size > 0 and next_size > max_file_bytes
        )
        output[unsafe_offset=1] = UInt64(line_len > max_file_bytes)
        return LOG_THROUGHPUT_OK

    if operation == LOG_RETENTION_POLICY_EXPIRED:
        # modified_epoch, oldest_allowed encoded with sign-bit bias, removable
        if input2 > 1:
            return LOG_THROUGHPUT_INVALID
        output[unsafe_offset=0] = UInt64(input0 < input1 and input2 == 1)
        return LOG_THROUGHPUT_OK

    if operation == LOG_RETENTION_POLICY_OVER_BUDGET:
        # remaining_count, max_files, total_bytes, total_budget, already_removed, removable
        if input4 > 1 or input5 > 1:
            return LOG_THROUGHPUT_INVALID
        var within_budget = input0 <= input1 and input2 <= input3
        output[unsafe_offset=0] = UInt64(within_budget)
        output[unsafe_offset=1] = UInt64(
            not within_budget and input4 == 0 and input5 == 1
        )
        return LOG_THROUGHPUT_OK

    return LOG_THROUGHPUT_INVALID
