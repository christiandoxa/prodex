from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson,
    ParsedJsonNode,
    JSON_TRUE,
    JSON_OBJECT,
    pj_kind,
    pj_field,
    pj_string_field,
    pj_text,
    pj_literal,
    pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from openai_chat_request_validation import ocr_validate, ocr_error_literal
from openai_chat_request_messages import ocr_message_count, ocr_write_messages

comptime OPENAI_CHAT_REQUEST_ABI: Int64 = 1

def ocr_write_pair_raw(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    key: StringSlice,
    value: Int64,
    count: Pointer[mut=True, Int64, _],
):
    if value < 0:
        return
    if count[] > 0:
        js_byte(sink, 44)
    js_string(sink, pj_literal(key))
    js_byte(sink, 58)
    js_raw(sink, tree, value)
    count[] += 1


def ocr_write_success(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    context: Int64,
    request: Int64,
):
    var default_model = pj_string_field(tree, context, StringSlice("default_model"))
    var input_model = pj_string_field(tree, context, StringSlice("input_model"))
    var request_model = pj_string_field(tree, request, StringSlice("model"))
    var selected_model = request_model
    if selected_model < 0:
        selected_model = input_model
    if selected_model < 0:
        selected_model = default_model

    js_byte(sink, 83)
    js_byte(sink, 123)
    var fields: Int64 = 0

    ocr_write_pair_raw(
        sink, tree, StringSlice("frequency_penalty"),
        pj_field(tree, request, StringSlice("frequency_penalty")), Pointer(to=fields)
    )

    var max_tokens = pj_field(tree, request, StringSlice("max_completion_tokens"))
    if max_tokens < 0:
        max_tokens = pj_field(tree, request, StringSlice("max_output_tokens"))
    if max_tokens < 0:
        max_tokens = pj_field(tree, request, StringSlice("max_tokens"))
    if max_tokens >= 0:
        if fields > 0:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('"max_tokens":'))
        js_raw(sink, tree, max_tokens)
        fields += 1

    if fields > 0:
        js_byte(sink, 44)
    js_literal(sink, StringSlice('"messages":'))
    ocr_write_messages(sink, tree, request)
    fields += 1

    if fields > 0:
        js_byte(sink, 44)
    js_literal(sink, StringSlice('"model":'))
    js_string(sink, pj_text(tree, selected_model))
    fields += 1

    ocr_write_pair_raw(
        sink, tree, StringSlice("parallel_tool_calls"),
        pj_field(tree, request, StringSlice("parallel_tool_calls")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("presence_penalty"),
        pj_field(tree, request, StringSlice("presence_penalty")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("seed"),
        pj_field(tree, request, StringSlice("seed")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("stop"),
        pj_field(tree, request, StringSlice("stop")), Pointer(to=fields)
    )

    if fields > 0:
        js_byte(sink, 44)
    js_literal(sink, StringSlice('"stream":'))
    if pj_kind(tree, pj_field(tree, request, StringSlice("stream"))) == JSON_TRUE:
        js_literal(sink, StringSlice("true"))
    else:
        js_literal(sink, StringSlice("false"))
    fields += 1

    ocr_write_pair_raw(
        sink, tree, StringSlice("temperature"),
        pj_field(tree, request, StringSlice("temperature")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("tool_choice"),
        pj_field(tree, request, StringSlice("tool_choice")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("tools"),
        pj_field(tree, request, StringSlice("tools")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("top_p"),
        pj_field(tree, request, StringSlice("top_p")), Pointer(to=fields)
    )
    ocr_write_pair_raw(
        sink, tree, StringSlice("user"),
        pj_field(tree, request, StringSlice("user")), Pointer(to=fields)
    )
    js_byte(sink, 125)


@export("prodex_mojo_openai_chat_request_v1")
def prodex_mojo_openai_chat_request_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != OPENAI_CHAT_REQUEST_ABI:
        return 4
    if operation != 0 or flag != 0 or measuring < 0 or measuring > 1 or raw_length < 0:
        return 1
    if (
        capacity < 0
        or metadata_address == 0
        or nodes_address == 0
        or scratch_address == 0
        or scratch_count < nodes_count
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return 1
    var provider = pj_string_field(tree, 0, StringSlice("provider"))
    var default_model = pj_string_field(tree, 0, StringSlice("default_model"))
    var request = pj_field(tree, 0, StringSlice("request"))
    if provider < 0 or default_model < 0 or request < 0:
        return 1

    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        capacity,
        0,
        measuring == 1,
        False,
    )
    if ocr_validate(Pointer(to=sink), tree, request, pj_text(tree, provider)):
        if ocr_message_count(tree, request) == 0:
            ocr_error_literal(
                Pointer(to=sink),
                StringSlice("Responses request must include a textual input or messages array"),
            )
        else:
            ocr_write_success(Pointer(to=sink), tree, 0, request)

    if sink.failed:
        return 3
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    meta[unsafe_offset=0] = 1
    meta[unsafe_offset=1] = sink.written
    return 0
