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

comptime RUNTIME_HEALTH_SCALAR_BAD_PAIRING_NEXT: Int64 = 8
comptime RUNTIME_HEALTH_SCALAR_BUMP_DECISION: Int64 = 9
comptime RUNTIME_HEALTH_SCALAR_RECOVERY_DECISION: Int64 = 10
comptime RUNTIME_HEALTH_SCALAR_INFLIGHT_WEIGHT: Int64 = 11
comptime RUNTIME_HEALTH_SCALAR_INFLIGHT_HARD_LIMIT: Int64 = 12
comptime RUNTIME_HEALTH_SCALAR_INFLIGHT_SOFT_LIMIT: Int64 = 13
comptime RUNTIME_HEALTH_SCALAR_LATENCY_PENALTY: Int64 = 14
comptime RUNTIME_HEALTH_SCALAR_LATENCY_NEXT_SCORE: Int64 = 15
comptime RUNTIME_HEALTH_SCALAR_LATENCY_FAILURE_SCORE: Int64 = 16
comptime RUNTIME_HEALTH_POLICY_ABI_VERSION: Int64 = 2

@export("prodex_runtime_health_policy_v2")
def prodex_runtime_health_policy_v2(
    abi_version: Int64,
    operation: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
    output_count: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_HEALTH_POLICY_ABI_VERSION:
        return 4
    if field_count < 0 or field_count > 12 or output_count < 1 or output_count > 5:
        return 1
    if fields_address == 0 or output_address == 0:
        return 1
    var fields = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(output_count):
        output[unsafe_offset=index] = 0

    if operation == RUNTIME_HEALTH_SCALAR_BAD_PAIRING_NEXT:
        if field_count != 3:
            return 1
        var current = fields[unsafe_offset=0]
        var delta = fields[unsafe_offset=1]
        var maximum = fields[unsafe_offset=2]
        if (
            current > UInt64(UINT32_MAX)
            or delta > UInt64(UINT32_MAX)
            or maximum > UInt64(UINT32_MAX)
        ):
            return 2
        output[unsafe_offset=0] = min(current + delta, maximum)
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_BUMP_DECISION:
        if field_count != 9 or output_count < 5:
            return 1
        var current = fields[unsafe_offset=0]
        var delta = fields[unsafe_offset=1]
        var maximum = fields[unsafe_offset=2]
        var threshold = fields[unsafe_offset=3]
        var already_open = fields[unsafe_offset=4]
        var current_stage = fields[unsafe_offset=5]
        var max_stage = fields[unsafe_offset=6]
        var base_seconds = fields[unsafe_offset=7]
        var max_seconds = fields[unsafe_offset=8]
        if (
            current > UInt64(UINT32_MAX)
            or delta > UInt64(UINT32_MAX)
            or maximum > UInt64(UINT32_MAX)
            or threshold > UInt64(UINT32_MAX)
            or already_open > 1
            or current_stage > UInt64(UINT32_MAX)
            or max_stage > UInt64(UINT32_MAX)
            or base_seconds > UInt64(INT64_MAX)
            or max_seconds > UInt64(INT64_MAX)
        ):
            return 2
        var next = min(current + delta, maximum)
        output[unsafe_offset=0] = next
        if next < threshold:
            return 0
        var stage: UInt64 = 0
        if already_open == 1:
            stage = min(current_stage + 1, max_stage)
        output[unsafe_offset=1] = 1
        output[unsafe_offset=2] = stage
        var exponent = max(next - threshold, UInt64(0))
        if exponent > 3:
            exponent = 3
        exponent += min(stage, max_stage)
        var multiplier = runtime_health_saturating_shift_multiplier(Int64(exponent))
        var seconds = max_seconds
        if multiplier != INT64_MAX and (
            base_seconds == 0 or UInt64(multiplier) <= UInt64(INT64_MAX) / base_seconds
        ):
            seconds = min(base_seconds * UInt64(multiplier), max_seconds)
        output[unsafe_offset=3] = 1
        output[unsafe_offset=4] = seconds
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_RECOVERY_DECISION:
        if field_count != 5 or output_count < 4:
            return 1
        var current_present = fields[unsafe_offset=0]
        var current_score = fields[unsafe_offset=1]
        var current_streak = fields[unsafe_offset=2]
        var max_streak = fields[unsafe_offset=3]
        var recovery_base = fields[unsafe_offset=4]
        if (
            current_present > 1
            or current_score > UInt64(UINT32_MAX)
            or current_streak > UInt64(UINT32_MAX)
            or max_streak > UInt64(UINT32_MAX)
            or recovery_base > UInt64(UINT32_MAX)
        ):
            return 2
        if current_present == 0:
            return 0
        var next_streak = min(current_streak + 1, max_streak)
        var extra: UInt64 = 0
        if next_streak > 1:
            extra = next_streak - 1
        if extra > 1:
            extra = 1
        var recovery = recovery_base + extra
        var next_score: UInt64 = 0
        if current_score > recovery:
            next_score = current_score - recovery
        if next_score == 0:
            return 0
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = next_score
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = next_streak
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_INFLIGHT_WEIGHT:
        if field_count != 1:
            return 1
        output[unsafe_offset=0] = 2 if fields[unsafe_offset=0] == 1 else 1
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_INFLIGHT_HARD_LIMIT:
        if field_count != 2:
            return 1
        if fields[unsafe_offset=1] == 0:
            return 2
        output[unsafe_offset=0] = max(fields[unsafe_offset=0], fields[unsafe_offset=1])
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_INFLIGHT_SOFT_LIMIT:
        if field_count != 3:
            return 1
        var route_kind = fields[unsafe_offset=0]
        var pressure_mode = fields[unsafe_offset=1]
        var base = fields[unsafe_offset=2]
        if route_kind > 3 or pressure_mode > 1:
            return 2
        base = max(base, UInt64(1))
        if pressure_mode == 0:
            output[unsafe_offset=0] = base
            return 0
        var reduction: UInt64 = 2
        if route_kind == 0 or route_kind == 2:
            reduction = 1
        var reduced_base: UInt64 = 1
        if base > reduction:
            reduced_base = base - reduction
        output[unsafe_offset=0] = reduced_base
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_LATENCY_PENALTY:
        if field_count != 4:
            return 1
        var elapsed = fields[unsafe_offset=0]
        var route_kind = fields[unsafe_offset=1]
        var stage_kind = fields[unsafe_offset=2]
        var maximum = fields[unsafe_offset=3]
        if route_kind > 3 or stage_kind > 2 or maximum > UInt64(UINT32_MAX):
            return 2
        var good: UInt64 = 100
        var warn: UInt64 = 250
        var poor: UInt64 = 600
        var severe: UInt64 = 1200
        if (route_kind == 0 and stage_kind == 1) or (route_kind == 2 and stage_kind == 2):
            good = 120
            warn = 300
            poor = 700
            severe = 1500
        elif route_kind == 1 or route_kind == 3:
            good = 80
            warn = 180
            poor = 400
            severe = 900
        if elapsed <= good:
            output[unsafe_offset=0] = 0
        elif elapsed <= warn:
            output[unsafe_offset=0] = 2
        elif elapsed <= poor:
            output[unsafe_offset=0] = 4
        elif elapsed <= severe:
            output[unsafe_offset=0] = 7
        else:
            output[unsafe_offset=0] = maximum
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_LATENCY_NEXT_SCORE:
        if field_count != 2:
            return 1
        var current = fields[unsafe_offset=0]
        var observed = fields[unsafe_offset=1]
        if current > UInt64(UINT32_MAX) or observed > UInt64(UINT32_MAX):
            return 2
        if observed == 0:
            if current > 2:
                output[unsafe_offset=0] = current - 2
        else:
            output[unsafe_offset=0] = ((current * 2) + observed + 2) / 3
        return 0

    if operation == RUNTIME_HEALTH_SCALAR_LATENCY_FAILURE_SCORE:
        if field_count != 3:
            return 1
        var current = fields[unsafe_offset=0]
        var penalty = fields[unsafe_offset=1]
        var maximum = fields[unsafe_offset=2]
        if (
            current > UInt64(UINT32_MAX)
            or penalty > UInt64(UINT32_MAX)
            or maximum > UInt64(UINT32_MAX)
        ):
            return 2
        output[unsafe_offset=0] = min(current + penalty, maximum)
        return 0

    return 1
