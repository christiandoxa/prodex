from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_slice_contains,
    rich_slice_prefix,
    rich_unicode_space,
    rich_utf8_continuation,
)
from rich_types import (
    ProdexRichContextRecord,
    ProdexRichContextResult,
    ProdexRichSlice,
    ProdexRichStringView,
    rich_view_ptr,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 4
comptime RICH_CONTEXT_MAX_LINES: Int64 = 65_536
comptime RICH_CONTEXT_MAX_TEXT_BYTES: Int64 = 1_048_576
comptime RICH_CONTEXT_STATUS_OK: Int64 = 0
comptime RICH_CONTEXT_STATUS_INVALID: Int64 = 1
comptime RICH_CONTEXT_STATUS_UTF8: Int64 = 2
comptime RICH_CONTEXT_STATUS_CAPACITY: Int64 = 3
comptime RICH_CONTEXT_STATUS_ABI: Int64 = 4
comptime RICH_CONTEXT_ISSUE_INVALID_UTF8: Int64 = 6
comptime RICH_CONTEXT_KIND_ERROR: Int64 = 1
comptime RICH_CONTEXT_KIND_FILE: Int64 = 2
comptime RICH_CONTEXT_KIND_DIFF: Int64 = 3
comptime RICH_CONTEXT_KIND_TEST: Int64 = 4
comptime RICH_CONTEXT_KIND_EXIT: Int64 = 5
comptime RICH_CONTEXT_KIND_STACK: Int64 = 6
comptime RICH_CONTEXT_KIND_DIAGNOSTIC: Int64 = 7


def context_utf8_error(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    var index: Int64 = 0
    while index < length:
        var lead = ptr[unsafe_offset=index]
        var width: Int64
        if lead <= 0x7F:
            width = 1
        elif lead >= 0xC2 and lead <= 0xDF:
            width = 2
        elif lead >= 0xE0 and lead <= 0xEF:
            width = 3
        elif lead >= 0xF0 and lead <= 0xF4:
            width = 4
        else:
            return index
        if index + width > length:
            return index
        if width >= 2 and not rich_utf8_continuation(ptr[unsafe_offset=index + 1]):
            return index + 1
        if width >= 3:
            var second = ptr[unsafe_offset=index + 1]
            if lead == 0xE0 and second < 0xA0 or lead == 0xED and second > 0x9F:
                return index
            if not rich_utf8_continuation(ptr[unsafe_offset=index + 2]):
                return index + 2
        if width == 4:
            var second = ptr[unsafe_offset=index + 1]
            if lead == 0xF0 and second < 0x90 or lead == 0xF4 and second > 0x8F:
                return index
            if not rich_utf8_continuation(ptr[unsafe_offset=index + 3]):
                return index + 3
        index += width
    return -1


def context_skip_ansi(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    if index + 1 >= end:
        return end
    var kind = ptr[unsafe_offset=index + 1]
    if kind == 91:
        var cursor = index + 2
        while cursor < end:
            var value = ptr[unsafe_offset=cursor]
            cursor += 1
            if value >= 0x40 and value <= 0x7E:
                return cursor
        return end
    if kind == 93:
        var cursor = index + 2
        var previous: UInt8 = 0
        while cursor < end:
            var value = ptr[unsafe_offset=cursor]
            cursor += 1
            if value == 7 or previous == 27 and value == 92:
                return cursor
            previous = value
        return end
    return index + 2


def context_has_content(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var index = start
    while index < end:
        if ptr[unsafe_offset=index] == 27:
            index = context_skip_ansi(ptr, index, end)
            continue
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if not rich_unicode_space(rich_codepoint(ptr, index, width)):
            return True
        index += width
    return False


def context_last_line(view: ProdexRichStringView) -> Int64:
    if view.len == 0:
        return 0
    var ptr = rich_view_ptr(view)
    var length = Int64(view.len)
    var cursor: Int64 = 0
    var line_number: Int64 = 0
    var last_content: Int64 = 0
    while cursor < length:
        var end = cursor
        while end < length and ptr[unsafe_offset=end] != 10 and ptr[unsafe_offset=end] != 13:
            end += 1
        line_number += 1
        if context_has_content(ptr, cursor, end):
            last_content = line_number
        if end == length:
            break
        cursor = end + 1
        if ptr[unsafe_offset=end] == 13 and cursor < length and ptr[unsafe_offset=cursor] == 10:
            cursor += 1
    return last_content


def context_copy_line(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    var output_start = written[]
    var offset = output_start
    var last_nonspace = output_start
    var started = False
    var cursor = start
    while cursor < end:
        if ptr[unsafe_offset=cursor] == 27:
            cursor = context_skip_ansi(ptr, cursor, end)
            continue
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var space = rich_unicode_space(rich_codepoint(ptr, cursor, width))
        if not started and space:
            cursor += width
            continue
        if offset > capacity - width:
            return ProdexRichSlice(-1, -1)
        for index in range(width):
            output[unsafe_offset=offset + index] = ptr[unsafe_offset=cursor + index]
        offset += width
        if not space:
            started = True
            last_nonspace = offset
        cursor += width
    written[] = last_nonspace
    return ProdexRichSlice(output_start, last_nonspace - output_start)


def context_hash(
    output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice
) -> UInt64:
    var hash: UInt64 = 1469598103934665603 ^ UInt64(slice.len)
    for index in range(slice.len):
        hash = (hash ^ UInt64(output[unsafe_offset=slice.offset + index])) * 1099511628211
    return hash


def context_slice_equal(
    output: Pointer[mut=True, UInt8, _], left: ProdexRichSlice, right: ProdexRichSlice
) -> Bool:
    if left.len != right.len:
        return False
    for index in range(left.len):
        if output[unsafe_offset=left.offset + index] != output[unsafe_offset=right.offset + index]:
            return False
    return True


def context_token_count(
    output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice
) -> Int64:
    var count: Int64 = 0
    var index: Int64 = 0
    while index < slice.len:
        while index < slice.len and output[unsafe_offset=slice.offset + index] <= 32:
            index += 1
        if index == slice.len:
            break
        count += 1
        while index < slice.len and output[unsafe_offset=slice.offset + index] > 32:
            index += 1
    return count


def context_range_contains(
    output: Pointer[mut=True, UInt8, _], start: Int64, end: Int64, needle: UInt8
) -> Bool:
    for index in range(start, end):
        if output[unsafe_offset=index] == needle:
            return True
    return False


def context_range_extension(
    output: Pointer[mut=True, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var dot = end - 1
    while dot >= start and output[unsafe_offset=dot] != 46:
        dot -= 1
    return dot > start and end - dot - 1 <= 12


def context_token_location(
    output: Pointer[mut=True, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var token_end = end
    while token_end > start and (output[unsafe_offset=token_end - 1] == 58 or output[unsafe_offset=token_end - 1] == 46):
        token_end -= 1
    var column = token_end - 1
    while column >= start and output[unsafe_offset=column] != 58:
        column -= 1
    if column <= start or column + 1 >= token_end:
        return False
    var index = column + 1
    while index < token_end and output[unsafe_offset=index] >= 48 and output[unsafe_offset=index] <= 57:
        index += 1
    if index != token_end:
        return False
    var line = column - 1
    while line >= start and output[unsafe_offset=line] != 58:
        line -= 1
    if line <= start or line + 1 >= column:
        return False
    index = line + 1
    while index < column and output[unsafe_offset=index] >= 48 and output[unsafe_offset=index] <= 57:
        index += 1
    return index == column and (context_range_contains(output, start, line, 47) or context_range_contains(output, start, line, 92) or context_range_extension(output, start, line))


def context_file_locations(
    output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice
) -> Int64:
    var count: Int64 = 0
    var index: Int64 = 0
    while index < slice.len:
        while index < slice.len and output[unsafe_offset=slice.offset + index] <= 32:
            index += 1
        var start = index
        while index < slice.len and output[unsafe_offset=slice.offset + index] > 32:
            index += 1
        if index > start and context_token_location(output, slice.offset + start, slice.offset + index):
            count += 1
    if rich_slice_contains(output, slice, StringSlice("line_number"), False) and (rich_slice_contains(output, slice, StringSlice(".rs"), False) or rich_slice_contains(output, slice, StringSlice(".md"), False)):
        count += 1
    if rich_slice_contains(output, slice, StringSlice("File \""), False) and rich_slice_contains(output, slice, StringSlice(", line "), False):
        count += 1
    return count


def context_zero_summary(
    output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice, prefix: StringSlice
) -> Bool:
    if not rich_slice_prefix(output, slice, prefix, True):
        return False
    var index = Int64(prefix.byte_length())
    while index < slice.len and (output[unsafe_offset=slice.offset + index] == 32 or output[unsafe_offset=slice.offset + index] == 9 or output[unsafe_offset=slice.offset + index] == 58 or output[unsafe_offset=slice.offset + index] == 61 or output[unsafe_offset=slice.offset + index] == 40):
        index += 1
    if index == slice.len or output[unsafe_offset=slice.offset + index] != 48:
        return False
    index += 1
    while index < slice.len and output[unsafe_offset=slice.offset + index] == 48:
        index += 1
    while index < slice.len and (output[unsafe_offset=slice.offset + index] == 32 or output[unsafe_offset=slice.offset + index] == 9 or output[unsafe_offset=slice.offset + index] == 41):
        index += 1
    return index == slice.len


def context_is_error(output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice) -> Bool:
    if context_zero_summary(output, slice, StringSlice("error:")) or context_zero_summary(output, slice, StringSlice("errors:")):
        return False
    return rich_slice_prefix(output, slice, StringSlice("error:"), True) or rich_slice_prefix(output, slice, StringSlice("error["), True) or rich_slice_prefix(output, slice, StringSlice("fatal:"), True) or rich_slice_prefix(output, slice, StringSlice("panic:"), True) or rich_slice_prefix(output, slice, StringSlice("npm err!"), True) or rich_slice_prefix(output, slice, StringSlice("npm error"), True) or rich_slice_prefix(output, slice, StringSlice("pnpm error"), True) or rich_slice_prefix(output, slice, StringSlice("yarn error"), True) or rich_slice_prefix(output, slice, StringSlice("bun error"), True) or rich_slice_prefix(output, slice, StringSlice("failed "), True) or rich_slice_prefix(output, slice, StringSlice("fail "), True) or rich_slice_prefix(output, slice, StringSlice("e   "), False) or rich_slice_contains(output, slice, StringSlice("panicked at"), True) or rich_slice_contains(output, slice, StringSlice("\"error\""), False) or rich_slice_contains(output, slice, StringSlice("status=error"), True) or rich_slice_contains(output, slice, StringSlice("level=error"), True) or rich_slice_contains(output, slice, StringSlice("exception"), True)


def context_is_test(output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice) -> Bool:
    return rich_slice_prefix(output, slice, StringSlice("test "), False) and rich_slice_contains(output, slice, StringSlice(" ... FAILED"), False) or rich_slice_prefix(output, slice, StringSlice("---- "), False) and rich_slice_contains(output, slice, StringSlice(" ----"), False) or rich_slice_prefix(output, slice, StringSlice("FAIL "), False) or rich_slice_prefix(output, slice, StringSlice("FAILED "), False) or rich_slice_prefix(output, slice, StringSlice("test result: FAILED"), False) or rich_slice_prefix(output, slice, StringSlice("failures:"), False) or rich_slice_contains(output, slice, StringSlice(" ... FAILED"), False)


def context_is_stack(output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice) -> Bool:
    return rich_slice_prefix(output, slice, StringSlice("stack backtrace:"), False) or rich_slice_prefix(output, slice, StringSlice("Traceback (most recent call last):"), False) or rich_slice_prefix(output, slice, StringSlice("Stack trace:"), False) or rich_slice_prefix(output, slice, StringSlice("stack trace:"), False) or rich_slice_prefix(output, slice, StringSlice("Backtrace:"), False) or rich_slice_prefix(output, slice, StringSlice("Caused by:"), False)


def context_is_diagnostic(output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice) -> Bool:
    return rich_slice_prefix(output, slice, StringSlice("error:"), False) or rich_slice_prefix(output, slice, StringSlice("error["), False) or rich_slice_prefix(output, slice, StringSlice("warning:"), False) or rich_slice_prefix(output, slice, StringSlice("warning["), False) or rich_slice_prefix(output, slice, StringSlice("--> "), False) or rich_slice_prefix(output, slice, StringSlice("::: "), False) or rich_slice_prefix(output, slice, StringSlice("= note:"), False) or rich_slice_prefix(output, slice, StringSlice("= help:"), False) or rich_slice_prefix(output, slice, StringSlice("help:"), False) or rich_slice_prefix(output, slice, StringSlice("note:"), False) or rich_slice_contains(output, slice, StringSlice("clippy::"), False)


def context_kind(output: Pointer[mut=True, UInt8, _], slice: ProdexRichSlice) -> Int64:
    if context_is_error(output, slice):
        return RICH_CONTEXT_KIND_ERROR
    if context_file_locations(output, slice) > 0:
        return RICH_CONTEXT_KIND_FILE
    if rich_slice_prefix(output, slice, StringSlice("@@ "), False) and rich_slice_contains(output, slice, StringSlice("@@"), False):
        return RICH_CONTEXT_KIND_DIFF
    if context_is_test(output, slice):
        return RICH_CONTEXT_KIND_TEST
    if rich_slice_contains(output, slice, StringSlice("exit status"), True) or rich_slice_contains(output, slice, StringSlice("exit code"), True) or rich_slice_contains(output, slice, StringSlice("exit_status"), True):
        return RICH_CONTEXT_KIND_EXIT
    if context_is_stack(output, slice):
        return RICH_CONTEXT_KIND_STACK
    if context_is_diagnostic(output, slice):
        return RICH_CONTEXT_KIND_DIAGNOSTIC
    return 0


def context_hash_capacity(count: Int64) -> Int64:
    var capacity: Int64 = 1
    var target = count * 2
    while capacity < target:
        capacity *= 2
    return capacity


@export("prodex_mojo_rich_context_analyze_v2")
def prodex_mojo_rich_context_analyze_v2(
    abi_version: Int64,
    input: ProdexRichStringView,
    output_records_address: UInt,
    record_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    hash_slots_address: UInt,
    hash_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_CONTEXT_STATUS_INVALID
    var result = Pointer[mut=True, ProdexRichContextResult, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[].abi_version = PRODEX_RICH_ABI_VERSION
    result[].line_count = 0
    result[].records_written = 0
    result[].required_records = 0
    result[].output_written = 0
    result[].required_output = 0
    result[].required_scratch = 0
    result[].issue_kind = 0
    result[].issue_offset = -1
    result[].issue_length = 0
    result[].counts = InlineArray[Int64, 7](fill=0)
    result[].noise_lines = 0
    result[].signal_lines = 0
    result[].token_count = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result[].issue_kind = RICH_CONTEXT_STATUS_ABI
        return RICH_CONTEXT_STATUS_ABI
    if input.len > UInt(RICH_CONTEXT_MAX_TEXT_BYTES) or input.len > 0 and input.ptr == 0:
        result[].issue_kind = RICH_CONTEXT_ISSUE_INVALID_UTF8
        result[].issue_offset = 0
        result[].issue_length = 1
        return RICH_CONTEXT_STATUS_UTF8
    if input.len > 0 and context_utf8_error(rich_view_ptr(input), Int64(input.len)) >= 0:
        result[].issue_kind = RICH_CONTEXT_ISSUE_INVALID_UTF8
        result[].issue_offset = context_utf8_error(rich_view_ptr(input), Int64(input.len))
        result[].issue_length = 1
        return RICH_CONTEXT_STATUS_UTF8
    var line_count = context_last_line(input)
    var required_hash = context_hash_capacity(line_count)
    result[].line_count = line_count
    result[].required_records = line_count
    result[].required_output = Int64(input.len)
    result[].required_scratch = required_hash
    if record_capacity < line_count or output_capacity < Int64(input.len) or hash_capacity < required_hash:
        return RICH_CONTEXT_STATUS_CAPACITY
    if line_count == 0:
        return RICH_CONTEXT_STATUS_OK
    if output_records_address == 0 or output_address == 0 or hash_slots_address == 0:
        return RICH_CONTEXT_STATUS_INVALID
    var records = Pointer[mut=True, ProdexRichContextRecord, MutUntrackedOrigin](
        unsafe_from_address=Int(output_records_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hash_slots_address)
    )
    for index in range(hash_capacity):
        slots[unsafe_offset=index] = -1
    var input_ptr = rich_view_ptr(input)
    var length = Int64(input.len)
    var cursor: Int64 = 0
    var line_number: Int64 = 0
    var record_count: Int64 = 0
    var written: Int64 = 0
    while cursor < length and line_number < line_count:
        var end = cursor
        while end < length and input_ptr[unsafe_offset=end] != 10 and input_ptr[unsafe_offset=end] != 13:
            end += 1
        line_number += 1
        var before = written
        var key = context_copy_line(input_ptr, cursor, end, output, output_capacity, Pointer(to=written))
        if key.len == 0:
            result[].noise_lines += 1
        else:
            var kind = context_kind(output, key)
            var file_count = context_file_locations(output, key)
            var is_error = context_is_error(output, key)
            var is_diff = rich_slice_prefix(output, key, StringSlice("@@ "), False) and rich_slice_contains(output, key, StringSlice("@@"), False)
            var is_test = context_is_test(output, key)
            var is_exit = rich_slice_contains(output, key, StringSlice("exit status"), True) or rich_slice_contains(output, key, StringSlice("exit code"), True) or rich_slice_contains(output, key, StringSlice("exit_status"), True)
            var is_stack = context_is_stack(output, key)
            var is_diag = context_is_diagnostic(output, key)
            if is_error:
                result[].counts[0] += 1
            result[].counts[1] += file_count
            if is_diff:
                result[].counts[2] += 1
            if is_test:
                result[].counts[3] += 1
            if is_exit:
                result[].counts[4] += 1
            if is_stack:
                result[].counts[5] += 1
            if is_diag:
                result[].counts[6] += 1
            var tokens = context_token_count(output, key)
            result[].token_count += tokens
            if kind == 0:
                written = before
                result[].noise_lines += 1
            else:
                var hash = context_hash(output, key)
                var slot = Int64(hash % UInt64(hash_capacity))
                var existing: Int64 = -1
                for _ in range(hash_capacity):
                    var candidate = slots[unsafe_offset=slot]
                    if candidate < 0:
                        slots[unsafe_offset=slot] = record_count
                        break
                    if context_slice_equal(output, key, records[unsafe_offset=candidate].key):
                        existing = candidate
                        break
                    slot += 1
                    if slot == hash_capacity:
                        slot = 0
                if existing >= 0:
                    records[unsafe_offset=existing].occurrences += 1
                    records[unsafe_offset=existing].duplicate_count += 1
                    written = before
                else:
                    records[unsafe_offset=record_count].key = key.copy()
                    records[unsafe_offset=record_count].kind = kind
                    if is_error:
                        records[unsafe_offset=record_count].severity = 2
                    elif is_diag:
                        records[unsafe_offset=record_count].severity = 1
                    else:
                        records[unsafe_offset=record_count].severity = 0
                    records[unsafe_offset=record_count].first_line = line_number
                    records[unsafe_offset=record_count].occurrences = 1
                    records[unsafe_offset=record_count].token_count = tokens
                    records[unsafe_offset=record_count].duplicate_count = 0
                    record_count += 1
                result[].signal_lines += 1
        if end == length:
            break
        cursor = end + 1
        if input_ptr[unsafe_offset=end] == 13 and cursor < length and input_ptr[unsafe_offset=cursor] == 10:
            cursor += 1
    result[].records_written = record_count
    result[].output_written = written
    return RICH_CONTEXT_STATUS_OK
