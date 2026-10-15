from std.memory import Pointer

from parsed_json import (
    ParsedJson,
    JSON_ARRAY,
    JSON_STRING,
    pj_child,
    pj_field,
    pj_is,
    pj_kind,
    pj_next,
    pj_string_field,
    pj_text,
    pj_trim,
)
from json_sink import js_raw_view
from openai_compat import (
    OpenAiCompatWriter,
    openai_compat_put_byte,
    openai_compat_put_json_string,
    openai_compat_put_literal,
    openai_compat_put_u64,
    openai_compat_put_view,
)
from openai_chat_response_common import ocr_response_u64
from rich_text import rich_view_ptr, rich_view_prefix
from rich_types import ProdexRichStringView


def ocr_bridge_write_message(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    message: Int64,
) -> Bool:
    var content = pj_field(tree, message, StringSlice("content"))
    if pj_kind(tree, content) != JSON_STRING or pj_text(tree, content).len == 0:
        return False
    return (
        openai_compat_put_literal(
            writer, StringSlice('{"type":"message","role":"assistant","content":')
        )
        and openai_compat_put_literal(writer, StringSlice('[{"type":"output_text","text":'))
        and openai_compat_put_json_string(writer, pj_text(tree, content))
        and openai_compat_put_literal(writer, StringSlice("}]}"))
    )


def ocr_bridge_split_flat_name(
    name: ProdexRichStringView,
) -> Tuple[Bool, ProdexRichStringView, ProdexRichStringView]:
    var ptr = rich_view_ptr(name)
    var separator: Int64 = -1
    for index in range(max(Int64(name.len) - 1, 0)):
        if ptr[unsafe_offset=index] == 45 and ptr[unsafe_offset=index + 1] == 45:
            separator = Int64(index)
    if separator >= 0:
        var namespace = ProdexRichStringView(name.ptr, UInt(separator))
        var local = ProdexRichStringView(
            name.ptr + UInt(separator + 2), name.len - UInt(separator + 2)
        )
        if pj_trim(namespace).len > 0 and pj_trim(local).len > 0:
            return (True, namespace.copy(), local.copy())
    if not rich_view_prefix["mcp__"](name, False):
        return (False, ProdexRichStringView(0, 0), name.copy())
    separator = -1
    var scan_end = Int(name.len) - 1
    if scan_end < 5:
        scan_end = 5
    for index in range(5, scan_end):
        if ptr[unsafe_offset=index] == 95 and ptr[unsafe_offset=index + 1] == 95:
            separator = Int64(index)
    if separator < 0:
        return (False, ProdexRichStringView(0, 0), name.copy())
    var namespace = ProdexRichStringView(name.ptr, UInt(separator))
    var local = ProdexRichStringView(
        name.ptr + UInt(separator + 2), name.len - UInt(separator + 2)
    )
    if pj_trim(local).len == 0:
        return (False, ProdexRichStringView(0, 0), name.copy())
    return (True, namespace.copy(), local.copy())


def ocr_bridge_write_tool_plan(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    plan: Int64,
) -> Bool:
    var kind = pj_string_field(tree, plan, StringSlice("kind"))
    var call_id = pj_string_field(tree, plan, StringSlice("call_id"))
    if kind < 0 or call_id < 0:
        return False
    if pj_is["tool_search"](tree, kind):
        var arguments = pj_string_field(tree, plan, StringSlice("arguments"))
        if arguments < 0:
            return False
        return (
            openai_compat_put_literal(
                writer, StringSlice('{"type":"tool_search_call","call_id":')
            )
            and openai_compat_put_json_string(writer, pj_text(tree, call_id))
            and openai_compat_put_literal(writer, StringSlice(',"execution":"client","arguments":'))
            and openai_compat_put_view(writer, pj_text(tree, arguments))
            and openai_compat_put_byte(writer, 125)
        )
    if pj_is["apply_patch"](tree, kind):
        var name = pj_string_field(tree, plan, StringSlice("name"))
        var input = pj_string_field(tree, plan, StringSlice("input"))
        if name < 0 or input < 0:
            return False
        return (
            openai_compat_put_literal(
                writer, StringSlice('{"type":"custom_tool_call","call_id":')
            )
            and openai_compat_put_json_string(writer, pj_text(tree, call_id))
            and openai_compat_put_literal(writer, StringSlice(',"name":'))
            and openai_compat_put_json_string(writer, pj_text(tree, name))
            and openai_compat_put_literal(writer, StringSlice(',"input":'))
            and openai_compat_put_json_string(writer, pj_text(tree, input))
            and openai_compat_put_byte(writer, 125)
        )
    if not pj_is["function"](tree, kind):
        return False
    var name = pj_string_field(tree, plan, StringSlice("name"))
    var arguments = pj_string_field(tree, plan, StringSlice("arguments"))
    if name < 0 or arguments < 0:
        return False
    var parts = ocr_bridge_split_flat_name(pj_text(tree, name))
    if not (
        openai_compat_put_literal(
            writer, StringSlice('{"type":"function_call","call_id":')
        )
        and openai_compat_put_json_string(writer, pj_text(tree, call_id))
        and openai_compat_put_literal(writer, StringSlice(',"name":'))
        and openai_compat_put_json_string(writer, parts[2])
        and openai_compat_put_literal(writer, StringSlice(',"arguments":'))
        and openai_compat_put_json_string(writer, pj_text(tree, arguments))
    ):
        return False
    if parts[0]:
        if not (
            openai_compat_put_literal(writer, StringSlice(',"namespace":'))
            and openai_compat_put_json_string(writer, parts[1])
        ):
            return False
    var signature = pj_string_field(tree, plan, StringSlice("thought_signature"))
    if signature >= 0:
        if not (
            openai_compat_put_literal(writer, StringSlice(',"gemini_thought_signature":'))
            and openai_compat_put_json_string(writer, pj_text(tree, signature))
        ):
            return False
    return openai_compat_put_byte(writer, 125)


