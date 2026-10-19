# Gemini conversation evidence and payload selection; transport remains native.
from std.memory import Pointer

from gemini_guardrails import (
    guard_process_exited_zero, guard_success_claim, guard_tool_failure,
    guard_verification_marker,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_view
from parsed_json import (
    JSON_ARRAY, JSON_OBJECT, JSON_STRING, ParsedJson, ParsedJsonNode, pj_child,
    pj_field, pj_is, pj_kind, pj_next, pj_string_field, pj_text, pj_trim, pj_valid,
)
from rich_types import ProdexRichStringView


def guard_collect_payload(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, root: Int64,
):
    var cursor = root
    while cursor >= 0:
        var kind = pj_kind(tree, cursor)
        var child: Int64 = -1
        if kind == JSON_ARRAY:
            child = pj_child(tree, cursor)
        elif kind == JSON_OBJECT:
            child = pj_field(tree, cursor, StringSlice("output"))
            if child < 0:
                child = pj_field(tree, cursor, StringSlice("content"))
            if child < 0:
                child = pj_field(tree, cursor, StringSlice("text"))
        elif kind == JSON_STRING:
            if sink[].written > 0:
                js_byte(sink, 10)
            js_view(sink, pj_text(tree, cursor))
        if child >= 0:
            cursor = child
            continue
        # Only arrays visit siblings; objects consume their first present field.
        while cursor != root:
            var parent = tree.nodes[unsafe_offset=cursor].parent
            var sibling = pj_next(tree, cursor)
            if pj_kind(tree, parent) == JSON_ARRAY and sibling >= 0:
                cursor = sibling
                break
            cursor = parent
        if cursor == root:
            break


def guard_unverified_claim(
    tree: ParsedJson, scratch: Pointer[mut=True, UInt8, MutUntrackedOrigin], capacity: Int64,
) -> Int64:
    if not guard_success_claim(pj_text(tree, pj_string_field(tree, 0, StringSlice("text")))):
        return 0
    var messages = pj_field(tree, 0, StringSlice("messages"))
    var start: Int64 = 1
    var index: Int64 = 0
    var message = pj_child(tree, messages)
    while message >= 0:
        if pj_is["user"](tree, pj_field(tree, message, StringSlice("role"))):
            start = index + 1
        index += 1
        message = pj_next(tree, message)
    var count: Int64 = 0
    var previous_failed = False
    var last_failed = False
    var last_failed_without_exit = False
    var last_verified = False
    message = pj_child(tree, messages)
    index = 0
    while message >= 0:
        if index >= start and pj_is["tool"](tree, pj_field(tree, message, StringSlice("role"))):
            var payload = pj_field(tree, message, StringSlice("output"))
            if payload < 0:
                payload = pj_field(tree, message, StringSlice("content"))
            var buffer = JsonSink(scratch, capacity, 0, False, False)
            guard_collect_payload(Pointer(to=buffer), tree, payload)
            if buffer.failed:
                return -1
            var text = ProdexRichStringView(UInt(Int(scratch)), UInt(buffer.written))
            if pj_trim(text).len > 0:
                previous_failed = last_failed_without_exit
                last_failed = guard_tool_failure(text)
                last_failed_without_exit = last_failed and not guard_process_exited_zero(text)
                last_verified = guard_verification_marker(text)
                count += 1
        index += 1
        message = pj_next(tree, message)
    if count == 0 or last_failed:
        return 1
    return Int64(not (last_verified and not previous_failed and not last_failed_without_exit))


@export("prodex_mojo_gemini_guardrail_conversation_v1")
def prodex_mojo_gemini_guardrail_conversation_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt, count: Int64, raw_address: UInt, raw_length: Int64,
    scratch_address: UInt, scratch_count: Int64, measuring: Int64,
    output_address: UInt, output_capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 1 or flag != 0
        or nodes_address == 0 or nodes_address % 8 != 0 or count <= 0
        or raw_length < 0 or (raw_length > 0 and raw_address == 0)
        or scratch_address == 0 or scratch_address % 8 != 0
        or scratch_count <= 0 or scratch_count > 0x7FFFFFFFFFFFFFFF // 16
        or metadata_address == 0 or metadata_address % 8 != 0
        or measuring < 0 or measuring > 1 or output_capacity < 0
        or (measuring == 1 and (output_address != 0 or output_capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        count, ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return 1
    var required = count
    for index in range(count):
        var length = Int64(tree.nodes[unsafe_offset=index].text.len)
        if length > 0x7FFFFFFFFFFFFFFF - required:
            return 1
        required += length
    var capacity = scratch_count * 16
    if capacity < required:
        return 1
    var prefix = pj_string_field(tree, 0, StringSlice("prefix"))
    var text = pj_string_field(tree, 0, StringSlice("text"))
    var messages = pj_field(tree, 0, StringSlice("messages"))
    if (operation == 0 and prefix < 0) or (
        operation == 1 and (text < 0 or pj_kind(tree, messages) != JSON_ARRAY)
    ):
        return 1
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        output_capacity, 0, measuring == 1, False,
    )
    if operation == 0:
        var buffer = JsonSink(scratch, capacity, 0, False, False)
        js_view(Pointer(to=buffer), pj_text(tree, prefix))
        guard_collect_payload(Pointer(to=buffer), tree, pj_field(tree, 0, StringSlice("value")))
        if buffer.failed:
            return 3
        js_string(Pointer(to=sink), ProdexRichStringView(scratch_address, UInt(buffer.written)))
    else:
        var unverified = guard_unverified_claim(tree, scratch, capacity)
        if unverified < 0:
            return 3
        js_literal(Pointer(to=sink), StringSlice("true") if unverified == 1 else StringSlice("false"))
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
