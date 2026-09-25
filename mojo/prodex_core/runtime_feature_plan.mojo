from std.math import isfinite
from std.memory import Pointer

comptime RUNTIME_FEATURE_PLAN_ABI_VERSION: Int64 = 1
comptime RUNTIME_FEATURE_PLAN_MAX_U64: UInt64 = 0xFFFFFFFFFFFFFFFF


def runtime_feature_reminder_sift_down(
    values: Pointer[mut=True, UInt64, MutUntrackedOrigin],
    root_start: Int64,
    end: Int64,
):
    var root = root_start
    while root * 2 + 1 < end:
        var child = root * 2 + 1
        if child + 1 < end and values[unsafe_offset=child] < values[
            unsafe_offset=child + 1
        ]:
            child += 1
        if values[unsafe_offset=root] >= values[unsafe_offset=child]:
            return
        var current = values[unsafe_offset=root]
        values[unsafe_offset=root] = values[unsafe_offset=child]
        values[unsafe_offset=child] = current
        root = child


def runtime_feature_sort_reminders(
    values: Pointer[mut=True, UInt64, MutUntrackedOrigin], count: Int64
):
    var start = count // 2 - 1
    while start >= 0:
        runtime_feature_reminder_sift_down(values, start, count)
        start -= 1

    var end = count - 1
    while end > 0:
        var first = values[unsafe_offset=0]
        values[unsafe_offset=0] = values[unsafe_offset=end]
        values[unsafe_offset=end] = first
        runtime_feature_reminder_sift_down(values, 0, end)
        end -= 1


def runtime_feature_reminder_percent(limit: UInt64, percent: UInt64) -> UInt64:
    if limit > RUNTIME_FEATURE_PLAN_MAX_U64 // percent:
        return RUNTIME_FEATURE_PLAN_MAX_U64 // 100
    return (limit * percent) // 100


