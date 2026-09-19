from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_matches_literal,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView
from json_view import (
    deepseek_json_byte,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
)


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
comptime KIRO_STREAM_CONTENT_TEXT: Int64 = 29
comptime KIRO_TOOL_ACTIVITY_ITEM: Int64 = 30
comptime KIRO_TOOL_ACTIVITY_TEXT: Int64 = 31
comptime KIRO_ACP_INITIALIZE_REQUEST: Int64 = 32
comptime KIRO_ACP_SESSION_NEW_REQUEST: Int64 = 33
comptime KIRO_ACP_SESSION_PROMPT_REQUEST: Int64 = 34
comptime KIRO_ACP_MODEL: Int64 = 35
comptime KIRO_ACP_ASSISTANT_OUTPUT: Int64 = 36
comptime KIRO_ACP_RESPONSE: Int64 = 37
comptime KIRO_ACP_CHAT_ASSISTANT: Int64 = 38
comptime KIRO_ACP_PLAN_ENTRY: Int64 = 39
comptime KIRO_ACP_ERROR: Int64 = 40
comptime KIRO_ACP_SESSION_INFO: Int64 = 41
comptime KIRO_ACP_METADATA: Int64 = 42
comptime KIRO_ACP_INCOMPLETE_DETAILS: Int64 = 43
comptime KIRO_MODEL_LIST: Int64 = 44
comptime KIRO_MODEL_NOT_FOUND: Int64 = 45
comptime KIRO_INVALID_REQUEST_ERROR: Int64 = 46
comptime KIRO_UNSUPPORTED_PATH_ERROR: Int64 = 47
comptime KIRO_REQUEST_VALIDATION_ERROR: Int64 = 48

comptime KIRO_REQUEST_VALIDATION_CHAT: Int64 = 1
comptime KIRO_REQUEST_VALIDATION_RESPONSES: Int64 = 2
comptime KIRO_REQUEST_VALIDATION_NONE: Int64 = 0
comptime KIRO_REQUEST_VALIDATION_CHAT_RESPONSE_FORMAT: Int64 = 1
comptime KIRO_REQUEST_VALIDATION_CHAT_CHOICE_COUNT: Int64 = 2
comptime KIRO_REQUEST_VALIDATION_CHAT_STOP: Int64 = 3
comptime KIRO_REQUEST_VALIDATION_CHAT_TEMPERATURE: Int64 = 4
comptime KIRO_REQUEST_VALIDATION_CHAT_TOP_P: Int64 = 5
comptime KIRO_REQUEST_VALIDATION_CHAT_PRESENCE_PENALTY: Int64 = 6
comptime KIRO_REQUEST_VALIDATION_CHAT_FREQUENCY_PENALTY: Int64 = 7
comptime KIRO_REQUEST_VALIDATION_CHAT_SEED: Int64 = 8
comptime KIRO_REQUEST_VALIDATION_CHAT_PARALLEL_TOOL_CALLS: Int64 = 9
comptime KIRO_REQUEST_VALIDATION_TOKEN_LIMIT: Int64 = 10
comptime KIRO_REQUEST_VALIDATION_GENERATION_CONTROL: Int64 = 11
comptime KIRO_REQUEST_VALIDATION_RESPONSE_STOP: Int64 = 12
comptime KIRO_REQUEST_VALIDATION_LOGPROBS: Int64 = 13
comptime KIRO_REQUEST_VALIDATION_TOP_LOGPROBS: Int64 = 14
comptime KIRO_REQUEST_VALIDATION_RESPONSE_FORMAT: Int64 = 15
comptime KIRO_REQUEST_VALIDATION_TOOL_CHOICE: Int64 = 16
comptime KIRO_REQUEST_VALIDATION_TOOLS: Int64 = 17
comptime KIRO_REQUEST_VALIDATION_WEB_SEARCH: Int64 = 18
comptime KIRO_REQUEST_VALIDATION_REASONING_EFFORT: Int64 = 19
comptime KIRO_REQUEST_FLAG_CHAT_RESPONSE_FORMAT: UInt64 = 1 << 0
comptime KIRO_REQUEST_FLAG_CHAT_CHOICE_COUNT: UInt64 = 1 << 1
comptime KIRO_REQUEST_FLAG_CHAT_STOP: UInt64 = 1 << 2
comptime KIRO_REQUEST_FLAG_CHAT_TEMPERATURE: UInt64 = 1 << 3
comptime KIRO_REQUEST_FLAG_CHAT_TOP_P: UInt64 = 1 << 4
comptime KIRO_REQUEST_FLAG_CHAT_PRESENCE_PENALTY: UInt64 = 1 << 5
comptime KIRO_REQUEST_FLAG_CHAT_FREQUENCY_PENALTY: UInt64 = 1 << 6
comptime KIRO_REQUEST_FLAG_CHAT_SEED: UInt64 = 1 << 7
comptime KIRO_REQUEST_FLAG_CHAT_PARALLEL_TOOL_CALLS: UInt64 = 1 << 8
comptime KIRO_REQUEST_FLAG_TOKEN_LIMIT: UInt64 = 1 << 9
comptime KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID: UInt64 = 1 << 10
comptime KIRO_REQUEST_FLAG_GENERATION_CONTROL: UInt64 = 1 << 11
comptime KIRO_REQUEST_FLAG_RESPONSE_STOP: UInt64 = 1 << 12
comptime KIRO_REQUEST_FLAG_LOGPROBS_UNSUPPORTED: UInt64 = 1 << 13
comptime KIRO_REQUEST_FLAG_LOGPROBS_INVALID: UInt64 = 1 << 14
comptime KIRO_REQUEST_FLAG_TOP_LOGPROBS: UInt64 = 1 << 15
comptime KIRO_REQUEST_FLAG_RESPONSE_FORMAT: UInt64 = 1 << 16
comptime KIRO_REQUEST_FLAG_TOOL_CHOICE: UInt64 = 1 << 17
comptime KIRO_REQUEST_FLAG_TOOLS: UInt64 = 1 << 18
comptime KIRO_REQUEST_FLAG_WEB_SEARCH: UInt64 = 1 << 19
comptime KIRO_REQUEST_FLAG_REASONING_EFFORT: UInt64 = 1 << 20
comptime KIRO_REQUEST_FLAG_MASK: UInt64 = (1 << 21) - 1


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
struct ProdexKiroRequestValidationInput(Copyable):
    var mode: Int64
    var flags: UInt64
    var detail: Int64
    var allow_token_limit: Int64


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


def kiro_activity_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def kiro_activity_contains(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return True
    if view.len < UInt(needle):
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for start in range(Int64(view.len) - needle + 1):
        var matched = True
        for offset in range(needle):
            if kiro_activity_ascii_lower(left[unsafe_offset=start + offset]) != kiro_activity_ascii_lower(right[unsafe_offset=offset]):
                matched = False
                break
        if matched:
            return True
    return False


def kiro_activity_field_safe(view: ProdexRichStringView) -> Bool:
    if view.len == 0 or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    var has_content = False
    var index: Int64 = 0
    while index < Int64(view.len):
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if not rich_unicode_space(rich_codepoint(ptr, index, width)):
            has_content = True
            break
        index += width
    if not has_content:
        return False
    for forbidden in [StringSlice("authorization"), StringSlice("bearer"), StringSlice("api_key"), StringSlice("apikey"), StringSlice("password"), StringSlice("sk-"), StringSlice("sk_"), StringSlice("secret"), StringSlice("token"), StringSlice("credential")]:
        if kiro_activity_contains(view, forbidden):
            return False
    for index in range(Int64(view.len)):
        var value = ptr[unsafe_offset=index]
        if value == 47 or value == 92 or value == 64:
            return False
    return True


def kiro_put_activity_codepoint(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    ptr: Pointer[mut=False, UInt8, _],
    index: Int64,
    width: Int64,
    codepoint: Int64,
) -> Bool:
    if codepoint == 34 or codepoint == 92:
        return kiro_put_byte(writer, 92) and kiro_put_byte(writer, UInt8(codepoint))
    if codepoint == 8:
        return kiro_put_literal(writer, StringSlice("\\b"))
    if codepoint == 9:
        return kiro_put_literal(writer, StringSlice("\\t"))
    if codepoint == 10:
        return kiro_put_literal(writer, StringSlice("\\n"))
    if codepoint == 12:
        return kiro_put_literal(writer, StringSlice("\\f"))
    if codepoint == 13:
        return kiro_put_literal(writer, StringSlice("\\r"))
    if codepoint < 32:
        return kiro_put_literal(writer, StringSlice("\\u00")) and kiro_put_hex_byte(writer, UInt8(codepoint))
    for offset in range(width):
        if not kiro_put_byte(writer, ptr[unsafe_offset=index + offset]):
            return False
    return True


def kiro_put_activity_field(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    maximum: Int64,
) -> Bool:
    if not kiro_activity_field_safe(view) or not kiro_put_byte(writer, 34):
        return False
    var ptr = rich_view_ptr(view)
    var cursor: Int64 = 0
    var normalized_length: Int64 = 0
    var word_started = False
    var pending_space = False
    while cursor < Int64(view.len):
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        if rich_unicode_space(codepoint):
            if word_started:
                pending_space = True
            cursor += width
            continue
        if pending_space:
            if normalized_length + 1 > maximum:
                break
            if not kiro_put_byte(writer, 32):
                return False
            normalized_length += 1
            pending_space = False
        if normalized_length + width > maximum:
            break
        if not kiro_put_activity_codepoint(writer, ptr, cursor, width, codepoint):
            return False
        normalized_length += width
        word_started = True
        cursor += width
    return kiro_put_byte(writer, 34)


def kiro_activity_status_code(
    view: ProdexRichStringView, present: Int64
) -> Int64:
    if present == 0 or view.len == 0:
        return 0
    var bounds = rich_trim_bounds(view)
    var trimmed = ProdexRichStringView(
        view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0])
    )
    if rich_view_matches_literal["pending"](trimmed, True):
        return 1
    if rich_view_matches_literal["in_progress"](trimmed, True):
        return 2
    if rich_view_matches_literal["running"](trimmed, True):
        return 3
    if rich_view_matches_literal["completed"](trimmed, True):
        return 4
    if rich_view_matches_literal["failed"](trimmed, True):
        return 5
    if rich_view_matches_literal["error"](trimmed, True):
        return 6
    if rich_view_matches_literal["cancelled"](trimmed, True):
        return 7
    if rich_view_matches_literal["truncated"](trimmed, True):
        return 8
    return 0


def kiro_put_activity_status(
    writer: Pointer[mut=True, KiroResponseWriter, _], status: Int64
) -> Bool:
    if status == 1:
        return kiro_put_literal(writer, StringSlice("pending"))
    if status == 2:
        return kiro_put_literal(writer, StringSlice("in_progress"))
    if status == 3:
        return kiro_put_literal(writer, StringSlice("running"))
    if status == 4:
        return kiro_put_literal(writer, StringSlice("completed"))
    if status == 5:
        return kiro_put_literal(writer, StringSlice("failed"))
    if status == 6:
        return kiro_put_literal(writer, StringSlice("error"))
    if status == 7:
        return kiro_put_literal(writer, StringSlice("cancelled"))
    if status == 8:
        return kiro_put_literal(writer, StringSlice("truncated"))
    return kiro_put_literal(writer, StringSlice("unknown"))


def kiro_activity_phase_code(status: Int64, initial: Int64) -> Int64:
    if status == 4:
        return 1
    if status == 5 or status == 6:
        return 2
    if status == 7:
        return 3
    if status == 8:
        return 4
    return 5 if initial == 1 else 6


