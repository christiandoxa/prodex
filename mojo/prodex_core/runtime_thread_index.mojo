from std.memory import Pointer

from parsed_json import (
    JSON_NULL,
    JSON_NUMBER,
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_field,
    pj_kind,
    pj_text,
    pj_valid,
)
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime THREAD_INDEX_ABI_VERSION: Int64 = 1
comptime THREAD_INDEX_MAX_JSON_BYTES: Int64 = 4 * 1024 * 1024
comptime THREAD_INDEX_MAX_JSON_NODES: Int64 = 65_536
comptime THREAD_INDEX_MAX_SEEN_CURSORS: Int64 = 65_536
comptime THREAD_INDEX_MAX_SEEN_CURSOR_BYTES: Int64 = 16 * 1024 * 1024

comptime OP_START: Int64 = 0
comptime OP_RESPONSE: Int64 = 1
comptime OP_EOF: Int64 = 2
comptime OP_INVALID_JSON: Int64 = 3

comptime SCOPE_FULL: Int64 = 0
comptime SCOPE_LATEST: Int64 = 1
comptime PHASE_INITIALIZE: Int64 = 0
comptime PHASE_LIST: Int64 = 1

comptime ACTION_IGNORE: Int64 = 0
comptime ACTION_SEND: Int64 = 1
comptime ACTION_DONE: Int64 = 2
comptime ACTION_ERROR: Int64 = 3

comptime CURSORS_KEEP: Int64 = 0
comptime CURSORS_CLEAR: Int64 = 1
comptime CURSORS_APPEND: Int64 = 2

comptime MARKER_READ: Int64 = 0
comptime MARKER_MATCH: Int64 = 1

comptime STATE_PRESENT: Int64 = 0
comptime STATE_STALE: Int64 = 1
comptime STATE_MISSING: Int64 = 2
comptime STATE_UNAVAILABLE: Int64 = 3

comptime REPAIR_CHECK_DATABASE_FILES: Int64 = 0
comptime REPAIR_RECONCILE: Int64 = 1
comptime REPAIR_CHECK_DIRTY_MARKER: Int64 = 2
comptime REPAIR_CLEAR_DIRTY_MARKER: Int64 = 3
comptime REPAIR_SAVE_DIRTY_MARKER: Int64 = 4
comptime REPAIR_NOOP: Int64 = 5

comptime PROTOCOL_ERROR_INVALID_JSON: StaticString = "Codex app-server returned invalid JSON during thread index reconciliation"
comptime PROTOCOL_ERROR_EOF: StaticString = "Codex app-server stopped during thread index reconciliation"
comptime PROTOCOL_ERROR_MISSING_RESULT: StaticString = "Codex app-server response is missing its result"
comptime PROTOCOL_ERROR_INVALID_CURSOR: StaticString = "Codex app-server returned an invalid thread list cursor"
comptime PROTOCOL_ERROR_REPEATED_CURSOR: StaticString = "Codex app-server repeated a thread list cursor"
comptime PROTOCOL_ERROR_UNKNOWN_APP_SERVER: StaticString = "unknown app-server error"
comptime PROTOCOL_ERROR_CODEX_PREFIX: StaticString = "Codex thread index reconciliation failed: "


def thread_index_output_ptr(address: UInt) -> Pointer[mut=True, UInt8, MutUntrackedOrigin]:
    return Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(address))


def thread_index_result_ptr(address: UInt) -> Pointer[mut=True, Int64, MutUntrackedOrigin]:
    return Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(address))


def thread_index_put_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def thread_index_put_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: StringSlice,
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not thread_index_put_byte(output, capacity, written, source[unsafe_offset=index]):
            return False
    return True


def thread_index_put_view(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: ProdexRichStringView,
) -> Bool:
    var length = Int64(value.len)
    if length < 0 or written[] < 0 or length > capacity - written[]:
        return False
    var source = rich_view_ptr(value)
    for index in range(length):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += length
    return True


