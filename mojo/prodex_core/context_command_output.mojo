from std.collections import Array

from std.memory import Pointer

from context_command_output_common import (
    CONTEXT_COMMAND_OUTPUT_FILE_LIST,
    CONTEXT_COMMAND_OUTPUT_GIT_DIFF,
    CONTEXT_COMMAND_OUTPUT_GIT_LOG,
    CONTEXT_COMMAND_OUTPUT_GIT_STATUS,
    CONTEXT_COMMAND_OUTPUT_MAX_BYTES,
    CONTEXT_COMMAND_OUTPUT_SEARCH,
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
    context_search_ascii_contains_exact,
    context_search_ascii_find_exact,
    context_search_ascii_starts_exact,
    context_search_find_byte,
    context_metadata_lower,
    context_text_trim_bounds,
)
from context_command_output_file_list import context_command_output_file_list
from context_command_output_git_log import context_command_output_git_log
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
        and value.operation != CONTEXT_COMMAND_OUTPUT_SEARCH
        and value.operation != CONTEXT_COMMAND_OUTPUT_GIT_LOG
        and value.operation != CONTEXT_COMMAND_OUTPUT_GIT_DIFF
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
    branch: Pointer[mut=True, Array[Int64, 2], _],
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
    branch: Pointer[mut=True, Array[Int64, 2], _],
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


def context_command_output_diff_stat_line(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var separator = context_search_ascii_find_exact[" | "](ptr, start, end)
    if separator < 0 or separator == start or separator + 3 >= end:
        return False
    for index in range(separator + 3, end):
        var value = ptr[unsafe_offset=index]
        if value == 43 or value == 45 or value >= 48 and value <= 57:
            return True
    return False


def context_command_output_diff_stat_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_search_ascii_contains_exact[" file changed"](ptr, start, end)
        or context_search_ascii_contains_exact[" files changed"](ptr, start, end)
        or context_search_ascii_contains_exact[" insertion"](ptr, start, end)
        or context_search_ascii_contains_exact[" deletion"](ptr, start, end)
    )