def kiro_put_activity_phase(
    writer: Pointer[mut=True, KiroResponseWriter, _], phase: Int64
) -> Bool:
    if phase == 1:
        return kiro_put_literal(writer, StringSlice("completed"))
    if phase == 2:
        return kiro_put_literal(writer, StringSlice("failed"))
    if phase == 3:
        return kiro_put_literal(writer, StringSlice("cancelled"))
    if phase == 4:
        return kiro_put_literal(writer, StringSlice("truncated"))
    if phase == 5:
        return kiro_put_literal(writer, StringSlice("started"))
    return kiro_put_literal(writer, StringSlice("updated"))


def kiro_write_activity_item(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    var title_safe = input.name_present == 1 and kiro_activity_field_safe(input.name)
    var kind_safe = input.model_present == 1 and kiro_activity_field_safe(input.model)
    if not kiro_put_literal(writer, StringSlice('{"type":"kiro_internal_activity","name":')):
        return False
    if title_safe:
        if not kiro_put_activity_field(writer, input.name, 160):
            return False
    elif kind_safe:
        if not kiro_put_activity_field(writer, input.model, 48):
            return False
    elif not kiro_put_literal(writer, StringSlice('"Kiro internal activity"')):
        return False
    var status = kiro_activity_status_code(input.status, input.status_present)
    if not kiro_put_literal(writer, StringSlice(",\"status\":\"")) or not kiro_put_activity_status(writer, status) or not kiro_put_literal(writer, StringSlice("\",\"phase\":\"")):
        return False
    if not kiro_put_activity_phase(writer, kiro_activity_phase_code(status, input.include_role)):
        return False
    if not kiro_put_literal(writer, StringSlice("\",\"kind\":")):
        return False
    if kind_safe:
        if not kiro_put_activity_field(writer, input.model, 48):
            return False
    elif not kiro_put_literal(writer, StringSlice("null")):
        return False
    if not kiro_put_literal(writer, StringSlice(',"details_omitted":')):
        return False
    if input.has_tool_calls == 1:
        if not kiro_put_literal(writer, StringSlice("true")):
            return False
    else:
        if not kiro_put_literal(writer, StringSlice("false")):
            return False
    return kiro_put_byte(writer, 125)


def kiro_write_activity_text(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    if not kiro_put_literal(writer, StringSlice("[Kiro activity: ")):
        return False
    if input.name_present == 1:
        if not kiro_put_view(writer, input.name):
            return False
    elif not kiro_put_literal(writer, StringSlice("Kiro internal activity")):
        return False
    if not kiro_put_literal(writer, StringSlice("; status=")):
        return False
    if input.status_present == 1:
        if not kiro_put_view(writer, input.status):
            return False
    elif not kiro_put_literal(writer, StringSlice("unknown")):
        return False
    if not kiro_put_literal(writer, StringSlice("; phase=")):
        return False
    if input.role_present == 1:
        if not kiro_put_view(writer, input.role):
            return False
    elif not kiro_put_literal(writer, StringSlice("updated")):
        return False
    if input.model_present == 1:
        if not kiro_put_literal(writer, StringSlice("; kind=")) or not kiro_put_view(writer, input.model):
            return False
    if input.has_tool_calls == 1 and not kiro_put_literal(writer, StringSlice("; details=omitted")):
        return False
    return kiro_put_literal(writer, StringSlice("]\n"))


def kiro_json_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def kiro_json_skip_ws(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = kiro_json_byte(view, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def kiro_json_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 65) + 10
    if value >= 97 and value <= 102:
        return Int64(value - 97) + 10
    return -1


def kiro_json_string_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or kiro_json_byte(view, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = kiro_json_byte(view, index)
        if value == 34:
            return index + 1
        if value == 92:
            if index + 1 >= end:
                return -1
            var escaped = kiro_json_byte(view, index + 1)
            if escaped == 117:
                if index + 5 >= end:
                    return -1
                for offset in range(2, 6):
                    if kiro_json_hex(kiro_json_byte(view, index + Int64(offset))) < 0:
                        return -1
                index += 6
            elif escaped == 34 or escaped == 92 or escaped == 47 or escaped == 98 or escaped == 102 or escaped == 110 or escaped == 114 or escaped == 116:
                index += 2
            else:
                return -1
        elif value < 32:
            return -1
        else:
            var width = rich_codepoint_width(value)
            if index + width > end:
                return -1
            index += width
    return -1


def kiro_json_number_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    if index < end and kiro_json_byte(view, index) == 45:
        index += 1
    if index >= end:
        return -1
    if kiro_json_byte(view, index) == 48:
        index += 1
        if index < end and kiro_json_byte(view, index) >= 48 and kiro_json_byte(view, index) <= 57:
            return -1
    elif kiro_json_byte(view, index) >= 49 and kiro_json_byte(view, index) <= 57:
        index += 1
        while index < end and kiro_json_byte(view, index) >= 48 and kiro_json_byte(view, index) <= 57:
            index += 1
    else:
        return -1
    if index < end and kiro_json_byte(view, index) == 46:
        index += 1
        if index >= end or kiro_json_byte(view, index) < 48 or kiro_json_byte(view, index) > 57:
            return -1
        while index < end and kiro_json_byte(view, index) >= 48 and kiro_json_byte(view, index) <= 57:
            index += 1
    if index < end and (kiro_json_byte(view, index) == 69 or kiro_json_byte(view, index) == 101):
        index += 1
        if index < end and (kiro_json_byte(view, index) == 43 or kiro_json_byte(view, index) == 45):
            index += 1
        if index >= end or kiro_json_byte(view, index) < 48 or kiro_json_byte(view, index) > 57:
            return -1
        while index < end and kiro_json_byte(view, index) >= 48 and kiro_json_byte(view, index) <= 57:
            index += 1
    return index


def kiro_json_literal_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    if start + 4 <= end and kiro_json_byte(view, start) == 116 and kiro_json_byte(view, start + 1) == 114 and kiro_json_byte(view, start + 2) == 117 and kiro_json_byte(view, start + 3) == 101:
        return start + 4
    if start + 5 <= end and kiro_json_byte(view, start) == 102 and kiro_json_byte(view, start + 1) == 97 and kiro_json_byte(view, start + 2) == 108 and kiro_json_byte(view, start + 3) == 115 and kiro_json_byte(view, start + 4) == 101:
        return start + 5
    if start + 4 <= end and kiro_json_byte(view, start) == 110 and kiro_json_byte(view, start + 1) == 117 and kiro_json_byte(view, start + 2) == 108 and kiro_json_byte(view, start + 3) == 108:
        return start + 4
    return -1


def kiro_json_value_end(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Int64:
    if depth > 128:
        return -1
    var index = kiro_json_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = kiro_json_byte(view, index)
    if opening == 34:
        return kiro_json_string_end(view, index, end)
    if opening == 91:
        index += 1
        index = kiro_json_skip_ws(view, index, end)
        if index < end and kiro_json_byte(view, index) == 93:
            return index + 1
        while index < end:
            var value_end = kiro_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = kiro_json_skip_ws(view, value_end, end)
            if index < end and kiro_json_byte(view, index) == 44:
                index = kiro_json_skip_ws(view, index + 1, end)
                continue
            if index < end and kiro_json_byte(view, index) == 93:
                return index + 1
            return -1
        return -1
    if opening == 123:
        index += 1
        index = kiro_json_skip_ws(view, index, end)
        if index < end and kiro_json_byte(view, index) == 125:
            return index + 1
        while index < end:
            var key_end = kiro_json_string_end(view, index, end)
            if key_end < 0:
                return -1
            index = kiro_json_skip_ws(view, key_end, end)
            if index >= end or kiro_json_byte(view, index) != 58:
                return -1
            var value_end = kiro_json_value_end(view, index + 1, end, depth + 1)
            if value_end < 0:
                return -1
            index = kiro_json_skip_ws(view, value_end, end)
            if index < end and kiro_json_byte(view, index) == 44:
                index = kiro_json_skip_ws(view, index + 1, end)
                continue
            if index < end and kiro_json_byte(view, index) == 125:
                return index + 1
            return -1
        return -1
    if opening == 116 or opening == 102 or opening == 110:
        return kiro_json_literal_end(view, index, end)
    return kiro_json_number_end(view, index, end)


def kiro_json_put_codepoint(
    writer: Pointer[mut=True, KiroResponseWriter, _], codepoint: Int64
) -> Bool:
    if codepoint <= 127:
        return kiro_put_byte(writer, UInt8(codepoint))
    if codepoint <= 2047:
        return kiro_put_byte(writer, UInt8(192 + (codepoint >> 6))) and kiro_put_byte(writer, UInt8(128 + (codepoint & 63)))
    if codepoint <= 65535:
        return kiro_put_byte(writer, UInt8(224 + (codepoint >> 12))) and kiro_put_byte(writer, UInt8(128 + ((codepoint >> 6) & 63))) and kiro_put_byte(writer, UInt8(128 + (codepoint & 63)))
    return kiro_put_byte(writer, UInt8(240 + (codepoint >> 18))) and kiro_put_byte(writer, UInt8(128 + ((codepoint >> 12) & 63))) and kiro_put_byte(writer, UInt8(128 + ((codepoint >> 6) & 63))) and kiro_put_byte(writer, UInt8(128 + (codepoint & 63)))


def kiro_json_put_string(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Int64:
    var ptr = rich_view_ptr(view)
    var index = start + 1
    var written: Int64 = 0
    while index < end - 1:
        var value = ptr[unsafe_offset=index]
        var codepoint: Int64
        var width: Int64
        if value == 92:
            if index + 1 >= end - 1:
                return -1
            var escaped = ptr[unsafe_offset=index + 1]
            if escaped == 117:
                if index + 5 >= end - 1:
                    return -1
                codepoint = 0
                for offset in range(2, 6):
                    var digit = kiro_json_hex(ptr[unsafe_offset=index + Int64(offset)])
                    if digit < 0:
                        return -1
                    codepoint = codepoint * 16 + digit
                index += 6
                if codepoint >= 55296 and codepoint <= 56319:
                    if index + 5 >= end - 1 or ptr[unsafe_offset=index] != 92 or ptr[unsafe_offset=index + 1] != 117:
                        return -1
                    var low: Int64 = 0
                    for offset in range(2, 6):
                        var digit = kiro_json_hex(ptr[unsafe_offset=index + Int64(offset)])
                        if digit < 0:
                            return -1
                        low = low * 16 + digit
                    if low < 56320 or low > 57343:
                        return -1
                    codepoint = 65536 + ((codepoint - 55296) << 10) + low - 56320
                    index += 6
                elif codepoint >= 56320 and codepoint <= 57343:
                    return -1
            else:
                if escaped == 34 or escaped == 92 or escaped == 47:
                    codepoint = Int64(escaped)
                elif escaped == 98:
                    codepoint = 8
                elif escaped == 102:
                    codepoint = 12
                elif escaped == 110:
                    codepoint = 10
                elif escaped == 114:
                    codepoint = 13
                elif escaped == 116:
                    codepoint = 9
                else:
                    return -1
                index += 2
        else:
            width = rich_codepoint_width(value)
            if index + width > end - 1:
                return -1
            codepoint = rich_codepoint(ptr, index, width)
            index += width
        if not kiro_json_put_codepoint(writer, codepoint):
            return -1
        written += 1
    return 1 if written > 0 else 0


def kiro_json_key_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var expected_length = Int64(literal.byte_length())
    if end - start - 2 != expected_length or kiro_json_byte(view, start) != 34 or kiro_json_byte(view, end - 1) != 34:
        return False
    var actual = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(expected_length):
        if actual[unsafe_offset=start + 1 + index] != expected[unsafe_offset=index]:
            return False
    return True


def kiro_json_write_content_value(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, KiroResponseWriter, _],
    depth: Int64,
) -> Int64:
    if depth > 128:
        return -1
    var index = kiro_json_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = kiro_json_byte(view, index)
    if opening == 34:
        var string_end = kiro_json_string_end(view, index, end)
        if string_end < 0:
            return -1
        return kiro_json_put_string(writer, view, index, string_end)
    if opening == 91:
        var found = False
        index = kiro_json_skip_ws(view, index + 1, end)
        if index < end and kiro_json_byte(view, index) == 93:
            return 0
        while index < end:
            var value_end = kiro_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            var result = kiro_json_write_content_value(view, index, value_end, writer, depth + 1)
            if result < 0:
                return -1
            if result == 1:
                found = True
            index = kiro_json_skip_ws(view, value_end, end)
            if index < end and kiro_json_byte(view, index) == 93:
                return 1 if found else 0
            if index >= end or kiro_json_byte(view, index) != 44:
                return -1
            index = kiro_json_skip_ws(view, index + 1, end)
        return -1
    if opening == 123:
        var content_start: Int64 = -1
        var content_end: Int64 = -1
        index = kiro_json_skip_ws(view, index + 1, end)
        if index < end and kiro_json_byte(view, index) == 125:
            return 0
        while index < end:
            var key_start = index
            var key_end = kiro_json_string_end(view, key_start, end)
            if key_end < 0:
                return -1
            index = kiro_json_skip_ws(view, key_end, end)
            if index >= end or kiro_json_byte(view, index) != 58:
                return -1
            var value_start = kiro_json_skip_ws(view, index + 1, end)
            var value_end = kiro_json_value_end(view, value_start, end, depth + 1)
            if value_end < 0:
                return -1
            if kiro_json_key_matches(view, key_start, key_end, StringSlice("text")) and kiro_json_byte(view, value_start) == 34:
                return kiro_json_put_string(writer, view, value_start, value_end)
            if kiro_json_key_matches(view, key_start, key_end, StringSlice("content")):
                content_start = value_start
                content_end = value_end
            index = kiro_json_skip_ws(view, value_end, end)
            if index < end and kiro_json_byte(view, index) == 125:
                if content_start >= 0:
                    return kiro_json_write_content_value(view, content_start, content_end, writer, depth + 1)
                return 0
            if index >= end or kiro_json_byte(view, index) != 44:
                return -1
            index = kiro_json_skip_ws(view, index + 1, end)
        return -1
    return 0


def kiro_json_array_length(view: ProdexRichStringView) -> Int64:
    var end = Int64(view.len)
    var index = kiro_json_skip_ws(view, 0, end)
    if index >= end or kiro_json_byte(view, index) != 91:
        return -1
    index = kiro_json_skip_ws(view, index + 1, end)
    if index < end and kiro_json_byte(view, index) == 93:
        return 0 if kiro_json_skip_ws(view, index + 1, end) == end else -1
    var count: Int64 = 0
    while index < end:
        var value_end = kiro_json_value_end(view, index, end, 0)
        if value_end < 0:
            return -1
        count += 1
        index = kiro_json_skip_ws(view, value_end, end)
        if index < end and kiro_json_byte(view, index) == 93:
            return count if kiro_json_skip_ws(view, index + 1, end) == end else -1
        if index >= end or kiro_json_byte(view, index) != 44:
            return -1
        index = kiro_json_skip_ws(view, index + 1, end)
    return -1


def kiro_put_bounded_activity_array(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var count = kiro_json_array_length(view)
    if count < 0 or not kiro_put_byte(writer, 91):
        return False
    var keep = count
    if count > 128:
        keep = 127
    var end = Int64(view.len)
    var index = kiro_json_skip_ws(view, 0, end) + 1
    index = kiro_json_skip_ws(view, index, end)
    for item_index in range(keep):
        var value_end = kiro_json_value_end(view, index, end, 0)
        if value_end < 0:
            return False
        if item_index > 0 and not kiro_put_byte(writer, 44):
            return False
        if not kiro_put_view_range(writer, view, index, value_end):
            return False
        index = kiro_json_skip_ws(view, value_end, end)
        if index < end and kiro_json_byte(view, index) == 44:
            index = kiro_json_skip_ws(view, index + 1, end)
    if count > 128:
        if keep > 0 and not kiro_put_byte(writer, 44):
            return False
        if not kiro_put_literal(writer, StringSlice('{"type":"kiro_internal_activity","name":"Additional Kiro activities omitted","status":"truncated","phase":"truncated","kind":null,"details_omitted":true}')):
            return False
    return kiro_put_byte(writer, 93)


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



def kiro_write_request_validation_error(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    var reason = Int64(input.request_id)
    var detail = Int64(input.used)
    var invalid = input.include_role != 0
    if reason == 1:
        return kiro_put_literal(writer, StringSlice("unsupported_response_format\nKiro provider only supports chat response_format type 'text' right now"))
    if reason == 2:
        return kiro_put_literal(writer, StringSlice("unsupported_choice_count\nKiro provider only supports chat completion parameter n=1 right now"))
    if reason == 3:
        return kiro_put_literal(writer, StringSlice("unsupported_stop\nKiro provider does not support chat stop sequences right now"))
    if reason == 4:
        return kiro_put_literal(writer, StringSlice("unsupported_temperature\nKiro provider does not support non-default chat temperature right now"))
    if reason == 5:
        return kiro_put_literal(writer, StringSlice("unsupported_top_p\nKiro provider does not support non-default chat top_p right now"))
    if reason == 6:
        return kiro_put_literal(writer, StringSlice("unsupported_presence_penalty\nKiro provider does not support non-default chat presence_penalty right now"))
    if reason == 7:
        return kiro_put_literal(writer, StringSlice("unsupported_frequency_penalty\nKiro provider does not support non-default chat frequency_penalty right now"))
    if reason == 8:
        return kiro_put_literal(writer, StringSlice("unsupported_seed\nKiro provider does not support chat seed right now"))
    if reason == 9:
        return kiro_put_literal(writer, StringSlice("unsupported_parallel_tool_calls\nKiro provider does not support chat parallel_tool_calls right now"))
    if reason == 10:
        var field = StringSlice("max_output_tokens")
        if detail == 1:
            field = StringSlice("max_tokens")
        elif detail == 2:
            field = StringSlice("max_completion_tokens")
        elif detail < 0 or detail > 2:
            return kiro_put_literal(writer, StringSlice("invalid_request\nKiro request capability validation returned an invalid token limit field"))
        if not kiro_put_literal(writer, StringSlice("unsupported_token_limit\nKiro ")):
            return False
        if not kiro_put_literal(writer, field):
            return False
        if invalid:
            return kiro_put_literal(writer, StringSlice(" must be a positive integer"))
        return kiro_put_literal(writer, StringSlice(" ACP does not expose the ")) and kiro_put_literal(writer, field) and kiro_put_literal(writer, StringSlice(" control"))
    if reason == 11:
        var field = StringSlice("temperature")
        if detail == 1:
            field = StringSlice("top_p")
        elif detail == 2:
            field = StringSlice("seed")
        return kiro_put_literal(writer, StringSlice("unsupported_generation_control\nKiro ACP does not expose the ")) and kiro_put_literal(writer, field) and kiro_put_literal(writer, StringSlice(" control"))
    if reason == 12:
        return kiro_put_literal(writer, StringSlice("unsupported_stop\nKiro ACP does not expose stop-sequence controls"))
    if reason == 13:
        if invalid:
            return kiro_put_literal(writer, StringSlice("invalid_logprobs\nKiro logprobs must be a boolean"))
        return kiro_put_literal(writer, StringSlice("unsupported_logprobs\nKiro ACP does not expose log probabilities"))
    if reason == 14:
        return kiro_put_literal(writer, StringSlice("unsupported_logprobs\nKiro ACP does not expose top_logprobs"))
    if reason == 15:
        return kiro_put_literal(writer, StringSlice("unsupported_response_format\nKiro ACP supports only text response format"))
    if reason == 16:
        return kiro_put_literal(writer, StringSlice("unsupported_tool_choice\nKiro ACP owns tool selection and cannot honor tool_choice"))
    if reason == 17:
        return kiro_put_literal(writer, StringSlice("unsupported_tools\nKiro ACP owns its tool inventory and cannot execute external tools"))
    if reason == 18:
        return kiro_put_literal(writer, StringSlice("unsupported_web_search_options\nKiro ACP owns web search and cannot honor web_search_options"))
    if reason == 19:
        return kiro_put_literal(writer, StringSlice("unsupported_reasoning_effort\nKiro ACP does not support reasoning effort `")) and kiro_put_view(writer, input.reason) and kiro_put_byte(writer, 96)
    return kiro_put_literal(writer, StringSlice("invalid_request\nKiro request capability validation returned an unknown reason"))

def kiro_write_operation(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    input: ProdexKiroKernelInput,
) -> Bool:
    var operation = input.operation
    if operation == KIRO_REQUEST_VALIDATION_ERROR:
        return kiro_write_request_validation_error(writer, input)
    if operation == KIRO_STREAM_CONTENT_TEXT:
        if input.input_present == 0:
            return False
        var end = Int64(input.input.len)
        var start = kiro_json_skip_ws(input.input, 0, end)
        var value_end = kiro_json_value_end(input.input, start, end, 0)
        if value_end < 0 or kiro_json_skip_ws(input.input, value_end, end) != end:
            return False
        return kiro_json_write_content_value(input.input, start, value_end, writer, 0) >= 0
    if operation == KIRO_TOOL_ACTIVITY_ITEM:
        return kiro_write_activity_item(writer, input)
    if operation == KIRO_TOOL_ACTIVITY_TEXT:
        return kiro_write_activity_text(writer, input)
    if operation == KIRO_ACP_INITIALIZE_REQUEST:
        return (
            kiro_put_literal(writer, StringSlice('{"jsonrpc":"2.0","id":'))
            and kiro_put_u64(writer, input.request_id)
            and kiro_put_literal(writer, StringSlice(',"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"auth":{"terminal":false}},"clientInfo":{"name":'))
            and kiro_put_json_string(writer, input.name)
            and kiro_put_literal(writer, StringSlice(',"title":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice(',"version":'))
            and kiro_put_json_string(writer, input.model)
            and kiro_put_literal(writer, StringSlice("}}}"))
        )
    if operation == KIRO_ACP_SESSION_NEW_REQUEST:
        return (
            kiro_put_literal(writer, StringSlice('{"jsonrpc":"2.0","id":'))
            and kiro_put_u64(writer, input.request_id)
            and kiro_put_literal(writer, StringSlice(',"method":"session/new","params":{"cwd":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice(',"mcpServers":[]}}'))
        )
    if operation == KIRO_ACP_SESSION_PROMPT_REQUEST:
        return (
            kiro_put_literal(writer, StringSlice('{"jsonrpc":"2.0","id":'))
            and kiro_put_u64(writer, input.request_id)
            and kiro_put_literal(writer, StringSlice(',"method":"session/prompt","params":{"sessionId":'))
            and kiro_put_json_string(writer, input.response_id)
            and kiro_put_literal(writer, StringSlice(',"prompt":[{"type":"text","text":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice("}]}}"))
        )
    if operation == KIRO_ACP_MODEL:
        return (
            kiro_put_literal(writer, StringSlice('{"id":'))
            and kiro_put_json_string(writer, input.model)
            and kiro_put_literal(writer, StringSlice(',"name":'))
            and kiro_put_json_string(writer, input.name)
            and kiro_put_literal(writer, StringSlice(',"object":"model","owned_by":"kiro-cli"}'))
        )
    if operation == KIRO_ACP_ASSISTANT_OUTPUT:
        return (
            kiro_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[{"type":"output_text","text":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice("}]}"))
        )
    if operation == KIRO_ACP_RESPONSE:
        return (
            kiro_put_literal(writer, StringSlice('{"id":'))
            and kiro_put_json_string(writer, input.response_id)
            and kiro_put_literal(writer, StringSlice(',"object":"response","created_at":'))
            and kiro_put_u64(writer, input.created_at)
            and kiro_put_literal(writer, StringSlice(',"model":'))
            and kiro_put_json_string(writer, input.model)
            and kiro_put_literal(writer, StringSlice(',"output":'))
            and kiro_put_view(writer, input.output)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_ACP_CHAT_ASSISTANT:
        var has_text = input.content_present == 1 and input.content.len > 0
        var has_reasoning = input.reason_present == 1 and input.reason.len > 0
        var has_tools = input.tool_calls_present == 1 and input.tool_calls.len > 2
        if not has_text and not has_reasoning and not has_tools:
            return True
        if not kiro_put_literal(writer, StringSlice('{"role":"assistant","content":')):
            return False
        if has_text:
            if not kiro_put_json_string(writer, input.content):
                return False
        elif has_tools:
            if not kiro_put_literal(writer, StringSlice('""')):
                return False
        elif not kiro_put_literal(writer, StringSlice("null")):
            return False
        if has_reasoning:
            if not kiro_put_literal(writer, StringSlice(',"reasoning_content":')) or not kiro_put_json_string(writer, input.reason):
                return False
        if has_tools:
            if not kiro_put_literal(writer, StringSlice(',"tool_calls":')) or not kiro_put_view(writer, input.tool_calls):
                return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_ACP_PLAN_ENTRY:
        return (
            kiro_put_literal(writer, StringSlice('{"content":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_literal(writer, StringSlice(',"priority":'))
            and kiro_put_json_string(writer, input.reason)
            and kiro_put_literal(writer, StringSlice(',"status":'))
            and kiro_put_json_string(writer, input.status)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_ACP_ERROR:
        return (
            kiro_put_literal(writer, StringSlice('{"code":'))
            and kiro_put_json_string(writer, input.call_id)
            and kiro_put_literal(writer, StringSlice(',"message":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_ACP_SESSION_INFO:
        if not kiro_put_literal(writer, StringSlice('{"title":')):
            return False
        if input.name_present == 1:
            if not kiro_put_json_string(writer, input.name):
                return False
        elif not kiro_put_literal(writer, StringSlice("null")):
            return False
        if not kiro_put_literal(writer, StringSlice(',"updated_at":')):
            return False
        if input.status_present == 1:
            if not kiro_put_json_string(writer, input.status):
                return False
        elif not kiro_put_literal(writer, StringSlice("null")):
            return False
        return kiro_put_byte(writer, 125)
    if operation == KIRO_ACP_METADATA:
        var has_fields = False
        if not kiro_put_literal(writer, StringSlice('{"kiro":{')):
            return False
        if input.reason_present == 1 and input.reason.len > 0:
            if not kiro_put_literal(writer, StringSlice('"reasoning_content":')) or not kiro_put_json_string(writer, input.reason):
                return False
            has_fields = True
        if input.input_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"usage_update":')) or not kiro_put_view(writer, input.input):
                return False
            has_fields = True
        if input.output_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"plan":')) or not kiro_put_view(writer, input.output):
                return False
            has_fields = True
        if input.tool_calls_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"available_commands":')) or not kiro_put_view(writer, input.tool_calls):
                return False
            has_fields = True
        if input.model_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"current_mode_id":')) or not kiro_put_json_string(writer, input.model):
                return False
            has_fields = True
        if input.name_present == 1 or input.status_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"session_info":{"title":')):
                return False
            if input.name_present == 1:
                if not kiro_put_json_string(writer, input.name):
                    return False
            elif not kiro_put_literal(writer, StringSlice("null")):
                return False
            if not kiro_put_literal(writer, StringSlice(',"updated_at":')):
                return False
            if input.status_present == 1:
                if not kiro_put_json_string(writer, input.status):
                    return False
            elif not kiro_put_literal(writer, StringSlice("null")):
                return False
            if not kiro_put_byte(writer, 125):
                return False
            has_fields = True
        if input.finish_reason_present == 1:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"stop_reason":')) or not kiro_put_json_string(writer, input.finish_reason):
                return False
            has_fields = True
        if input.extra_present == 1 and input.extra.len > 2:
            if has_fields and not kiro_put_byte(writer, 44):
                return False
            if not kiro_put_literal(writer, StringSlice('"tool_activities":')) or not kiro_put_bounded_activity_array(writer, input.extra):
                return False
            has_fields = True
        if not has_fields:
            writer[].written = 0
            return True
        return kiro_put_literal(writer, StringSlice("}}"))
    if operation == KIRO_ACP_INCOMPLETE_DETAILS:
        return (
            kiro_put_literal(writer, StringSlice('{"reason":'))
            and kiro_put_json_string(writer, input.reason)
            and kiro_put_literal(writer, StringSlice(',"message":'))
            and kiro_put_json_string(writer, input.content)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_MODEL_LIST:
        return (
            kiro_put_literal(writer, StringSlice('{"object":"list","data":'))
            and kiro_put_view(writer, input.output)
            and kiro_put_byte(writer, 125)
        )
    if operation == KIRO_MODEL_NOT_FOUND:
        if not kiro_put_literal(writer, StringSlice('{"error":{"message":')) or not kiro_put_json_string_with_prefix(writer, StringSlice("model '"), input.model):
            return False
        writer[].written -= 1
        return kiro_put_literal(writer, StringSlice('\' is not available for kiro","type":"invalid_request_error","code":"model_not_found"}}'))
    if operation == KIRO_INVALID_REQUEST_ERROR or operation == KIRO_UNSUPPORTED_PATH_ERROR:
        if not kiro_put_literal(writer, StringSlice('{"error":{"message":')):
            return False
        if operation == KIRO_UNSUPPORTED_PATH_ERROR:
            if not kiro_put_json_string_with_prefix(writer, StringSlice("Kiro provider does not support "), input.content):
                return False
            writer[].written -= 1
            if not kiro_put_literal(writer, StringSlice(' yet"')):
                return False
        elif not kiro_put_json_string(writer, input.content):
            return False
        return (
            kiro_put_literal(writer, StringSlice(',"type":"invalid_request_error","code":'))
            and kiro_put_json_string(writer, input.status)
            and kiro_put_literal(writer, StringSlice("}}"))
        )
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


def kiro_request_validation_input_valid(
    input: ProdexKiroRequestValidationInput
) -> Bool:
    if input.mode < KIRO_REQUEST_VALIDATION_CHAT or input.mode > KIRO_REQUEST_VALIDATION_RESPONSES:
        return False
    if input.flags & ~KIRO_REQUEST_FLAG_MASK != 0:
        return False
    if input.detail < -1 or input.detail > 2:
        return False
    return kiro_flag_valid(input.allow_token_limit)


def kiro_request_validation_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if input_address == 0 or output_address == 0:
        return KIRO_KERNEL_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_NONE
    output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return KIRO_KERNEL_STATUS_ABI
    var input = Pointer[
        mut=False, ProdexKiroRequestValidationInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var value = input[].copy()
    if not kiro_request_validation_input_valid(value):
        return KIRO_KERNEL_STATUS_INVALID

    if value.mode == KIRO_REQUEST_VALIDATION_CHAT:
        if value.flags & KIRO_REQUEST_FLAG_CHAT_RESPONSE_FORMAT != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_RESPONSE_FORMAT
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_CHOICE_COUNT != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_CHOICE_COUNT
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_STOP != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_STOP
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_TEMPERATURE != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_TEMPERATURE
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_TOP_P != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_TOP_P
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_PRESENCE_PENALTY != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_PRESENCE_PENALTY
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_FREQUENCY_PENALTY != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_FREQUENCY_PENALTY
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_SEED != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_SEED
        elif value.flags & KIRO_REQUEST_FLAG_CHAT_PARALLEL_TOOL_CALLS != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_PARALLEL_TOOL_CALLS
        elif value.flags & KIRO_REQUEST_FLAG_TOKEN_LIMIT != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOKEN_LIMIT
            output[unsafe_offset=1] = value.detail
            output[unsafe_offset=2] = Int64(
                value.flags & KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID != 0
            )
            return KIRO_KERNEL_STATUS_OK
        return KIRO_KERNEL_STATUS_OK

    if value.flags & KIRO_REQUEST_FLAG_GENERATION_CONTROL != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_GENERATION_CONTROL
        output[unsafe_offset=1] = value.detail
    elif value.allow_token_limit == 0 and value.flags & KIRO_REQUEST_FLAG_TOKEN_LIMIT != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOKEN_LIMIT
        output[unsafe_offset=1] = value.detail
        output[unsafe_offset=2] = Int64(
            value.flags & KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID != 0
        )
        return KIRO_KERNEL_STATUS_OK
    elif value.flags & KIRO_REQUEST_FLAG_RESPONSE_STOP != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_RESPONSE_STOP
    elif value.flags & (
        KIRO_REQUEST_FLAG_LOGPROBS_UNSUPPORTED | KIRO_REQUEST_FLAG_LOGPROBS_INVALID
    ) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_LOGPROBS
        output[unsafe_offset=2] = Int64(
            value.flags & KIRO_REQUEST_FLAG_LOGPROBS_INVALID != 0
        )
    elif value.flags & KIRO_REQUEST_FLAG_TOP_LOGPROBS != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOP_LOGPROBS
    elif value.flags & KIRO_REQUEST_FLAG_RESPONSE_FORMAT != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_RESPONSE_FORMAT
    elif value.flags & KIRO_REQUEST_FLAG_TOOL_CHOICE != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOOL_CHOICE
    elif value.flags & KIRO_REQUEST_FLAG_TOOLS != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOOLS
    elif value.flags & KIRO_REQUEST_FLAG_WEB_SEARCH != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_WEB_SEARCH
    elif value.flags & KIRO_REQUEST_FLAG_REASONING_EFFORT != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_REASONING_EFFORT
    return KIRO_KERNEL_STATUS_OK


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

# Raw JSON request validation. This removes Rust fact-extraction from the
# production Mojo path while preserving the same ordered policy plan.
def kiro_raw_root(view: ProdexRichStringView) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    if start >= Int64(view.len) or deepseek_json_byte(view, start) != 123:
        return result^
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len):
        return result^
    result[0] = start
    result[1] = end
    return result^

def kiro_raw_present(bounds: InlineArray[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]

def kiro_raw_is_literal(
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    literal: StringSlice,
) -> Bool:
    if not kiro_raw_present(bounds):
        return False
    if deepseek_json_byte(view, bounds[0]) == 34:
        return deepseek_json_raw_equals(view, bounds[0], bounds[1], literal)
    var length = Int64(literal.byte_length())
    if bounds[1] - bounds[0] != length:
        return False
    var actual = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if actual[unsafe_offset=bounds[0] + index] != expected[unsafe_offset=index]:
            return False
    return True

def kiro_raw_is_null(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    return kiro_raw_is_literal(view, bounds, StringSlice("null"))

def kiro_raw_is_true(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    return kiro_raw_is_literal(view, bounds, StringSlice("true"))

def kiro_raw_is_false(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    return kiro_raw_is_literal(view, bounds, StringSlice("false"))

def kiro_raw_nonempty_string(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds) or deepseek_json_byte(view, bounds[0]) != 34:
        return False
    # JSON "" is the only empty string spelling after parsing.
    return bounds[1] - bounds[0] > 2

def kiro_raw_number_default(
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    default_one: Bool,
) -> Bool:
    if not kiro_raw_present(bounds):
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0]
    var end = bounds[1]
    var negative = False
    if index < end and ptr[unsafe_offset=index] == 45:
        negative = True
        index += 1
    if index >= end:
        return False

    var digit_index: Int64 = 0
    var integer_digits: Int64 = 0
    var one_index: Int64 = -1
    var nonzero_count: Int64 = 0
    var saw_digit = False
    while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
        var digit = ptr[unsafe_offset=index]
        if digit != 48:
            nonzero_count += 1
            if digit == 49:
                one_index = digit_index
        digit_index += 1
        integer_digits += 1
        saw_digit = True
        index += 1
    if index < end and ptr[unsafe_offset=index] == 46:
        index += 1
        var fraction_digits: Int64 = 0
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            var digit = ptr[unsafe_offset=index]
            if digit != 48:
                nonzero_count += 1
                if digit == 49:
                    one_index = digit_index
            digit_index += 1
            fraction_digits += 1
            saw_digit = True
            index += 1
        if fraction_digits == 0:
            return False
    if not saw_digit:
        return False

    var exponent: Int64 = 0
    if index < end and (ptr[unsafe_offset=index] == 101 or ptr[unsafe_offset=index] == 69):
        index += 1
        var exponent_negative = False
        if index < end and (ptr[unsafe_offset=index] == 43 or ptr[unsafe_offset=index] == 45):
            exponent_negative = ptr[unsafe_offset=index] == 45
            index += 1
        var exponent_digits: Int64 = 0
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            exponent_digits += 1
            if exponent < 1_000_000:
                exponent = exponent * 10 + Int64(ptr[unsafe_offset=index] - 48)
            index += 1
        if exponent_digits == 0:
            return False
        if exponent_negative:
            exponent = -exponent
    if index != end:
        return False

    if not default_one:
        return nonzero_count == 0
    if negative or nonzero_count != 1 or one_index < 0:
        return False
    return integer_digits - 1 - one_index + exponent == 0

def kiro_raw_positive_u64_integer(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds):
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0]
    var value: UInt64 = 0
    var digits: Int64 = 0
    while index < bounds[1]:
        var byte = ptr[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > 1844674407370955161 or (
            value == 1844674407370955161 and digit > 5
        ):
            return False
        value = value * 10 + digit
        digits += 1
        index += 1
    return digits > 0 and value > 0

def kiro_raw_stop_requested(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds) or kiro_raw_is_null(view, bounds):
        return False
    var opening = deepseek_json_byte(view, bounds[0])
    if opening == 34:
        return kiro_raw_nonempty_string(view, bounds)
    if opening != 91:
        return True
    var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while cursor < bounds[1] - 1:
        var value_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, 0)
        if value_end < 0:
            return True
        var item = InlineArray[Int64, 2](fill=-1)
        item[0] = cursor
        item[1] = value_end
        if deepseek_json_byte(view, cursor) != 34 or kiro_raw_nonempty_string(view, item):
            return True
        cursor = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
            continue
        break
    return False

def kiro_raw_supported_response_format(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds) or kiro_raw_is_null(view, bounds):
        return True
    if deepseek_json_byte(view, bounds[0]) != 123:
        return False
    var kind = deepseek_json_object_member(
        view, bounds[0], bounds[1], StringSlice("type")
    )
    return kiro_raw_is_literal(view, kind, StringSlice("text"))

def kiro_raw_token_limit(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
) -> InlineArray[Int64, 3]:
    # present, field-index, positive
    var result = InlineArray[Int64, 3](fill=0)
    var field_index: Int64 = 0
    for key in [
        StringSlice("max_output_tokens"),
        StringSlice("max_tokens"),
        StringSlice("max_completion_tokens"),
    ]:
        var bounds = deepseek_json_object_member(view, root[0], root[1], key)
        if kiro_raw_present(bounds) and not kiro_raw_is_null(view, bounds):
            result[0] = 1
            result[1] = field_index
            result[2] = Int64(kiro_raw_positive_u64_integer(view, bounds))
            return result^
        field_index += 1
    return result^

def kiro_raw_response_format_supported(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
) -> Bool:
    var direct = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("response_format")
    )
    if kiro_raw_present(direct) and not kiro_raw_supported_response_format(view, direct):
        return False
    var text = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("text")
    )
    if kiro_raw_present(text) and deepseek_json_byte(view, text[0]) == 123:
        var nested = deepseek_json_object_member(
            view, text[0], text[1], StringSlice("format")
        )
        if kiro_raw_present(nested) and not kiro_raw_supported_response_format(view, nested):
            return False
    return True

def kiro_raw_array_empty(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds) or deepseek_json_byte(view, bounds[0]) != 91:
        return False
    return deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1) == bounds[1] - 1

def kiro_raw_reasoning_effort_supported(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds) or deepseek_json_byte(view, bounds[0]) != 34:
        return True
    return (
        kiro_raw_is_literal(view, bounds, StringSlice("none"))
        or kiro_raw_is_literal(view, bounds, StringSlice("low"))
        or kiro_raw_is_literal(view, bounds, StringSlice("medium"))
        or kiro_raw_is_literal(view, bounds, StringSlice("high"))
        or kiro_raw_is_literal(view, bounds, StringSlice("xhigh"))
        or kiro_raw_is_literal(view, bounds, StringSlice("max"))
    )

def kiro_raw_validation_flags(
    view: ProdexRichStringView,
    mode: Int64,
    detail_out: Pointer[mut=True, Int64, _],
) -> UInt64:
    var root = kiro_raw_root(view)
    if root[0] < 0:
        detail_out[] = -2
        return 0
    var flags: UInt64 = 0
    var token = kiro_raw_token_limit(view, root)
    detail_out[] = -1

    if mode == KIRO_REQUEST_VALIDATION_CHAT:
        var response_format = deepseek_json_object_member(
            view, root[0], root[1], StringSlice("response_format")
        )
        if kiro_raw_present(response_format) and not kiro_raw_supported_response_format(view, response_format):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_RESPONSE_FORMAT)
        var n = deepseek_json_object_member(view, root[0], root[1], StringSlice("n"))
        if kiro_raw_present(n) and not kiro_raw_is_null(view, n):
            if not kiro_raw_positive_u64_integer(view, n) or not kiro_raw_is_literal(view, n, StringSlice("1")):
                flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_CHOICE_COUNT)
        var stop = deepseek_json_object_member(view, root[0], root[1], StringSlice("stop"))
        if kiro_raw_stop_requested(view, stop):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_STOP)
        var temperature = deepseek_json_object_member(view, root[0], root[1], StringSlice("temperature"))
        if kiro_raw_present(temperature) and not kiro_raw_is_null(view, temperature) and not kiro_raw_number_default(view, temperature, True):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_TEMPERATURE)
        var top_p = deepseek_json_object_member(view, root[0], root[1], StringSlice("top_p"))
        if kiro_raw_present(top_p) and not kiro_raw_is_null(view, top_p) and not kiro_raw_number_default(view, top_p, True):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_TOP_P)
        var presence = deepseek_json_object_member(view, root[0], root[1], StringSlice("presence_penalty"))
        if kiro_raw_present(presence) and not kiro_raw_is_null(view, presence) and not kiro_raw_number_default(view, presence, False):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_PRESENCE_PENALTY)
        var frequency = deepseek_json_object_member(view, root[0], root[1], StringSlice("frequency_penalty"))
        if kiro_raw_present(frequency) and not kiro_raw_is_null(view, frequency) and not kiro_raw_number_default(view, frequency, False):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_FREQUENCY_PENALTY)
        var seed = deepseek_json_object_member(view, root[0], root[1], StringSlice("seed"))
        if kiro_raw_present(seed) and not kiro_raw_is_null(view, seed):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_SEED)
        var parallel = deepseek_json_object_member(view, root[0], root[1], StringSlice("parallel_tool_calls"))
        if kiro_raw_present(parallel) and not kiro_raw_is_null(view, parallel) and not kiro_raw_is_true(view, parallel):
            flags |= UInt64(KIRO_REQUEST_FLAG_CHAT_PARALLEL_TOOL_CALLS)
        if token[0] == 1:
            flags |= UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT)
            if token[2] == 0:
                flags |= UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID)
            detail_out[] = token[1]
        return flags

    for detail_index in range(Int64(3)):
        var key = StringSlice("temperature")
        if detail_index == 1:
            key = StringSlice("top_p")
        elif detail_index == 2:
            key = StringSlice("seed")
        var bounds = deepseek_json_object_member(view, root[0], root[1], key)
        if detail_out[] < 0 and kiro_raw_present(bounds) and not kiro_raw_is_null(view, bounds):
            flags |= UInt64(KIRO_REQUEST_FLAG_GENERATION_CONTROL)
            detail_out[] = detail_index

    if token[0] == 1:
        flags |= UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT)
        if token[2] == 0:
            flags |= UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID)
        if detail_out[] < 0:
            detail_out[] = token[1]

    for key in [StringSlice("stop"), StringSlice("stop_sequences"), StringSlice("stopSequences")]:
        if kiro_raw_stop_requested(
            view, deepseek_json_object_member(view, root[0], root[1], key)
        ):
            flags |= UInt64(KIRO_REQUEST_FLAG_RESPONSE_STOP)

    var logprobs = deepseek_json_object_member(view, root[0], root[1], StringSlice("logprobs"))
    if kiro_raw_present(logprobs) and not kiro_raw_is_null(view, logprobs) and not kiro_raw_is_false(view, logprobs):
        if kiro_raw_is_true(view, logprobs):
            flags |= UInt64(KIRO_REQUEST_FLAG_LOGPROBS_UNSUPPORTED)
        else:
            flags |= UInt64(KIRO_REQUEST_FLAG_LOGPROBS_INVALID)

    var top_logprobs = deepseek_json_object_member(view, root[0], root[1], StringSlice("top_logprobs"))
    if kiro_raw_present(top_logprobs) and not kiro_raw_is_null(view, top_logprobs):
        flags |= UInt64(KIRO_REQUEST_FLAG_TOP_LOGPROBS)

    if not kiro_raw_response_format_supported(view, root):
        flags |= UInt64(KIRO_REQUEST_FLAG_RESPONSE_FORMAT)

    var tool_choice = deepseek_json_object_member(view, root[0], root[1], StringSlice("tool_choice"))
    if kiro_raw_present(tool_choice) and not kiro_raw_is_null(view, tool_choice) and not kiro_raw_is_literal(view, tool_choice, StringSlice("auto")):
        flags |= UInt64(KIRO_REQUEST_FLAG_TOOL_CHOICE)

    var tools = deepseek_json_object_member(view, root[0], root[1], StringSlice("tools"))
    if kiro_raw_present(tools) and not kiro_raw_is_null(view, tools):
        if deepseek_json_byte(view, tools[0]) != 91 or not kiro_raw_array_empty(view, tools):
            flags |= UInt64(KIRO_REQUEST_FLAG_TOOLS)

    if kiro_raw_present(deepseek_json_object_member(view, root[0], root[1], StringSlice("web_search_options"))):
        flags |= UInt64(KIRO_REQUEST_FLAG_WEB_SEARCH)

    var effort = InlineArray[Int64, 2](fill=-1)
    var reasoning = deepseek_json_object_member(view, root[0], root[1], StringSlice("reasoning"))
    if kiro_raw_present(reasoning) and deepseek_json_byte(view, reasoning[0]) == 123:
        effort = deepseek_json_object_member(view, reasoning[0], reasoning[1], StringSlice("effort"))
    if not kiro_raw_present(effort):
        effort = deepseek_json_object_member(view, root[0], root[1], StringSlice("reasoning_effort"))
    if not kiro_raw_reasoning_effort_supported(view, effort):
        flags |= UInt64(KIRO_REQUEST_FLAG_REASONING_EFFORT)
    return flags

def kiro_raw_apply_validation(
    mode: Int64,
    flags: UInt64,
    detail: Int64,
    allow_token_limit: Int64,
    output: Pointer[mut=True, Int64, _],
):
    output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_NONE
    output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = 0
    if mode == KIRO_REQUEST_VALIDATION_CHAT:
        if flags & UInt64(KIRO_REQUEST_FLAG_CHAT_RESPONSE_FORMAT) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_RESPONSE_FORMAT
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_CHOICE_COUNT) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_CHOICE_COUNT
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_STOP) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_STOP
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_TEMPERATURE) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_TEMPERATURE
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_TOP_P) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_TOP_P
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_PRESENCE_PENALTY) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_PRESENCE_PENALTY
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_FREQUENCY_PENALTY) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_FREQUENCY_PENALTY
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_SEED) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_SEED
        elif flags & UInt64(KIRO_REQUEST_FLAG_CHAT_PARALLEL_TOOL_CALLS) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_CHAT_PARALLEL_TOOL_CALLS
        elif flags & UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT) != 0:
            output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOKEN_LIMIT
            output[unsafe_offset=1] = detail
            output[unsafe_offset=2] = Int64(flags & UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID) != 0)
        return

    if flags & UInt64(KIRO_REQUEST_FLAG_GENERATION_CONTROL) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_GENERATION_CONTROL
        output[unsafe_offset=1] = detail
    elif allow_token_limit == 0 and flags & UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOKEN_LIMIT
        output[unsafe_offset=1] = detail
        output[unsafe_offset=2] = Int64(flags & UInt64(KIRO_REQUEST_FLAG_TOKEN_LIMIT_INVALID) != 0)
    elif flags & UInt64(KIRO_REQUEST_FLAG_RESPONSE_STOP) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_RESPONSE_STOP
    elif flags & UInt64(KIRO_REQUEST_FLAG_LOGPROBS_UNSUPPORTED | KIRO_REQUEST_FLAG_LOGPROBS_INVALID) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_LOGPROBS
        output[unsafe_offset=2] = Int64(flags & UInt64(KIRO_REQUEST_FLAG_LOGPROBS_INVALID) != 0)
    elif flags & UInt64(KIRO_REQUEST_FLAG_TOP_LOGPROBS) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOP_LOGPROBS
    elif flags & UInt64(KIRO_REQUEST_FLAG_RESPONSE_FORMAT) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_RESPONSE_FORMAT
    elif flags & UInt64(KIRO_REQUEST_FLAG_TOOL_CHOICE) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOOL_CHOICE
    elif flags & UInt64(KIRO_REQUEST_FLAG_TOOLS) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_TOOLS
    elif flags & UInt64(KIRO_REQUEST_FLAG_WEB_SEARCH) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_WEB_SEARCH
    elif flags & UInt64(KIRO_REQUEST_FLAG_REASONING_EFFORT) != 0:
        output[unsafe_offset=0] = KIRO_REQUEST_VALIDATION_REASONING_EFFORT

