# Payload display semantics. Rust supplies Serde's JSON presentation and wraps
# the resulting display records using the terminal's Unicode width tables.
from std.memory import Pointer
from rich_types import ProdexRichStringView, rich_view_ptr
from rich_text import rich_codepoint_width, rich_view_valid
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_STRING, JSON_NUMBER, JSON_FALSE, JSON_TRUE,
    JSON_ARRAY, JSON_OBJECT, pj_kind, pj_child, pj_next, pj_field, pj_string_field,
    pj_text, pj_is, pj_literal, pj_equal, pj_trim,
    pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_escaped


def payload_decimal(sink: Pointer[mut=True, JsonSink, _], value: Int64):
    var divisor: Int64 = 1
    while divisor <= value // 10:
        divisor *= 10
    while divisor > 0:
        js_byte(sink, UInt8((value // divisor) % 10) + 48)
        divisor //= 10


def payload_begin(sink: Pointer[mut=True, JsonSink, _], kind: UInt8):
    # The opening array is the only byte before the first record.
    if sink[].written > 1:
        js_byte(sink, 44)
    js_byte(sink, 91)
    js_byte(sink, 48 + kind)
    js_literal(sink, StringSlice(",\""))


def payload_end(sink: Pointer[mut=True, JsonSink, _]):
    js_literal(sink, StringSlice("\"]"))


def payload_count(tree: ParsedJson, index: Int64) -> Int64:
    var count: Int64 = 0
    var child = pj_child(tree, index)
    while child >= 0:
        count += 1
        child = pj_next(tree, child)
    return count


def payload_has_type(tree: ParsedJson, item: Int64, expected: StringSlice) -> Bool:
    if pj_equal(pj_text(tree, pj_field(tree, item, StringSlice("type"))), pj_literal(expected)):
        return True
    var content = pj_field(tree, item, StringSlice("content"))
    if pj_kind(tree, content) != JSON_ARRAY:
        return False
    var child = pj_child(tree, content)
    while child >= 0:
        if pj_equal(pj_text(tree, pj_field(tree, child, StringSlice("type"))), pj_literal(expected)):
            return True
        child = pj_next(tree, child)
    return False


def payload_space(sink: Pointer[mut=True, JsonSink, _], count: Pointer[mut=True, Int64, _]):
    if count[] > 0:
        js_byte(sink, 32)
    count[] += 1


def payload_summary(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var count: Int64 = 0
    for key in [
        StringSlice("model"), StringSlice("stream"), StringSlice("store"),
        StringSlice("tool_choice"), StringSlice("parallel_tool_calls"),
        StringSlice("prompt_cache_key"),
    ]:
        var node = pj_field(tree, 0, key)
        var kind = pj_kind(tree, node)
        if kind != JSON_STRING and kind != JSON_NUMBER and kind != JSON_FALSE and kind != JSON_TRUE:
            continue
        payload_space(sink, Pointer(to=count))
        js_escaped(sink, pj_literal(key))
        js_byte(sink, 61)
        if kind == JSON_STRING:
            var text = pj_text(tree, node)
            var length = Int64(text.len)
            if length > 80:
                var source = rich_view_ptr(text)
                var end: Int64 = 0
                var scalars = 0
                while end < length and scalars < 77:
                    end += rich_codepoint_width(source[unsafe_offset=end])
                    scalars += 1
                js_escaped(sink, ProdexRichStringView(text.ptr, UInt(end)))
                js_literal(sink, StringSlice("..."))
            else:
                js_escaped(sink, text)
        else:
            var raw = tree.nodes[unsafe_offset=node].copy()
            js_escaped(sink, ProdexRichStringView(tree.raw.ptr + UInt(raw.raw_start), UInt(raw.raw_length)))
    var effort = pj_string_field(tree, pj_field(tree, 0, StringSlice("reasoning")), StringSlice("effort"))
    if effort >= 0:
        payload_space(sink, Pointer(to=count))
        js_literal(sink, StringSlice("effort="))
        js_escaped(sink, pj_text(tree, effort))
    var input = pj_field(tree, 0, StringSlice("input"))
    var compaction = pj_field(tree, 0, StringSlice("compaction")) >= 0
    if pj_kind(tree, input) == JSON_ARRAY:
        var images: Int64 = 0
        var audio: Int64 = 0
        var child = pj_child(tree, input)
        while child >= 0:
            images += Int64(payload_has_type(tree, child, StringSlice("input_image")))
            audio += Int64(payload_has_type(tree, child, StringSlice("input_audio")))
            compaction = compaction or pj_is["compaction"](tree, pj_field(tree, child, StringSlice("type")))
            child = pj_next(tree, child)
        payload_space(sink, Pointer(to=count))
        js_literal(sink, StringSlice("input_items="))
        payload_decimal(sink, payload_count(tree, input))
        if images > 0:
            js_literal(sink, StringSlice(" images="))
            payload_decimal(sink, images)
        if audio > 0:
            js_literal(sink, StringSlice(" audio="))
            payload_decimal(sink, audio)
    var tools = pj_field(tree, 0, StringSlice("tools"))
    if pj_kind(tree, tools) == JSON_ARRAY:
        payload_space(sink, Pointer(to=count))
        js_literal(sink, StringSlice("tools="))
        payload_decimal(sink, payload_count(tree, tools))
    if pj_field(tree, 0, StringSlice("previous_response_id")) >= 0:
        payload_space(sink, Pointer(to=count))
        js_literal(sink, StringSlice("continuation=previous_response"))
    if compaction:
        payload_space(sink, Pointer(to=count))
        js_literal(sink, StringSlice("compaction=yes"))


def payload_metadata(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var metadata = pj_field(tree, 0, StringSlice("client_metadata"))
    var count: Int64 = 0
    for key in [
        StringSlice("session_id"), StringSlice("thread_id"), StringSlice("turn_id"),
        StringSlice("context_window_id"), StringSlice("x-codex-window-id"),
        StringSlice("x-codex-installation-id"),
    ]:
        var node = pj_string_field(tree, metadata, key)
        if node < 0:
            continue
        if count == 0:
            js_literal(sink, StringSlice("client_metadata: "))
        payload_space(sink, Pointer(to=count))
        var text = pj_text(tree, node)
        if key == StringSlice("context_window_id"):
            js_literal(sink, StringSlice("ctx="))
            # str::get(..4) keeps the original string if byte 4 splits UTF-8.
            if text.len >= 4:
                var source = rich_view_ptr(text)
                if text.len == 4 or source[unsafe_offset=4] & 0xC0 != 0x80:
                    text.len = 4
        else:
            js_escaped(sink, pj_literal(key))
            js_byte(sink, 61)
        js_escaped(sink, text)


def payload_content_field(tree: ParsedJson, index: Int64) -> Int64:
    for key in [StringSlice("text"), StringSlice("content"), StringSlice("input_text"), StringSlice("output_text")]:
        var child = pj_field(tree, index, key)
        if child >= 0:
            return child
    return -1


def payload_content_nonblank(tree: ParsedJson, index: Int64) -> Bool:
    var kind = pj_kind(tree, index)
    if kind == JSON_STRING:
        return pj_trim(pj_text(tree, index)).len > 0
    if kind == JSON_ARRAY:
        var child = pj_child(tree, index)
        while child >= 0:
            if payload_content_nonblank(tree, child):
                return True
            child = pj_next(tree, child)
        return False
    if kind == JSON_OBJECT:
        var child = payload_content_field(tree, index)
        if child >= 0:
            return payload_content_nonblank(tree, child)
    return True


def payload_content(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    serialized: Pointer[mut=False, ProdexRichStringView, _], index: Int64,
):
    var kind = pj_kind(tree, index)
    if kind == JSON_STRING:
        js_escaped(sink, pj_text(tree, index))
    elif kind == JSON_ARRAY:
        var child = pj_child(tree, index)
        var first = True
        while child >= 0:
            if payload_content_nonblank(tree, child):
                if not first:
                    js_literal(sink, StringSlice("\\n"))
                payload_content(sink, tree, serialized, child)
                first = False
            child = pj_next(tree, child)
    elif kind == JSON_OBJECT and payload_content_field(tree, index) >= 0:
        payload_content(sink, tree, serialized, payload_content_field(tree, index))
    else:
        js_escaped(sink, serialized[unsafe_offset=index * 2])


def payload_item(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    serialized: Pointer[mut=False, ProdexRichStringView, _], item: Int64, index: Int64,
):
    var object = pj_kind(tree, item) == JSON_OBJECT
    var item_type = pj_string_field(tree, item, StringSlice("type"))
    var call = pj_is["function_call"](tree, item_type)
    var output = pj_is["function_call_output"](tree, item_type)
    payload_begin(sink, 1)
    js_literal(sink, StringSlice("input["))
    payload_decimal(sink, index)
    js_byte(sink, 93)
    if call:
        js_literal(sink, StringSlice(" tool-call:"))
        var name = pj_string_field(tree, item, StringSlice("name"))
        if name >= 0:
            js_escaped(sink, pj_text(tree, name))
        else:
            js_literal(sink, StringSlice("tool"))
    elif output:
        js_literal(sink, StringSlice(" tool-output"))
    elif object:
        js_byte(sink, 32)
        var role = pj_string_field(tree, item, StringSlice("role"))
        if role < 0:
            role = item_type
        if role >= 0:
            js_escaped(sink, pj_text(tree, role))
        else:
            js_literal(sink, StringSlice("message"))
    js_byte(sink, 58)
    payload_end(sink)
    payload_begin(sink, 2)
    if call or output:
        var field = StringSlice("arguments") if call else StringSlice("output")
        var node = pj_field(tree, item, field)
        if node >= 0:
            if pj_kind(tree, node) == JSON_STRING:
                var parsed = serialized[unsafe_offset=node * 2 + 1].copy()
                js_escaped(sink, parsed.copy() if parsed.len > 0 else pj_text(tree, node))
            else:
                js_escaped(sink, serialized[unsafe_offset=node * 2])
    elif object:
        var content = pj_field(tree, item, StringSlice("content"))
        if content >= 0 and payload_content_nonblank(tree, content):
            payload_content(sink, tree, serialized, content)
        else:
            var text = pj_string_field(tree, item, StringSlice("text"))
            if text >= 0:
                js_escaped(sink, pj_text(tree, text))
            else:
                js_escaped(sink, serialized[unsafe_offset=item * 2])
    else:
        js_escaped(sink, serialized[unsafe_offset=item * 2])
    payload_end(sink)


def payload_render(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    serialized: Pointer[mut=False, ProdexRichStringView, _],
):
    js_byte(sink, 91)
    if pj_kind(tree, 0) == JSON_OBJECT:
        for part in range(2):
            var probe = JsonSink(sink[].output, 0, 0, True, False)
            if part == 0:
                payload_summary(Pointer(to=probe), tree)
            else:
                payload_metadata(Pointer(to=probe), tree)
            if probe.written > 0:
                payload_begin(sink, 0)
                if part == 0:
                    payload_summary(sink, tree)
                else:
                    payload_metadata(sink, tree)
                payload_end(sink)
        var instructions = pj_string_field(tree, 0, StringSlice("instructions"))
        if instructions >= 0:
            payload_begin(sink, 1)
            js_literal(sink, StringSlice("instructions:"))
            payload_end(sink)
            payload_begin(sink, 2)
            js_escaped(sink, pj_text(tree, instructions))
            payload_end(sink)
        var input = pj_field(tree, 0, StringSlice("input"))
        if pj_kind(tree, input) == JSON_ARRAY:
            var index: Int64 = 0
            var child = pj_child(tree, input)
            while child >= 0:
                payload_item(sink, tree, serialized, child, index)
                index += 1
                child = pj_next(tree, child)
        elif input >= 0:
            payload_begin(sink, 1)
            js_literal(sink, StringSlice("input:"))
            payload_end(sink)
            payload_begin(sink, 2)
            js_escaped(sink, pj_text(tree, input) if pj_kind(tree, input) == JSON_STRING else serialized[unsafe_offset=input * 2].copy())
            payload_end(sink)
        var tools = pj_field(tree, 0, StringSlice("tools"))
        if pj_kind(tree, tools) == JSON_ARRAY:
            payload_begin(sink, 0)
            js_literal(sink, StringSlice("tools: "))
            payload_decimal(sink, payload_count(tree, tools))
            var first = True
            var child = pj_child(tree, tools)
            while child >= 0:
                var name = pj_string_field(tree, child, StringSlice("name"))
                if name < 0:
                    name = pj_string_field(tree, pj_field(tree, child, StringSlice("function")), StringSlice("name"))
                if name >= 0:
                    js_literal(sink, StringSlice(" ") if first else StringSlice(", "))
                    js_escaped(sink, pj_text(tree, name))
                    first = False
                child = pj_next(tree, child)
            payload_end(sink)
    if sink[].written == 1:
        payload_begin(sink, 0)
        js_escaped(sink, serialized[unsafe_offset=0])
        payload_end(sink)
    js_byte(sink, 93)


@export("prodex_mojo_log_payload_projection_v1")
def prodex_mojo_log_payload_projection_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64, raw_address: UInt64, raw_length: Int64,
    serialized_address: UInt64, serialized_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (operation != 0 or flag != 0 or nodes_count <= 0 or nodes_count > 1_048_576
        or nodes_address == 0 or serialized_address == 0 or serialized_count != nodes_count * 2
        or raw_length < 0 or capacity < 0 or metadata_address == 0
        or (measuring != 0 and measuring != 1)
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var serialized = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(serialized_address))
    for index in range(nodes_count):
        # Serde's recursion limit bounds both rendering and ABI-only callers.
        var depth = 0
        var parent = tree.nodes[unsafe_offset=index].parent
        while parent >= 0:
            depth += 1
            if depth >= 128:
                return 1
            parent = tree.nodes[unsafe_offset=parent].parent
        if (not rich_view_valid(serialized[unsafe_offset=index * 2], 0x7FFFFFFFFFFFFFFF)
            or not rich_view_valid(serialized[unsafe_offset=index * 2 + 1], 0x7FFFFFFFFFFFFFFF)):
            return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, measuring == 1, False,
    )
    payload_render(Pointer(to=sink), tree, serialized)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
