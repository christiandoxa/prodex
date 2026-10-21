# Completed-run retention; identifiers, locks and process ownership stay in Rust.
from std.memory import Pointer
from index_order import index_order_sort

comptime SUPER_RUN_TERMINAL_LIMIT: Int64 = 32


@fieldwise_init
struct SuperRunRetentionRow(Copyable):
    var terminal: Int64
    var created_at: UInt64
    var finished_at: UInt64
    var finished_present: Int64


def super_run_retention_compare(data: UInt, _width: Int64, left: Int64, right: Int64) -> Int64:
    var rows = Pointer[mut=False, SuperRunRetentionRow, ImmUntrackedOrigin](unsafe_from_address=Int(data))
    var a = rows[unsafe_offset=left].copy()
    var b = rows[unsafe_offset=right].copy()
    if a.terminal != b.terminal:
        return -1 if a.terminal == 1 else 1
    var a_time = a.finished_at if a.finished_present == 1 else a.created_at
    var b_time = b.finished_at if b.finished_present == 1 else b.created_at
    if a_time != b_time:
        return -1 if a_time < b_time else 1
    if a.created_at != b.created_at:
        return -1 if a.created_at < b.created_at else 1
    # BTreeMap acquisition supplies lexical run-ID order for exact time ties.
    return Int64(-1 if left < right else 1 if left > right else 0)


@export("prodex_super_run_retention_v1")
def prodex_super_run_retention_v1(
    version: Int64, rows_address: UInt, count: Int64,
    keep_address: UInt, keep_capacity: Int64, scratch_address: UInt, scratch_capacity: Int64,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if count < 0 or count > 9223372036854775807 // 32:
        return 1
    if count > 0 and (rows_address == 0 or rows_address % 8 != 0 or keep_address == 0 or scratch_address == 0 or scratch_address % 8 != 0):
        return 1
    if keep_capacity < count or scratch_capacity < count:
        return 3
    var rows = Pointer[mut=False, SuperRunRetentionRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    var terminal: Int64 = 0
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if row.terminal < 0 or row.terminal > 1 or row.finished_present < 0 or row.finished_present > 1:
            return 1
        if row.finished_present == 0 and row.finished_at != 0:
            return 1
        terminal += row.terminal
    var keep = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(keep_address))
    for index in range(count):
        keep[unsafe_offset=index] = 1
    if terminal <= SUPER_RUN_TERMINAL_LIMIT:
        return 0
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    for index in range(count):
        indices[unsafe_offset=index] = index
    index_order_sort[super_run_retention_compare](rows_address, 0, indices, count)
    for index in range(terminal - SUPER_RUN_TERMINAL_LIMIT):
        keep[unsafe_offset=indices[unsafe_offset=index]] = 0
    return 0


comptime SUPER_RUN_EVENT_LIMIT: Int64 = 256


@export("prodex_super_run_event_push_v1")
def prodex_super_run_event_push_v1(
    version: Int64, event_count: Int64, next_seq: UInt64, output_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if event_count < 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var incremented = next_seq
    if incremented < 18446744073709551615:
        incremented += 1
    output[unsafe_offset=0] = incremented
    output[unsafe_offset=1] = UInt64(max(event_count - (SUPER_RUN_EVENT_LIMIT - 1), 0))
    return 0


@export("prodex_super_run_events_page_v1")
def prodex_super_run_events_page_v1(
    version: Int64, sequences_address: UInt, count: Int64,
    after_seq: UInt64, next_seq: UInt64, limit: UInt64,
    indices_address: UInt, indices_capacity: Int64, output_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if count < 0 or count > 9223372036854775807 // 8:
        return 1
    if count > 0 and (sequences_address == 0 or sequences_address % 8 != 0 or indices_address == 0 or indices_address % 8 != 0):
        return 1
    if output_address == 0 or output_address % 8 != 0:
        return 1
    if indices_capacity < count:
        return 3
    var sequences = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(sequences_address))
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(indices_address))
    var first_seq = next_seq
    if count > 0:
        first_seq = sequences[unsafe_offset=0]
    var after_next = after_seq
    if after_next < 18446744073709551615:
        after_next += 1
    var maximum = Int64(min(limit, UInt64(SUPER_RUN_EVENT_LIMIT)))
    var written: Int64 = 0
    for index in range(count):
        if written == maximum:
            break
        if sequences[unsafe_offset=index] > after_seq:
            indices[unsafe_offset=written] = index
            written += 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = written
    output[unsafe_offset=1] = 1 if after_next < first_seq else 0
    return 0
