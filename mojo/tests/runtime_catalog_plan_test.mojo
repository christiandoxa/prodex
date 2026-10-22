from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NUMBER, JSON_NULL, pj_literal
from rich_types import ProdexRichSlice
from rich_text import rich_slice_contains
from runtime_catalog_plan import prodex_mojo_runtime_catalog_plan_v1


def has(output: Pointer[mut=True, UInt8, _], written: Int64, expected: StringSlice) -> Bool:
    return rich_slice_contains(output, ProdexRichSlice(0, written), expected, False)


def main() raises:
    var raw = StringSlice("{\"provider\":\"copilot\",\"launch_model\":\" custom \",\"context_window\":100000,\"auto_compact_token_limit\":90000,\"dynamic_models\":[{\"slug\":\"custom\",\"display_name\":\"Dynamic\",\"description\":null,\"context_window\":300},{\"slug\":\"DUP\",\"display_name\":\"\",\"description\":\"\",\"context_window\":null},{\"slug\":\"fresh\",\"display_name\":null,\"description\":null,\"context_window\":null},{\"slug\":\"unsigned-limit\",\"display_name\":null,\"description\":null,\"context_window\":18446744073709551615}],\"static_models\":[{\"id\":\"base\",\"display_name\":\"Base\",\"description\":\"Base description\",\"context_window_tokens\":1000,\"aliases\":[\"custom\"],\"supported_reasoning_efforts\":[\"max\"],\"default_reasoning_effort\":\"max\"},{\"id\":\"other\",\"display_name\":\"Other\",\"description\":\"Other description\",\"context_window_tokens\":400,\"aliases\":[\"dup\"],\"supported_reasoning_efforts\":null,\"default_reasoning_effort\":null}]}")
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=46))
    var scratch_storage = alloc(Layout[Int64](count=368))
    var output_storage = alloc(Layout[UInt8](count=16384))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 0, 853)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_STRING, -1, 2, 0, pj_literal(StringSlice("provider")), pj_literal(StringSlice("copilot")), 12, 9)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_STRING, -1, 3, 0, pj_literal(StringSlice("launch_model")), pj_literal(StringSlice(" custom ")), 37, 10)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_NUMBER, -1, 4, 0, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 65, 6)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_NUMBER, -1, 5, 0, pj_literal(StringSlice("auto_compact_token_limit")), pj_literal(StringSlice("")), 99, 5)
    nodes[unsafe_offset=5] = ParsedJsonNode(JSON_ARRAY, 6, 26, 0, pj_literal(StringSlice("dynamic_models")), pj_literal(StringSlice("")), 122, 337)
    nodes[unsafe_offset=6] = ParsedJsonNode(JSON_OBJECT, 7, 11, 5, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 123, 82)
    nodes[unsafe_offset=7] = ParsedJsonNode(JSON_STRING, -1, 8, 6, pj_literal(StringSlice("slug")), pj_literal(StringSlice("custom")), 131, 8)
    nodes[unsafe_offset=8] = ParsedJsonNode(JSON_STRING, -1, 9, 6, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("Dynamic")), 155, 9)
    nodes[unsafe_offset=9] = ParsedJsonNode(JSON_NULL, -1, 10, 6, pj_literal(StringSlice("description")), pj_literal(StringSlice("")), 179, 4)
    nodes[unsafe_offset=10] = ParsedJsonNode(JSON_NUMBER, -1, -1, 6, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 201, 3)
    nodes[unsafe_offset=11] = ParsedJsonNode(JSON_OBJECT, 12, 16, 5, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 206, 71)
    nodes[unsafe_offset=12] = ParsedJsonNode(JSON_STRING, -1, 13, 11, pj_literal(StringSlice("slug")), pj_literal(StringSlice("DUP")), 214, 5)
    nodes[unsafe_offset=13] = ParsedJsonNode(JSON_STRING, -1, 14, 11, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("")), 235, 2)
    nodes[unsafe_offset=14] = ParsedJsonNode(JSON_STRING, -1, 15, 11, pj_literal(StringSlice("description")), pj_literal(StringSlice("")), 252, 2)
    nodes[unsafe_offset=15] = ParsedJsonNode(JSON_NULL, -1, -1, 11, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 272, 4)
    nodes[unsafe_offset=16] = ParsedJsonNode(JSON_OBJECT, 17, 21, 5, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 278, 77)
    nodes[unsafe_offset=17] = ParsedJsonNode(JSON_STRING, -1, 18, 16, pj_literal(StringSlice("slug")), pj_literal(StringSlice("fresh")), 286, 7)
    nodes[unsafe_offset=18] = ParsedJsonNode(JSON_NULL, -1, 19, 16, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("")), 309, 4)
    nodes[unsafe_offset=19] = ParsedJsonNode(JSON_NULL, -1, 20, 16, pj_literal(StringSlice("description")), pj_literal(StringSlice("")), 328, 4)
    nodes[unsafe_offset=20] = ParsedJsonNode(JSON_NULL, -1, -1, 16, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 350, 4)
    nodes[unsafe_offset=21] = ParsedJsonNode(JSON_OBJECT, 22, -1, 5, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 356, 102)
    nodes[unsafe_offset=22] = ParsedJsonNode(JSON_STRING, -1, 23, 21, pj_literal(StringSlice("slug")), pj_literal(StringSlice("unsigned-limit")), 364, 16)
    nodes[unsafe_offset=23] = ParsedJsonNode(JSON_NULL, -1, 24, 21, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("")), 396, 4)
    nodes[unsafe_offset=24] = ParsedJsonNode(JSON_NULL, -1, 25, 21, pj_literal(StringSlice("description")), pj_literal(StringSlice("")), 415, 4)
    nodes[unsafe_offset=25] = ParsedJsonNode(JSON_NUMBER, -1, -1, 21, pj_literal(StringSlice("context_window")), pj_literal(StringSlice("")), 437, 20)
    nodes[unsafe_offset=26] = ParsedJsonNode(JSON_ARRAY, 27, -1, 0, pj_literal(StringSlice("static_models")), pj_literal(StringSlice("")), 476, 376)
    nodes[unsafe_offset=27] = ParsedJsonNode(JSON_OBJECT, 28, 37, 26, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 477, 189)
    nodes[unsafe_offset=28] = ParsedJsonNode(JSON_STRING, -1, 29, 27, pj_literal(StringSlice("id")), pj_literal(StringSlice("base")), 483, 6)
    nodes[unsafe_offset=29] = ParsedJsonNode(JSON_STRING, -1, 30, 27, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("Base")), 505, 6)
    nodes[unsafe_offset=30] = ParsedJsonNode(JSON_STRING, -1, 31, 27, pj_literal(StringSlice("description")), pj_literal(StringSlice("Base description")), 526, 18)
    nodes[unsafe_offset=31] = ParsedJsonNode(JSON_NUMBER, -1, 32, 27, pj_literal(StringSlice("context_window_tokens")), pj_literal(StringSlice("")), 569, 4)
    nodes[unsafe_offset=32] = ParsedJsonNode(JSON_ARRAY, 33, 34, 27, pj_literal(StringSlice("aliases")), pj_literal(StringSlice("")), 584, 10)
    nodes[unsafe_offset=33] = ParsedJsonNode(JSON_STRING, -1, -1, 32, pj_literal(StringSlice("")), pj_literal(StringSlice("custom")), 585, 8)
    nodes[unsafe_offset=34] = ParsedJsonNode(JSON_ARRAY, 35, 36, 27, pj_literal(StringSlice("supported_reasoning_efforts")), pj_literal(StringSlice("")), 625, 7)
    nodes[unsafe_offset=35] = ParsedJsonNode(JSON_STRING, -1, -1, 34, pj_literal(StringSlice("")), pj_literal(StringSlice("max")), 626, 5)
    nodes[unsafe_offset=36] = ParsedJsonNode(JSON_STRING, -1, -1, 27, pj_literal(StringSlice("default_reasoning_effort")), pj_literal(StringSlice("max")), 660, 5)
    nodes[unsafe_offset=37] = ParsedJsonNode(JSON_OBJECT, 38, -1, 26, pj_literal(StringSlice("")), pj_literal(StringSlice("")), 667, 184)
    nodes[unsafe_offset=38] = ParsedJsonNode(JSON_STRING, -1, 39, 37, pj_literal(StringSlice("id")), pj_literal(StringSlice("other")), 673, 7)
    nodes[unsafe_offset=39] = ParsedJsonNode(JSON_STRING, -1, 40, 37, pj_literal(StringSlice("display_name")), pj_literal(StringSlice("Other")), 696, 7)
    nodes[unsafe_offset=40] = ParsedJsonNode(JSON_STRING, -1, 41, 37, pj_literal(StringSlice("description")), pj_literal(StringSlice("Other description")), 718, 19)
    nodes[unsafe_offset=41] = ParsedJsonNode(JSON_NUMBER, -1, 42, 37, pj_literal(StringSlice("context_window_tokens")), pj_literal(StringSlice("")), 762, 3)
    nodes[unsafe_offset=42] = ParsedJsonNode(JSON_ARRAY, 43, 44, 37, pj_literal(StringSlice("aliases")), pj_literal(StringSlice("")), 776, 7)
    nodes[unsafe_offset=43] = ParsedJsonNode(JSON_STRING, -1, -1, 42, pj_literal(StringSlice("")), pj_literal(StringSlice("dup")), 777, 5)
    nodes[unsafe_offset=44] = ParsedJsonNode(JSON_NULL, -1, 45, 37, pj_literal(StringSlice("supported_reasoning_efforts")), pj_literal(StringSlice("")), 814, 4)
    nodes[unsafe_offset=45] = ParsedJsonNode(JSON_NULL, -1, -1, 37, pj_literal(StringSlice("default_reasoning_effort")), pj_literal(StringSlice("")), 846, 4)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    var count: Int64 = 46
    var length: Int64 = 853
    assert prodex_mojo_runtime_catalog_plan_v1(1, 6, 0, input, count, source, length, scratch, count * 4, 1, 0, 0, meta) == 0
    var required = metadata[unsafe_offset=1]
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_runtime_catalog_plan_v1(1, 6, 0, input, count, source, length, scratch, count * 4, 0, target, required - 1, meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_mojo_runtime_catalog_plan_v1(1, 6, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('{"Ok":[{"slug":"custom","display_name":"Dynamic","description":"Base description","priority":1'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"slug":"DUP","display_name":"","description":""'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":1000,"auto_compact_token_limit":950'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":400,"auto_compact_token_limit":380'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":100000,"auto_compact_token_limit":90000'))
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":18446744073709551615,"auto_compact_token_limit":184467440737095516'))
    var limit_status = prodex_mojo_runtime_catalog_plan_v1(1, 5, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta)
    assert limit_status == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('{"Err":"provider model catalog exceeds the hard limit of 5 entries"}'))
    nodes[unsafe_offset=2].text = pj_literal(StringSlice("custom"))
    var status = prodex_mojo_runtime_catalog_plan_v1(1, 6, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta)
    assert status == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":300,"auto_compact_token_limit":285'))
    nodes[unsafe_offset=2].text = pj_literal(StringSlice("gpt-5.4"))
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 0
    assert has(output, metadata[unsafe_offset=1], StringSlice('"context_window":922000,"auto_compact_token_limit":875900'))
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_mojo_runtime_catalog_plan_v1(2, 7, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 4
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 1, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 1
    assert prodex_mojo_runtime_catalog_plan_v1(1, -1, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 1
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input + 1, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 1
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input, count, source, length, scratch, count * 4 - 1, 0, target, 16384, meta) == 1
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input, count, source, length, scratch, count * 4, 1, target, 16384, meta) == 1
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta + 1) == 1
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("deepseek"))
    assert prodex_mojo_runtime_catalog_plan_v1(1, 7, 0, input, count, source, length, scratch, count * 4, 0, target, 16384, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(nodes_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
