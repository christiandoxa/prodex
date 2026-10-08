from std.memory import Pointer

from json_sink import JsonSink, js_byte, js_escaped, js_literal, js_raw, js_raw_view, js_string, js_view
from parsed_json import (
    JSON_ARRAY,
    JSON_NULL,
    JSON_OBJECT,
    JSON_STRING,
    JSON_TRUE,
    ParsedJson,
    ParsedJsonNode,
    pj_child,
    pj_equal,
    pj_field,
    pj_kind,
    pj_next,
    pj_string_field,
    pj_text,
    pj_trim_bounds,
    pj_valid,
)
from rich_types import ProdexRichStringView

comptime GEMINI_CHAT_ASSISTANT_ABI_VERSION: Int64 = 1
comptime GEMINI_CHAT_ASSISTANT_PLAN: Int64 = 0
comptime GEMINI_CHAT_ASSISTANT_ASSEMBLE: Int64 = 1
comptime GEMINI_CHAT_ASSISTANT_MAX_BYTES: Int64 = 67_108_864


def gemini_chat_array_item(tree: ParsedJson, array: Int64, index: Int64) -> Int64:
    var item = pj_child(tree, array)
    for _ in range(index):
        item = pj_next(tree, item)
    return item


def gemini_chat_parts(tree: ParsedJson, response: Int64) -> Int64:
    var candidates = pj_field(tree, response, StringSlice("candidates"))
    if pj_kind(tree, candidates) != JSON_ARRAY:
        return -1
    var candidate = pj_child(tree, candidates)
    var content = pj_field(tree, candidate, StringSlice("content"))
    var parts = pj_field(tree, content, StringSlice("parts"))
    return parts if pj_kind(tree, parts) == JSON_ARRAY else -1


def gemini_chat_plan_tool_callbacks(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    response: Int64,
) -> Bool:
    js_byte(writer, 91)
    var first = True
    var parts = gemini_chat_parts(tree, response)
    var part = pj_child(tree, parts)
    while part >= 0:
        var call = pj_field(tree, part, StringSlice("functionCall"))
        if call >= 0:
            if not first:
                js_byte(writer, 44)
            js_literal(writer, StringSlice('{"name":'))
            var name = pj_string_field(tree, call, StringSlice("name"))
            if name >= 0:
                js_string(writer, pj_text(tree, name))
            else:
                js_literal(writer, StringSlice('"tool_call"'))
            js_literal(writer, StringSlice(',"args":'))
            var arguments = pj_field(tree, call, StringSlice("args"))
            if arguments >= 0:
                js_raw(writer, tree, arguments)
            else:
                js_literal(writer, StringSlice("{}"))
            js_byte(writer, 125)
            first = False
        part = pj_next(tree, part)
    js_byte(writer, 93)
    return not writer[].failed


def gemini_chat_suppresses_visible_text(tree: ParsedJson, parts: Int64) -> Bool:
    var part = pj_child(tree, parts)
    while part >= 0:
        if pj_field(tree, part, StringSlice("functionCall")) >= 0:
            return True
        part = pj_next(tree, part)
    return False


def gemini_chat_part_data_valid(tree: ParsedJson, data: Int64) -> Bool:
    if pj_kind(tree, data) != JSON_OBJECT:
        return False
    var visible = pj_field(tree, data, StringSlice("visible"))
    var special = pj_field(tree, data, StringSlice("special"))
    var media = pj_field(tree, data, StringSlice("media"))
    return (
        visible >= 0
        and (pj_kind(tree, visible) == JSON_NULL or pj_kind(tree, visible) == JSON_STRING)
        and special >= 0
        and (pj_kind(tree, special) == JSON_NULL or pj_kind(tree, special) == JSON_STRING)
        and media >= 0
        and (pj_kind(tree, media) == JSON_NULL or pj_kind(tree, media) == JSON_OBJECT)
    )


def gemini_chat_array_matches_parts(
    tree: ParsedJson, parts: Int64, values: Int64
) -> Bool:
    if pj_kind(tree, values) != JSON_ARRAY:
        return False
    var part = pj_child(tree, parts)
    var item = pj_child(tree, values)
    while part >= 0 and item >= 0:
        if not gemini_chat_part_data_valid(tree, item):
            return False
        part = pj_next(tree, part)
        item = pj_next(tree, item)
    return part < 0 and item < 0