def kiro_request_validation_json_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    allow_token_limit: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return KIRO_KERNEL_STATUS_ABI
    if (
        mode < KIRO_REQUEST_VALIDATION_CHAT
        or mode > KIRO_REQUEST_VALIDATION_RESPONSES
        or not kiro_flag_valid(allow_token_limit)
        or input_length < 0
        or input_length > KIRO_KERNEL_MAX_BYTES
        or input_address == 0
        or output_address == 0
    ):
        return KIRO_KERNEL_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, KIRO_KERNEL_MAX_BYTES):
        return KIRO_KERNEL_STATUS_UTF8
    var detail: Int64 = -1
    var flags = kiro_raw_validation_flags(view, mode, Pointer(to=detail))
    if detail == -2:
        return KIRO_KERNEL_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    kiro_raw_apply_validation(mode, flags, detail, allow_token_limit, output)
    return KIRO_KERNEL_STATUS_OK

# Chat-completions compatibility rewrite. The Rust boundary canonicalizes JSON
# first; Mojo owns deterministic message/tool/default normalization.
comptime KIRO_CHAT_REWRITE_ISSUE_NONE: Int64 = 0
comptime KIRO_CHAT_REWRITE_ISSUE_MISSING_MESSAGES: Int64 = 1
comptime KIRO_CHAT_REWRITE_ISSUE_INVALID_MESSAGES: Int64 = 2

