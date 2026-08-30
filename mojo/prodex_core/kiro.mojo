from std.memory import Pointer

from rich_text import (
    rich_trim_bounds,
    rich_view_matches_literal,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime KIRO_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime KIRO_KERNEL_STATUS_OK: Int64 = 0
comptime KIRO_KERNEL_STATUS_INVALID: Int64 = 1
comptime KIRO_KERNEL_STATUS_UTF8: Int64 = 2
comptime KIRO_KERNEL_STATUS_CAPACITY: Int64 = 3
comptime KIRO_KERNEL_STATUS_ABI: Int64 = 4

comptime KIRO_REQUEST_BODY: Int64 = 1
comptime KIRO_PROMPT_SECTION: Int64 = 2
comptime KIRO_RESPONSE_MESSAGE_ITEM: Int64 = 3
comptime KIRO_RESPONSE_FUNCTION_CALL_ITEM: Int64 = 4
comptime KIRO_RESPONSE_FUNCTION_CALL_OUTPUT_ITEM: Int64 = 5
comptime KIRO_LEGACY_FUNCTION_TOOL: Int64 = 6
comptime KIRO_LEGACY_TOOL_CHOICE: Int64 = 7
comptime KIRO_CHAT_COMPLETION_RESPONSE: Int64 = 8
comptime KIRO_ANTHROPIC_TOOL_USE_BLOCK: Int64 = 9
comptime KIRO_ANTHROPIC_RESPONSE: Int64 = 10
comptime KIRO_CHAT_COMPLETION_CHUNK: Int64 = 11
comptime KIRO_CHAT_ROLE_DELTA: Int64 = 12
comptime KIRO_CHAT_EMPTY_DELTA: Int64 = 13
comptime KIRO_CHAT_TEXT_DELTA: Int64 = 14
comptime KIRO_CHAT_REASONING_DELTA: Int64 = 15
comptime KIRO_CHAT_TOOL_CALL_DELTA: Int64 = 16
comptime KIRO_OUTPUT_TEXT_DELTA_EVENT: Int64 = 17
comptime KIRO_RESPONSE_CREATED_EVENT: Int64 = 18
comptime KIRO_OUTPUT_ITEM_ADDED_EVENT: Int64 = 19
comptime KIRO_OUTPUT_ITEM_DONE_EVENT: Int64 = 20
comptime KIRO_RESPONSE_COMPLETED_EVENT: Int64 = 21
comptime KIRO_RESPONSE_FAILED_EVENT: Int64 = 22
comptime KIRO_RESPONSE_INCOMPLETE_EVENT: Int64 = 23
comptime KIRO_TOOL_CALL_ARGUMENTS_DELTA_CHAT_VALUE: Int64 = 24
comptime KIRO_USAGE_UPDATE: Int64 = 25
comptime KIRO_STREAM_TOOL_ARGUMENTS: Int64 = 26
comptime KIRO_FINISH_REASON: Int64 = 27
comptime KIRO_CHAT_TOOL_CALL_ITEM: Int64 = 28


@fieldwise_init
struct ProdexKiroKernelInput(Copyable):
    var operation: Int64
    var sequence_number: UInt64
    var created_at: UInt64
    var request_id: UInt64
    var used: UInt64
    var size: UInt64
    var include_role: Int64
    var has_tool_calls: Int64
    var response_id_present: Int64
    var model_present: Int64
    var role_present: Int64
    var content_present: Int64
    var reason_present: Int64
    var call_id_present: Int64
    var name_present: Int64
    var arguments_present: Int64
    var input_present: Int64
    var output_present: Int64
    var tool_calls_present: Int64
    var requested_model_present: Int64
    var metadata_present: Int64
    var finish_reason_present: Int64
    var status_present: Int64
    var error_present: Int64
    var extra_present: Int64
    var incomplete_reason_present: Int64
    var response_id: ProdexRichStringView
    var model: ProdexRichStringView
    var role: ProdexRichStringView
    var content: ProdexRichStringView
    var reason: ProdexRichStringView
    var call_id: ProdexRichStringView
    var name: ProdexRichStringView
    var arguments: ProdexRichStringView
    var input: ProdexRichStringView
    var output: ProdexRichStringView
    var tool_calls: ProdexRichStringView
    var requested_model: ProdexRichStringView
    var metadata: ProdexRichStringView
    var finish_reason: ProdexRichStringView
    var status: ProdexRichStringView
    var error: ProdexRichStringView
    var extra: ProdexRichStringView
    var incomplete_reason: ProdexRichStringView


@fieldwise_init
struct KiroResponseWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def kiro_put_byte(
    writer: Pointer[mut=True, KiroResponseWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def kiro_put_literal(
    writer: Pointer[mut=True, KiroResponseWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not kiro_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def kiro_put_hex_byte(
    writer: Pointer[mut=True, KiroResponseWriter, _], value: UInt8
) -> Bool:
    var high = (value >> 4) & 15
    var low = value & 15
    if high < 10:
        high += 48
    else:
        high += 87
    if low < 10:
        low += 48
    else:
        low += 87
    return kiro_put_byte(writer, high) and kiro_put_byte(writer, low)


def kiro_put_json_string_with_prefix(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    prefix: StringSlice,
    view: ProdexRichStringView,
) -> Bool:
    if not kiro_put_byte(writer, 34) or not kiro_put_literal(writer, prefix):
        return False
    if view.len > 0:
        var ptr = rich_view_ptr(view)
        for index in range(Int64(view.len)):
            var value = ptr[unsafe_offset=index]
            if value == 34 or value == 92:
                if not kiro_put_byte(writer, 92) or not kiro_put_byte(writer, value):
                    return False
            elif value == 8:
                if not kiro_put_literal(writer, StringSlice("\\b")):
                    return False
            elif value == 9:
                if not kiro_put_literal(writer, StringSlice("\\t")):
                    return False
            elif value == 10:
                if not kiro_put_literal(writer, StringSlice("\\n")):
                    return False
            elif value == 12:
                if not kiro_put_literal(writer, StringSlice("\\f")):
                    return False
            elif value == 13:
                if not kiro_put_literal(writer, StringSlice("\\r")):
                    return False
            elif value < 32:
                if not kiro_put_literal(writer, StringSlice("\\u00")) or not kiro_put_hex_byte(writer, value):
                    return False
            elif not kiro_put_byte(writer, value):
                return False
    return kiro_put_byte(writer, 34)


def kiro_put_json_string(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return kiro_put_json_string_with_prefix(writer, StringSlice(""), view)


def kiro_put_view(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len == 0:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if not kiro_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def kiro_put_view_range(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not kiro_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def kiro_put_trimmed_view(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var bounds = rich_trim_bounds(view)
    return kiro_put_view_range(writer, view, bounds[0], bounds[1])


def kiro_put_u64(
    writer: Pointer[mut=True, KiroResponseWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return kiro_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not kiro_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def kiro_put_optional_json_string(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    key: StringSlice,
    present: Int64,
    view: ProdexRichStringView,
) -> Bool:
    if present == 0:
        return True
    return kiro_put_literal(writer, key) and kiro_put_json_string(writer, view)


def kiro_put_optional_view(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    key: StringSlice,
    present: Int64,
    view: ProdexRichStringView,
) -> Bool:
    if present == 0:
        return True
    return kiro_put_literal(writer, key) and kiro_put_view(writer, view)


def kiro_put_extra_fields(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    present: Int64,
    view: ProdexRichStringView,
    has_fields: Bool,
) -> Bool:
    if present == 0:
        return True
    if view.len < 2:
        return False
    if view.len == 2:
        return True
    if has_fields and not kiro_put_byte(writer, 44):
        return False
    return kiro_put_view_range(writer, view, 1, Int64(view.len) - 1)


def kiro_put_event_prefix(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    event_type: StringSlice,
    sequence_number: UInt64,
    created_at: UInt64,
    include_created_at: Bool,
) -> Bool:
    if not kiro_put_literal(writer, StringSlice('{"type":"')):
        return False
    if not kiro_put_literal(writer, event_type):
        return False
    if not kiro_put_literal(writer, StringSlice('","sequence_number":')) or not kiro_put_u64(writer, sequence_number):
        return False
    if include_created_at:
        if not kiro_put_literal(writer, StringSlice(',"created_at":')) or not kiro_put_u64(writer, created_at):
            return False
    return True


def kiro_put_prompt_role(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    role: ProdexRichStringView,
) -> Bool:
    if rich_view_matches_literal["system"](role, False):
        return kiro_put_literal(writer, StringSlice("System"))
    if rich_view_matches_literal["assistant"](role, False):
        return kiro_put_literal(writer, StringSlice("Assistant"))
    if rich_view_matches_literal["tool"](role, False):
        return kiro_put_literal(writer, StringSlice("Tool"))
    return kiro_put_literal(writer, StringSlice("User"))


def kiro_put_chat_finish_reason(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    if input.has_tool_calls == 1:
        return kiro_put_literal(writer, StringSlice("tool_calls"))
    if input.incomplete_reason_present == 1 and rich_view_matches_literal[
        "max_output_tokens"
    ](input.incomplete_reason, False):
        return kiro_put_literal(writer, StringSlice("length"))
    return kiro_put_literal(writer, StringSlice("stop"))


def kiro_put_anthropic_stop_reason(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    if input.has_tool_calls == 1:
        return kiro_put_literal(writer, StringSlice('"tool_use"'))
    if input.reason_present == 1:
        if rich_view_matches_literal["max_output_tokens"](input.reason, False) or rich_view_matches_literal[
            "max_tokens"
        ](input.reason, False):
            return kiro_put_literal(writer, StringSlice('"max_tokens"'))
        if rich_view_matches_literal["tool_use"](input.reason, False):
            return kiro_put_literal(writer, StringSlice('"tool_use"'))
    return kiro_put_literal(writer, StringSlice('"end_turn"'))


def kiro_write_operation(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    var operation = input.operation
    if operation == KIRO_REQUEST_BODY:
        var has_fields = False
        if not kiro_put_byte(writer, 123):
            return False
        if input.model_present == 1:
            if not kiro_put_literal(writer, StringSlice('"model":')) or not kiro_put_json_string(writer, input.model):
                return False
            has_fields = True
        if input.input_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"input":')) or not kiro_put_view(writer, input.input):
                return False
            has_fields = True
        if not kiro_put_extra_fields(writer, input.extra_present, input.extra, has_fields):
            return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_PROMPT_SECTION:
        var content_bounds = rich_trim_bounds(input.content)
        var tool_bounds = rich_trim_bounds(input.tool_calls)
        var has_content = content_bounds[1] > content_bounds[0]
        var has_tools = input.tool_calls_present == 1 and tool_bounds[1] > tool_bounds[0]
        if not has_content and not has_tools:
            return False
        if not kiro_put_prompt_role(writer, input.role) or not kiro_put_literal(writer, StringSlice(":\n")):
            return False
        if has_content and not kiro_put_view_range(writer, input.content, content_bounds[0], content_bounds[1]):
            return False
        if has_tools:
            if has_content and not kiro_put_byte(writer, 10):
                return False
            if not kiro_put_view_range(writer, input.tool_calls, tool_bounds[0], tool_bounds[1]):
                return False
        return True
    if operation == KIRO_RESPONSE_MESSAGE_ITEM:
        if not kiro_put_literal(writer, StringSlice('{"type":"message","role":')):
            return False
        if input.role_present == 1:
            if not kiro_put_json_string(writer, input.role):
                return False
        elif not kiro_put_literal(writer, StringSlice('"user"')):
            return False
        return (
            kiro_put_literal(writer, StringSlice(',"content":[{"type":"input_text","text":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice("}]}"))
        )
    if operation == KIRO_RESPONSE_FUNCTION_CALL_ITEM:
        if not kiro_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
            return False
        if input.call_id_present == 1:
            if not kiro_put_json_string(writer, input.call_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"call_kiro"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"name":')):
            return False
        if input.name_present == 1:
            if not kiro_put_json_string(writer, input.name):
                return False
        elif not kiro_put_literal(writer, StringSlice('"tool_call"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"arguments":')):
            return False
        if input.arguments_present == 1:
            if not kiro_put_json_string(writer, input.arguments):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice('"{}"')):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_RESPONSE_FUNCTION_CALL_OUTPUT_ITEM:
        if not kiro_put_literal(writer, StringSlice('{"type":"function_call_output","call_id":')):
            return False
        if input.call_id_present == 1:
            if not kiro_put_json_string(writer, input.call_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"call_kiro"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"output":')):
            return False
        if input.output_present == 1:
            if not kiro_put_json_string(writer, input.output):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice('""')):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_LEGACY_FUNCTION_TOOL:
        if not kiro_put_literal(writer, StringSlice('{"type":"function","function":{"name":')):
            return False
        if not kiro_put_json_string(writer, input.name):
            return False
        if input.content_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"description":')) or not kiro_put_json_string(writer, input.content):
                return False
        if input.input_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"parameters":')) or not kiro_put_view(writer, input.input):
                return False
        return kiro_put_literal(writer, StringSlice("}}"))
    if operation == KIRO_LEGACY_TOOL_CHOICE:
        if input.role_present == 1:
            return kiro_put_json_string(writer, input.role)
        return (
            kiro_put_literal(writer, StringSlice('{"type":"function","function":{"name":'))
            and kiro_put_json_string(writer, input.name)
            and kiro_put_literal(writer, StringSlice("}}"))
        )
    if operation == KIRO_CHAT_COMPLETION_RESPONSE:
        if not kiro_put_literal(writer, StringSlice('{"id":')):
            return False
        if input.response_id_present == 1:
            if not kiro_put_json_string_with_prefix(writer, StringSlice("chatcmpl_"), input.response_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"chatcmpl_kiro_')) or not kiro_put_u64(writer, input.request_id) or not kiro_put_byte(writer, 34):
            return False
        if not kiro_put_literal(writer, StringSlice(',"object":"chat.completion","created":')) or not kiro_put_u64(writer, input.created_at):
            return False
        if not kiro_put_literal(writer, StringSlice(',"model":')):
            return False
        if input.model_present == 1:
            if not kiro_put_json_string(writer, input.model):
                return False
        elif not kiro_put_literal(writer, StringSlice('"kiro-cli"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"choices":[{"index":0,"message":{"role":"assistant","content":')):
            return False
        if input.content_present == 1:
            if not kiro_put_view(writer, input.content):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice("null")):
                return False
        if input.tool_calls_present == 1 and input.has_tool_calls == 1:
            if not kiro_put_literal(writer, StringSlice(',"tool_calls":')) or not kiro_put_view(writer, input.tool_calls):
                return False
        if input.reason_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"reasoning_content":')) or not kiro_put_json_string(writer, input.reason):
                return False
        if input.status_present == 1 and rich_view_matches_literal["failed"](input.status, False) and input.error_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"refusal":')) or not kiro_put_json_string(writer, input.error):
                return False
        if not kiro_put_literal(writer, StringSlice('},"finish_reason":"')):
            return False
        if not kiro_put_chat_finish_reason(writer, input) or not kiro_put_literal(writer, StringSlice('"}]')):
            return False
        if input.requested_model_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"requested_model":')) or not kiro_put_json_string(writer, input.requested_model):
                return False
        if input.metadata_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"metadata":')) or not kiro_put_view(writer, input.metadata):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_ANTHROPIC_TOOL_USE_BLOCK:
        if not kiro_put_literal(writer, StringSlice('{"type":"tool_use","id":')):
            return False
        if not kiro_put_json_string(writer, input.call_id) or not kiro_put_literal(writer, StringSlice(',"name":')):
            return False
        if not kiro_put_json_string(writer, input.name) or not kiro_put_literal(writer, StringSlice(',"input":')):
            return False
        if input.input_present == 1:
            if not kiro_put_view(writer, input.input):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice("{}")):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_ANTHROPIC_RESPONSE:
        if not kiro_put_literal(writer, StringSlice('{"id":')):
            return False
        if input.response_id_present == 1:
            if not kiro_put_json_string(writer, input.response_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"msg_kiro"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"type":"message","role":"assistant","model":')):
            return False
        if input.requested_model_present == 1:
            if not kiro_put_json_string(writer, input.requested_model):
                return False
        elif not kiro_put_literal(writer, StringSlice('"kiro-cli"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"content":[')):
            return False
        var content_written = False
        if input.tool_calls_present == 1 and input.tool_calls.len > 2:
            if not kiro_put_view_range(writer, input.tool_calls, 1, Int64(input.tool_calls.len) - 1):
                return False
            content_written = True
        if input.content_present == 1 and input.content.len > 0:
            if content_written and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('{"type":"text","text":')) or not kiro_put_json_string(writer, input.content) or not kiro_put_byte(writer, 125):
                return False
        if not kiro_put_literal(writer, StringSlice('],"stop_reason":')) or not kiro_put_anthropic_stop_reason(writer, input):
            return False
        return (
            kiro_put_literal(writer, StringSlice(',"stop_sequence":null,"usage":{"input_tokens":'))
            and kiro_put_u64(writer, input.used)
            and kiro_put_literal(writer, StringSlice(',"output_tokens":'))
            and kiro_put_u64(writer, input.size)
            and kiro_put_literal(writer, StringSlice("}}"))
        )
    if operation == KIRO_CHAT_COMPLETION_CHUNK:
        if not kiro_put_literal(writer, StringSlice('data: {"id":')):
            return False
        if input.response_id_present == 1:
            if not kiro_put_json_string(writer, input.response_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"chatcmpl_kiro"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"object":"chat.completion.chunk","choices":[{"index":0,"delta":')):
            return False
        if input.content_present == 1:
            if not kiro_put_view(writer, input.content):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice("{}")):
                return False
        if input.finish_reason_present == 1:
            if not kiro_put_literal(writer, StringSlice(',"finish_reason":')) or not kiro_put_json_string(writer, input.finish_reason):
                return False
        if not kiro_put_literal(writer, StringSlice("}]}")):
            return False
        if input.model_present == 1 and input.model.len > 0:
            if not kiro_put_literal(writer, StringSlice(',"model":')) or not kiro_put_json_string(writer, input.model):
                return False
        return kiro_put_literal(writer, StringSlice("}\n\n"))
    if operation == KIRO_CHAT_ROLE_DELTA:
        return kiro_put_literal(writer, StringSlice('{"role":"assistant"}'))
    if operation == KIRO_CHAT_EMPTY_DELTA:
        return kiro_put_literal(writer, StringSlice("{}"))
    if operation == KIRO_CHAT_TEXT_DELTA or operation == KIRO_CHAT_REASONING_DELTA:
        if not kiro_put_byte(writer, 123):
            return False
        if input.include_role == 1:
            if not kiro_put_literal(writer, StringSlice('"role":"assistant",')):
                return False
        if operation == KIRO_CHAT_TEXT_DELTA:
            if not kiro_put_literal(writer, StringSlice('"content":')):
                return False
        else:
            if not kiro_put_literal(writer, StringSlice('"reasoning_content":')):
                return False
        if not kiro_put_json_string(writer, input.content) or not kiro_put_byte(writer, 125):
            return False
        return True
    if operation == KIRO_CHAT_TOOL_CALL_DELTA:
        if not kiro_put_literal(writer, StringSlice("{")):
            return False
        if input.include_role == 1 and not kiro_put_literal(writer, StringSlice('"role":"assistant",')):
            return False
        return (
            kiro_put_literal(writer, StringSlice('"tool_calls":[{"index":0,"id":'))
            and kiro_put_json_string(writer, input.call_id)
            and kiro_put_literal(writer, StringSlice(',"type":"function","function":{"name":'))
            and kiro_put_json_string(writer, input.name)
            and kiro_put_literal(writer, StringSlice(',"arguments":'))
            and kiro_put_json_string(writer, input.arguments)
            and kiro_put_literal(writer, StringSlice("}}]}"))
        )
    if operation == KIRO_OUTPUT_TEXT_DELTA_EVENT:
        return (
            kiro_put_event_prefix(writer, StringSlice("response.output_text.delta"), input.sequence_number, input.created_at, True)
            and kiro_put_literal(writer, StringSlice(',"response_id":'))
            and kiro_put_json_string(writer, input.response_id)
            and kiro_put_literal(writer, StringSlice(',"delta":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_RESPONSE_CREATED_EVENT:
        return (
            kiro_put_event_prefix(writer, StringSlice("response.created"), input.sequence_number, input.created_at, True)
            and kiro_put_literal(writer, StringSlice(',"response":{"id":'))
            and kiro_put_json_string(writer, input.response_id)
            and kiro_put_literal(writer, StringSlice("}}"))
        )
    if operation == KIRO_OUTPUT_ITEM_ADDED_EVENT or operation == KIRO_OUTPUT_ITEM_DONE_EVENT:
        var event_type = StringSlice("response.output_item.added")
        if operation == KIRO_OUTPUT_ITEM_DONE_EVENT:
            event_type = StringSlice("response.output_item.done")
        if not kiro_put_event_prefix(writer, event_type, input.sequence_number, input.created_at, False):
            return False
        if not kiro_put_literal(writer, StringSlice(',"item":')) or not kiro_put_view(writer, input.output):
            return False
        if operation == KIRO_OUTPUT_ITEM_DONE_EVENT:
            if not kiro_put_literal(writer, StringSlice(',"response_id":')) or not kiro_put_json_string(writer, input.response_id):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_RESPONSE_COMPLETED_EVENT or operation == KIRO_RESPONSE_FAILED_EVENT or operation == KIRO_RESPONSE_INCOMPLETE_EVENT:
        var event_type = StringSlice("response.completed")
        if operation == KIRO_RESPONSE_FAILED_EVENT:
            event_type = StringSlice("response.failed")
        elif operation == KIRO_RESPONSE_INCOMPLETE_EVENT:
            event_type = StringSlice("response.incomplete")
        return (
            kiro_put_event_prefix(writer, event_type, input.sequence_number, input.created_at, True)
            and kiro_put_literal(writer, StringSlice(',"response":'))
            and kiro_put_view(writer, input.output)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_TOOL_CALL_ARGUMENTS_DELTA_CHAT_VALUE:
        return (
            kiro_put_literal(writer, StringSlice('{"choices":[{"delta":{"tool_calls":[{"id":'))
            and kiro_put_json_string(writer, input.call_id)
            and kiro_put_literal(writer, StringSlice(',"function":{"arguments":'))
            and kiro_put_json_string(writer, input.arguments)
            and kiro_put_literal(writer, StringSlice("}}]}}]}"))
        )
    if operation == KIRO_USAGE_UPDATE:
        if not kiro_put_literal(writer, StringSlice('{"used":')) or not kiro_put_u64(writer, input.used):
            return False
        if not kiro_put_literal(writer, StringSlice(',"size":')) or not kiro_put_u64(writer, input.size):
            return False
        var remaining: UInt64 = 0
        if input.size > input.used:
            remaining = input.size - input.used
        if not kiro_put_literal(writer, StringSlice(',"remaining":')) or not kiro_put_u64(writer, remaining):
            return False
        if not kiro_put_extra_fields(writer, input.extra_present, input.extra, True):
            return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_STREAM_TOOL_ARGUMENTS:
        if input.input_present == 1:
            return kiro_put_literal(writer, StringSlice('{"details_omitted":true}'))
        return kiro_put_literal(writer, StringSlice("{}"))
    if operation == KIRO_FINISH_REASON:
        if not kiro_put_byte(writer, 34):
            return False
        if not kiro_put_chat_finish_reason(writer, input):
            return False
        return kiro_put_byte(writer, 34)
    if operation == KIRO_CHAT_TOOL_CALL_ITEM:
        if not kiro_put_literal(writer, StringSlice('{"id":')):
            return False
        if input.call_id_present == 1:
            if not kiro_put_json_string(writer, input.call_id):
                return False
        elif not kiro_put_literal(writer, StringSlice('"call_kiro"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"type":"function","function":{"name":')):
            return False
        if input.name_present == 1:
            if not kiro_put_json_string(writer, input.name):
                return False
        elif not kiro_put_literal(writer, StringSlice('"tool_call"')):
            return False
        if not kiro_put_literal(writer, StringSlice(',"arguments":')):
            return False
        if input.arguments_present == 1:
            if not kiro_put_json_string(writer, input.arguments):
                return False
        elif not kiro_put_literal(writer, StringSlice('"{}"')):
            return False
        return kiro_put_literal(writer, StringSlice("}}"))
    return False


def kiro_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def kiro_input_valid(input: ProdexKiroKernelInput) -> Bool:
    return (
        kiro_flag_valid(input.include_role)
        and kiro_flag_valid(input.has_tool_calls)
        and kiro_flag_valid(input.response_id_present)
        and kiro_flag_valid(input.model_present)
        and kiro_flag_valid(input.role_present)
        and kiro_flag_valid(input.content_present)
        and kiro_flag_valid(input.reason_present)
        and kiro_flag_valid(input.call_id_present)
        and kiro_flag_valid(input.name_present)
        and kiro_flag_valid(input.arguments_present)
        and kiro_flag_valid(input.input_present)
        and kiro_flag_valid(input.output_present)
        and kiro_flag_valid(input.tool_calls_present)
        and kiro_flag_valid(input.requested_model_present)
        and kiro_flag_valid(input.metadata_present)
        and kiro_flag_valid(input.finish_reason_present)
        and kiro_flag_valid(input.status_present)
        and kiro_flag_valid(input.error_present)
        and kiro_flag_valid(input.extra_present)
        and kiro_flag_valid(input.incomplete_reason_present)
        and rich_view_valid(input.response_id, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.model, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.role, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.content, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.reason, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.call_id, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.name, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.arguments, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.input, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.output, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.tool_calls, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.requested_model, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.metadata, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.finish_reason, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.status, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.error, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.extra, KIRO_KERNEL_MAX_BYTES)
        and rich_view_valid(input.incomplete_reason, KIRO_KERNEL_MAX_BYTES)
    )


def kiro_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return KIRO_KERNEL_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return KIRO_KERNEL_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexKiroKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not kiro_input_valid(input[].copy()):
        return KIRO_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = KiroResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not kiro_write_operation(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return KIRO_KERNEL_STATUS_CAPACITY
        return KIRO_KERNEL_STATUS_INVALID
    written[] = writer.written
    return KIRO_KERNEL_STATUS_OK
