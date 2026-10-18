# Request-wide artifact aliases and references over host-acquired JSON.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_codepoint, rich_codepoint_width, rich_unicode_space, rich_view_order_sort
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_OBJECT, JSON_STRING, pj_valid, pj_kind,
    pj_field, pj_child, pj_next, pj_text, pj_literal, pj_equal, pj_less,
)
from smart_context_static_request import ssr_field, ssr_static_item
from smart_context_artifact_ref import (
    byte_at, literal_at, trim_bounds, alias_end, alias_declaration_bounds,
    canonical_reference_bounds, write_canonical_id, range_payload_start, range_item,
    MAX_TOKEN_BYTES, body_marker_present,
)
from json_sink import JsonSink, js_byte, js_literal, js_string, js_u64


def ap_slice(view: ProdexRichStringView, start: Int64, end: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(view.ptr + UInt(start), UInt(end - start))


def ap_next(view: ProdexRichStringView, initial: Int64) -> Tuple[Int64, Int64, Int64]:
    var cursor = initial
    var start = initial
    while cursor < Int64(view.len):
        var byte = byte_at(view.ptr, cursor)
        var width = rich_codepoint_width(byte)
        var delimiter = byte == 41 or byte == 93 or byte == 125 or rich_unicode_space(
            rich_codepoint(Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(view.ptr)), cursor, width),
        )
        if delimiter:
            if cursor > start:
                return (start, cursor, cursor + width)
            start = cursor + width
        cursor += width
    return (start, cursor, cursor)


def ap_alias_text(view: ProdexRichStringView) -> Bool:
    var at = False
    var equals = False
    for index in range(Int64(view.len)):
        var byte = byte_at(view.ptr, index)
        at |= byte == 64
        equals |= byte == 61
    return at and equals


def ap_reference_text(view: ProdexRichStringView) -> Bool:
    if body_marker_present(view.ptr, Int64(view.len)):
        return True
    for index in range(Int64(view.len)):
        if byte_at(view.ptr, index) == 64 or literal_at["prodex smart context artifact"](view.ptr, index, Int64(view.len)) or literal_at["prodex-sc "](view.ptr, index, Int64(view.len)):
            return True
    return False


@fieldwise_init
struct ArtifactAliases(Copyable):
    var keys: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin]
    var ids: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin]
    var order: Pointer[mut=True, Int64, MutUntrackedOrigin]
    var id_bytes: UInt
    var capacity: Int64
    var count: Int64

    def find(self, key: ProdexRichStringView) -> ProdexRichStringView:
        var first: Int64 = 0
        var last = self.count
        while first < last:
            var middle = first + (last - first) // 2
            if pj_less(self.keys[unsafe_offset=self.order[unsafe_offset=middle]], key):
                first = middle + 1
            else:
                last = middle
        if first < self.count:
            var index = self.order[unsafe_offset=first]
            if pj_equal(self.keys[unsafe_offset=index], key):
                return self.ids[unsafe_offset=index].copy()
        return ProdexRichStringView(0, 0)


def ap_aliases(tree: ParsedJson, operation: Int64, aliases: Pointer[mut=True, ArtifactAliases, _], measuring: Bool) -> Int64:
    var count: Int64 = 0
    if operation == 2:
        var node = pj_child(tree, pj_field(tree, 0, StringSlice("aliases")))
        while node >= 0:
            if pj_kind(tree, node) != JSON_STRING:
                return -1
            if not measuring:
                if count >= aliases[].capacity:
                    return -1
                aliases[].keys[unsafe_offset=count] = tree.nodes[unsafe_offset=node].key.copy()
                aliases[].ids[unsafe_offset=count] = pj_text(tree, node)
            count += 1
            node = pj_next(tree, node)
        return count
    for node in range(tree.count):
        if pj_kind(tree, node) != JSON_STRING:
            continue
        var text = pj_text(tree, node)
        if not ap_alias_text(text):
            continue
        var cursor: Int64 = 0
        while cursor < Int64(text.len):
            var span = ap_next(text, cursor)
            cursor = span[2]
            var token = ap_slice(text, span[0], span[1])
            if token.len > UInt(MAX_TOKEN_BYTES):
                return -1
            var declaration = alias_declaration_bounds(token.ptr, Int64(token.len))
            if declaration[0] < 0:
                continue
            if not measuring:
                if count >= aliases[].capacity:
                    return -1
                var address = aliases[].id_bytes + UInt(count * 72)
                var written: Int64 = 0
                if not write_canonical_id(
                    token.ptr, declaration[2], declaration[3], declaration[4],
                    Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(address)), 72, Pointer(to=written),
                ):
                    return -1
                aliases[].keys[unsafe_offset=count] = ap_slice(token, declaration[0], declaration[1])
                aliases[].ids[unsafe_offset=count] = ProdexRichStringView(address, UInt(written))
            count += 1
    return count


def ap_tree(nodes: UInt64, count: Int64, raw: UInt64, length: Int64) -> ParsedJson:
    return ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes)),
        count, ProdexRichStringView(UInt(raw), UInt(length)),
    )


def ap_input_valid(operation: Int64, nodes: UInt64, count: Int64, raw: UInt64, length: Int64) -> Bool:
    return operation >= 0 and operation <= 2 and nodes != 0 and count > 0 and count <= 1048576 and raw != 0 and length >= 0 and length <= 67108864


def ap_tree_valid(tree: ParsedJson, operation: Int64) -> Bool:
    var acquired_bytes: UInt = 0
    for index in range(tree.count):
        var node = tree.nodes[unsafe_offset=index].copy()
        for view in [node.key.copy(), node.text.copy()]:
            if view.len > 67108864 - acquired_bytes:
                return False
            acquired_bytes += view.len
    return pj_valid(tree) and (operation != 2 or (
        pj_kind(tree, pj_field(tree, 0, StringSlice("aliases"))) == JSON_OBJECT
        and pj_kind(tree, pj_field(tree, 0, StringSlice("text"))) == JSON_STRING
    ))


@export("prodex_smart_context_artifact_alias_count_v1")
def prodex_smart_context_artifact_alias_count_v1(
    abi: Int64, operation: Int64, nodes: UInt64, count: Int64, raw: UInt64, length: Int64, result: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if not ap_input_valid(operation, nodes, count, raw, length) or result == 0:
        return 1
    var tree = ap_tree(nodes, count, raw, length)
    if not ap_tree_valid(tree, operation):
        return 1
    # Capacity discovery never dereferences alias storage.
    var placeholder = Array[UInt64, 4](fill=0)
    var placeholder_address = Int(placeholder.unsafe_ptr())
    var empty = ArtifactAliases(
        Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=placeholder_address),
        Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=placeholder_address),
        Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=placeholder_address), 0, 0, 0,
    )
    var required = ap_aliases(tree, operation, Pointer(to=empty), True)
    if required < 0:
        return 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(result))[] = required
    return 0


def ap_ranges(sink: Pointer[mut=True, JsonSink, _], token: ProdexRichStringView, start: Int64, end: Int64):
    var cursor = range_payload_start(token.ptr, start, end)
    var first_start: Int64 = 0
    var first_end: Int64 = 0
    js_literal(sink, StringSlice(',"line_ranges":['))
    var count: Int64 = 0
    while cursor < end:
        var item = range_item(token.ptr, cursor, end)
        cursor = item[2]
        if item[0] <= 0 or item[1] < item[0]:
            continue
        if count == 0:
            first_start = item[0]
            first_end = item[1]
        else:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"start":'))
        js_u64(sink, UInt64(item[0]))
        js_literal(sink, StringSlice(',"end":'))
        js_u64(sink, UInt64(item[1]))
        js_byte(sink, 125)
        count += 1
    js_literal(sink, StringSlice('],"line_range":'))
    if count == 0:
        js_literal(sink, StringSlice("null"))
    else:
        js_literal(sink, StringSlice('{"start":'))
        js_u64(sink, UInt64(first_start))
        js_literal(sink, StringSlice(',"end":'))
        js_u64(sink, UInt64(first_end))
        js_byte(sink, 125)


def ap_references(sink: Pointer[mut=True, JsonSink, _], text: ProdexRichStringView, aliases: ArtifactAliases, initial: Int64) -> Int64:
    var cursor: Int64 = 0
    var count = initial
    while cursor < Int64(text.len):
        var span = ap_next(text, cursor)
        cursor = span[2]
        var token = ap_slice(text, span[0], span[1])
        if token.len > UInt(MAX_TOKEN_BYTES):
            return -1
        if alias_declaration_bounds(token.ptr, Int64(token.len))[0] >= 0:
            continue
        var bounds = trim_bounds(token.ptr, Int64(token.len))
        var marker = ap_slice(token, bounds[0], bounds[1])
        var alias = alias_end(token.ptr, bounds[0], bounds[1])
        var id = ProdexRichStringView(0, 0)
        var range_start = alias
        var id_buffer = Array[UInt8, 72](fill=0)
        if alias >= 0:
            id = aliases.find(ap_slice(token, bounds[0], alias))
        if id.len == 0:
            var reference = canonical_reference_bounds(token.ptr, bounds[0], bounds[1])
            if reference[0] < 0:
                continue
            var written: Int64 = 0
            if not write_canonical_id(token.ptr, reference[0], reference[1], reference[2], id_buffer.unsafe_ptr(), 72, Pointer(to=written)):
                return -1
            id = ProdexRichStringView(UInt(Int(id_buffer.unsafe_ptr())), UInt(written))
            range_start = reference[2]
        if count > 0:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('{"id":'))
        js_string(sink, id)
        js_literal(sink, StringSlice(',"marker":'))
        js_string(sink, marker)
        ap_ranges(sink, token, range_start, bounds[1])
        js_byte(sink, 125)
        count += 1
    return count


def ap_render(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64, aliases: ArtifactAliases, blocked: Pointer[mut=True, Int64, _]) -> Bool:
    js_literal(sink, StringSlice('{"aliases":{'))
    for position in range(aliases.count):
        if position > 0:
            js_byte(sink, 44)
        var index = aliases.order[unsafe_offset=position]
        js_string(sink, aliases.keys[unsafe_offset=index])
        js_byte(sink, 58)
        js_string(sink, aliases.ids[unsafe_offset=index])
    js_literal(sink, StringSlice('},"references":['))
    var count: Int64 = 0
    if operation == 2:
        count = ap_references(sink, pj_text(tree, pj_field(tree, 0, StringSlice("text"))), aliases, count)
    elif operation == 0:
        for index in range(tree.count):
            var node = tree.nodes[unsafe_offset=index].copy()
            blocked[unsafe_offset=index] = Int64(
                node.parent >= 0 and blocked[unsafe_offset=node.parent] != 0
                or ssr_static_item(tree, index) or ssr_field(node.key),
            )
            if node.kind == JSON_STRING and blocked[unsafe_offset=index] == 0 and ap_reference_text(node.text):
                count = ap_references(sink, node.text, aliases, count)
                if count < 0:
                    return False
    js_literal(sink, StringSlice("]}"))
    return count >= 0


@export("prodex_smart_context_artifact_projection_v1")
def prodex_smart_context_artifact_projection_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes: UInt64, count: Int64, raw: UInt64, length: Int64,
    scratch: UInt64, scratch_count: Int64, measuring: Int64,
    output: UInt64, capacity: Int64, metadata: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if not ap_input_valid(operation, nodes, count, raw, length) or flag != 0 or scratch == 0 or metadata == 0 or measuring < 0 or measuring > 1 or capacity < 0 or measuring == 0 and output == 0:
        return 1
    var tree = ap_tree(nodes, count, raw, length)
    if not ap_tree_valid(tree, operation):
        return 1
    if scratch_count < count + 1 or scratch_count > count + 1 + 67108864 * 7:
        return 1
    var alias_capacity = (scratch_count - count - 1) // 7
    var base = scratch + UInt64(count * 16)
    var aliases = ArtifactAliases(
        Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(base)),
        Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(base + UInt64(alias_capacity * 16))),
        Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(base + UInt64(alias_capacity * 32))),
        UInt(base + UInt64(alias_capacity * 40)), alias_capacity, 0,
    )
    var found = ap_aliases(tree, operation, Pointer(to=aliases), False)
    if found < 0:
        return 3
    for index in range(found):
        aliases.order[unsafe_offset=index] = index
    rich_view_order_sort(
        Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(aliases.keys)), aliases.order, found,
    )
    # The first declaration in traversal order owns a repeated alias.
    for position in range(found):
        var index = aliases.order[unsafe_offset=position]
        if aliases.count > 0 and pj_equal(aliases.keys[unsafe_offset=aliases.order[unsafe_offset=aliases.count - 1]], aliases.keys[unsafe_offset=index]):
            aliases.order[unsafe_offset=aliases.count - 1] = min(index, aliases.order[unsafe_offset=aliases.count - 1])
        else:
            aliases.order[unsafe_offset=aliases.count] = index
            aliases.count += 1
    var blocked = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch))
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output)), capacity, 0, True, False)
    if not ap_render(Pointer(to=sink), tree, operation, aliases, blocked):
        return 1
    if sink.failed or measuring == 0 and sink.written > capacity:
        return 3
    if measuring == 0:
        sink.written = 0
        sink.measuring = False
        if not ap_render(Pointer(to=sink), tree, operation, aliases, blocked):
            return 1
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata))
    meta[unsafe_offset=0] = 1
    meta[unsafe_offset=1] = sink.written
    return 0
