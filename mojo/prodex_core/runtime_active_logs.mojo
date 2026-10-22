from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr
from core_file_policy import core_file_dynamic_prefix, core_file_parse_u32_range
from index_order import index_order_sort

comptime ACTIVE_LOG_ABI: Int64 = 1
comptime ACTIVE_LOG_MAX: Int64 = 9_223_372_036_854_775_807


def active_log_pid(name: ProdexRichStringView, prefix: ProdexRichStringView) -> Tuple[Bool, UInt64]:
    if name.len <= prefix.len + 1 or not core_file_dynamic_prefix(name, prefix):
        return (False, UInt64(0))
    var bytes = rich_view_ptr(name)
    if bytes[unsafe_offset=Int64(prefix.len)] != 45:
        return (False, UInt64(0))
    var start = Int64(prefix.len) + 1
    var end = start
    while end < Int64(name.len) and bytes[unsafe_offset=end] != 45:
        end += 1
    if end == Int64(name.len):
        return (False, UInt64(0))
    if bytes[unsafe_offset=start] == 43:
        start += 1
    return core_file_parse_u32_range(name, start, end)


@export("prodex_runtime_log_pid_v1")
def prodex_runtime_log_pid_v1(
    abi: Int64, name_address: UInt64, name_length: Int64,
    prefix_address: UInt64, prefix_length: Int64, output_address: UInt64,
) abi("C") -> Int64:
    if abi != ACTIVE_LOG_ABI:
        return 4
    if name_length < 0 or prefix_length < 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    var name = ProdexRichStringView(UInt(name_address), UInt(name_length))
    var prefix = ProdexRichStringView(UInt(prefix_address), UInt(prefix_length))
    if not rich_view_valid(name, ACTIVE_LOG_MAX) or not rich_view_valid(prefix, ACTIVE_LOG_MAX):
        return 1
    var result = active_log_pid(name, prefix)
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = UInt64(result[0])
    output[unsafe_offset=1] = result[1]
    return 0


def active_log_process_compare(data: UInt, width: Int64, left: Int64, right: Int64) -> Int64:
    var rows = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(data))
    var a = rows[unsafe_offset=left * width]
    var b = rows[unsafe_offset=right * width]
    return Int64(-1 if a < b else (1 if a > b else 0))


def active_log_candidate_compare(data: UInt, width: Int64, left: Int64, right: Int64) -> Int64:
    var pids = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(data))
    var a = pids[unsafe_offset=left]
    var b = pids[unsafe_offset=right]
    if a != b:
        return Int64(-1 if a < b else 1)
    return Int64(-1 if left > right else (1 if left < right else 0))


def active_log_owner_present(
    rows: Pointer[mut=False, UInt64, _], owners: Pointer[mut=True, Int64, _],
    count: Int64, pid: UInt64,
) -> Bool:
    var first: Int64 = 0
    var end = count
    while first < end:
        var middle = first + (end - first) // 2
        var value = rows[unsafe_offset=owners[unsafe_offset=middle] * 2]
        if value < pid:
            first = middle + 1
        else:
            end = middle
    return first < count and rows[unsafe_offset=owners[unsafe_offset=first] * 2] == pid


@export("prodex_active_runtime_logs_v1")
def prodex_active_runtime_logs_v1(
    abi: Int64, process_address: UInt64, process_count: Int64,
    pids_address: UInt64, path_count: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    output_address: UInt64, output_capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != ACTIVE_LOG_ABI:
        return 4
    if (
        process_count < 0 or process_count > ACTIVE_LOG_MAX // 16
        or path_count < 0 or path_count > ACTIVE_LOG_MAX // 8
        or process_count > ACTIVE_LOG_MAX // 8 - path_count
        or scratch_count < process_count + path_count or output_capacity < path_count
        or metadata_address == 0 or metadata_address % 8 != 0
    ):
        return 1
    if (
        (process_count > 0 and (process_address == 0 or process_address % 8 != 0))
        or (path_count > 0 and (pids_address == 0 or pids_address % 8 != 0 or output_address == 0 or output_address % 8 != 0))
        or (process_count + path_count > 0 and (scratch_address == 0 or scratch_address % 8 != 0))
    ):
        return 1
    var processes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(process_address))
    var pids = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(pids_address))
    for index in range(process_count):
        if processes[unsafe_offset=index * 2] > 4_294_967_295 or processes[unsafe_offset=index * 2 + 1] > 1:
            return 1
    for index in range(path_count):
        var pid = pids[unsafe_offset=index]
        if pid > 4_294_967_295 and pid != 18_446_744_073_709_551_615:
            return 1

    var owners = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var owner_count: Int64 = 0
    for index in range(process_count):
        if processes[unsafe_offset=index * 2 + 1] == 1:
            owners[unsafe_offset=owner_count] = index
            owner_count += 1
    index_order_sort[active_log_process_compare](UInt(process_address), 2, owners, owner_count)
    var indices_address = scratch_address + UInt64(process_count) * 8
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(indices_address))
    var candidate_count: Int64 = 0
    for index in range(path_count):
        var pid = pids[unsafe_offset=index]
        if active_log_owner_present(processes, owners, owner_count, pid):
            indices[unsafe_offset=candidate_count] = index
            candidate_count += 1
    index_order_sort[active_log_candidate_compare](UInt(pids_address), 1, indices, candidate_count)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written: Int64 = 0
    var previous_pid: Int64 = -1
    for index in range(candidate_count):
        var selected = indices[unsafe_offset=index]
        var pid = Int64(pids[unsafe_offset=selected])
        if pid != previous_pid:
            output[unsafe_offset=written] = selected
            written += 1
            previous_pid = pid
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = Int64(owner_count > 0)
    metadata[unsafe_offset=1] = written
    return 0
