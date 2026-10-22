from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING, JSON_NUMBER, pj_literal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_contains
from runtime_model_catalog import prodex_mojo_runtime_model_catalog_v1


def main() raises:
    var raw = StringSlice("{\"provider\":\"local\",\"slug\":\"synthetic\",\"context_window\":0,\"auto_compact_token_limit\":18446744073709551615}")
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=5))
    var scratch_storage = alloc(Layout[Int64](count=10))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 0, 106)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_STRING, -1, 2, 0, pj_literal(StringSlice("provider")), pj_literal(StringSlice("local")), 12, 7)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_STRING, -1, 3, 0, pj_literal(StringSlice("slug")), pj_literal(StringSlice("synthetic")), 27, 11)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_NUMBER, -1, 4, 0, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 56, 1)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_NUMBER, -1, -1, 0, pj_literal(StringSlice("auto_compact_token_limit")), pj_literal(StringSlice("")), 85, 20)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    var length: Int64 = 106
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 5, source, length, scratch, 5, 0, target, 4096, meta) == 0
    for text in [
        StringSlice('"slug":"synthetic","display_name":"synthetic","description":"Local OpenAI-compatible model routed through Prodex."'),
        StringSlice('"default_reasoning_level":"high"'),
        StringSlice('"effort":"xhigh","description":"Max reasoning effort"'),
        StringSlice('"priority":1'),
        StringSlice('"base_instructions":""'),
        StringSlice('"supports_reasoning_summaries":false'),
        StringSlice('"input_modalities":["text"]'),
        StringSlice('"supports_search_tool":true'),
        StringSlice('"context_window":0,"max_context_window":0,"auto_compact_token_limit":18446744073709551615'),
    ]:
        assert rich_slice_contains(output, ProdexRichSlice(0, metadata[unsafe_offset=1]), text, False)
    var required = metadata[unsafe_offset=1]
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 5, source, length, scratch, 5, 0, target, required - 1, meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
