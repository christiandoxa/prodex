from std.memory import Pointer

def prodex_runtime_quota_pressure_band_for_status(
    remaining_percent: Int64,
    has_value: Int64,
    thin_threshold: Int64,
    critical_threshold: Int64,
) -> Int64:
    if has_value == 0:
        return 0
    if remaining_percent == 0:
        return 3
    if remaining_percent <= critical_threshold:
        return 2
    if remaining_percent <= thin_threshold:
        return 1
    return 0

@export("prodex_runtime_quota_pressure_band_for_route")
def prodex_runtime_quota_pressure_band_for_route(
    five_hour_remaining_percent: Int64,
    five_hour_has_value: Int64,
    weekly_remaining_percent: Int64,
    weekly_has_value: Int64,
    route_kind: Int64,
) abi("C") -> Int64:
    if five_hour_has_value == 0 and weekly_has_value == 0:
        return 4
    if (five_hour_has_value != 0 and five_hour_remaining_percent == 0) or (
        weekly_has_value != 0 and weekly_remaining_percent == 0
    ):
        return 3

    var thin_weekly: Int64 = 10
    var thin_five_hour: Int64 = 5
    var critical_weekly: Int64 = 5
    var critical_five_hour: Int64 = 3
    if route_kind == 0 or route_kind == 2:
        thin_weekly = 20
        thin_five_hour = 10
        critical_weekly = 10
        critical_five_hour = 5

    var weekly_band = prodex_runtime_quota_pressure_band_for_status(
        weekly_remaining_percent,
        weekly_has_value,
        thin_weekly,
        critical_weekly,
    )
    var five_hour_band = prodex_runtime_quota_pressure_band_for_status(
        five_hour_remaining_percent,
        five_hour_has_value,
        thin_five_hour,
        critical_five_hour,
    )
    if weekly_band > five_hour_band:
        return weekly_band
    return five_hour_band

@export("prodex_runtime_quota_profile_score_batch")
def prodex_runtime_quota_profile_score_batch(
    weekly_pressure: Pointer[mut=False, Int64, _],
    five_hour_pressure: Pointer[mut=False, Int64, _],
    scale_bps: Pointer[mut=False, Int64, _],
    weekly_remaining: Pointer[mut=False, Int64, _],
    five_hour_remaining: Pointer[mut=False, Int64, _],
    reserve_bias: Pointer[mut=False, Int64, _],
    weekly_weight: Pointer[mut=False, Int64, _],
    total_pressure: Pointer[mut=True, Int64, _],
    scaled_weekly_pressure: Pointer[mut=True, Int64, _],
    scaled_five_hour_pressure: Pointer[mut=True, Int64, _],
    reserve_floor: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64:
        return 1

    for index in range(count):
        var weekly_value = weekly_pressure[unsafe_offset=index]
        var five_hour_value = five_hour_pressure[unsafe_offset=index]
        var scale_value = scale_bps[unsafe_offset=index]
        var weekly_remaining_value = weekly_remaining[unsafe_offset=index]
        var five_hour_remaining_value = five_hour_remaining[unsafe_offset=index]
        var reserve_bias_value = reserve_bias[unsafe_offset=index]
        var weekly_weight_value = weekly_weight[unsafe_offset=index]
        if weekly_value < 0 or five_hour_value < 0 or scale_value < 0 or reserve_bias_value < 0 or weekly_weight_value < 0:
            return 2
        if weekly_remaining_value < 0 or weekly_remaining_value > 100 or five_hour_remaining_value < 0 or five_hour_remaining_value > 100:
            return 2

        var weekly_scaled = runtime_quota_scale_pressure(weekly_value, scale_value)
        var five_hour_scaled = runtime_quota_scale_pressure(five_hour_value, scale_value)
        var weighted_weekly = runtime_quota_saturating_mul(weekly_scaled, weekly_weight_value)
        var total = runtime_quota_saturating_add(reserve_bias_value, weighted_weekly)
        total = runtime_quota_saturating_add(total, five_hour_scaled)
        total_pressure[unsafe_offset=index] = total
        scaled_weekly_pressure[unsafe_offset=index] = weekly_scaled
        scaled_five_hour_pressure[unsafe_offset=index] = five_hour_scaled
        if weekly_remaining_value < five_hour_remaining_value:
            reserve_floor[unsafe_offset=index] = weekly_remaining_value
        else:
            reserve_floor[unsafe_offset=index] = five_hour_remaining_value

    return 0

comptime INT64_MAX: Int64 = 9223372036854775807

def runtime_quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    return left + right

def runtime_quota_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > INT64_MAX / right:
        return INT64_MAX
    return left * right

def runtime_quota_scale_pressure(pressure: Int64, scale_bps: Int64) -> Int64:
    if pressure == INT64_MAX:
        return INT64_MAX
    var scale = scale_bps
    if scale < 0:
        scale = 0
    if pressure < 0:
        return pressure
    if scale == 0 or pressure == 0:
        return 0
    if pressure > INT64_MAX / scale:
        return INT64_MAX
    return (pressure * scale) / 10_000

comptime RUNTIME_CANDIDATE_PLAN_FIELD_COUNT: Int64 = 22
comptime RUNTIME_CANDIDATE_PLAN_MAX_COUNT: Int64 = 256

def runtime_candidate_field(fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64) -> Int64:
    return fields[unsafe_offset=(index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT) + field]

def runtime_candidate_source_sort_key(route_kind: Int64, source: Int64) -> Int64:
    if route_kind == 0 or route_kind == 2:
        return source
    return 0

def runtime_candidate_ready_less(
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
    route_kind: Int64,
) -> Bool:
    var left_value = runtime_candidate_field(fields, left, 1)
    var right_value = runtime_candidate_field(fields, right, 1)
    if left_value != right_value:
        return left_value < right_value

    # quota_sort_key = (band, a, b, c, Reverse(d), Reverse(e), Reverse(f), g, h)
    left_value = runtime_candidate_field(fields, left, 2)
    right_value = runtime_candidate_field(fields, right, 2)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 3)
    right_value = runtime_candidate_field(fields, right, 3)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 4)
    right_value = runtime_candidate_field(fields, right, 4)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 5)
    right_value = runtime_candidate_field(fields, right, 5)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 6)
    right_value = runtime_candidate_field(fields, right, 6)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 7)
    right_value = runtime_candidate_field(fields, right, 7)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_candidate_field(fields, left, 8)
    right_value = runtime_candidate_field(fields, right, 8)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 9)
    right_value = runtime_candidate_field(fields, right, 9)
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_candidate_source_sort_key(
        route_kind,
        runtime_candidate_field(fields, left, 11),
    )
    right_value = runtime_candidate_source_sort_key(
        route_kind,
        runtime_candidate_field(fields, right, 11),
    )
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 12)
    right_value = runtime_candidate_field(fields, right, 12)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 13)
    right_value = runtime_candidate_field(fields, right, 13)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 14)
    right_value = runtime_candidate_field(fields, right, 14)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 15)
    right_value = runtime_candidate_field(fields, right, 15)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 16)
    right_value = runtime_candidate_field(fields, right, 16)
    if left_value != right_value:
        return left_value < right_value
    left_value = runtime_candidate_field(fields, left, 17)
    right_value = runtime_candidate_field(fields, right, 17)
    if left_value != right_value:
        return left_value < right_value
    return left < right

def runtime_candidate_less(
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
    route_kind: Int64,
    fallback: Bool,
) -> Bool:
    if fallback:
        var left_value = runtime_candidate_field(fields, left, 18)
        var right_value = runtime_candidate_field(fields, right, 18)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 19)
        right_value = runtime_candidate_field(fields, right, 19)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 20)
        right_value = runtime_candidate_field(fields, right, 20)
        if left_value != right_value:
            return left_value < right_value
        left_value = runtime_candidate_field(fields, left, 21)
        right_value = runtime_candidate_field(fields, right, 21)
        if left_value != right_value:
            return left_value < right_value
    return runtime_candidate_ready_less(fields, left, right, route_kind)

@export("prodex_runtime_candidate_plan_batch")
def prodex_runtime_candidate_plan_batch(
    fields: Pointer[mut=False, Int64, _],
    ready_indices: Pointer[mut=True, Int64, _],
    ready_count: Pointer[mut=True, Int64, _],
    fallback_indices: Pointer[mut=True, Int64, _],
    fallback_count: Pointer[mut=True, Int64, _],
    count: Int64,
    route_kind: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT:
        return 1
    if route_kind < 0 or route_kind > 3:
        return 2

    for index in range(count):
        if runtime_candidate_field(fields, index, 0) < 0 or runtime_candidate_field(fields, index, 0) > 1:
            return 2
        if runtime_candidate_field(fields, index, 11) < 0 or runtime_candidate_field(fields, index, 11) > 1:
            return 2
        if runtime_candidate_field(fields, index, 14) < 0 or runtime_candidate_field(fields, index, 14) > 1:
            return 2
        if runtime_candidate_field(fields, index, 1) < 0 or runtime_candidate_field(fields, index, 12) < 0 or runtime_candidate_field(fields, index, 13) < 0:
            return 2
        if runtime_candidate_field(fields, index, 16) < 0 or runtime_candidate_field(fields, index, 18) < 0:
            return 2

    var ready_len: Int64 = 0
    for index in range(count):
        if runtime_candidate_field(fields, index, 0) == 0:
            ready_indices[unsafe_offset=ready_len] = index
            ready_len += 1
    ready_count[unsafe_offset=0] = ready_len

    var fallback_len: Int64 = 0
    for index in range(count):
        fallback_indices[unsafe_offset=fallback_len] = index
        fallback_len += 1
    fallback_count[unsafe_offset=0] = fallback_len

    # ponytail: bounded O(n²) selection keeps the ABI allocation-free; replace with
    # a verified stable sort only if the runtime pool exceeds 256 candidates.
    for position in range(ready_len):
        var best = position
        for offset in range(position + 1, ready_len):
            var candidate = ready_indices[unsafe_offset=offset]
            var current = ready_indices[unsafe_offset=best]
            if runtime_candidate_less(fields, candidate, current, route_kind, False):
                best = offset
        if best != position:
            var selected = ready_indices[unsafe_offset=best]
            ready_indices[unsafe_offset=best] = ready_indices[unsafe_offset=position]
            ready_indices[unsafe_offset=position] = selected

    for position in range(fallback_len):
        var best = position
        for offset in range(position + 1, fallback_len):
            var candidate = fallback_indices[unsafe_offset=offset]
            var current = fallback_indices[unsafe_offset=best]
            if runtime_candidate_less(fields, candidate, current, route_kind, True):
                best = offset
        if best != position:
            var selected = fallback_indices[unsafe_offset=best]
            fallback_indices[unsafe_offset=best] = fallback_indices[unsafe_offset=position]
            fallback_indices[unsafe_offset=position] = selected
    return 0
