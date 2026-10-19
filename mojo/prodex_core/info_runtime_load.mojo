# Runtime-load aggregation over parsed log observations; all scratch is caller-owned.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_compare
from info_runway import info_runway_saturating_sub


@fieldwise_init
struct InfoLoadRow(Copyable):
    var group: UInt64
    var timestamp: Int64
    var units: UInt64
    var profile: ProdexRichStringView


def info_load_index_before(
    rows: Pointer[mut=False, InfoLoadRow, ImmUntrackedOrigin], left: Int64, right: Int64,
    group_order: Bool,
) -> Bool:
    var a = rows[unsafe_offset=left].copy()
    var b = rows[unsafe_offset=right].copy()
    if group_order:
        if a.group != b.group:
            return a.group < b.group
    elif a.timestamp != b.timestamp:
        return a.timestamp < b.timestamp
    var compared = rich_view_compare(a.profile, b.profile)
    if compared != 0:
        return compared < 0
    return left < right


def info_load_sift(
    rows: Pointer[mut=False, InfoLoadRow, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin], root: Int64, end: Int64,
    group_order: Bool,
):
    var parent = root
    while parent <= (end - 1) // 2:
        var child = parent * 2 + 1
        if child > end:
            return
        if child < end and info_load_index_before(rows, indices[unsafe_offset=child], indices[unsafe_offset=child + 1], group_order=group_order):
            child += 1
        if not info_load_index_before(rows, indices[unsafe_offset=parent], indices[unsafe_offset=child], group_order=group_order):
            return
        var value = indices[unsafe_offset=parent]
        indices[unsafe_offset=parent] = indices[unsafe_offset=child]
        indices[unsafe_offset=child] = value
        parent = child


def info_load_sort(
    rows: Pointer[mut=False, InfoLoadRow, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin], count: Int64, group_order: Bool,
):
    var start = count // 2
    while start > 0:
        start -= 1
        info_load_sift(rows, indices, start, count - 1, group_order=group_order)
    var end = count - 1
    while end > 0:
        var value = indices[unsafe_offset=0]
        indices[unsafe_offset=0] = indices[unsafe_offset=end]
        indices[unsafe_offset=end] = value
        end -= 1
        info_load_sift(rows, indices, 0, end, group_order=group_order)


def info_runtime_load_plan(
    observations_address: UInt, observation_count: Int64,
    inflight_address: UInt, inflight_count: Int64, now: Int64,
    recent_seconds: Int64, lookback_seconds: Int64, output_address: UInt, output_words: Int64,
) -> Int64:
    if observation_count < 0 or inflight_count < 0 or output_words < 0 or observation_count > 0x7FFFFFFFFFFFFFFF // 40 or inflight_count > 0x7FFFFFFFFFFFFFFF // 40:
        return 1
    if (observation_count > 0 and (observations_address == 0 or observations_address % 8 != 0)) or (inflight_count > 0 and (inflight_address == 0 or inflight_address % 8 != 0)) or output_address == 0 or output_address % 8 != 0:
        return 1
    if output_words < 5 + observation_count + inflight_count:
        return 3
    var observations = Pointer[mut=False, InfoLoadRow, ImmUntrackedOrigin](unsafe_from_address=Int(observations_address))
    var inflight = Pointer[mut=False, InfoLoadRow, ImmUntrackedOrigin](unsafe_from_address=Int(inflight_address))
    for index in range(observation_count):
        var row = observations[unsafe_offset=index].copy()
        if row.group != 0 or row.units != 0 or not rich_view_valid(row.profile, 0x7FFFFFFFFFFFFFFF):
            return 1
    for index in range(inflight_count):
        var row = inflight[unsafe_offset=index].copy()
        if row.timestamp != 0 or not rich_view_valid(row.profile, 0x7FFFFFFFFFFFFFFF):
            return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var selected = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + 40))
    var inflight_order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + 40 + UInt(observation_count) * 8))
    for index in range(inflight_count):
        inflight_order[unsafe_offset=index] = index
    info_load_sort(inflight, inflight_order, inflight_count, group_order=True)
    var active_units: UInt64 = 0
    for position in range(inflight_count):
        var row = inflight[unsafe_offset=inflight_order[unsafe_offset=position]].copy()
        if position + 1 < inflight_count:
            var next_row = inflight[unsafe_offset=inflight_order[unsafe_offset=position + 1]].copy()
            if row.group == next_row.group and rich_view_compare(row.profile, next_row.profile) == 0:
                continue
        active_units += row.units
    var recent_cutoff = info_runway_saturating_sub(now, recent_seconds)
    var lookback_cutoff = info_runway_saturating_sub(now, lookback_seconds)
    var selected_count: Int64 = 0
    var recent_count: UInt64 = 0
    var first: Int64 = 0
    var last: Int64 = 0
    for index in range(observation_count):
        var row = observations[unsafe_offset=index].copy()
        if row.timestamp >= lookback_cutoff:
            selected[unsafe_offset=selected_count] = index
            selected_count += 1
        if row.timestamp >= recent_cutoff:
            if recent_count == 0:
                first = row.timestamp
                last = row.timestamp
            else:
                first = min(first, row.timestamp)
                last = max(last, row.timestamp)
            recent_count += 1
    info_load_sort(observations, selected, selected_count, group_order=False)
    output[unsafe_offset=0] = UInt64(selected_count)
    output[unsafe_offset=1] = active_units
    output[unsafe_offset=2] = recent_count
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + 24))[] = first
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + 32))[] = last
    return 0
