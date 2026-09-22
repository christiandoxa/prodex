# Stable first-output lookup and globally emitted-call ownership for one batch.
# Scratch is exactly two Int64 words per validated JSON node.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, pj_child, pj_next, pj_field, pj_text, pj_nonblank, pj_is,
    pj_less, pj_equal,
)


def ds_output_id(tree: ParsedJson, node: Int64) -> Int64:
    return pj_field(tree, node, StringSlice("tool_call_id"))


def ds_output_less(tree: ParsedJson, left: Int64, right: Int64) -> Bool:
    var a = pj_text(tree, ds_output_id(tree, left))
    var b = pj_text(tree, ds_output_id(tree, right))
    return pj_less(a, b) or pj_equal(a, b) and left < right


def ds_output_swap(values: Pointer[mut=True, Int64, _], left: Int64, right: Int64):
    var value = values[unsafe_offset=left]
    values[unsafe_offset=left] = values[unsafe_offset=right]
    values[unsafe_offset=right] = value


def ds_output_sift(tree: ParsedJson, values: Pointer[mut=True, Int64, _], initial: Int64, count: Int64):
    var root = initial
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        if child + 1 < count and ds_output_less(tree, values[unsafe_offset=child], values[unsafe_offset=child + 1]):
            child += 1
        if not ds_output_less(tree, values[unsafe_offset=root], values[unsafe_offset=child]):
            break
        ds_output_swap(values, root, child)
        root = child


def ds_outputs(tree: ParsedJson, scratch: Pointer[mut=True, Int64, _]) -> Int64:
    var count: Int64 = 0
    var message = pj_child(tree, 0)
    while message >= 0:
        if pj_is["tool"](tree, pj_field(tree, message, StringSlice("role"))) and pj_nonblank(tree, ds_output_id(tree, message)):
            scratch[unsafe_offset=count] = message
            count += 1
        message = pj_next(tree, message)
    var root = count // 2
    while root > 0:
        root -= 1
        ds_output_sift(tree, scratch, root, count)
    var end = count
    while end > 1:
        end -= 1
        ds_output_swap(scratch, 0, end)
        ds_output_sift(tree, scratch, 0, end)
    var unique: Int64 = 0
    for item in range(count):
        var node = scratch[unsafe_offset=item]
        if unique == 0 or not pj_equal(pj_text(tree, ds_output_id(tree, node)), pj_text(tree, ds_output_id(tree, scratch[unsafe_offset=unique - 1]))):
            scratch[unsafe_offset=unique] = node
            scratch[unsafe_offset=tree.count + unique] = -1
            unique += 1
    return unique


def ds_call_slot(tree: ParsedJson, scratch: Pointer[mut=True, Int64, _], count: Int64, call: Int64) -> Int64:
    var id_node = pj_field(tree, call, StringSlice("id"))
    if not pj_nonblank(tree, id_node):
        return -1
    var id = pj_text(tree, id_node)
    var start: Int64 = 0
    var end = count
    while start < end:
        var middle = start + (end - start) // 2
        var candidate = pj_text(tree, ds_output_id(tree, scratch[unsafe_offset=middle]))
        if pj_less(candidate, id):
            start = middle + 1
        else:
            end = middle
    if start < count and pj_equal(pj_text(tree, ds_output_id(tree, scratch[unsafe_offset=start])), id):
        return start
    return -1


def ds_call_selected(tree: ParsedJson, scratch: Pointer[mut=True, Int64, _], count: Int64, call: Int64) -> Bool:
    var slot = ds_call_slot(tree, scratch, count, call)
    return slot >= 0 and scratch[unsafe_offset=tree.count + slot] == call
