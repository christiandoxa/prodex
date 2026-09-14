from std.memory import Pointer

from context_command_output_common import (
    CONTEXT_COMMAND_OUTPUT_SEARCH,
    CONTEXT_COMMAND_OUTPUT_STATUS_ABI,
    CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY,
    CONTEXT_COMMAND_OUTPUT_STATUS_INVALID,
    CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH,
    CONTEXT_COMMAND_OUTPUT_STATUS_OK,
    CONTEXT_COMMAND_OUTPUT_STATUS_UTF8,
    CONTEXT_COMMAND_OUTPUT_MAX_BYTES,
    PRODEX_RICH_ABI_VERSION,
    ContextCommandOutputWriter,
    ProdexContextCommandOutputInput,
    ProdexContextCommandOutputRecord,
    context_command_output_next_line,
    context_command_output_put_byte,
    context_command_output_put_i64,
    context_command_output_put_literal,
    context_command_output_put_range,
    context_command_output_record_item_index,
)
from context_command_output_file_list import (
    context_file_list_bytes_less,
    context_file_list_write_truncated,
)
from context_text import (
    CONTEXT_GIT_SEARCH_DIRECT_MATCH,
    CONTEXT_GIT_SEARCH_HEADING_MATCH,
    CONTEXT_GIT_SEARCH_HEADING_PATH,
    CONTEXT_GIT_SEARCH_JSON_LINE,
    CONTEXT_GIT_SEARCH_JSON_MATCH,
    context_search_classify_line,
    context_text_trim_bounds,
)
from rich_text import rich_view_ptr, rich_view_valid

comptime CONTEXT_SEARCH_GROUP: Int64 = 11
comptime CONTEXT_SEARCH_MATCH: Int64 = 12
comptime CONTEXT_SEARCH_META: Int64 = 13
comptime CONTEXT_SEARCH_OTHER: Int64 = 14


def context_search_append_record(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    occurrences: UInt64,
    category: Int64,
    offset: Int64,
    length: Int64,
) -> Bool:
    if length < 0 or record_count[] >= record_capacity:
        return False
    records[unsafe_offset=record_count[]] = ProdexContextCommandOutputRecord(
        occurrences, category, offset, length
    )
    record_count[] += 1
    return True


def context_search_add_match(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    path: Pointer[mut=False, UInt8, _],
    path_length: Int64,
    text: Pointer[mut=False, UInt8, _],
    text_length: Int64,
    line_number: Int64,
) -> Bool:
    var path_start = scratch[].written
    if not context_command_output_put_range(scratch, path, 0, path_length):
        return False
    var group = context_command_output_record_item_index(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        CONTEXT_SEARCH_GROUP,
        path_start,
    )
    if group < 0:
        return False
    var text_start = scratch[].written
    if not context_command_output_put_range(scratch, text, 0, text_length):
        return False
    if not context_search_append_record(
        records,
        record_capacity,
        record_count,
        UInt64(group),
        CONTEXT_SEARCH_MATCH,
        text_start,
        text_length,
    ):
        return False
    return context_search_append_record(
        records,
        record_capacity,
        record_count,
        UInt64(line_number) if line_number >= 0 else 0,
        CONTEXT_SEARCH_META,
        Int64(1) if line_number >= 0 else Int64(0),
        0,
    )


