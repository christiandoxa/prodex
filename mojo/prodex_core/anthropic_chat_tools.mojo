from std.memory import Pointer

from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson,
    JSON_STRING,
    JSON_ARRAY,
    JSON_OBJECT,
    pj_kind,
    pj_child,
    pj_next,
    pj_text,
    pj_field,
    pj_string_field,
    pj_is,
    pj_equal,
    pj_literal,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from anthropic_chat_common import (
    anthropic_chat_error,
    anthropic_chat_error_field,
    anthropic_chat_positive_u64,
    anthropic_chat_tool_name,
)


def anthropic_chat_tool_function(tree: ParsedJson, tool: Int64) -> Int64:
    var function = pj_field(tree, tool, StringSlice("function"))
    return function if pj_kind(tree, function) == JSON_OBJECT else tool


def anthropic_chat_validate_tools(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, tools: Int64
) -> Bool:
    if tools < 0:
        return True
    if pj_kind(tree, tools) != JSON_ARRAY:
        anthropic_chat_error(sink, StringSlice("Responses `tools` must be an array"))
        return False
    var tool = pj_child(tree, tools)
    while tool >= 0:
        if pj_kind(tree, tool) != JSON_OBJECT:
            anthropic_chat_error(
                sink, StringSlice("Responses function tool must be an object")
            )
            return False
        var function = anthropic_chat_tool_function(tree, tool)
        if pj_string_field(tree, function, StringSlice("name")) < 0:
            anthropic_chat_error(
                sink, StringSlice("Responses function tool must contain name")
            )
            return False
        tool = pj_next(tree, tool)
    return True


def anthropic_chat_write_tool(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, tool: Int64
):
    var function = anthropic_chat_tool_function(tree, tool)
    var name = pj_string_field(tree, function, StringSlice("name"))
    var namespace = pj_string_field(tree, function, StringSlice("namespace"))
    var parameters = pj_field(tree, function, StringSlice("parameters"))
    var description = pj_field(tree, function, StringSlice("description"))
    js_byte(sink, 123)
    var fields: Int64 = 0
    if description >= 0:
        js_literal(sink, StringSlice('"description":'))
        js_raw(sink, tree, description)
        fields += 1
    if fields > 0:
        js_byte(sink, 44)
    js_literal(sink, StringSlice('"input_schema":'))
    if parameters >= 0:
        js_raw(sink, tree, parameters)
    else:
        js_literal(sink, StringSlice('{"properties":{},"type":"object"}'))
    js_literal(sink, StringSlice(',"name":'))
    anthropic_chat_tool_name(
        sink,
        pj_text(tree, namespace) if namespace >= 0 else ProdexRichStringView(0, 0),
        pj_text(tree, name),
    )
    js_byte(sink, 125)


def anthropic_chat_web_key_allowed(key: ProdexRichStringView) -> Bool:
    return (
        pj_equal(key, pj_literal(StringSlice("search_context_size")))
        or pj_equal(key, pj_literal(StringSlice("allowed_domains")))
        or pj_equal(key, pj_literal(StringSlice("blocked_domains")))
        or pj_equal(key, pj_literal(StringSlice("user_location")))
        or pj_equal(key, pj_literal(StringSlice("max_uses")))
    )


def anthropic_chat_validate_domains(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    options: Int64,
    field: StringSlice,
) -> Bool:
    var domains = pj_field(tree, options, field)
    if domains < 0:
        return True
    if pj_kind(tree, domains) != JSON_ARRAY:
        anthropic_chat_error_field(
            sink,
            StringSlice("Anthropic web search "),
            pj_literal(field),
            StringSlice(" must be an array"),
        )
        return False
    var domain = pj_child(tree, domains)
    while domain >= 0:
        if pj_kind(tree, domain) != JSON_STRING or pj_text(tree, domain).len == 0:
            anthropic_chat_error_field(
                sink,
                StringSlice("Anthropic web search "),
                pj_literal(field),
                StringSlice(" entries must be non-empty strings"),
            )
            return False
        domain = pj_next(tree, domain)
    return True


def anthropic_chat_web_degradation(
    tree: ParsedJson, options: Int64
) -> Int64:
    var size = pj_string_field(tree, options, StringSlice("search_context_size"))
    if pj_is["low"](tree, size):
        return 1
    if pj_is["medium"](tree, size):
        return 2
    if pj_is["high"](tree, size):
        return 3
    return 0