def kiro_raw_member(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    return deepseek_json_object_member(view, root[0], root[1], key)

def kiro_raw_string_nonblank(
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not kiro_raw_present(bounds) or deepseek_json_byte(view, bounds[0]) != 34:
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0] + 1
    var end = bounds[1] - 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        var codepoint: Int64
        var width: Int64
        if value == 92:
            if index + 1 >= end:
                return False
            var escaped = ptr[unsafe_offset=index + 1]
            if escaped == 117:
                if index + 5 >= end:
                    return False
                codepoint = 0
                for offset in range(2, 6):
                    var digit = kiro_json_hex(ptr[unsafe_offset=index + Int64(offset)])
                    if digit < 0:
                        return False
                    codepoint = codepoint * 16 + digit
                index += 6
                if codepoint >= 55296 and codepoint <= 56319:
                    if index + 5 >= end or ptr[unsafe_offset=index] != 92 or ptr[unsafe_offset=index + 1] != 117:
                        return False
                    var low: Int64 = 0
                    for offset in range(2, 6):
                        var digit = kiro_json_hex(ptr[unsafe_offset=index + Int64(offset)])
                        if digit < 0:
                            return False
                        low = low * 16 + digit
                    if low < 56320 or low > 57343:
                        return False
                    codepoint = 65536 + ((codepoint - 55296) << 10) + low - 56320
                    index += 6
                elif codepoint >= 56320 and codepoint <= 57343:
                    return False
            else:
                if escaped == 98:
                    codepoint = 8
                elif escaped == 102:
                    codepoint = 12
                elif escaped == 110:
                    codepoint = 10
                elif escaped == 114:
                    codepoint = 13
                elif escaped == 116:
                    codepoint = 9
                elif escaped == 34 or escaped == 92 or escaped == 47:
                    codepoint = Int64(escaped)
                else:
                    return False
                index += 2
        else:
            width = rich_codepoint_width(value)
            if index + width > end:
                return False
            codepoint = rich_codepoint(ptr, index, width)
            index += width
        if not rich_unicode_space(codepoint):
            return True
    return False

def kiro_raw_put_string_token(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    return (
        kiro_raw_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and kiro_put_view_range(writer, view, bounds[0], bounds[1])
    )

def kiro_raw_put_default_or_string(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    default_value: StringSlice,
) -> Bool:
    if kiro_raw_present(bounds) and deepseek_json_byte(view, bounds[0]) == 34:
        return kiro_put_view_range(writer, view, bounds[0], bounds[1])
    return kiro_put_literal(writer, default_value)

def kiro_raw_text_piece(
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    writer: Pointer[mut=True, KiroResponseWriter, _],
    add_separator: Bool,
    depth: Int64,
) -> Int64:
    if depth > 128 or not kiro_raw_present(bounds):
        return -1
    var opening = deepseek_json_byte(view, bounds[0])
    if opening == 34:
        if not kiro_raw_string_nonblank(view, bounds):
            return 0
        if add_separator and not kiro_put_literal(writer, StringSlice("\n")):
            return -1
        return 1 if kiro_put_view_range(writer, view, bounds[0] + 1, bounds[1] - 1) else -1
    if opening == 91:
        var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
        var found = False
        while cursor < bounds[1] - 1:
            var item_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, depth + 1)
            if item_end < 0:
                return -1
            var item = InlineArray[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            var result: Int64 = 0
            if deepseek_json_byte(view, cursor) == 123:
                var text = deepseek_json_object_member(
                    view, cursor, item_end, StringSlice("text")
                )
                if kiro_raw_present(text) and deepseek_json_byte(view, text[0]) == 34 and kiro_raw_string_nonblank(view, text):
                    result = kiro_raw_text_piece(view, text, writer, found or add_separator, depth + 1)
                else:
                    result = kiro_raw_text_piece(view, item, writer, found or add_separator, depth + 1)
            else:
                result = kiro_raw_text_piece(view, item, writer, found or add_separator, depth + 1)
            if result < 0:
                return -1
            if result == 1:
                found = True
            cursor = deepseek_json_skip_ws(view, item_end, bounds[1] - 1)
            if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
                continue
            if cursor != bounds[1] - 1:
                return -1
            break
        return 1 if found else 0
    if opening == 123:
        var text = deepseek_json_object_member(
            view, bounds[0], bounds[1], StringSlice("text")
        )
        if kiro_raw_present(text) and deepseek_json_byte(view, text[0]) == 34:
            return kiro_raw_text_piece(view, text, writer, add_separator, depth + 1)
        return 0
    return 0

def kiro_raw_put_text_json_string(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Int64:
    var saved = writer[].written
    if not kiro_put_byte(writer, 34):
        return -1
    var result = kiro_raw_text_piece(view, bounds, writer, False, 0)
    if result != 1:
        writer[].written = saved
        return result
    if not kiro_put_byte(writer, 34):
        writer[].written = saved
        return -1
    return 1

def kiro_raw_object_string_member(
    view: ProdexRichStringView,
    object_bounds: InlineArray[Int64, 2],
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    if not kiro_raw_present(object_bounds) or deepseek_json_byte(view, object_bounds[0]) != 123:
        return InlineArray[Int64, 2](fill=-1)^
    var value = deepseek_json_object_member(
        view, object_bounds[0], object_bounds[1], key
    )
    if kiro_raw_present(value) and deepseek_json_byte(view, value[0]) == 34:
        return value
    return InlineArray[Int64, 2](fill=-1)^

def kiro_raw_role_is(
    view: ProdexRichStringView,
    role: InlineArray[Int64, 2],
    literal: StringSlice,
) -> Bool:
    return kiro_raw_present(role) and deepseek_json_raw_equals(
        view, role[0], role[1], literal
    )

def kiro_raw_item_prefix(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    if first[] == 1:
        first[] = 0
        return True
    return kiro_put_byte(writer, 44)

def kiro_raw_write_message_item(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    message: InlineArray[Int64, 2],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    var role = kiro_raw_member(view, message, StringSlice("role"))
    if kiro_raw_role_is(view, role, StringSlice("tool")) or kiro_raw_role_is(
        view, role, StringSlice("function")
    ):
        return True
    var content = kiro_raw_member(view, message, StringSlice("content"))
    var saved = writer[].written
    var first_saved = first[]
    if not kiro_raw_item_prefix(writer, first):
        return False
    if (
        not kiro_put_literal(writer, StringSlice('{"type":"message","role":'))
        or not kiro_raw_put_default_or_string(
            writer, view, role, StringSlice('"user"')
        )
        or not kiro_put_literal(
            writer, StringSlice(',"content":[{"type":"input_text","text":')
        )
    ):
        writer[].written = saved
        first[] = first_saved
        return False
    var text_result = kiro_raw_put_text_json_string(writer, view, content)
    if text_result == 0:
        writer[].written = saved
        first[] = first_saved
        return True
    if text_result < 0 or not kiro_put_literal(writer, StringSlice("}]}")):
        writer[].written = saved
        first[] = first_saved
        return False
    return True

def kiro_raw_write_function_call_item(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    call_id: InlineArray[Int64, 2],
    name: InlineArray[Int64, 2],
    arguments: InlineArray[Int64, 2],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    if not kiro_raw_item_prefix(writer, first):
        return False
    return (
        kiro_put_literal(writer, StringSlice('{"type":"function_call","call_id":'))
        and kiro_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_kiro"')
        )
        and kiro_put_literal(writer, StringSlice(',"name":'))
        and kiro_raw_put_default_or_string(
            writer, view, name, StringSlice('"tool_call"')
        )
        and kiro_put_literal(writer, StringSlice(',"arguments":'))
        and kiro_raw_put_default_or_string(
            writer, view, arguments, StringSlice('"{}"')
        )
        and kiro_put_byte(writer, 125)
    )

def kiro_raw_write_tool_calls(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    message: InlineArray[Int64, 2],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    var role = kiro_raw_member(view, message, StringSlice("role"))
    if not kiro_raw_role_is(view, role, StringSlice("assistant")):
        return True
    var tool_calls = kiro_raw_member(view, message, StringSlice("tool_calls"))
    if not kiro_raw_present(tool_calls) or deepseek_json_byte(view, tool_calls[0]) != 91:
        return True
    var cursor = deepseek_json_skip_ws(view, tool_calls[0] + 1, tool_calls[1] - 1)
    while cursor < tool_calls[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, tool_calls[1] - 1, 0)
        if item_end < 0:
            return False
        if deepseek_json_byte(view, cursor) == 123:
            var item = InlineArray[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            var function = kiro_raw_member(view, item, StringSlice("function"))
            if kiro_raw_present(function) and deepseek_json_byte(view, function[0]) == 123:
                var call_id = kiro_raw_member(view, item, StringSlice("id"))
                var name = kiro_raw_member(view, function, StringSlice("name"))
                var arguments = kiro_raw_member(
                    view, function, StringSlice("arguments")
                )
                if not kiro_raw_write_function_call_item(
                    writer, view, call_id, name, arguments, first
                ):
                    return False
        cursor = deepseek_json_skip_ws(view, item_end, tool_calls[1] - 1)
        if cursor < tool_calls[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, tool_calls[1] - 1)
            continue
        if cursor != tool_calls[1] - 1:
            return False
        break
    return True

def kiro_raw_write_legacy_function_call_item(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    message: InlineArray[Int64, 2],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    var role = kiro_raw_member(view, message, StringSlice("role"))
    if not kiro_raw_role_is(view, role, StringSlice("assistant")):
        return True
    var tool_calls = kiro_raw_member(view, message, StringSlice("tool_calls"))
    if kiro_raw_present(tool_calls):
        return True
    var function = kiro_raw_member(view, message, StringSlice("function_call"))
    if not kiro_raw_present(function) or deepseek_json_byte(view, function[0]) != 123:
        return True
    var name = kiro_raw_member(view, function, StringSlice("name"))
    var arguments = kiro_raw_member(view, function, StringSlice("arguments"))
    var call_id = kiro_raw_member(view, function, StringSlice("call_id"))
    if not kiro_raw_present(call_id):
        call_id = kiro_raw_member(view, function, StringSlice("id"))
    if not kiro_raw_present(call_id):
        call_id = name.copy()
    return kiro_raw_write_function_call_item(
        writer, view, call_id, name, arguments, first
    )

def kiro_raw_write_tool_output_item(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    message: InlineArray[Int64, 2],
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    var role = kiro_raw_member(view, message, StringSlice("role"))
    if not (
        kiro_raw_role_is(view, role, StringSlice("tool"))
        or kiro_raw_role_is(view, role, StringSlice("function"))
    ):
        return True
    var call_id = kiro_raw_member(view, message, StringSlice("tool_call_id"))
    if not kiro_raw_present(call_id):
        call_id = kiro_raw_member(view, message, StringSlice("call_id"))
    if not kiro_raw_present(call_id):
        call_id = kiro_raw_member(view, message, StringSlice("name"))
    var content = kiro_raw_member(view, message, StringSlice("content"))
    if not kiro_raw_item_prefix(writer, first):
        return False
    if (
        not kiro_put_literal(
            writer, StringSlice('{"type":"function_call_output","call_id":')
        )
        or not kiro_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_kiro"')
        )
        or not kiro_put_literal(writer, StringSlice(',"output":'))
    ):
        return False
    var saved = writer[].written
    var text_result = kiro_raw_put_text_json_string(writer, view, content)
    if text_result == 0:
        writer[].written = saved
        if not kiro_put_literal(writer, StringSlice('""')):
            return False
    elif text_result < 0:
        return False
    return kiro_put_byte(writer, 125)

def kiro_raw_write_chat_input(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    messages: InlineArray[Int64, 2],
) -> Bool:
    if not kiro_raw_present(messages) or deepseek_json_byte(view, messages[0]) != 91:
        return False
    if not kiro_put_byte(writer, 91):
        return False
    var first: Int64 = 1
    var first_ptr = Pointer(to=first)
    var cursor = deepseek_json_skip_ws(view, messages[0] + 1, messages[1] - 1)
    while cursor < messages[1] - 1:
        var message_end = deepseek_json_value_end(
            view, cursor, messages[1] - 1, 0
        )
        if message_end < 0:
            return False
        if deepseek_json_byte(view, cursor) == 123:
            var message = InlineArray[Int64, 2](fill=-1)
            message[0] = cursor
            message[1] = message_end
            if (
                not kiro_raw_write_message_item(
                    writer, view, message, first_ptr
                )
                or not kiro_raw_write_tool_calls(
                    writer, view, message, first_ptr
                )
                or not kiro_raw_write_legacy_function_call_item(
                    writer, view, message, first_ptr
                )
                or not kiro_raw_write_tool_output_item(
                    writer, view, message, first_ptr
                )
            ):
                return False
        cursor = deepseek_json_skip_ws(view, message_end, messages[1] - 1)
        if cursor < messages[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, messages[1] - 1)
            continue
        if cursor != messages[1] - 1:
            return False
        break
    return kiro_put_byte(writer, 93)

def kiro_raw_key_is(
    view: ProdexRichStringView,
    key_start: Int64,
    key_end: Int64,
    literal: StringSlice,
) -> Bool:
    return deepseek_json_raw_equals(view, key_start, key_end, literal)

def kiro_raw_chat_drop_key(
    view: ProdexRichStringView,
    key_start: Int64,
    key_end: Int64,
    has_input: Bool,
) -> Bool:
    if (
        kiro_raw_key_is(view, key_start, key_end, StringSlice("n"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("user"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("stop"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("temperature"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("top_p"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("presence_penalty"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("frequency_penalty"))
        or kiro_raw_key_is(view, key_start, key_end, StringSlice("parallel_tool_calls"))
    ):
        return True
    return (
        not has_input
        and kiro_raw_key_is(view, key_start, key_end, StringSlice("messages"))
    )

def kiro_raw_put_member(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    key_start: Int64,
    key_end: Int64,
    value_start: Int64,
    value_end: Int64,
    first: Pointer[mut=True, Int64, _],
) -> Bool:
    if first[] == 0 and not kiro_put_byte(writer, 44):
        return False
    first[] = 0
    return (
        kiro_put_view_range(writer, view, key_start, key_end)
        and kiro_put_byte(writer, 58)
        and kiro_put_view_range(writer, view, value_start, value_end)
    )

def kiro_raw_rewrite_chat_request(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    issue: Pointer[mut=True, Int64, _],
) -> Bool:
    issue[] = KIRO_CHAT_REWRITE_ISSUE_NONE
    var root = kiro_raw_root(view)
    if root[0] < 0:
        return False
    var input = kiro_raw_member(view, root, StringSlice("input"))
    var has_input = kiro_raw_present(input)
    var messages = kiro_raw_member(view, root, StringSlice("messages"))
    if not has_input:
        if not kiro_raw_present(messages):
            issue[] = KIRO_CHAT_REWRITE_ISSUE_MISSING_MESSAGES
            return True
        if deepseek_json_byte(view, messages[0]) != 91:
            issue[] = KIRO_CHAT_REWRITE_ISSUE_INVALID_MESSAGES
            return True

    if not kiro_put_byte(writer, 123):
        return False
    var first: Int64 = 1
    var first_ptr = Pointer(to=first)
    var cursor = deepseek_json_skip_ws(view, root[0] + 1, root[1] - 1)
    while cursor < root[1] - 1:
        var key_start = cursor
        var key_end = deepseek_json_string_end(view, key_start, root[1] - 1)
        if key_end < 0:
            return False
        cursor = deepseek_json_skip_ws(view, key_end, root[1] - 1)
        if cursor >= root[1] - 1 or deepseek_json_byte(view, cursor) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, cursor + 1, root[1] - 1)
        var value_end = deepseek_json_value_end(view, value_start, root[1] - 1, 0)
        if value_end < 0:
            return False
        if not kiro_raw_chat_drop_key(
            view, key_start, key_end, has_input
        ):
            if not kiro_raw_put_member(
                writer,
                view,
                key_start,
                key_end,
                value_start,
                value_end,
                first_ptr,
            ):
                return False
        cursor = deepseek_json_skip_ws(view, value_end, root[1] - 1)
        if cursor < root[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, root[1] - 1)
            continue
        if cursor != root[1] - 1:
            return False
        break

    if not has_input:
        if first == 0 and not kiro_put_byte(writer, 44):
            return False
        first = 0
        if (
            not kiro_put_literal(writer, StringSlice('"input":'))
            or not kiro_raw_write_chat_input(writer, view, messages)
        ):
            return False
    return kiro_put_byte(writer, 125)

def kiro_chat_request_rewrite_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    issue_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return KIRO_KERNEL_STATUS_ABI
    if (
        input_length < 0
        or input_length > KIRO_KERNEL_MAX_BYTES
        or output_capacity <= 0
        or input_address == 0
        or output_address == 0
        or written_address == 0
        or issue_address == 0
    ):
        return KIRO_KERNEL_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, KIRO_KERNEL_MAX_BYTES):
        return KIRO_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var issue = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(issue_address)
    )
    written[] = 0
    issue[] = KIRO_CHAT_REWRITE_ISSUE_NONE
    var writer = KiroResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not kiro_raw_rewrite_chat_request(writer_ptr, view, issue):
        if writer.written >= output_capacity:
            written[] = writer.written
            return KIRO_KERNEL_STATUS_CAPACITY
        return KIRO_KERNEL_STATUS_INVALID
    written[] = writer.written
    return KIRO_KERNEL_STATUS_OK

def kiro_raw_string_empty(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    return (
        kiro_raw_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and bounds[1] - bounds[0] == 2
    )

def kiro_raw_put_prefixed_string_token(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    prefix: StringSlice,
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not kiro_raw_present(bounds) or deepseek_json_byte(view, bounds[0]) != 34:
        return False
    if not kiro_put_byte(writer, 34) or not kiro_put_literal(writer, prefix):
        return False
    if not kiro_put_view_range(writer, view, bounds[0] + 1, bounds[1] - 1):
        return False
    return kiro_put_byte(writer, 34)

def kiro_raw_json_positive_integer(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not kiro_raw_present(bounds):
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0]
    if index >= bounds[1]:
        return False
    while index < bounds[1]:
        var value = ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            return False
        index += 1
    return True

def kiro_raw_first_output_message_text(
    view: ProdexRichStringView,
    output: InlineArray[Int64, 2],
) -> InlineArray[Int64, 2]:
    var missing = InlineArray[Int64, 2](fill=-1)
    if not kiro_raw_present(output) or deepseek_json_byte(view, output[0]) != 91:
        return missing^
    var cursor = deepseek_json_skip_ws(view, output[0] + 1, output[1] - 1)
    while cursor < output[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, output[1] - 1, 0)
        if item_end < 0:
            return missing^
        if deepseek_json_byte(view, cursor) == 123:
            var item = InlineArray[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            var kind = kiro_raw_member(view, item, StringSlice("type"))
            if kiro_raw_present(kind) and deepseek_json_raw_equals(
                view, kind[0], kind[1], StringSlice("message")
            ):
                var content = kiro_raw_member(view, item, StringSlice("content"))
                if kiro_raw_present(content) and deepseek_json_byte(view, content[0]) == 91:
                    var first = deepseek_json_skip_ws(
                        view, content[0] + 1, content[1] - 1
                    )
                    if first < content[1] - 1:
                        var first_end = deepseek_json_value_end(
                            view, first, content[1] - 1, 0
                        )
                        if first_end > first and deepseek_json_byte(view, first) == 123:
                            var first_item = InlineArray[Int64, 2](fill=-1)
                            first_item[0] = first
                            first_item[1] = first_end
                            var text = kiro_raw_member(
                                view, first_item, StringSlice("text")
                            )
                            if kiro_raw_present(text) and deepseek_json_byte(view, text[0]) == 34:
                                return text^
            cursor = deepseek_json_skip_ws(view, item_end, output[1] - 1)
            if cursor < output[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, output[1] - 1)
                continue
            break
        return missing^
    return missing^

def kiro_raw_function_call_count(
    view: ProdexRichStringView,
    output: InlineArray[Int64, 2],
) -> Int64:
    if not kiro_raw_present(output) or deepseek_json_byte(view, output[0]) != 91:
        return 0
    var count: Int64 = 0
    var cursor = deepseek_json_skip_ws(view, output[0] + 1, output[1] - 1)
    while cursor < output[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, output[1] - 1, 0)
        if item_end < 0:
            return -1
        if deepseek_json_byte(view, cursor) == 123:
            var item = InlineArray[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            var kind = kiro_raw_member(view, item, StringSlice("type"))
            if kiro_raw_present(kind) and deepseek_json_raw_equals(
                view, kind[0], kind[1], StringSlice("function_call")
            ):
                count += 1
        cursor = deepseek_json_skip_ws(view, item_end, output[1] - 1)
        if cursor < output[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, output[1] - 1)
            continue
        if cursor != output[1] - 1:
            return -1
        break
    return count

def kiro_raw_put_chat_tool_calls(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    output: InlineArray[Int64, 2],
) -> Bool:
    if not kiro_put_byte(writer, 91):
        return False
    var first_written = True
    var cursor = deepseek_json_skip_ws(view, output[0] + 1, output[1] - 1)
    while cursor < output[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, output[1] - 1, 0)
        if item_end < 0:
            return False
        if deepseek_json_byte(view, cursor) == 123:
            var item = InlineArray[Int64, 2](fill=-1)
            item[0] = cursor
            item[1] = item_end
            var kind = kiro_raw_member(view, item, StringSlice("type"))
            if kiro_raw_present(kind) and deepseek_json_raw_equals(
                view, kind[0], kind[1], StringSlice("function_call")
            ):
                if not first_written and not kiro_put_byte(writer, 44):
                    return False
                first_written = False
                var call_id = kiro_raw_member(view, item, StringSlice("call_id"))
                var name = kiro_raw_member(view, item, StringSlice("name"))
                var arguments = kiro_raw_member(view, item, StringSlice("arguments"))
                if (
                    not kiro_put_literal(writer, StringSlice('{"id":'))
                    or not kiro_raw_put_default_or_string(
                        writer, view, call_id, StringSlice('"call_kiro"')
                    )
                    or not kiro_put_literal(
                        writer, StringSlice(',"type":"function","function":{"name":')
                    )
                    or not kiro_raw_put_default_or_string(
                        writer, view, name, StringSlice('"tool_call"')
                    )
                    or not kiro_put_literal(writer, StringSlice(',"arguments":'))
                    or not kiro_raw_put_default_or_string(
                        writer, view, arguments, StringSlice('"{}"')
                    )
                    or not kiro_put_literal(writer, StringSlice("}}"))
                ):
                    return False
        cursor = deepseek_json_skip_ws(view, item_end, output[1] - 1)
        if cursor < output[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, output[1] - 1)
            continue
        if cursor != output[1] - 1:
            return False
        break
    return kiro_put_byte(writer, 93)

def kiro_raw_nested_member(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    first_key: StringSlice,
    second_key: StringSlice,
) -> InlineArray[Int64, 2]:
    var first = kiro_raw_member(view, root, first_key)
    if not kiro_raw_present(first) or deepseek_json_byte(view, first[0]) != 123:
        return InlineArray[Int64, 2](fill=-1)^
    return kiro_raw_member(view, first, second_key)

def kiro_raw_chat_response(
    writer: Pointer[mut=True, KiroResponseWriter, _],
    view: ProdexRichStringView,
    request_id: UInt64,
) -> Bool:
    var root = kiro_raw_root(view)
    if root[0] < 0:
        return False
    var response_id = kiro_raw_member(view, root, StringSlice("id"))
    var created = kiro_raw_member(view, root, StringSlice("created_at"))
    var model = kiro_raw_member(view, root, StringSlice("model"))
    var output = kiro_raw_member(view, root, StringSlice("output"))
    var assistant_text = kiro_raw_first_output_message_text(view, output)
    var function_call_count = kiro_raw_function_call_count(view, output)
    if function_call_count < 0:
        return False
    var has_tool_calls = function_call_count > 0
    var metadata = kiro_raw_member(view, root, StringSlice("metadata"))
    var kiro_metadata = kiro_raw_member(view, metadata, StringSlice("kiro"))
    var reasoning = kiro_raw_member(
        view, kiro_metadata, StringSlice("reasoning_content")
    )
    var status = kiro_raw_member(view, root, StringSlice("status"))
    var error = kiro_raw_member(view, root, StringSlice("error"))
    var error_message = kiro_raw_member(view, error, StringSlice("message"))
    var requested_model = kiro_raw_member(
        view, root, StringSlice("requested_model")
    )
    var incomplete = kiro_raw_member(
        view, root, StringSlice("incomplete_details")
    )
    var incomplete_reason = kiro_raw_member(
        view, incomplete, StringSlice("reason")
    )

    if not kiro_put_literal(writer, StringSlice('{"id":')):
        return False
    if kiro_raw_present(response_id) and deepseek_json_byte(view, response_id[0]) == 34:
        if not kiro_raw_put_prefixed_string_token(
            writer, StringSlice("chatcmpl_"), view, response_id
        ):
            return False
    elif (
        not kiro_put_literal(writer, StringSlice('"chatcmpl_kiro_'))
        or not kiro_put_u64(writer, request_id)
        or not kiro_put_byte(writer, 34)
    ):
        return False
    if not kiro_put_literal(writer, StringSlice(',"object":"chat.completion","created":')):
        return False
    if kiro_raw_json_positive_integer(view, created):
        if not kiro_put_view_range(writer, view, created[0], created[1]):
            return False
    elif not kiro_put_byte(writer, 48):
        return False
    if not kiro_put_literal(writer, StringSlice(',"model":')):
        return False
    if (
        not kiro_raw_put_default_or_string(
            writer, view, model, StringSlice('"kiro-cli"')
        )
        or not kiro_put_literal(
            writer, StringSlice(',"choices":[{"index":0,"message":{"role":"assistant","content":')
        )
    ):
        return False
    if kiro_raw_present(assistant_text) and not kiro_raw_string_empty(view, assistant_text):
        if not kiro_put_view_range(
            writer, view, assistant_text[0], assistant_text[1]
        ):
            return False
    elif has_tool_calls:
        if not kiro_put_literal(writer, StringSlice("null")):
            return False
    elif not kiro_put_literal(writer, StringSlice('""')):
        return False

    if has_tool_calls:
        if (
            not kiro_put_literal(writer, StringSlice(',"tool_calls":'))
            or not kiro_raw_put_chat_tool_calls(writer, view, output)
        ):
            return False
    if kiro_raw_present(reasoning) and not kiro_raw_string_empty(view, reasoning):
        if (
            not kiro_put_literal(writer, StringSlice(',"reasoning_content":'))
            or not kiro_put_view_range(writer, view, reasoning[0], reasoning[1])
        ):
            return False
    if (
        kiro_raw_present(status)
        and deepseek_json_raw_equals(view, status[0], status[1], StringSlice("failed"))
        and kiro_raw_present(error_message)
        and deepseek_json_byte(view, error_message[0]) == 34
    ):
        if (
            not kiro_put_literal(writer, StringSlice(',"refusal":'))
            or not kiro_put_view_range(
                writer, view, error_message[0], error_message[1]
            )
        ):
            return False
    if not kiro_put_literal(writer, StringSlice('},"finish_reason":"')):
        return False
    if has_tool_calls:
        if not kiro_put_literal(writer, StringSlice("tool_calls")):
            return False
    elif (
        kiro_raw_present(incomplete_reason)
        and deepseek_json_raw_equals(
            view,
            incomplete_reason[0],
            incomplete_reason[1],
            StringSlice("max_output_tokens"),
        )
    ):
        if not kiro_put_literal(writer, StringSlice("length")):
            return False
    elif not kiro_put_literal(writer, StringSlice("stop")):
        return False
    if not kiro_put_literal(writer, StringSlice('"}]')):
        return False
    if kiro_raw_present(requested_model):
        if (
            not kiro_put_literal(writer, StringSlice(',"requested_model":'))
            or not kiro_put_view_range(
                writer, view, requested_model[0], requested_model[1]
            )
        ):
            return False
    if kiro_raw_present(metadata):
        if (
            not kiro_put_literal(writer, StringSlice(',"metadata":'))
            or not kiro_put_view_range(writer, view, metadata[0], metadata[1])
        ):
            return False
    return kiro_put_byte(writer, 125)

def kiro_chat_response_rewrite_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    request_id: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return KIRO_KERNEL_STATUS_ABI
    if (
        input_length < 0
        or input_length > KIRO_KERNEL_MAX_BYTES
        or output_capacity <= 0
        or input_address == 0
        or output_address == 0
        or written_address == 0
    ):
        return KIRO_KERNEL_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, KIRO_KERNEL_MAX_BYTES):
        return KIRO_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var writer = KiroResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not kiro_raw_chat_response(writer_ptr, view, request_id):
        if writer.written >= output_capacity:
            written[] = writer.written
            return KIRO_KERNEL_STATUS_CAPACITY
        return KIRO_KERNEL_STATUS_INVALID
    written[] = writer.written
    return KIRO_KERNEL_STATUS_OK
