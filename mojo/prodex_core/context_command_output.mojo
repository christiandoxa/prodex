from std.memory import Pointer

from context_command_output_common import (
    CONTEXT_COMMAND_OUTPUT_FILE_LIST,
    CONTEXT_COMMAND_OUTPUT_GIT_STATUS,
    CONTEXT_COMMAND_OUTPUT_MAX_BYTES,
    CONTEXT_COMMAND_OUTPUT_STATUS_ABI,
    CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY,
    CONTEXT_COMMAND_OUTPUT_STATUS_INVALID,
    CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH,
    CONTEXT_COMMAND_OUTPUT_STATUS_OK,
    CONTEXT_COMMAND_OUTPUT_STATUS_UTF8,
    CONTEXT_STATUS_CONFLICTED,
    CONTEXT_STATUS_DELETED,
    CONTEXT_STATUS_MODIFIED,
    CONTEXT_STATUS_OTHER,
    CONTEXT_STATUS_RENAMED,
    CONTEXT_STATUS_STAGED,
    CONTEXT_STATUS_UNTRACKED,
    PRODEX_RICH_ABI_VERSION,
    ContextCommandOutputWriter,
    ProdexContextCommandOutputInput,
    ProdexContextCommandOutputRecord,
    context_command_output_add_conflict_item,
    context_command_output_add_long_item,
    context_command_output_add_raw_item,
    context_command_output_add_short_item,
    context_command_output_copy_branch,
    context_command_output_git_status_short_format,
    context_command_output_long_section,
    context_command_output_next_line,
    context_command_output_put_byte,
    context_command_output_put_i64,
    context_command_output_put_literal,
    context_command_output_put_range,
    context_command_output_range_contains_literal,
    context_command_output_range_equals_literal,
    context_command_output_short_category,
    context_command_output_short_status_line,
)
from context_text import (
    context_search_ascii_starts_exact,
    context_search_find_byte,
    context_text_trim_bounds,
)
from context_command_output_file_list import context_command_output_file_list
from rich_text import rich_view_ptr, rich_view_valid


@export("prodex_mojo_context_command_output_size_v1")
def prodex_mojo_context_command_output_size_v1(
    abi_version: Int64,
    input_address: UInt,
    meaningful_lines_address: UInt,
    meaningful_bytes_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return CONTEXT_COMMAND_OUTPUT_STATUS_ABI
    if (
        input_address == 0
        or meaningful_lines_address == 0
        or meaningful_bytes_address == 0
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexContextCommandOutputInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var meaningful_lines = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(meaningful_lines_address)
    )
    var meaningful_bytes = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(meaningful_bytes_address)
    )
    meaningful_lines[] = 0
    meaningful_bytes[] = 0
    var value = input[].copy()
    if (
        value.operation != CONTEXT_COMMAND_OUTPUT_GIT_STATUS
        and value.operation != CONTEXT_COMMAND_OUTPUT_FILE_LIST
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    if not rich_view_valid(value.input, CONTEXT_COMMAND_OUTPUT_MAX_BYTES):
        return CONTEXT_COMMAND_OUTPUT_STATUS_UTF8
    var ptr = rich_view_ptr(value.input)
    var length = Int64(value.input.len)
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var bounds = context_text_trim_bounds(ptr, cursor, line[0])
        if bounds[0] < bounds[1]:
            meaningful_lines[] += 1
            meaningful_bytes[] += bounds[1] - bounds[0]
        cursor = line[1]
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK


def context_command_output_add_short_status(
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
    var first = ptr[unsafe_offset=line_start]
    var second = ptr[unsafe_offset=line_start + 1]
    if first == 63 and second == 63:
        return context_command_output_add_raw_item(
            records,
            record_capacity,
            record_count,
            hash_slots,
            hash_capacity,
            scratch,
            CONTEXT_STATUS_UNTRACKED,
            ptr,
            line_start + 3,
            line_end,
        )
    if first == 85 or second == 85:
        return context_command_output_add_conflict_item(
            records,
            record_capacity,
            record_count,
            hash_slots,
            hash_capacity,
            scratch,
            ptr,
            line_start,
            line_end,
        )
    var first_category = context_command_output_short_category(first, True)
    if first_category != 0 and not context_command_output_add_short_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        first_category,
        first,
        ptr,
        line_start + 3,
        line_end,
    ):
        return False
    var second_category = context_command_output_short_category(second, False)
    if second_category != 0 and not context_command_output_add_short_item(
        records,
        record_capacity,
        record_count,
        hash_slots,
        hash_capacity,
        scratch,
        second_category,
        second,
        ptr,
        line_start + 3,
        line_end,
    ):
        return False
    return True


def context_command_output_parse_short_status(
    input: ProdexContextCommandOutputInput,
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    branch: Pointer[mut=True, InlineArray[Int64, 2], _],
) -> Bool:
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_search_ascii_starts_exact["## "](ptr, cursor, line[0]):
            if not context_command_output_copy_branch(
                scratch, ptr, cursor + 3, line[0], branch
            ):
                return False
        elif context_command_output_short_status_line(ptr, cursor, line[0]):
            if not context_command_output_add_short_status(
                records,
                record_capacity,
                record_count,
                hash_slots,
                hash_capacity,
                scratch,
                ptr,
                cursor,
                line[0],
            ):
                return False
        cursor = line[1]
    return True


def context_command_output_parse_long_status(
    input: ProdexContextCommandOutputInput,
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    branch: Pointer[mut=True, InlineArray[Int64, 2], _],
    clean: Pointer[mut=True, Bool, _],
) -> Bool:
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var section = CONTEXT_STATUS_OTHER
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var bounds = context_text_trim_bounds(ptr, cursor, line[0])
        var start = bounds[0]
        var end = bounds[1]
        if start >= end:
            cursor = line[1]
            continue
        if context_search_ascii_starts_exact["(use "](ptr, start, end):
            cursor = line[1]
            continue
        if context_search_ascii_starts_exact["On branch "](ptr, start, end):
            if not context_command_output_copy_branch(
                scratch, ptr, start + 10, end, branch
            ):
                return False
            cursor = line[1]
            continue
        if context_search_ascii_starts_exact["HEAD detached "](ptr, start, end):
            if not context_command_output_copy_branch(
                scratch, ptr, start, end, branch
            ):
                return False
            cursor = line[1]
            continue
        if context_command_output_range_contains_literal(
            ptr, start, end, StringSlice("nothing to commit")
        ) or context_command_output_range_contains_literal(
            ptr, start, end, StringSlice("working tree clean")
        ):
            clean[] = True
            cursor = line[1]
            continue
        section = context_command_output_long_section(ptr, start, end, section)
        if ptr[unsafe_offset=end - 1] == 58:
            cursor = line[1]
            continue
        if (
            section == CONTEXT_STATUS_OTHER
            and context_search_ascii_starts_exact["Your branch "](
                ptr, start, end
            )
        ):
            cursor = line[1]
            continue
        var category = section
        if section == CONTEXT_STATUS_MODIFIED:
            var colon = context_search_find_byte(ptr, start, end, 58)
            if colon >= 0:
                var status = context_text_trim_bounds(ptr, start, colon)
                if context_command_output_range_equals_literal(
                    ptr, status[0], status[1], StringSlice("deleted")
                ):
                    category = CONTEXT_STATUS_DELETED
        if (
            section == CONTEXT_STATUS_STAGED
            or section == CONTEXT_STATUS_MODIFIED
            or section == CONTEXT_STATUS_CONFLICTED
        ):
            if not context_command_output_add_long_item(
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
            ):
                return False
        elif not context_command_output_add_raw_item(
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
        ):
            return False
        cursor = line[1]
    return True


def context_command_output_write_category(
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    scratch: Pointer[mut=True, UInt8, _],
    category: Int64,
    limit: Int64,
) -> Bool:
    var count: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == category:
            count += 1
    if count == 0:
        return True
    var label_written = (
        context_command_output_put_literal(
            writer, StringSlice("staged")
        ) if category
        == CONTEXT_STATUS_STAGED else context_command_output_put_literal(
            writer, StringSlice("modified")
        ) if category
        == CONTEXT_STATUS_MODIFIED else context_command_output_put_literal(
            writer, StringSlice("deleted")
        ) if category
        == CONTEXT_STATUS_DELETED else context_command_output_put_literal(
            writer, StringSlice("renamed")
        ) if category
        == CONTEXT_STATUS_RENAMED else context_command_output_put_literal(
            writer, StringSlice("conflicted")
        ) if category
        == CONTEXT_STATUS_CONFLICTED else context_command_output_put_literal(
            writer, StringSlice("untracked")
        ) if category
        == CONTEXT_STATUS_UNTRACKED else context_command_output_put_literal(
            writer, StringSlice("other")
        )
    )
    if (
        not label_written
        or not context_command_output_put_literal(writer, StringSlice(" ("))
        or not context_command_output_put_i64(writer, count)
        or not context_command_output_put_literal(writer, StringSlice("): "))
    ):
        return False
    var rendered: Int64 = 0
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if record.category != category or rendered >= limit:
            continue
        if rendered > 0 and not context_command_output_put_literal(
            writer, StringSlice(", ")
        ):
            return False
        if not context_command_output_put_range(
            writer, scratch, record.offset, record.offset + record.len
        ):
            return False
        rendered += 1
    if count > limit and (
        not context_command_output_put_literal(writer, StringSlice(" (+"))
        or not context_command_output_put_i64(writer, count - limit)
        or not context_command_output_put_literal(writer, StringSlice(" more)"))
    ):
        return False
    return context_command_output_put_byte(writer, 10)


def context_command_output_git_status(
    input: ProdexContextCommandOutputInput,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    scratch_writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Int64:
    var record_count: Int64 = 0
    var branch = InlineArray[Int64, 2](fill=-1)
    var branch_ptr = Pointer(to=branch)
    var clean = False
    var clean_ptr = Pointer(to=clean)
    var count_ptr = Pointer(to=record_count)
    var ptr = rich_view_ptr(input.input)
    var parsed = context_command_output_parse_short_status(
        input,
        records,
        record_capacity,
        count_ptr,
        hash_slots,
        hash_capacity,
        scratch_writer,
        branch_ptr,
    ) if context_command_output_git_status_short_format(
        ptr, Int64(input.input.len)
    ) else context_command_output_parse_long_status(
        input,
        records,
        record_capacity,
        count_ptr,
        hash_slots,
        hash_capacity,
        scratch_writer,
        branch_ptr,
        clean_ptr,
    )
    if not parsed:
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    if record_count == 0 and branch[0] < 0 and not clean:
        return CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH
    if not context_command_output_put_literal(
        writer, StringSlice("sum: git status\n")
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    if branch[0] >= 0 and (
        not context_command_output_put_literal(writer, StringSlice("branch: "))
        or not context_command_output_put_range(
            writer,
            scratch_writer[].output,
            branch[0],
            branch[0] + branch[1],
        )
        or not context_command_output_put_byte(writer, 10)
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    if clean and not context_command_output_put_literal(
        writer, StringSlice("clean: true\n")
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var path_entries = max(input.max_path_entries, UInt64(1))
    var category_limit = path_entries / 6
    if path_entries % 6 != 0:
        category_limit += 1
    category_limit = min(max(category_limit, UInt64(4)), UInt64(record_count))
    for category in range(CONTEXT_STATUS_STAGED, CONTEXT_STATUS_OTHER + 1):
        if not context_command_output_write_category(
            writer,
            records,
            record_count,
            scratch_writer[].output,
            category,
            Int64(category_limit),
        ):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK


@export("prodex_mojo_context_command_output_v1")
def prodex_mojo_context_command_output_v1(
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
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return CONTEXT_COMMAND_OUTPUT_STATUS_ABI
    if (
        input_address == 0
        or output_address == 0
        or records_address == 0
        or scratch_address == 0
        or hash_slots_address == 0
        or written_address == 0
        or output_capacity <= 0
        or record_capacity <= 0
        or scratch_capacity <= 0
        or hash_capacity <= 0
        or hash_capacity & (hash_capacity - 1) != 0
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexContextCommandOutputInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var records = Pointer[
        mut=True, ProdexContextCommandOutputRecord, MutUntrackedOrigin
    ](unsafe_from_address=Int(records_address))
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var hash_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hash_slots_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var value = input[].copy()
    if (
        value.operation != CONTEXT_COMMAND_OUTPUT_GIT_STATUS
        and value.operation != CONTEXT_COMMAND_OUTPUT_FILE_LIST
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_INVALID
    if not rich_view_valid(value.input, CONTEXT_COMMAND_OUTPUT_MAX_BYTES):
        return CONTEXT_COMMAND_OUTPUT_STATUS_UTF8
    var writer = ContextCommandOutputWriter(output, output_capacity, 0)
    var scratch_writer = ContextCommandOutputWriter(
        scratch, scratch_capacity, 0
    )
    var status = (
        context_command_output_git_status(
            value,
            Pointer(to=writer),
            records,
            record_capacity,
            Pointer(to=scratch_writer),
            hash_slots,
            hash_capacity,
        )
        if value.operation == CONTEXT_COMMAND_OUTPUT_GIT_STATUS
        else context_command_output_file_list(
            value,
            Pointer(to=writer),
            records,
            record_capacity,
            Pointer(to=scratch_writer),
            hash_slots,
            hash_capacity,
        )
    )
    written[] = writer.written
    return status
