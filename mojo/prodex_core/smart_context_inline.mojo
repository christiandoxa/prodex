# Inline request references: traversal, duplicate planning, marker rendering,
# strict parsing and source verification. SHA-256 acquisition stays in Rust.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr, rich_view_prefix, rich_view_valid
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_STRING,
    pj_valid, pj_kind, pj_text, pj_field, pj_child, pj_next, pj_equal,
)
from json_sink import JsonSink, js_view, js_string, js_byte, js_literal, js_u64
from smart_context import (
    SmartContextDuplicateTextCandidate, smart_context_duplicate_text_plan_kernel,
    smart_context_duplicate_text_bucket,
)
from smart_context_artifact_ref import canonical_reference_bounds, literal_at, byte_at


@fieldwise_init
struct InlineTree(Copyable):
    var nodes: UInt
    var count: Int64
    var raw: ProdexRichStringView
    var hashes: UInt
    var hash_count: Int64

    def tree(self) -> ParsedJson:
        return ParsedJson(
            Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(self.nodes)),
            self.count, self.raw.copy(),
        )


@fieldwise_init
struct InlineInput(Copyable):
    var source: InlineTree
    var candidate: InlineTree
    var operation: Int64
    var scratch: UInt
    var scratch_words: Int64


def inline_subtree_end(tree: ParsedJson, root: Int64) -> Int64:
    var node = tree.nodes[unsafe_offset=root].copy()
    var end = node.raw_start + node.raw_length
    var index = root + 1
    while index < tree.count and tree.nodes[unsafe_offset=index].raw_start < end:
        index += 1
    return index


def inline_tree_valid(tree: ParsedJson) -> Bool:
    if not pj_valid(tree):
        return False
    var root = tree.nodes[unsafe_offset=0].copy()
    if root.raw_start != 0 or root.raw_length != Int64(tree.raw.len):
        return False
    for index in range(Int64(1), tree.count):
        var node = tree.nodes[unsafe_offset=index].copy()
        var parent = tree.nodes[unsafe_offset=node.parent].copy()
        var end = node.raw_start + node.raw_length
        if (
            node.raw_start < parent.raw_start or end > parent.raw_start + parent.raw_length
            or node.raw_start < tree.nodes[unsafe_offset=index - 1].raw_start
            or node.next_sibling >= 0 and end > tree.nodes[unsafe_offset=node.next_sibling].raw_start
        ):
            return False
    return True


def inline_uint(address: UInt, start: Int64, end: Int64) -> Tuple[Bool, UInt64]:
    var cursor = start
    if cursor < end and byte_at(address, cursor) == 43:
        cursor += 1
    if cursor >= end:
        return (False, 0)
    var value: UInt64 = 0
    while cursor < end:
        var byte = byte_at(address, cursor)
        if byte < 48 or byte > 57:
            return (False, 0)
        var digit = UInt64(byte - 48)
        if value > (18446744073709551615 - digit) // 10:
            return (False, 0)
        value = value * 10 + digit
        cursor += 1
    return (True, value)


def inline_find[literal: StaticString](view: ProdexRichStringView, start: Int64) -> Int64:
    var length = Int64(view.len)
    for index in range(start, length):
        if literal_at[literal](view.ptr, index, length):
            return index
    return -1


