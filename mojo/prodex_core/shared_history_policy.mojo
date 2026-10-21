from std.memory import Pointer

comptime SHARED_HISTORY_ABI_VERSION: Int64 = 1
comptime SHARED_HISTORY_OK: Int64 = 0
comptime SHARED_HISTORY_INVALID: Int64 = 1
comptime SHARED_HISTORY_ABI: Int64 = 4
comptime SHARED_HISTORY_MAX_LINES: Int64 = 4_000_000
comptime SHARED_HISTORY_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


def history_hash(address: UInt, length: Int64) -> UInt64:
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var value: UInt64 = 1469598103934665603 ^ UInt64(length)
    for index in range(length):
        value = (value ^ UInt64(ptr[unsafe_offset=index])) * UInt64(1099511628211)
    return value


def history_equal(
    addresses: Pointer[mut=False, UInt64, _],
    lengths: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
) -> Bool:
    var left_length = lengths[unsafe_offset=left]
    if left_length != lengths[unsafe_offset=right]:
        return False
    var left_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(addresses[unsafe_offset=left])
    )
    var right_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(addresses[unsafe_offset=right])
    )
    for index in range(left_length):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def history_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if SHARED_HISTORY_UINT64_MAX - left < right:
        return SHARED_HISTORY_UINT64_MAX
    return left + right


@export("prodex_shared_history_dedup_plan_v1")
def prodex_shared_history_dedup_plan_v1(
    abi_version: Int64,
    addresses_address: UInt,
    lengths_address: UInt,
    count: Int64,
    slots_address: UInt,
    slot_count: Int64,
    keep_address: UInt,
    max_bytes: UInt64,
    output_count_address: UInt,
    output_total_bytes_address: UInt,
    output_exceeds_address: UInt,
) abi("C") -> Int64:
    if abi_version != SHARED_HISTORY_ABI_VERSION:
        return SHARED_HISTORY_ABI
    if (
        count < 0
        or count > SHARED_HISTORY_MAX_LINES
        or slot_count < 0
        or (count > 0 and (
            addresses_address == 0
            or lengths_address == 0
            or slots_address == 0
            or keep_address == 0
            or slot_count < count
        ))
        or output_count_address == 0
        or output_total_bytes_address == 0
        or output_exceeds_address == 0
    ):
        return SHARED_HISTORY_INVALID

    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    var output_total_bytes = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_total_bytes_address)
    )
    var output_exceeds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_exceeds_address)
    )
    output_count[] = 0
    output_total_bytes[] = 0
    output_exceeds[] = 0
    if count == 0:
        return SHARED_HISTORY_OK

    var addresses = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(addresses_address)
    )
    var lengths = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(lengths_address)
    )
    var slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(slots_address)
    )
    var keep = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(keep_address)
    )

    for index in range(slot_count):
        slots[unsafe_offset=index] = 0
    for index in range(count):
        keep[unsafe_offset=index] = 0

    var unique_count: Int64 = 0
    var total_bytes: UInt64 = 0
    for index in range(count):
        var length = lengths[unsafe_offset=index]
        if length <= 0 or addresses[unsafe_offset=index] == 0:
            return SHARED_HISTORY_INVALID

        var slot = Int64(
            history_hash(UInt(addresses[unsafe_offset=index]), length) % UInt64(slot_count)
        )
        var duplicate = False
        var probing = True
        while probing:
            var stored = slots[unsafe_offset=slot]
            if stored == 0:
                probing = False
            else:
                var previous = stored - 1
                if history_equal(addresses, lengths, previous, index):
                    duplicate = True
                    probing = False
                else:
                    slot += 1
                    if slot == slot_count:
                        slot = 0

        if duplicate:
            continue

        slots[unsafe_offset=slot] = index + 1
        keep[unsafe_offset=index] = 1
        if unique_count > 0:
            total_bytes = history_saturating_add(total_bytes, UInt64(1))
        total_bytes = history_saturating_add(total_bytes, UInt64(length))
        unique_count += 1

    output_count[] = unique_count
    output_total_bytes[] = total_bytes
    output_exceeds[] = Int64(total_bytes > max_bytes)
    return SHARED_HISTORY_OK


# Chronological merge ordering for Codex history.jsonl. Missing/noninteger
# timestamps retain their original positions and separate sortable runs of
# integer-timestamp events, preserving the preexisting Rust comparison:
# two timestamped rows use timestamp then original order; otherwise
# original input order wins.
comptime SHARED_HISTORY_ORDER_ABI: Int64 = 1


def shared_history_timestamp_less(
    rows: Pointer[mut=False, Int64, _],
    left: Int64, right: Int64,
) -> Bool:
    var lhs = rows[unsafe_offset=left * 2 + 1]
    var rhs = rows[unsafe_offset=right * 2 + 1]
    if lhs != rhs:
        return lhs < rhs
    return left < right


def shared_history_order_heap_down(
    rows: Pointer[mut=False, Int64, _],
    indices: Pointer[mut=True, Int64, _],
    base: Int64, root_start: Int64, count: Int64,
):
    var root = root_start
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        var swap = root
        if shared_history_timestamp_less(
            rows,
            indices[unsafe_offset=base + swap],
            indices[unsafe_offset=base + child],
        ):
            swap = child
        if child + 1 < count and shared_history_timestamp_less(
            rows,
            indices[unsafe_offset=base + swap],
            indices[unsafe_offset=base + child + 1],
        ):
            swap = child + 1
        if swap == root:
            return
        var previous = indices[unsafe_offset=base + root]
        indices[unsafe_offset=base + root] = indices[unsafe_offset=base + swap]
        indices[unsafe_offset=base + swap] = previous
        root = swap


@export("prodex_shared_history_timestamp_order_v1")
def prodex_shared_history_timestamp_order_v1(
    abi_version: Int64,
    rows_address: UInt, row_count: Int64,
    indices_address: UInt, indices_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != SHARED_HISTORY_ORDER_ABI:
        return SHARED_HISTORY_ABI
    if (
        row_count < 0 or row_count > SHARED_HISTORY_MAX_LINES
        or (row_count > 0 and (rows_address == 0 or rows_address % 8 != 0))
        or indices_address == 0 or indices_address % 8 != 0
        or indices_capacity < row_count
    ):
        return SHARED_HISTORY_INVALID
    var rows = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows_address)
    )
    # Validate the complete input before mutating caller-owned output.
    for index in range(row_count):
        var present = rows[unsafe_offset=index * 2]
        if present != 0 and present != 1:
            return SHARED_HISTORY_INVALID

    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(indices_address)
    )
    for index in range(row_count):
        indices[unsafe_offset=index] = index

    var run_start = Int64(0)
    while run_start < row_count:
        if rows[unsafe_offset=run_start * 2] == 0:
            run_start += 1
            continue
        var run_end = run_start + 1
        while run_end < row_count and rows[unsafe_offset=run_end * 2] == 1:
            run_end += 1
        var length = run_end - run_start
        var start = length // 2
        while start > 0:
            start -= 1
            shared_history_order_heap_down(
                rows, indices, run_start, start, length,
            )
        var end = length
        while end > 1:
            end -= 1
            var saved = indices[unsafe_offset=run_start]
            indices[unsafe_offset=run_start] = indices[unsafe_offset=run_start + end]
            indices[unsafe_offset=run_start + end] = saved
            shared_history_order_heap_down(
                rows, indices, run_start, 0, end,
            )
        run_start = run_end
    return SHARED_HISTORY_OK
