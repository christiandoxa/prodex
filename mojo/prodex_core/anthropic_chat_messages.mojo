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
    pj_nonblank,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_escaped, js_view
from json_view import deepseek_json_skip_ws, deepseek_json_value_end, deepseek_json_byte
from anthropic_chat_common import (
    anthropic_chat_error,
    anthropic_chat_arguments_view,
    anthropic_chat_arguments_object_valid,
    anthropic_chat_tool_name,
)


def anthropic_chat_source_role(tree: ParsedJson, message: Int64) -> Int64:
    var role = pj_string_field(tree, message, StringSlice("role"))
    if pj_is["system"](tree, role) or pj_is["developer"](tree, role):
        return -1
    if pj_is["assistant"](tree, role):
        return 1
    return 0


def anthropic_chat_is_tool_message(tree: ParsedJson, message: Int64) -> Bool:
    return pj_is["tool"](
        tree, pj_string_field(tree, message, StringSlice("role"))
    )


def anthropic_chat_message_has_blocks(tree: ParsedJson, message: Int64) -> Bool:
    if anthropic_chat_source_role(tree, message) < 0:
        return False
    if anthropic_chat_is_tool_message(tree, message):
        return True
    var content = pj_string_field(tree, message, StringSlice("content"))
    if content >= 0 and pj_text(tree, content).len > 0:
        return True
    var calls = pj_field(tree, message, StringSlice("tool_calls"))
    return pj_kind(tree, calls) == JSON_ARRAY and pj_child(tree, calls) >= 0


def anthropic_chat_validate_tool_call(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, call: Int64
) -> Bool:
    var function = pj_field(tree, call, StringSlice("function"))
    if pj_kind(tree, function) != JSON_OBJECT:
        anthropic_chat_error(sink, StringSlice("function call must contain function"))
        return False
    var name = pj_string_field(tree, function, StringSlice("name"))
    if name < 0:
        anthropic_chat_error(sink, StringSlice("function call must contain name"))
        return False
    var arguments = pj_field(tree, function, StringSlice("arguments"))
    if arguments < 0 or pj_kind(tree, arguments) != JSON_STRING:
        return True
    var view = anthropic_chat_arguments_view(tree, arguments)
    if not anthropic_chat_arguments_object_valid(view):
        # Preserve the historical distinction between malformed JSON and a
        # valid non-object JSON value.
        var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
        var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
        if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len):
            anthropic_chat_error(
                sink, StringSlice("function call arguments must be valid JSON")
            )
        elif start >= Int64(view.len) or deepseek_json_byte(view, start) != 123:
            anthropic_chat_error(
                sink, StringSlice("function call arguments must be a JSON object")
            )
        else:
            anthropic_chat_error(
                sink, StringSlice("function call arguments must be valid JSON")
            )
        return False
    return True


def anthropic_chat_validate_messages(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, messages: Int64
) -> Bool:
    if pj_kind(tree, messages) != JSON_ARRAY:
        anthropic_chat_error(
            sink, StringSlice("translated Responses request must contain messages")
        )
        return False
    var translated: Int64 = 0
    var message = pj_child(tree, messages)
    while message >= 0:
        if pj_kind(tree, message) != JSON_OBJECT:
            anthropic_chat_error(sink, StringSlice("translated message must be an object"))
            return False
        var calls = pj_field(tree, message, StringSlice("tool_calls"))
        if pj_kind(tree, calls) == JSON_ARRAY:
            var call = pj_child(tree, calls)
            while call >= 0:
                if not anthropic_chat_validate_tool_call(sink, tree, call):
                    return False
                call = pj_next(tree, call)
        if anthropic_chat_message_has_blocks(tree, message):
            translated += 1
        message = pj_next(tree, message)
    if translated == 0:
        anthropic_chat_error(
            sink,
            StringSlice("Responses request must contain at least one user or assistant message"),
        )
        return False
    return True


def anthropic_chat_system_will_render(tree: ParsedJson, messages: Int64) -> Bool:
    var count: Int64 = 0
    var any_bytes = False
    var message = pj_child(tree, messages)
    while message >= 0:
        if anthropic_chat_source_role(tree, message) == -1:
            var content = pj_string_field(tree, message, StringSlice("content"))
            if content >= 0:
                count += 1
                if pj_text(tree, content).len > 0:
                    any_bytes = True
        message = pj_next(tree, message)
    return any_bytes or count > 1


def anthropic_chat_write_system(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, messages: Int64
):
    js_byte(sink, 34)
    var wrote = False
    var message = pj_child(tree, messages)
    while message >= 0:
        if anthropic_chat_source_role(tree, message) == -1:
            var content = pj_string_field(tree, message, StringSlice("content"))
            if content >= 0:
                if wrote:
                    js_literal(sink, StringSlice("\\n\\n"))
                js_escaped(sink, pj_text(tree, content))
                wrote = True
        message = pj_next(tree, message)
    js_byte(sink, 34)


