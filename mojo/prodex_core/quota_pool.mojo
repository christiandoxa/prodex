from std.memory import Pointer
from runtime_math import INT64_MAX, runtime_quota_saturating_add

comptime QUOTA_POOL_INPUT_FIELD_COUNT: Int64 = 7
comptime STATUS_QUOTA_SUMMARY_ABI_VERSION: Int64 = 2
comptime STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT: Int64 = 15


def quota_status_summary_input_field(
    fields: Pointer[mut=False, Int64, _], row: Int64, field: Int64
) -> Int64:
    return fields[unsafe_offset=(row * STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT) + field]


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


@export("prodex_quota_status_summary_v2")
def prodex_quota_status_summary_v2(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    count: Int64,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != STATUS_QUOTA_SUMMARY_ABI_VERSION:
        return 4
    if (
        count < 0
        or count > INT64_MAX // STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT
        or (count > 0 and input_address == 0)
        or output_address == 0
    ):
        return 1
    if output_capacity < 12:
        return 3
    var input_fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output_fields = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )

    var compatible_profiles: Int64 = 0
    var unavailable_profiles: Int64 = 0
    var live_profiles: Int64 = 0
    var snapshot_profiles: Int64 = 0
    var five_hour_profiles: Int64 = 0
    var five_hour_remaining: Int64 = 0
    var earliest_five_hour: Int64 = 0
    var has_earliest_five_hour: Int64 = 0
    var weekly_profiles: Int64 = 0
    var weekly_remaining: Int64 = 0
    var earliest_weekly: Int64 = 0
    var has_earliest_weekly: Int64 = 0

    for row in range(count):
        var compatible = quota_status_summary_input_field(input_fields, row, 0)
        var report_succeeded = quota_status_summary_input_field(input_fields, row, 1)
        var snapshot_usable = quota_status_summary_input_field(input_fields, row, 2)
        if (compatible != 0 and compatible != 1) or (
            report_succeeded != 0 and report_succeeded != 1
        ) or (snapshot_usable != 0 and snapshot_usable != 1):
            return 1

        var report_five_present = quota_status_summary_input_field(input_fields, row, 4)
        var report_weekly_present = quota_status_summary_input_field(input_fields, row, 7)
        var cached_five_present = quota_status_summary_input_field(input_fields, row, 10)
        var cached_weekly_present = quota_status_summary_input_field(input_fields, row, 13)
        if (report_five_present != 0 and report_five_present != 1) or (
            report_weekly_present != 0 and report_weekly_present != 1
        ) or (cached_five_present != 0 and cached_five_present != 1) or (
            cached_weekly_present != 0 and cached_weekly_present != 1
        ):
            return 1
        if (report_five_present == 1 and quota_status_summary_input_field(input_fields, row, 3) < 0) or (
            report_weekly_present == 1 and quota_status_summary_input_field(input_fields, row, 6) < 0
        ) or (cached_five_present == 1 and quota_status_summary_input_field(input_fields, row, 9) < 0) or (
            cached_weekly_present == 1 and quota_status_summary_input_field(input_fields, row, 12) < 0
        ):
            return 1

        if compatible == 0:
            continue
        compatible_profiles += 1

        if report_succeeded == 0 and snapshot_usable == 0:
            unavailable_profiles += 1
            continue
        var window_offset: Int64 = 3 if report_succeeded == 1 else 9

        var five_present = quota_status_summary_input_field(
            input_fields, row, window_offset + 1
        )
        var weekly_present = quota_status_summary_input_field(
            input_fields, row, window_offset + 4
        )
        if five_present == 0 and weekly_present == 0:
            unavailable_profiles += 1
            continue

        if report_succeeded == 1:
            live_profiles += 1
        else:
            snapshot_profiles += 1

        if five_present == 1:
            var remaining = quota_status_summary_input_field(input_fields, row, window_offset)
            var reset_at = quota_status_summary_input_field(input_fields, row, window_offset + 2)
            five_hour_profiles += 1
            five_hour_remaining = runtime_quota_saturating_add(
                five_hour_remaining, remaining
            )
            if reset_at != INT64_MAX and (
                has_earliest_five_hour == 0 or reset_at < earliest_five_hour
            ):
                earliest_five_hour = reset_at
                has_earliest_five_hour = 1

        if weekly_present == 1:
            var remaining = quota_status_summary_input_field(
                input_fields, row, window_offset + 3
            )
            var reset_at = quota_status_summary_input_field(
                input_fields, row, window_offset + 5
            )
            weekly_profiles += 1
            weekly_remaining = runtime_quota_saturating_add(weekly_remaining, remaining)
            if reset_at != INT64_MAX and (
                has_earliest_weekly == 0 or reset_at < earliest_weekly
            ):
                earliest_weekly = reset_at
                has_earliest_weekly = 1

    output_fields[unsafe_offset=0] = compatible_profiles
    output_fields[unsafe_offset=1] = unavailable_profiles
    output_fields[unsafe_offset=2] = five_hour_profiles
    output_fields[unsafe_offset=3] = five_hour_remaining
    output_fields[unsafe_offset=4] = earliest_five_hour
    output_fields[unsafe_offset=5] = has_earliest_five_hour
    output_fields[unsafe_offset=6] = weekly_profiles
    output_fields[unsafe_offset=7] = weekly_remaining
    output_fields[unsafe_offset=8] = earliest_weekly
    output_fields[unsafe_offset=9] = has_earliest_weekly
    output_fields[unsafe_offset=10] = live_profiles
    output_fields[unsafe_offset=11] = snapshot_profiles
    return 0

