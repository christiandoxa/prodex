from std.memory import Pointer

comptime RUNTIME_TUNING_INT64_MAX: Int64 = 9223372036854775807


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


@export("prodex_runtime_proxy_preset_defaults_v1")
def prodex_runtime_proxy_preset_defaults_v1(
    preset: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if preset < 0 or preset > 3:
        return 1
    for index in range(19):
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
        output[unsafe_offset=12] = 4
        output[unsafe_offset=13] = 32
        output[unsafe_offset=14] = 64
        output[unsafe_offset=15] = 2
        output[unsafe_offset=16] = 16
        output[unsafe_offset=17] = 32
        output[unsafe_offset=18] = 1
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
        output[unsafe_offset=12] = 12
        output[unsafe_offset=13] = 96
        output[unsafe_offset=14] = 384
        output[unsafe_offset=15] = 6
        output[unsafe_offset=16] = 48
        output[unsafe_offset=17] = 96
        output[unsafe_offset=18] = 2
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
        output[unsafe_offset=12] = 16
        output[unsafe_offset=13] = 128
        output[unsafe_offset=14] = 512
        output[unsafe_offset=15] = 8
        output[unsafe_offset=16] = 64
        output[unsafe_offset=17] = 128
        output[unsafe_offset=18] = 3
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
