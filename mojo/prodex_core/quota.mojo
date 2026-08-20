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
