from std.collections import Array

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
    context_command_output_slices_equal,
)
from context_command_output_file_list import context_file_list_write_truncated
from context_text import (
    context_search_ascii_contains_exact,
    context_search_ascii_starts_exact,
    context_search_find_byte,
    context_text_trim_bounds,
)
from rich_text import rich_view_ptr

comptime GIT_LOG_COMMIT: Int64 = 20
comptime GIT_LOG_METADATA: Int64 = 21
comptime GIT_LOG_SUBJECT: Int64 = 22
comptime GIT_LOG_STAT_LINE: Int64 = 23
comptime GIT_LOG_STAT_SUMMARY: Int64 = 24


def git_log_hash_valid(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start < 7 or end - start > 64:
        return False
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if not (
            value >= 48 and value <= 57
            or value >= 65 and value <= 70
            or value >= 97 and value <= 102
        ):
            return False
    return True


def git_log_header_bounds(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    var bounds = context_text_trim_bounds(ptr, start, end)
    var line_start = bounds[0]
    var line_end = bounds[1]
    if context_search_ascii_starts_exact["commit "](
        ptr, line_start, line_end
    ):
        var hash_start = line_start + 7
        var hash_end = hash_start
        while hash_end < line_end and ptr[unsafe_offset=hash_end] != 32:
            hash_end += 1
        if git_log_hash_valid(ptr, hash_start, hash_end):
            result[0] = line_start
            result[1] = hash_end
        return result^
    var separator = context_search_find_byte(
        ptr, line_start, line_end, 32
    )
    if separator > line_start and separator + 1 < line_end and git_log_hash_valid(
        ptr, line_start, separator
    ):
        result[0] = line_start
        result[1] = line_end
    return result^


def git_log_stat_line(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var line_start = bounds[0]
    var line_end = bounds[1]
    var pipe: Int64 = -1
    for index in range(line_start, line_end - 2):
        if ptr[unsafe_offset=index] == 32 and ptr[unsafe_offset=index + 1] == 124 and ptr[unsafe_offset=index + 2] == 32:
            pipe = index
            break
    if pipe <= line_start or pipe + 3 >= line_end:
        return False
    if context_search_ascii_starts_exact["Bin "](
        ptr, pipe + 3, line_end
    ):
        return True
    for index in range(pipe + 3, line_end):
        var value = ptr[unsafe_offset=index]
        if value == 43 or value == 45:
            return True
    var token_end = pipe + 3
    while token_end < line_end and ptr[unsafe_offset=token_end] != 32:
        var value = ptr[unsafe_offset=token_end]
        if value < 48 or value > 57:
            return False
        token_end += 1
    return token_end > pipe + 3


def git_log_stat_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_search_ascii_contains_exact[" file changed"](ptr, start, end)
        or context_search_ascii_contains_exact[" files changed"](ptr, start, end)
        or context_search_ascii_contains_exact[" insertion"](ptr, start, end)
        or context_search_ascii_contains_exact[" deletion"](ptr, start, end)
    )


def git_log_append_unique(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
    commit: Int64,
    category: Int64,
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var item_start = scratch[].written
    if not context_command_output_put_range(
        scratch, ptr, bounds[0], bounds[1]
    ):
        return False
    var length = scratch[].written - item_start
    for index in range(record_count[]):
        var existing = records[unsafe_offset=index].copy()
        if existing.category == category and existing.occurrences == UInt64(commit) and existing.len == length and context_command_output_slices_equal(
            scratch[].output, existing.offset, item_start, length
        ):
            scratch[].written = item_start
            return True
    if record_count[] >= record_capacity:
        return False
    records[unsafe_offset=record_count[]] = ProdexContextCommandOutputRecord(
        UInt64(commit), category, item_start, length
    )
    record_count[] += 1
    return True


def git_log_parse(
    input: ProdexContextCommandOutputInput,
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    record_count: Pointer[mut=True, Int64, _],
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Bool:
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var current: Int64 = -1
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        var header = git_log_header_bounds(ptr, cursor, line[0])
        if header[0] >= 0:
            var start = scratch[].written
            if not context_command_output_put_range(
                scratch, ptr, header[0], header[1]
            ) or record_count[] >= record_capacity:
                return False
            current = record_count[]
            records[unsafe_offset=current] = ProdexContextCommandOutputRecord(
                0, GIT_LOG_COMMIT, start, scratch[].written - start
            )
            record_count[] += 1
            cursor = line[1]
            continue
        if current >= 0:
            var bounds = context_text_trim_bounds(ptr, cursor, line[0])
            if bounds[0] < bounds[1]:
                var category: Int64 = 0
                if context_search_ascii_starts_exact["Author:"](
                    ptr, bounds[0], bounds[1]
                ) or context_search_ascii_starts_exact["Date:"](
                    ptr, bounds[0], bounds[1]
                ):
                    category = GIT_LOG_METADATA
                elif git_log_stat_line(ptr, cursor, line[0]):
                    category = GIT_LOG_STAT_LINE
                elif git_log_stat_summary(ptr, bounds[0], bounds[1]):
                    category = GIT_LOG_STAT_SUMMARY
                elif line[0] - cursor >= 4 and ptr[unsafe_offset=cursor] == 32 and ptr[unsafe_offset=cursor + 1] == 32 and ptr[unsafe_offset=cursor + 2] == 32 and ptr[unsafe_offset=cursor + 3] == 32:
                    var subjects: Int64 = 0
                    for index in range(record_count[]):
                        var record = records[unsafe_offset=index].copy()
                        if record.category == GIT_LOG_SUBJECT and record.occurrences == UInt64(current):
                            subjects += 1
                    if subjects < 2:
                        category = GIT_LOG_SUBJECT
                if category != 0 and not git_log_append_unique(
                    records,
                    record_capacity,
                    record_count,
                    scratch,
                    current,
                    category,
                    ptr,
                    cursor,
                    line[0],
                ):
                    return False
        cursor = line[1]
    return True


def git_log_commit_qualified(
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_count: Int64,
    commit: Int64,
) -> Bool:
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if record.occurrences == UInt64(commit) and (record.category == GIT_LOG_STAT_LINE or record.category == GIT_LOG_STAT_SUMMARY):
            return True
    return False


def context_command_output_git_log(
    input: ProdexContextCommandOutputInput,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Int64:
    var record_count: Int64 = 0
    if not git_log_parse(
        input, records, record_capacity, Pointer(to=record_count), scratch
    ):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var commits: Int64 = 0
    var stat_files: Int64 = 0
    for index in range(record_count):
        if records[unsafe_offset=index].category == GIT_LOG_COMMIT and git_log_commit_qualified(
            records, record_count, index
        ):
            commits += 1
        elif records[unsafe_offset=index].category == GIT_LOG_STAT_LINE:
            stat_files += 1
    if commits == 0:
        return CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH
    if not context_command_output_put_literal(writer, StringSlice("sum: git log --stat commits=")) or not context_command_output_put_i64(
        writer, commits
    ) or not context_command_output_put_literal(writer, StringSlice(", stat_files=")) or not context_command_output_put_i64(
        writer, stat_files
    ) or not context_command_output_put_byte(writer, 10):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var commit_limit = max(input.max_lines, UInt64(24)) / 8
    commit_limit = min(max(commit_limit, UInt64(2)), UInt64(commits))
    var stat_limit = max(input.max_path_entries, UInt64(1)) / max(
        commit_limit, UInt64(1)
    )
    stat_limit = min(max(stat_limit, UInt64(2)), UInt64(8))
    var rendered_commits: UInt64 = 0
    for index in range(record_count):
        var commit = records[unsafe_offset=index].copy()
        if commit.category != GIT_LOG_COMMIT or not git_log_commit_qualified(
            records, record_count, index
        ) or rendered_commits >= commit_limit:
            continue
        if not context_command_output_put_literal(writer, StringSlice("commit: ")) or not context_file_list_write_truncated(
            writer,
            scratch[].output,
            commit.offset,
            commit.offset + commit.len,
            input.max_line_chars,
        ):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
        for category in range(GIT_LOG_METADATA, GIT_LOG_STAT_SUMMARY + 1):
            if category == GIT_LOG_STAT_LINE:
                continue
            var rendered: Int64 = 0
            for record_index in range(record_count):
                var record = records[unsafe_offset=record_index].copy()
                if record.category != category or record.occurrences != UInt64(index) or category == GIT_LOG_METADATA and rendered >= 2 or category == GIT_LOG_SUBJECT and rendered >= 2:
                    continue
                if category == GIT_LOG_SUBJECT and not context_command_output_put_literal(writer, StringSlice("  subject: ")):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                if category == GIT_LOG_METADATA and not context_command_output_put_literal(writer, StringSlice("  ")):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                if category == GIT_LOG_STAT_SUMMARY and not context_command_output_put_literal(writer, StringSlice("  stat totals: ")):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                if not context_file_list_write_truncated(writer, scratch[].output, record.offset, record.offset + record.len, input.max_line_chars):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                rendered += 1
        var file_count: Int64 = 0
        for record_index in range(record_count):
            var record = records[unsafe_offset=record_index].copy()
            if record.category == GIT_LOG_STAT_LINE and record.occurrences == UInt64(index):
                file_count += 1
        if file_count > 0:
            if not context_command_output_put_literal(writer, StringSlice("  stat files (")) or not context_command_output_put_i64(writer, file_count) or not context_command_output_put_literal(writer, StringSlice("):\n")):
                return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
            var position: UInt64 = 0
            var head = stat_limit / 2 + stat_limit % 2
            var tail = stat_limit - head
            var omitted = False
            for record_index in range(record_count):
                var record = records[unsafe_offset=record_index].copy()
                if record.category != GIT_LOG_STAT_LINE or record.occurrences != UInt64(index):
                    continue
                if UInt64(file_count) <= stat_limit or position < head or UInt64(file_count) - position <= tail:
                    if not context_command_output_put_literal(writer, StringSlice("    ")) or not context_file_list_write_truncated(writer, scratch[].output, record.offset, record.offset + record.len, input.max_line_chars):
                        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                elif not omitted:
                    if not context_command_output_put_literal(writer, StringSlice("    [... omitted ")) or not context_command_output_put_i64(writer, Int64(UInt64(file_count) - head - tail)) or not context_command_output_put_literal(writer, StringSlice(" stat entries ...]\n")):
                        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                    omitted = True
                position += 1
        rendered_commits += 1
    if UInt64(commits) > commit_limit:
        if not context_command_output_put_literal(writer, StringSlice("[... omitted ")) or not context_command_output_put_i64(writer, Int64(UInt64(commits) - commit_limit)) or not context_command_output_put_literal(writer, StringSlice(" commits ...]\n")):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK
