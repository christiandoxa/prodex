from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
)
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime STRUCTURED_JSON_MAX_BYTES: Int64 = 4_194_304
comptime STRUCTURED_JSON_MAX_VISITED: Int64 = 2048
comptime STRUCTURED_JSON_MAX_DEPTH: Int64 = 8
comptime STRUCTURED_JSON_MAX_KEYS: Int64 = 16
comptime STRUCTURED_JSON_MAX_LEVELS: Int64 = 10
comptime STRUCTURED_JSON_MAX_SAMPLES: Int64 = 8
comptime STRUCTURED_JSON_NDJSON_MIN_RECORDS: Int64 = 2

@fieldwise_init
struct StructuredJsonWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64

@fieldwise_init
struct StructuredJsonSummary(Copyable):
    var visited: Int64
    var objects: Int64
    var arrays: Int64
    var scalars: Int64
    var key_count: Int64
    var key_starts: InlineArray[Int64, 16]
    var key_ends: InlineArray[Int64, 16]
    var key_counts: InlineArray[Int64, 16]
    var level_count: Int64
    var level_starts: InlineArray[Int64, 10]
    var level_ends: InlineArray[Int64, 10]
    var level_counts: InlineArray[Int64, 10]
    var error_count: Int64
    var error_key_starts: InlineArray[Int64, 8]
    var error_key_ends: InlineArray[Int64, 8]
    var error_value_starts: InlineArray[Int64, 8]
    var error_value_ends: InlineArray[Int64, 8]
    var path_count: Int64
    var path_key_starts: InlineArray[Int64, 8]
    var path_key_ends: InlineArray[Int64, 8]
    var path_value_starts: InlineArray[Int64, 8]
    var path_value_ends: InlineArray[Int64, 8]
    var id_count: Int64
    var id_key_starts: InlineArray[Int64, 8]
    var id_key_ends: InlineArray[Int64, 8]
    var id_value_starts: InlineArray[Int64, 8]
    var id_value_ends: InlineArray[Int64, 8]

def structured_json_summary() -> StructuredJsonSummary:
    return StructuredJsonSummary(
        0, 0, 0, 0, 0,
        InlineArray[Int64, 16](fill=-1), InlineArray[Int64, 16](fill=-1), InlineArray[Int64, 16](fill=0),
        0,
        InlineArray[Int64, 10](fill=-1), InlineArray[Int64, 10](fill=-1), InlineArray[Int64, 10](fill=0),
        0,
        InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1),
        0,
        InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1),
        0,
        InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1), InlineArray[Int64, 8](fill=-1),
    )

def structured_json_put_byte(writer: Pointer[mut=True, StructuredJsonWriter, _], value: UInt8) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True

def structured_json_put_literal(writer: Pointer[mut=True, StructuredJsonWriter, _], value: StringSlice) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not structured_json_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True

