from std.memory import Pointer

from rich_types import ProdexRichStringView
from parsed_json import (
    ParsedJson,
    ParsedJsonNode,
    JSON_TRUE,
    JSON_STRING,
    JSON_ARRAY,
    JSON_OBJECT,
    pj_kind,
    pj_child,
    pj_next,
    pj_text,
    pj_field,
    pj_equal,
    pj_literal,
    pj_valid,
)
from json_sink import JsonSink, js_byte, js_literal, js_raw
from anthropic_chat_common import anthropic_chat_error, anthropic_chat_error_field
from anthropic_chat_messages import (
    anthropic_chat_validate_messages,
    anthropic_chat_system_will_render,
    anthropic_chat_write_system,
    anthropic_chat_write_messages,
)
from anthropic_chat_tools import (
    anthropic_chat_validate_tools,
    anthropic_chat_validate_web_search,
    anthropic_chat_validate_tool_choice,
    anthropic_chat_web_degradation,
    anthropic_chat_tool_choice_none,
    anthropic_chat_tools_present,
    anthropic_chat_write_tools,
    anthropic_chat_write_tool_choice,
)

comptime ANTHROPIC_CHAT_ABI_VERSION: Int64 = 1


def anthropic_chat_key_allowed(key: ProdexRichStringView) -> Bool:
    return (
        pj_equal(key, pj_literal(StringSlice("model")))
        or pj_equal(key, pj_literal(StringSlice("messages")))
        or pj_equal(key, pj_literal(StringSlice("max_tokens")))
        or pj_equal(key, pj_literal(StringSlice("stream")))
        or pj_equal(key, pj_literal(StringSlice("temperature")))
        or pj_equal(key, pj_literal(StringSlice("top_p")))
        or pj_equal(key, pj_literal(StringSlice("stop")))
        or pj_equal(key, pj_literal(StringSlice("tools")))
        or pj_equal(key, pj_literal(StringSlice("tool_choice")))
        or pj_equal(key, pj_literal(StringSlice("stream_options")))
        or pj_equal(key, pj_literal(StringSlice("web_search_options")))
        or pj_equal(key, pj_literal(StringSlice("parallel_tool_calls")))
    )


def anthropic_chat_validate_fields(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, chat: Int64
) -> Bool:
    var field = pj_child(tree, chat)
    while field >= 0:
        var key = tree.nodes[unsafe_offset=field].key.copy()
        if pj_equal(key, pj_literal(StringSlice("parallel_tool_calls"))):
            if pj_kind(tree, field) != JSON_TRUE:
                anthropic_chat_error(
                    sink,
                    StringSlice(
                        "Anthropic Messages only accepts `parallel_tool_calls=true`"
                    ),
                )
                return False
        elif not anthropic_chat_key_allowed(key):
            anthropic_chat_error_field(
                sink,
                StringSlice("Anthropic Messages does not translate chat field `"),
                key,
                StringSlice("`"),
            )
            return False
        field = pj_next(tree, field)
    return True


def anthropic_chat_validate_stop(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, stop: Int64
) -> Bool:
    if stop < 0 or pj_kind(tree, stop) == JSON_STRING or pj_kind(tree, stop) == JSON_ARRAY:
        return True
    anthropic_chat_error(
        sink, StringSlice("Responses `stop` must be a string or array")
    )
    return False


def anthropic_chat_write_stop(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, stop: Int64
):
    if pj_kind(tree, stop) == JSON_STRING:
        js_byte(sink, 91)
        js_raw(sink, tree, stop)
        js_byte(sink, 93)
    else:
        js_raw(sink, tree, stop)


