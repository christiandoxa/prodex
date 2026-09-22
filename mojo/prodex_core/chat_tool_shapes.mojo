from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_prefix
from parsed_json import (
    ParsedJson, JSON_FALSE, JSON_TRUE, JSON_STRING, JSON_ARRAY, JSON_OBJECT,
    pj_kind, pj_text, pj_child, pj_next, pj_field, pj_string_field, pj_is,
    pj_nonblank, pj_trim, pj_literal, pj_equal,
)
from json_sink import JsonSink, js_byte, js_literal, js_escaped, js_string, js_raw, js_raw_view
from chat_tool_names import tool_namespaced_string, tool_name_segment_bounds, tool_name_segment, tool_name_sort


def tool_begin(sink: Pointer[mut=True, JsonSink, _], count: Pointer[mut=True, Int64, _]):
    if count[] > 0:
        js_byte(sink, 44)
    count[] += 1
    js_literal(sink, StringSlice('{"type":"function","function":{"name":'))


def tool_description(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64):
    var description = pj_string_field(tree, object, StringSlice("description"))
    if pj_nonblank(tree, description):
        js_literal(sink, StringSlice(',"description":'))
        js_string(sink, pj_text(tree, description))


def tool_parameters(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64):
    var parameters = pj_field(tree, object, StringSlice("parameters"))
    if parameters < 0:
        parameters = pj_field(tree, object, StringSlice("parametersJsonSchema"))
    if parameters < 0:
        parameters = pj_field(tree, object, StringSlice("input_schema"))
    if parameters < 0:
        parameters = pj_field(tree, object, StringSlice("schema"))
    if parameters >= 0:
        js_literal(sink, StringSlice(',"parameters":'))
        js_raw(sink, tree, parameters)
    var strict = pj_field(tree, object, StringSlice("strict"))
    if pj_kind(tree, strict) == JSON_FALSE or pj_kind(tree, strict) == JSON_TRUE:
        js_literal(sink, StringSlice(',"strict":'))
        js_raw(sink, tree, strict)


def tool_custom(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64,
    count: Pointer[mut=True, Int64, _],
) -> Bool:
    if not pj_is["custom"](tree, pj_field(tree, object, StringSlice("type"))):
        return False
    var name = pj_string_field(tree, object, StringSlice("name"))
    if not pj_nonblank(tree, name):
        return False
    var patch = pj_is["apply_patch"](tree, name)
    tool_begin(sink, count)
    js_string(sink, pj_text(tree, name))
    js_literal(sink, StringSlice(',"description":"'))
    var description = pj_string_field(tree, object, StringSlice("description"))
    if pj_nonblank(tree, description):
        js_escaped(sink, pj_text(tree, description))
    else:
        js_literal(sink, StringSlice("Freeform custom tool input."))
    js_literal(sink, StringSlice("\\n\\n"))
    if patch:
        js_escaped(sink, pj_literal(StringSlice("Call this custom/freeform tool with the exact Codex apply_patch grammar in the `input` string field. The first line must be `*** Begin Patch`, the last line must be `*** End Patch`, and hunks must use `*** Add File:`, `*** Delete File:`, or `*** Update File:`. For `*** Add File: path`, every new file content line must start with `+`; for example `+hello`, and a blank content line is `+`. Do not pass unified diff headers such as `--- a/...` or `+++ b/...` as the top-level input.")))
    else:
        js_literal(sink, StringSlice("Call this custom/freeform tool with the exact raw tool input in the `input` string field."))
    var format = pj_field(tree, object, StringSlice("format"))
    if format >= 0:
        js_literal(sink, StringSlice("\\n\\nOriginal custom tool format JSON: "))
        js_escaped(sink, js_raw_view(tree, format))
    js_literal(sink, StringSlice('","parameters":{"type":"object","properties":{"input":{"type":"string","description":'))
    if patch:
        js_string(sink, pj_literal(StringSlice("Exact raw apply_patch input. For Add File, prefix every new file content line with '+'.")))
    else:
        js_string(sink, pj_literal(StringSlice("Exact raw input for the custom/freeform tool.")))
    js_literal(sink, StringSlice('}},"required":["input"],"additionalProperties":false}}}'))
    return True


def tool_function(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64,
    count: Pointer[mut=True, Int64, _],
):
    if tool_custom(sink, tree, object, count):
        return
    var function = object
    var kind = pj_field(tree, object, StringSlice("type"))
    if pj_is["function"](tree, kind):
        var nested = pj_field(tree, object, StringSlice("function"))
        if pj_kind(tree, nested) == JSON_OBJECT:
            function = nested
    else:
        var name = pj_string_field(tree, object, StringSlice("name"))
        if name < 0:
            return
        var has_schema = (
            pj_field(tree, object, StringSlice("parameters")) >= 0
            or pj_field(tree, object, StringSlice("parametersJsonSchema")) >= 0
            or pj_field(tree, object, StringSlice("input_schema")) >= 0
            or pj_field(tree, object, StringSlice("schema")) >= 0
        )
        if not has_schema or not (rich_view_prefix["mcp"](pj_text(tree, kind), False) or rich_view_prefix["mcp__"](pj_text(tree, name), False)):
            return
    var name = pj_string_field(tree, function, StringSlice("name"))
    if not pj_nonblank(tree, name):
        return
    tool_begin(sink, count)
    js_string(sink, pj_text(tree, name))
    tool_description(sink, tree, function)
    tool_parameters(sink, tree, function)
    js_literal(sink, StringSlice("}}"))


