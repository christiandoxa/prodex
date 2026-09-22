# Read-only, caller-owned JSON trees. Serde retains JSON parsing/serialization
# compatibility; semantic kernels traverse decoded strings and stable indices.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import (
    rich_view_valid, rich_view_ptr, rich_codepoint, rich_codepoint_width,
    rich_view_matches_literal, rich_view_prefix,
)
from launch_args_common import launch_rust_space

comptime JSON_NULL: Int64 = 0
comptime JSON_FALSE: Int64 = 1
comptime JSON_TRUE: Int64 = 2
comptime JSON_NUMBER: Int64 = 3
comptime JSON_STRING: Int64 = 4
comptime JSON_ARRAY: Int64 = 5
comptime JSON_OBJECT: Int64 = 6

@fieldwise_init
struct ParsedJsonNode(Copyable):
    var kind: Int64
    var first_child: Int64
    var next_sibling: Int64
    var parent: Int64
    var key: ProdexRichStringView
    var text: ProdexRichStringView
    var raw_start: Int64
    var raw_length: Int64

@fieldwise_init
struct ParsedJson(Copyable):
    var nodes: Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin]
    var count: Int64
    var raw: ProdexRichStringView


def pj_kind(tree: ParsedJson, index: Int64) -> Int64:
    return tree.nodes[unsafe_offset=index].kind if index >= 0 and index < tree.count else -1


def pj_text(tree: ParsedJson, index: Int64) -> ProdexRichStringView:
    if pj_kind(tree, index) == JSON_STRING:
        return tree.nodes[unsafe_offset=index].text.copy()
    return ProdexRichStringView(0, 0)


def pj_child(tree: ParsedJson, index: Int64) -> Int64:
    if index < 0 or index >= tree.count:
        return -1
    return tree.nodes[unsafe_offset=index].first_child


def pj_next(tree: ParsedJson, index: Int64) -> Int64:
    if index < 0 or index >= tree.count:
        return -1
    return tree.nodes[unsafe_offset=index].next_sibling


def pj_equal(left: ProdexRichStringView, right: ProdexRichStringView) -> Bool:
    if left.len != right.len:
        return False
    var a = rich_view_ptr(left)
    var b = rich_view_ptr(right)
    for i in range(Int64(left.len)):
        if a[unsafe_offset=i] != b[unsafe_offset=i]:
            return False
    return True


def pj_less(left: ProdexRichStringView, right: ProdexRichStringView) -> Bool:
    var a = rich_view_ptr(left)
    var b = rich_view_ptr(right)
    for i in range(min(Int64(left.len), Int64(right.len))):
        if a[unsafe_offset=i] != b[unsafe_offset=i]:
            return a[unsafe_offset=i] < b[unsafe_offset=i]
    return left.len < right.len


def pj_literal(value: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def pj_field(tree: ParsedJson, index: Int64, name: StringSlice) -> Int64:
    if pj_kind(tree, index) != JSON_OBJECT:
        return -1
    var child = pj_child(tree, index)
    var key = pj_literal(name)
    while child >= 0:
        if pj_equal(tree.nodes[unsafe_offset=child].key, key):
            return child
        child = pj_next(tree, child)
    return -1


def pj_string_field(tree: ParsedJson, index: Int64, name: StringSlice) -> Int64:
    var child = pj_field(tree, index, name)
    return child if pj_kind(tree, child) == JSON_STRING else -1


def pj_is[value: StaticString](tree: ParsedJson, index: Int64) -> Bool:
    return pj_kind(tree, index) == JSON_STRING and rich_view_matches_literal[value](pj_text(tree, index), False)


def pj_trim(view: ProdexRichStringView) -> ProdexRichStringView:
    var ptr = rich_view_ptr(view)
    var first: Int64 = 0
    var last: Int64 = 0
    var cursor: Int64 = 0
    var leading = True
    while cursor < Int64(view.len):
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var space = launch_rust_space(rich_codepoint(ptr, cursor, width))
        cursor += width
        if leading and space:
            first = cursor
        else:
            leading = False
        if not space:
            last = cursor
    if last <= first:
        return ProdexRichStringView(0, 0)
    return ProdexRichStringView(view.ptr + UInt(first), UInt(last - first))


def pj_nonblank(tree: ParsedJson, index: Int64) -> Bool:
    return pj_kind(tree, index) == JSON_STRING and pj_trim(pj_text(tree, index)).len > 0


def pj_valid(tree: ParsedJson) -> Bool:
    if tree.count <= 0 or tree.count > 0x7FFFFFFFFFFFFFFF // 80:
        return False
    if not rich_view_valid(tree.raw, 0x7FFFFFFFFFFFFFFF):
        return False
    for index in range(tree.count):
        var node = tree.nodes[unsafe_offset=index].copy()
        if node.kind < JSON_NULL or node.kind > JSON_OBJECT:
            return False
        if index == 0:
            if node.parent != -1:
                return False
        elif node.parent < 0 or node.parent >= index or tree.nodes[unsafe_offset=node.parent].kind < JSON_ARRAY:
            return False
        if node.first_child != -1 and (node.first_child <= index or node.first_child >= tree.count or node.kind < JSON_ARRAY):
            return False
        if node.next_sibling != -1 and (node.next_sibling <= index or node.next_sibling >= tree.count or index == 0):
            return False
        if node.first_child >= 0 and tree.nodes[unsafe_offset=node.first_child].parent != index:
            return False
        if node.next_sibling >= 0 and tree.nodes[unsafe_offset=node.next_sibling].parent != node.parent:
            return False
        if not rich_view_valid(node.key, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(node.text, 0x7FFFFFFFFFFFFFFF):
            return False
        if node.raw_start < 0 or node.raw_length < 0 or node.raw_start > Int64(tree.raw.len) or node.raw_length > Int64(tree.raw.len) - node.raw_start:
            return False
    return True
