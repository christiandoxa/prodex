from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NUMBER, JSON_NULL, JSON_FALSE, pj_literal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_contains
from runtime_model_catalog import prodex_mojo_runtime_model_catalog_v1


def equals(output: Pointer[mut=True, UInt8, _], written: Int64, expected: StringSlice) -> Bool:
    return written == Int64(expected.byte_length()) and rich_slice_contains(output, ProdexRichSlice(0, written), expected, False)


def main() raises:
    var raw = StringSlice("{\"models\":[{\"id\":\" 　Model雪 \\n\",\"name\":\"\",\"description\":false,\"context_window_tokens\":1,\"context_window\":999},{\"id\":null,\"slug\":\"ignored\"},false,{\"model\":\"other\",\"max_prompt_tokens\":0,\"capabilities\":{\"limits\":{\"max_prompt_tokens\":444}},\"context_window\":777}]}")
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=19))
    var scratch_storage = alloc(Layout[Int64](count=38))
    var output_storage = alloc(Layout[UInt8](count=2048))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 0, 262)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_ARRAY, 2, -1, 0, pj_literal(StringSlice("models")), pj_literal(StringSlice("")), 10, 251)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_OBJECT, 3, 8, 1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 11, 101)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_STRING, -1, 4, 2, pj_literal(StringSlice("id")), pj_literal(StringSlice(" 　Model雪 \n")), 17, 17)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_STRING, -1, 5, 2, pj_literal(StringSlice("name")), pj_literal(StringSlice("")), 42, 2)
    nodes[unsafe_offset=5] = ParsedJsonNode(JSON_FALSE, -1, 6, 2, pj_literal(StringSlice("description")), pj_literal(StringSlice("")), 59, 5)
    nodes[unsafe_offset=6] = ParsedJsonNode(JSON_NUMBER, -1, 7, 2, pj_literal(StringSlice("context_window_tokens")), pj_literal(StringSlice("")), 89, 1)
    nodes[unsafe_offset=7] = ParsedJsonNode(JSON_NUMBER, -1, -1, 2, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 108, 3)
    nodes[unsafe_offset=8] = ParsedJsonNode(JSON_OBJECT, 9, 11, 1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 113, 28)
    nodes[unsafe_offset=9] = ParsedJsonNode(JSON_NULL, -1, 10, 8, pj_literal(StringSlice("id")), pj_literal(StringSlice("")), 119, 4)
    nodes[unsafe_offset=10] = ParsedJsonNode(JSON_STRING, -1, -1, 8, pj_literal(StringSlice("slug")), pj_literal(StringSlice("ignored")), 131, 9)
    nodes[unsafe_offset=11] = ParsedJsonNode(JSON_FALSE, -1, 12, 1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 142, 5)
    nodes[unsafe_offset=12] = ParsedJsonNode(JSON_OBJECT, 13, -1, 1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 148, 112)
    nodes[unsafe_offset=13] = ParsedJsonNode(JSON_STRING, -1, 14, 12, pj_literal(StringSlice("model")), pj_literal(StringSlice("other")), 157, 7)
    nodes[unsafe_offset=14] = ParsedJsonNode(JSON_NUMBER, -1, 15, 12, pj_literal(StringSlice("max_prompt_tokens")), pj_literal(StringSlice("")), 185, 1)
    nodes[unsafe_offset=15] = ParsedJsonNode(JSON_OBJECT, 16, 18, 12, pj_literal(StringSlice("capabilities")), pj_literal(StringSlice("")), 202, 36)
    nodes[unsafe_offset=16] = ParsedJsonNode(JSON_OBJECT, 17, -1, 15, pj_literal(StringSlice("limits")), pj_literal(StringSlice("")), 212, 25)
    nodes[unsafe_offset=17] = ParsedJsonNode(JSON_NUMBER, -1, -1, 16, pj_literal(StringSlice("max_prompt_tokens")), pj_literal(StringSlice("")), 233, 3)
    nodes[unsafe_offset=18] = ParsedJsonNode(JSON_NUMBER, -1, -1, 12, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 256, 3)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    var count: Int64 = 19
    var length: Int64 = 262
    assert prodex_mojo_runtime_model_catalog_v1(1, 4, 1, input, count, source, length, scratch, count, 1, 0, 0, meta) == 0
    var required = metadata[unsafe_offset=1]
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_runtime_model_catalog_v1(1, 4, 1, input, count, source, length, scratch, count, 0, target, required - 1, meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_mojo_runtime_model_catalog_v1(1, 4, 1, input, count, source, length, scratch, count, 0, target, 2048, meta) == 0
    assert equals(output, metadata[unsafe_offset=1], StringSlice("{\"Ok\":[{\"slug\":\"Model雪\",\"display_name\":\"\",\"description\":null,\"context_window\":null},{\"slug\":\"other\",\"display_name\":null,\"description\":null,\"context_window\":777}]}"))
    assert prodex_mojo_runtime_model_catalog_v1(1, 3, 1, input, count, source, length, scratch, count, 0, target, 2048, meta) == 0
    assert equals(output, metadata[unsafe_offset=1], StringSlice('{"Err":"provider model catalog exceeds the hard limit of 3 entries"}'))
    nodes[unsafe_offset=0].first_child = -1
    assert prodex_mojo_runtime_model_catalog_v1(1, 4, 1, input, 1, source, length, scratch, count, 0, target, 2048, meta) == 0
    assert equals(output, metadata[unsafe_offset=1], StringSlice('{"Err":"provider model catalog is missing models array"}'))
    nodes[unsafe_offset=0].first_child = 1
    nodes[unsafe_offset=1].first_child = -1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 1, input, 2, source, length, scratch, count, 0, target, 2048, meta) == 0
    assert equals(output, metadata[unsafe_offset=1], StringSlice('{"Ok":[]}'))
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_runtime_model_catalog_v1(1, -1, 1, input, 2, source, length, scratch, count, 0, target, 2048, meta) == 1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 2, input, 2, source, length, scratch, count, 0, target, 2048, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
