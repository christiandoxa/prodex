# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_guardrail_conversation_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_guardrail_conversation import prodex_mojo_gemini_guardrail_conversation_v1
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, pj_literal
from rich_types import ProdexRichStringView


def main() raises:
    var storage = alloc(Layout[ParsedJsonNode](count=5))
    var nodes = storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_STRING, -1, 2, 0, pj_literal(StringSlice("prefix")), pj_literal(StringSlice("p")), 0, 0)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_ARRAY, 3, -1, 0, pj_literal(StringSlice("value")), empty.copy(), 0, 0)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_STRING, -1, 4, 2, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_STRING, -1, -1, 2, empty.copy(), pj_literal(StringSlice("x")), 0, 0)
    var scratch_storage = alloc(Layout[UInt64](count=16))
    var scratch = UInt(Int(scratch_storage.unsafe_ptr()))
    var meta_storage = alloc(Layout[Int64](count=2))
    var meta = meta_storage.unsafe_ptr()
    var source = UInt(Int(nodes))
    var metadata = UInt(Int(meta))
    assert prodex_mojo_gemini_guardrail_conversation_v1(1, 0, 0, source, 5, 0, 0, scratch, 8, 1, 0, 0, metadata) == 0
    # The wire result escapes each LF inside its JSON string.
    assert meta[unsafe_offset=0] == 1 and meta[unsafe_offset=1] == 8
    var output_storage = alloc(Layout[UInt8](count=8))
    var output = output_storage.unsafe_ptr()
    assert prodex_mojo_gemini_guardrail_conversation_v1(1, 0, 0, source, 5, 0, 0, scratch, 8, 0, UInt(Int(output)), 8, metadata) == 0
    assert output[unsafe_offset=0] == 34 and output[unsafe_offset=1] == 112
    assert output[unsafe_offset=2] == 92 and output[unsafe_offset=3] == 110
    assert output[unsafe_offset=4] == 92 and output[unsafe_offset=5] == 110
    assert output[unsafe_offset=6] == 120 and output[unsafe_offset=7] == 34
    meta[unsafe_offset=0] = 71
    assert prodex_mojo_gemini_guardrail_conversation_v1(2, 0, 0, source, 5, 0, 0, scratch, 8, 1, 0, 0, metadata) == 4
    assert meta[unsafe_offset=0] == 71
    nodes[unsafe_offset=1].text = pj_literal(StringSlice("a longer fixture prefix"))
    assert prodex_mojo_gemini_guardrail_conversation_v1(1, 0, 0, source, 5, 0, 0, scratch, 1, 1, 0, 0, metadata) == 1
    assert meta[unsafe_offset=0] == 71
    dealloc(output_storage^)
    dealloc(meta_storage^)
    dealloc(scratch_storage^)
    dealloc(storage^)