@export("prodex_mojo_runtime_feature_plan_v1")
def prodex_mojo_runtime_feature_plan_v1(
    abi_version: Int64,
    fields_address: UInt64,
    values_address: UInt64,
    weights_address: UInt64,
    configured_reminders_address: UInt64,
    configured_reminders_count: Int64,
    output_address: UInt64,
    output_reminders_address: UInt64,
    output_reminders_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_FEATURE_PLAN_ABI_VERSION:
        return 4
    if (
        fields_address == 0
        or values_address == 0
        or weights_address == 0
        or output_address == 0
        or configured_reminders_count < 0
        or configured_reminders_count > 0x7FFFFFFFFFFFFFFF // 8
        or output_reminders_capacity < 0
        or output_reminders_address == 0
        or (
            configured_reminders_count > 0
            and configured_reminders_address == 0
        )
    ):
        return 1

    var fields = Pointer[mut=False, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var values = Pointer[mut=False, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var weights = Pointer[mut=False, Float64, MutUntrackedOrigin](
        unsafe_from_address=Int(weights_address)
    )
    var configured_reminders = Pointer[mut=False, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(configured_reminders_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var output_reminders = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_reminders_address)
    )

    var web_search_mode = fields[unsafe_offset=0]
    var rollout_limit_present = fields[unsafe_offset=1]
    var sampling_weight_present = fields[unsafe_offset=2]
    var prefill_weight_present = fields[unsafe_offset=3]
    var current_time_reminder = fields[unsafe_offset=4]
    var interval_present = fields[unsafe_offset=5]
    var clock_source = fields[unsafe_offset=6]
    var respect_system_proxy = fields[unsafe_offset=7]
    var no_respect_system_proxy = fields[unsafe_offset=8]

    if (
        web_search_mode < -1
        or web_search_mode > 3
        or rollout_limit_present < 0
        or rollout_limit_present > 1
        or sampling_weight_present < 0
        or sampling_weight_present > 1
        or prefill_weight_present < 0
        or prefill_weight_present > 1
        or current_time_reminder < 0
        or current_time_reminder > 1
        or interval_present < 0
        or interval_present > 1
        or clock_source < -1
        or clock_source > 1
        or respect_system_proxy < 0
        or respect_system_proxy > 1
        or no_respect_system_proxy < 0
        or no_respect_system_proxy > 1
    ):
        return 1

    var rollout_limit = values[unsafe_offset=0]
    var interval = values[unsafe_offset=1]
    var sampling_weight = weights[unsafe_offset=0]
    var prefill_weight = weights[unsafe_offset=1]
    var rollout_enabled = rollout_limit_present == 1 and rollout_limit > 1
    var reminder_count: Int64 = 0
    if rollout_enabled:
        if output_reminders_capacity < max(configured_reminders_count, 3):
            return 3
        for index in range(configured_reminders_count):
            var reminder = configured_reminders[unsafe_offset=index]
            if reminder > 0 and reminder < rollout_limit:
                output_reminders[unsafe_offset=reminder_count] = reminder
                reminder_count += 1

        if reminder_count == 0:
            var reminder = runtime_feature_reminder_percent(rollout_limit, 75)
            if reminder > 0 and reminder < rollout_limit:
                output_reminders[unsafe_offset=reminder_count] = reminder
                reminder_count += 1
            reminder = runtime_feature_reminder_percent(rollout_limit, 50)
            if reminder > 0 and reminder < rollout_limit:
                output_reminders[unsafe_offset=reminder_count] = reminder
                reminder_count += 1
            reminder = runtime_feature_reminder_percent(rollout_limit, 25)
            if reminder > 0 and reminder < rollout_limit:
                output_reminders[unsafe_offset=reminder_count] = reminder
                reminder_count += 1
            if reminder_count == 0:
                output_reminders[unsafe_offset=0] = rollout_limit - 1
                reminder_count = 1

        runtime_feature_sort_reminders(output_reminders, reminder_count)
        var unique_count: Int64 = 0
        for read_index in range(reminder_count):
            var reminder = output_reminders[unsafe_offset=read_index]
            if unique_count == 0 or reminder != output_reminders[
                unsafe_offset=unique_count - 1
            ]:
                output_reminders[unsafe_offset=unique_count] = reminder
                unique_count += 1
        var left: Int64 = 0
        var right = unique_count - 1
        while left < right:
            var first = output_reminders[unsafe_offset=left]
            output_reminders[unsafe_offset=left] = output_reminders[
                unsafe_offset=right
            ]
            output_reminders[unsafe_offset=right] = first
            left += 1
            right -= 1
        reminder_count = unique_count
    var time_reminder_enabled = (
        current_time_reminder == 1 or interval_present == 1 or clock_source >= 0
    )
    var interval_enabled = interval_present == 1 and interval > 0
    var effective_clock_source = clock_source if time_reminder_enabled else -1
    var proxy_override: Int64 = -1
    if respect_system_proxy == 1:
        proxy_override = 1
    elif no_respect_system_proxy == 1:
        proxy_override = 0

    output[unsafe_offset=0] = web_search_mode
    output[unsafe_offset=1] = 1 if rollout_enabled else 0
    output[unsafe_offset=2] = reminder_count
    output[unsafe_offset=3] = 1 if (
        rollout_enabled
        and sampling_weight_present == 1
        and sampling_weight >= 0.0
        and isfinite(sampling_weight)
    ) else 0
    output[unsafe_offset=4] = 1 if (
        rollout_enabled
        and prefill_weight_present == 1
        and prefill_weight >= 0.0
        and isfinite(prefill_weight)
    ) else 0
    output[unsafe_offset=5] = 1 if time_reminder_enabled else 0
    output[unsafe_offset=6] = 1 if interval_enabled else 0
    output[unsafe_offset=7] = effective_clock_source
    output[unsafe_offset=8] = proxy_override
    return 0
