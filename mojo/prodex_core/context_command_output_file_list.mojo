from std.memory import Pointer

from context_command_output_common import (
    CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY,
    CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH,
    CONTEXT_COMMAND_OUTPUT_STATUS_OK,
    ContextCommandOutputWriter,
    ProdexContextCommandOutputInput,
    ProdexContextCommandOutputRecord,
    context_command_output_next_line,
    context_command_output_put_byte,
    context_command_output_put_i64,
    context_command_output_put_literal,
    context_command_output_put_range,
    context_command_output_record_item,
)
from context_text import (
    context_search_ascii_contains_exact,
    context_search_ascii_starts_exact,
    context_search_file_list_candidate,
    context_search_last_marker_end,
    context_text_codepoint_width,
    context_text_trim_bounds,
    context_text_whitespace_width,
)
from rich_text import rich_view_ptr


comptime CONTEXT_FILE_LIST_ENTRY: Int64 = 8
comptime CONTEXT_FILE_LIST_ROOT: Int64 = 9
comptime CONTEXT_FILE_LIST_EXTENSION: Int64 = 10


def context_file_list_append_record(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    category: Int64,
    start: Int64,
    length: Int64,
) -> Bool:
    if length < 0 or record_count[] >= record_capacity:
        return False
    records[unsafe_offset=record_count[]] = ProdexContextCommandOutputRecord(
        1, category, start, length
    )
    record_count[] += 1
    return True


