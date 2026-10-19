from std.memory import Pointer

# Stable top-K selection for persisted Smart Context rewrite-safety records.
# The existing/incoming concatenation is host storage; only compiled Mojo
# decides freshness, timestamp ordering, and bounded history retention.
comptime SMART_CONTEXT_REWRITE_HISTORY_ABI_VERSION: Int64 = 1


@export("prodex_smart_context_rewrite_history_select_v1")
def prodex_smart_context_rewrite_history_select_v1(
    abi_version: Int64,
    timestamps_address: UInt,
    record_count: Int64,
    now_secs: UInt64,
    ttl_secs: UInt64,
    history_limit: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REWRITE_HISTORY_ABI_VERSION:
        return 4
    if (
        record_count < 0 or history_limit < 0
        or (record_count > 0 and timestamps_address == 0)
        or output_address == 0 or written_address == 0
        or output_capacity < min(record_count, history_limit)
    ):
        return 1
    var times = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(timestamps_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var count: Int64 = 0
    for index in range(record_count):
        var timestamp = times[unsafe_offset=index]
        # Rust's saturating subtraction: a zero timestamp or any future
        # timestamp is always fresh, including when the local clock moves.
        if (
            timestamp != 0
            and now_secs > timestamp
            and now_secs - timestamp > ttl_secs
        ):
            continue
        if history_limit == 0:
            continue
        # Sorted order is (observed_at_unix_secs, original_position).
        # Later positions win equal timestamps, matching Rust stable sort
        # followed by draining all but the last K entries.
        var pos: Int64 = 0
        while pos < count:
            var seen = times[unsafe_offset=Int64(output[unsafe_offset=pos])]
            if seen > timestamp:
                break
            pos += 1
        if count < history_limit:
            var next = count
            while next > pos:
                output[unsafe_offset=next] = output[unsafe_offset=next - 1]
                next -= 1
            output[unsafe_offset=pos] = UInt64(index)
            count += 1
        elif pos > 0:
            # Selection already contains K slots. The incoming timestamp
            # ranks above the oldest selected record, so drop that oldest
            # and re-insert without sorting the full input collection.
            var shift: Int64 = 0
            while shift < pos - 1:
                output[unsafe_offset=shift] = output[unsafe_offset=shift + 1]
                shift += 1
            output[unsafe_offset=pos - 1] = UInt64(index)
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = count
    return 0


# Preserve preexisting history entries including duplicates. Each incoming
# serialized sample joins the appended history only if its exact canonical
# JSON bytes have never appeared among existing and accepted incoming
# entries. The old Rust contains() and final drain policy are
# compiled together, with full-history duplicate checks before tail trim.
def smart_context_history_same_json(
    views: Pointer[mut=False, UInt64, ImmUntrackedOrigin],
    left_index: Int64,
    right_index: Int64,
) -> Bool:
    var left_length = views[unsafe_offset=left_index * 2 + 1]
    var right_length = views[unsafe_offset=right_index * 2 + 1]
    if left_length != right_length:
        return False
    if left_length == 0:
        return True
    var left = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(views[unsafe_offset=left_index * 2])
    )
    var right = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(views[unsafe_offset=right_index * 2])
    )
    for index in range(Int64(left_length)):
        if left[unsafe_offset=index] != right[unsafe_offset=index]:
            return False
    return True


@export("prodex_smart_context_unique_tail_select_v1")
def prodex_smart_context_unique_tail_select_v1(
    abi_version: Int64,
    views_address: UInt,
    existing_count: Int64,
    total_count: Int64,
    limit: Int64,
    scratch_address: UInt,
    scratch_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REWRITE_HISTORY_ABI_VERSION:
        return 4
    if (
        existing_count < 0 or total_count < existing_count
        or total_count > 0x7FFFFFFFFFFFFFFF // 2
        or limit < 0
        or (total_count > 0 and views_address == 0)
        or scratch_address == 0 or scratch_capacity < total_count
        or output_address == 0 or output_capacity < min(limit, total_count)
        or written_address == 0
    ):
        return 1
    var views = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(views_address)
    )
    # Validate all source lengths and pointers before any caller-owned writes.
    for index in range(total_count):
        var pointer = views[unsafe_offset=index * 2]
        var length = views[unsafe_offset=index * 2 + 1]
        if length > 0x7FFFFFFFFFFFFFFF or (length > 0 and pointer == 0):
            return 1
    var accepted = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var accepted_count: Int64 = 0
    for index in range(total_count):
        var new_item = index >= existing_count
        var duplicate = False
        if new_item:
            for cursor in range(accepted_count):
                if smart_context_history_same_json(
                    views, Int64(accepted[unsafe_offset=cursor]), index
                ):
                    duplicate = True
                    break
        if not duplicate:
            accepted[unsafe_offset=accepted_count] = UInt64(index)
            accepted_count += 1

    var keep = min(accepted_count, limit)
    var skipped = accepted_count - keep
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(keep):
        output[unsafe_offset=index] = accepted[unsafe_offset=skipped + index]
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = keep
    return 0