def ocr_bridge_response_write(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    tree: ParsedJson,
    context: Int64,
) -> Bool:
    var response = pj_field(tree, context, StringSlice("response"))
    var fallback_id = pj_string_field(tree, context, StringSlice("fallback_response_id"))
    var response_id = pj_string_field(tree, response, StringSlice("id"))
    if response_id < 0:
        response_id = fallback_id
    var fallback_created = ocr_response_u64(
        tree, pj_field(tree, context, StringSlice("fallback_created_at"))
    )
    if not fallback_created[0]:
        return False
    var created = ocr_response_u64(tree, pj_field(tree, response, StringSlice("created")))
    var created_at = created[1] if created[0] else fallback_created[1]
    var default_model = pj_string_field(tree, context, StringSlice("default_model"))
    var model = pj_string_field(tree, response, StringSlice("model"))
    if model < 0:
        model = default_model
    if response_id < 0 or model < 0:
        return False
    if not (
        openai_compat_put_literal(writer, StringSlice('{"id":'))
        and openai_compat_put_json_string(writer, pj_text(tree, response_id))
        and openai_compat_put_literal(
            writer, StringSlice(',"object":"response","created_at":')
        )
        and openai_compat_put_u64(writer, created_at)
        and openai_compat_put_literal(writer, StringSlice(',"model":'))
        and openai_compat_put_json_string(writer, pj_text(tree, model))
        and openai_compat_put_literal(writer, StringSlice(',"output":['))
    ):
        return False
    var choices = pj_field(tree, response, StringSlice("choices"))
    var choice = pj_child(tree, choices) if pj_kind(tree, choices) == JSON_ARRAY else -1
    var message = pj_field(tree, choice, StringSlice("message"))
    var output_count: Int64 = 0
    var message_content = pj_field(tree, message, StringSlice("content"))
    if message >= 0 and pj_kind(tree, message_content) == JSON_STRING and pj_text(tree, message_content).len > 0:
        if not ocr_bridge_write_message(writer, tree, message):
            return False
        output_count = 1
    var plans = pj_field(tree, context, StringSlice("tool_plans"))
    if pj_kind(tree, plans) == JSON_ARRAY:
        var plan = pj_child(tree, plans)
        while plan >= 0:
            if output_count > 0 and not openai_compat_put_byte(writer, 44):
                return False
            if not ocr_bridge_write_tool_plan(writer, tree, plan):
                return False
            output_count += 1
            plan = pj_next(tree, plan)
    if not openai_compat_put_byte(writer, 93):
        return False
    var error = pj_string_field(tree, context, StringSlice("tool_call_error"))
    if error >= 0:
        if not (
            openai_compat_put_literal(writer, StringSlice(',"status":"failed","error":{"code":"invalid_tool_call_arguments","message":'))
            and openai_compat_put_json_string(writer, pj_text(tree, error))
            and openai_compat_put_byte(writer, 125)
        ):
            return False
    var usage = pj_field(tree, context, StringSlice("usage"))
    if usage >= 0:
        if not (
            openai_compat_put_literal(writer, StringSlice(',"usage":'))
            and openai_compat_put_view(writer, js_raw_view(tree, usage))
        ):
            return False
    var metadata = pj_field(tree, context, StringSlice("metadata"))
    if metadata >= 0:
        var key = pj_string_field(tree, context, StringSlice("provider_metadata_key"))
        if key < 0 or not (
            openai_compat_put_literal(writer, StringSlice(',"metadata":{'))
            and openai_compat_put_json_string(writer, pj_text(tree, key))
            and openai_compat_put_byte(writer, 58)
            and openai_compat_put_view(writer, js_raw_view(tree, metadata))
            and openai_compat_put_byte(writer, 125)
        ):
            return False
    return openai_compat_put_byte(writer, 125)
