from std.memory import Pointer

from context_text import (
    context_search_ascii_contains_exact,
    context_search_ascii_starts_exact,
    context_search_find_byte,
    context_text_trim_bounds,
)
from rich_types import ProdexRichStringView

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime CONTEXT_COMMAND_OUTPUT_MAX_BYTES: Int64 = 9_223_372_036_854_775_807
comptime CONTEXT_COMMAND_OUTPUT_GIT_STATUS: Int64 = 1
comptime CONTEXT_COMMAND_OUTPUT_FILE_LIST: Int64 = 2
comptime CONTEXT_COMMAND_OUTPUT_STATUS_OK: Int64 = 0
comptime CONTEXT_COMMAND_OUTPUT_STATUS_INVALID: Int64 = 1
comptime CONTEXT_COMMAND_OUTPUT_STATUS_UTF8: Int64 = 2
comptime CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY: Int64 = 3
comptime CONTEXT_COMMAND_OUTPUT_STATUS_ABI: Int64 = 4
comptime CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH: Int64 = 5

comptime CONTEXT_STATUS_STAGED: Int64 = 1
comptime CONTEXT_STATUS_MODIFIED: Int64 = 2
comptime CONTEXT_STATUS_DELETED: Int64 = 3
comptime CONTEXT_STATUS_RENAMED: Int64 = 4
comptime CONTEXT_STATUS_CONFLICTED: Int64 = 5
comptime CONTEXT_STATUS_UNTRACKED: Int64 = 6
comptime CONTEXT_STATUS_OTHER: Int64 = 7


@fieldwise_init
struct ProdexContextCommandOutputInput(Copyable):
    var operation: Int64
    var max_path_entries: UInt64
    var max_lines: UInt64
    var max_line_chars: UInt64
    var input: ProdexRichStringView


@fieldwise_init
struct ProdexContextCommandOutputRecord(Copyable):
    var occurrences: UInt64
    var category: Int64
    var offset: Int64
    var len: Int64


