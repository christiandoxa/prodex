from std.memory import Pointer

from json_sink import JsonSink, js_escaped, js_literal, js_raw, js_string, js_view
from parsed_json import (
    ParsedJson,
    ParsedJsonNode,
    JSON_ARRAY,
    JSON_OBJECT,
    JSON_STRING,
    JSON_TRUE,
    pj_child,
    pj_equal,
    pj_field,
    pj_kind,
    pj_literal,
    pj_next,
    pj_string_field,
    pj_text,
    pj_valid,
)
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


comptime GEMINI_COMPACT_REWRITE_ABI_VERSION: Int64 = 1
comptime GEMINI_COMPACT_REWRITE_REQUEST: Int64 = 1
comptime GEMINI_COMPACT_REWRITE_SUMMARY: Int64 = 2
comptime GEMINI_COMPACT_REWRITE_STATUS_OK: Int64 = 0
comptime GEMINI_COMPACT_REWRITE_STATUS_INVALID: Int64 = 1
comptime GEMINI_COMPACT_REWRITE_STATUS_UTF8: Int64 = 2
comptime GEMINI_COMPACT_REWRITE_STATUS_CAPACITY: Int64 = 3
comptime GEMINI_COMPACT_REWRITE_STATUS_ABI: Int64 = 4
comptime GEMINI_COMPACT_REWRITE_MAX_BYTES: Int64 = 16_777_216

comptime GEMINI_COMPACT_REASON_PROVIDER: Int64 = 0
comptime GEMINI_COMPACT_REASON_TIMEOUT: Int64 = 1
comptime GEMINI_COMPACT_REASON_UNAVAILABLE: Int64 = 2
comptime GEMINI_COMPACT_REASON_UNSUPPORTED: Int64 = 3
comptime GEMINI_COMPACT_REASON_INVALID_RESPONSE: Int64 = 4

comptime GEMINI_COMPACT_INSTRUCTIONS = StringSlice(
    "Compact the supplied coding-agent transcript into one durable continuation summary. "
    "Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, "
    "commands and test results, unresolved failures, current worktree state, and the next concrete steps. "
    "Remove redundant narration and obsolete intermediate reasoning. Do not call tools. "
    "Return only the continuation summary, with no preamble or completion claim."
)
comptime GEMINI_COMPACT_SUMMARY_PREFIX = StringSlice(
    "Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:"
)


def gemini_compact_key(node: ParsedJsonNode, name: StringSlice) -> Bool:
    return pj_equal(node.key, pj_literal(name))


def gemini_compact_skip_request_key(node: ParsedJsonNode) -> Bool:
    # Keep previous_response_id in rewritten JSON so Rust's binding adapter
    # continues to pin this compact request to its owning Gemini profile.
    return (
        gemini_compact_key(node, StringSlice("include"))
        or gemini_compact_key(node, StringSlice("instructions"))
        or gemini_compact_key(node, StringSlice("model"))
        or gemini_compact_key(node, StringSlice("parallel_tool_calls"))
        or gemini_compact_key(node, StringSlice("prompt_cache_key"))
        or gemini_compact_key(node, StringSlice("prodex_gemini_compaction"))
        or gemini_compact_key(node, StringSlice("stream"))
        or gemini_compact_key(node, StringSlice("store"))
        or gemini_compact_key(node, StringSlice("text"))
        or gemini_compact_key(node, StringSlice("tool_choice"))
        or gemini_compact_key(node, StringSlice("tools"))
    )


def gemini_compact_write_instruction_item(
    sink: Pointer[mut=True, JsonSink, _]
) -> Bool:
    js_literal(
        sink,
        StringSlice(
            '{"type":"message","role":"user","content":[{"type":"input_text","text":"'
        ),
    )
    js_escaped(
        sink,
        ProdexRichStringView(
            UInt(Int(GEMINI_COMPACT_INSTRUCTIONS.unsafe_ptr())),
            UInt(GEMINI_COMPACT_INSTRUCTIONS.byte_length()),
        ),
    )
    js_literal(sink, StringSlice('"}]}'))
    return not sink[].failed


def gemini_compact_write_input(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, input: Int64
) -> Bool:
    if pj_kind(tree, input) != JSON_ARRAY:
        return False
    js_literal(sink, StringSlice("["))
    var child = pj_child(tree, input)
    var first = True
    while child >= 0:
        if not first:
            js_literal(sink, StringSlice(","))
        js_raw(sink, tree, child)
        first = False
        child = pj_next(tree, child)
    if not first:
        js_literal(sink, StringSlice(","))
    if not gemini_compact_write_instruction_item(sink):
        return False
    js_literal(sink, StringSlice("]"))
    return not sink[].failed


