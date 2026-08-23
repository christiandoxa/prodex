from std.memory import Pointer
from runtime_math import (
    INT64_MAX,
    INT64_MIN,
    runtime_quota_saturating_add,
    runtime_quota_saturating_mul,
    runtime_quota_scale_pressure,
)

comptime RUNTIME_PROFILE_SCHEDULE_FIELD_COUNT: Int64 = 16
comptime RUNTIME_PROFILE_SCHEDULE_MAX_COUNT: Int64 = 256


def runtime_profile_schedule_field(
    fields: Pointer[mut=False, Int64, _],
    index: Int64,
    field: Int64,
) -> Int64:
    return fields[
        unsafe_offset=(index * RUNTIME_PROFILE_SCHEDULE_FIELD_COUNT) + field
    ]


def runtime_profile_score_within_bps(
    candidate_score: Int64,
    best_score: Int64,
    bps: Int64,
) -> Bool:
    if candidate_score <= best_score:
        return True
    if best_score <= 0:
        return False
    var extra = (best_score / 10_000) * bps
    extra = runtime_quota_saturating_add(
        extra,
        ((best_score % 10_000) * bps) / 10_000,
    )
    return candidate_score <= runtime_quota_saturating_add(best_score, extra)


def runtime_profile_schedule_near_optimal(
    fields: Pointer[mut=False, Int64, _],
    total_pressure: Pointer[mut=False, Int64, _],
    index: Int64,
    best_provider_priority: Int64,
    best_total_pressure: Int64,
) -> Int64:
    if runtime_profile_schedule_field(
        fields, index, 7
    ) == best_provider_priority and runtime_profile_score_within_bps(
        total_pressure[unsafe_offset=index],
        best_total_pressure,
        1_000,
    ):
        return 0
    return 1


def runtime_profile_schedule_less(
    fields: Pointer[mut=False, Int64, _],
    total_pressure: Pointer[mut=False, Int64, _],
    scaled_weekly_pressure: Pointer[mut=False, Int64, _],
    scaled_five_hour_pressure: Pointer[mut=False, Int64, _],
    reserve_floor: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
    best_provider_priority: Int64,
    best_total_pressure: Int64,
) -> Bool:
    var left_value = runtime_profile_schedule_field(fields, left, 7)
    var right_value = runtime_profile_schedule_field(fields, right, 7)
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_profile_schedule_near_optimal(
        fields,
        total_pressure,
        left,
        best_provider_priority,
        best_total_pressure,
    )
    right_value = runtime_profile_schedule_near_optimal(
        fields,
        total_pressure,
        right,
        best_provider_priority,
        best_total_pressure,
    )
    if left_value != right_value:
        return left_value < right_value

    var left_recently_used: Int64 = 0
    if left_value == 0 and runtime_profile_schedule_field(fields, left, 8) != 0:
        left_recently_used = 1
    var right_recently_used: Int64 = 0
    if (
        right_value == 0
        and runtime_profile_schedule_field(fields, right, 8) != 0
    ):
        right_recently_used = 1
    if left_recently_used != right_recently_used:
        return left_recently_used < right_recently_used

    var left_last_selected_at = INT64_MIN
    if left_value == 0:
        left_last_selected_at = runtime_profile_schedule_field(fields, left, 9)
    var right_last_selected_at = INT64_MIN
    if right_value == 0:
        right_last_selected_at = runtime_profile_schedule_field(
            fields, right, 9
        )
    if left_last_selected_at != right_last_selected_at:
        return left_last_selected_at < right_last_selected_at

    left_value = runtime_profile_schedule_field(fields, left, 7)
    right_value = runtime_profile_schedule_field(fields, right, 7)
    if left_value != right_value:
        return left_value < right_value
    left_value = total_pressure[unsafe_offset=left]
    right_value = total_pressure[unsafe_offset=right]
    if left_value != right_value:
        return left_value < right_value
    left_value = scaled_weekly_pressure[unsafe_offset=left]
    right_value = scaled_weekly_pressure[unsafe_offset=right]
    if left_value != right_value:
        return left_value < right_value
    left_value = scaled_five_hour_pressure[unsafe_offset=left]
    right_value = scaled_five_hour_pressure[unsafe_offset=right]
    if left_value != right_value:
        return left_value < right_value

    left_value = reserve_floor[unsafe_offset=left]
    right_value = reserve_floor[unsafe_offset=right]
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_profile_schedule_field(fields, left, 3)
    right_value = runtime_profile_schedule_field(fields, right, 3)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_profile_schedule_field(fields, left, 4)
    right_value = runtime_profile_schedule_field(fields, right, 4)
    if left_value != right_value:
        return left_value > right_value

    for field in range(10, 13):
        left_value = runtime_profile_schedule_field(fields, left, Int64(field))
        right_value = runtime_profile_schedule_field(
            fields, right, Int64(field)
        )
        if left_value != right_value:
            return left_value < right_value
    left_value = runtime_profile_schedule_field(fields, left, 13)
    right_value = runtime_profile_schedule_field(fields, right, 13)
    if left_value != right_value:
        return left_value > right_value
    left_value = runtime_profile_schedule_field(fields, left, 15)
    right_value = runtime_profile_schedule_field(fields, right, 15)
    if left_value != right_value:
        return left_value < right_value
    return left < right