# Choose which quota windows and availability summaries are published to the
# terminal. The host only formats wall-clock timestamps and stored totals.
comptime QUOTA_POOL_SUMMARY_SECTION_ABI_VERSION: Int64 = 1
comptime QUOTA_POOL_SECTION_READY: Int64 = 1
comptime QUOTA_POOL_SECTION_FIVE_HOUR: Int64 = 2
comptime QUOTA_POOL_SECTION_WEEKLY: Int64 = 3
comptime QUOTA_POOL_SECTION_MAIN: Int64 = 4
comptime QUOTA_POOL_SECTION_EMPTY_FIVE_HOUR: Int64 = 5
comptime QUOTA_POOL_SECTION_EMPTY_WEEKLY: Int64 = 6


@export("prodex_quota_pool_summary_sections_v1")
def prodex_quota_pool_summary_sections_v1(
    abi_version: Int64,
    openai_profiles: Int64,
    main_profiles: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != QUOTA_POOL_SUMMARY_SECTION_ABI_VERSION:
        return 4
    if (
        openai_profiles < 0 or main_profiles < 0
        or output_address == 0 or output_capacity < 4
    ):
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    output[unsafe_offset=3] = 0
    if openai_profiles > 0:
        output[unsafe_offset=0] = 3
        output[unsafe_offset=1] = QUOTA_POOL_SECTION_READY
        output[unsafe_offset=2] = QUOTA_POOL_SECTION_FIVE_HOUR
        output[unsafe_offset=3] = QUOTA_POOL_SECTION_WEEKLY
    elif main_profiles > 0:
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = QUOTA_POOL_SECTION_MAIN
    else:
        output[unsafe_offset=0] = 2
        output[unsafe_offset=1] = QUOTA_POOL_SECTION_EMPTY_FIVE_HOUR
        output[unsafe_offset=2] = QUOTA_POOL_SECTION_EMPTY_WEEKLY
    return 0


# The caller observes provider-specific availability and fetch timestamps.
# Mojo owns total/available counts and maximum timestamp across all reports,
# including failed reports. Validate every row before writing any result.
comptime QUOTA_REPORT_OVERVIEW_ABI: Int64 = 1
comptime QUOTA_REPORT_OVERVIEW_MAX_REPORTS: Int64 = 1_048_576


@export("prodex_quota_pool_report_overview_v1")
def prodex_quota_pool_report_overview_v1(
    abi_version: Int64,
    input_address: UInt,
    report_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != QUOTA_REPORT_OVERVIEW_ABI:
        return 4
    if (
        report_count < 0
        or report_count > QUOTA_REPORT_OVERVIEW_MAX_REPORTS
        or (report_count > 0 and (input_address == 0 or input_address % 8 != 0))
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    if output_capacity < 4:
        return 3
    var rows = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var available: Int64 = 0
    var latest: Int64 = 0
    var has_latest = False
    for index in range(report_count):
        var fetched_at = rows[unsafe_offset=index * 2]
        var usable = rows[unsafe_offset=index * 2 + 1]
        if usable != 0 and usable != 1:
            return 1
        available += usable
        if not has_latest or fetched_at > latest:
            latest = fetched_at
            has_latest = True
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = report_count
    output[unsafe_offset=1] = available
    output[unsafe_offset=2] = 1 if has_latest else 0
    output[unsafe_offset=3] = latest
    return 0