def context_command_output_diff_header(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_search_ascii_starts_exact["diff --git "](ptr, start, end)


def context_command_output_diff_path(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Bool:
    var marker = context_search_ascii_find_exact[" b/"](ptr, start, end)
    if marker >= 0:
        return context_command_output_put_range(writer, ptr, marker + 3, end)
    marker = context_search_ascii_find_exact["+++ b/"](ptr, start, end)
    if marker >= 0:
        return context_command_output_put_range(writer, ptr, marker + 6, end)
    return context_command_output_put_literal(writer, StringSlice("unknown"))


def context_command_output_diff_write_section_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Bool:
    var added: Int64 = 0
    var removed: Int64 = 0
    var hunks: Int64 = 0
    var binary = False
    var context_start: Int64 = -1
    var context_end: Int64 = -1
    var cursor = start
    while cursor < end:
        var line = context_command_output_next_line(ptr, cursor, end)
        if context_search_ascii_starts_exact["@@ "](ptr, cursor, line[0]):
            hunks += 1
            var first_marker = context_search_ascii_find_exact["@@"](ptr, cursor, line[0])
            var second_marker = context_search_ascii_find_exact["@@"](ptr, first_marker + 2, line[0]) if first_marker >= 0 else -1
            if second_marker >= 0:
                var context = context_text_trim_bounds(ptr, second_marker + 2, line[0])
                if context[0] < context[1] and context_start < 0:
                    context_start = context[0]
                    context_end = context[1]
        elif context_search_ascii_starts_exact["+"](ptr, cursor, line[0]) and not context_search_ascii_starts_exact["+++ "](ptr, cursor, line[0]):
            added += 1
        elif context_search_ascii_starts_exact["-"](ptr, cursor, line[0]) and not context_search_ascii_starts_exact["--- "](ptr, cursor, line[0]):
            removed += 1
        elif context_search_ascii_starts_exact["Binary files "](ptr, cursor, line[0]) or context_search_ascii_starts_exact["GIT binary patch"](ptr, cursor, line[0]):
            binary = True
        cursor = line[1]
    var header = context_command_output_next_line(ptr, start, end)
    if not context_command_output_diff_path(ptr, start, header[0], writer):
        return False
    if not context_command_output_put_literal(writer, StringSlice(": +")) or not context_command_output_put_i64(writer, added) or not context_command_output_put_literal(writer, StringSlice(", -")) or not context_command_output_put_i64(writer, removed) or not context_command_output_put_literal(writer, StringSlice(", ")) or not context_command_output_put_i64(writer, hunks) or not context_command_output_put_literal(writer, StringSlice(" hunks")):
        return False
    if binary and not context_command_output_put_literal(writer, StringSlice(", binary")):
        return False
    if context_start >= 0 and (
        not context_command_output_put_literal(writer, StringSlice(", ctx="))
        or not context_command_output_put_range(writer, ptr, context_start, context_end)
    ):
        return False
    return context_command_output_put_byte(writer, 10)


def context_command_output_diff_excerpt_structural(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_search_ascii_starts_exact["diff --git "](ptr, start, end)
        or context_search_ascii_starts_exact["@@ "](ptr, start, end)
        or context_search_ascii_starts_exact["--- "](ptr, start, end)
        or context_search_ascii_starts_exact["+++ "](ptr, start, end)
        or context_search_ascii_starts_exact["new file mode "](ptr, start, end)
        or context_search_ascii_starts_exact["deleted file mode "](ptr, start, end)
        or context_search_ascii_starts_exact["old mode "](ptr, start, end)
        or context_search_ascii_starts_exact["new mode "](ptr, start, end)
        or context_search_ascii_starts_exact["rename from "](ptr, start, end)
        or context_search_ascii_starts_exact["rename to "](ptr, start, end)
        or context_search_ascii_starts_exact["similarity index "](ptr, start, end)
        or context_search_ascii_starts_exact["dissimilarity index "](ptr, start, end)
        or context_search_ascii_starts_exact["Binary files "](ptr, start, end)
        or context_search_ascii_starts_exact["GIT binary patch"](ptr, start, end)
    )


def context_command_output_diff_line_matches_term(
    line: Pointer[mut=False, UInt8, _], start: Int64, end: Int64,
    term: Pointer[mut=False, UInt8, _], term_start: Int64, term_end: Int64,
) -> Bool:
    var term_length = term_end - term_start
    if term_length == 0 or term_length > end - start:
        return term_length == 0
    for offset in range(start, end - term_length + 1):
        var matched = True
        for index in range(term_length):
            if context_metadata_lower(line[unsafe_offset=offset + index]) != context_metadata_lower(term[unsafe_offset=term_start + index]):
                matched = False
                break
        if matched:
            return True
    return False


def context_command_output_diff_line_matches_intent(
    input: ProdexContextCommandOutputInput,
    start: Int64,
    end: Int64,
) -> Bool:
    if input.intent.len == 0:
        return True
    var line = rich_view_ptr(input.input)
    var terms = rich_view_ptr(input.intent)
    var cursor: Int64 = 0
    var term_start: Int64 = 0
    while cursor <= Int64(input.intent.len):
        if cursor == Int64(input.intent.len) or terms[unsafe_offset=cursor] == 44:
            var term_end = cursor
            while term_start < term_end and terms[unsafe_offset=term_start] == 32:
                term_start += 1
            while term_end > term_start and terms[unsafe_offset=term_end - 1] == 32:
                term_end -= 1
            if context_command_output_diff_line_matches_term(line, start, end, terms, term_start, term_end):
                return True
            term_start = cursor + 1
        cursor += 1
    return False


def context_command_output_git_diff(
    input: ProdexContextCommandOutputInput,
    writer: Pointer[mut=True, ContextCommandOutputWriter, _],
    records: Pointer[mut=True, ProdexContextCommandOutputRecord, _],
    record_capacity: Int64,
    scratch: Pointer[mut=True, ContextCommandOutputWriter, _],
) -> Int64:
    _ = records
    _ = record_capacity
    _ = scratch
    var ptr = rich_view_ptr(input.input)
    var length = Int64(input.input.len)
    var sections: Int64 = 0
    var total_added: Int64 = 0
    var total_removed: Int64 = 0
    var total_hunks: Int64 = 0
    var stat_lines: Int64 = 0
    var stat_summary = False
    var cursor: Int64 = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_command_output_diff_header(ptr, cursor, line[0]):
            sections += 1
        if context_command_output_diff_stat_line(ptr, cursor, line[0]):
            stat_lines += 1
        if context_command_output_diff_stat_summary(ptr, cursor, line[0]):
            stat_summary = True
        cursor = line[1]
    if sections == 0:
        if stat_lines == 0 or not stat_summary:
            return CONTEXT_COMMAND_OUTPUT_STATUS_NO_MATCH
        if not context_command_output_put_literal(writer, StringSlice("sum: git diff stat_only entries=")) or not context_command_output_put_i64(writer, stat_lines) or not context_command_output_put_byte(writer, 10):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
        cursor = 0
        while cursor < length:
            var line = context_command_output_next_line(ptr, cursor, length)
            if context_command_output_diff_stat_summary(ptr, cursor, line[0]):
                var bounds = context_text_trim_bounds(ptr, cursor, line[0])
                if not context_command_output_put_literal(writer, StringSlice("stat totals: ")) or not context_command_output_put_range(writer, ptr, bounds[0], bounds[1]) or not context_command_output_put_byte(writer, 10):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
            cursor = line[1]
        if not context_command_output_put_literal(writer, StringSlice("stat files:\n")):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
        cursor = 0
        while cursor < length:
            var line = context_command_output_next_line(ptr, cursor, length)
            if context_command_output_diff_stat_line(ptr, cursor, line[0]) and not context_command_output_diff_stat_summary(ptr, cursor, line[0]):
                if not context_command_output_put_literal(writer, StringSlice("  ")) or not context_command_output_put_range(writer, ptr, cursor, line[0]) or not context_command_output_put_byte(writer, 10):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
            cursor = line[1]
        return CONTEXT_COMMAND_OUTPUT_STATUS_OK

    # ponytail: repeated section scans keep the bounded ABI record-free; index sections if diffs dominate profiles.
    cursor = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_command_output_diff_header(ptr, cursor, line[0]):
            var section_end = length
            var probe = line[1]
            while probe < length:
                var next = context_command_output_next_line(ptr, probe, length)
                if context_command_output_diff_header(ptr, probe, next[0]):
                    section_end = probe
                    break
                probe = next[1]
            var section_added: Int64 = 0
            var section_removed: Int64 = 0
            var section_hunks: Int64 = 0
            var section_cursor = cursor
            while section_cursor < section_end:
                var section_line = context_command_output_next_line(ptr, section_cursor, section_end)
                if context_search_ascii_starts_exact["@@ "](ptr, section_cursor, section_line[0]):
                    section_hunks += 1
                elif context_search_ascii_starts_exact["+"](ptr, section_cursor, section_line[0]) and not context_search_ascii_starts_exact["+++ "](ptr, section_cursor, section_line[0]):
                    section_added += 1
                elif context_search_ascii_starts_exact["-"](ptr, section_cursor, section_line[0]) and not context_search_ascii_starts_exact["--- "](ptr, section_cursor, section_line[0]):
                    section_removed += 1
                section_cursor = section_line[1]
            total_added += section_added
            total_removed += section_removed
            total_hunks += section_hunks
        cursor = line[1]
    if not context_command_output_put_literal(writer, StringSlice("sum: git diff files=")) or not context_command_output_put_i64(writer, sections) or not context_command_output_put_literal(writer, StringSlice(", +")) or not context_command_output_put_i64(writer, total_added) or not context_command_output_put_literal(writer, StringSlice(", -")) or not context_command_output_put_i64(writer, total_removed) or not context_command_output_put_literal(writer, StringSlice(", hunks=")) or not context_command_output_put_i64(writer, total_hunks) or not context_command_output_put_byte(writer, 10):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    if input.intent.len > 0:
        if not context_command_output_put_literal(writer, StringSlice("int: git diff focus for ")) or not context_command_output_put_range(writer, rich_view_ptr(input.intent), 0, Int64(input.intent.len)) or not context_command_output_put_byte(writer, 10):
            return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    cursor = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_command_output_diff_header(ptr, cursor, line[0]):
            var section_end = length
            var probe = line[1]
            while probe < length:
                var next = context_command_output_next_line(ptr, probe, length)
                if context_command_output_diff_header(ptr, probe, next[0]):
                    section_end = probe
                    break
                probe = next[1]
            if not context_command_output_diff_write_section_summary(ptr, cursor, section_end, writer):
                return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
        cursor = line[1]
    if not context_command_output_put_literal(writer, StringSlice("\ndiff excerpts:\n")):
        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
    var structural_lines: Int64 = 0
    cursor = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_command_output_diff_excerpt_structural(ptr, cursor, line[0]):
            structural_lines += 1
        cursor = line[1]
    var max_lines = Int64(input.max_lines)
    var fixed_lines = 1 + sections + 2 + structural_lines + sections
    var detail_budget = max_lines - 1 - fixed_lines if max_lines > fixed_lines + 1 else 0
    var per_section_budget = detail_budget / sections if sections > 0 else 0
    cursor = 0
    while cursor < length:
        var line = context_command_output_next_line(ptr, cursor, length)
        if context_command_output_diff_header(ptr, cursor, line[0]):
            var section_end = length
            var probe = line[1]
            while probe < length:
                var next = context_command_output_next_line(ptr, probe, length)
                if context_command_output_diff_header(ptr, probe, next[0]):
                    section_end = probe
                    break
                probe = next[1]
            var detail_written: Int64 = 0
            var omitted: Int64 = 0
            var section_cursor = cursor
            while section_cursor < section_end:
                var section_line = context_command_output_next_line(ptr, section_cursor, section_end)
                var structural = context_command_output_diff_excerpt_structural(ptr, section_cursor, section_line[0])
                var matches_intent = context_command_output_diff_line_matches_intent(input, section_cursor, section_line[0])
                if structural or matches_intent and detail_written < per_section_budget or input.intent.len == 0 and detail_written < per_section_budget:
                    if not context_command_output_put_range(writer, ptr, section_cursor, section_line[0]) or not context_command_output_put_byte(writer, 10):
                        return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
                    if not structural:
                        detail_written += 1
                else:
                    omitted += 1
                section_cursor = section_line[1]
            if omitted > 0:
                if not context_command_output_put_literal(writer, StringSlice("[... omitted ")) or not context_command_output_put_i64(writer, omitted) or not context_command_output_put_literal(writer, StringSlice(" diff lines ...]\n")):
                    return CONTEXT_COMMAND_OUTPUT_STATUS_CAPACITY
        cursor = line[1]
    return CONTEXT_COMMAND_OUTPUT_STATUS_OK


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
    var branch = Array[Int64, 2](fill=-1)
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
        and value.operation != CONTEXT_COMMAND_OUTPUT_GIT_LOG
        and value.operation != CONTEXT_COMMAND_OUTPUT_GIT_DIFF
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
        if value.operation == CONTEXT_COMMAND_OUTPUT_FILE_LIST
        else context_command_output_git_log(
            value,
            Pointer(to=writer),
            records,
            record_capacity,
            Pointer(to=scratch_writer),
        )
        if value.operation == CONTEXT_COMMAND_OUTPUT_GIT_LOG
        else context_command_output_git_diff(
            value,
            Pointer(to=writer),
            records,
            record_capacity,
            Pointer(to=scratch_writer),
        )
    )
    written[] = writer.written
    return status
