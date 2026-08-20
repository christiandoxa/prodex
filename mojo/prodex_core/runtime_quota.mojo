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
from std.memory import Pointer

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