def context_search_parse(
    input: ProdexContextCommandOutputInput,
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    path_output: Pointer[mut=True, UInt8, _],
    path_capacity: Int64,
    text_output: Pointer[mut=True, UInt8, _],
    text_capacity: Int64,
) -> Bool:
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var heading_start: Int64 = -1
    var heading_length: Int64 = 0
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var line_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(input.input.ptr) + Int(cursor)
        )
        var result = context_search_classify_line(
            line_ptr,
            line[0] - cursor,
            False,
            path_output,
            path_capacity,
            text_output,
            text_capacity,
        )
        var flags = result[0]
        if flags == CONTEXT_GIT_SEARCH_DIRECT_MATCH or flags & CONTEXT_GIT_SEARCH_JSON_MATCH != 0:
            if not context_search_add_match(
                records,
                record_capacity,
                record_count,
                hash_slots,
                hash_capacity,
                scratch,
                path_output,
                result[2],
                text_output,
                result[3],
                result[1],
            ):
                return False
        else:
            var heading_present = heading_start >= 0
            var heading_result = context_search_classify_line(
                line_ptr,
                line[0] - cursor,
                heading_present,
                path_output,
                path_capacity,
                text_output,
                text_capacity,
            )
            if heading_result[0] == CONTEXT_GIT_SEARCH_HEADING_MATCH:
                for index in range(heading_length):
                    path_output[unsafe_offset=index] = scratch[].output[
                        unsafe_offset=heading_start + index
                    ]
                if not context_search_add_match(
                    records,
                    record_capacity,
                    record_count,
                    hash_slots,
                    hash_capacity,
                    scratch,
                    path_output,
                    heading_length,
                    text_output,
                    heading_result[3],
                    heading_result[1],
                ):
                    return False
            elif flags == CONTEXT_GIT_SEARCH_HEADING_PATH:
                heading_start = scratch[].written
                heading_length = result[2]
                if not context_command_output_put_range(
                    scratch, path_output, 0, heading_length
                ):
                    return False
            elif flags & CONTEXT_GIT_SEARCH_JSON_LINE == 0 and line[0] > cursor:
                var bounds = context_text_trim_bounds(
                    line_ptr, 0, line[0] - cursor
                )
                var other_start = scratch[].written
                if bounds[0] < bounds[1] and (
                    not context_command_output_put_range(
                        scratch, line_ptr, bounds[0], bounds[1]
                    )
                    or not context_search_append_record(
                    records,
                    record_capacity,
                    record_count,
                    1,
                    CONTEXT_SEARCH_OTHER,
                    other_start,
                    scratch[].written - other_start,
                )):
                    return False
        cursor = line[1]
    return True


def context_search_write_groups(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    scratch: Pointer[mut=True, UInt8, _],
    input: ProdexContextCommandOutputInput,
) -> Bool:
    var groups: Int64 = 0
    var matches: Int64 = 0
    for index in range(record_count):
        var category = records[unsafe_offset=index].category
        if category == CONTEXT_SEARCH_GROUP:
            groups += 1
        elif category == CONTEXT_SEARCH_MATCH:
            matches += 1
    if not context_command_output_put_literal(writer, StringSlice("sum: search matches=")) or not context_command_output_put_i64(
        writer, matches
    ) or not context_command_output_put_literal(writer, StringSlice(", files=")) or not context_command_output_put_i64(
        writer, groups
    ) or not context_command_output_put_byte(writer, 10):
        return False
    for _ in range(groups):
        var best: Int64 = -1
        for index in range(record_count):
            var candidate = records[unsafe_offset=index].copy()
            if candidate.category != CONTEXT_SEARCH_GROUP:
                continue
            if best < 0:
                best = index
            else:
                var current = records[unsafe_offset=best].copy()
                if context_file_list_bytes_less(
                    scratch,
                    candidate.offset,
                    candidate.len,
                    current.offset,
                    current.len,
                ):
                    best = index
        if best < 0:
            return False
        var group = records[unsafe_offset=best].copy()
        if not context_command_output_put_range(
            writer, scratch, group.offset, group.offset + group.len
        ) or not context_command_output_put_literal(writer, StringSlice(" (")) or not context_command_output_put_i64(
            writer, Int64(group.occurrences)
        ) or not context_command_output_put_literal(writer, StringSlice(" matches):\n")):
            return False
        var rendered: UInt64 = 0
        var limit = max(input.max_search_matches, UInt64(1))
        for index in range(record_count):
            var record = records[unsafe_offset=index].copy()
            if record.category != CONTEXT_SEARCH_MATCH or record.occurrences != UInt64(best):
                continue
            if rendered < limit:
                if not context_command_output_put_literal(writer, StringSlice("  ")):
                    return False
                var meta = records[unsafe_offset=index + 1].copy()
                if meta.offset == 1 and (
                    not context_command_output_put_i64(writer, Int64(meta.occurrences))
                    or not context_command_output_put_literal(writer, StringSlice(": "))
                ):
                    return False
                if not context_file_list_write_truncated(
                    writer,
                    scratch,
                    record.offset,
                    record.offset + record.len,
                    input.max_line_chars,
                ):
                    return False
            rendered += 1
        if rendered > limit and (
            not context_command_output_put_literal(writer, StringSlice("  [... "))
            or not context_command_output_put_i64(writer, Int64(rendered - limit))
            or not context_command_output_put_literal(
                writer, StringSlice(" more matches in this file ...]\n")
            )
        ):
            return False
        records[unsafe_offset=best].category = -CONTEXT_SEARCH_GROUP
    return True


