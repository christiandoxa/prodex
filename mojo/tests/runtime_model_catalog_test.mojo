from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING, JSON_NUMBER, JSON_NULL, JSON_ARRAY, pj_literal, pj_equal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_contains
from runtime_model_catalog import prodex_mojo_runtime_model_catalog_v1, runtime_reasoning_description


def node(kind: Int64, next: Int64, key: StringSlice, text: StringSlice, start: Int64, length: Int64) -> ParsedJsonNode:
    return ParsedJsonNode(kind, -1, next, 0, pj_literal(key), pj_literal(text), start, length)


def has(output: Pointer[mut=True, UInt8, _], written: Int64, text: StringSlice) -> Bool:
    return rich_slice_contains(output, ProdexRichSlice(0, written), text, False)


def main() raises:
    for effort, description in [
        (StringSlice("none"), StringSlice("No reasoning effort")),
        (StringSlice("minimal"), StringSlice("Minimal reasoning effort")),
        (StringSlice("low"), StringSlice("Low reasoning effort")),
        (StringSlice("medium"), StringSlice("Medium reasoning effort")),
        (StringSlice("high"), StringSlice("High reasoning effort")),
        (StringSlice("xhigh"), StringSlice("Extra-high reasoning effort")),
        (StringSlice("max"), StringSlice("Max reasoning effort")),
        (StringSlice("ultra"), StringSlice("Ultra reasoning effort")),
    ]:
        assert pj_equal(runtime_reasoning_description(pj_literal(effort)), pj_literal(description))
    assert runtime_reasoning_description(pj_literal(StringSlice("unknown"))).len == 0
    var raw = StringSlice('{"provider":"copilot","slug":"synthetic","display_name":"Display","description":"Description","priority":1,"context_window":200000,"auto_compact_token_limit":190000,"default_reasoning_effort":null,"supported_reasoning_efforts":null}')
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=11))
    var scratch_storage = alloc(Layout[Int64](count=22))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var empty = StringSlice("")
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, pj_literal(empty), pj_literal(empty), 0, 232)
    nodes[unsafe_offset=1] = node(JSON_STRING, 2, StringSlice("provider"), StringSlice("copilot"), 12, 9)
    nodes[unsafe_offset=2] = node(JSON_STRING, 3, StringSlice("slug"), StringSlice("synthetic"), 29, 11)
    nodes[unsafe_offset=3] = node(JSON_STRING, 4, StringSlice("display_name"), StringSlice("Display"), 56, 9)
    nodes[unsafe_offset=4] = node(JSON_STRING, 5, StringSlice("description"), StringSlice("Description"), 80, 13)
    nodes[unsafe_offset=5] = node(JSON_NUMBER, 6, StringSlice("priority"), empty, 105, 1)
    nodes[unsafe_offset=6] = node(JSON_NUMBER, 7, StringSlice("context_window"), empty, 124, 6)
    nodes[unsafe_offset=7] = node(JSON_NUMBER, 8, StringSlice("auto_compact_token_limit"), empty, 158, 6)
    nodes[unsafe_offset=8] = node(JSON_NULL, 9, StringSlice("default_reasoning_effort"), empty, 192, 4)
    nodes[unsafe_offset=9] = node(JSON_NULL, -1, StringSlice("supported_reasoning_efforts"), empty, 227, 4)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 1, 0, 0, meta) == 0
    var required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 0, target, required - 1, meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"default_reasoning_level":"high"'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"effort":"xhigh","description":"Extra-high reasoning effort"'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"input_modalities":["text","image"]'))
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("deepseek"))
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"description":"DeepSeek max reasoning effort"'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"input_modalities":["text"]'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supports_reasoning_summaries":false'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"base_instructions":"You are Codex, a coding agent.'))
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("kiro"))
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supports_reasoning_summaries":false'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supports_reasoning_summary_parameter":false'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"input_modalities":["text"]'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supports_search_tool":false'))
    nodes[unsafe_offset=9].kind = JSON_ARRAY
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 10, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supported_reasoning_levels":[]'))
    nodes[unsafe_offset=9].first_child = 10
    nodes[unsafe_offset=10] = node(JSON_STRING, -1, empty, StringSlice("unknown"), 0, 0)
    nodes[unsafe_offset=10].parent = 9
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"supported_reasoning_levels":[]'))
    nodes[unsafe_offset=10].text = pj_literal(StringSlice("ultra"))
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"effort":"ultra","description":"Ultra reasoning effort"'))
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_mojo_runtime_model_catalog_v1(2, 0, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 4
    assert prodex_mojo_runtime_model_catalog_v1(1, 1, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 2, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input + 1, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 11, source, 232, scratch, 11, 1, target, 0, meta) == 1
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta + 1) == 1
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("unsupported"))
    assert prodex_mojo_runtime_model_catalog_v1(1, 0, 0, input, 11, source, 232, scratch, 11, 0, target, 4096, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
