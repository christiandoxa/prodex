from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import ParsedJson, JSON_ARRAY, JSON_OBJECT, pj_kind, pj_child, pj_next, pj_field, pj_equal
from json_sink import JsonSink, js_byte, js_literal, js_string, js_raw
from deepseek_message_common import ds_field_prefix


def ds_key_member(tree: ParsedJson, object: Int64, key: ProdexRichStringView) -> Int64:
    var child = pj_child(tree, object)
    while child >= 0:
        if pj_equal(key, tree.nodes[unsafe_offset=child].key):
            return child
        child = pj_next(tree, child)
    return -1


def ds_merge_objects(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, left: Int64, right: Int64, merge_nested: Bool):
    js_byte(sink, 123)
    var count: Int64 = 0
    var child = pj_child(tree, left)
    while child >= 0:
        ds_field_prefix(sink, tree, child, Pointer(to=count))
        var replacement = ds_key_member(tree, right, tree.nodes[unsafe_offset=child].key)
        if replacement < 0:
            js_raw(sink, tree, child)
        elif merge_nested and pj_kind(tree, replacement) == JSON_OBJECT:
            # Object-valued metadata does not overwrite an existing scalar.
            # For two objects only their immediate child fields are merged.
            if pj_kind(tree, child) == JSON_OBJECT:
                ds_merge_objects(sink, tree, child, replacement, False)
            else:
                js_raw(sink, tree, child)
        else:
            js_raw(sink, tree, replacement)
        child = pj_next(tree, child)
    child = pj_child(tree, right)
    while child >= 0:
        if ds_key_member(tree, left, tree.nodes[unsafe_offset=child].key) < 0:
            ds_field_prefix(sink, tree, child, Pointer(to=count))
            js_raw(sink, tree, child)
        child = pj_next(tree, child)
    js_byte(sink, 125)


def ds_merge_metadata(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson) -> Bool:
    if pj_kind(tree, 0) != JSON_ARRAY:
        return False
    var response = pj_child(tree, 0)
    var metadata = pj_next(tree, response)
    if response < 0:
        return False
    var old_metadata = pj_field(tree, response, StringSlice("metadata"))
    if pj_kind(tree, response) != JSON_OBJECT or pj_kind(tree, metadata) != JSON_OBJECT or old_metadata >= 0 and pj_kind(tree, old_metadata) != JSON_OBJECT:
        js_raw(sink, tree, response)
        return True
    var count: Int64 = 0
    js_byte(sink, 123)
    var child = pj_child(tree, response)
    while child >= 0:
        ds_field_prefix(sink, tree, child, Pointer(to=count))
        if child == old_metadata:
            ds_merge_objects(sink, tree, old_metadata, metadata, True)
        else:
            js_raw(sink, tree, child)
        child = pj_next(tree, child)
    if old_metadata < 0:
        if count > 0:
            js_byte(sink, 44)
        js_literal(sink, StringSlice('"metadata":'))
        ds_merge_objects(sink, tree, -1, metadata, True)
    js_byte(sink, 125)
    return True
