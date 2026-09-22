from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr, rich_view_prefix, rich_codepoint_width
from parsed_json import pj_trim, pj_less
from json_sink import JsonSink, js_byte, js_literal, js_escaped


def tool_name_contains_pair(view: ProdexRichStringView) -> Bool:
    var ptr = rich_view_ptr(view)
    for index in range(max(Int64(view.len) - 1, 0)):
        if ptr[unsafe_offset=index] == 95 and ptr[unsafe_offset=index + 1] == 95:
            return True
    return False


def tool_namespaced_string(
    sink: Pointer[mut=True, JsonSink, _], namespace: ProdexRichStringView,
    name: ProdexRichStringView, add_mcp_prefix: Bool,
):
    var ns = pj_trim(namespace)
    if add_mcp_prefix and ns.len > 0:
        ns = ProdexRichStringView(namespace.ptr, ns.ptr + ns.len - namespace.ptr)
    var label = pj_trim(name)
    var separator_pair = (
        (add_mcp_prefix or rich_view_prefix["mcp__"](ns, False))
        and (ns.len == 0 or rich_view_ptr(ns)[unsafe_offset=Int64(ns.len) - 1] != 95)
        and (label.len == 0 or rich_view_ptr(label)[unsafe_offset=0] != 95)
        and not tool_name_contains_pair(label)
    )
    js_byte(sink, 34)
    if add_mcp_prefix:
        js_literal(sink, StringSlice("mcp__"))
    js_escaped(sink, ns)
    js_literal(sink, StringSlice("__") if separator_pair else StringSlice("--"))
    js_escaped(sink, label)
    js_byte(sink, 34)


def tool_name_alnum(byte: UInt8) -> Bool:
    return byte >= 48 and byte <= 57 or byte >= 65 and byte <= 90 or byte >= 97 and byte <= 122


def tool_name_segment_bounds(view: ProdexRichStringView) -> ProdexRichStringView:
    var ptr = rich_view_ptr(view)
    var first: Int64 = -1
    var end: Int64 = 0
    var cursor: Int64 = 0
    while cursor < Int64(view.len):
        if tool_name_alnum(ptr[unsafe_offset=cursor]):
            if first < 0:
                first = cursor
            end = cursor + 1
        cursor += rich_codepoint_width(ptr[unsafe_offset=cursor])
    if first < 0:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(view.ptr + UInt(first), UInt(end - first))


def tool_name_segment(sink: Pointer[mut=True, JsonSink, _], bounded: ProdexRichStringView):
    var ptr = rich_view_ptr(bounded)
    var cursor: Int64 = 0
    while cursor < Int64(bounded.len):
        var byte = ptr[unsafe_offset=cursor]
        js_byte(sink, byte if tool_name_alnum(byte) or byte == 95 else UInt8(95))
        cursor += rich_codepoint_width(byte)


def tool_name_swap(values: Pointer[mut=True, ProdexRichStringView, _], left: Int64, right: Int64):
    var value = values[unsafe_offset=left].copy()
    values[unsafe_offset=left] = values[unsafe_offset=right].copy()
    values[unsafe_offset=right] = value^


def tool_name_sift(values: Pointer[mut=True, ProdexRichStringView, _], initial: Int64, count: Int64):
    var root = initial
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        if child + 1 < count and pj_less(values[unsafe_offset=child], values[unsafe_offset=child + 1]):
            child += 1
        if not pj_less(values[unsafe_offset=root], values[unsafe_offset=child]):
            break
        tool_name_swap(values, root, child)
        root = child


def tool_name_sort(values: Pointer[mut=True, ProdexRichStringView, _], count: Int64):
    var root = count // 2
    while root > 0:
        root -= 1
        tool_name_sift(values, root, count)
    var end = count
    while end > 1:
        end -= 1
        tool_name_swap(values, 0, end)
        tool_name_sift(values, 0, end)
