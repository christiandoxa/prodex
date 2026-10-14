from std.memory import Pointer
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from runtime_math import INT64_MAX, INT64_MIN, runtime_quota_saturating_add

comptime INFO_RUNWAY_ESTIMATE_ABI_VERSION: Int64 = 1
comptime INFO_RUNWAY_ESTIMATE_MAX_OBSERVATIONS: Int64 = 4096

def info_runway_views_equal(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_bytes[unsafe_offset=index] != right_bytes[unsafe_offset=index]:
            return False
    return True


def info_runway_views_less(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    var common = min(Int64(left.len), Int64(right.len))
    for index in range(common):
        if left_bytes[unsafe_offset=index] < right_bytes[unsafe_offset=index]:
            return True
        if left_bytes[unsafe_offset=index] > right_bytes[unsafe_offset=index]:
            return False
    return left.len < right.len


def info_runway_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def info_runway_remaining(
    five_hour: Pointer[mut=False, Int64, _],
    weekly: Pointer[mut=False, Int64, _],
    index: Int64,
    window: Int64,
) -> Int64:
    if window == 0:
        return five_hour[unsafe_offset=index]
    return weekly[unsafe_offset=index]


def info_runway_index_less(
    left: Int64,
    right: Int64,
    profiles: Pointer[mut=False, ProdexRichStringView, _],
    timestamps: Pointer[mut=False, Int64, _],
) -> Bool:
    var left_profile = profiles[unsafe_offset=left].copy()
    var right_profile = profiles[unsafe_offset=right].copy()
    if info_runway_views_less(left_profile, right_profile):
        return True
    if info_runway_views_less(right_profile, left_profile):
        return False
    return timestamps[unsafe_offset=left] < timestamps[unsafe_offset=right]


@no_inline
def info_runway_group_earliest(
    group_start: Int64, group_end: Int64,
    ordered: Pointer[mut=True, Int64, MutUntrackedOrigin],
    five_hour: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    weekly: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    window: Int64,
) -> Int64:
    var latest_index = ordered[unsafe_offset=group_end - 1]
    var earliest_index = latest_index
    var current_window_remaining = info_runway_remaining(
        five_hour, weekly, latest_index, window
    )
    var probe = group_end - 2
    while probe >= group_start:
        var candidate_index = ordered[unsafe_offset=probe]
        var candidate_remaining = info_runway_remaining(
            five_hour, weekly, candidate_index, window
        )
        if candidate_remaining < current_window_remaining:
            break
        earliest_index = candidate_index
        current_window_remaining = candidate_remaining
        probe -= 1
    return earliest_index


@export("prodex_mojo_info_runway_estimate_v1")
def prodex_mojo_info_runway_estimate_v1(
    abi_version: Int64,
    timestamps_address: UInt,
    profiles_address: UInt,
    five_hour_address: UInt,
    weekly_address: UInt,
    ordered_indices_address: UInt,
    count: Int64,
    window: Int64,
    current_remaining: Int64,
    now: Int64,
    minimum_span_seconds: Int64,
    present_address: UInt,
    burn_per_hour_address: UInt,
    observed_profiles_address: UInt,
    observed_span_seconds_address: UInt,
    exhaust_at_address: UInt,
) abi("C") -> Int64:
    if abi_version != INFO_RUNWAY_ESTIMATE_ABI_VERSION:
        return 4
    if (
        count < 0
        or count > INFO_RUNWAY_ESTIMATE_MAX_OBSERVATIONS
        or window < 0
        or window > 1
        or minimum_span_seconds < 0
        or present_address == 0
        or burn_per_hour_address == 0
        or observed_profiles_address == 0
        or observed_span_seconds_address == 0
        or exhaust_at_address == 0
    ):
        return 1
    if count > 0 and (
        timestamps_address == 0
        or profiles_address == 0
        or five_hour_address == 0
        or weekly_address == 0
        or ordered_indices_address == 0
    ):
        return 1

    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(present_address)
    )
    var burn_per_hour = Pointer[mut=True, Float64, MutUntrackedOrigin](
        unsafe_from_address=Int(burn_per_hour_address)
    )
    var observed_profiles = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(observed_profiles_address)
    )
    var observed_span_seconds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(observed_span_seconds_address)
    )
    var exhaust_at = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(exhaust_at_address)
    )
    present[] = 0
    burn_per_hour[] = 0.0
    observed_profiles[] = 0
    observed_span_seconds[] = 0
    exhaust_at[] = now
    if count == 0:
        if current_remaining <= 0:
            present[] = 1
        return 0

    var timestamps = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(timestamps_address)
    )
    var profiles = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(profiles_address)
    )
    var five_hour = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(five_hour_address)
    )
    var weekly = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(weekly_address)
    )
    var ordered = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ordered_indices_address)
    )
    for index in range(count):
        if not rich_view_valid(profiles[unsafe_offset=index].copy(), 65_536):
            return 1
        ordered[unsafe_offset=index] = index

    # shortcut: insertion sort is bounded by 4096 observations; use heap sort if the info log cap grows.
    var position: Int64 = 1
    while position < count:
        var selected = ordered[unsafe_offset=position]
        var insertion = position
        while insertion > 0:
            var previous = ordered[unsafe_offset=insertion - 1]
            if not info_runway_index_less(selected, previous, profiles, timestamps):
                break
            ordered[unsafe_offset=insertion] = previous
            insertion -= 1
        ordered[unsafe_offset=insertion] = selected
        position += 1

    if current_remaining <= 0:
        present[] = 1
        return 0

    var total_burn_per_hour = 0.0
    var total_profiles: Int64 = 0
    var earliest_timestamp = INT64_MAX
    var latest_timestamp = INT64_MIN
    var group_start: Int64 = 0
    while group_start < count:
        var group_end = group_start + 1
        var group_profile = profiles[unsafe_offset=ordered[unsafe_offset=group_start]].copy()
        while group_end < count and info_runway_views_equal(
            group_profile,
            profiles[unsafe_offset=ordered[unsafe_offset=group_end]].copy(),
        ):
            group_end += 1
        if group_end - group_start >= 2:
            var latest_index = ordered[unsafe_offset=group_end - 1]
            var earliest_index = info_runway_group_earliest(group_start, group_end, ordered, five_hour, weekly, window)
            var burned = info_runway_saturating_sub(
                info_runway_remaining(five_hour, weekly, earliest_index, window),
                info_runway_remaining(five_hour, weekly, latest_index, window),
            )
            var span_seconds = info_runway_saturating_sub(
                timestamps[unsafe_offset=latest_index],
                timestamps[unsafe_offset=earliest_index],
            )
            if burned > 0 and span_seconds >= minimum_span_seconds:
                total_burn_per_hour += Float64(burned) * 3600.0 / Float64(span_seconds)
                total_profiles += 1
                earliest_timestamp = min(
                    earliest_timestamp, timestamps[unsafe_offset=earliest_index]
                )
                latest_timestamp = max(
                    latest_timestamp, timestamps[unsafe_offset=latest_index]
                )
        group_start = group_end

    if total_burn_per_hour <= 0.0 or total_profiles == 0:
        return 0
    var seconds_until_exhaustion = (
        Float64(current_remaining) / total_burn_per_hour * 3600.0
    )
    var exhaustion_seconds: Int64 = 0
    if seconds_until_exhaustion >= Float64(INT64_MAX):
        exhaustion_seconds = INT64_MAX
    elif seconds_until_exhaustion > 0.0:
        exhaustion_seconds = Int64(seconds_until_exhaustion)
        if Float64(exhaustion_seconds) < seconds_until_exhaustion:
            exhaustion_seconds += 1
    burn_per_hour[] = total_burn_per_hour
    observed_profiles[] = total_profiles
    observed_span_seconds[] = info_runway_saturating_sub(
        latest_timestamp, earliest_timestamp
    )
    exhaust_at[] = runtime_quota_saturating_add(now, exhaustion_seconds)
    present[] = 1
    return 0


