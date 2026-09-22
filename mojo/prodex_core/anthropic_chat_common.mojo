from std.memory import Pointer

from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr
from parsed_json import (
    ParsedJson,
    JSON_NUMBER,
    JSON_STRING,
    pj_kind,
    pj_text,
    pj_field,
)
from json_sink import JsonSink, js_byte, js_view, js_literal, js_string, js_escaped
from json_view import (
    deepseek_json_byte,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)


def anthropic_chat_error(
    sink: Pointer[mut=True, JsonSink, _], message: StringSlice
):
    js_byte(sink, 69)
    js_literal(sink, message)


def anthropic_chat_error_field(
    sink: Pointer[mut=True, JsonSink, _],
    prefix: StringSlice,
    field: ProdexRichStringView,
    suffix: StringSlice,
):
    js_byte(sink, 69)
    js_literal(sink, prefix)
    js_view(sink, field)
    js_literal(sink, suffix)


def anthropic_chat_positive_u64(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) != JSON_NUMBER:
        return False
    var node = tree.nodes[unsafe_offset=index].copy()
    var view = ProdexRichStringView(
        tree.raw.ptr + UInt(node.raw_start), UInt(node.raw_length)
    )
    if view.len == 0:
        return False
    var ptr = rich_view_ptr(view)
    var value: UInt64 = 0
    for offset in range(Int64(view.len)):
        var byte = ptr[unsafe_offset=offset]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > UInt64(1844674407370955161):
            return False
        if value == UInt64(1844674407370955161) and digit > 5:
            return False
        value = value * 10 + digit
    return value > 0


def anthropic_chat_field_string(
    tree: ParsedJson,
    object: Int64,
    first: StringSlice,
    second: StringSlice,
) -> Int64:
    var value = pj_field(tree, object, first)
    if value < 0:
        value = pj_field(tree, object, second)
    return value if pj_kind(tree, value) == JSON_STRING else -1


def anthropic_chat_arguments_view(
    tree: ParsedJson, index: Int64
) -> ProdexRichStringView:
    if pj_kind(tree, index) != JSON_STRING:
        return ProdexRichStringView(0, 0)
    return pj_text(tree, index)


def anthropic_chat_arguments_object_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    if start >= Int64(view.len) or deepseek_json_byte(view, start) != 123:
        return False
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0:
        return False
    end = deepseek_json_skip_ws(view, end, Int64(view.len))
    return end == Int64(view.len)


def anthropic_chat_last_dot(view: ProdexRichStringView) -> Int64:
    if view.len < 3:
        return -1
    var ptr = rich_view_ptr(view)
    var found: Int64 = -1
    for index in range(Int64(view.len)):
        if ptr[unsafe_offset=index] == 46:
            found = index
    if found <= 0 or found >= Int64(view.len) - 1:
        return -1
    return found


def anthropic_chat_tool_name(
    sink: Pointer[mut=True, JsonSink, _],
    namespace: ProdexRichStringView,
    name: ProdexRichStringView,
):
    js_byte(sink, 34)
    if namespace.len > 0:
        js_escaped(sink, namespace)
        js_literal(sink, StringSlice("--"))
        js_escaped(sink, name)
    else:
        var dot = anthropic_chat_last_dot(name)
        if dot < 0:
            js_escaped(sink, name)
        else:
            js_escaped(sink, ProdexRichStringView(name.ptr, UInt(dot)))
            js_literal(sink, StringSlice("--"))
            js_escaped(
                sink,
                ProdexRichStringView(
                    name.ptr + UInt(dot + 1), name.len - UInt(dot + 1)
                ),
            )
    js_byte(sink, 34)


def anthropic_chat_tool_name_raw(
    namespace: ProdexRichStringView, name: ProdexRichStringView
) -> Bool:
    return namespace.len > 0 or name.len > 0