def anthropic_chat_write_body(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, chat: Int64
):
    var model = pj_field(tree, chat, StringSlice("model"))
    var messages = pj_field(tree, chat, StringSlice("messages"))
    var max_tokens = pj_field(tree, chat, StringSlice("max_tokens"))
    var stream = pj_field(tree, chat, StringSlice("stream"))
    var temperature = pj_field(tree, chat, StringSlice("temperature"))
    var top_p = pj_field(tree, chat, StringSlice("top_p"))
    var stop = pj_field(tree, chat, StringSlice("stop"))
    var tools = pj_field(tree, chat, StringSlice("tools"))
    var choice = pj_field(tree, chat, StringSlice("tool_choice"))
    var web_search = pj_field(tree, chat, StringSlice("web_search_options"))

    js_byte(sink, 123)
    js_literal(sink, StringSlice('"max_tokens":'))
    if max_tokens >= 0:
        js_raw(sink, tree, max_tokens)
    else:
        js_literal(sink, StringSlice("4096"))

    js_literal(sink, StringSlice(',"messages":'))
    anthropic_chat_write_messages(sink, tree, messages)

    js_literal(sink, StringSlice(',"model":'))
    if model >= 0:
        js_raw(sink, tree, model)
    else:
        js_literal(sink, StringSlice('"auto"'))

    if stop >= 0:
        js_literal(sink, StringSlice(',"stop_sequences":'))
        anthropic_chat_write_stop(sink, tree, stop)

    js_literal(sink, StringSlice(',"stream":'))
    js_literal(
        sink,
        StringSlice("true") if pj_kind(tree, stream) == JSON_TRUE else StringSlice("false"),
    )

    if anthropic_chat_system_will_render(tree, messages):
        js_literal(sink, StringSlice(',"system":'))
        anthropic_chat_write_system(sink, tree, messages)

    if temperature >= 0:
        js_literal(sink, StringSlice(',"temperature":'))
        js_raw(sink, tree, temperature)
    if anthropic_chat_tools_present(tree, tools, web_search, choice):
        js_literal(sink, StringSlice(',"tools":'))
        anthropic_chat_write_tools(sink, tree, tools, web_search)
    if choice >= 0 and not anthropic_chat_tool_choice_none(tree, choice):
        js_literal(sink, StringSlice(',"tool_choice":'))
        anthropic_chat_write_tool_choice(sink, tree, choice)
    if top_p >= 0:
        js_literal(sink, StringSlice(',"top_p":'))
        js_raw(sink, tree, top_p)
    js_byte(sink, 125)


def anthropic_chat_validate(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, chat: Int64
) -> Bool:
    if pj_kind(tree, chat) != JSON_OBJECT:
        anthropic_chat_error(
            sink, StringSlice("translated request body must be a JSON object")
        )
        return False
    if not anthropic_chat_validate_fields(sink, tree, chat):
        return False
    var messages = pj_field(tree, chat, StringSlice("messages"))
    if not anthropic_chat_validate_messages(sink, tree, messages):
        return False
    if not anthropic_chat_validate_stop(
        sink, tree, pj_field(tree, chat, StringSlice("stop"))
    ):
        return False
    if not anthropic_chat_validate_tools(
        sink, tree, pj_field(tree, chat, StringSlice("tools"))
    ):
        return False
    if not anthropic_chat_validate_web_search(
        sink, tree, pj_field(tree, chat, StringSlice("web_search_options"))
    ):
        return False
    if not anthropic_chat_validate_tool_choice(
        sink, tree, pj_field(tree, chat, StringSlice("tool_choice"))
    ):
        return False
    return True


@export("prodex_mojo_anthropic_chat_request_v1")
def prodex_mojo_anthropic_chat_request_v1(
    abi: Int64,
    operation: Int64,
    flag: Int64,
    nodes_address: UInt64,
    nodes_count: Int64,
    raw_address: UInt64,
    raw_length: Int64,
    scratch_address: UInt64,
    scratch_count: Int64,
    measuring: Int64,
    output_address: UInt64,
    capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != ANTHROPIC_CHAT_ABI_VERSION:
        return 4
    if operation != 0 or flag != 0 or measuring < 0 or measuring > 1 or raw_length < 0:
        return 1
    if (
        nodes_address == 0
        or scratch_address == 0
        or metadata_address == 0
        or nodes_count <= 0
        or scratch_count < nodes_count
        or capacity < 0
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
    if not pj_valid(tree):
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
    if anthropic_chat_validate(Pointer(to=sink), tree, 0):
        var choice = pj_field(tree, 0, StringSlice("tool_choice"))
        var web_search = pj_field(tree, 0, StringSlice("web_search_options"))
        var degradation = (
            0
            if anthropic_chat_tool_choice_none(tree, choice)
            else anthropic_chat_web_degradation(tree, web_search)
        )
        js_byte(Pointer(to=sink), UInt8(68 if degradation > 0 else 83))
        if degradation > 0:
            js_byte(Pointer(to=sink), UInt8(48 + degradation))
        anthropic_chat_write_body(Pointer(to=sink), tree, 0)

    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