def anthropic_chat_validate_web_search(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, options: Int64
) -> Bool:
    if options < 0:
        return True
    if pj_kind(tree, options) != JSON_OBJECT:
        anthropic_chat_error(
            sink, StringSlice("web_search_options must be an object")
        )
        return False
    var child = pj_child(tree, options)
    while child >= 0:
        if not anthropic_chat_web_key_allowed(
            tree.nodes[unsafe_offset=child].key.copy()
        ):
            anthropic_chat_error_field(
                sink,
                StringSlice("Anthropic Messages does not translate web_search_options field `"),
                tree.nodes[unsafe_offset=child].key.copy(),
                StringSlice("`"),
            )
            return False
        child = pj_next(tree, child)
    if (
        pj_field(tree, options, StringSlice("allowed_domains")) >= 0
        and pj_field(tree, options, StringSlice("blocked_domains")) >= 0
    ):
        anthropic_chat_error(
            sink,
            StringSlice(
                "Anthropic web search cannot combine allowed_domains and blocked_domains"
            ),
        )
        return False
    if not anthropic_chat_validate_domains(
        sink, tree, options, StringSlice("allowed_domains")
    ):
        return False
    if not anthropic_chat_validate_domains(
        sink, tree, options, StringSlice("blocked_domains")
    ):
        return False
    var max_uses = pj_field(tree, options, StringSlice("max_uses"))
    if max_uses >= 0 and not anthropic_chat_positive_u64(tree, max_uses):
        anthropic_chat_error(
            sink, StringSlice("Anthropic web search max_uses must be a positive integer")
        )
        return False
    var location = pj_field(tree, options, StringSlice("user_location"))
    if location >= 0 and pj_kind(tree, location) != JSON_OBJECT:
        anthropic_chat_error(
            sink, StringSlice("Anthropic web search user_location must be an object")
        )
        return False
    var size = pj_field(tree, options, StringSlice("search_context_size"))
    if size >= 0 and not (
        pj_is["low"](tree, size)
        or pj_is["medium"](tree, size)
        or pj_is["high"](tree, size)
    ):
        anthropic_chat_error(
            sink,
            StringSlice(
                "Anthropic web search search_context_size must be low, medium, or high"
            ),
        )
        return False
    return True


def anthropic_chat_write_web_search(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, options: Int64
):
    js_literal(
        sink,
        StringSlice('{"name":"web_search","type":"web_search_20250305"'),
    )
    for field in range(4):
        var index: Int64
        if field == 0:
            index = pj_field(tree, options, StringSlice("allowed_domains"))
            if index >= 0:
                js_literal(sink, StringSlice(',"allowed_domains":'))
        elif field == 1:
            index = pj_field(tree, options, StringSlice("blocked_domains"))
            if index >= 0:
                js_literal(sink, StringSlice(',"blocked_domains":'))
        elif field == 2:
            index = pj_field(tree, options, StringSlice("user_location"))
            if index >= 0:
                js_literal(sink, StringSlice(',"user_location":'))
        else:
            index = pj_field(tree, options, StringSlice("max_uses"))
            if index >= 0:
                js_literal(sink, StringSlice(',"max_uses":'))
        if index >= 0:
            js_raw(sink, tree, index)
    js_byte(sink, 125)


def anthropic_chat_tool_choice_none(tree: ParsedJson, choice: Int64) -> Bool:
    return pj_is["none"](tree, choice)


def anthropic_chat_validate_tool_choice(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, choice: Int64
) -> Bool:
    if choice < 0:
        return True
    if pj_kind(tree, choice) == JSON_STRING:
        if (
            pj_is["auto"](tree, choice)
            or pj_is["required"](tree, choice)
            or pj_is["none"](tree, choice)
        ):
            return True
        anthropic_chat_error_field(
            sink,
            StringSlice("unsupported Responses tool_choice `"),
            pj_text(tree, choice),
            StringSlice("`"),
        )
        return False
    if pj_kind(tree, choice) != JSON_OBJECT:
        anthropic_chat_error(
            sink, StringSlice("unsupported Responses tool_choice shape")
        )
        return False
    if not pj_is["function"](
        tree, pj_field(tree, choice, StringSlice("type"))
    ):
        anthropic_chat_error(
            sink, StringSlice("unsupported Responses tool_choice shape")
        )
        return False
    var name = pj_field(tree, choice, StringSlice("name"))
    if name < 0:
        var function = pj_field(tree, choice, StringSlice("function"))
        name = pj_field(tree, function, StringSlice("name"))
    if pj_kind(tree, name) != JSON_STRING:
        anthropic_chat_error(
            sink, StringSlice("function tool_choice must contain name")
        )
        return False
    return True


def anthropic_chat_write_tool_choice(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, choice: Int64
):
    if pj_is["auto"](tree, choice):
        js_literal(sink, StringSlice('{"type":"auto"}'))
        return
    if pj_is["required"](tree, choice):
        js_literal(sink, StringSlice('{"type":"any"}'))
        return
    var name = pj_field(tree, choice, StringSlice("name"))
    if name < 0:
        name = pj_field(
            tree, pj_field(tree, choice, StringSlice("function")), StringSlice("name")
        )
    var namespace = pj_string_field(tree, choice, StringSlice("namespace"))
    js_literal(sink, StringSlice('{"name":'))
    anthropic_chat_tool_name(
        sink,
        pj_text(tree, namespace) if namespace >= 0 else ProdexRichStringView(0, 0),
        pj_text(tree, name),
    )
    js_literal(sink, StringSlice(',"type":"tool"}'))


def anthropic_chat_tools_present(
    tree: ParsedJson, tools: Int64, web_search: Int64, choice: Int64
) -> Bool:
    if anthropic_chat_tool_choice_none(tree, choice):
        return False
    if pj_kind(tree, tools) == JSON_ARRAY and pj_child(tree, tools) >= 0:
        return True
    return web_search >= 0


def anthropic_chat_write_tools(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    tools: Int64,
    web_search: Int64,
):
    js_byte(sink, 91)
    var wrote = False
    if pj_kind(tree, tools) == JSON_ARRAY:
        var tool = pj_child(tree, tools)
        while tool >= 0:
            if wrote:
                js_byte(sink, 44)
            anthropic_chat_write_tool(sink, tree, tool)
            wrote = True
            tool = pj_next(tree, tool)
    if web_search >= 0:
        if wrote:
            js_byte(sink, 44)
        anthropic_chat_write_web_search(sink, tree, web_search)
    js_byte(sink, 93)
