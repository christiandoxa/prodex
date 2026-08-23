from std.memory import Pointer

comptime RUNTIME_TUNING_INT64_MAX: Int64 = 9223372036854775807


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