def gemini_chat_native_indices(
    tree: ParsedJson,
    parts: Int64,
    data_array: Int64,
    scratch: Pointer[mut=True, Int64, _],
    scratch_capacity: Int64,
) -> Int64:
    var count: Int64 = 0
    var part = pj_child(tree, parts)
    var data = pj_child(tree, data_array)
    while part >= 0 and data >= 0:
        var media = pj_field(tree, data, StringSlice("media"))
        var has_media = pj_kind(tree, media) != JSON_NULL
        var has_video = pj_field(tree, part, StringSlice("videoMetadata")) >= 0
        if has_media:
            if count >= scratch_capacity:
                return -1
            scratch[unsafe_offset=count] = part
            count += 1
        elif has_video:
            var duplicate = False
            # ponytail: O(n²) dedup; hash/index table if large part arrays make it measurable.
            for prior in range(count):
                if pj_equal(
                    js_raw_view(tree, scratch[unsafe_offset=prior]),
                    js_raw_view(tree, part),
                ):
                    duplicate = True
                    break
            if not duplicate:
                if count >= scratch_capacity:
                    return -1
                scratch[unsafe_offset=count] = part
                count += 1
        part = pj_next(tree, part)
        data = pj_next(tree, data)
    return count if part < 0 and data < 0 else -1


def gemini_chat_metrics(
    tree: ParsedJson,
    parts: Int64,
    data_array: Int64,
    blocked_array: Int64,
    wrapped_array: Int64,
    suppress_visible: Bool,
) -> Array[Int64, 4]:
    var result = Array[Int64, 4](fill=0)
    var part = pj_child(tree, parts)
    var data = pj_child(tree, data_array)
    var blocked = pj_child(tree, blocked_array)
    var wrapped = pj_child(tree, wrapped_array)
    while part >= 0 and data >= 0:
        var visible = pj_field(tree, data, StringSlice("visible"))
        var special = pj_field(tree, data, StringSlice("special"))
        var media = pj_field(tree, data, StringSlice("media"))
        var source_text = pj_string_field(tree, part, StringSlice("text"))
        var is_thought = pj_kind(
            tree, pj_field(tree, part, StringSlice("thought"))
        ) == JSON_TRUE
        if is_thought and source_text >= 0:
            result[1] += Int64(pj_text(tree, source_text).len)
        elif not suppress_visible and visible >= 0 and pj_kind(tree, visible) == JSON_STRING:
            result[0] += Int64(pj_text(tree, visible).len)
        if special >= 0 and pj_kind(tree, special) == JSON_STRING:
            if result[0] > 0:
                result[0] += 1
            result[0] += Int64(pj_text(tree, special).len)
        if pj_kind(tree, media) != JSON_NULL:
            result[2] += 1

        var call = pj_field(tree, part, StringSlice("functionCall"))
        if call >= 0:
            if pj_kind(tree, blocked) == JSON_STRING:
                if result[0] > 0:
                    result[0] += 1
                result[0] += Int64(pj_text(tree, blocked).len)
            elif pj_kind(tree, blocked) == JSON_NULL:
                result[3] += 1
            else:
                return Array[Int64, 4](fill=-1)^
            if pj_kind(tree, wrapped) != JSON_STRING:
                return Array[Int64, 4](fill=-1)^
            blocked = pj_next(tree, blocked)
            wrapped = pj_next(tree, wrapped)
        part = pj_next(tree, part)
        data = pj_next(tree, data)
    if part >= 0 or data >= 0 or blocked >= 0 or wrapped >= 0:
        return Array[Int64, 4](fill=-1)^
    return result^


