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

comptime RUNTIME_HEALTH_SCALAR_ABI_VERSION: Int64 = 1
comptime RUNTIME_HEALTH_SCALAR_EFFECTIVE_SCORE: Int64 = 1
comptime RUNTIME_HEALTH_SCALAR_COUPLING_SCORE: Int64 = 2
comptime RUNTIME_HEALTH_SCALAR_PERFORMANCE_SCORE: Int64 = 3
comptime RUNTIME_HEALTH_SCALAR_BACKOFF_SORT_KEY: Int64 = 4
comptime RUNTIME_HEALTH_SCALAR_HALF_OPEN_SECONDS: Int64 = 5
comptime RUNTIME_HEALTH_SCALAR_OPEN_SECONDS: Int64 = 6
comptime RUNTIME_HEALTH_SCALAR_SOFTEN_UNTIL: Int64 = 7

def runtime_health_saturating_shift_multiplier(exponent: Int64) -> Int64:
    if exponent <= 0:
        return 1
    if exponent >= 62:
        return INT64_MAX
    return Int64(1) << exponent

def runtime_health_active_until(until: Int64, now: Int64) -> Int64:
    return until if until > now else -1

@export("prodex_runtime_health_scalar_v1")
def prodex_runtime_health_scalar_v1(
    abi_version: Int64,
    operation: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
    output_count: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_HEALTH_SCALAR_ABI_VERSION:
        return 4
    if field_count < 0 or field_count > 16 or output_count < 1 or output_count > 4:
        return 1
    if fields_address == 0 or output_address == 0:
        return 1
    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(output_count):
        output[unsafe_offset=index] = 0

    if operation == RUNTIME_HEALTH_SCALAR_EFFECTIVE_SCORE:
        if field_count != 4:
            return 1
        var score = fields[unsafe_offset=0]
        if score < 0 or score > UINT32_MAX:
            return 2
        output[unsafe_offset=0] = runtime_profile_health_effective_score(
            score,
            fields[unsafe_offset=1],
            fields[unsafe_offset=2],
            fields[unsafe_offset=3],
        )
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_COUPLING_SCORE:
        if field_count != 7:
            return 1
        var route_score = fields[unsafe_offset=0]
        var bad_score = fields[unsafe_offset=2]
        if route_score < 0 or route_score > UINT32_MAX or bad_score < 0 or bad_score > UINT32_MAX:
            return 2
        var route = runtime_profile_health_effective_score(
            route_score,
            fields[unsafe_offset=1],
            fields[unsafe_offset=4],
            fields[unsafe_offset=5],
        )
        var bad = runtime_profile_health_effective_score(
            bad_score,
            fields[unsafe_offset=3],
            fields[unsafe_offset=4],
            fields[unsafe_offset=6],
        )
        output[unsafe_offset=0] = runtime_profile_health_saturating_add(route, bad) / 2
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_PERFORMANCE_SCORE:
        if field_count != 6:
            return 1
        var route_score = fields[unsafe_offset=0]
        var coupled_score = fields[unsafe_offset=2]
        if route_score < 0 or route_score > UINT32_MAX or coupled_score < 0 or coupled_score > UINT32_MAX:
            return 2
        var route = runtime_profile_health_effective_score(
            route_score,
            fields[unsafe_offset=1],
            fields[unsafe_offset=4],
            fields[unsafe_offset=5],
        )
        var coupled = runtime_profile_health_effective_score(
            coupled_score,
            fields[unsafe_offset=3],
            fields[unsafe_offset=4],
            fields[unsafe_offset=5],
        ) / 2
        output[unsafe_offset=0] = runtime_profile_health_saturating_add(route, coupled)
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_BACKOFF_SORT_KEY:
        if field_count != 4 or output_count != 4:
            return 1
        var circuit = runtime_health_active_until(fields[unsafe_offset=0], fields[unsafe_offset=3])
        var transport = runtime_health_active_until(fields[unsafe_offset=1], fields[unsafe_offset=3])
        var retry = runtime_health_active_until(fields[unsafe_offset=2], fields[unsafe_offset=3])
        if circuit < 0 and transport < 0 and retry < 0:
            return 0
        if circuit >= 0 and transport < 0 and retry < 0:
            output[unsafe_offset=0] = 1
            output[unsafe_offset=1] = circuit
            return 0
        if circuit < 0 and transport >= 0 and retry < 0:
            output[unsafe_offset=0] = 2
            output[unsafe_offset=1] = transport
            return 0
        if circuit < 0 and transport < 0 and retry >= 0:
            output[unsafe_offset=0] = 3
            output[unsafe_offset=1] = retry
            return 0
        if circuit >= 0 and transport >= 0 and retry < 0:
            output[unsafe_offset=0] = 4
            output[unsafe_offset=1] = min(circuit, transport)
            output[unsafe_offset=2] = max(circuit, transport)
            return 0
        if circuit >= 0 and transport < 0 and retry >= 0:
            output[unsafe_offset=0] = 5
            output[unsafe_offset=1] = min(circuit, retry)
            output[unsafe_offset=2] = max(circuit, retry)
            return 0
        if circuit < 0 and transport >= 0 and retry >= 0:
            output[unsafe_offset=0] = 6
            output[unsafe_offset=1] = min(transport, retry)
            output[unsafe_offset=2] = max(transport, retry)
            return 0
        output[unsafe_offset=0] = 7
        output[unsafe_offset=1] = min(circuit, min(transport, retry))
        output[unsafe_offset=2] = max(circuit, max(transport, retry))
        output[unsafe_offset=3] = retry
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_HALF_OPEN_SECONDS:
        if field_count != 4:
            return 1
        var score = fields[unsafe_offset=0]
        var threshold = fields[unsafe_offset=1]
        var base = fields[unsafe_offset=2]
        var maximum = fields[unsafe_offset=3]
        if score < 0 or threshold < 0 or base < 0 or maximum < 0:
            return 2
        var exponent = max(score - threshold, 0)
        if exponent > 3:
            exponent = 3
        var multiplier = runtime_health_saturating_shift_multiplier(exponent)
        output[unsafe_offset=0] = min(runtime_profile_health_saturating_elapsed(base * multiplier, 0), maximum)
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_OPEN_SECONDS:
        if field_count != 6:
            return 1
        var score = fields[unsafe_offset=0]
        var reopen = fields[unsafe_offset=1]
        var threshold = fields[unsafe_offset=2]
        var max_stage = fields[unsafe_offset=3]
        var base = fields[unsafe_offset=4]
        var maximum = fields[unsafe_offset=5]
        if score < 0 or reopen < 0 or threshold < 0 or max_stage < 0 or base < 0 or maximum < 0:
            return 2
        var exponent = max(score - threshold, 0)
        if exponent > 3:
            exponent = 3
        exponent += min(reopen, max_stage)
        var multiplier = runtime_health_saturating_shift_multiplier(exponent)
        if multiplier == INT64_MAX or (base > 0 and multiplier > INT64_MAX / base):
            output[unsafe_offset=0] = maximum
        else:
            output[unsafe_offset=0] = min(base * multiplier, maximum)
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_SOFTEN_UNTIL:
        if field_count != 3 or output_count < 3:
            return 1
        var until = fields[unsafe_offset=0]
        var now = fields[unsafe_offset=1]
        var max_future = max(fields[unsafe_offset=2], 0)
        if until <= now:
            output[unsafe_offset=0] = 0
            output[unsafe_offset=1] = until
            output[unsafe_offset=2] = 1
            return 0
        var max_until = now
        if max_future > 0 and now > INT64_MAX - max_future:
            max_until = INT64_MAX
        else:
            max_until = now + max_future
        var next_until = min(until, max_until)
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = next_until
        output[unsafe_offset=2] = 1 if next_until != until else 0
        return 0

    return 1
