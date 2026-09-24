from std.memory import Pointer

from parsed_json import (
    ParsedJson,
    ParsedJsonNode,
    JSON_ARRAY,
    JSON_OBJECT,
    JSON_NULL,
    JSON_NUMBER,
    JSON_STRING,
    pj_child,
    pj_field,
    pj_kind,
    pj_literal,
    pj_next,
    pj_string_field,
    pj_text,
    pj_valid,
)
from json_sink import js_raw_view
from openai_compat import (
    OpenAiCompatWriter,
    openai_compat_put_byte,
    openai_compat_put_json_escaped_range,
    openai_compat_put_json_string,
    openai_compat_put_json_string_with_rtk_value,
    openai_compat_put_literal,
    openai_compat_put_u64,
    openai_compat_split_tool_name_parts,
    openai_compat_write_usage_values,
    openai_compat_writer_status,
)
from rich_text import rich_view_ptr
from rich_types import ProdexRichStringView

comptime OPENAI_CHAT_RESPONSE_ABI: Int64 = 1


def ocr_response_array_item_text(tree: ParsedJson, index: Int64) -> Int64:
    var text = pj_string_field(tree, index, StringSlice("text"))
    if text >= 0:
        return text
    return pj_string_field(tree, index, StringSlice("content"))


def ocr_response_text_plan(tree: ParsedJson, index: Int64) -> Tuple[Int64, Int64]:
    var kind = pj_kind(tree, index)
    if kind == JSON_STRING:
        return (1, index)
    if kind == JSON_ARRAY:
        var child = pj_child(tree, index)
        while child >= 0:
            var text = ocr_response_array_item_text(tree, child)
            if text >= 0 and pj_text(tree, text).len > 0:
                return (2, index)
            child = pj_next(tree, child)
        return (0, -1)
    if kind != JSON_OBJECT:
        return (0, -1)

    var text = pj_string_field(tree, index, StringSlice("text"))
    if text >= 0:
        return (1, text)
    var content = pj_field(tree, index, StringSlice("content"))
    var nested = ocr_response_text_plan(tree, content)
    if nested[0] != 0:
        return nested
    var output_text = pj_string_field(tree, index, StringSlice("output_text"))
    if output_text >= 0:
        return (1, output_text)
    return (0, -1)


def ocr_response_write_text_value(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], text: ProdexRichStringView
) -> Bool:
    return (
        openai_compat_put_literal(
            writer, StringSlice('{"type":"output_text","text":')
        )
        and openai_compat_put_json_string(writer, text)
        and openai_compat_put_byte(writer, 125)
    )


def ocr_response_write_text_plan(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    plan: Tuple[Int64, Int64],
) -> Bool:
    if plan[0] == 1:
        return ocr_response_write_text_value(writer, pj_text(tree, plan[1]))
    if plan[0] != 2:
        return False

    if not openai_compat_put_literal(
        writer, StringSlice('{"type":"output_text","text":"')
    ):
        return False
    var child = pj_child(tree, plan[1])
    var first = True
    while child >= 0:
        var text = ocr_response_array_item_text(tree, child)
        if text >= 0:
            var value = pj_text(tree, text)
            if value.len > 0:
                if not first:
                    if not openai_compat_put_byte(writer, 92) or not openai_compat_put_byte(
                        writer, 110
                    ):
                        return False
                if not openai_compat_put_json_escaped_range(
                    writer, value, 0, Int64(value.len)
                ):
                    return False
                first = False
        child = pj_next(tree, child)
    return openai_compat_put_literal(writer, StringSlice('"}'))


def ocr_response_content_present(tree: ParsedJson, content: Int64) -> Bool:
    var kind = pj_kind(tree, content)
    if kind == JSON_STRING:
        return pj_text(tree, content).len > 0
    if kind == JSON_ARRAY:
        var child = pj_child(tree, content)
        while child >= 0:
            var text = ocr_response_array_item_text(tree, child)
            if text >= 0 and pj_text(tree, text).len > 0:
                return True
            child = pj_next(tree, child)
        return False
    var plan = ocr_response_text_plan(tree, content)
    if plan[0] == 1:
        return pj_text(tree, plan[1]).len > 0
    return plan[0] == 2