def thread_index_put_u64(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt64,
) -> Bool:
    if value == 0:
        return thread_index_put_byte(output, capacity, written, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not thread_index_put_byte(
            output,
            capacity,
            written,
            UInt8(remaining / divisor) + 48,
        ):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def thread_index_put_json_string(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: ProdexRichStringView,
) -> Bool:
    if not thread_index_put_byte(output, capacity, written, 34):
        return False
    var source = rich_view_ptr(value)
    var run_start: Int64 = 0
    for index in range(Int64(value.len)):
        var byte = source[unsafe_offset=index]
        if byte != 34 and byte != 92 and byte >= 32:
            continue
        if not thread_index_put_view(
            output,
            capacity,
            written,
            ProdexRichStringView(value.ptr + UInt(run_start), UInt(index - run_start)),
        ):
            return False
        run_start = index + 1
        if byte == 34 or byte == 92:
            if not thread_index_put_byte(output, capacity, written, 92) or not thread_index_put_byte(output, capacity, written, byte):
                return False
        elif byte == 8:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\b")):
                return False
        elif byte == 9:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\t")):
                return False
        elif byte == 10:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\n")):
                return False
        elif byte == 12:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\f")):
                return False
        elif byte == 13:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\r")):
                return False
        else:
            if not thread_index_put_literal(output, capacity, written, StringSlice("\\u00")):
                return False
            var high = byte >> 4
            var low = byte & 15
            if not thread_index_put_byte(output, capacity, written, high + UInt8(48 if high < 10 else 87)):
                return False
            if not thread_index_put_byte(output, capacity, written, low + UInt8(48 if low < 10 else 87)):
                return False
    if not thread_index_put_view(
        output,
        capacity,
        written,
        ProdexRichStringView(value.ptr + UInt(run_start), value.len - UInt(run_start)),
    ):
        return False
    return thread_index_put_byte(output, capacity, written, 34)


def thread_index_write_request(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    request_id: Int64,
    scope: Int64,
    archived: Bool,
    cursor_present: Bool,
    cursor: ProdexRichStringView,
    client_version: ProdexRichStringView,
    initialize: Bool,
) -> Bool:
    if initialize:
        return (
            thread_index_put_literal(output, capacity, written, StringSlice("{\"id\":"))
            and thread_index_put_u64(output, capacity, written, UInt64(request_id))
            and thread_index_put_literal(output, capacity, written, StringSlice(",\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"prodex-thread-index-reconciliation\",\"version\":"))
            and thread_index_put_json_string(output, capacity, written, client_version)
            and thread_index_put_literal(output, capacity, written, StringSlice("}}}"))
        )
    return (
        thread_index_put_literal(output, capacity, written, StringSlice("{\"id\":"))
        and thread_index_put_u64(output, capacity, written, UInt64(request_id))
        and thread_index_put_literal(output, capacity, written, StringSlice(",\"method\":\"thread/list\",\"params\":{\"archived\":"))
        and thread_index_put_literal(
            output,
            capacity,
            written,
            StringSlice("true") if archived else StringSlice("false"),
        )
        and thread_index_put_literal(output, capacity, written, StringSlice(",\"cursor\":"))
        and (
            thread_index_put_json_string(output, capacity, written, cursor)
            if cursor_present
            else thread_index_put_literal(output, capacity, written, StringSlice("null"))
        )
        and thread_index_put_literal(output, capacity, written, StringSlice(",\"limit\":"))
        and thread_index_put_u64(
            output,
            capacity,
            written,
            UInt64(1 if scope == SCOPE_LATEST else 100),
        )
        and thread_index_put_literal(output, capacity, written, StringSlice(",\"modelProviders\":[],\"sortKey\":\"updated_at\",\"sourceKinds\":[],\"useStateDbOnly\":false}}"))
    )


def thread_index_output_error(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, Int64, _],
    message: StringSlice,
) -> Bool:
    result[unsafe_offset=0] = ACTION_ERROR
    result[unsafe_offset=4] = 0
    result[unsafe_offset=5] = 0
    result[unsafe_offset=6] = 0
    result[unsafe_offset=7] = 0
    result[unsafe_offset=8] = 0
    result[unsafe_offset=9] = CURSORS_KEEP
    if not thread_index_put_literal(output, capacity, written, message):
        return False
    return True


def thread_index_number_equals(tree: ParsedJson, index: Int64, expected: Int64) -> Bool:
    if expected < 0 or pj_kind(tree, index) != JSON_NUMBER:
        return False
    var node = tree.nodes[unsafe_offset=index].copy()
    if node.raw_length <= 0 or node.raw_length > 20:
        return False
    var source = rich_view_ptr(tree.raw)
    var value: UInt64 = 0
    for offset in range(node.raw_length):
        var byte = source[unsafe_offset=node.raw_start + offset]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > 1844674407370955161 or (value == 1844674407370955161 and digit > 5):
            return False
        value = value * 10 + digit
    return value == UInt64(expected)


def thread_index_next_id(expected_id: Int64) -> Int64:
    if expected_id >= 0x7FFFFFFFFFFFFFFF:
        return -1
    return expected_id + 1


def thread_index_write_list_plan(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, Int64, _],
    scope: Int64,
    archived: Bool,
    expected_id: Int64,
    cursor_present: Bool,
    cursor: ProdexRichStringView,
) -> Int64:
    var request_id = thread_index_next_id(expected_id)
    if request_id < 0:
        return thread_index_response_error(
            output,
            capacity,
            written,
            result,
            StringSlice("thread index reconciliation request id overflow"),
        )
    var start = written[]
    if not thread_index_write_request(
        output,
        capacity,
        written,
        request_id,
        scope,
        archived,
        cursor_present,
        cursor,
        ProdexRichStringView(0, 0),
        False,
    ):
        return 3
    result[unsafe_offset=0] = ACTION_SEND
    result[unsafe_offset=1] = PHASE_LIST
    result[unsafe_offset=2] = Int64(archived)
    result[unsafe_offset=3] = request_id
    result[unsafe_offset=4] = 1
    result[unsafe_offset=5] = start
    result[unsafe_offset=6] = written[] - start
    result[unsafe_offset=7] = 0
    result[unsafe_offset=8] = 0
    result[unsafe_offset=11] = written[]
    return 0


def thread_index_response_error(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, Int64, _],
    message: StringSlice,
) -> Int64:
    if not thread_index_output_error(output, capacity, written, result, message):
        return 3
    result[unsafe_offset=11] = written[]
    return 0


@export("prodex_runtime_thread_index_protocol_v1")
def prodex_runtime_thread_index_protocol_v1(
    abi_version: Int64,
    operation: Int64,
    scope: Int64,
    phase: Int64,
    expected_id: Int64,
    archived: Int64,
    seen_cursors_address: UInt,
    seen_cursors_count: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    version_address: UInt,
    version_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    cursor_output_address: UInt,
    cursor_output_capacity: Int64,
    cursor_written_address: UInt,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != THREAD_INDEX_ABI_VERSION:
        return 4
    if (
        operation < OP_START
        or operation > OP_INVALID_JSON
        or scope < SCOPE_FULL
        or scope > SCOPE_LATEST
        or phase < PHASE_INITIALIZE
        or phase > PHASE_LIST
        or expected_id <= 0
        or archived < 0
        or archived > 1
        or seen_cursors_count < 0
        or seen_cursors_count > THREAD_INDEX_MAX_SEEN_CURSORS
        or nodes_count < 0
        or nodes_count > THREAD_INDEX_MAX_JSON_NODES
        or raw_length < 0
        or raw_length > THREAD_INDEX_MAX_JSON_BYTES
        or version_length < 0
        or version_length > 256
        or output_capacity < 0
        or output_capacity > THREAD_INDEX_MAX_JSON_BYTES + 2048
        or cursor_output_capacity < 0
        or cursor_output_capacity > THREAD_INDEX_MAX_JSON_BYTES
        or output_address == 0
        or result_address == 0
        or cursor_written_address == 0
        or cursor_output_address == 0
        or seen_cursors_count > 0 and seen_cursors_address == 0
        or nodes_count > 0 and nodes_address == 0
        or raw_length > 0 and raw_address == 0
        or version_length > 0 and version_address == 0
    ):
        return 1
    var output = thread_index_output_ptr(output_address)
    var cursor_output = thread_index_output_ptr(cursor_output_address)
    var cursor_written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(cursor_written_address)
    )
    var result = thread_index_result_ptr(result_address)
    for index in range(12):
        result[unsafe_offset=index] = 0
    cursor_written[] = 0
    result[unsafe_offset=1] = phase
    result[unsafe_offset=2] = archived
    result[unsafe_offset=3] = expected_id
    result[unsafe_offset=5] = 0
    result[unsafe_offset=7] = 0

    if operation == OP_START:
        if (
            phase != PHASE_INITIALIZE
            or expected_id != 1
            or archived != 0
            or seen_cursors_count != 0
            or nodes_count != 0
            or raw_length != 0
            or version_length == 0
        ):
            return 1
        var version = ProdexRichStringView(version_address, UInt(version_length))
        if not rich_view_valid(version, 256):
            return 2
        var written: Int64 = 0
        if not thread_index_write_request(
            output,
            output_capacity,
            Pointer(to=written),
            1,
            scope,
            False,
            False,
            ProdexRichStringView(0, 0),
            version,
            True,
        ):
            return 3
        result[unsafe_offset=0] = ACTION_SEND
        result[unsafe_offset=4] = 1
        result[unsafe_offset=5] = 0
        result[unsafe_offset=6] = written
        result[unsafe_offset=11] = written
        return 0

    if operation == OP_EOF:
        var written: Int64 = 0
        return thread_index_response_error(
            output,
            output_capacity,
            Pointer(to=written),
            result,
            StringSlice(PROTOCOL_ERROR_EOF),
        )

    if operation == OP_INVALID_JSON:
        var written: Int64 = 0
        return thread_index_response_error(
            output,
            output_capacity,
            Pointer(to=written),
            result,
            StringSlice(PROTOCOL_ERROR_INVALID_JSON),
        )

    if phase != PHASE_INITIALIZE and phase != PHASE_LIST:
        return 1
    if nodes_count <= 0:
        return 1
    var nodes = Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
        unsafe_from_address=Int(nodes_address)
    )
    var raw = ProdexRichStringView(raw_address, UInt(raw_length))
    if not rich_view_valid(raw, THREAD_INDEX_MAX_JSON_BYTES):
        return 2
    var tree = ParsedJson(nodes, nodes_count, raw.copy())
    if not pj_valid(tree):
        return 1
    var root = Int64(0)
    var id_index = pj_field(tree, root, StringSlice("id"))
    if not thread_index_number_equals(tree, id_index, expected_id):
        result[unsafe_offset=0] = ACTION_IGNORE
        return 0

    var error_index = pj_field(tree, root, StringSlice("error"))
    if error_index >= 0:
        var detail_index = pj_field(tree, error_index, StringSlice("message"))
        var detail = pj_text(tree, detail_index)
        if pj_kind(tree, detail_index) != JSON_STRING:
            detail = ProdexRichStringView(
                UInt(Int(StringSlice(PROTOCOL_ERROR_UNKNOWN_APP_SERVER).unsafe_ptr())),
                UInt(StringSlice(PROTOCOL_ERROR_UNKNOWN_APP_SERVER).byte_length()),
            )
        var written: Int64 = 0
        if not thread_index_put_literal(
            output,
            output_capacity,
            Pointer(to=written),
            StringSlice(PROTOCOL_ERROR_CODEX_PREFIX),
        ) or not thread_index_put_view(output, output_capacity, Pointer(to=written), detail):
            return 3
        result[unsafe_offset=0] = ACTION_ERROR
        result[unsafe_offset=11] = written
        return 0

    var result_index = pj_field(tree, root, StringSlice("result"))
    if result_index < 0:
        var written: Int64 = 0
        return thread_index_response_error(
            output,
            output_capacity,
            Pointer(to=written),
            result,
            StringSlice(PROTOCOL_ERROR_MISSING_RESULT),
        )
    if phase == PHASE_INITIALIZE:
        var written: Int64 = 0
        var notification_start = written
        if not thread_index_put_literal(
            output,
            output_capacity,
            Pointer(to=written),
            StringSlice("{\"method\":\"initialized\"}"),
        ):
            return 3
        var notification_length = written - notification_start
        if not thread_index_put_byte(output, output_capacity, Pointer(to=written), 10):
            return 3
        var request_start = written
        if not thread_index_write_request(
            output,
            output_capacity,
            Pointer(to=written),
            2,
            scope,
            False,
            False,
            ProdexRichStringView(0, 0),
            ProdexRichStringView(0, 0),
            False,
        ):
            return 3
        result[unsafe_offset=0] = ACTION_SEND
        result[unsafe_offset=1] = PHASE_LIST
        result[unsafe_offset=2] = 0
        result[unsafe_offset=3] = 2
        result[unsafe_offset=4] = 2
        result[unsafe_offset=5] = notification_start
        result[unsafe_offset=6] = notification_length
        result[unsafe_offset=7] = request_start
        result[unsafe_offset=8] = written - request_start
        result[unsafe_offset=11] = written
        return 0

    var cursor_index = pj_field(tree, result_index, StringSlice("nextCursor"))
    var cursor_present = cursor_index >= 0 and pj_kind(tree, cursor_index) != JSON_NULL
    var cursor = ProdexRichStringView(0, 0)
    if cursor_present:
        if pj_kind(tree, cursor_index) != JSON_STRING:
            var written: Int64 = 0
            return thread_index_response_error(
                output,
                output_capacity,
                Pointer(to=written),
                result,
                StringSlice(PROTOCOL_ERROR_INVALID_CURSOR),
            )
        cursor = pj_text(tree, cursor_index)

    if scope == SCOPE_LATEST:
        result[unsafe_offset=0] = ACTION_DONE
        return 0

    if cursor_present:
        var seen = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(seen_cursors_address))
        var total_seen_bytes: Int64 = 0
        for index in range(seen_cursors_count):
            var previous = seen[unsafe_offset=index].copy()
            if not rich_view_valid(previous, THREAD_INDEX_MAX_JSON_BYTES):
                return 2
            total_seen_bytes += Int64(previous.len)
            if total_seen_bytes > THREAD_INDEX_MAX_SEEN_CURSOR_BYTES:
                return 1
            if previous.len == cursor.len:
                var previous_ptr = rich_view_ptr(previous)
                var cursor_ptr = rich_view_ptr(cursor)
                var same = True
                for byte_index in range(Int64(cursor.len)):
                    if previous_ptr[unsafe_offset=byte_index] != cursor_ptr[unsafe_offset=byte_index]:
                        same = False
                        break
                if same:
                    var written: Int64 = 0
                    return thread_index_response_error(
                        output,
                        output_capacity,
                        Pointer(to=written),
                        result,
                        StringSlice(PROTOCOL_ERROR_REPEATED_CURSOR),
                    )
        if seen_cursors_count >= THREAD_INDEX_MAX_SEEN_CURSORS:
            return 3
        if Int64(cursor.len) > THREAD_INDEX_MAX_SEEN_CURSOR_BYTES - total_seen_bytes:
            return 3
        if cursor.len > UInt(cursor_output_capacity):
            return 3
        var cursor_source = rich_view_ptr(cursor)
        for index in range(Int64(cursor.len)):
            cursor_output[unsafe_offset=index] = cursor_source[unsafe_offset=index]
        cursor_written[] = Int64(cursor.len)
        result[unsafe_offset=9] = CURSORS_APPEND
        var written: Int64 = 0
        return thread_index_write_list_plan(
            output,
            output_capacity,
            Pointer(to=written),
            result,
            scope,
            archived == 1,
            expected_id,
            True,
            cursor,
        )

    if archived == 0:
        var written: Int64 = 0
        var status = thread_index_write_list_plan(
            output,
            output_capacity,
            Pointer(to=written),
            result,
            scope,
            True,
            expected_id,
            False,
            ProdexRichStringView(0, 0),
        )
        if status != 0:
            return status
        if result[unsafe_offset=0] == ACTION_SEND:
            result[unsafe_offset=9] = CURSORS_CLEAR
        return 0
    result[unsafe_offset=0] = ACTION_DONE
    return 0


@export("prodex_runtime_thread_index_dirty_marker_v1")
def prodex_runtime_thread_index_dirty_marker_v1(
    abi_version: Int64,
    operation: Int64,
    parse_valid: Int64,
    target_address: UInt,
    target_length: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != THREAD_INDEX_ABI_VERSION:
        return 4
    if (
        operation < MARKER_READ
        or operation > MARKER_MATCH
        or parse_valid < 0
        or parse_valid > 1
        or target_length < 0
        or target_length > THREAD_INDEX_MAX_JSON_BYTES
        or nodes_count < 0
        or nodes_count > THREAD_INDEX_MAX_JSON_NODES
        or raw_length < 0
        or raw_length > THREAD_INDEX_MAX_JSON_BYTES
        or output_capacity < 1
        or output_capacity > THREAD_INDEX_MAX_JSON_BYTES + 16
        or output_address == 0
        or result_address == 0
        or target_length > 0 and target_address == 0
        or nodes_count > 0 and nodes_address == 0
        or raw_length > 0 and raw_address == 0
    ):
        return 1
    var output = thread_index_output_ptr(output_address)
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = 0
    result[unsafe_offset=1] = 0
    if parse_valid == 0:
        return 0
    if nodes_count <= 0:
        return 1
    var nodes = Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
        unsafe_from_address=Int(nodes_address)
    )
    var raw = ProdexRichStringView(raw_address, UInt(raw_length))
    if not rich_view_valid(raw, THREAD_INDEX_MAX_JSON_BYTES):
        return 2
    var tree = ParsedJson(nodes, nodes_count, raw.copy())
    if not pj_valid(tree):
        return 1
    var root = Int64(0)
    if pj_kind(tree, root) != JSON_OBJECT:
        return 0
    var schema = pj_field(tree, root, StringSlice("schema_version"))
    if not thread_index_number_equals(tree, schema, 1):
        return 0
    var path_index = pj_field(tree, root, StringSlice("rollout_path"))
    if pj_kind(tree, path_index) != JSON_STRING:
        return 0
    var path = pj_text(tree, path_index)
    if operation == MARKER_MATCH:
        var target = ProdexRichStringView(target_address, UInt(target_length))
        if not rich_view_valid(target, THREAD_INDEX_MAX_JSON_BYTES):
            return 2
        if target.len != path.len:
            return 0
        var target_ptr = rich_view_ptr(target)
        var path_ptr = rich_view_ptr(path)
        for index in range(Int64(target.len)):
            if target_ptr[unsafe_offset=index] != path_ptr[unsafe_offset=index]:
                return 0
        result[unsafe_offset=0] = 1
        return 0

    if path.len > UInt(output_capacity):
        return 3
    var source = rich_view_ptr(path)
    for index in range(Int64(path.len)):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    result[unsafe_offset=0] = 1
    result[unsafe_offset=1] = Int64(path.len)
    return 0


@export("prodex_runtime_thread_index_dirty_marker_contents_v1")
def prodex_runtime_thread_index_dirty_marker_contents_v1(
    abi_version: Int64,
    path_address: UInt,
    path_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != THREAD_INDEX_ABI_VERSION:
        return 4
    if (
        path_length < 0
        or path_length > THREAD_INDEX_MAX_JSON_BYTES
        or output_capacity < 1
        or output_capacity > THREAD_INDEX_MAX_JSON_BYTES * 6 + 64
        or output_address == 0
        or written_address == 0
        or path_length > 0 and path_address == 0
    ):
        return 1
    var path = ProdexRichStringView(path_address, UInt(path_length))
    if not rich_view_valid(path, THREAD_INDEX_MAX_JSON_BYTES):
        return 2
    var output = thread_index_output_ptr(output_address)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not thread_index_put_literal(
        output,
        output_capacity,
        written,
        StringSlice("{\"schema_version\":1,\"rollout_path\":"),
    ) or not thread_index_put_json_string(output, output_capacity, written, path) or not thread_index_put_byte(
        output, output_capacity, written, 125
    ):
        return 3
    return 0


@export("prodex_runtime_thread_index_state_combine_v1")
def prodex_runtime_thread_index_state_combine_v1(
    abi_version: Int64,
    current_state: Int64,
    observed_state: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != THREAD_INDEX_ABI_VERSION:
        return 4
    if (
        current_state < STATE_PRESENT
        or current_state > STATE_UNAVAILABLE
        or observed_state < STATE_PRESENT
        or observed_state > STATE_UNAVAILABLE
        or result_address == 0
    ):
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    output[] = min(current_state, observed_state)
    return 0


@export("prodex_runtime_thread_index_repair_action_v1")
def prodex_runtime_thread_index_repair_action_v1(
    abi_version: Int64,
    initial_state: Int64,
    progress: Int64,
    database_files_exist: Int64,
    reconciliation_succeeded: Int64,
    verified_state: Int64,
    marker_matches: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != THREAD_INDEX_ABI_VERSION:
        return 4
    if (
        initial_state < STATE_PRESENT
        or initial_state > STATE_UNAVAILABLE
        or progress < 0
        or progress > 3
        or database_files_exist < 0
        or database_files_exist > 1
        or reconciliation_succeeded < 0
        or reconciliation_succeeded > 1
        or verified_state < STATE_PRESENT
        or verified_state > STATE_UNAVAILABLE
        or marker_matches < 0
        or marker_matches > 1
        or result_address == 0
    ):
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )

    if progress == 0:
        if database_files_exist != 0 or reconciliation_succeeded != 0 or marker_matches != 0:
            return 1
        if initial_state == STATE_PRESENT:
            output[] = REPAIR_CHECK_DIRTY_MARKER
        elif initial_state == STATE_STALE or initial_state == STATE_MISSING:
            output[] = REPAIR_RECONCILE
        else:
            output[] = REPAIR_CHECK_DATABASE_FILES
        return 0

    if progress == 1:
        if initial_state != STATE_UNAVAILABLE or reconciliation_succeeded != 0 or marker_matches != 0:
            return 1
        output[] = REPAIR_SAVE_DIRTY_MARKER if database_files_exist == 1 else REPAIR_NOOP
        return 0

    if progress == 2:
        if (
            (initial_state != STATE_STALE and initial_state != STATE_MISSING)
            or database_files_exist != 0
            or marker_matches != 0
        ):
            return 1
        if reconciliation_succeeded == 1 and verified_state == STATE_PRESENT:
            output[] = REPAIR_CHECK_DIRTY_MARKER
        else:
            output[] = REPAIR_SAVE_DIRTY_MARKER
        return 0

    if (
        (initial_state != STATE_PRESENT and initial_state != STATE_STALE and initial_state != STATE_MISSING)
        or database_files_exist != 0
        or (initial_state == STATE_PRESENT and reconciliation_succeeded != 0)
        or (initial_state != STATE_PRESENT and (reconciliation_succeeded != 1 or verified_state != STATE_PRESENT))
    ):
        return 1
    output[] = REPAIR_CLEAR_DIRTY_MARKER if marker_matches == 1 else REPAIR_NOOP
    return 0