def structured_json_put_range(writer: Pointer[mut=True, StructuredJsonWriter, _], view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not structured_json_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True

def structured_json_put_u64(writer: Pointer[mut=True, StructuredJsonWriter, _], value: UInt64) -> Bool:
    if value == 0:
        return structured_json_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not structured_json_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True

def structured_json_put_i64(writer: Pointer[mut=True, StructuredJsonWriter, _], value: Int64) -> Bool:
    if value < 0:
        return structured_json_put_byte(writer, 45) and structured_json_put_u64(writer, UInt64(-value))
    return structured_json_put_u64(writer, UInt64(value))

def structured_json_range_equals(view: ProdexRichStringView, ls: Int64, le: Int64, rs: Int64, re: Int64) -> Bool:
    if ls < 0 or rs < 0 or le - ls != re - rs:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(le - ls):
        if ptr[unsafe_offset=ls + index] != ptr[unsafe_offset=rs + index]:
            return False
    return True

def structured_json_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value

def structured_json_quoted_key_matches(view: ProdexRichStringView, start: Int64, end: Int64, literal: StringSlice) -> Bool:
    if start < 0 or end - start != Int64(literal.byte_length()) + 2:
        return False
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var expected = literal.unsafe_ptr()
    var ptr = rich_view_ptr(view)
    for index in range(Int64(literal.byte_length())):
        if structured_json_ascii_lower(ptr[unsafe_offset=start + 1 + index]) != expected[unsafe_offset=index]:
            return False
    return True

def structured_json_key_is_level(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    return (
        structured_json_quoted_key_matches(view, start, end, StringSlice("level"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("severity"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("status"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("state"))
    )

def structured_json_key_is_error(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    return (
        structured_json_quoted_key_matches(view, start, end, StringSlice("error"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("errors"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("err"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("exception"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("failure"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("failures"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("fatal"))
    )

def structured_json_key_is_path(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    return (
        structured_json_quoted_key_matches(view, start, end, StringSlice("path"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("file"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("filename"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("filepath"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("source"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("target"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("uri"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("url"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("cwd"))
    )

def structured_json_key_is_id(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    if start < 0 or end - start < 4:
        return False
    if structured_json_quoted_key_matches(view, start, end, StringSlice("id")):
        return True
    var ptr = rich_view_ptr(view)
    var last = end - 2
    return (
        last - 1 >= start + 1
        and structured_json_ascii_lower(ptr[unsafe_offset=last - 1]) == 105
        and structured_json_ascii_lower(ptr[unsafe_offset=last]) == 100
    )

def structured_json_value_is_scalar(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    if start < 0 or end <= start:
        return False
    var first = deepseek_json_byte(view, start)
    return first != 123 and first != 91

def structured_json_level_is_failure(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    return (
        structured_json_quoted_key_matches(view, start, end, StringSlice("error"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("fatal"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("failed"))
        or structured_json_quoted_key_matches(view, start, end, StringSlice("failure"))
    )

def structured_json_record_sample(
    count: Pointer[mut=True, Int64, _],
    key_starts: Pointer[mut=True, Int64, _],
    key_ends: Pointer[mut=True, Int64, _],
    value_starts: Pointer[mut=True, Int64, _],
    value_ends: Pointer[mut=True, Int64, _],
    key_start: Int64,
    key_end: Int64,
    value_start: Int64,
    value_end: Int64,
):
    if count[] >= STRUCTURED_JSON_MAX_SAMPLES:
        return
    for index in range(count[]):
        if (
            key_starts[unsafe_offset=index] == key_start
            and key_ends[unsafe_offset=index] == key_end
            and value_starts[unsafe_offset=index] == value_start
            and value_ends[unsafe_offset=index] == value_end
        ):
            return
    var slot = count[]
    key_starts[unsafe_offset=slot] = key_start
    key_ends[unsafe_offset=slot] = key_end
    value_starts[unsafe_offset=slot] = value_start
    value_ends[unsafe_offset=slot] = value_end
    count[] += 1

def structured_json_record_key(
    view: ProdexRichStringView,
    summary: Pointer[mut=True, StructuredJsonSummary, _],
    key_start: Int64,
    key_end: Int64,
):
    for index in range(summary[].key_count):
        if structured_json_range_equals(
            view, key_start, key_end,
            summary[].key_starts.unsafe_ptr()[unsafe_offset=index], summary[].key_ends.unsafe_ptr()[unsafe_offset=index]
        ):
            summary[].key_counts.unsafe_ptr()[unsafe_offset=index] += 1
            return
    if summary[].key_count < STRUCTURED_JSON_MAX_KEYS:
        var slot = Int(summary[].key_count)
        summary[].key_starts.unsafe_ptr()[unsafe_offset=slot] = key_start
        summary[].key_ends.unsafe_ptr()[unsafe_offset=slot] = key_end
        summary[].key_counts.unsafe_ptr()[unsafe_offset=slot] = 1
        summary[].key_count += 1

def structured_json_record_level(
    view: ProdexRichStringView,
    summary: Pointer[mut=True, StructuredJsonSummary, _],
    value_start: Int64,
    value_end: Int64,
):
    if value_start < 0 or value_end <= value_start or deepseek_json_byte(view, value_start) != 34:
        return
    for index in range(summary[].level_count):
        if structured_json_range_equals(
            view, value_start, value_end,
            summary[].level_starts.unsafe_ptr()[unsafe_offset=index], summary[].level_ends.unsafe_ptr()[unsafe_offset=index]
        ):
            summary[].level_counts.unsafe_ptr()[unsafe_offset=index] += 1
            return
    if summary[].level_count < STRUCTURED_JSON_MAX_LEVELS:
        var slot = Int(summary[].level_count)
        summary[].level_starts.unsafe_ptr()[unsafe_offset=slot] = value_start
        summary[].level_ends.unsafe_ptr()[unsafe_offset=slot] = value_end
        summary[].level_counts.unsafe_ptr()[unsafe_offset=slot] = 1
        summary[].level_count += 1

def structured_json_collect_key_sample(
    view: ProdexRichStringView,
    summary: Pointer[mut=True, StructuredJsonSummary, _],
    key_start: Int64,
    key_end: Int64,
    value_start: Int64,
    value_end: Int64,
):
    if structured_json_key_is_level(view, key_start, key_end):
        structured_json_record_level(view, summary, value_start, value_end)
        if structured_json_level_is_failure(view, value_start, value_end):
            structured_json_record_sample(
                Pointer(to=summary[].error_count), summary[].error_key_starts.unsafe_ptr(),
                summary[].error_key_ends.unsafe_ptr(), summary[].error_value_starts.unsafe_ptr(),
                summary[].error_value_ends.unsafe_ptr(), key_start, key_end, value_start, value_end,
            )
    if structured_json_key_is_error(view, key_start, key_end):
        structured_json_record_sample(
            Pointer(to=summary[].error_count), summary[].error_key_starts.unsafe_ptr(),
            summary[].error_key_ends.unsafe_ptr(), summary[].error_value_starts.unsafe_ptr(),
            summary[].error_value_ends.unsafe_ptr(), key_start, key_end, value_start, value_end,
        )
    if structured_json_key_is_path(view, key_start, key_end) and structured_json_value_is_scalar(view, value_start, value_end):
        structured_json_record_sample(
            Pointer(to=summary[].path_count), summary[].path_key_starts.unsafe_ptr(),
            summary[].path_key_ends.unsafe_ptr(), summary[].path_value_starts.unsafe_ptr(),
            summary[].path_value_ends.unsafe_ptr(), key_start, key_end, value_start, value_end,
        )
    if structured_json_key_is_id(view, key_start, key_end) and structured_json_value_is_scalar(view, value_start, value_end):
        structured_json_record_sample(
            Pointer(to=summary[].id_count), summary[].id_key_starts.unsafe_ptr(),
            summary[].id_key_ends.unsafe_ptr(), summary[].id_value_starts.unsafe_ptr(),
            summary[].id_value_ends.unsafe_ptr(), key_start, key_end, value_start, value_end,
        )

def structured_json_summarize(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
    summary: Pointer[mut=True, StructuredJsonSummary, _],
) -> Bool:
    if summary[].visited >= STRUCTURED_JSON_MAX_VISITED or depth > STRUCTURED_JSON_MAX_DEPTH:
        return True
    if start < 0 or end <= start:
        return False
    summary[].visited += 1
    var first = deepseek_json_byte(view, start)
    if first == 123:
        summary[].objects += 1
        var cursor = deepseek_json_skip_ws(view, start + 1, end - 1)
        while cursor < end - 1:
            var key_start = cursor
            var key_end = deepseek_json_string_end(view, key_start, end - 1)
            if key_end < 0:
                return False
            cursor = deepseek_json_skip_ws(view, key_end, end - 1)
            if cursor >= end - 1 or deepseek_json_byte(view, cursor) != 58:
                return False
            var value_start = deepseek_json_skip_ws(view, cursor + 1, end - 1)
            var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
            if value_end < 0:
                return False
            structured_json_record_key(view, summary, key_start, key_end)
            structured_json_collect_key_sample(view, summary, key_start, key_end, value_start, value_end)
            if not structured_json_summarize(view, value_start, value_end, depth + 1, summary):
                return False
            if summary[].visited >= STRUCTURED_JSON_MAX_VISITED:
                return True
            cursor = deepseek_json_skip_ws(view, value_end, end - 1)
            if cursor < end - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, end - 1)
                continue
            if cursor == end - 1:
                break
            return False
        return True
    if first == 91:
        summary[].arrays += 1
        var cursor = deepseek_json_skip_ws(view, start + 1, end - 1)
        while cursor < end - 1:
            var value_end = deepseek_json_value_end(view, cursor, end - 1, 0)
            if value_end < 0:
                return False
            if not structured_json_summarize(view, cursor, value_end, depth + 1, summary):
                return False
            if summary[].visited >= STRUCTURED_JSON_MAX_VISITED:
                return True
            cursor = deepseek_json_skip_ws(view, value_end, end - 1)
            if cursor < end - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, end - 1)
                continue
            if cursor == end - 1:
                break
            return False
        return True
    summary[].scalars += 1
    return True

def structured_json_direct_count(view: ProdexRichStringView, start: Int64, end: Int64) -> Int64:
    if start < 0 or end <= start:
        return 0
    var first = deepseek_json_byte(view, start)
    if first == 123:
        var count: Int64 = 0
        var cursor = deepseek_json_skip_ws(view, start + 1, end - 1)
        while cursor < end - 1:
            var key_end = deepseek_json_string_end(view, cursor, end - 1)
            if key_end < 0:
                return -1
            cursor = deepseek_json_skip_ws(view, key_end, end - 1)
            if cursor >= end - 1 or deepseek_json_byte(view, cursor) != 58:
                return -1
            var value_start = deepseek_json_skip_ws(view, cursor + 1, end - 1)
            var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
            if value_end < 0:
                return -1
            count += 1
            cursor = deepseek_json_skip_ws(view, value_end, end - 1)
            if cursor < end - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, end - 1)
            elif cursor != end - 1:
                return -1
        return count
    if first == 91:
        var count: Int64 = 0
        var cursor = deepseek_json_skip_ws(view, start + 1, end - 1)
        while cursor < end - 1:
            var value_end = deepseek_json_value_end(view, cursor, end - 1, 0)
            if value_end < 0:
                return -1
            count += 1
            cursor = deepseek_json_skip_ws(view, value_end, end - 1)
            if cursor < end - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, end - 1)
            elif cursor != end - 1:
                return -1
        return count
    return 0

def structured_json_put_key_content(
    writer: Pointer[mut=True, StructuredJsonWriter, _], view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end - start < 2:
        return False
    return structured_json_put_range(writer, view, start + 1, end - 1)

def structured_json_put_scalar_sample(
    writer: Pointer[mut=True, StructuredJsonWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    max_bytes: Int64,
) -> Bool:
    if start < 0 or end <= start:
        return structured_json_put_literal(writer, StringSlice("null"))
    var length = end - start
    if length > max_bytes:
        return (
            structured_json_put_literal(writer, StringSlice("value(bytes="))
            and structured_json_put_i64(writer, length)
            and structured_json_put_byte(writer, 41)
        )
    if deepseek_json_byte(view, start) == 34 and length >= 2:
        return structured_json_put_range(writer, view, start + 1, end - 1)
    if deepseek_json_byte(view, start) == 123:
        if not structured_json_put_literal(writer, StringSlice("object=")):
            return False
    elif deepseek_json_byte(view, start) == 91:
        if not structured_json_put_literal(writer, StringSlice("array=")):
            return False
    return structured_json_put_range(writer, view, start, end)

def structured_json_put_count_map(
    writer: Pointer[mut=True, StructuredJsonWriter, _],
    view: ProdexRichStringView,
    label: StringSlice,
    count: Int64,
    starts: Pointer[mut=True, Int64, _],
    ends: Pointer[mut=True, Int64, _],
    counts: Pointer[mut=True, Int64, _],
) -> Bool:
    if count <= 0:
        return True
    if not structured_json_put_literal(writer, label):
        return False
    for index in range(count):
        if index > 0 and not structured_json_put_literal(writer, StringSlice(", ")):
            return False
        if not structured_json_put_key_content(writer, view, starts[unsafe_offset=index], ends[unsafe_offset=index]):
            return False
        if not structured_json_put_byte(writer, 61) or not structured_json_put_i64(writer, counts[unsafe_offset=index]):
            return False
    return structured_json_put_byte(writer, 10)

def structured_json_put_sample_group(
    writer: Pointer[mut=True, StructuredJsonWriter, _],
    view: ProdexRichStringView,
    label: StringSlice,
    count: Int64,
    key_starts: Pointer[mut=True, Int64, _],
    key_ends: Pointer[mut=True, Int64, _],
    value_starts: Pointer[mut=True, Int64, _],
    value_ends: Pointer[mut=True, Int64, _],
    max_line_bytes: Int64,
) -> Bool:
    if count <= 0:
        return True
    if not structured_json_put_literal(writer, label) or not structured_json_put_byte(writer, 10):
        return False
    var sample_max = max_line_bytes - 16
    if sample_max < 32:
        sample_max = 32
    if sample_max > 512:
        sample_max = 512
    for index in range(count):
        if not structured_json_put_literal(writer, StringSlice("  ")):
            return False
        if not structured_json_put_key_content(writer, view, key_starts[unsafe_offset=index], key_ends[unsafe_offset=index]):
            return False
        if not structured_json_put_byte(writer, 61):
            return False
        if not structured_json_put_scalar_sample(
            writer, view, value_starts[unsafe_offset=index], value_ends[unsafe_offset=index], sample_max
        ):
            return False
        if not structured_json_put_byte(writer, 10):
            return False
    return True

def structured_json_write_summary(
    writer: Pointer[mut=True, StructuredJsonWriter, _],
    view: ProdexRichStringView,
    summary: Pointer[mut=True, StructuredJsonSummary, _],
    max_line_bytes: Int64,
) -> Bool:
    if not structured_json_put_literal(writer, StringSlice("nodes: objects=")) or not structured_json_put_i64(writer, summary[].objects):
        return False
    if not structured_json_put_literal(writer, StringSlice(", arrays=")) or not structured_json_put_i64(writer, summary[].arrays):
        return False
    if not structured_json_put_literal(writer, StringSlice(", scalars=")) or not structured_json_put_i64(writer, summary[].scalars):
        return False
    if not structured_json_put_literal(writer, StringSlice(", visited=")) or not structured_json_put_i64(writer, summary[].visited) or not structured_json_put_byte(writer, 10):
        return False
    if not structured_json_put_count_map(
        writer, view, StringSlice("keys: "), summary[].key_count,
        summary[].key_starts.unsafe_ptr(), summary[].key_ends.unsafe_ptr(), summary[].key_counts.unsafe_ptr(),
    ):
        return False
    if not structured_json_put_count_map(
        writer, view, StringSlice("levels: "), summary[].level_count,
        summary[].level_starts.unsafe_ptr(), summary[].level_ends.unsafe_ptr(), summary[].level_counts.unsafe_ptr(),
    ):
        return False
    if not structured_json_put_sample_group(
        writer, view, StringSlice("error samples:"), summary[].error_count,
        summary[].error_key_starts.unsafe_ptr(), summary[].error_key_ends.unsafe_ptr(),
        summary[].error_value_starts.unsafe_ptr(), summary[].error_value_ends.unsafe_ptr(), max_line_bytes,
    ):
        return False
    if not structured_json_put_sample_group(
        writer, view, StringSlice("path samples:"), summary[].path_count,
        summary[].path_key_starts.unsafe_ptr(), summary[].path_key_ends.unsafe_ptr(),
        summary[].path_value_starts.unsafe_ptr(), summary[].path_value_ends.unsafe_ptr(), max_line_bytes,
    ):
        return False
    return structured_json_put_sample_group(
        writer, view, StringSlice("id samples:"), summary[].id_count,
        summary[].id_key_starts.unsafe_ptr(), summary[].id_key_ends.unsafe_ptr(),
        summary[].id_value_starts.unsafe_ptr(), summary[].id_value_ends.unsafe_ptr(), max_line_bytes,
    )

def structured_json_whole(
    view: ProdexRichStringView,
    line_count: Int64,
    max_line_bytes: Int64,
    writer: Pointer[mut=True, StructuredJsonWriter, _],
) -> Bool:
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    if start >= Int64(view.len):
        return False
    var first = deepseek_json_byte(view, start)
    if first != 123 and first != 91:
        return False
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len):
        return False
    var summary = structured_json_summary()
    var summary_ptr = Pointer(to=summary)
    if not structured_json_summarize(view, start, end, 0, summary_ptr):
        return False
    if not structured_json_put_literal(writer, StringSlice("pcs: json (")) or not structured_json_put_i64(writer, line_count) or not structured_json_put_literal(writer, StringSlice("->sum)\nshape: ")):
        return False
    if first == 123:
        if not structured_json_put_literal(writer, StringSlice("object keys=")):
            return False
    else:
        if not structured_json_put_literal(writer, StringSlice("array items=")):
            return False
    var direct = structured_json_direct_count(view, start, end)
    if direct < 0 or not structured_json_put_i64(writer, direct) or not structured_json_put_byte(writer, 10):
        return False
    return structured_json_write_summary(writer, view, summary_ptr, max_line_bytes)

def structured_json_ndjson(
    view: ProdexRichStringView,
    line_count: Int64,
    max_line_bytes: Int64,
    writer: Pointer[mut=True, StructuredJsonWriter, _],
) -> Bool:
    var summary = structured_json_summary()
    var summary_ptr = Pointer(to=summary)
    var parsed: Int64 = 0
    var non_empty: Int64 = 0
    var line_start: Int64 = 0
    var length = Int64(view.len)
    var cursor: Int64 = 0
    while cursor <= length:
        if cursor == length or deepseek_json_byte(view, cursor) == 10:
            var start = deepseek_json_skip_ws(view, line_start, cursor)
            var end = cursor
            while end > start and (
                deepseek_json_byte(view, end - 1) == 9
                or deepseek_json_byte(view, end - 1) == 13
                or deepseek_json_byte(view, end - 1) == 32
            ):
                end -= 1
            if start < end:
                non_empty += 1
                var value_end = deepseek_json_value_end(view, start, end, 0)
                if value_end >= 0 and deepseek_json_skip_ws(view, value_end, end) == end:
                    parsed += 1
                    if not structured_json_summarize(view, start, end, 0, summary_ptr):
                        return False
            line_start = cursor + 1
        cursor += 1
    if parsed < STRUCTURED_JSON_NDJSON_MIN_RECORDS or non_empty < STRUCTURED_JSON_NDJSON_MIN_RECORDS or parsed * 2 < non_empty:
        return False
    if not structured_json_put_literal(writer, StringSlice("pcs: ndjson (")) or not structured_json_put_i64(writer, line_count) or not structured_json_put_literal(writer, StringSlice("->sum)\nsum: ndjson records=")):
        return False
    if not structured_json_put_i64(writer, parsed) or not structured_json_put_literal(writer, StringSlice(", non_json=")) or not structured_json_put_i64(writer, non_empty - parsed) or not structured_json_put_byte(writer, 10):
        return False
    return structured_json_write_summary(writer, view, summary_ptr, max_line_bytes)

@export("prodex_context_structured_json_compact_v1")
def context_structured_json_compact_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    line_count: Int64,
    max_line_bytes: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if input_length < 0 or input_length > STRUCTURED_JSON_MAX_BYTES or line_count < 0 or max_line_bytes < 0:
        return 1
    if input_length > 0 and input_address == 0:
        return 1
    if output_address == 0 or output_capacity <= 0 or written_address == 0:
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, STRUCTURED_JSON_MAX_BYTES):
        return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    var writer = StructuredJsonWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if structured_json_whole(view, line_count, max_line_bytes, writer_ptr):
        written[] = writer.written
        return 0
    writer.written = 0
    if structured_json_ndjson(view, line_count, max_line_bytes, writer_ptr):
        written[] = writer.written
        return 0
    written[] = 0
    return 0