@export("prodex_runtime_quota_profile_schedule_batch")
def prodex_runtime_quota_profile_schedule_batch(
    fields: Pointer[mut=False, Int64, _],
    total_pressure: Pointer[mut=True, Int64, _],
    scaled_weekly_pressure: Pointer[mut=True, Int64, _],
    scaled_five_hour_pressure: Pointer[mut=True, Int64, _],
    reserve_floor: Pointer[mut=True, Int64, _],
    ordered_indices: Pointer[mut=True, Int64, _],
    ordered_count: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT:
        return 1

    for index in range(count):
        var weekly_value = runtime_profile_schedule_field(fields, index, 0)
        var five_hour_value = runtime_profile_schedule_field(fields, index, 1)
        var scale_value = runtime_profile_schedule_field(fields, index, 2)
        var weekly_remaining_value = runtime_profile_schedule_field(
            fields, index, 3
        )
        var five_hour_remaining_value = runtime_profile_schedule_field(
            fields, index, 4
        )
        var windows_complete = runtime_profile_schedule_field(fields, index, 5)
        var weekly_weight_value = runtime_profile_schedule_field(
            fields, index, 6
        )
        var provider_priority = runtime_profile_schedule_field(fields, index, 7)
        var in_selection_cooldown = runtime_profile_schedule_field(
            fields, index, 8
        )
        var quota_source = runtime_profile_schedule_field(fields, index, 12)
        var preferred = runtime_profile_schedule_field(fields, index, 13)
        var affinity_preferred = runtime_profile_schedule_field(
            fields, index, 14
        )
        var order_index = runtime_profile_schedule_field(fields, index, 15)
        if (
            weekly_value < 0
            or five_hour_value < 0
            or scale_value < 0
            or weekly_weight_value < 0
        ):
            return 2
        if (
            weekly_remaining_value < 0
            or weekly_remaining_value > 100
            or five_hour_remaining_value < 0
            or five_hour_remaining_value > 100
        ):
            return 2
        if windows_complete < 0 or windows_complete > 1:
            return 2
        if (
            provider_priority < 0
            or in_selection_cooldown < 0
            or in_selection_cooldown > 1
        ):
            return 2
        if (
            quota_source < 0
            or quota_source > 1
            or preferred < 0
            or preferred > 1
            or affinity_preferred < 0
            or affinity_preferred > 1
            or order_index < 0
        ):
            return 2

        var weekly_scaled = runtime_quota_scale_pressure(
            weekly_value, scale_value
        )
        var five_hour_scaled = runtime_quota_scale_pressure(
            five_hour_value, scale_value
        )
        var weighted_weekly = runtime_quota_saturating_mul(
            weekly_scaled, weekly_weight_value
        )
        var reserve_bias_value = INT64_MAX / 4
        if (
            windows_complete == 1
            and weekly_remaining_value != 0
            and five_hour_remaining_value != 0
        ):
            if weekly_remaining_value <= 10 or five_hour_remaining_value <= 5:
                reserve_bias_value = 1_000_000
            elif (
                weekly_remaining_value <= 20 or five_hour_remaining_value <= 10
            ):
                reserve_bias_value = 250_000
            else:
                reserve_bias_value = 0
        var total = runtime_quota_saturating_add(
            reserve_bias_value, weighted_weekly
        )
        total = runtime_quota_saturating_add(total, five_hour_scaled)
        total_pressure[unsafe_offset=index] = total
        scaled_weekly_pressure[unsafe_offset=index] = weekly_scaled
        scaled_five_hour_pressure[unsafe_offset=index] = five_hour_scaled
        if weekly_remaining_value < five_hour_remaining_value:
            reserve_floor[unsafe_offset=index] = weekly_remaining_value
        else:
            reserve_floor[unsafe_offset=index] = five_hour_remaining_value

    var best_provider_priority = INT64_MAX
    for index in range(count):
        var provider_priority = runtime_profile_schedule_field(fields, index, 7)
        if provider_priority < best_provider_priority:
            best_provider_priority = provider_priority
    var best_total_pressure = INT64_MAX
    for index in range(count):
        if (
            runtime_profile_schedule_field(fields, index, 7)
            == best_provider_priority
            and total_pressure[unsafe_offset=index] < best_total_pressure
        ):
            best_total_pressure = total_pressure[unsafe_offset=index]

    var sorted_count: Int64 = 0
    for index in range(count):
        var position = sorted_count
        while position > 0 and runtime_profile_schedule_less(
            fields,
            total_pressure,
            scaled_weekly_pressure,
            scaled_five_hour_pressure,
            reserve_floor,
            index,
            ordered_indices[unsafe_offset=position - 1],
            best_provider_priority,
            best_total_pressure,
        ):
            ordered_indices[unsafe_offset=position] = ordered_indices[
                unsafe_offset=position - 1
            ]
            position -= 1
        ordered_indices[unsafe_offset=position] = index
        sorted_count += 1

    var affinity_position: Int64 = -1
    for position in range(sorted_count):
        var index = ordered_indices[unsafe_offset=position]
        if (
            runtime_profile_schedule_field(fields, index, 14) == 1
            and runtime_profile_schedule_field(fields, index, 8) == 0
        ):
            affinity_position = position
            break
    if affinity_position > 0:
        var affinity_index = ordered_indices[unsafe_offset=affinity_position]
        var selected_index = ordered_indices[unsafe_offset=0]
        if runtime_profile_schedule_field(
            fields, affinity_index, 7
        ) == runtime_profile_schedule_field(
            fields, selected_index, 7
        ) and runtime_profile_score_within_bps(
            total_pressure[unsafe_offset=affinity_index],
            total_pressure[unsafe_offset=selected_index],
            500,
        ):
            var position = affinity_position
            while position > 0:
                ordered_indices[unsafe_offset=position] = ordered_indices[
                    unsafe_offset=position - 1
                ]
                position -= 1
            ordered_indices[unsafe_offset=0] = affinity_index

    ordered_count[unsafe_offset=0] = sorted_count
    return 0
