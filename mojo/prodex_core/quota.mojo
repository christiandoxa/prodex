from std.memory import Pointer

@export("prodex_quota_remaining_percent")
def prodex_quota_remaining_percent(
    used_percent: Int64,
    has_value: Int64,
) abi("C") -> Int64:
    if has_value == 0:
        return 0
    if used_percent < 0:
        return 100
    if used_percent > 100:
        return 0
    return 100 - used_percent

@export("prodex_quota_window_status")
def prodex_quota_window_status(
    remaining_percent: Int64,
    has_window: Int64,
) abi("C") -> Int64:
    if has_window == 0:
        return 4
    if remaining_percent == 0:
        return 3
    if remaining_percent <= 5:
        return 2
    if remaining_percent <= 15:
        return 1
    return 0

def prodex_quota_pressure_band_for_status(status: Int64) -> Int64:
    if status == 0:
        return 0
    if status == 1:
        return 1
    if status == 2:
        return 2
    if status == 3:
        return 3
    return 4

@export("prodex_quota_pressure_band")
def prodex_quota_pressure_band(
    five_hour_status: Int64,
    weekly_status: Int64,
) abi("C") -> Int64:
    var five_hour_band = prodex_quota_pressure_band_for_status(five_hour_status)
    var weekly_band = prodex_quota_pressure_band_for_status(weekly_status)
    if five_hour_band > weekly_band:
        return five_hour_band
    return weekly_band

@export("prodex_quota_window_pair_has_ready_limit")
def prodex_quota_window_pair_has_ready_limit(
    first_used_percent: Int64,
    first_has_value: Int64,
    second_used_percent: Int64,
    second_has_value: Int64,
) abi("C") -> Int64:
    if first_has_value == 0 and second_has_value == 0:
        return 0
    if first_has_value != 0 and first_used_percent >= 100:
        return 0
    if second_has_value != 0 and second_used_percent >= 100:
        return 0
    return 1

comptime QUOTA_MAIN_AGGREGATION_MAX_COUNT: Int64 = 1_024
comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775807 - 1

def quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    if right < 0 and left < INT64_MIN - right:
        return INT64_MIN
    return left + right

@export("prodex_quota_main_aggregate_batch")
def prodex_quota_main_aggregate_batch(
    remaining_percent: Pointer[mut=False, Int64, _],
    remaining_present: Pointer[mut=False, Int64, _],
    reset_at: Pointer[mut=False, Int64, _],
    reset_present: Pointer[mut=False, Int64, _],
    profiles_with_data: Pointer[mut=True, Int64, _],
    pool_remaining: Pointer[mut=True, Int64, _],
    earliest_reset_at: Pointer[mut=True, Int64, _],
    earliest_present: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > QUOTA_MAIN_AGGREGATION_MAX_COUNT:
        return 1

    var profile_count: Int64 = 0
    var remaining_total: Int64 = 0
    var has_earliest: Int64 = 0
    var earliest: Int64 = 0
    for index in range(count):
        var has_remaining = remaining_present[unsafe_offset=index]
        var has_reset = reset_present[unsafe_offset=index]
        if (has_remaining != 0 and has_remaining != 1) or (has_reset != 0 and has_reset != 1):
            return 2
        if has_remaining == 1:
            profile_count += 1
            remaining_total = quota_saturating_add(
                remaining_total,
                remaining_percent[unsafe_offset=index],
            )
            if has_reset == 1:
                var reset = reset_at[unsafe_offset=index]
                if has_earliest == 0 or reset < earliest:
                    earliest = reset
                    has_earliest = 1
    profiles_with_data[unsafe_offset=0] = profile_count
    pool_remaining[unsafe_offset=0] = remaining_total
    earliest_reset_at[unsafe_offset=0] = earliest
    earliest_present[unsafe_offset=0] = has_earliest
    return 0
