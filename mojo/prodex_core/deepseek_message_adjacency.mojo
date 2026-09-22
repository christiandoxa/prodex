from std.memory import Pointer
from parsed_json import ParsedJson, JSON_ARRAY, pj_kind, pj_child, pj_next, pj_field, pj_is
from json_sink import JsonSink, js_byte, js_raw
from deepseek_message_common import (
    DS_MESSAGE_ORIGINAL, DS_MESSAGE_DROP_CALLS, DS_MESSAGE_SELECTED_CALLS,
    ds_calls, ds_message_has_content, ds_message,
)
from deepseek_message_index import ds_outputs, ds_call_slot, ds_call_selected


def ds_repair_adjacency(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, scratch: Pointer[mut=True, Int64, _]) -> Bool:
    if pj_kind(tree, 0) != JSON_ARRAY:
        return False
    var outputs_count = ds_outputs(tree, scratch)
    var message = pj_child(tree, 0)
    var written: Int64 = 0
    js_byte(sink, 91)
    while message >= 0:
        # With no valid output IDs, the historical drop-unanswered path retains
        # otherwise untouched tool-role messages. Do not unify those two cases.
        if outputs_count > 0 and pj_is["tool"](tree, pj_field(tree, message, StringSlice("role"))):
            message = pj_next(tree, message)
            continue
        var calls = ds_calls(tree, message)
        var mode = DS_MESSAGE_ORIGINAL
        var selected: Int64 = 0
        if calls >= 0:
            var call = pj_child(tree, calls)
            while call >= 0:
                var slot = ds_call_slot(tree, scratch, outputs_count, call)
                if slot >= 0 and scratch[unsafe_offset=tree.count + slot] < 0:
                    scratch[unsafe_offset=tree.count + slot] = call
                    selected += 1
                call = pj_next(tree, call)
            if selected == 0:
                if not ds_message_has_content(tree, message):
                    message = pj_next(tree, message)
                    continue
                mode = DS_MESSAGE_DROP_CALLS
            else:
                mode = DS_MESSAGE_SELECTED_CALLS
        if written > 0:
            js_byte(sink, 44)
        ds_message(sink, tree, message, mode, scratch, outputs_count)
        written += 1
        if selected > 0:
            var call = pj_child(tree, calls)
            while call >= 0:
                if ds_call_selected(tree, scratch, outputs_count, call):
                    var slot = ds_call_slot(tree, scratch, outputs_count, call)
                    js_byte(sink, 44)
                    js_raw(sink, tree, scratch[unsafe_offset=slot])
                    written += 1
                call = pj_next(tree, call)
        message = pj_next(tree, message)
    js_byte(sink, 93)
    return True
