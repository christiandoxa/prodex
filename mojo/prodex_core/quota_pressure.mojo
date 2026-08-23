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
