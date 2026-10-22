from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NUMBER, JSON_NULL, JSON_TRUE, pj_literal
from rich_types import ProdexRichStringView
from copilot_model_catalog import prodex_mojo_copilot_model_catalog_v1


def node(kind: Int64, parent: Int64, first: Int64, next: Int64, key: StringSlice, text: StringSlice, start: Int64 = 0, length: Int64 = 0) -> ParsedJsonNode:
    return ParsedJsonNode(kind, first, next, parent, pj_literal(key), pj_literal(text), start, length)


def check_text(output: Pointer[mut=True, UInt8, _], length: Int64, expected: StringSlice):
    assert length == Int64(expected.byte_length())
    var ptr = expected.unsafe_ptr()
    for i in range(length):
        assert output[unsafe_offset=i] == ptr[unsafe_offset=i]


def main() raises:
    var raw = StringSlice('{"models":[{"id":" x ","name":" Display ","model":"fallback","context_window":42,"max_prompt_tokens":7,"capabilities":{"a":null,"nested":[null,{"n":null,"ok":true}]}}]}')
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=15))
    var scratch_storage = alloc(Layout[Int64](count=30))
    var output_storage = alloc(Layout[UInt8](count=1024))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var empty = StringSlice("")
    nodes[unsafe_offset=0] = node(JSON_OBJECT, -1, 1, -1, empty, empty, 0, 168)
    nodes[unsafe_offset=1] = node(JSON_ARRAY, 0, 2, -1, StringSlice("models"), empty)
    nodes[unsafe_offset=2] = node(JSON_OBJECT, 1, 3, -1, empty, empty)
    nodes[unsafe_offset=3] = node(JSON_STRING, 2, -1, 4, StringSlice("id"), StringSlice(" x "))
    nodes[unsafe_offset=4] = node(JSON_STRING, 2, -1, 5, StringSlice("name"), StringSlice(" Display "))
    nodes[unsafe_offset=5] = node(JSON_STRING, 2, -1, 6, StringSlice("model"), StringSlice("fallback"))
    nodes[unsafe_offset=6] = node(JSON_NUMBER, 2, -1, 7, StringSlice("context_window"), empty, 78, 2)
    nodes[unsafe_offset=7] = node(JSON_NUMBER, 2, -1, 8, StringSlice("max_prompt_tokens"), empty, 101, 1)
    nodes[unsafe_offset=8] = node(JSON_OBJECT, 2, 9, -1, StringSlice("capabilities"), empty)
    nodes[unsafe_offset=9] = node(JSON_NULL, 8, -1, 10, StringSlice("a"), empty, 123, 4)
    nodes[unsafe_offset=10] = node(JSON_ARRAY, 8, 11, -1, StringSlice("nested"), empty)
    nodes[unsafe_offset=11] = node(JSON_NULL, 10, -1, 12, empty, empty, 123, 4)
    nodes[unsafe_offset=12] = node(JSON_OBJECT, 10, 13, -1, empty, empty)
    nodes[unsafe_offset=13] = node(JSON_NULL, 12, -1, 14, StringSlice("n"), empty, 123, 4)
    nodes[unsafe_offset=14] = node(JSON_TRUE, 12, -1, -1, StringSlice("ok"), empty, 158, 4)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 1, 0, 0, meta) == 0
    assert metadata[unsafe_offset=0] == 1
    var required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, required - 1, meta) == 3
    assert metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88 and output[] == 93
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, required, meta) == 0
    check_text(output, metadata[unsafe_offset=1], StringSlice('{"Ok":[{"id":"x","object":"model","owned_by":"github-copilot","display_name":"Display","description":"GitHub Copilot model available for this account: Display.","context_window":7,"input_cost_per_million_microusd":0,"output_cost_per_million_microusd":0,"max_context_window":42,"max_prompt_tokens":7,"capabilities":{"nested":[null,{"ok":true}]}}]}'))
    assert prodex_mojo_copilot_model_catalog_v1(1, 0, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 0
    check_text(output, metadata[unsafe_offset=1], StringSlice('{"Err":"Copilot model catalog exceeds the hard limit of 0 entries"}'))
    nodes[unsafe_offset=3].kind = JSON_NULL
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 0
    check_text(output, metadata[unsafe_offset=1], StringSlice('{"Ok":[]}'))
    nodes[unsafe_offset=3].kind = JSON_STRING
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_mojo_copilot_model_catalog_v1(2, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 4
    assert prodex_mojo_copilot_model_catalog_v1(1, -1, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 1, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input + 1, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 0, source, 168, scratch, 15, 0, target, 1024, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 14, 0, target, 1024, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 1, target, 0, meta) == 1
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta + 1) == 1
    nodes[unsafe_offset=14].parent = 14
    assert prodex_mojo_copilot_model_catalog_v1(1, 1024, 0, input, 15, source, 168, scratch, 15, 0, target, 1024, meta) == 1
    assert metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88 and output[] == 93
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