@fieldwise_init
struct ContextCommandOutputWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def context_command_output_put_byte(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def context_command_output_put_literal(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not context_command_output_put_byte(
            writer, ptr[unsafe_offset=index]
        ):
            return False
    return True


def context_command_output_put_range(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start:
        return False
    for index in range(start, end):
        if not context_command_output_put_byte(
            writer, ptr[unsafe_offset=index]
        ):
            return False
    return True


def context_command_output_put_i64(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _], value: Int64
) -> Bool:
    if value < 0:
        return False
    if value == 0:
        return context_command_output_put_byte(writer, 48)
    var divisor: Int64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not context_command_output_put_byte(
            writer, UInt8(remaining / divisor) + 48
        ):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def context_command_output_range_equals_literal(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    value: StringSlice,
) -> Bool:
    var length = Int64(value.byte_length())
    if end - start != length:
        return False
    var expected = value.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def context_command_output_range_contains_literal(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    value: StringSlice,
) -> Bool:
    var length = Int64(value.byte_length())
    if length == 0:
        return True
    if end - start < length:
        return False
    var expected = value.unsafe_ptr()
    for offset in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if (
                ptr[unsafe_offset=offset + index]
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def context_command_output_valid_short_status(value: UInt8) -> Bool:
    return (
        value == 32
        or value == 77
        or value == 65
        or value == 68
        or value == 82
        or value == 67
        or value == 85
        or value == 63
        or value == 33
    )


def context_command_output_short_status_line(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        end - start >= 3
        and context_command_output_valid_short_status(ptr[unsafe_offset=start])
        and context_command_output_valid_short_status(
            ptr[unsafe_offset=start + 1]
        )
        and ptr[unsafe_offset=start + 2] == 32
    )


def context_command_output_next_line(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=end)
    var line_end = start
    while line_end < end and ptr[unsafe_offset=line_end] != 10:
        line_end += 1
    result[0] = line_end
    result[1] = line_end + 1 if line_end < end else end
    return result^


def context_command_output_git_status_short_format(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var trimmed = context_text_trim_bounds(ptr, cursor, line[0])
        if trimmed[0] < trimmed[1] and not (
            context_command_output_short_status_line(ptr, cursor, line[0])
            or context_search_ascii_starts_exact["## "](ptr, cursor, line[0])
        ):
            return False
        cursor = line[1]
    return True


def context_command_output_hash_bytes(
    ptr: Pointer[mut=True, UInt8, _],
    start: Int64,
    end: Int64,
    category: Int64,
) -> UInt64:
    var hash: UInt64 = 1469598103934665603 ^ UInt64(category)
    for index in range(start, end):
        hash ^= UInt64(ptr[unsafe_offset=index])
        hash *= 1099511628211
    return hash


def context_command_output_slices_equal(
    ptr: Pointer[mut=True, UInt8, _],
    left_start: Int64,
    right_start: Int64,
    length: Int64,
) -> Bool:
    for index in range(length):
        if (
            ptr[unsafe_offset=left_start + index]
            != ptr[unsafe_offset=right_start + index]
        ):
            return False
    return True


def context_command_output_record_item(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    category: Int64,
    start: Int64,
) -> Bool:
    var length = scratch[].written - start
    if length < 0 or record_count[] >= record_capacity:
        return False
    var hash = context_command_output_hash_bytes(
        scratch[].output, start, scratch[].written, category
    )
    var slot = Int64(hash % UInt64(hash_capacity))
    for _ in range(hash_capacity):
        var stored = hash_slots[unsafe_offset=slot]
        if stored == 0:
            records[
                unsafe_offset=record_count[]
            ] = ProdexContextCommandOutputRecord(1, category, start, length)
            hash_slots[unsafe_offset=slot] = record_count[] + 1
            record_count[] += 1
            return True
        var existing = records[unsafe_offset=stored - 1].copy()
        if (
            existing.category == category
            and existing.len == length
            and context_command_output_slices_equal(
                scratch[].output, existing.offset, start, length
            )
        ):
            scratch[].written = start
            records[unsafe_offset=stored - 1].occurrences += 1
            return True
        slot = (slot + 1) % hash_capacity
    return False


def context_command_output_add_raw_item(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    category: Int64,
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var item_start = scratch[].written
    if not context_command_output_put_range(scratch, ptr, bounds[0], bounds[1]):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        category,
        item_start,
    )


def context_command_output_add_short_item(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    category: Int64,
    status: UInt8,
    ptr: Pointer[mut=False, UInt8, _],
    path_start: Int64,
    path_end: Int64,
) -> Bool:
    var path = context_text_trim_bounds(ptr, path_start, path_end)
    var item_start = scratch[].written
    if status != 63 and (
        not context_command_output_put_byte(scratch, status)
        or not context_command_output_put_byte(scratch, 32)
    ):
        return False
    if not context_command_output_put_range(scratch, ptr, path[0], path[1]):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        category,
        item_start,
    )


def context_command_output_add_conflict_item(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    ptr: Pointer[mut=False, UInt8, _],
    line_start: Int64,
    line_end: Int64,
) -> Bool:
    var status = context_text_trim_bounds(ptr, line_start, line_start + 2)
    var path = context_text_trim_bounds(ptr, line_start + 3, line_end)
    var item_start = scratch[].written
    if (
        not context_command_output_put_range(scratch, ptr, status[0], status[1])
        or not context_command_output_put_byte(scratch, 32)
        or not context_command_output_put_range(scratch, ptr, path[0], path[1])
    ):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        CONTEXT_STATUS_CONFLICTED,
        item_start,
    )


def context_command_output_add_long_item(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    category: Int64,
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    var colon = context_search_find_byte(ptr, start, end, 58)
    if colon < 0:
        return context_command_output_add_raw_item(
            records,
            record_capacity,
            record_count,
            hash_slots,
            hash_capacity,
            scratch,
            category,
            ptr,
            start,
            end,
        )
    var status = context_text_trim_bounds(ptr, start, colon)
    var path = context_text_trim_bounds(ptr, colon + 1, end)
    var item_start = scratch[].written
    if (
        not context_command_output_put_range(scratch, ptr, status[0], status[1])
        or not context_command_output_put_literal(scratch, StringSlice(": "))
        or not context_command_output_put_range(scratch, ptr, path[0], path[1])
    ):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        category,
        item_start,
    )


def context_command_output_short_category(status: UInt8, index: Bool) -> Int64:
    if status == 77 or status == 65:
        return CONTEXT_STATUS_STAGED if index else CONTEXT_STATUS_MODIFIED
    if status == 68:
        return CONTEXT_STATUS_DELETED
    if status == 82 or status == 67:
        return CONTEXT_STATUS_RENAMED
    if status == 63:
        return CONTEXT_STATUS_UNTRACKED
    if status == 32 or status == 33:
        return 0
    return CONTEXT_STATUS_OTHER


def context_command_output_copy_branch(
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    branch: Pointer[mut=True, InlineArray[Int64, 2], _],
) -> Bool:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var offset = scratch[].written
    if not context_command_output_put_range(scratch, ptr, bounds[0], bounds[1]):
        return False
    branch[][0] = offset
    branch[][1] = scratch[].written - offset
    return True


def context_command_output_long_section(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    current: Int64,
) -> Int64:
    if context_command_output_range_equals_literal(
        ptr, start, end, StringSlice("Changes to be committed:")
    ):
        return CONTEXT_STATUS_STAGED
    if context_command_output_range_equals_literal(
        ptr, start, end, StringSlice("Changes not staged for commit:")
    ):
        return CONTEXT_STATUS_MODIFIED
    if context_command_output_range_equals_literal(
        ptr, start, end, StringSlice("Untracked files:")
    ):
        return CONTEXT_STATUS_UNTRACKED
    if context_command_output_range_equals_literal(
        ptr, start, end, StringSlice("Unmerged paths:")
    ):
        return CONTEXT_STATUS_CONFLICTED
    return current
