from std.memory import Pointer

from runtime_math import (
    INT64_MAX,
    INT64_MIN,
    runtime_precommit_budget_plan,
    runtime_quota_scale_pressure,
    runtime_quota_saturating_add,
    runtime_quota_saturating_mul,
)


@export("prodex_runtime_precommit_budget_plan_v1")
def prodex_runtime_precommit_budget_plan_v1(
    continuation: Int64,
    pressure_mode: Int64,
    standard_attempt_limit: Int64,
    standard_budget_ms: Int64,
    continuation_attempt_limit: Int64,
    continuation_budget_ms: Int64,
    pressure_attempt_limit: Int64,
    pressure_budget_ms: Int64,
    profile_count: Int64,
    attempts_per_profile: Int64,
    attempt_limit_out: Pointer[mut=True, Int64, _],
    budget_ms_out: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    return runtime_precommit_budget_plan(
        continuation,
        pressure_mode,
        standard_attempt_limit,
        standard_budget_ms,
        continuation_attempt_limit,
        continuation_budget_ms,
        pressure_attempt_limit,
        pressure_budget_ms,
        profile_count,
        attempts_per_profile,
        attempt_limit_out,
        budget_ms_out,
    )


def runtime_i64_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    return left - right


def runtime_quota_snapshot_window(
    status: Int64,
    remaining: Int64,
    reset_at: Int64,
    now: Int64,
    output: Pointer[mut=True, Int64, _],
    offset: Int64,
):
    if reset_at != INT64_MAX and reset_at <= now:
        output[unsafe_offset=offset] = 0
        output[unsafe_offset=offset + 1] = 100
    else:
        output[unsafe_offset=offset] = status
        output[unsafe_offset=offset + 1] = remaining
    output[unsafe_offset=offset + 2] = reset_at


@export("prodex_runtime_quota_snapshot_plan_v1")
def prodex_runtime_quota_snapshot_plan_v1(
    five_hour_status: Int64,
    five_hour_remaining: Int64,
    five_hour_reset_at: Int64,
    weekly_status: Int64,
    weekly_remaining: Int64,
    weekly_reset_at: Int64,
    route_kind: Int64,
    checked_at: Int64,
    now: Int64,
    stale_grace_seconds: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if (
        five_hour_status < 0
        or five_hour_status > 4
        or weekly_status < 0
        or weekly_status > 4
        or route_kind < 0
        or route_kind > 3
        or stale_grace_seconds < 0
    ):
        return 1

    # Remaining values are raw signed observations. The typed status and reset
    # determine usability; rejecting a 300-percent fixture here used to panic
    # before the caller could report inflight saturation. Do not clamp or
    # reclassify the observation: the feature-off snapshot path preserves it.
    runtime_quota_snapshot_window(
        five_hour_status,
        five_hour_remaining,
        five_hour_reset_at,
        now,
        output,
        0,
    )
    runtime_quota_snapshot_window(
        weekly_status,
        weekly_remaining,
        weekly_reset_at,
        now,
        output,
        3,
    )
    if output[unsafe_offset=0] == 4 and output[unsafe_offset=3] != 4:
        output[unsafe_offset=0] = 0
        output[unsafe_offset=1] = 100
        output[unsafe_offset=2] = INT64_MAX
    elif output[unsafe_offset=3] == 4 and output[unsafe_offset=0] != 4:
        output[unsafe_offset=3] = 0
        output[unsafe_offset=4] = 100
        output[unsafe_offset=5] = INT64_MAX
    output[unsafe_offset=6] = max(
        output[unsafe_offset=0], output[unsafe_offset=3]
    )

    var five_hour_hold_active = (
        five_hour_status == 3
        and five_hour_reset_at != INT64_MAX
        and five_hour_reset_at > now
    )
    var weekly_hold_active = (
        weekly_status == 3
        and weekly_reset_at != INT64_MAX
        and weekly_reset_at > now
    )
    var hold_active = five_hour_hold_active or weekly_hold_active
    var five_hour_hold_expired = (
        five_hour_status == 3
        and five_hour_reset_at != INT64_MAX
        and five_hour_reset_at <= now
    )
    var weekly_hold_expired = (
        weekly_status == 3
        and weekly_reset_at != INT64_MAX
        and weekly_reset_at <= now
    )
    var hold_expired = five_hour_hold_expired or weekly_hold_expired
    output[unsafe_offset=7] = Int64(hold_active)
    output[unsafe_offset=8] = Int64(hold_expired)
    output[unsafe_offset=9] = Int64(
        hold_active
        or (
            not hold_expired
            and runtime_i64_saturating_sub(now, checked_at)
            <= stale_grace_seconds
        )
    )
    return 0


def runtime_quota_main_route(route_kind: Int64) -> Bool:
    return route_kind == 0 or route_kind == 2


def runtime_quota_gate_block_reason(
    five_hour_status: Int64, route_kind: Int64
) -> Int64:
    if five_hour_status == 3:
        return 1
    if runtime_quota_main_route(route_kind) and five_hour_status == 3:
        return 2
    return 0


@export("prodex_runtime_quota_gate_plan_v1")
def prodex_runtime_quota_gate_plan_v1(
    five_hour_status: Int64,
    five_hour_reset_at: Int64,
    weekly_status: Int64,
    weekly_reset_at: Int64,
    route_kind: Int64,
    source: Int64,
    has_continuation_context: Int64,
    has_alternative_quota_profile: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if (
        five_hour_status < 0
        or five_hour_status > 4
        or weekly_status < 0
        or weekly_status > 4
        or route_kind < 0
        or route_kind > 3
        or source < -1
        or source > 1
        or has_continuation_context < 0
        or has_continuation_context > 1
        or has_alternative_quota_profile < 0
        or has_alternative_quota_profile > 1
    ):
        return 1

    var main_route = runtime_quota_main_route(route_kind)
    var requires_probe = (
        main_route
        and source != 0
        and (
            five_hour_status == 2
            or weekly_status == 2
            or five_hour_status == 4
            or weekly_status == 4
        )
    )
    var requires_live_after_probe = (
        main_route
        and source != 0
        and (five_hour_status == 4 or weekly_status == 4)
    )
    var block_reason = runtime_quota_gate_block_reason(
        five_hour_status, route_kind
    )
    var blocking_reset_at = INT64_MIN
    if five_hour_status == 3 and five_hour_reset_at != INT64_MAX:
        blocking_reset_at = five_hour_reset_at
    if weekly_status == 3 and weekly_reset_at != INT64_MAX:
        blocking_reset_at = max(blocking_reset_at, weekly_reset_at)

    var initial_decision: Int64 = 0
    var initial_reason: Int64 = 0
    if has_continuation_context == 1 and source == 1 and block_reason != 0:
        initial_decision = 2
        initial_reason = block_reason
    elif requires_probe:
        initial_decision = 1

    var final_reason: Int64 = 0
    if (
        main_route
        and has_alternative_quota_profile == 1
        and weekly_status == 3
    ):
        final_reason = 1
    elif requires_live_after_probe and has_alternative_quota_profile == 1:
        final_reason = 3
    elif block_reason != 0:
        final_reason = block_reason

    output[unsafe_offset=0] = Int64(requires_probe)
    output[unsafe_offset=1] = Int64(requires_live_after_probe)
    output[unsafe_offset=2] = block_reason
    output[unsafe_offset=3] = blocking_reset_at
    output[unsafe_offset=4] = initial_decision
    output[unsafe_offset=5] = initial_reason
    output[unsafe_offset=6] = Int64(final_reason != 0)
    output[unsafe_offset=7] = final_reason
    return 0


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


def runtime_quota_write_score(
    pressure_band: Pointer[mut=True, Int64, _],
    total_pressure: Pointer[mut=True, Int64, _],
    weekly_pressure: Pointer[mut=True, Int64, _],
    five_hour_pressure: Pointer[mut=True, Int64, _],
    reserve_floor: Pointer[mut=True, Int64, _],
    weekly_remaining: Pointer[mut=True, Int64, _],
    five_hour_remaining: Pointer[mut=True, Int64, _],
    weekly_reset_at: Pointer[mut=True, Int64, _],
    five_hour_reset_at: Pointer[mut=True, Int64, _],
    index: Int64,
    band: Int64,
    weekly_pressure_value: Int64,
    five_hour_pressure_value: Int64,
    weekly_remaining_value: Int64,
    five_hour_remaining_value: Int64,
    weekly_reset_at_value: Int64,
    five_hour_reset_at_value: Int64,
    route_kind: Int64,
) -> None:
    var reserve_bias: Int64 = 0
    if band == 1:
        reserve_bias = 250_000
    elif band == 2:
        reserve_bias = 1_000_000
    elif band == 3 or band == 4:
        reserve_bias = 2305843009213693951

    var weekly_weight: Int64 = 8
    if route_kind == 0 or route_kind == 2:
        weekly_weight = 10

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
    weekly_reset_at[unsafe_offset=index] = weekly_reset_at_value
    five_hour_reset_at[unsafe_offset=index] = five_hour_reset_at_value


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
        runtime_quota_write_score(
            pressure_band,
            total_pressure,
            weekly_pressure,
            five_hour_pressure,
            reserve_floor,
            weekly_remaining,
            five_hour_remaining,
            weekly_reset_at,
            five_hour_reset_at,
            index,
            band,
            weekly_pressure_value,
            five_hour_pressure_value,
            weekly_remaining_value,
            five_hour_remaining_value,
            runtime_quota_score_field(fields, index, 6),
            runtime_quota_score_field(fields, index, 7),
            route_kind,
        )
    return 0


comptime RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT: Int64 = 9


def runtime_quota_route_score_field(
    fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64
) -> Int64:
    return fields[
        unsafe_offset=(index * RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT) + field
    ]


def runtime_quota_route_score_band(
    five_hour_remaining_percent: Int64,
    five_hour_has_value: Int64,
    weekly_remaining_percent: Int64,
    weekly_has_value: Int64,
    route_kind: Int64,
) -> Int64:
    if five_hour_has_value == 0 or weekly_has_value == 0:
        return 4
    return prodex_runtime_quota_pressure_band_for_route(
        five_hour_remaining_percent,
        five_hour_has_value,
        weekly_remaining_percent,
        weekly_has_value,
        route_kind,
    )


@export("prodex_runtime_quota_route_score_resolution_batch")
def prodex_runtime_quota_route_score_resolution_batch(
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
    if (
        fields_address == 0
        or pressure_band_address == 0
        or total_pressure_address == 0
        or weekly_pressure_address == 0
        or five_hour_pressure_address == 0
        or reserve_floor_address == 0
        or weekly_remaining_address == 0
        or five_hour_remaining_address == 0
        or weekly_reset_at_address == 0
        or five_hour_reset_at_address == 0
    ):
        return 1

    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var pressure_band = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(pressure_band_address)
    )
    var total_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(total_pressure_address)
    )
    var weekly_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(weekly_pressure_address)
    )
    var five_hour_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(five_hour_pressure_address)
    )
    var reserve_floor = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(reserve_floor_address)
    )
    var weekly_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(weekly_remaining_address)
    )
    var five_hour_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(five_hour_remaining_address)
    )
    var weekly_reset_at = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(weekly_reset_at_address)
    )
    var five_hour_reset_at = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(five_hour_reset_at_address)
    )

    for index in range(count):
        var weekly_pressure_value = runtime_quota_route_score_field(
            fields, index, 0
        )
        var five_hour_pressure_value = runtime_quota_route_score_field(
            fields, index, 1
        )
        var scale_bps = runtime_quota_route_score_field(fields, index, 2)
        var weekly_remaining_value = runtime_quota_route_score_field(
            fields, index, 3
        )
        var five_hour_remaining_value = runtime_quota_route_score_field(
            fields, index, 4
        )
        var weekly_has_value = runtime_quota_route_score_field(fields, index, 5)
        var five_hour_has_value = runtime_quota_route_score_field(
            fields, index, 6
        )
        if (
            weekly_pressure_value < 0
            or five_hour_pressure_value < 0
            or scale_bps < 0
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

        var band = runtime_quota_route_score_band(
            five_hour_remaining_value,
            five_hour_has_value,
            weekly_remaining_value,
            weekly_has_value,
            route_kind,
        )
        runtime_quota_write_score(
            pressure_band,
            total_pressure,
            weekly_pressure,
            five_hour_pressure,
            reserve_floor,
            weekly_remaining,
            five_hour_remaining,
            weekly_reset_at,
            five_hour_reset_at,
            index,
            band,
            runtime_quota_scale_pressure(weekly_pressure_value, scale_bps),
            runtime_quota_scale_pressure(five_hour_pressure_value, scale_bps),
            weekly_remaining_value,
            five_hour_remaining_value,
            runtime_quota_route_score_field(fields, index, 7),
            runtime_quota_route_score_field(fields, index, 8),
            route_kind,
        )
    return 0