def inline_reference_source(
    text: ProdexRichStringView,
    source: ParsedJson,
    roots: Pointer[mut=True, Int64, _],
    root_count: Int64,
    hashes: Pointer[mut=False, ProdexRichStringView, _],
) -> Int64:
    if not rich_view_prefix["[prodex-context-ref v=1 source=original-input["](text, False):
        return -1
    var index_start = Int64(StringSlice("[prodex-context-ref v=1 source=original-input[").byte_length())
    var index_end = inline_find["] digest="](text, index_start)
    if index_end < 0:
        return -1
    var index = inline_uint(text.ptr, index_start, index_end)
    var digest_start = index_end + 9
    var digest_end = inline_find[" bytes="](text, digest_start)
    var length = Int64(text.len)
    if not index[0] or index[1] >= UInt64(root_count) or digest_end < 0 or byte_at(text.ptr, length - 1) != 93:
        return -1
    var size = inline_uint(text.ptr, digest_end + 7, length - 1)
    var bounds = canonical_reference_bounds(text.ptr, digest_start, digest_end)
    if not size[0] or (bounds[0] != 1 and bounds[0] != 2) or bounds[2] != digest_end:
        return -1
    var digest = ProdexRichStringView(text.ptr + UInt(digest_start), UInt(digest_end - digest_start))
    var legacy: UInt64 = 0
    if bounds[0] == 1:
        if not rich_view_prefix["sc:"](digest, False):
            return -1
        for cursor in range(bounds[1], bounds[2]):
            var byte = byte_at(text.ptr, cursor)
            if byte >= 48 and byte <= 57:
                legacy = legacy * 16 + UInt64(byte - 48)
            elif byte >= 97 and byte <= 102:
                legacy = legacy * 16 + UInt64(byte - 87)
            else:
                return -1
    var root = roots[unsafe_offset=Int(index[1])]
    var end = inline_subtree_end(source, root)
    for node in range(root, end):
        if pj_kind(source, node) != JSON_STRING:
            continue
        var original = pj_text(source, node)
        if UInt64(original.len) != size[1]:
            continue
        if (
            bounds[0] == 2 and pj_equal(digest, hashes[unsafe_offset=node])
            or bounds[0] == 1 and legacy == smart_context_duplicate_text_bucket(original)
        ):
            return node
    return -1


def inline_write(
    sink: Pointer[mut=True, JsonSink, _],
    tree: ParsedJson,
    source: ParsedJson,
    replacements: Pointer[mut=True, Int64, _],
    hashes: Pointer[mut=False, ProdexRichStringView, _],
    expand: Bool,
):
    var cursor: Int64 = 0
    for index in range(tree.count):
        var replacement = replacements[unsafe_offset=index]
        if replacement < 0:
            continue
        var node = tree.nodes[unsafe_offset=index].copy()
        js_view(sink, ProdexRichStringView(tree.raw.ptr + UInt(cursor), UInt(node.raw_start - cursor)))
        if expand:
            js_string(sink, pj_text(source, replacement))
        else:
            var digest = hashes[unsafe_offset=index].copy()
            js_byte(sink, 34)
            js_literal(sink, StringSlice("[prodex-context-ref v=1 source=original-input["))
            js_u64(sink, UInt64(replacement))
            js_literal(sink, StringSlice("] digest="))
            js_view(sink, digest)
            js_literal(sink, StringSlice(" bytes="))
            js_u64(sink, UInt64(node.text.len))
            js_literal(sink, StringSlice("]\""))
        cursor = node.raw_start + node.raw_length
    js_view(sink, ProdexRichStringView(tree.raw.ptr + UInt(cursor), tree.raw.len - UInt(cursor)))