def gemini_compact_write_request(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson
) -> Bool:
    if pj_kind(tree, 0) != JSON_OBJECT:
        return False
    var input = pj_field(tree, 0, StringSlice("input"))
    if pj_kind(tree, input) != JSON_ARRAY:
        return False

    js_literal(sink, StringSlice("{"))
    var child = pj_child(tree, 0)
    var first = True
    while child >= 0:
        var node = tree.nodes[unsafe_offset=child].copy()
        if not gemini_compact_skip_request_key(node):
            if not first:
                js_literal(sink, StringSlice(","))
            js_string(sink, node.key)
            js_literal(sink, StringSlice(":"))
            if gemini_compact_key(node, StringSlice("input")):
                if not gemini_compact_write_input(sink, tree, child):
                    return False
            else:
                js_raw(sink, tree, child)
            first = False
        child = pj_next(tree, child)

    if not first:
        js_literal(sink, StringSlice(","))
    js_literal(sink, StringSlice('"instructions":'))
    js_string(
        sink,
        ProdexRichStringView(
            UInt(Int(GEMINI_COMPACT_INSTRUCTIONS.unsafe_ptr())),
            UInt(GEMINI_COMPACT_INSTRUCTIONS.byte_length()),
        ),
    )
    js_literal(sink, StringSlice(',"model":"chat-compression-default"'))
    js_literal(sink, StringSlice(',"parallel_tool_calls":false'))
    js_literal(sink, StringSlice(',"prodex_gemini_compaction":true'))
    js_literal(sink, StringSlice(',"stream":false,"store":false}'))
    return not sink[].failed


def gemini_compact_append_text(
    sink: Pointer[mut=True, JsonSink, _],
    view: ProdexRichStringView,
    found: Pointer[mut=True, Bool, _],
) -> Bool:
    var bounds = rich_trim_bounds(view)
    if bounds[0] >= bounds[1]:
        return True
    if found[]:
        js_literal(sink, StringSlice("\n"))
    js_view(
        sink,
        ProdexRichStringView(
            view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0])
        ),
    )
    found[] = True
    return not sink[].failed


def gemini_compact_first_array_item(tree: ParsedJson, array: Int64) -> Int64:
    if pj_kind(tree, array) != JSON_ARRAY:
        return -1
    return pj_child(tree, array)


def gemini_compact_write_message_content(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    content: Int64,
    found: Pointer[mut=True, Bool, _],
) -> Bool:
    if pj_kind(tree, content) == JSON_STRING:
        return gemini_compact_append_text(sink, pj_text(tree, content), found)
    if pj_kind(tree, content) != JSON_ARRAY:
        return True
    var child = pj_child(tree, content)
    while child >= 0:
        if pj_kind(tree, child) == JSON_OBJECT:
            var type = pj_string_field(tree, child, StringSlice("type"))
            if (
                type >= 0
                and (
                    pj_equal(pj_text(tree, type), pj_literal(StringSlice("output_text")))
                    or pj_equal(pj_text(tree, type), pj_literal(StringSlice("input_text")))
                )
            ):
                var text = pj_string_field(tree, child, StringSlice("text"))
                if text >= 0 and not gemini_compact_append_text(
                    sink, pj_text(tree, text), found
                ):
                    return False
            else:
                var text = pj_string_field(tree, child, StringSlice("text"))
                if text >= 0 and not gemini_compact_append_text(
                    sink, pj_text(tree, text), found
                ):
                    return False
        child = pj_next(tree, child)
    return not sink[].failed


def gemini_compact_write_openai_summary(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    root: Int64,
    found: Pointer[mut=True, Bool, _],
) -> Bool:
    var choices = pj_field(tree, root, StringSlice("choices"))
    var choice = gemini_compact_first_array_item(tree, choices)
    if choice < 0:
        return True
    var message = pj_field(tree, choice, StringSlice("message"))
    if pj_kind(tree, message) != JSON_OBJECT:
        return True
    return gemini_compact_write_message_content(
        sink, tree, pj_field(tree, message, StringSlice("content")), found
    )


def gemini_compact_write_responses_summary(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    root: Int64,
    found: Pointer[mut=True, Bool, _],
) -> Bool:
    var output = pj_field(tree, root, StringSlice("output"))
    if pj_kind(tree, output) != JSON_ARRAY:
        return True
    var item = pj_child(tree, output)
    while item >= 0:
        if pj_kind(tree, item) == JSON_OBJECT:
            var type = pj_string_field(tree, item, StringSlice("type"))
            if type >= 0 and pj_equal(
                pj_text(tree, type), pj_literal(StringSlice("message"))
            ):
                if not gemini_compact_write_message_content(
                    sink, tree, pj_field(tree, item, StringSlice("content")), found
                ):
                    return False
        item = pj_next(tree, item)
    return not sink[].failed


def gemini_compact_write_gemini_summary(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    root: Int64,
    found: Pointer[mut=True, Bool, _],
) -> Bool:
    var candidates = pj_field(tree, root, StringSlice("candidates"))
    var candidate = gemini_compact_first_array_item(tree, candidates)
    if candidate < 0:
        return True
    var content = pj_field(tree, candidate, StringSlice("content"))
    var parts = pj_field(tree, content, StringSlice("parts"))
    if pj_kind(tree, parts) != JSON_ARRAY:
        return True
    var part = pj_child(tree, parts)
    while part >= 0:
        if pj_kind(tree, part) == JSON_OBJECT:
            var thought = pj_field(tree, part, StringSlice("thought"))
            if pj_kind(tree, thought) != JSON_TRUE:
                var text = pj_string_field(tree, part, StringSlice("text"))
                if text >= 0 and not gemini_compact_append_text(
                    sink, pj_text(tree, text), found
                ):
                    return False
        part = pj_next(tree, part)
    return not sink[].failed


def gemini_compact_write_summary(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson
) -> Bool:
    if pj_kind(tree, 0) != JSON_OBJECT:
        return False
    var root: Int64 = 0
    var response = pj_field(tree, 0, StringSlice("response"))
    var trace = pj_string_field(tree, 0, StringSlice("traceId"))
    if pj_kind(tree, response) == JSON_OBJECT and trace >= 0:
        root = response

    var found = False
    if not gemini_compact_write_openai_summary(
        sink, tree, root, Pointer(to=found)
    ):
        return False
    if found:
        return True
    if not gemini_compact_write_responses_summary(
        sink, tree, root, Pointer(to=found)
    ):
        return False
    if found:
        return True
    if not gemini_compact_write_gemini_summary(
        sink, tree, root, Pointer(to=found)
    ):
        return False
    if found:
        return True
    var output_text = pj_string_field(tree, root, StringSlice("output_text"))
    if output_text >= 0:
        return gemini_compact_append_text(sink, pj_text(tree, output_text), Pointer(to=found))
    return True


@export("prodex_mojo_gemini_compact_response_body_v1")
def prodex_mojo_gemini_compact_response_body_v1(
    abi_version: Int64,
    summary_address: UInt,
    summary_length: Int64,
    measuring: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_COMPACT_REWRITE_ABI_VERSION:
        return GEMINI_COMPACT_REWRITE_STATUS_ABI
    if (
        summary_length < 0
        or summary_length > GEMINI_COMPACT_REWRITE_MAX_BYTES
        or (summary_length > 0 and summary_address == 0)
        or measuring < 0
        or measuring > 1
        or output_capacity < 0
        or written_address == 0
        or measuring == 1
        and (output_address != 0 or output_capacity != 0)
        or measuring == 0
        and output_address == 0
    ):
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    var summary = ProdexRichStringView(summary_address, UInt(summary_length))
    if not rich_view_valid(summary, GEMINI_COMPACT_REWRITE_MAX_BYTES):
        return GEMINI_COMPACT_REWRITE_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var sink = JsonSink(output, output_capacity, 0, measuring == 1, False)
    js_literal(
        Pointer(to=sink),
        StringSlice(
            '{"output":[{"type":"message","role":"user","content":[{"type":"input_text","text":"'
        ),
    )
    js_escaped(
        Pointer(to=sink),
        ProdexRichStringView(
            UInt(Int(GEMINI_COMPACT_SUMMARY_PREFIX.unsafe_ptr())),
            UInt(GEMINI_COMPACT_SUMMARY_PREFIX.byte_length()),
        ),
    )
    js_literal(Pointer(to=sink), StringSlice("\\n\\n"))
    var bounds = rich_trim_bounds(summary)
    if bounds[0] < bounds[1]:
        js_escaped(
            Pointer(to=sink),
            ProdexRichStringView(
                summary.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0])
            ),
        )
    js_literal(Pointer(to=sink), StringSlice('"}]}]}'))
    if sink.failed:
        if measuring == 1 or sink.written >= output_capacity:
            return GEMINI_COMPACT_REWRITE_STATUS_CAPACITY
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    written[] = sink.written
    return GEMINI_COMPACT_REWRITE_STATUS_OK


@export("prodex_mojo_gemini_compact_rewrite_v1")
def prodex_mojo_gemini_compact_rewrite_v1(
    abi_version: Int64,
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
    if abi_version != GEMINI_COMPACT_REWRITE_ABI_VERSION:
        return GEMINI_COMPACT_REWRITE_STATUS_ABI
    if (
        operation < GEMINI_COMPACT_REWRITE_REQUEST
        or operation > GEMINI_COMPACT_REWRITE_SUMMARY
        or flag != 0
        or nodes_address == 0
        or nodes_count <= 0
        or nodes_count > 0x7FFFFFFFFFFFFFFF // 80
        or raw_address == 0
        or raw_length <= 0
        or raw_length > GEMINI_COMPACT_REWRITE_MAX_BYTES
        or scratch_address == 0
        or scratch_count < nodes_count
        or measuring < 0
        or measuring > 1
        or output_capacity < 0
        or metadata_address == 0
    ):
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    if measuring == 1 and (output_address != 0 or output_capacity != 0):
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    if measuring == 0 and output_address == 0:
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    if not pj_valid(tree) or not rich_view_valid(tree.raw, GEMINI_COMPACT_REWRITE_MAX_BYTES):
        return GEMINI_COMPACT_REWRITE_STATUS_UTF8
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = 0
    metadata[unsafe_offset=1] = 0
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var sink = JsonSink(output, output_capacity, 0, measuring == 1, False)
    var ok = (
        gemini_compact_write_request(Pointer(to=sink), tree)
        if operation == GEMINI_COMPACT_REWRITE_REQUEST
        else gemini_compact_write_summary(Pointer(to=sink), tree)
    )
    if not ok:
        if sink.failed and (measuring == 1 or sink.written >= output_capacity):
            return GEMINI_COMPACT_REWRITE_STATUS_CAPACITY
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    if operation == GEMINI_COMPACT_REWRITE_SUMMARY and sink.written == 0:
        metadata[unsafe_offset=0] = 0
    else:
        metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return GEMINI_COMPACT_REWRITE_STATUS_OK


def gemini_compact_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value


def gemini_compact_contains(view: ProdexRichStringView, needle: StringSlice) -> Bool:
    if needle.byte_length() == 0 or view.len < UInt(needle.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = needle.unsafe_ptr()
    var limit = Int64(view.len) - Int64(needle.byte_length())
    for start in range(limit + 1):
        var matched = True
        for index in range(Int64(needle.byte_length())):
            if gemini_compact_ascii_lower(source[unsafe_offset=start + index]) != gemini_compact_ascii_lower(wanted[unsafe_offset=index]):
                matched = False
                break
        if matched:
            return True
    return False


@export("prodex_mojo_gemini_compact_error_reason_v1")
def prodex_mojo_gemini_compact_error_reason_v1(
    abi_version: Int64,
    text_address: UInt,
    text_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_COMPACT_REWRITE_ABI_VERSION:
        return GEMINI_COMPACT_REWRITE_STATUS_ABI
    if output_address == 0 or text_length < 0 or (text_length > 0 and text_address == 0):
        return GEMINI_COMPACT_REWRITE_STATUS_INVALID
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if not rich_view_valid(text, GEMINI_COMPACT_REWRITE_MAX_BYTES):
        return GEMINI_COMPACT_REWRITE_STATUS_UTF8
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = GEMINI_COMPACT_REASON_PROVIDER
    if gemini_compact_contains(text, StringSlice("timed out")) or gemini_compact_contains(text, StringSlice("timeout")):
        output[] = GEMINI_COMPACT_REASON_TIMEOUT
    elif (
        gemini_compact_contains(text, StringSlice("unavailable"))
        or gemini_compact_contains(text, StringSlice("connection refused"))
        or gemini_compact_contains(text, StringSlice("could not connect"))
        or gemini_compact_contains(text, StringSlice("failed to spawn"))
    ):
        output[] = GEMINI_COMPACT_REASON_UNAVAILABLE
    elif gemini_compact_contains(text, StringSlice("unsupported")):
        output[] = GEMINI_COMPACT_REASON_UNSUPPORTED
    elif (
        gemini_compact_contains(text, StringSlice("parse"))
        or gemini_compact_contains(text, StringSlice("missing"))
        or gemini_compact_contains(text, StringSlice("no summary"))
        or gemini_compact_contains(text, StringSlice("invalid"))
        or gemini_compact_contains(text, StringSlice("unexpectedly returned"))
    ):
        output[] = GEMINI_COMPACT_REASON_INVALID_RESPONSE
    return GEMINI_COMPACT_REWRITE_STATUS_OK