def tool_namespace(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64,
    count: Pointer[mut=True, Int64, _],
):
    if not pj_is["namespace"](tree, pj_field(tree, object, StringSlice("type"))):
        return
    var namespace = pj_string_field(tree, object, StringSlice("name"))
    if not pj_nonblank(tree, namespace):
        return
    var tools = pj_field(tree, object, StringSlice("tools"))
    if pj_kind(tree, tools) != JSON_ARRAY:
        return
    var tool = pj_child(tree, tools)
    while tool >= 0:
        var name = pj_string_field(tree, tool, StringSlice("name"))
        if pj_is["function"](tree, pj_field(tree, tool, StringSlice("type"))) and pj_nonblank(tree, name):
            tool_begin(sink, count)
            tool_namespaced_string(sink, pj_text(tree, namespace), pj_text(tree, name), False)
            tool_description(sink, tree, tool)
            tool_parameters(sink, tree, tool)
            js_literal(sink, StringSlice("}}"))
        tool = pj_next(tree, tool)


def tool_search(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64,
    count: Pointer[mut=True, Int64, _],
):
    if not pj_is["tool_search"](tree, pj_field(tree, object, StringSlice("type"))):
        return
    tool_begin(sink, count)
    js_literal(sink, StringSlice('"tool_search"'))
    tool_description(sink, tree, object)
    var parameters = pj_field(tree, object, StringSlice("parameters"))
    if parameters >= 0:
        js_literal(sink, StringSlice(',"parameters":'))
        js_raw(sink, tree, parameters)
    js_literal(sink, StringSlice("}}"))


def tool_server_name(tree: ParsedJson, object: Int64) -> Int64:
    var name = pj_string_field(tree, object, StringSlice("mcp_server_name"))
    if name < 0:
        name = pj_string_field(tree, object, StringSlice("server_label"))
    if name < 0:
        name = pj_string_field(tree, object, StringSlice("server_name"))
    if name < 0:
        name = pj_string_field(tree, object, StringSlice("name"))
    return name


def tool_mcp_names(
    tree: ParsedJson, object: Int64, names: Pointer[mut=True, ProdexRichStringView, _],
) -> Int64:
    var count: Int64 = 0
    var allowed = pj_field(tree, object, StringSlice("allowed_tools"))
    if pj_kind(tree, allowed) == JSON_ARRAY:
        var item = pj_child(tree, allowed)
        while item >= 0:
            var name = pj_trim(pj_text(tree, item))
            if name.len > 0:
                names[unsafe_offset=count] = name^
                count += 1
            item = pj_next(tree, item)
    var default_config = pj_field(tree, object, StringSlice("default_config"))
    var default_enabled = pj_kind(tree, pj_field(tree, default_config, StringSlice("enabled"))) != JSON_FALSE
    var configs = pj_field(tree, object, StringSlice("configs"))
    if pj_kind(tree, configs) == JSON_OBJECT:
        var config = pj_child(tree, configs)
        while config >= 0:
            var enabled = pj_kind(tree, pj_field(tree, config, StringSlice("enabled")))
            if enabled == JSON_TRUE or enabled != JSON_FALSE and default_enabled:
                names[unsafe_offset=count] = tree.nodes[unsafe_offset=config].key.copy()
                count += 1
            config = pj_next(tree, config)
    tool_name_sort(names, count)
    return count


def tool_mcp(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, object: Int64,
    count: Pointer[mut=True, Int64, _], scratch: Pointer[mut=True, ProdexRichStringView, _],
):
    var kind = pj_field(tree, object, StringSlice("type"))
    if not pj_is["mcp"](tree, kind) and not pj_is["mcp_toolset"](tree, kind):
        return
    var server = tool_server_name(tree, object)
    if not pj_nonblank(tree, server):
        return
    var server_segment = tool_name_segment_bounds(pj_text(tree, server))
    if server_segment.len == 0:
        return
    var names_count = tool_mcp_names(tree, object, scratch)
    for index in range(names_count):
        var name = scratch[unsafe_offset=index].copy()
        if index > 0 and pj_equal(name, scratch[unsafe_offset=index - 1]):
            continue
        var segment = tool_name_segment_bounds(name)
        if segment.len == 0:
            continue
        tool_begin(sink, count)
        js_byte(sink, 34)
        if not rich_view_prefix["mcp__"](pj_trim(name), False):
            js_literal(sink, StringSlice("mcp__"))
            tool_name_segment(sink, server_segment)
            js_literal(sink, StringSlice("__"))
        tool_name_segment(sink, segment)
        js_literal(sink, StringSlice('","description":'))
        var description = pj_string_field(tree, object, StringSlice("description"))
        if pj_nonblank(tree, description):
            js_string(sink, pj_text(tree, description))
        else:
            js_literal(sink, StringSlice('"MCP tool '))
            js_escaped(sink, name)
            js_literal(sink, StringSlice(" from "))
            js_escaped(sink, pj_text(tree, server))
            js_literal(sink, StringSlice('."'))
        js_literal(sink, StringSlice(',"parameters":{"type":"object","additionalProperties":true}}}'))
