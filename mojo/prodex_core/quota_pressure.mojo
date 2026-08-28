from std.memory import Pointer

from runtime_math import (
    runtime_quota_saturating_add,
    runtime_quota_saturating_mul,
)


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


comptime RUNTIME_QUOTA_SCORE_FIELD_COUNT: Int64 = 8
comptime RUNTIME_QUOTA_SCORE_MAX_COUNT: Int64 = 256


def runtime_quota_score_field(
    fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64
) -> Int64:
    return fields[
        unsafe_offset=(index * RUNTIME_QUOTA_SCORE_FIELD_COUNT) + field
    ]


@export("prodex_runtime_quota_score_batch")
def prodex_runtime_quota_score_batch(
    fields_address: UInt,
    pressure_band_address: UInt,
    total_pressure_address: UInt,
    weekly_pressure_address: UInt,
    five_hour_pressure_address: UInt,
    reserve_floor_address: UInt,
    weekly_remaining_address: UInt,
    five_hour_remaining_address: UInt,
    weekly_reset_at_address: UInt,
    five_hour_reset_at_address: UInt,
    count: Int64,
    route_kind: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_QUOTA_SCORE_MAX_COUNT:
        return 1
    if route_kind < 0 or route_kind > 3:
        return 1
    if count == 0:
        return 0
    if fields_address == 0 or pressure_band_address == 0 or total_pressure_address == 0 or weekly_pressure_address == 0 or five_hour_pressure_address == 0 or reserve_floor_address == 0 or weekly_remaining_address == 0 or five_hour_remaining_address == 0 or weekly_reset_at_address == 0 or five_hour_reset_at_address == 0:
        return 1

    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(fields_address))
    var pressure_band = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(pressure_band_address))
    var total_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(total_pressure_address))
    var weekly_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_pressure_address))
    var five_hour_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_pressure_address))
    var reserve_floor = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(reserve_floor_address))
    var weekly_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_remaining_address))
    var five_hour_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_remaining_address))
    var weekly_reset_at = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_reset_at_address))
    var five_hour_reset_at = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_reset_at_address))

    var weekly_weight: Int64 = 8
    if route_kind == 0 or route_kind == 2:
        weekly_weight = 10

    for index in range(count):
        var weekly_pressure_value = runtime_quota_score_field(fields, index, 0)
        var five_hour_pressure_value = runtime_quota_score_field(fields, index, 1)
        var weekly_remaining_value = runtime_quota_score_field(fields, index, 2)
        var five_hour_remaining_value = runtime_quota_score_field(fields, index, 3)
        var weekly_has_value = runtime_quota_score_field(fields, index, 4)
        var five_hour_has_value = runtime_quota_score_field(fields, index, 5)
        if (
            weekly_pressure_value < 0
            or five_hour_pressure_value < 0
            or weekly_remaining_value < 0
            or weekly_remaining_value > 100
            or five_hour_remaining_value < 0
            or five_hour_remaining_value > 100
            or weekly_has_value < 0
            or weekly_has_value > 1
            or five_hour_has_value < 0
            or five_hour_has_value > 1
        ):
            return 2

        var band = prodex_runtime_quota_pressure_band_for_route(
            five_hour_remaining_value,
            five_hour_has_value,
            weekly_remaining_value,
            weekly_has_value,
            route_kind,
        )
        var reserve_bias: Int64 = 0
        if band == 1:
            reserve_bias = 250_000
        elif band == 2:
            reserve_bias = 1_000_000
        elif band == 3 or band == 4:
            reserve_bias = 2305843009213693951

        var total = runtime_quota_saturating_add(
            reserve_bias,
            runtime_quota_saturating_mul(weekly_pressure_value, weekly_weight),
        )
        total = runtime_quota_saturating_add(total, five_hour_pressure_value)
        pressure_band[unsafe_offset=index] = band
        total_pressure[unsafe_offset=index] = total
        weekly_pressure[unsafe_offset=index] = weekly_pressure_value
        five_hour_pressure[unsafe_offset=index] = five_hour_pressure_value
        if weekly_remaining_value < five_hour_remaining_value:
            reserve_floor[unsafe_offset=index] = weekly_remaining_value
        else:
            reserve_floor[unsafe_offset=index] = five_hour_remaining_value
        weekly_remaining[unsafe_offset=index] = weekly_remaining_value
        five_hour_remaining[unsafe_offset=index] = five_hour_remaining_value
        weekly_reset_at[unsafe_offset=index] = runtime_quota_score_field(
            fields, index, 6
        )
        five_hour_reset_at[unsafe_offset=index] = runtime_quota_score_field(
            fields, index, 7
        )
    return 0