def ocr_response_write_content(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    content: Int64,
) -> Bool:
    if not openai_compat_put_byte(writer, 91):
        return False
    var kind = pj_kind(tree, content)
    if kind == JSON_STRING:
        if not ocr_response_write_text_value(writer, pj_text(tree, content)):
            return False
    elif kind == JSON_ARRAY:
        var child = pj_child(tree, content)
        var first = True
        while child >= 0:
            var text = ocr_response_array_item_text(tree, child)
            if text >= 0 and pj_text(tree, text).len > 0:
                if not first and not openai_compat_put_byte(writer, 44):
                    return False
                if not ocr_response_write_text_value(writer, pj_text(tree, text)):
                    return False
                first = False
            child = pj_next(tree, child)
    else:
        var plan = ocr_response_text_plan(tree, content)
        if not ocr_response_write_text_plan(writer, tree, plan):
            return False
    return openai_compat_put_byte(writer, 93)


def ocr_response_write_message(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    message: Int64,
) -> Bool:
    var content = pj_field(tree, message, StringSlice("content"))
    if not ocr_response_content_present(tree, content):
        return False
    if not openai_compat_put_literal(
        writer, StringSlice('{"type":"message","role":')
    ):
        return False
    var role = pj_string_field(tree, message, StringSlice("role"))
    if role >= 0:
        if not openai_compat_put_json_string(writer, pj_text(tree, role)):
            return False
    elif not openai_compat_put_literal(writer, StringSlice('"assistant"')):
        return False
    return (
        openai_compat_put_literal(writer, StringSlice(',"content":'))
        and ocr_response_write_content(writer, tree, content)
        and openai_compat_put_byte(writer, 125)
    )


def ocr_response_write_tool_call(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    tool_call: Int64,
    function: Int64,
    name_index: Int64,
) -> Bool:
    var flat_name = pj_text(tree, name_index)
    var parts = openai_compat_split_tool_name_parts(flat_name)
    if not openai_compat_put_literal(
        writer, StringSlice('{"type":"function_call","call_id":')
    ):
        return False
    var call_id = pj_string_field(tree, tool_call, StringSlice("id"))
    if call_id >= 0:
        if not openai_compat_put_json_string(writer, pj_text(tree, call_id)):
            return False
    elif not openai_compat_put_literal(writer, StringSlice('"call_prodex"')):
        return False
    if not openai_compat_put_literal(writer, StringSlice(',"name":')):
        return False
    if not openai_compat_put_json_string(writer, parts[2]):
        return False
    if not openai_compat_put_literal(writer, StringSlice(',"arguments":')):
        return False

    var arguments = pj_field(tree, function, StringSlice("arguments"))
    var argument_text = pj_literal(StringSlice("{}"))
    if arguments >= 0:
        if pj_kind(tree, arguments) == JSON_STRING:
            argument_text = pj_text(tree, arguments)
        else:
            argument_text = js_raw_view(tree, arguments)
    if not openai_compat_put_json_string_with_rtk_value(
        writer, flat_name, argument_text
    ):
        return False
    if parts[0]:
        if not openai_compat_put_literal(writer, StringSlice(',"namespace":')):
            return False
        if not openai_compat_put_json_string(writer, parts[1]):
            return False
    return openai_compat_put_byte(writer, 125)


def ocr_response_u64(tree: ParsedJson, index: Int64) -> Tuple[Bool, UInt64]:
    if pj_kind(tree, index) != JSON_NUMBER:
        return (False, UInt64(0))
    var raw = js_raw_view(tree, index)
    if raw.len == 0:
        return (False, UInt64(0))
    var ptr = rich_view_ptr(raw)
    var value: UInt64 = 0
    var maximum_prefix: UInt64 = 1844674407370955161
    for offset in range(Int64(raw.len)):
        var digit = ptr[unsafe_offset=offset]
        if digit < 48 or digit > 57:
            return (False, UInt64(0))
        var numeric_digit = UInt64(digit - 48)
        if value > maximum_prefix or (
            value == maximum_prefix and numeric_digit > UInt64(5)
        ):
            return (False, UInt64(0))
        value = value * UInt64(10) + numeric_digit
    return (True, value)


def ocr_response_usage_values(
    tree: ParsedJson, response: Int64
) -> Tuple[Bool, UInt64, UInt64, UInt64]:
    var usage = pj_field(tree, response, StringSlice("usage"))
    if pj_kind(tree, usage) != JSON_OBJECT:
        return (False, UInt64(0), UInt64(0), UInt64(0))
    var input = pj_field(tree, usage, StringSlice("prompt_tokens"))
    if input < 0:
        input = pj_field(tree, usage, StringSlice("input_tokens"))
    var input_value = ocr_response_u64(tree, input)
    if not input_value[0]:
        return (False, UInt64(0), UInt64(0), UInt64(0))

    var output = pj_field(tree, usage, StringSlice("completion_tokens"))
    if output < 0:
        output = pj_field(tree, usage, StringSlice("output_tokens"))
    var output_value = ocr_response_u64(tree, output)
    var output_tokens = output_value[1] if output_value[0] else UInt64(0)
    var total = pj_field(tree, usage, StringSlice("total_tokens"))
    var total_value = ocr_response_u64(tree, total)
    var total_tokens = (
        total_value[1]
        if total_value[0]
        else input_value[1] + output_tokens
    )
    return (True, input_value[1], output_tokens, total_tokens)


def ocr_response_write(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    response: Int64,
    default_created_at: UInt64,
) -> Bool:
    if not openai_compat_put_literal(writer, StringSlice('{"id":')):
        return False
    var response_id = pj_string_field(tree, response, StringSlice("id"))
    if response_id >= 0:
        if not openai_compat_put_json_string(writer, pj_text(tree, response_id)):
            return False
    elif not openai_compat_put_literal(writer, StringSlice('"resp_prodex"')):
        return False
    if not openai_compat_put_literal(
        writer, StringSlice(',"object":"response","created_at":')
    ):
        return False
    var created = ocr_response_u64(
        tree, pj_field(tree, response, StringSlice("created"))
    )
    var created_at = created[1] if created[0] else default_created_at
    if not openai_compat_put_u64(writer, created_at):
        return False
    if not openai_compat_put_literal(writer, StringSlice(',"model":')):
        return False
    var model = pj_string_field(tree, response, StringSlice("model"))
    if model >= 0:
        if not openai_compat_put_json_string(writer, pj_text(tree, model)):
            return False
    elif not openai_compat_put_literal(writer, StringSlice('"unknown"')):
        return False
    if not openai_compat_put_literal(writer, StringSlice(',"output":[')):
        return False

    var choices = pj_field(tree, response, StringSlice("choices"))
    var choice = pj_child(tree, choices) if pj_kind(tree, choices) == JSON_ARRAY else -1
    var message = pj_field(tree, choice, StringSlice("message"))
    var output_count: Int64 = 0
    if message >= 0 and ocr_response_content_present(
        tree, pj_field(tree, message, StringSlice("content"))
    ):
        if not ocr_response_write_message(writer, tree, message):
            return False
        output_count += 1
    var tool_calls = pj_field(tree, message, StringSlice("tool_calls"))
    if pj_kind(tree, tool_calls) == JSON_ARRAY:
        var tool_call = pj_child(tree, tool_calls)
        while tool_call >= 0:
            var function = pj_field(tree, tool_call, StringSlice("function"))
            var name = pj_string_field(tree, function, StringSlice("name"))
            if name >= 0:
                if output_count > 0 and not openai_compat_put_byte(writer, 44):
                    return False
                if not ocr_response_write_tool_call(
                    writer, tree, tool_call, function, name
                ):
                    return False
                output_count += 1
            tool_call = pj_next(tree, tool_call)
    if not openai_compat_put_byte(writer, 93):
        return False

    var usage = ocr_response_usage_values(tree, response)
    if usage[0]:
        if not openai_compat_put_literal(writer, StringSlice(',"usage":')):
            return False
        if not openai_compat_write_usage_values(
            writer, usage[1], usage[2], usage[3]
        ):
            return False
    return openai_compat_put_byte(writer, 125)


def ocr_response_stream_event_write(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    response: Int64,
) -> Tuple[Bool, Bool]:
    var choices = pj_field(tree, response, StringSlice("choices"))
    var choice = pj_child(tree, choices) if pj_kind(tree, choices) == JSON_ARRAY else -1
    if choice < 0:
        return (False, True)
    var delta = pj_field(tree, choice, StringSlice("delta"))
    var tool_calls = pj_field(tree, delta, StringSlice("tool_calls"))
    var tool_call = pj_child(tree, tool_calls) if pj_kind(tree, tool_calls) == JSON_ARRAY else -1
    var function = pj_field(tree, tool_call, StringSlice("function"))
    var arguments = pj_field(tree, function, StringSlice("arguments"))
    if pj_kind(tree, arguments) == JSON_STRING:
        if not openai_compat_put_literal(
            writer,
            StringSlice(
                'event: response.function_call_arguments.delta\ndata: {'
            ),
        ):
            return (True, False)
        var call_id = pj_string_field(tree, tool_call, StringSlice("id"))
        if call_id >= 0:
            if not openai_compat_put_literal(writer, StringSlice('"call_id":')):
                return (True, False)
            if not openai_compat_put_json_string(writer, pj_text(tree, call_id)):
                return (True, False)
            if not openai_compat_put_byte(writer, 44):
                return (True, False)
        var name = pj_string_field(tree, function, StringSlice("name"))
        var flat_name = pj_text(tree, name) if name >= 0 else pj_literal(StringSlice(""))
        return (
            True,
            openai_compat_put_literal(writer, StringSlice('"delta":'))
            and openai_compat_put_json_string_with_rtk_value(
                writer, flat_name, pj_text(tree, arguments)
            )
            and openai_compat_put_literal(
                writer,
                StringSlice(
                    ',"type":"response.function_call_arguments.delta"}\n\n'
                ),
            ),
        )

    var text = pj_string_field(tree, delta, StringSlice("content"))
    if text >= 0:
        return (
            True,
            openai_compat_put_literal(
                writer,
                StringSlice('event: response.output_text.delta\ndata: {"delta":'),
            )
            and openai_compat_put_json_string(writer, pj_text(tree, text))
            and openai_compat_put_literal(
                writer,
                StringSlice(
                    ',"type":"response.output_text.delta"}\n\n'
                ),
            ),
        )

    var finish_reason = pj_field(tree, choice, StringSlice("finish_reason"))
    if finish_reason >= 0 and pj_kind(tree, finish_reason) != JSON_NULL:
        return (
            True,
            openai_compat_put_literal(
                writer,
                StringSlice('event: response.completed\ndata: {}\n\n'),
            ),
        )
    return (False, True)


@export("prodex_mojo_openai_chat_response_v1")
def prodex_mojo_openai_chat_response_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != OPENAI_CHAT_RESPONSE_ABI:
        return 4
    if operation < 0 or operation > 1 or flag != 0 or measuring < 0 or measuring > 1 or raw_length < 0:
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
    var response = pj_field(tree, 0, StringSlice("response"))
    if response < 0:
        return 1
    var default_created_at: UInt64 = 0
    if operation == 0:
        var default_node = pj_field(tree, 0, StringSlice("fallback_created_at"))
        var fallback = ocr_response_u64(tree, default_node)
        if not fallback[0]:
            return 1
        default_created_at = fallback[1]

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = OpenAiCompatWriter(output, capacity, 0, measuring == 1)
    var writer_ptr = Pointer(to=writer)
    var present = True
    var success = True
    if operation == 0:
        success = ocr_response_write(writer_ptr, tree, response, default_created_at)
    else:
        var event = ocr_response_stream_event_write(writer_ptr, tree, response)
        present = event[0]
        success = event[1]
    var status = openai_compat_writer_status(writer_ptr, success)
    if status != 0:
        return status
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    meta[unsafe_offset=0] = Int64(present)
    meta[unsafe_offset=1] = writer.written
    return 0
