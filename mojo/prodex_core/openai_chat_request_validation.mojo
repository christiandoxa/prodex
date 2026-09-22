from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr
from parsed_json import (
    ParsedJson,
    JSON_FALSE,
    JSON_NUMBER,
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
)
from json_sink import JsonSink, js_byte, js_view, js_literal, js_raw_view

def ocr_error(
    sink: Pointer[mut=True, JsonSink, _],
    provider: ProdexRichStringView,
    suffix: StringSlice,
):
    js_byte(sink, 69)
    js_view(sink, provider)
    js_literal(sink, suffix)


def ocr_error_literal(
    sink: Pointer[mut=True, JsonSink, _],
    message: StringSlice,
):
    js_byte(sink, 69)
    js_literal(sink, message)


def ocr_number_gt_one(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) != JSON_NUMBER:
        return False
    var raw = js_raw_view(tree, index)
    if raw.len == 0:
        return False
    var ptr = rich_view_ptr(raw)
    var value: UInt64 = 0
    for offset in range(Int64(raw.len)):
        var byte = ptr[unsafe_offset=offset]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > UInt64(1844674407370955161):
            return False
        if value == UInt64(1844674407370955161) and digit > 5:
            return False
        value = value * 10 + digit
    return value > 1


def ocr_tool_choice_supported(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_STRING:
        return (
            pj_is["auto"](tree, index)
            or pj_is["none"](tree, index)
            or pj_is["required"](tree, index)
        )
    if pj_kind(tree, index) != JSON_OBJECT:
        return False
    return pj_is["function"](tree, pj_field(tree, index, StringSlice("type")))


def ocr_tools_function_only(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) != JSON_ARRAY:
        return True
    var tool = pj_child(tree, index)
    while tool >= 0:
        if not pj_is["function"](tree, pj_field(tree, tool, StringSlice("type"))):
            return False
        tool = pj_next(tree, tool)
    return True


def ocr_input_has_custom(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_ARRAY:
        var child = pj_child(tree, index)
        while child >= 0:
            if ocr_input_has_custom(tree, child):
                return True
            child = pj_next(tree, child)
        return False
    if pj_kind(tree, index) != JSON_OBJECT:
        return False
    var kind = pj_field(tree, index, StringSlice("type"))
    if pj_is["custom_tool_call"](tree, kind) or pj_is["tool_search_call"](tree, kind):
        return True
    var content = pj_field(tree, index, StringSlice("content"))
    return content >= 0 and ocr_input_has_custom(tree, content)


def ocr_content_has_non_text(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_STRING:
        return False
    if pj_kind(tree, index) == JSON_ARRAY:
        var item = pj_child(tree, index)
        while item >= 0:
            if pj_kind(tree, item) == JSON_OBJECT:
                var kind = pj_field(tree, item, StringSlice("type"))
                if not (
                    pj_is["input_text"](tree, kind)
                    or pj_is["output_text"](tree, kind)
                    or pj_is["text"](tree, kind)
                ):
                    return True
            item = pj_next(tree, item)
        return False
    if pj_kind(tree, index) == JSON_OBJECT:
        var kind = pj_field(tree, index, StringSlice("type"))
        return not (
            pj_is["input_text"](tree, kind)
            or pj_is["output_text"](tree, kind)
            or pj_is["text"](tree, kind)
        )
    return False


def ocr_input_has_non_text(tree: ParsedJson, index: Int64) -> Bool:
    if pj_kind(tree, index) == JSON_ARRAY:
        var child = pj_child(tree, index)
        while child >= 0:
            if ocr_input_has_non_text(tree, child):
                return True
            child = pj_next(tree, child)
        return False
    if pj_kind(tree, index) != JSON_OBJECT:
        return False
    var kind = pj_field(tree, index, StringSlice("type"))
    if pj_kind(tree, kind) == JSON_STRING and not (
        pj_is["message"](tree, kind)
        or pj_is["input_text"](tree, kind)
        or pj_is["output_text"](tree, kind)
        or pj_is["function_call"](tree, kind)
        or pj_is["function_call_output"](tree, kind)
    ):
        if (
            pj_field(tree, index, StringSlice("text")) >= 0
            or pj_field(tree, index, StringSlice("content")) >= 0
            or pj_is["input_image"](tree, kind)
            or pj_is["input_audio"](tree, kind)
        ):
            return True
    var content = pj_field(tree, index, StringSlice("content"))
    return content >= 0 and ocr_content_has_non_text(tree, content)

def ocr_validate(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    request: Int64,
    provider: ProdexRichStringView,
) -> Bool:
    if pj_kind(tree, request) != JSON_OBJECT:
        ocr_error_literal(sink, StringSlice("Responses request body must be a JSON object"))
        return False
    if pj_field(tree, request, StringSlice("messages")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat expects Responses input, not raw chat-completions messages"))
        return False
    if pj_field(tree, request, StringSlice("response_format")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate response_format controls"))
        return False
    if pj_field(tree, request, StringSlice("reasoning")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not map Responses reasoning controls"))
        return False
    if pj_field(tree, request, StringSlice("previous_response_id")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not map previous_response_id continuation state"))
        return False
    var text = pj_field(tree, request, StringSlice("text"))
    if pj_kind(tree, text) == JSON_OBJECT and pj_field(tree, text, StringSlice("format")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate text.format controls"))
        return False
    if ocr_number_gt_one(tree, pj_field(tree, request, StringSlice("n"))):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat returns only the first choice and does not support n>1"))
        return False
    if pj_field(tree, request, StringSlice("metadata")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate request metadata"))
        return False
    if pj_field(tree, request, StringSlice("safety_identifier")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate safety_identifier"))
        return False
    if pj_field(tree, request, StringSlice("web_search_options")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate web_search_options"))
        return False
    var tools = pj_field(tree, request, StringSlice("tools"))
    if not ocr_tools_function_only(tree, tools):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat only forwards function tools"))
        return False
    var choice = pj_field(tree, request, StringSlice("tool_choice"))
    if choice >= 0 and not ocr_tool_choice_supported(tree, choice):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat only forwards function tool_choice controls"))
        return False
    if pj_kind(tree, pj_field(tree, request, StringSlice("parallel_tool_calls"))) == JSON_FALSE:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not prove a compatible parallel_tool_calls=false control"))
        return False
    if (
        pj_field(tree, request, StringSlice("logprobs")) >= 0
        or pj_field(tree, request, StringSlice("top_logprobs")) >= 0
    ):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate logprobs controls"))
        return False
    if pj_field(tree, request, StringSlice("stop_sequences")) >= 0:
        ocr_error(sink, provider, StringSlice(" Responses chat-compat does not translate stop_sequences"))
        return False
    var input = pj_field(tree, request, StringSlice("input"))
    if input >= 0 and ocr_input_has_custom(tree, input):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat only translates message/function-call history items"))
        return False
    if input >= 0 and ocr_input_has_non_text(tree, input):
        ocr_error(sink, provider, StringSlice(" Responses chat-compat currently translates only text input content"))
        return False
    return True
