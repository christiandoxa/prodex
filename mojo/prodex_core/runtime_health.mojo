from std.memory import Pointer
from runtime_math import INT64_MAX

comptime RUNTIME_PROFILE_HEALTH_SCORE_ABI_VERSION: Int64 = 1
comptime RUNTIME_PROFILE_HEALTH_SCORE_FIELD_COUNT: Int64 = 14
comptime RUNTIME_PROFILE_HEALTH_SCORE_MAX_COUNT: Int64 = 256
comptime UINT32_MAX: Int64 = 4_294_967_295


def runtime_profile_health_saturating_elapsed(now: Int64, updated_at: Int64) -> Int64:
    if now <= updated_at:
        return 0
    if updated_at < 0 and now > INT64_MAX + updated_at:
        return INT64_MAX
    return now - updated_at


def runtime_profile_health_effective_score(
    score: Int64, updated_at: Int64, now: Int64, decay_seconds: Int64
) -> Int64:
    var divisor = decay_seconds
    if divisor < 1:
        divisor = 1
    var decay = runtime_profile_health_saturating_elapsed(now, updated_at) / divisor
    if decay > UINT32_MAX:
        decay = UINT32_MAX
    if score <= decay:
        return 0
    return score - decay


def runtime_profile_health_saturating_add(left: Int64, right: Int64) -> Int64:
    if left >= UINT32_MAX - right:
        return UINT32_MAX
    return left + right


@export("prodex_runtime_profile_health_sort_key_batch_v1")
def prodex_runtime_profile_health_sort_key_batch_v1(
    abi_version: Int64,
    fields_address: UInt,
    output_address: UInt,
    count: Int64,
    now: Int64,
    health_decay_seconds: Int64,
    bad_pairing_decay_seconds: Int64,
    performance_decay_seconds: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROFILE_HEALTH_SCORE_ABI_VERSION:
        return 4
    if count < 0 or count > RUNTIME_PROFILE_HEALTH_SCORE_MAX_COUNT:
        return 1
    if count == 0:
        return 0
    if fields_address == 0 or output_address == 0:
        return 1

    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(count):
        var base = index * RUNTIME_PROFILE_HEALTH_SCORE_FIELD_COUNT
        for field in range(RUNTIME_PROFILE_HEALTH_SCORE_FIELD_COUNT):
            var value = fields[unsafe_offset=base + field]
            if field % 2 == 0 and (value < 0 or value > UINT32_MAX):
                return 2

        var global_score = runtime_profile_health_effective_score(
            fields[unsafe_offset=base],
            fields[unsafe_offset=base + 1],
            now,
            health_decay_seconds,
        )
        var route = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 2],
            fields[unsafe_offset=base + 3],
            now,
            health_decay_seconds,
        )
        var route_bad_pairing = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 4],
            fields[unsafe_offset=base + 5],
            now,
            bad_pairing_decay_seconds,
        )
        var coupled_health = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 6],
            fields[unsafe_offset=base + 7],
            now,
            health_decay_seconds,
        )
        var coupled_bad_pairing = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 8],
            fields[unsafe_offset=base + 9],
            now,
            bad_pairing_decay_seconds,
        )
        var route_performance = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 10],
            fields[unsafe_offset=base + 11],
            now,
            performance_decay_seconds,
        )
        var coupled_performance = runtime_profile_health_effective_score(
            fields[unsafe_offset=base + 12],
            fields[unsafe_offset=base + 13],
            now,
            performance_decay_seconds,
        ) / 2
        var coupling = runtime_profile_health_saturating_add(
            coupled_health, coupled_bad_pairing
        ) / 2
        var value = runtime_profile_health_saturating_add(global_score, route)
        value = runtime_profile_health_saturating_add(value, route_bad_pairing)
        value = runtime_profile_health_saturating_add(value, coupling)
        value = runtime_profile_health_saturating_add(value, route_performance)
        value = runtime_profile_health_saturating_add(value, coupled_performance)
        output[unsafe_offset=index] = value
    return 0