def context_file_list_mode_valid(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start < 10:
        return False
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if not (
            value == 45
            or value == 100
            or value == 108
            or value == 99
            or value == 98
            or value == 112
            or value == 115
            or value == 68
            or value == 114
            or value == 119
            or value == 120
            or value == 83
            or value == 84
            or value == 116
            or value == 43
        ):
            return False
    return True


def context_file_list_copy_ls_path(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Int64:
    if start >= end or context_search_ascii_starts_exact["total "](
        ptr, start, end
    ):
        return -1
    var first = ptr[unsafe_offset=start]
    if not (
        first == 45
        or first == 100
        or first == 108
        or first == 99
        or first == 98
        or first == 112
        or first == 115
        or first == 68
    ):
        return -1
    var cursor = start
    var token_index: Int64 = 0
    var mode_end = start
    var item_start = scratch[].written
    while cursor < end:
        while cursor < end:
            var width = context_text_whitespace_width(ptr, cursor, end)
            if width == 0:
                break
            cursor += width
        if cursor >= end:
            break
        var token_start = cursor
        while cursor < end and context_text_whitespace_width(
            ptr, cursor, end
        ) == 0:
            cursor += context_text_codepoint_width(ptr[unsafe_offset=cursor])
        if token_index == 0:
            mode_end = cursor
        if token_index >= 8:
            if token_index > 8 and not context_command_output_put_byte(
                scratch, 32
            ):
                return -2
            if not context_command_output_put_range(
                scratch, ptr, token_start, cursor
            ):
                return -2
        token_index += 1
    if (
        token_index < 9
        or not context_file_list_mode_valid(ptr, start, mode_end)
        or scratch[].written == item_start
    ):
        scratch[].written = item_start
        return -1
    if scratch[].written - item_start == 1 and scratch[].output[
        unsafe_offset=item_start
    ] == 46:
        scratch[].written = item_start
        return -1
    if scratch[].written - item_start == 2 and scratch[].output[
        unsafe_offset=item_start
    ] == 46 and scratch[].output[unsafe_offset=item_start + 1] == 46:
        scratch[].written = item_start
        return -1
    return item_start


def context_file_list_copy_entry(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Int64:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var line_start = bounds[0]
    var line_end = bounds[1]
    if line_start >= line_end:
        return -1
    var excluded = (
        ptr[unsafe_offset=line_start] == 35
        or context_search_ascii_starts_exact["[..."](
            ptr, line_start, line_end
        )
        or context_search_ascii_contains_exact[" directories, "](
            ptr, line_start, line_end
        )
        or context_search_ascii_contains_exact["://"](
            ptr, line_start, line_end
        )
    )
    var decorated = context_search_ascii_starts_exact["./"](
        ptr, line_start, line_end
    ) or ptr[unsafe_offset=line_start] == 47 or context_search_ascii_starts_exact["|-- "](
        ptr, line_start, line_end
    ) or context_search_ascii_starts_exact["`-- "](
        ptr, line_start, line_end
    ) or context_search_ascii_contains_exact["├── "](
        ptr, line_start, line_end
    ) or context_search_ascii_contains_exact["└── "](
        ptr, line_start, line_end
    )
    var whitespace = False
    var whitespace_cursor = line_start
    while whitespace_cursor < line_end:
        var width = context_text_whitespace_width(
            ptr, whitespace_cursor, line_end
        )
        if width > 0:
            whitespace = True
            break
        whitespace_cursor += context_text_codepoint_width(
            ptr[unsafe_offset=whitespace_cursor]
        )
    if not excluded and (
        decorated
        or not whitespace
        and context_search_file_list_candidate(ptr, line_start, line_end)
    ):
        var item_start = scratch[].written
        var marker_end: Int64 = -1
        if decorated:
            var candidate = context_search_last_marker_end["|-- "](
                ptr, line_start, line_end
            )
            if candidate > marker_end:
                marker_end = candidate
            candidate = context_search_last_marker_end["`-- "](
                ptr, line_start, line_end
            )
            if candidate > marker_end:
                marker_end = candidate
            candidate = context_search_last_marker_end["├── "](
                ptr, line_start, line_end
            )
            if candidate > marker_end:
                marker_end = candidate
            candidate = context_search_last_marker_end["└── "](
                ptr, line_start, line_end
            )
            if candidate > marker_end:
                marker_end = candidate
        var suffix = context_text_trim_bounds(
            ptr,
            line_start if marker_end < 0 else marker_end,
            line_end,
        )
        for index in range(suffix[0], suffix[1]):
            var value = ptr[unsafe_offset=index]
            if value == 92:
                value = 47
            if not context_command_output_put_byte(scratch, value):
                return -2
        return item_start
    return context_file_list_copy_ls_path(
        ptr, line_start, line_end, scratch
    )


def context_file_list_add_root(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    entry_start: Int64,
    entry_end: Int64,
) -> Bool:
    var start = entry_start
    while start + 1 < entry_end and scratch[].output[
        unsafe_offset=start
    ] == 46 and scratch[].output[unsafe_offset=start + 1] == 47:
        start += 2
    while start < entry_end and scratch[].output[unsafe_offset=start] == 47:
        start += 1
    var end = start
    while end < entry_end and scratch[].output[unsafe_offset=end] != 47:
        end += 1
    var root_start = scratch[].written
    if start == end:
        if not context_command_output_put_byte(scratch, 46):
            return False
    elif not context_command_output_put_range(
        scratch, scratch[].output, start, end
    ):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        CONTEXT_FILE_LIST_ROOT,
        root_start,
    )


def context_file_list_add_extension(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    entry_start: Int64,
    entry_end: Int64,
) -> Bool:
    var end = entry_end
    while end > entry_start and scratch[].output[unsafe_offset=end - 1] == 47:
        end -= 1
    var dot: Int64 = -1
    var file_start = entry_start
    var cursor = end
    while cursor > entry_start:
        cursor -= 1
        var value = scratch[].output[unsafe_offset=cursor]
        if value == 47:
            file_start = cursor + 1
            break
        if value == 46 and dot < 0:
            dot = cursor
    var valid = dot >= file_start and dot + 1 < end and end - dot - 1 <= 12
    if valid:
        for index in range(dot + 1, end):
            var value = scratch[].output[unsafe_offset=index]
            if not (
                value >= 48
                and value <= 57
                or value >= 65
                and value <= 90
                or value >= 97
                and value <= 122
                or value == 95
                or value == 45
            ):
                valid = False
                break
    var extension_start = scratch[].written
    if valid:
        for index in range(dot + 1, end):
            var value = scratch[].output[unsafe_offset=index]
            if value >= 65 and value <= 90:
                value += 32
            if not context_command_output_put_byte(scratch, value):
                return False
    elif not context_command_output_put_literal(scratch, StringSlice("none")):
        return False
    return context_command_output_record_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        CONTEXT_FILE_LIST_EXTENSION,
        extension_start,
    )


def context_file_list_parse(
    input: ProdexContextCommandOutputInput,
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Bool:
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var entry_start: Int64 = -1
        if cursor < line[0]:
            entry_start = context_file_list_copy_entry(
                ptr, cursor, line[0], scratch
            )
        if entry_start == -2:
            return False
        if entry_start >= 0:
            var entry_end = scratch[].written
            if not context_file_list_append_record(
                records,
                record_capacity,
                record_count,
                CONTEXT_FILE_LIST_ENTRY,
                entry_start,
                entry_end - entry_start,
            ) or not context_file_list_add_root(
                records,
                record_capacity,
                record_count,
                hash_slots,
                hash_capacity,
                scratch,
                entry_start,
                entry_end,
            ) or not context_file_list_add_extension(
                records,
                record_capacity,
                record_count,
                hash_slots,
                hash_capacity,
                scratch,
                entry_start,
                entry_end,
            ):
                return False
        cursor = line[1]
    return True


def context_file_list_bytes_less(
    ptr: Pointer[mut=True, UInt8, _],
    left_start: Int64,
    left_length: Int64,
    right_start: Int64,
    right_length: Int64,
) -> Bool:
    var common = min(left_length, right_length)
    for index in range(common):
        var left = ptr[unsafe_offset=left_start + index]
        var right = ptr[unsafe_offset=right_start + index]
        if left != right:
            return left < right
    return left_length < right_length


def context_file_list_write_count_map(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    scratch: Pointer[mut=True, UInt8, _],
    category: Int64,
    label: StringSlice,
) -> Bool:
    if not context_command_output_put_literal(writer, label) or not context_command_output_put_literal(
        writer, StringSlice(": ")
    ):
        return False
    var unique: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == category:
            unique += 1
    var rendered: Int64 = 0
    while rendered < min(unique, 8):
        var best: Int64 = -1
        for index in range(record_count):
            var candidate = records[unsafe_offset=index].copy()
            if candidate.category != category:
                continue
            if best < 0:
                best = index
                continue
            var current = records[unsafe_offset=best].copy()
            if candidate.occurrences > current.occurrences or (
                candidate.occurrences == current.occurrences
                and context_file_list_bytes_less(
                    scratch,
                    candidate.offset,
                    candidate.len,
                    current.offset,
                    current.len,
                )
            ):
                best = index
        if best < 0:
            return False
        if rendered > 0 and not context_command_output_put_literal(
            writer, StringSlice(", ")
        ):
            return False
        var record = records[unsafe_offset=best].copy()
        if not context_command_output_put_range(
            writer, scratch, record.offset, record.offset + record.len
        ) or not context_command_output_put_byte(writer, 61) or not context_command_output_put_i64(
            writer, Int64(record.occurrences)
        ):
            return False
        records[unsafe_offset=best].category = -category
        rendered += 1
    if unique > 8 and (
        not context_command_output_put_literal(writer, StringSlice(" (+"))
        or not context_command_output_put_i64(writer, unique - 8)
        or not context_command_output_put_literal(writer, StringSlice(" more)"))
    ):
        return False
    return context_command_output_put_byte(writer, 10)


def context_file_list_write_truncated(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    ptr: Pointer[mut=True, UInt8, _],
    start: Int64,
    end: Int64,
    requested_max: UInt64,
) -> Bool:
    var max_chars = max(requested_max, UInt64(24))
    var char_count: Int64 = 0
    var cursor = start
    while cursor < end:
        cursor += context_text_codepoint_width(ptr[unsafe_offset=cursor])
        char_count += 1
    if UInt64(char_count) <= max_chars:
        return context_command_output_put_range(writer, ptr, start, end) and context_command_output_put_byte(
            writer, 10
        )
    var tail_chars = min(UInt64(16), max_chars / 3)
    var head_chars = (
        max_chars - tail_chars - 24
        if max_chars > tail_chars + 24
        else UInt64(0)
    )
    var head_end = start
    for _ in range(Int64(head_chars)):
        head_end += context_text_codepoint_width(ptr[unsafe_offset=head_end])
    var tail_start = end
    for _ in range(Int64(tail_chars)):
        tail_start -= 1
        while tail_start > start and ptr[unsafe_offset=tail_start] >= 0x80 and ptr[
            unsafe_offset=tail_start
        ] <= 0xBF:
            tail_start -= 1
    return (
        context_command_output_put_range(writer, ptr, start, head_end)
        and context_command_output_put_literal(writer, StringSlice(" [... "))
        and context_command_output_put_i64(
            writer, char_count - Int64(head_chars) - Int64(tail_chars)
        )
        and context_command_output_put_literal(writer, StringSlice(" chars omitted ...] "))
        and context_command_output_put_range(writer, ptr, tail_start, end)
        and context_command_output_put_byte(writer, 10)
    )


def context_file_list_write_entries(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    scratch: Pointer[mut=True, UInt8, _],
    input: ProdexContextCommandOutputInput,
    entry_count: Int64,
) -> Bool:
    var limit = max(input.max_path_entries, UInt64(1))
    var head = limit / 2
    if limit % 2 != 0:
        head += 1
    var tail = limit - head
    var position: Int64 = 0
    var omitted_written = False
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if record.category != CONTEXT_FILE_LIST_ENTRY:
            continue
        var include = UInt64(entry_count) <= limit or UInt64(position) < head or UInt64(
            entry_count - position
        ) <= tail
        if include:
            if not context_file_list_write_truncated(
                writer,
                scratch,
                record.offset,
                record.offset + record.len,
                input.max_line_chars,
            ):
                return False
        elif not omitted_written:
            if (
                not context_command_output_put_literal(writer, StringSlice("[... omitted "))
                or not context_command_output_put_i64(
                    writer,
                    entry_count - min(entry_count, Int64(head)) - min(
                        entry_count - min(entry_count, Int64(head)), Int64(tail)
                    ),
                )
                or not context_command_output_put_literal(
                    writer, StringSlice(" file-list entries ...]\n")
                )
            ):
                return False
            omitted_written = True
        position += 1
    return True


def context_command_output_file_list(
    input: ProdexContextCommandOutputInput,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    scratch_writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Int64:
    var record_count: Int64 = 0
    if not context_file_list_parse(
        input,
        records,
        record_capacity,
        Pointer(to=record_count),
        hash_slots,
        hash_capacity,
        scratch_writer,
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var entry_count: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == CONTEXT_FILE_LIST_ENTRY:
            entry_count += 1
    if entry_count == 0:
        return CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH
    if not context_command_output_put_literal(writer, StringSlice("sum: files entries=")) or not context_command_output_put_i64(
        writer, entry_count
    ) or not context_command_output_put_byte(writer, 10) or not context_file_list_write_count_map(
        writer,
        records,
        record_count,
        scratch_writer[].output,
        CONTEXT_FILE_LIST_ROOT,
        StringSlice("top roots"),
    ) or not context_file_list_write_count_map(
        writer,
        records,
        record_count,
        scratch_writer[].output,
        CONTEXT_FILE_LIST_EXTENSION,
        StringSlice("extensions"),
    ) or not context_command_output_put_literal(writer, StringSlice("entries:\n")) or not context_file_list_write_entries(
        writer,
        records,
        record_count,
        scratch_writer[].output,
        input,
        entry_count,
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK
