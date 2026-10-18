# Static prompt acquisition, exact text-part precedence, stable IDs and legacy
# cache-key consensus over the shared host-acquired JSON tree.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import (
    rich_view_ptr, rich_view_prefix, rich_codepoint_width, rich_codepoint,
    rich_unicode_space, rich_view_order_sort, rich_view_valid,
)
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING,
    pj_valid, pj_kind, pj_text, pj_field, pj_child, pj_next,
    pj_literal, pj_trim, pj_equal,
)
from json_sink import JsonSink, js_byte, js_string, js_literal, js_escaped, js_u64


def ssr_field(view: ProdexRichStringView) -> Bool:
    return (
        pj_equal(view, pj_literal(StringSlice("instructions")))
        or pj_equal(view, pj_literal(StringSlice("system")))
        or pj_equal(view, pj_literal(StringSlice("developer")))
    )


def ssr_static_item(tree: ParsedJson, node: Int64) -> Bool:
    return ssr_role(pj_text(tree, pj_field(tree, node, StringSlice("role"))))


def ssr_role(view: ProdexRichStringView) -> Bool:
    return pj_equal(view, pj_literal(StringSlice("system"))) or pj_equal(view, pj_literal(StringSlice("developer")))


@export("prodex_smart_context_static_boundary_v1")
def prodex_smart_context_static_boundary_v1(
    abi: Int64, operation: Int64, address: UInt64, length: Int64, result_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 1 or length < 0 or result_address == 0:
        return 1
    var view = ProdexRichStringView(UInt(address), UInt(length))
    if not rich_view_valid(view, 67108864):
        return 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(result_address))[] = Int64(
        ssr_field(view) if operation == 0 else ssr_role(view),
    )
    return 0


def ssr_collect_parts(
    tree: ParsedJson, root: Int64,
    parts: Pointer[mut=True, Int64, _],
    stack: Pointer[mut=True, Int64, _],
    previous: Pointer[mut=True, Int64, _],
    count: Int64,
) -> Int64:
    if root < 0:
        return count
    stack[unsafe_offset=0] = root
    var pending: Int64 = 1
    var written = count
    while pending > 0:
        pending -= 1
        var node = stack[unsafe_offset=pending]
        var kind = pj_kind(tree, node)
        if kind == JSON_STRING:
            if pj_trim(pj_text(tree, node)).len > 0:
                if written >= tree.count:
                    return -1
                parts[unsafe_offset=written] = node
                written += 1
        elif kind == JSON_ARRAY:
            var child = pj_child(tree, node)
            while child >= 0 and pj_next(tree, child) >= 0:
                child = pj_next(tree, child)
            while child >= 0:
                if pending >= tree.count:
                    return -1
                stack[unsafe_offset=pending] = child
                pending += 1
                child = previous[unsafe_offset=child]
        elif kind == JSON_OBJECT:
            for key in [StringSlice("content"), StringSlice("input_text"), StringSlice("text")]:
                var child = pj_field(tree, node, key)
                if child >= 0:
                    if pending >= tree.count:
                        return -1
                    stack[unsafe_offset=pending] = child
                    pending += 1
    return written


def ssr_legacy_hash(text: ProdexRichStringView) -> ProdexRichStringView:
    var hash = pj_trim(text)
    if rich_view_prefix["psc static "](hash, False):
        hash = ProdexRichStringView(hash.ptr + 11, hash.len - 11)
    elif rich_view_prefix["prodex static context unchanged "](hash, False):
        var prefix = UInt(StringSlice("prodex static context unchanged ").byte_length())
        hash = ProdexRichStringView(hash.ptr + prefix, hash.len - prefix)
    else:
        return ProdexRichStringView(0, 0)
    if not rich_view_prefix["scpc2:"](hash, False) and not rich_view_prefix["scpc:"](hash, False):
        return ProdexRichStringView(0, 0)
    var source = rich_view_ptr(hash)
    var cursor: Int64 = 0
    while cursor < Int64(hash.len):
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if rich_unicode_space(rich_codepoint(source, cursor, width)):
            return ProdexRichStringView(0, 0)
        cursor += width
    return hash^


def ssr_render(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson,
    records: Pointer[mut=True, Int64, _],
    ids: Pointer[mut=True, ProdexRichStringView, _],
    order: Pointer[mut=True, Int64, _],
    parts: Pointer[mut=True, Int64, _], count: Int64,
    legacy_hash: ProdexRichStringView,
):
    js_literal(sink, StringSlice('{"items":['))
    for position in range(count):
        var index = order[unsafe_offset=position]
        if position > 0:
            js_byte(sink, 44)
        js_byte(sink, 91)
        js_string(sink, ids[unsafe_offset=index])
        js_literal(sink, StringSlice(',"'))
        var first = records[unsafe_offset=index * 2]
        var last = records[unsafe_offset=index * 2 + 1]
        for part in range(first, last):
            if part > first:
                js_literal(sink, StringSlice("\\n"))
            js_escaped(sink, pj_text(tree, parts[unsafe_offset=part]))
        js_literal(sink, StringSlice('"]'))
    js_literal(sink, StringSlice('],"legacy_hash":'))
    if legacy_hash.len > 0:
        js_string(sink, legacy_hash)
    else:
        js_literal(sink, StringSlice("null"))
    js_byte(sink, 125)


@export("prodex_smart_context_static_request_v1")
def prodex_smart_context_static_request_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation != 0 or flag != 0
        or count <= 0 or count > 1048576 or nodes_address == 0
        or raw_length < 0 or raw_length > 67108864 or raw_address == 0
        or scratch_address == 0 or scratch_count < count * 7
        or metadata_address == 0 or capacity < 0 or measuring < 0 or measuring > 1
        or measuring == 0 and output_address == 0
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, True, False,
    )
    # Per node: 16-byte spans, 64-byte IDs, four 8-byte index buffers.
    var records = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var ids = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address + UInt64(count * 16)))
    var id_bytes = scratch_address + UInt64(count * 32)
    var order = records.unsafe_offset(Int(count * 10))
    var parts = records.unsafe_offset(Int(count * 11))
    var stack = records.unsafe_offset(Int(count * 12))
    var previous = records.unsafe_offset(Int(count * 13))
    var item_count: Int64 = 0
    var part_count: Int64 = 0
    var legacy_hash = ProdexRichStringView(0, 0)
    var legacy_valid = True
    for index in range(count):
        previous[unsafe_offset=index] = -1
    for index in range(count):
        var child = pj_child(tree, index)
        var prior: Int64 = -1
        while child >= 0:
            previous[unsafe_offset=child] = prior
            prior = child
            child = pj_next(tree, child)
    for key in [StringSlice("instructions"), StringSlice("system"), StringSlice("developer")]:
        var node = pj_field(tree, 0, key)
        if pj_kind(tree, node) == JSON_STRING and pj_trim(pj_text(tree, node)).len > 0:
            parts[unsafe_offset=part_count] = node
            ids[unsafe_offset=item_count] = pj_literal(key)
            records[unsafe_offset=item_count * 2] = part_count
            part_count += 1
            records[unsafe_offset=item_count * 2 + 1] = part_count
            item_count += 1
    var input = pj_field(tree, 0, StringSlice("input"))
    var message = pj_child(tree, input) if pj_kind(tree, input) == JSON_ARRAY else -1
    var input_index: UInt64 = 0
    while message >= 0:
        if ssr_static_item(tree, message):
            var content = pj_field(tree, message, StringSlice("content"))
            var input_text = pj_field(tree, message, StringSlice("input_text"))
            var first = part_count
            if pj_kind(tree, content) == JSON_STRING:
                if pj_trim(pj_text(tree, content)).len > 0:
                    parts[unsafe_offset=part_count] = content
                    part_count += 1
            elif pj_kind(tree, input_text) == JSON_STRING:
                if pj_trim(pj_text(tree, input_text)).len > 0:
                    parts[unsafe_offset=part_count] = input_text
                    part_count += 1
            else:
                part_count = ssr_collect_parts(tree, content, parts, stack, previous, part_count)
                if part_count < 0:
                    return 1
            if part_count > first:
                if item_count >= count:
                    return 1
                var id_address = id_bytes + UInt64(item_count * 48)
                var id_sink = JsonSink(
                    Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(id_address)),
                    48, 0, False, False,
                )
                js_literal(Pointer(to=id_sink), StringSlice("input["))
                js_u64(Pointer(to=id_sink), input_index)
                js_literal(Pointer(to=id_sink), StringSlice("]."))
                var role = pj_text(tree, pj_field(tree, message, StringSlice("role")))
                js_escaped(Pointer(to=id_sink), role)
                if id_sink.failed:
                    return 3
                ids[unsafe_offset=item_count] = ProdexRichStringView(UInt(id_address), UInt(id_sink.written))
                records[unsafe_offset=item_count * 2] = first
                records[unsafe_offset=item_count * 2 + 1] = part_count
                item_count += 1
        message = pj_next(tree, message)
        input_index += 1
    for index in range(item_count):
        order[unsafe_offset=index] = index
        var first = records[unsafe_offset=index * 2]
        var last = records[unsafe_offset=index * 2 + 1]
        var hash = ssr_legacy_hash(pj_text(tree, parts[unsafe_offset=first]))
        if last - first != 1 or hash.len == 0 or index > 0 and not pj_equal(hash, legacy_hash):
            legacy_valid = False
        if index == 0:
            legacy_hash = hash.copy()
    if not legacy_valid:
        legacy_hash = ProdexRichStringView(0, 0)
    rich_view_order_sort(
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(ids)),
        order, item_count,
    )
    ssr_render(Pointer(to=sink), tree, records, ids, order, parts, item_count, legacy_hash)
    if sink.failed or measuring == 0 and sink.written > capacity:
        return 3
    if measuring == 0:
        sink.written = 0
        sink.measuring = False
        ssr_render(Pointer(to=sink), tree, records, ids, order, parts, item_count, legacy_hash)
        if sink.failed:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
