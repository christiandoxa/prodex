from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime GEMINI_CONFIG_STATUS_OK: Int64 = 0
comptime GEMINI_CONFIG_STATUS_INVALID: Int64 = 1
comptime GEMINI_CONFIG_STATUS_UTF8: Int64 = 2
comptime GEMINI_CONFIG_STATUS_CAPACITY: Int64 = 3
comptime GEMINI_CONFIG_STATUS_ABI: Int64 = 4

comptime GEMINI_CONFIG_MODEL_USES_THINKING_LEVEL: Int64 = 1
comptime GEMINI_CONFIG_THINKING_CONFIG: Int64 = 2
comptime GEMINI_CONFIG_TEXT_FORMAT: Int64 = 3
comptime GEMINI_CONFIG_RESPONSE_FORMAT: Int64 = 4
comptime GEMINI_CONFIG_CONTINUATION_METADATA: Int64 = 5
comptime GEMINI_CONFIG_TOOL_CALL_SIGNATURES: Int64 = 6
comptime GEMINI_CONFIG_VALIDATE_CANDIDATE_COUNT: Int64 = 7


@fieldwise_init
struct GeminiConfigKernelInput(Copyable):
    var operation: Int64
    var number: UInt64
    var number_present: Int64
    var primary_present: Int64
    var secondary_present: Int64
    var tertiary_present: Int64
    var quaternary_present: Int64
    var primary: ProdexRichStringView
    var secondary: ProdexRichStringView
    var tertiary: ProdexRichStringView
    var quaternary: ProdexRichStringView