def anthropic_chat_write_tool_result(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, message: Int64
):
    var call_id = pj_string_field(tree, message, StringSlice("tool_call_id"))
    var content = pj_string_field(tree, message, StringSlice("content"))
    js_literal(sink, StringSlice('{"content":'))
    if content >= 0:
        js_string(sink, pj_text(tree, content))
    else:
        js_literal(sink, StringSlice('""'))
    js_literal(sink, StringSlice(',"tool_use_id":'))
    if call_id >= 0:
        js_string(sink, pj_text(tree, call_id))
    else:
        js_literal(sink, StringSlice('"call_prodex"'))
    js_literal(sink, StringSlice(',"type":"tool_result"}'))


def anthropic_chat_write_tool_use(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    message: Int64,
    call: Int64,
):
    var function = pj_field(tree, call, StringSlice("function"))
    var name = pj_string_field(tree, function, StringSlice("name"))
    var namespace = pj_string_field(tree, message, StringSlice("namespace"))
    var call_id = pj_string_field(tree, call, StringSlice("id"))
    var arguments = pj_string_field(tree, function, StringSlice("arguments"))
    js_literal(sink, StringSlice('{"id":'))
    if call_id >= 0:
        js_string(sink, pj_text(tree, call_id))
    else:
        js_literal(sink, StringSlice('"call_prodex"'))
    js_literal(sink, StringSlice(',"input":'))
    if arguments >= 0:
        js_view(sink, pj_text(tree, arguments))
    else:
        js_literal(sink, StringSlice("{}"))
    js_literal(sink, StringSlice(',"name":'))
    anthropic_chat_tool_name(
        sink,
        pj_text(tree, namespace) if namespace >= 0 else ProdexRichStringView(0, 0),
        pj_text(tree, name),
    )
    js_literal(sink, StringSlice(',"type":"tool_use"}'))


def anthropic_chat_write_non_tool_blocks(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    message: Int64,
    wrote: Pointer[mut=True, Bool, _],
):
    var content = pj_string_field(tree, message, StringSlice("content"))
    if content >= 0 and pj_text(tree, content).len > 0:
        if wrote[]:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"text":'))
        js_string(sink, pj_text(tree, content))
        js_literal(sink, StringSlice(',"type":"text"}'))
        wrote[] = True
    var calls = pj_field(tree, message, StringSlice("tool_calls"))
    if pj_kind(tree, calls) == JSON_ARRAY:
        var call = pj_child(tree, calls)
        while call >= 0:
            if wrote[]:
                js_byte(sink, 44)
            anthropic_chat_write_tool_use(sink, tree, message, call)
            wrote[] = True
            call = pj_next(tree, call)


def anthropic_chat_next_emitted(tree: ParsedJson, start: Int64) -> Int64:
    var current = start
    while current >= 0:
        if anthropic_chat_message_has_blocks(tree, current):
            return current
        current = pj_next(tree, current)
    return -1


def anthropic_chat_write_message_group(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    start: Int64,
    role: Int64,
) -> Int64:
    js_literal(sink, StringSlice('{"content":['))
    var wrote = False

    # Rust inserts tool_result blocks before pre-existing non-tool blocks while
    # preserving the relative order of all tool results.
    var probe = start
    var stop: Int64 = -1
    while probe >= 0:
        if anthropic_chat_message_has_blocks(tree, probe):
            if anthropic_chat_source_role(tree, probe) != role:
                stop = probe
                break
            if anthropic_chat_is_tool_message(tree, probe):
                if wrote:
                    js_byte(sink, 44)
                anthropic_chat_write_tool_result(sink, tree, probe)
                wrote = True
        probe = pj_next(tree, probe)

    probe = start
    while probe >= 0 and probe != stop:
        if (
            anthropic_chat_message_has_blocks(tree, probe)
            and anthropic_chat_source_role(tree, probe) == role
            and not anthropic_chat_is_tool_message(tree, probe)
        ):
            anthropic_chat_write_non_tool_blocks(
                sink, tree, probe, Pointer(to=wrote)
            )
        probe = pj_next(tree, probe)

    js_literal(sink, StringSlice('],"role":"'))
    js_literal(
        sink,
        StringSlice("assistant") if role == 1 else StringSlice("user"),
    )
    js_literal(sink, StringSlice('"}'))
    return stop


def anthropic_chat_write_messages(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, messages: Int64
):
    js_byte(sink, 91)
    var wrote = False
    var current = anthropic_chat_next_emitted(tree, pj_child(tree, messages))
    while current >= 0:
        var role = anthropic_chat_source_role(tree, current)
        if wrote:
            js_byte(sink, 44)
        var stop = anthropic_chat_write_message_group(
            sink, tree, current, role
        )
        wrote = True
        current = anthropic_chat_next_emitted(
            tree, stop if stop >= 0 else -1
        )
    js_byte(sink, 93)
