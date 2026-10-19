# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_precommit_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from gemini_precommit import prodex_mojo_gemini_precommit_v1
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, pj_literal
from rich_types import ProdexRichStringView


def main() raises:
    var storage = alloc(Layout[ParsedJsonNode](count=4))
    var nodes = storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_ARRAY, 2, -1, 0, pj_literal(StringSlice("candidates")), empty.copy(), 0, 0)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_OBJECT, 3, -1, 1, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_STRING, -1, -1, 2, pj_literal(StringSlice("finishReason")), pj_literal(StringSlice("OTHER")), 0, 0)
    var output_storage = alloc(Layout[Int64](count=4))
    var output = output_storage.unsafe_ptr()
    var scratch_storage = alloc(Layout[UInt8](count=5))
    var scratch = UInt(Int(scratch_storage.unsafe_ptr()))
    var source = UInt(Int(nodes))
    var destination = UInt(Int(output))
    assert prodex_mojo_gemini_precommit_v1(1, source, 4, 0, 0, 0, 0, 0, 0, scratch, 5, destination) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=3] == 3
    nodes[unsafe_offset=3].text = pj_literal(StringSlice("STOP"))
    assert prodex_mojo_gemini_precommit_v1(1, source, 4, 0, 0, 0, 0, 0, 0, scratch, 5, destination) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=3] == -1
    assert prodex_mojo_gemini_precommit_v1(1, source, 4, 0, 0, 0, 0, 0, 1, scratch, 5, destination) == 0
    assert output[unsafe_offset=0] == 1 and output[unsafe_offset=2] == 1
    output[unsafe_offset=0] = 71
    assert prodex_mojo_gemini_precommit_v1(2, source, 4, 0, 0, 0, 0, 0, 0, scratch, 5, destination) == 4
    assert output[unsafe_offset=0] == 71
    assert prodex_mojo_gemini_precommit_v1(1, source, 4, 0, 0, 0, 0, 0, 0, scratch, 3, destination) == 1
    assert output[unsafe_offset=0] == 71
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(storage^)
