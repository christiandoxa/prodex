from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_prefix
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_STRING, JSON_ARRAY, JSON_OBJECT,
    pj_kind, pj_child, pj_next, pj_text, pj_field, pj_string_field, pj_is,
    pj_nonblank, pj_valid, pj_literal,
)
from json_sink import JsonSink, js_byte, js_string, js_raw, js_literal
from chat_tool_shapes import tool_function, tool_namespace, tool_search, tool_mcp
from chat_tool_names import tool_namespaced_string


def chat_web_search_tool(tree: ParsedJson, node: Int64) -> Bool:
    var kind = pj_field(tree, node, StringSlice("type"))
    return pj_is["web_search"](tree, kind) or pj_is["web_search_preview"](tree, kind) or rich_view_prefix["web_search_preview_"](pj_text(tree, kind), False)


def chat_tools(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, scratch: Pointer[mut=True, ProdexRichStringView, _]) -> Bool:
    var tools = pj_field(tree, 0, StringSlice("tools"))
    if pj_kind(tree, tools) != JSON_ARRAY:
        return False
    var count: Int64 = 0
    js_byte(sink, 91)
    var tool = pj_child(tree, tools)
    while tool >= 0:
        if pj_kind(tree, tool) == JSON_OBJECT and not chat_web_search_tool(tree, tool):
            tool_function(sink, tree, tool, Pointer(to=count))
            tool_namespace(sink, tree, tool, Pointer(to=count))
            tool_search(sink, tree, tool, Pointer(to=count))
            tool_mcp(sink, tree, tool, Pointer(to=count), scratch)
        tool = pj_next(tree, tool)
    js_byte(sink, 93)
    return count > 0


def chat_choice_namespace(tree: ParsedJson, object: Int64) -> Int64:
    var namespace = pj_string_field(tree, object, StringSlice("namespace"))
    if namespace < 0:
        namespace = pj_string_field(tree, object, StringSlice("server_label"))
    if namespace < 0:
        namespace = pj_string_field(tree, object, StringSlice("mcp_server_name"))
    if namespace < 0:
        namespace = pj_string_field(tree, object, StringSlice("server_name"))
    if namespace < 0:
        namespace = pj_string_field(tree, pj_field(tree, object, StringSlice("function")), StringSlice("namespace"))
    return namespace


def chat_choice(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, thinking: Bool) -> Bool:
    if thinking:
        return False
    var choice = pj_field(tree, 0, StringSlice("tool_choice"))
    if pj_kind(tree, choice) == JSON_STRING:
        if pj_is["auto"](tree, choice) or pj_is["none"](tree, choice) or pj_is["required"](tree, choice):
            js_string(sink, pj_text(tree, choice))
            return True
        return False
    var kind = pj_field(tree, choice, StringSlice("type"))
    var mcp = rich_view_prefix["mcp"](pj_text(tree, kind), False)
    if not mcp and not pj_is["function"](tree, kind):
        return False
    var name = pj_string_field(tree, choice, StringSlice("name"))
    if name < 0:
        name = pj_string_field(tree, pj_field(tree, choice, StringSlice("function")), StringSlice("name"))
    if not pj_nonblank(tree, name):
        return False
    var namespace = chat_choice_namespace(tree, choice)
    js_literal(sink, StringSlice('{"type":"function","function":{"name":'))
    if pj_nonblank(tree, namespace):
        var ns = pj_text(tree, namespace)
        tool_namespaced_string(sink, ns, pj_text(tree, name), mcp and not rich_view_prefix["mcp__"](ns, False))
    else:
        js_string(sink, pj_text(tree, name))
    js_literal(sink, StringSlice("}}"))
    return True


def chat_web_options(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson) -> Bool:
    var tools = pj_field(tree, 0, StringSlice("tools"))
    if pj_kind(tree, tools) != JSON_ARRAY:
        return False
    var tool = pj_child(tree, tools)
    while tool >= 0 and not chat_web_search_tool(tree, tool):
        tool = pj_next(tree, tool)
    if tool < 0:
        return False
    js_byte(sink, 123)
    var written: Int64 = 0
    var size = pj_string_field(tree, tool, StringSlice("search_context_size"))
    if size < 0:
        size = pj_string_field(tree, tool, StringSlice("context_size"))
    if pj_is["low"](tree, size) or pj_is["medium"](tree, size) or pj_is["high"](tree, size):
        js_literal(sink, StringSlice('"search_context_size":'))
        js_string(sink, pj_text(tree, size))
        written += 1
    for field in range(4):
        var name = StringSlice("allowed_domains")
        if field == 1:
            name = StringSlice("blocked_domains")
        elif field == 2:
            name = StringSlice("max_uses")
        elif field == 3:
            name = StringSlice("user_location")
        var value = pj_field(tree, tool, name)
        if field == 3 and value < 0:
            value = pj_field(tree, tool, StringSlice("location"))
        if value >= 0:
            if written > 0:
                js_byte(sink, 44)
            js_string(sink, pj_literal(name))
            js_byte(sink, 58)
            js_raw(sink, tree, value)
            written += 1
    js_byte(sink, 125)
    return True


def chat_without_web_options(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson) -> Bool:
    var removed = pj_field(tree, 0, StringSlice("web_search_options"))
    if removed < 0:
        return False
    js_byte(sink, 123)
    var child = pj_child(tree, 0)
    var written: Int64 = 0
    while child >= 0:
        if child != removed:
            if written > 0:
                js_byte(sink, 44)
            js_string(sink, tree.nodes[unsafe_offset=child].key)
            js_byte(sink, 58)
            js_raw(sink, tree, child)
            written += 1
        child = pj_next(tree, child)
    js_byte(sink, 125)
    return True


@export("prodex_mojo_chat_tools_v1")
def prodex_mojo_chat_tools_v1(
    abi: Int64, operation: Int64, thinking: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 4 or thinking < 0 or thinking > 1 or measuring < 0 or measuring > 1 or raw_length < 0:
        return 1
    if capacity < 0 or metadata_address == 0 or nodes_address == 0 or scratch_address == 0 or scratch_count < nodes_count:
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var scratch = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var present = False
    if operation == 0:
        present = chat_tools(Pointer(to=sink), tree, scratch)
    elif operation == 1:
        present = chat_choice(Pointer(to=sink), tree, thinking == 1)
    elif operation == 2:
        present = chat_web_options(Pointer(to=sink), tree)
    elif operation == 3:
        present = chat_without_web_options(Pointer(to=sink), tree)
    elif operation == 4:
        var namespace = pj_child(tree, 0)
        var name = pj_next(tree, namespace)
        if pj_kind(tree, 0) == JSON_ARRAY and pj_kind(tree, namespace) == JSON_STRING and pj_kind(tree, name) == JSON_STRING:
            tool_namespaced_string(Pointer(to=sink), pj_text(tree, namespace), pj_text(tree, name), False)
            present = True
    if sink.failed:
        return 3
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    meta[unsafe_offset=0] = Int64(present)
    meta[unsafe_offset=1] = sink.written
    return 0