def gemini_chat_put_text(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    parts: Int64,
    data_array: Int64,
    blocked_array: Int64,
    suppress_visible: Bool,
):
    js_byte(writer, 34)
    var part = pj_child(tree, parts)
    var data = pj_child(tree, data_array)
    var blocked = pj_child(tree, blocked_array)
    var has_text = False
    while part >= 0 and data >= 0:
        var visible = pj_field(tree, data, StringSlice("visible"))
        var special = pj_field(tree, data, StringSlice("special"))
        var is_thought = pj_kind(
            tree, pj_field(tree, part, StringSlice("thought"))
        ) == JSON_TRUE
        if not is_thought and not suppress_visible and visible >= 0 and pj_kind(tree, visible) == JSON_STRING:
            var visible_text = pj_text(tree, visible)
            js_escaped(writer, visible_text)
            has_text = has_text or visible_text.len > 0
        if special >= 0 and pj_kind(tree, special) == JSON_STRING:
            if has_text:
                js_escaped(writer, ProdexRichStringView(UInt(Int(StringSlice("\n").unsafe_ptr())), 1))
            var special_text = pj_text(tree, special)
            js_escaped(writer, special_text)
            has_text = has_text or special_text.len > 0

        var call = pj_field(tree, part, StringSlice("functionCall"))
        if call >= 0:
            if pj_kind(tree, blocked) == JSON_STRING:
                if has_text:
                    js_escaped(writer, ProdexRichStringView(UInt(Int(StringSlice("\n").unsafe_ptr())), 1))
                var blocked_text = pj_text(tree, blocked)
                js_escaped(writer, blocked_text)
                has_text = has_text or blocked_text.len > 0
            blocked = pj_next(tree, blocked)
        part = pj_next(tree, part)
        data = pj_next(tree, data)
    js_byte(writer, 34)


def gemini_chat_put_reasoning(
    writer: Pointer[mut=True, JsonSink, _], tree: ParsedJson, parts: Int64
):
    js_byte(writer, 34)
    var part = pj_child(tree, parts)
    while part >= 0:
        var source_text = pj_string_field(tree, part, StringSlice("text"))
        var is_thought = pj_kind(
            tree, pj_field(tree, part, StringSlice("thought"))
        ) == JSON_TRUE
        if is_thought and source_text >= 0:
            js_escaped(writer, pj_text(tree, source_text))
        part = pj_next(tree, part)
    js_byte(writer, 34)


def gemini_chat_put_media(
    writer: Pointer[mut=True, JsonSink, _], tree: ParsedJson, data_array: Int64
):
    js_byte(writer, 91)
    var first = True
    var data = pj_child(tree, data_array)
    while data >= 0:
        var media = pj_field(tree, data, StringSlice("media"))
        if pj_kind(tree, media) != JSON_NULL:
            if not first:
                js_byte(writer, 44)
            js_raw(writer, tree, media)
            first = False
        data = pj_next(tree, data)
    js_byte(writer, 93)


def gemini_chat_put_native_parts(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    native_indices: Pointer[mut=True, Int64, _],
    native_count: Int64,
):
    js_byte(writer, 91)
    for index in range(native_count):
        if index > 0:
            js_byte(writer, 44)
        js_raw(writer, tree, native_indices[unsafe_offset=index])
    js_byte(writer, 93)


def gemini_chat_put_u64(writer: Pointer[mut=True, JsonSink, _], value: UInt64):
    if value == 0:
        js_byte(writer, 48)
        return
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        js_byte(writer, UInt8(remaining / divisor) + 48)
        remaining %= divisor
        divisor /= 10


