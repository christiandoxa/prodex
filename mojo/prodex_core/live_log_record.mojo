from std.memory import Pointer

comptime LIVE_LOG_RECORD_ABI_VERSION: Int64 = 1
comptime LIVE_LOG_RECORD_OK: Int64 = 0
comptime LIVE_LOG_RECORD_INVALID: Int64 = 1
comptime LIVE_LOG_RECORD_CAPACITY: Int64 = 2
comptime LIVE_LOG_RECORD_ABI: Int64 = 4
comptime LIVE_LOG_RECORD_MAX_BYTES: Int64 = 128 * 1024
comptime LIVE_LOG_NESTED_STRING_MAX_BYTES: Int64 = 8 * 1024
comptime LIVE_LOG_RECORD_PREFIX_RESERVE: Int64 = 32
comptime LIVE_LOG_PAGE_MAX_INPUT: Int64 = 2048
comptime LIVE_LOG_FOLLOW_MAX_PENDING_BYTES: Int64 = 1024 * 1024
comptime LIVE_LOG_FOLLOW_MAX_LINE_BYTES: Int64 = 2 * 1024 * 1024
comptime LIVE_LOG_FOLLOW_MAX_INPUT_LINES: Int64 = 2 * 1024 * 1024


def live_log_utf8_prefix_end(
    input: Pointer[mut=False, UInt8, _], length: Int64, boundary: Int64
) -> Tuple[Int64, Bool]:
    var cursor: Int64 = 0
    var end: Int64 = 0
    while cursor < length and cursor < boundary:
        var first = input[unsafe_offset=cursor]
        var width: Int64
        if first < 128:
            width = 1
        elif first >= 194 and first <= 223:
            width = 2
        elif first >= 224 and first <= 239:
            width = 3
        elif first >= 240 and first <= 244:
            width = 4
        else:
            return (0, False)
        if width > length - cursor:
            return (0, False)
        var second: UInt8 = 0
        for offset in range(Int64(1), width):
            var next_byte = input[unsafe_offset=cursor + offset]
            if next_byte < 128 or next_byte > 191:
                return (0, False)
            if offset == 1:
                second = next_byte
        if (first == 224 and second < 160) or (first == 237 and second > 159):
            return (0, False)
        if (first == 240 and second < 144) or (first == 244 and second > 143):
            return (0, False)
        cursor += width
        end = cursor
    return (end, True)


@export("prodex_live_log_record_over_limit_v1")
def prodex_live_log_record_over_limit_v1(
    abi_version: Int64, record_length: Int64, output_address: UInt
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if record_length < 0 or output_address == 0:
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(record_length > LIVE_LOG_RECORD_MAX_BYTES)
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_string_clip_end_v1")
def prodex_live_log_string_clip_end_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        input_length < 0
        or output_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if input_length <= LIVE_LOG_NESTED_STRING_MAX_BYTES:
        output[unsafe_offset=0] = input_length
        return LIVE_LOG_RECORD_OK
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var prefix_result = live_log_utf8_prefix_end(
        input, input_length, LIVE_LOG_NESTED_STRING_MAX_BYTES
    )
    var end = prefix_result[0]
    var valid = prefix_result[1]
    if not valid:
        return LIVE_LOG_RECORD_INVALID
    output[unsafe_offset=0] = end
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_json_plan_v1")
def prodex_live_log_json_plan_v1(
    abi_version: Int64, serialized_length: Int64, output_address: UInt
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if serialized_length < 0 or output_address == 0:
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var compact = serialized_length > LIVE_LOG_RECORD_MAX_BYTES
    output[unsafe_offset=0] = Int64(compact)
    output[unsafe_offset=1] = Int64(compact)
    output[unsafe_offset=2] = Int64(compact)
    output[unsafe_offset=3] = Int64(compact)
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_plain_text_truncate_v1")
def prodex_live_log_plain_text_truncate_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        input_length <= LIVE_LOG_RECORD_MAX_BYTES
        or output_capacity < 0
        or output_address == 0
        or written_address == 0
        or input_address == 0
    ):
        return LIVE_LOG_RECORD_INVALID
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var prefix_result = live_log_utf8_prefix_end(
        input,
        input_length,
        LIVE_LOG_RECORD_MAX_BYTES - LIVE_LOG_RECORD_PREFIX_RESERVE,
    )
    var end = prefix_result[0]
    var valid = prefix_result[1]
    if not valid:
        return LIVE_LOG_RECORD_INVALID
    var tail = StringSlice(" …[truncated]\n")
    var tail_pointer = tail.unsafe_ptr()
    var tail_length = Int64(tail.byte_length())
    var required = end + tail_length
    if output_capacity < required:
        return LIVE_LOG_RECORD_CAPACITY
    for index in range(end):
        output[unsafe_offset=index] = input[unsafe_offset=index]
    for index in range(tail_length):
        output[unsafe_offset=end + index] = tail_pointer[unsafe_offset=index]
    written[unsafe_offset=0] = required
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_snapshot_plan_v1")
def prodex_live_log_snapshot_plan_v1(
    abi_version: Int64,
    sequences_address: UInt,
    sequence_count: Int64,
    after: UInt64,
    limit: Int64,
    max_entries: Int64,
    next_sequence: UInt64,
    dropped: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        sequence_count < 0
        or sequence_count > LIVE_LOG_PAGE_MAX_INPUT
        or limit < 0
        or max_entries < 0
        or max_entries > LIVE_LOG_PAGE_MAX_INPUT
        or output_address == 0
        or (sequence_count > 0 and sequences_address == 0)
    ):
        return LIVE_LOG_RECORD_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var count = sequence_count
    var sequences = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(sequences_address)
    )
    var previous: UInt64 = 0
    for index in range(count):
        var sequence = sequences[unsafe_offset=index]
        if index > 0 and sequence < previous:
            return LIVE_LOG_RECORD_INVALID
        previous = sequence

    var start: Int64 = 0
    while start < count and sequences[unsafe_offset=start] <= after:
        start += 1
    var requested = limit
    if requested > max_entries:
        requested = max_entries
    var available = count - start
    var selected = available if available < requested else requested
    var cursor = next_sequence
    if selected > 0:
        cursor = sequences[unsafe_offset=start + selected - 1]
    output[unsafe_offset=0] = UInt64(start)
    output[unsafe_offset=1] = UInt64(selected)
    output[unsafe_offset=2] = cursor
    output[unsafe_offset=3] = dropped
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_follow_plan_v1")
def prodex_live_log_follow_plan_v1(
    abi_version: Int64,
    line_lengths_address: UInt,
    line_count: Int64,
    pending_length: Int64,
    keep_address: UInt,
    dropped_address: UInt,
    pending_gap_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        line_count < 0
        or line_count > LIVE_LOG_FOLLOW_MAX_INPUT_LINES
        or pending_length < 0
        or dropped_address == 0
        or pending_gap_address == 0
        or (line_count > 0 and (line_lengths_address == 0 or keep_address == 0))
    ):
        return LIVE_LOG_RECORD_INVALID

    var lengths = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(line_lengths_address)
    )
    var keep = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(keep_address)
    )
    var dropped = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(dropped_address)
    )
    var pending_gap = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(pending_gap_address)
    )
    var dropped_count: Int64 = 0
    for index in range(line_count):
        var keep_line = lengths[unsafe_offset=index] <= LIVE_LOG_FOLLOW_MAX_LINE_BYTES
        keep[unsafe_offset=index] = Int64(keep_line)
        if not keep_line:
            dropped_count += 1
    dropped[] = dropped_count
    pending_gap[] = Int64(pending_length > LIVE_LOG_FOLLOW_MAX_PENDING_BYTES)
    return LIVE_LOG_RECORD_OK


# Typed in-memory retention decisions. Synchronization, path keys, and
# actual queue updates are host responsibilities.
comptime LIVE_LOG_BUCKET_ENTRY_LIMIT: UInt64 = 512
comptime LIVE_LOG_BUCKET_BYTE_LIMIT: UInt64 = 2 * 1024 * 1024
comptime LIVE_LOG_TOTAL_ENTRY_LIMIT: UInt64 = 2048
comptime LIVE_LOG_TOTAL_BYTE_LIMIT: UInt64 = 8 * 1024 * 1024
comptime LIVE_LOG_PATH_LIMIT: UInt64 = 64


@export("prodex_live_log_retention_trim_v1")
def prodex_live_log_retention_trim_v1(
    abi_version: Int64, scope: Int64, entries: UInt64, bytes: UInt64,
    paths: UInt64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if scope < 0 or scope > 1 or output_address == 0 or output_address % 8 != 0:
        return LIVE_LOG_RECORD_INVALID
    var trim = (
        entries > LIVE_LOG_BUCKET_ENTRY_LIMIT
        or bytes > LIVE_LOG_BUCKET_BYTE_LIMIT
    ) if scope == 0 else (
        entries > LIVE_LOG_TOTAL_ENTRY_LIMIT
        or bytes > LIVE_LOG_TOTAL_BYTE_LIMIT
        or paths > LIVE_LOG_PATH_LIMIT
    )
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = Int64(trim)
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_oldest_path_v1")
def prodex_live_log_oldest_path_v1(
    abi_version: Int64,
    sequences_address: UInt, sequence_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        sequence_count <= 0 or sequence_count > 65
        or sequences_address == 0 or sequences_address % 8 != 0
        or output_address == 0 or output_address % 8 != 0
    ):
        return LIVE_LOG_RECORD_INVALID
    var sequences = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(sequences_address)
    )
    var index: Int64 = 0
    var oldest = sequences[unsafe_offset=0]
    for candidate in range(Int64(1), sequence_count):
        var next_value = sequences[unsafe_offset=candidate]
        if next_value < oldest:
            oldest = next_value
            index = candidate
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = index
    return LIVE_LOG_RECORD_OK


# Canonical compact JSON output when an oversized live record retains only
# selected event/pid/timestamp metadata and a fixed truncation marker.
# The Rust host serializes existing Serde field values only; the compiled
# owner chooses ordering, conditional emission, punctuation and newline.
from json_sink import JsonSink, js_byte, js_literal, js_view
from rich_text import rich_view_valid
from rich_types import ProdexRichStringView

comptime LIVE_LOG_COMPACT_FIELD_COUNT: Int64 = 3


def live_log_compact_field(
    rows: Pointer[mut=False, UInt64, _], index: Int64,
) -> ProdexRichStringView:
    return ProdexRichStringView(
        UInt(rows[unsafe_offset=index * 3 + 1]),
        UInt(rows[unsafe_offset=index * 3 + 2]),
    )


@export("prodex_live_log_compact_json_v1")
def prodex_live_log_compact_json_v1(
    abi_version: Int64, fields_address: UInt, field_count: Int64,
    output_address: UInt, output_capacity: Int64, measuring: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        fields_address == 0 or fields_address % 8 != 0
        or field_count != LIVE_LOG_COMPACT_FIELD_COUNT
        or written_address == 0 or written_address % 8 != 0
        or output_capacity < 0 or (measuring != 0 and measuring != 1)
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return LIVE_LOG_RECORD_INVALID
    var fields = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    for index in range(LIVE_LOG_COMPACT_FIELD_COUNT):
        var tag = fields[unsafe_offset=index * 3]
        var address = fields[unsafe_offset=index * 3 + 1]
        var length = fields[unsafe_offset=index * 3 + 2]
        if tag > 1:
            return LIVE_LOG_RECORD_INVALID
        if tag == 0 and (address != 0 or length != 0):
            return LIVE_LOG_RECORD_INVALID
        if tag == 1 and not rich_view_valid(
            ProdexRichStringView(UInt(address), UInt(length)),
            0x7FFFFFFFFFFFFFFF,
        ):
            return LIVE_LOG_RECORD_INVALID
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ), output_capacity, 0, measuring == 1, False,
    )
    if fields[unsafe_offset=0] == 1:
        js_literal(Pointer(to=sink), StringSlice('{"event":'))
        js_view(Pointer(to=sink), live_log_compact_field(fields, 0))
        js_byte(Pointer(to=sink), 44)
    else:
        js_byte(Pointer(to=sink), 123)
    js_literal(Pointer(to=sink), StringSlice('"message":"[live log record truncated]"'))
    if fields[unsafe_offset=3] == 1:
        js_literal(Pointer(to=sink), StringSlice(',"pid":'))
        js_view(Pointer(to=sink), live_log_compact_field(fields, 1))
    if fields[unsafe_offset=6] == 1:
        js_literal(Pointer(to=sink), StringSlice(',"timestamp":'))
        js_view(Pointer(to=sink), live_log_compact_field(fields, 2))
    js_byte(Pointer(to=sink), 125)
    js_byte(Pointer(to=sink), 10)
    if sink.failed:
        return LIVE_LOG_RECORD_CAPACITY
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = sink.written
    return LIVE_LOG_RECORD_OK


# Compiled JSON-tree owner for bounded live-log JSON. The host acquires
# Serde's canonical number lexemes and borrowed string/key views; this
# owner alone traverses containers, clips nested values at UTF-8-safe
# boundaries, and serializes the canonical clipped JSON document.
from parsed_json import (
    JSON_NULL, JSON_FALSE, JSON_TRUE, JSON_NUMBER, JSON_STRING,
    JSON_ARRAY, JSON_OBJECT, ParsedJson, ParsedJsonNode,
    pj_child, pj_kind, pj_next, pj_text, pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_raw
from rich_types import ProdexRichStringView


def live_log_json_emit_node(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64,
) -> Bool:
    var kind = pj_kind(tree, node)
    if kind == JSON_NULL:
        js_literal(sink, StringSlice("null"))
    elif kind == JSON_FALSE:
        js_literal(sink, StringSlice("false"))
    elif kind == JSON_TRUE:
        js_literal(sink, StringSlice("true"))
    elif kind == JSON_NUMBER:
        js_raw(sink, tree, node)
    elif kind == JSON_STRING:
        var value = pj_text(tree, node)
        var length = Int64(value.len)
        js_byte(sink, 34)
        if length > LIVE_LOG_NESTED_STRING_MAX_BYTES:
            var clipped = live_log_utf8_prefix_end(
                Pointer[mut=False, UInt8, ImmUntrackedOrigin](
                    unsafe_from_address=Int(value.ptr)
                ), length, LIVE_LOG_NESTED_STRING_MAX_BYTES,
            )
            if not clipped[1]:
                return False
            js_escaped(
                sink, ProdexRichStringView(value.ptr, UInt(clipped[0])),
            )
            js_literal(sink, StringSlice(" …[truncated]"))
        else:
            js_escaped(sink, value)
        js_byte(sink, 34)
    elif kind == JSON_ARRAY:
        js_byte(sink, 91)
    elif kind == JSON_OBJECT:
        js_byte(sink, 123)
    else:
        return False
    return True


def live_log_json_project(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson) -> Bool:
    var current: Int64 = 0
    while current >= 0:
        var kind = pj_kind(tree, current)
        if current != 0:
            var parent = tree.nodes[unsafe_offset=current].parent
            if pj_child(tree, parent) != current:
                js_byte(sink, 44)
            if pj_kind(tree, parent) == JSON_OBJECT:
                js_string(sink, tree.nodes[unsafe_offset=current].key)
                js_byte(sink, 58)
        if not live_log_json_emit_node(sink, tree, current):
            return False
        var child: Int64 = -1
        if kind == JSON_ARRAY or kind == JSON_OBJECT:
            child = pj_child(tree, current)
        if child >= 0:
            current = child
            continue
        if kind == JSON_ARRAY:
            js_byte(sink, 93)
        elif kind == JSON_OBJECT:
            js_byte(sink, 125)
        var advanced = False
        while current != 0:
            var sibling = pj_next(tree, current)
            if sibling >= 0:
                current = sibling
                advanced = True
                break
            var parent = tree.nodes[unsafe_offset=current].parent
            var parent_kind = pj_kind(tree, parent)
            if parent_kind == JSON_ARRAY:
                js_byte(sink, 93)
            elif parent_kind == JSON_OBJECT:
                js_byte(sink, 125)
            current = parent
        if not advanced:
            break
    return not sink[].failed


@export("prodex_live_log_json_projection_v1")
def prodex_live_log_json_projection_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt, node_count: Int64,
    raw_address: UInt, raw_length: Int64,
    scratch_address: UInt, scratch_count: Int64,
    measuring: Int64, output_address: UInt, output_capacity: Int64,
    metadata_address: UInt,
) abi("C") -> Int64:
    if abi != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        operation != 0 or flag != 0
        or node_count <= 0 or nodes_address == 0
        or nodes_address % 8 != 0
        or raw_length < 0 or (raw_length > 0 and raw_address == 0)
        or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count < node_count
        or metadata_address == 0 or metadata_address % 8 != 0
        or measuring < 0 or measuring > 1 or output_capacity < 0
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return LIVE_LOG_RECORD_INVALID
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ), node_count, ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree):
        return LIVE_LOG_RECORD_INVALID
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ), output_capacity, 0, measuring == 1, False,
    )
    if not live_log_json_project(Pointer(to=sink), tree):
        # The shared JSON-tree Rust adapter expects status 3 for capacity;
        # the older scalar live-log ABI uses status 2.
        return 3 if sink.failed else LIVE_LOG_RECORD_INVALID
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return LIVE_LOG_RECORD_OK


# Live file-follow byte framing: binary last-complete-line prefix followed
# by UTF-8 line lengths after the host performs the historical lossy decode.
# These two operations replace Rust rposition and split_inclusive length
# acquisition without moving host file I/O or decoded String ownership.
@export("prodex_live_log_follow_last_newline_v1")
def prodex_live_log_follow_last_newline_v1(
    abi_version: Int64,
    buffer_address: UInt, buffer_length: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        buffer_length < 0 or (buffer_length > 0 and buffer_address == 0)
        or result_address == 0 or result_address % 8 != 0
    ):
        return LIVE_LOG_RECORD_INVALID
    var best = Int64(0)
    if buffer_length > 0:
        var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(buffer_address)
        )
        for index in range(buffer_length):
            if bytes[unsafe_offset=index] == 10:
                best = index + 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = best
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_follow_line_lengths_v1")
def prodex_live_log_follow_line_lengths_v1(
    abi_version: Int64,
    text_address: UInt, text_length: Int64,
    output_address: UInt, output_capacity: Int64,
    measuring: Int64, count_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        text_length < 0 or (text_length > 0 and text_address == 0)
        or output_capacity < 0
        or (measuring != 0 and measuring != 1)
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
        or count_address == 0 or count_address % 8 != 0
    ):
        return LIVE_LOG_RECORD_INVALID
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(text_address)
    )
    if text_length > 0:
        var utf8 = live_log_utf8_prefix_end(bytes, text_length, text_length)
        if not utf8[1] or utf8[0] != text_length:
            return LIVE_LOG_RECORD_INVALID
    var count = Int64(0)
    for index in range(text_length):
        if bytes[unsafe_offset=index] == 10:
            count += 1
    if text_length > 0 and bytes[unsafe_offset=text_length - 1] != 10:
        count += 1
    if count > LIVE_LOG_FOLLOW_MAX_INPUT_LINES:
        return LIVE_LOG_RECORD_INVALID
    if measuring == 0 and output_capacity < count:
        return LIVE_LOG_RECORD_CAPACITY

    if measuring == 0:
        var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var first = Int64(0)
        var item = Int64(0)
        for index in range(text_length):
            if bytes[unsafe_offset=index] == 10:
                output[unsafe_offset=item] = index + 1 - first
                first = index + 1
                item += 1
        if first < text_length:
            output[unsafe_offset=item] = text_length - first
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(count_address)
    )[] = count
    return LIVE_LOG_RECORD_OK
