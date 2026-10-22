from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_NULL, JSON_TRUE, pj_literal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_matches_literal
from main_catalog_projection import prodex_mojo_main_catalog_projection_v1


def main() raises:
    var raw = StringSlice("[{\"slug\":null,\"id\":\" demo \",\"aliases\":[\" a \",\"\",true]}]")
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=8))
    var scratch_storage = alloc(Layout[Int64](count=16))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_ARRAY, 1, -1, -1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 0, 55)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_OBJECT, 2, -1, 0, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 1, 53)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_NULL, -1, 3, 1, pj_literal(StringSlice("slug")), pj_literal(StringSlice("")), 9, 4)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_STRING, -1, 4, 1, pj_literal(StringSlice("id")), pj_literal(StringSlice(" demo ")), 19, 8)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_ARRAY, 5, -1, 1, pj_literal(StringSlice("aliases")), pj_literal(StringSlice("")), 38, 15)
    nodes[unsafe_offset=5] = ParsedJsonNode(JSON_STRING, -1, 6, 4, pj_literal(StringSlice("")), pj_literal(StringSlice(" a ")), 39, 5)
    nodes[unsafe_offset=6] = ParsedJsonNode(JSON_STRING, -1, 7, 4, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 45, 2)
    nodes[unsafe_offset=7] = ParsedJsonNode(JSON_TRUE, -1, -1, 4, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 48, 4)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    var length: Int64 = 55
    assert prodex_mojo_main_catalog_projection_v1(1, 0, 0, input, 8, source, length, scratch, 8, 0, target, 4096, meta) == 0
    assert rich_slice_matches_literal["[{\"id\":\"demo\",\"label\":\"demo\",\"priority\":9223372036854775807,\"supported\":true,\"hidden\":false,\"listed\":true,\"efforts\":[],\"aliases\":[\"a\"],\"default_effort\":null}]"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    var required = metadata[unsafe_offset=1]
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_main_catalog_projection_v1(1, 0, 0, input, 8, source, length, scratch, 8, 0, target, required - 1, meta) == 3
    assert prodex_mojo_main_catalog_projection_v1(2, 0, 0, input, 8, source, length, scratch, 8, 0, target, 4096, meta) == 4
    assert prodex_mojo_main_catalog_projection_v1(1, 1, 0, input, 8, source, length, scratch, 8, 0, target, 4096, meta) == 1
    assert prodex_mojo_main_catalog_projection_v1(1, 0, 1, input, 8, source, length, scratch, 8, 0, target, 4096, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
