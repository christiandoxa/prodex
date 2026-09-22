from std.memory import Pointer
from parsed_json import (
    ParsedJson, JSON_NULL, JSON_STRING, JSON_ARRAY, JSON_OBJECT,
    pj_kind, pj_child, pj_next, pj_field, pj_nonblank, pj_is,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from deepseek_message_index import ds_call_selected

comptime DS_MESSAGE_ORIGINAL: Int64 = 0
comptime DS_MESSAGE_THINKING: Int64 = 1
comptime DS_MESSAGE_ASSISTANT: Int64 = 2
comptime DS_MESSAGE_DROP_CALLS: Int64 = 3
comptime DS_MESSAGE_SELECTED_CALLS: Int64 = 4


def ds_calls(tree: ParsedJson, message: Int64) -> Int64:
    var calls = pj_field(tree, message, StringSlice("tool_calls"))
    return calls if pj_kind(tree, calls) == JSON_ARRAY and pj_child(tree, calls) >= 0 else -1


def ds_value_has_content(tree: ParsedJson, node: Int64) -> Bool:
    var kind = pj_kind(tree, node)
    if kind == JSON_NULL or kind == -1:
        return False
    if kind == JSON_STRING:
        return pj_nonblank(tree, node)
    if kind == JSON_ARRAY or kind == JSON_OBJECT:
        return pj_child(tree, node) >= 0
    return True


def ds_message_has_content(tree: ParsedJson, message: Int64) -> Bool:
    return ds_value_has_content(tree, pj_field(tree, message, StringSlice("content"))) or ds_value_has_content(tree, pj_field(tree, message, StringSlice("reasoning_content")))


def ds_field_prefix(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, node: Int64, count: Pointer[mut=True, Int64, _]):
    if count[] > 0:
        js_byte(sink, 44)
    count[] += 1
    js_string(sink, tree.nodes[unsafe_offset=node].key)
    js_byte(sink, 58)


def ds_message(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, message: Int64,
    mode: Int64, scratch: Pointer[mut=True, Int64, _], outputs_count: Int64,
):
    var calls = ds_calls(tree, message)
    var assistant = pj_is["assistant"](tree, pj_field(tree, message, StringSlice("role")))
    var normalize = assistant and calls >= 0 and (mode == DS_MESSAGE_THINKING or mode == DS_MESSAGE_ASSISTANT or mode == DS_MESSAGE_SELECTED_CALLS)
    var content = pj_field(tree, message, StringSlice("content"))
    var reasoning = pj_field(tree, message, StringSlice("reasoning_content"))
    var empty_content = normalize and (content < 0 or pj_kind(tree, content) == JSON_NULL)
    var empty_reasoning = normalize and mode == DS_MESSAGE_THINKING and pj_kind(tree, reasoning) != JSON_STRING
    if not empty_content and not empty_reasoning and mode != DS_MESSAGE_DROP_CALLS and mode != DS_MESSAGE_SELECTED_CALLS:
        js_raw(sink, tree, message)
        return
    var fields: Int64 = 0
    js_byte(sink, 123)
    var child = pj_child(tree, message)
    while child >= 0:
        if child == calls and mode == DS_MESSAGE_DROP_CALLS:
            child = pj_next(tree, child)
            continue
        ds_field_prefix(sink, tree, child, Pointer(to=fields))
        if child == content and empty_content or child == reasoning and empty_reasoning:
            js_literal(sink, StringSlice('""'))
        elif child == calls and mode == DS_MESSAGE_SELECTED_CALLS:
            js_byte(sink, 91)
            var selected: Int64 = 0
            var call = pj_child(tree, calls)
            while call >= 0:
                if ds_call_selected(tree, scratch, outputs_count, call):
                    if selected > 0:
                        js_byte(sink, 44)
                    js_raw(sink, tree, call)
                    selected += 1
                call = pj_next(tree, call)
            js_byte(sink, 93)
        else:
            js_raw(sink, tree, child)
        child = pj_next(tree, child)
    if empty_content and content < 0:
        if fields > 0:
            js_byte(sink, 44)
        fields += 1
        js_literal(sink, StringSlice('"content":""'))
    if empty_reasoning and reasoning < 0:
        if fields > 0:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('"reasoning_content":""'))
    js_byte(sink, 125)


def ds_normalize_thinking(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, scratch: Pointer[mut=True, Int64, _]) -> Bool:
    if pj_kind(tree, 0) != JSON_ARRAY:
        return False
    js_byte(sink, 91)
    var message = pj_child(tree, 0)
    var count: Int64 = 0
    while message >= 0:
        if count > 0:
            js_byte(sink, 44)
        ds_message(sink, tree, message, DS_MESSAGE_THINKING, scratch, 0)
        count += 1
        message = pj_next(tree, message)
    js_byte(sink, 93)
    return True