def gemini_chat_put_tool_calls(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    parts: Int64,
    blocked_array: Int64,
    wrapped_array: Int64,
    request_id: Int64,
):
    js_byte(writer, 91)
    var first = True
    var part_index: Int64 = 0
    var part = pj_child(tree, parts)
    var blocked = pj_child(tree, blocked_array)
    var wrapped = pj_child(tree, wrapped_array)
    var request_id_text = pj_text(tree, request_id)
    while part >= 0:
        var call = pj_field(tree, part, StringSlice("functionCall"))
        if call >= 0:
            if pj_kind(tree, blocked) == JSON_NULL:
                if not first:
                    js_byte(writer, 44)
                js_literal(writer, StringSlice('{"id":'))
                var explicit_id = pj_string_field(tree, call, StringSlice("id"))
                var id_bounds = pj_trim_bounds(pj_text(tree, explicit_id))
                if explicit_id >= 0 and id_bounds[1] > id_bounds[0]:
                    js_string(writer, pj_text(tree, explicit_id))
                else:
                    js_byte(writer, 34)
                    js_literal(writer, StringSlice("call_gemini_"))
                    js_view(writer, request_id_text)
                    js_byte(writer, 95)
                    gemini_chat_put_u64(writer, UInt64(part_index))
                    js_byte(writer, 34)
                js_literal(writer, StringSlice(',"type":"function","function":{"name":'))
                var name = pj_string_field(tree, call, StringSlice("name"))
                if name >= 0:
                    js_string(writer, pj_text(tree, name))
                else:
                    js_literal(writer, StringSlice('"tool_call"'))
                js_literal(writer, StringSlice(',"arguments":'))
                js_string(writer, pj_text(tree, wrapped))
                js_byte(writer, 125)
                var signature = pj_string_field(tree, part, StringSlice("thoughtSignature"))
                if signature < 0:
                    signature = pj_string_field(tree, call, StringSlice("thoughtSignature"))
                if signature >= 0:
                    js_literal(writer, StringSlice(',"gemini_thought_signature":'))
                    js_string(writer, pj_text(tree, signature))
                js_byte(writer, 125)
                first = False
            blocked = pj_next(tree, blocked)
            wrapped = pj_next(tree, wrapped)
        part = pj_next(tree, part)
        part_index += 1
    js_byte(writer, 93)


def gemini_chat_write_message(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    parts: Int64,
    data_array: Int64,
    blocked_array: Int64,
    wrapped_array: Int64,
    metadata: Int64,
    request_id: Int64,
    metrics: Array[Int64, 4],
    native_indices: Pointer[mut=True, Int64, _],
    native_count: Int64,
    suppress_visible: Bool,
) -> Bool:
    js_literal(writer, StringSlice('{"role":"assistant","content":'))
    if metrics[0] > 0:
        gemini_chat_put_text(writer, tree, parts, data_array, blocked_array, suppress_visible)
    elif metrics[3] > 0:
        js_literal(writer, StringSlice('""'))
    else:
        js_literal(writer, StringSlice("null"))
    if metrics[1] > 0:
        js_literal(writer, StringSlice(',"reasoning_content":'))
        gemini_chat_put_reasoning(writer, tree, parts)
    if metrics[2] > 0:
        js_literal(writer, StringSlice(',"gemini_media_content":'))
        gemini_chat_put_media(writer, tree, data_array)
    if native_count > 0:
        js_literal(writer, StringSlice(',"gemini_native_parts":'))
        gemini_chat_put_native_parts(writer, tree, native_indices, native_count)
    if pj_kind(tree, metadata) != JSON_NULL:
        js_literal(writer, StringSlice(',"gemini_metadata":'))
        js_raw(writer, tree, metadata)
    if metrics[3] > 0:
        js_literal(writer, StringSlice(',"tool_calls":'))
        gemini_chat_put_tool_calls(
            writer, tree, parts, blocked_array, wrapped_array, request_id
        )
    js_byte(writer, 125)
    return not writer[].failed


def gemini_chat_assemble(
    writer: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    response: Int64,
    data_array: Int64,
    blocked_array: Int64,
    wrapped_array: Int64,
    metadata: Int64,
    request_id: Int64,
    scratch: Pointer[mut=True, Int64, _],
    scratch_capacity: Int64,
) -> Tuple[Bool, Bool]:
    var parts = gemini_chat_parts(tree, response)
    if parts < 0:
        return (True, False)
    if not gemini_chat_array_matches_parts(tree, parts, data_array):
        return (False, False)
    if pj_kind(tree, blocked_array) != JSON_ARRAY or pj_kind(tree, wrapped_array) != JSON_ARRAY:
        return (False, False)
    if pj_kind(tree, metadata) == -1 or pj_kind(tree, request_id) != JSON_STRING:
        return (False, False)

    var callback_count: Int64 = 0
    var part = pj_child(tree, parts)
    while part >= 0:
        if pj_field(tree, part, StringSlice("functionCall")) >= 0:
            callback_count += 1
        part = pj_next(tree, part)
    var blocked_count: Int64 = 0
    var item = pj_child(tree, blocked_array)
    while item >= 0:
        if pj_kind(tree, item) != JSON_NULL and pj_kind(tree, item) != JSON_STRING:
            return (False, False)
        blocked_count += 1
        item = pj_next(tree, item)
    var wrapped_count: Int64 = 0
    item = pj_child(tree, wrapped_array)
    while item >= 0:
        if pj_kind(tree, item) != JSON_STRING:
            return (False, False)
        wrapped_count += 1
        item = pj_next(tree, item)
    if callback_count != blocked_count or callback_count != wrapped_count:
        return (False, False)

    var native_count = gemini_chat_native_indices(
        tree, parts, data_array, scratch, scratch_capacity
    )
    if native_count < 0:
        return (False, False)
    var suppress_visible = gemini_chat_suppresses_visible_text(tree, parts)
    var metrics = gemini_chat_metrics(
        tree,
        parts,
        data_array,
        blocked_array,
        wrapped_array,
        suppress_visible,
    )
    if metrics[0] < 0:
        return (False, False)
    var present = metrics[0] > 0 or metrics[1] > 0 or metrics[2] > 0 or metrics[3] > 0
    if not present:
        return (True, False)
    return (
        gemini_chat_write_message(
            writer,
            tree,
            parts,
            data_array,
            blocked_array,
            wrapped_array,
            metadata,
            request_id,
            metrics,
            scratch,
            native_count,
            suppress_visible,
        ),
        True,
    )


@export("prodex_mojo_gemini_chat_assistant_response_v1")
def gemini_chat_assistant_response_kernel_v1(
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
    output_capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != GEMINI_CHAT_ASSISTANT_ABI_VERSION:
        return 4
    if (
        operation < GEMINI_CHAT_ASSISTANT_PLAN
        or operation > GEMINI_CHAT_ASSISTANT_ASSEMBLE
        or flag != 0
        or nodes_address == 0
        or nodes_count <= 0
        or nodes_count > GEMINI_CHAT_ASSISTANT_MAX_BYTES
        or raw_length <= 0
        or raw_length > GEMINI_CHAT_ASSISTANT_MAX_BYTES
        or scratch_address == 0
        or scratch_count < nodes_count
        or metadata_address == 0
        or measuring < 0
        or measuring > 1
        or output_capacity < 0
    ):
        return 1
    if measuring == 1:
        if output_address != 0 or output_capacity != 0:
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
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var root_count: Int64 = 0
    var root_item = pj_child(tree, 0)
    while root_item >= 0:
        root_count += 1
        root_item = pj_next(tree, root_item)
    if operation == GEMINI_CHAT_ASSISTANT_PLAN and root_count != 1:
        return 1
    if operation == GEMINI_CHAT_ASSISTANT_ASSEMBLE and root_count != 6:
        return 1

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = JsonSink(output, output_capacity, 0, measuring == 1, False)
    var writer_ptr = Pointer(to=writer)
    var present = True
    var success = True
    if operation == GEMINI_CHAT_ASSISTANT_PLAN:
        success = gemini_chat_plan_tool_callbacks(
            writer_ptr, tree, pj_child(tree, 0)
        )
    else:
        var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(scratch_address)
        )
        var assembled = gemini_chat_assemble(
            writer_ptr,
            tree,
            gemini_chat_array_item(tree, 0, 0),
            gemini_chat_array_item(tree, 0, 1),
            gemini_chat_array_item(tree, 0, 2),
            gemini_chat_array_item(tree, 0, 3),
            gemini_chat_array_item(tree, 0, 4),
            gemini_chat_array_item(tree, 0, 5),
            scratch,
            scratch_count,
        )
        success = assembled[0]
        present = assembled[1]

    var output_metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    output_metadata[unsafe_offset=0] = Int64(present)
    output_metadata[unsafe_offset=1] = writer.written
    if not success or writer.failed:
        return 3 if writer.failed else 1
    return 0
