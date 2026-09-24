from std.memory import Pointer

comptime INT64_MAX: Int64 = 9223372036854775807
comptime QUOTA_POOL_INPUT_FIELD_COUNT: Int64 = 7


def quota_pool_input_field(
    fields: Pointer[mut=False, Int64, _], row: Int64, field: Int64
) -> Int64:
    return fields[unsafe_offset=(row * QUOTA_POOL_INPUT_FIELD_COUNT) + field]


@export("prodex_quota_openai_pool_aggregate_v1")
def prodex_quota_openai_pool_aggregate_v1(
    input_fields: Pointer[mut=False, Int64, _],
    output_fields: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0:
        return 1

    var profiles: Int64 = 0
    var ready_profiles: Int64 = 0
    var five_hour_profiles: Int64 = 0
    var weekly_profiles: Int64 = 0
    var ready_five_hour_profiles: Int64 = 0
    var ready_weekly_profiles: Int64 = 0
    var five_hour_remaining: Int64 = 0
    var weekly_remaining: Int64 = 0
    var ready_five_hour_remaining: Int64 = 0
    var ready_weekly_remaining: Int64 = 0
    var earliest_five_hour: Int64 = 0
    var earliest_weekly: Int64 = 0
    var has_earliest_five_hour: Int64 = 0
    var has_earliest_weekly: Int64 = 0

    for row in range(count):
        var five_present = quota_pool_input_field(input_fields, row, 1)
        var weekly_present = quota_pool_input_field(input_fields, row, 4)
        var ready = quota_pool_input_field(input_fields, row, 6)
        if (five_present != 0 and five_present != 1) or (
            weekly_present != 0 and weekly_present != 1
        ) or (ready != 0 and ready != 1):
            return 2

        if five_present == 1:
            var remaining = quota_pool_input_field(input_fields, row, 0)
            if remaining < 0 or remaining > 100:
                return 3
        if weekly_present == 1:
            var remaining = quota_pool_input_field(input_fields, row, 3)
            if remaining < 0 or remaining > 100:
                return 3

        if five_present == 0 and weekly_present == 0:
            continue
        profiles += 1
        if ready == 1:
            ready_profiles += 1

        if five_present == 1:
            var remaining = quota_pool_input_field(input_fields, row, 0)
            var reset_at = quota_pool_input_field(input_fields, row, 2)
            five_hour_profiles += 1
            five_hour_remaining += remaining
            if ready == 1:
                ready_five_hour_profiles += 1
                ready_five_hour_remaining += remaining
            if reset_at != INT64_MAX and (
                has_earliest_five_hour == 0 or reset_at < earliest_five_hour
            ):
                earliest_five_hour = reset_at
                has_earliest_five_hour = 1

        if weekly_present == 1:
            var remaining = quota_pool_input_field(input_fields, row, 3)
            var reset_at = quota_pool_input_field(input_fields, row, 5)
            weekly_profiles += 1
            weekly_remaining += remaining
            if ready == 1:
                ready_weekly_profiles += 1
                ready_weekly_remaining += remaining
            if reset_at != INT64_MAX and (
                has_earliest_weekly == 0 or reset_at < earliest_weekly
            ):
                earliest_weekly = reset_at
                has_earliest_weekly = 1

    output_fields[unsafe_offset=0] = profiles
    output_fields[unsafe_offset=1] = ready_profiles
    output_fields[unsafe_offset=2] = five_hour_profiles
    output_fields[unsafe_offset=3] = weekly_profiles
    output_fields[unsafe_offset=4] = ready_five_hour_profiles
    output_fields[unsafe_offset=5] = ready_weekly_profiles
    output_fields[unsafe_offset=6] = five_hour_remaining
    output_fields[unsafe_offset=7] = weekly_remaining
    output_fields[unsafe_offset=8] = ready_five_hour_remaining
    output_fields[unsafe_offset=9] = ready_weekly_remaining
    output_fields[unsafe_offset=10] = earliest_five_hour
    output_fields[unsafe_offset=11] = has_earliest_five_hour
    output_fields[unsafe_offset=12] = earliest_weekly
    output_fields[unsafe_offset=13] = has_earliest_weekly
    return 0