@fieldwise_init
struct GeminiConfigWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def gemini_config_put_byte(
    writer: Pointer[mut=True, GeminiConfigWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def gemini_config_put_literal(
    writer: Pointer[mut=True, GeminiConfigWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not gemini_config_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_config_put_u64(
    writer: Pointer[mut=True, GeminiConfigWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return gemini_config_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not gemini_config_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def gemini_config_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def gemini_config_view_equals[literal: StaticString](
    view: ProdexRichStringView, lowercase: Bool
) -> Bool:
    if view.len != UInt(literal.byte_length()) or not rich_view_valid(view, 4_194_304):
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        var value = left[unsafe_offset=index]
        if lowercase:
            value = gemini_config_lower(value)
        if value != right[unsafe_offset=index]:
            return False
    return True


def gemini_config_view_contains[literal: StaticString](
    view: ProdexRichStringView, lowercase: Bool
) -> Bool:
    if not rich_view_valid(view, 4_194_304):
        return False
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if length > Int64(view.len):
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for start in range(Int64(view.len) - length + 1):
        var matched = True
        for index in range(length):
            var value = left[unsafe_offset=start + index]
            if lowercase:
                value = gemini_config_lower(value)
            if value != right[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def gemini_config_range_equals[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, lowercase: Bool
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var right = literal.unsafe_ptr()
    for index in range(end - start):
        var value = ptr[unsafe_offset=start + index]
        if lowercase:
            value = gemini_config_lower(value)
        if value != right[unsafe_offset=index]:
            return False
    return True


def gemini_config_skip_space(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var index = start
    while index < length:
        var value = ptr[unsafe_offset=index]
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def gemini_config_json_value_end(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var index = gemini_config_skip_space(ptr, length, start)
    if index >= length:
        return -1
    var first = ptr[unsafe_offset=index]
    if first == 34:
        index += 1
        var escaped = False
        while index < length:
            var value = ptr[unsafe_offset=index]
            if escaped:
                escaped = False
            elif value == 92:
                escaped = True
            elif value == 34:
                return index + 1
            index += 1
        return -1
    if first == 123 or first == 91:
        var open = first
        var close = UInt8(125) if first == 123 else UInt8(93)
        var depth: Int64 = 0
        var in_string = False
        var escaped = False
        while index < length:
            var value = ptr[unsafe_offset=index]
            if in_string:
                if escaped:
                    escaped = False
                elif value == 92:
                    escaped = True
                elif value == 34:
                    in_string = False
            elif value == 34:
                in_string = True
            elif value == open:
                depth += 1
            elif value == close:
                depth -= 1
                if depth == 0:
                    return index + 1
            index += 1
        return -1
    while index < length:
        var value = ptr[unsafe_offset=index]
        if value == 44 or value == 125:
            break
        index += 1
    while index > start and (
        ptr[unsafe_offset=index - 1] == 9
        or ptr[unsafe_offset=index - 1] == 10
        or ptr[unsafe_offset=index - 1] == 13
        or ptr[unsafe_offset=index - 1] == 32
    ):
        index -= 1
    return index


def gemini_config_json_key_value(
    view: ProdexRichStringView, key: StringSlice
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if not rich_view_valid(view, 4_194_304) or view.len < 2:
        return result^
    var ptr = rich_view_ptr(view)
    var length = Int64(view.len)
    var key_length = Int64(key.byte_length())
    var key_ptr = key.unsafe_ptr()
    var index: Int64 = 0
    var depth: Int64 = 0
    var in_string = False
    var escaped = False
    while index < length:
        var value = ptr[unsafe_offset=index]
        if in_string:
            if escaped:
                escaped = False
            elif value == 92:
                escaped = True
            elif value == 34:
                in_string = False
            index += 1
            continue
        if value == 34:
            var start = index + 1
            var end = start
            while end < length and ptr[unsafe_offset=end] != 34:
                if ptr[unsafe_offset=end] == 92:
                    end += 1
                end += 1
            if depth == 1 and end - start == key_length:
                var matches = True
                for offset in range(key_length):
                    if ptr[unsafe_offset=start + offset] != key_ptr[unsafe_offset=offset]:
                        matches = False
                        break
                if matches:
                    var cursor = gemini_config_skip_space(ptr, length, end + 1)
                    if cursor < length and ptr[unsafe_offset=cursor] == 58:
                        var value_start = gemini_config_skip_space(ptr, length, cursor + 1)
                        var value_end = gemini_config_json_value_end(ptr, length, value_start)
                        if value_end >= value_start:
                            result[0] = value_start
                            result[1] = value_end
                            return result^
            index = end + 1
            continue
        if value == 123:
            depth += 1
        elif value == 125:
            depth -= 1
        index += 1
    return result^


def gemini_config_put_raw_range(
    writer: Pointer[mut=True, GeminiConfigWriter, _],
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    for index in range(start, end):
        if not gemini_config_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_config_put_json_string_literal(
    writer: Pointer[mut=True, GeminiConfigWriter, _], value: StringSlice
) -> Bool:
    if not gemini_config_put_byte(writer, 34):
        return False
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        var value = ptr[unsafe_offset=index]
        if value == 34 or value == 92:
            if not gemini_config_put_byte(writer, 92) or not gemini_config_put_byte(writer, value):
                return False
        else:
            if not gemini_config_put_byte(writer, value):
                return False
    return gemini_config_put_byte(writer, 34)


def gemini_config_model_uses_thinking_level(view: ProdexRichStringView) -> Bool:
    return gemini_config_view_contains["gemini-3"](view, True) or gemini_config_view_contains[
        "gemma-3"
    ](view, True) or gemini_config_view_contains["gemma-4"](view, True)


def gemini_config_effort_is(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    if view.len != UInt(literal.byte_length()) or not rich_view_valid(view, 64):
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if gemini_config_lower(left[unsafe_offset=index]) != gemini_config_lower(
            right[unsafe_offset=index]
        ):
            return False
    return True


def gemini_config_thinking(
    writer: Pointer[mut=True, GeminiConfigWriter, _],
    input: GeminiConfigKernelInput,
) -> Bool:
    var effort = input.secondary.copy()
    if input.secondary_present == 0:
        effort = ProdexRichStringView(0, 0)
    if gemini_config_effort_is(effort, StringSlice("none")) or gemini_config_effort_is(
        effort, StringSlice("minimal")
    ):
        return gemini_config_put_literal(
            writer, StringSlice('{"includeThoughts":false,"thinkingBudget":0}')
        )
    if gemini_config_model_uses_thinking_level(input.primary):
        var level = StringSlice("HIGH")
        if gemini_config_effort_is(effort, StringSlice("low")):
            level = StringSlice("LOW")
        elif gemini_config_effort_is(effort, StringSlice("medium")):
            level = StringSlice("MEDIUM")
        return gemini_config_put_literal(
            writer, StringSlice('{"includeThoughts":true,"thinkingLevel":"')
        ) and gemini_config_put_literal(writer, level) and gemini_config_put_literal(
            writer, StringSlice('"}')
        )
    var budget: UInt64 = 8_192
    if input.number_present != 0:
        budget = input.number
    elif gemini_config_effort_is(effort, StringSlice("low")):
        budget = 1_024
    elif gemini_config_effort_is(effort, StringSlice("xhigh")):
        budget = 24_576
    return gemini_config_put_literal(
        writer, StringSlice('{"includeThoughts":true,"thinkingBudget":')
    ) and gemini_config_put_u64(writer, budget) and gemini_config_put_byte(writer, 125)


def gemini_config_text_format(
    writer: Pointer[mut=True, GeminiConfigWriter, _], view: ProdexRichStringView
) -> Bool:
    var type = gemini_config_json_key_value(view, StringSlice("type"))
    if type[0] < 0:
        return gemini_config_put_literal(writer, StringSlice("{}"))
    var ptr = rich_view_ptr(view)
    var json_type = type[1] - type[0]
    if json_type < 2 or ptr[unsafe_offset=type[0]] != 34:
        return gemini_config_put_literal(writer, StringSlice("{}"))
    var type_start = type[0] + 1
    var type_end = type[1] - 1
    var is_object = gemini_config_range_equals["json_object"](
        ptr, type_start, type_end, False
    )
    if is_object:
        return gemini_config_put_literal(
            writer, StringSlice("{\"responseMimeType\":\"application/json\"}")
        )
    var is_schema = gemini_config_range_equals["json_schema"](
        ptr, type_start, type_end, False
    )
    if is_schema:
        var schema_key = gemini_config_json_key_value(view, StringSlice("schema"))
        if schema_key[0] < 0:
            schema_key = gemini_config_json_key_value(view, StringSlice("json_schema"))
        if schema_key[0] >= 0:
            return gemini_config_put_literal(
                writer, StringSlice('{"responseMimeType":"application/json","responseJsonSchema":')
            ) and gemini_config_put_raw_range(writer, ptr, schema_key[0], schema_key[1]) and gemini_config_put_byte(writer, 125)
        return gemini_config_put_literal(
            writer, StringSlice('{"responseMimeType":"application/json"}')
        )
    return gemini_config_put_literal(writer, StringSlice("{}"))


def gemini_config_candidate_count(
    writer: Pointer[mut=True, GeminiConfigWriter, _], view: ProdexRichStringView
) -> Bool:
    var snake = gemini_config_json_key_value(view, StringSlice("candidate_count"))
    var camel = gemini_config_json_key_value(view, StringSlice("candidateCount"))
    var ptr = rich_view_ptr(view)
    var snake_present = snake[0] >= 0 and not (
        snake[1] - snake[0] == 4 and ptr[unsafe_offset=snake[0]] == 110
    )
    var camel_present = camel[0] >= 0 and not (
        camel[1] - camel[0] == 4 and ptr[unsafe_offset=camel[0]] == 110
    )
    if snake_present and camel_present:
        var conflict = False
        var snake_len = snake[1] - snake[0]
        var camel_len = camel[1] - camel[0]
        if snake_len != camel_len:
            conflict = True
        else:
            for index in range(snake_len):
                if ptr[unsafe_offset=snake[0] + index] != ptr[unsafe_offset=camel[0] + index]:
                    conflict = True
                    break
        if conflict:
            return gemini_config_put_literal(writer, StringSlice("{\"conflict\":true}"))
    if snake_present and not (
        snake[1] - snake[0] == 1 and ptr[unsafe_offset=snake[0]] == 49
    ):
        return gemini_config_put_literal(
            writer, StringSlice('{"invalidField":"candidate_count"}')
        )
    if camel_present and not (
        camel[1] - camel[0] == 1 and ptr[unsafe_offset=camel[0]] == 49
    ):
        return gemini_config_put_literal(
            writer, StringSlice('{"invalidField":"candidateCount"}')
        )
    return gemini_config_put_literal(writer, StringSlice("{}"))


def gemini_config_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return GEMINI_CONFIG_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0:
        return GEMINI_CONFIG_STATUS_INVALID
    if output_capacity < 0:
        return GEMINI_CONFIG_STATUS_CAPACITY
    var input = Pointer[mut=False, GeminiConfigKernelInput, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var value = input[].copy()
    if value.operation < GEMINI_CONFIG_MODEL_USES_THINKING_LEVEL or value.operation > GEMINI_CONFIG_VALIDATE_CANDIDATE_COUNT:
        return GEMINI_CONFIG_STATUS_INVALID
    if value.number_present < 0 or value.number_present > 1 or value.primary_present < 0 or value.primary_present > 1 or value.secondary_present < 0 or value.secondary_present > 1 or value.tertiary_present < 0 or value.tertiary_present > 1 or value.quaternary_present < 0 or value.quaternary_present > 1:
        return GEMINI_CONFIG_STATUS_INVALID
    if value.primary_present != 0 and not rich_view_valid(value.primary, 4_194_304):
        return GEMINI_CONFIG_STATUS_UTF8
    if value.secondary_present != 0 and not rich_view_valid(value.secondary, 4_194_304):
        return GEMINI_CONFIG_STATUS_UTF8
    if value.tertiary_present != 0 and not rich_view_valid(value.tertiary, 4_194_304):
        return GEMINI_CONFIG_STATUS_UTF8
    if value.quaternary_present != 0 and not rich_view_valid(value.quaternary, 4_194_304):
        return GEMINI_CONFIG_STATUS_UTF8
    var writer = GeminiConfigWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    var ok: Bool
    if value.operation == GEMINI_CONFIG_MODEL_USES_THINKING_LEVEL:
        ok = gemini_config_put_literal(writer_ptr, StringSlice("true")) if gemini_config_model_uses_thinking_level(value.primary) else gemini_config_put_literal(writer_ptr, StringSlice("false"))
    elif value.operation == GEMINI_CONFIG_THINKING_CONFIG:
        ok = gemini_config_thinking(writer_ptr, value)
    elif value.operation == GEMINI_CONFIG_TEXT_FORMAT:
        ok = gemini_config_text_format(writer_ptr, value.primary)
    elif value.operation == GEMINI_CONFIG_VALIDATE_CANDIDATE_COUNT:
        ok = gemini_config_candidate_count(writer_ptr, value.primary)
    else:
        return GEMINI_CONFIG_STATUS_INVALID
    if not ok:
        if writer.written >= output_capacity:
            written[] = writer.written
            return GEMINI_CONFIG_STATUS_CAPACITY
        return GEMINI_CONFIG_STATUS_INVALID
    written[] = writer.written
    return GEMINI_CONFIG_STATUS_OK
