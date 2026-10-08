from std.memory import Pointer

from json_sink import JsonSink, js_byte, js_escaped, js_literal, js_raw_view, js_string
from parsed_json import (
    JSON_OBJECT,
    JSON_STRING,
    ParsedJson,
    ParsedJsonNode,
    pj_field,
    pj_kind,
    pj_string_field,
    pj_text,
    pj_valid,
)
from rich_text import rich_trim_bounds, rich_view_prefix, rich_view_ptr
from rich_types import ProdexRichStringView

comptime GEMINI_RESPONSE_MEDIA_ABI_VERSION: Int64 = 1
comptime GEMINI_RESPONSE_MEDIA_MAX_BYTES: Int64 = 67_108_864
comptime GEMINI_RESPONSE_MEDIA_CONTENT: Int64 = 0
comptime GEMINI_RESPONSE_MEDIA_SPECIAL_TEXT: Int64 = 1
comptime GEMINI_RESPONSE_MEDIA_IMAGE_GENERATION: Int64 = 2


def media_field(
    tree: ParsedJson, parent: Int64, primary: StringSlice, fallback: StringSlice
) -> Int64:
    var value = pj_field(tree, parent, primary)
    if value < 0:
        value = pj_field(tree, parent, fallback)
    return value


def media_image_mime(mime: ProdexRichStringView) -> Bool:
    return rich_view_prefix["image/"](mime, False)


def media_mime_for_uri(uri: ProdexRichStringView) -> ProdexRichStringView:
    var ptr = rich_view_ptr(uri)
    var length = Int64(uri.len)
    var dot: Int64 = -1
    var index = length - 1
    while index >= 0:
        if ptr[unsafe_offset=index] == 46:
            dot = index
            break
        index -= 1
    if dot < 0 or dot + 1 >= length:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(uri.ptr + UInt(dot + 1), UInt(length - dot - 1))


def media_mime_literal(value: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def media_mime_label(uri: ProdexRichStringView) -> ProdexRichStringView:
    var extension = media_mime_for_uri(uri)
    if rich_view_prefix["png"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("image/png"))
    if (rich_view_prefix["jpg"](extension, True) and extension.len == 3) or (
        rich_view_prefix["jpeg"](extension, True) and extension.len == 4
    ):
        return media_mime_literal(StringSlice("image/jpeg"))
    if rich_view_prefix["gif"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("image/gif"))
    if rich_view_prefix["webp"](extension, True) and extension.len == 4:
        return media_mime_literal(StringSlice("image/webp"))
    if rich_view_prefix["mp3"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("audio/mpeg"))
    if rich_view_prefix["wav"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("audio/wav"))
    if rich_view_prefix["mp4"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("video/mp4"))
    if rich_view_prefix["mov"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("video/quicktime"))
    if rich_view_prefix["pdf"](extension, True) and extension.len == 3:
        return media_mime_literal(StringSlice("application/pdf"))
    return media_mime_literal(StringSlice("application/octet-stream"))


def media_put_u64(sink: Pointer[mut=True, JsonSink, _], value: UInt64):
    if value == 0:
        js_byte(sink, 48)
        return
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        js_byte(sink, UInt8(remaining / divisor) + 48)
        remaining %= divisor
        divisor /= 10


def media_put_inline_text(
    sink: Pointer[mut=True, JsonSink, _], mime: ProdexRichStringView, data: ProdexRichStringView
):
    js_literal(sink, StringSlice('{"type":"output_text","text":"Gemini returned inline '))
    js_escaped(sink, mime)
    js_literal(sink, StringSlice(" media ("))
    media_put_u64(sink, UInt64(data.len))
    js_literal(sink, StringSlice(' base64 characters."}'))


def media_put_content(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    part: Int64,
) -> Bool:
    var inline_data = media_field(tree, part, StringSlice("inlineData"), StringSlice("inline_data"))
    if inline_data >= 0:
        var mime_node = media_field(tree, inline_data, StringSlice("mimeType"), StringSlice("mime_type"))
        var data_node = pj_string_field(tree, inline_data, StringSlice("data"))
        if data_node < 0:
            return False
        var mime = pj_text(tree, mime_node) if mime_node >= 0 else ProdexRichStringView(0, 0)
        var data = pj_text(tree, data_node)
        if mime.len == 0:
            mime = ProdexRichStringView(
                UInt(Int(StringSlice("application/octet-stream").unsafe_ptr())),
                UInt(StringSlice("application/octet-stream").byte_length()),
            )
        if media_image_mime(mime):
            js_literal(sink, StringSlice('{"type":"input_image","image_url":"data:'))
            js_escaped(sink, mime)
            js_literal(sink, StringSlice(";base64,"))
            js_escaped(sink, data)
            js_literal(sink, StringSlice('"}'))
        else:
            media_put_inline_text(sink, mime, data)
        return True

    var file_data = media_field(tree, part, StringSlice("fileData"), StringSlice("file_data"))
    if file_data >= 0:
        var uri_node = media_field(tree, file_data, StringSlice("fileUri"), StringSlice("file_uri"))
        if uri_node < 0 or pj_kind(tree, uri_node) != JSON_STRING:
            return False
        var uri = pj_text(tree, uri_node)
        var mime_node = media_field(tree, file_data, StringSlice("mimeType"), StringSlice("mime_type"))
        var mime = (
            pj_text(tree, mime_node)
            if pj_kind(tree, mime_node) == JSON_STRING
            else media_mime_label(uri)
        )
        if media_image_mime(mime):
            js_literal(sink, StringSlice('{"type":"input_image","image_url":'))
            js_string(sink, uri)
            js_byte(sink, 125)
        else:
            js_literal(sink, StringSlice('{"type":"output_text","text":"Gemini returned '))
            js_escaped(sink, mime)
            js_literal(sink, StringSlice(" media: "))
            js_escaped(sink, uri)
            js_literal(sink, StringSlice('"}'))
        return True

    var text_node = pj_string_field(tree, part, StringSlice("text"))
    if text_node < 0:
        return False
    var text = pj_text(tree, text_node)
    if not rich_view_prefix["data:"](text, False):
        return False
    var ptr = rich_view_ptr(text)
    var comma: Int64 = -1
    var index: Int64 = 5
    while index < Int64(text.len):
        if ptr[unsafe_offset=index] == 44:
            comma = index
            break
        index += 1
    if comma < 0 or comma < 12:
        return False
    var metadata = ProdexRichStringView(text.ptr + UInt(5), UInt(comma - 5))
    if not rich_view_prefix[";base64"](metadata, False) or metadata.len < 7:
        return False
    var mime = ProdexRichStringView(metadata.ptr, metadata.len - UInt(7))
    var data = ProdexRichStringView(text.ptr + UInt(comma + 1), text.len - UInt(comma + 1))
    if media_image_mime(mime):
        js_literal(sink, StringSlice('{"type":"input_image","image_url":"data:'))
        js_escaped(sink, mime)
        js_literal(sink, StringSlice(";base64,"))
        js_escaped(sink, data)
        js_literal(sink, StringSlice('"}'))
    else:
        media_put_inline_text(sink, mime, data)
    return True


def media_put_special_text(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, part: Int64
) -> Bool:
    var executable = pj_field(tree, part, StringSlice("executableCode"))
    if executable >= 0:
        var language_node = pj_string_field(tree, executable, StringSlice("language"))
        var code_node = pj_string_field(tree, executable, StringSlice("code"))
        var language = pj_text(tree, language_node) if language_node >= 0 else ProdexRichStringView(
            UInt(Int(StringSlice("text").unsafe_ptr())), UInt(StringSlice("text").byte_length())
        )
        var code = pj_text(tree, code_node) if code_node >= 0 else ProdexRichStringView(0, 0)
        var bounds = rich_trim_bounds(code)
        if bounds[1] <= bounds[0]:
            return False
        js_byte(sink, 34)
        js_literal(sink, StringSlice("Gemini executable code ("))
        js_escaped(sink, language)
        js_literal(sink, StringSlice("):\\n```"))
        js_escaped(sink, language)
        js_literal(sink, StringSlice("\\n"))
        js_escaped(sink, code)
        js_literal(sink, StringSlice("\\n```"))
        js_byte(sink, 34)
        return True

    var result = pj_field(tree, part, StringSlice("codeExecutionResult"))
    if result >= 0:
        var outcome_node = pj_string_field(tree, result, StringSlice("outcome"))
        var output_node = pj_string_field(tree, result, StringSlice("output"))
        var outcome = pj_text(tree, outcome_node) if outcome_node >= 0 else ProdexRichStringView(
            UInt(Int(StringSlice("OUTCOME_UNSPECIFIED").unsafe_ptr())), UInt(StringSlice("OUTCOME_UNSPECIFIED").byte_length())
        )
        var output = pj_text(tree, output_node) if output_node >= 0 else ProdexRichStringView(0, 0)
        js_byte(sink, 34)
        js_literal(sink, StringSlice("Gemini code execution result ("))
        js_escaped(sink, outcome)
        js_literal(sink, StringSlice("):\\n```text\\n"))
        js_escaped(sink, output)
        js_literal(sink, StringSlice("\\n```"))
        js_byte(sink, 34)
        return True

    var metadata = pj_field(tree, part, StringSlice("videoMetadata"))
    if metadata >= 0:
        js_byte(sink, 34)
        js_literal(sink, StringSlice("Gemini video metadata: "))
        js_escaped(sink, js_raw_view(tree, metadata))
        js_byte(sink, 34)
        return True
    return False


def media_put_image_generation(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    part: Int64,
    response_id: Int64,
    index: UInt64,
) -> Bool:
    var inline_data = media_field(tree, part, StringSlice("inlineData"), StringSlice("inline_data"))
    if inline_data < 0:
        return False
    var mime_node = media_field(tree, inline_data, StringSlice("mimeType"), StringSlice("mime_type"))
    var data_node = pj_string_field(tree, inline_data, StringSlice("data"))
    if data_node < 0:
        return False
    var mime = pj_text(tree, mime_node) if mime_node >= 0 else ProdexRichStringView(0, 0)
    if not media_image_mime(mime):
        return False
    js_literal(sink, StringSlice('{"type":"image_generation_call","id":"ig_'))
    js_escaped(sink, pj_text(tree, response_id))
    js_byte(sink, 95)
    media_put_u64(sink, index)
    js_literal(sink, StringSlice('","status":"completed","result":'))
    js_string(sink, pj_text(tree, data_node))
    js_byte(sink, 125)
    return True


def gemini_response_media_kernel_v1(
    abi: Int64,
    operation: Int64,
    flag: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    scratch_address: UInt,
    scratch_count: Int64,
    measuring: Int64,
    output_address: UInt,
    output_capacity: Int64,
    metadata_address: UInt,
) abi("C") -> Int64:
    if abi != GEMINI_RESPONSE_MEDIA_ABI_VERSION:
        return 4
    if (
        operation < GEMINI_RESPONSE_MEDIA_CONTENT
        or operation > GEMINI_RESPONSE_MEDIA_IMAGE_GENERATION
        or flag != 0
        or nodes_address == 0
        or nodes_count <= 0
        or nodes_count > GEMINI_RESPONSE_MEDIA_MAX_BYTES
        or raw_address == 0
        or raw_length <= 0
        or raw_length > GEMINI_RESPONSE_MEDIA_MAX_BYTES
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
        ProdexRichStringView(UInt(Int(raw_address)), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return 1
    var part = pj_field(tree, 0, StringSlice("part"))
    if pj_kind(tree, part) != JSON_OBJECT:
        return 1

    var context = pj_string_field(tree, 0, StringSlice("response_id"))
    var index_node = pj_field(tree, 0, StringSlice("index"))
    var index: UInt64 = 0
    if operation == GEMINI_RESPONSE_MEDIA_IMAGE_GENERATION:
        if context < 0 or pj_kind(tree, index_node) != JSON_STRING:
            return 1
        var number = pj_text(tree, index_node)
        var ptr = rich_view_ptr(number)
        for offset in range(Int64(number.len)):
            var value = ptr[unsafe_offset=offset]
            if value < 48 or value > 57:
                return 1
            var digit = UInt64(value - 48)
            if index > 1844674407370955161 or (
                index == 1844674407370955161 and digit > 5
            ):
                return 1
            index = index * 10 + digit

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var sink = JsonSink(output, output_capacity, 0, measuring == 1, False)
    var sink_ptr = Pointer(to=sink)
    var present = False
    if operation == GEMINI_RESPONSE_MEDIA_CONTENT:
        present = media_put_content(sink_ptr, tree, part)
    elif operation == GEMINI_RESPONSE_MEDIA_SPECIAL_TEXT:
        present = media_put_special_text(sink_ptr, tree, part)
    else:
        present = media_put_image_generation(sink_ptr, tree, part, context, index)
    var metadata_ptr = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata_ptr[unsafe_offset=0] = Int64(present and not sink.failed)
    metadata_ptr[unsafe_offset=1] = sink.written
    if sink.failed:
        return 3
    return 0
