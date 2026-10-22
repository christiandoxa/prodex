from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NUMBER, pj_literal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_contains
from runtime_catalog_plan import prodex_mojo_runtime_catalog_plan_v1


def main() raises:
    var raw = StringSlice("{\"provider\":\"deepseek\",\"launch_model\":\" PRO \",\"context_window\":0,\"auto_compact_token_limit\":18446744073709551615,\"dynamic_models\":[],\"static_models\":[{\"id\":\"pro\",\"display_name\":\"DeepSeek Pro\",\"description\":\"Known\",\"context_window_tokens\":999,\"aliases\":[]}]}")
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=14))
    var scratch_storage = alloc(Layout[Int64](count=112))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 0, 257)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_STRING, -1, 2, 0, pj_literal(StringSlice("provider")), pj_literal(StringSlice("deepseek")), 12, 10)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_STRING, -1, 3, 0, pj_literal(StringSlice("launch_model")), pj_literal(StringSlice(" PRO ")), 38, 7)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_NUMBER, -1, 4, 0, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 63, 1)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_NUMBER, -1, 5, 0, pj_literal(StringSlice("auto_compact_token_limit")), pj_literal(StringSlice("")), 92, 20)
    nodes[unsafe_offset=5] = ParsedJsonNode(JSON_ARRAY, -1, 6, 0, pj_literal(StringSlice("dynamic_models")), pj_literal(StringSlice("")), 130, 2)
    nodes[unsafe_offset=6] = ParsedJsonNode(JSON_ARRAY, 7, -1, 0, pj_literal(StringSlice("static_models")), pj_literal(StringSlice("")), 149, 107)
    nodes[unsafe_offset=7] = ParsedJsonNode(JSON_OBJECT, 8, -1, 6, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 150, 105)
    nodes[unsafe_offset=8] = ParsedJsonNode(JSON_STRING, -1, 9, 7, pj_literal(StringSlice("id")), pj_literal(StringSlice("pro")), 156, 5)
    nodes[unsafe_offset=9] = ParsedJsonNode(JSON_STRING, -1, 10, 7, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("DeepSeek Pro")), 177, 14)
    nodes[unsafe_offset=10] = ParsedJsonNode(JSON_STRING, -1, 11, 7, pj_literal(StringSlice("description")), pj_literal(StringSlice("Known")), 206, 7)
    nodes[unsafe_offset=11] = ParsedJsonNode(JSON_NUMBER, -1, 12, 7, pj_literal(StringSlice("context_window_tokens")), pj_literal(StringSlice("")), 238, 3)
    nodes[unsafe_offset=12] = ParsedJsonNode(JSON_ARRAY, -1, -1, 7, pj_literal(StringSlice("aliases")), pj_literal(StringSlice("")), 252, 2)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    var length: Int64 = 257
    assert prodex_mojo_runtime_catalog_plan_v1(1, 1, 0, input, 13, source, length, scratch, 52, 0, target, 4096, meta) == 0
    var expected = StringSlice("{\"Ok\":[{\"slug\":\"PRO\",\"display_name\":\"DeepSeek Pro\",\"description\":\"Known\",\"priority\":1,\"context_window\":0,\"auto_compact_token_limit\":18446744073709551615}]}")
    assert metadata[unsafe_offset=1] == Int64(expected.byte_length())
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), expected, False)
    nodes[unsafe_offset=2].text = pj_literal(StringSlice("unknown\n雪"))
    assert prodex_mojo_runtime_catalog_plan_v1(1, 2, 0, input, 13, source, length, scratch, 52, 0, target, 4096, meta) == 0
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), StringSlice('"description":"DeepSeek model routed through the Prodex Responses adapter."'), False)
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), StringSlice('"priority":2,"context_window":0,"auto_compact_token_limit":18446744073709551615'), False)
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("gemini"))
    nodes[unsafe_offset=2].text = pj_literal(StringSlice(" PRO "))
    assert prodex_mojo_runtime_catalog_plan_v1(1, 1, 0, input, 13, source, length, scratch, 52, 0, target, 4096, meta) == 0
    assert metadata[unsafe_offset=1] == Int64(expected.byte_length())
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), expected, False)
    nodes[unsafe_offset=2].text = pj_literal(StringSlice("unknown\n雪"))
    assert prodex_mojo_runtime_catalog_plan_v1(1, 2, 0, input, 13, source, length, scratch, 52, 0, target, 4096, meta) == 0
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), StringSlice('"description":"Gemini model routed through the Prodex Responses adapter."'), False)
    nodes[unsafe_offset=2].text = pj_literal(StringSlice("catalog-alias"))
    nodes[unsafe_offset=12].first_child = 13
    nodes[unsafe_offset=13] = ParsedJsonNode(JSON_STRING, -1, -1, 12, pj_literal(StringSlice("")), pj_literal(StringSlice("catalog-alias")), 0, 0)
    assert prodex_mojo_runtime_catalog_plan_v1(1, 2, 0, input, 14, source, length, scratch, 56, 0, target, 4096, meta) == 0
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), StringSlice('"slug":"catalog-alias","display_name":"catalog-alias","description":"Gemini model routed through the Prodex Responses adapter."'), False)
    assert prodex_mojo_runtime_catalog_plan_v1(1, 1, 0, input, 14, source, length, scratch, 56, 0, target, 4096, meta) == 0
    var limit_error = StringSlice('{"Err":"Gemini model catalog exceeds the hard limit of 1 entries"}')
    assert metadata[unsafe_offset=1] == Int64(limit_error.byte_length())
    assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), limit_error, False)
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