def inline_kernel(
    input: InlineInput,
    measuring: Bool,
    output_address: UInt,
    capacity: Int64,
    meta: Pointer[mut=True, Int64, _],
) raises -> Int64:
    if input.operation < 0 or input.operation > 2:
        return 1
    if input.source.nodes == 0 or input.source.count <= 0 or input.source.count > 1048576 or input.source.raw.len > 67108864:
        return 1
    var source = input.source.tree()
    if not inline_tree_valid(source):
        return 1
    if input.operation != 0 and (
        input.source.hash_count != source.count or input.source.hashes == 0
    ):
        return 1
    var hashes = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(input.source.hashes))
    if input.operation != 0:
        for index in range(source.count):
            if not rich_view_valid(hashes[unsafe_offset=index], 68):
                return 1
    var tree = source.copy()
    if input.operation == 2:
        if input.candidate.nodes == 0 or input.candidate.count <= 0 or input.candidate.count > 1048576 or input.candidate.raw.len > 67108864:
            return 1
        tree = input.candidate.tree()
        if not inline_tree_valid(tree):
            return 1
    if input.scratch == 0 or input.scratch_words < max(source.count, tree.count) * 12 + 1:
        return 1
    var replacements = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(input.scratch))
    for index in range(tree.count):
        replacements[unsafe_offset=index] = -1
    var count: Int64 = 0
    var exceeded: Int64 = 0
    var input_node = pj_field(source, 0, StringSlice("input"))
    if input.operation == 2:
        if pj_kind(source, input_node) != JSON_ARRAY:
            meta[unsafe_offset=0] = 0
            meta[unsafe_offset=1] = -1
            meta[unsafe_offset=2] = 0
            return 0
        var roots = replacements.unsafe_offset(Int(tree.count))
        var root_count: Int64 = 0
        var root = pj_child(source, input_node)
        while root >= 0:
            roots[unsafe_offset=root_count] = root
            root_count += 1
            root = pj_next(source, root)
        for index in range(tree.count):
            var text = pj_text(tree, index)
            if pj_kind(tree, index) != JSON_STRING or not rich_view_prefix["[prodex-context-ref "](text, False):
                continue
            var original = inline_reference_source(text, source, roots, root_count, hashes)
            if original < 0:
                meta[unsafe_offset=0] = 0
                meta[unsafe_offset=1] = -1
                meta[unsafe_offset=2] = 0
                return 0
            replacements[unsafe_offset=index] = original
            count += 1
    else:
        var candidates_address = input.scratch + UInt(source.count * 8)
        var candidates = Pointer[mut=True, SmartContextDuplicateTextCandidate, MutUntrackedOrigin](unsafe_from_address=Int(candidates_address))
        var node_indices = replacements.unsafe_offset(Int(source.count * 8))
        var candidate_count: Int64 = 0
        var root = pj_child(source, input_node) if pj_kind(source, input_node) == JSON_ARRAY else -1
        var input_index: Int64 = 0
        while root >= 0:
            var role = pj_text(source, pj_field(source, root, StringSlice("role")))
            var end = inline_subtree_end(source, root)
            for node in range(root, end):
                if pj_kind(source, node) == JSON_STRING:
                    if candidate_count >= source.count:
                        return 1
                    candidates[unsafe_offset=candidate_count] = SmartContextDuplicateTextCandidate(
                        hashes[unsafe_offset=node].copy() if input.operation == 1 else ProdexRichStringView(0, 0),
                        pj_text(source, node), role.copy(), input_index,
                    )
                    node_indices[unsafe_offset=candidate_count] = node
                    candidate_count += 1
            root = pj_next(source, root)
            input_index += 1
        var planned = replacements.unsafe_offset(Int(source.count * 9))
        var buckets = replacements.unsafe_offset(Int(source.count * 10))
        var status = smart_context_duplicate_text_plan_kernel(
            candidates_address, UInt(Int(planned)),
            UInt(Int(buckets)), source.count * 2 + 1,
            UInt(Int(Pointer(to=count))), UInt(Int(Pointer(to=exceeded))),
            candidate_count, Int64(0 if input.operation == 1 else 1),
        )
        if status != 0:
            return status
        for index in range(candidate_count):
            replacements[unsafe_offset=node_indices[unsafe_offset=index]] = planned[unsafe_offset=index]
    var cursor: Int64 = 0
    for index in range(tree.count):
        if replacements[unsafe_offset=index] >= 0:
            var node = tree.nodes[unsafe_offset=index].copy()
            if node.raw_start < cursor:
                return 1
            cursor = node.raw_start + node.raw_length
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, True, False,
    )
    var render = input.operation == 2 or input.operation == 1 and count > 0
    if render:
        inline_write(Pointer(to=sink), tree, source, replacements, hashes, input.operation == 2)
    if sink.failed:
        return 3
    if not measuring:
        if sink.written > capacity:
            return 3
        sink.measuring = False
        sink.written = 0
        if render:
            inline_write(Pointer(to=sink), tree, source, replacements, hashes, input.operation == 2)
        if sink.failed:
            return 3
    meta[unsafe_offset=0] = sink.written
    meta[unsafe_offset=1] = count
    meta[unsafe_offset=2] = exceeded
    return 0


@export("prodex_smart_context_inline_v1")
def prodex_smart_context_inline_v1(
    abi: Int64,
    input_address: UInt,
    measuring: Int64,
    output_address: UInt,
    capacity: Int64,
    metadata_address: UInt,
    metadata_count: Int64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        input_address == 0 or metadata_address == 0 or metadata_count < 3
        or measuring < 0 or measuring > 1 or capacity < 0
        or measuring == 0 and capacity > 0 and output_address == 0
    ):
        return 1
    var input = Pointer[mut=False, InlineInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))[].copy()
    try:
        return inline_kernel(
            input, measuring == 1, output_address, capacity,
            Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address)),
        )
    except:
        return 1
