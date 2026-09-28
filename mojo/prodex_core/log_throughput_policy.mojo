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
    output[0] = Int64(reset)
    output[1] = Int64(append)
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