def context_search_write_other(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    scratch: Pointer[mut=True, UInt8, _],
    max_line_chars: UInt64,
) -> Bool:
    var count: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == CONTEXT_SEARCH_OTHER:
            count += 1
    if count == 0:
        return True
    if not context_command_output_put_literal(writer, StringSlice("other lines (")) or not context_command_output_put_i64(
        writer, count
    ) or not context_command_output_put_literal(writer, StringSlice("):\n")):
        return False
    var rendered: Int64 = 0
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if record.category != CONTEXT_SEARCH_OTHER or rendered >= 4:
            continue
        if not context_command_output_put_literal(writer, StringSlice("  ")) or not context_file_list_write_truncated(
            writer,
            scratch,
            record.offset,
            record.offset + record.len,
            max_line_chars,
        ):
            return False
        rendered += 1
    if count > 4:
        return context_command_output_put_literal(writer, StringSlice("  [... ")) and context_command_output_put_i64(
            writer, count - 4
        ) and context_command_output_put_literal(writer, StringSlice(" more other lines ...]\n"))
    return True


@export("prodex_mojo_context_search_output_v1")
def prodex_mojo_context_search_output_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    records_address: UInt,
    record_capacity: Int64,
    scratch_address: UInt,
    scratch_capacity: Int64,
    hash_slots_address: UInt,
    hash_capacity: Int64,
    path_output_address: UInt,
    path_capacity: Int64,
    text_output_address: UInt,
    text_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return CONTEXT_COMMAND_OUTPUT_STATUS_ABI
    if input_address == 0 or output_address == 0 or records_address == 0 or scratch_address == 0 or hash_slots_address == 0 or path_output_address == 0 or text_output_address == 0 or written_address == 0 or output_capacity <= 0 or record_capacity <= 0 or scratch_capacity <= 0 or hash_capacity <= 0 or hash_capacity & (hash_capacity - 1) != 0 or path_capacity <= 0 or text_capacity <= 0:
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    var input_pointer = Pointer[mut=False, ProdexContextCommandOutputInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))
    var input = input_pointer[].copy()
    if input.operation != CONTEXT_COMMAND_OUTPUT_SEARCH:
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    if not rich_view_valid(input.input, CONTEXT_COMMAND_OUTPUT_MAX_BYTES):
        return CONTEXT_COMMAND_OUTPUT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var records = Pointer[mut=True, ProdexContextCommandOutputRecord, MutUntrackedOrigin](unsafe_from_address=Int(records_address))
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var hash_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(hash_slots_address))
    var path_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(path_output_address))
    var text_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(text_output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    var writer = ContextCommandOutputWriter(output, output_capacity, 0)
    var scratch_writer = ContextCommandOutputWriter(scratch, scratch_capacity, 0)
    var record_count: Int64 = 0
    if not context_search_parse(input, records, record_capacity, Pointer(to=record_count), hash_slots, hash_capacity, Pointer(to=scratch_writer), path_output, path_capacity, text_output, text_capacity):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var match_count: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == CONTEXT_SEARCH_MATCH:
            match_count += 1
    if match_count == 0:
        return CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH
    if not context_search_write_groups(Pointer(to=writer), records, record_count, scratch, input) or not context_search_write_other(Pointer(to=writer), records, record_count, scratch, input.max_line_chars):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    written[] = writer.written
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK
