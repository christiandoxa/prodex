# Gemini value and input-text collection from a Serde-decoded JSON tree.
# The host owns acquisition and PathBuf; compiled Mojo alone decides which
# values are eligible and the depth-first traversal / output order.
from std.memory import Pointer
from rich_text import rich_trim_bounds, rich_view_matches_literal, rich_view_ptr, rich_view_valid
from parsed_json import (
    JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_NUMBER, ParsedJson, ParsedJsonNode,
    pj_child, pj_field, pj_kind, pj_next, pj_text, pj_trim, pj_valid, pj_literal,
)
from json_sink import JsonSink, js_byte, js_string, js_literal, js_raw_view
from rich_types import ProdexRichStringView

comptime GEMINI_VALUE_ABI: Int64 = 1
comptime GEMINI_VALUE_STRINGS: Int64 = 0
comptime GEMINI_VALUE_PATHS: Int64 = 1
comptime GEMINI_VALUE_INPUT_TEXT: Int64 = 2
comptime GEMINI_VALUE_STREAM_ERROR: Int64 = 3


def gemini_value_emit(
    sink: Pointer[mut=True, JsonSink, _], text: ProdexRichStringView,
    emitted: Pointer[mut=True, Bool, _],
):
    if emitted[]:
        js_byte(sink, 44)
    js_string(sink, text)
    emitted[] = True


def gemini_value_collect(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64,
):
    js_byte(sink, 91)
    var root = pj_field(tree, 0, StringSlice("value"))
    if root >= 0:
        var emitted = False
        var cursor = root
        while cursor >= 0:
            var kind = pj_kind(tree, cursor)
            var child: Int64 = -1
            if kind == JSON_STRING:
                var text = pj_text(tree, cursor)
                if operation == GEMINI_VALUE_PATHS:
                    text = pj_trim(text)
                if operation != GEMINI_VALUE_PATHS or text.len > 0:
                    gemini_value_emit(sink, text, Pointer(to=emitted))
            elif kind == JSON_ARRAY:
                child = pj_child(tree, cursor)
            elif kind == JSON_OBJECT and operation == GEMINI_VALUE_INPUT_TEXT:
                var text_field = pj_field(tree, cursor, StringSlice("text"))
                if text_field < 0:
                    text_field = pj_field(tree, cursor, StringSlice("content"))
                if pj_kind(tree, text_field) == JSON_STRING:
                    gemini_value_emit(
                        sink, pj_text(tree, text_field), Pointer(to=emitted),
                    )
                child = pj_field(tree, cursor, StringSlice("content"))
            if child >= 0:
                cursor = child
                continue
            # Arrays descend in index order; objects follow only `content`.
            # Walk parent links rather than creating a Rust or Mojo recursive stack.
            var advanced = False
            while cursor != root:
                var parent = tree.nodes[unsafe_offset=cursor].parent
                var sibling = pj_next(tree, cursor)
                if pj_kind(tree, parent) == JSON_ARRAY and sibling >= 0:
                    cursor = sibling
                    advanced = True
                    break
                cursor = parent
            if not advanced:
                break
    js_byte(sink, 93)


# Serde Number::as_i64 accepts decimal integers in the signed 64-bit range,
# but not fractional/exponent values nor out-of-range unsigned integers.
# Serde has already canonicalized the lexical representation in the host.
def gemini_value_signed_integer(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var source = rich_view_ptr(view)
    var cursor: Int64 = 0
    var negative = source[unsafe_offset=0] == 45
    if negative:
        cursor = 1
    if cursor >= Int64(view.len):
        return False
    var maximum = UInt64(9223372036854775808) if negative else UInt64(9223372036854775807)
    var value = UInt64(0)
    while cursor < Int64(view.len):
        var byte = source[unsafe_offset=cursor]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > (maximum - digit) // UInt64(10):
            return False
        value = value * UInt64(10) + digit
        cursor += 1
    return True


def gemini_value_stream_error(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var error = pj_field(tree, 0, StringSlice("error"))
    if error < 0:
        js_literal(sink, StringSlice("null"))
        return
    var code = pj_field(tree, error, StringSlice("status"))
    if code < 0:
        code = pj_field(tree, error, StringSlice("code"))
    var code_text = pj_literal(StringSlice("provider_stream_error"))
    if pj_kind(tree, code) == JSON_STRING:
        code_text = pj_text(tree, code)
    elif pj_kind(tree, code) == JSON_NUMBER:
        var numeric_text = js_raw_view(tree, code)
        if gemini_value_signed_integer(numeric_text):
            code_text = numeric_text.copy()
    var message_node = pj_field(tree, error, StringSlice("message"))
    var message = pj_literal(StringSlice("Gemini stream returned an embedded provider error"))
    if pj_kind(tree, message_node) == JSON_STRING:
        message = pj_text(tree, message_node)
    js_byte(sink, 91)
    js_string(sink, code_text)
    js_byte(sink, 44)
    js_string(sink, message)
    js_byte(sink, 93)


@export("prodex_mojo_gemini_value_policy_v1")
def prodex_mojo_gemini_value_policy_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt, node_count: Int64, raw_address: UInt, raw_length: Int64,
    scratch_address: UInt, scratch_count: Int64, measuring: Int64,
    output_address: UInt, output_capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if abi != GEMINI_VALUE_ABI:
        return 4
    if (
        operation < GEMINI_VALUE_STRINGS or operation > GEMINI_VALUE_STREAM_ERROR
        or flag != 0 or node_count <= 0 or nodes_address == 0
        or nodes_address % 8 != 0 or raw_length < 0
        or (raw_length > 0 and raw_address == 0)
        or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count < node_count or metadata_address == 0
        or metadata_address % 8 != 0 or measuring < 0 or measuring > 1
        or output_capacity < 0
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ), node_count, ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ), output_capacity, 0, measuring == 1, False,
    )
    if operation == GEMINI_VALUE_STREAM_ERROR:
        gemini_value_stream_error(Pointer(to=sink), tree)
    else:
        gemini_value_collect(Pointer(to=sink), tree, operation)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0


# Scalar Gemini policy shares the same compiled owner. Rust supplies UTF-8,
# allocates a five-word output and validates returned byte ranges only.
comptime GEMINI_VALUE_SCALAR_TOOL_PARTS: Int64 = 0
comptime GEMINI_VALUE_SCALAR_SKIP_PATH: Int64 = 1


@export("prodex_mojo_gemini_value_scalar_v1")
def prodex_mojo_gemini_value_scalar_v1(
    abi: Int64, operation: Int64,
    text_address: UInt, text_length: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi != GEMINI_VALUE_ABI:
        return 4
    if (
        operation < GEMINI_VALUE_SCALAR_TOOL_PARTS
        or operation > GEMINI_VALUE_SCALAR_SKIP_PATH
        or text_length < 0 or (text_length > 0 and text_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 1
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(5):
        result[unsafe_offset=index] = 0
    if operation == GEMINI_VALUE_SCALAR_SKIP_PATH:
        result[unsafe_offset=0] = UInt64(
            rich_view_matches_literal[".git"](text, False)
            or rich_view_matches_literal["node_modules"](text, False)
            or rich_view_matches_literal["target"](text, False)
            or rich_view_matches_literal["dist"](text, False)
            or rich_view_matches_literal["build"](text, False)
            or rich_view_matches_literal["__pycache__"](text, False)
            or rich_view_matches_literal["vendor"](text, False)
        )
        return 0

    var bounds = rich_trim_bounds(text)
    var start = bounds[0]
    var end = bounds[1]
    var open: Int64 = -1
    var close: Int64 = -1
    var source = rich_view_ptr(text)
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if value == 40 and open < 0:
            open = index
        if value == 41:
            close = index
    if open < 0 or close <= open:
        return 0
    var tool = ProdexRichStringView(
        text.ptr + UInt(start), UInt(open - start)
    )
    var pattern = ProdexRichStringView(
        text.ptr + UInt(open + 1), UInt(close - open - 1)
    )
    var tool_bounds = rich_trim_bounds(tool)
    var pattern_bounds = rich_trim_bounds(pattern)
    if tool_bounds[0] == tool_bounds[1] or pattern_bounds[0] == pattern_bounds[1]:
        return 0
    result[unsafe_offset=0] = 1
    result[unsafe_offset=1] = UInt64(start + tool_bounds[0])
    result[unsafe_offset=2] = UInt64(start + tool_bounds[1])
    result[unsafe_offset=3] = UInt64(open + 1 + pattern_bounds[0])
    result[unsafe_offset=4] = UInt64(open + 1 + pattern_bounds[1])
    return 0
