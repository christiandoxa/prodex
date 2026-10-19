# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_simple_response_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from std.sys.info import size_of
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from gemini_sse_state import GeminiSimplePart, gemini_simple_response_plan


def main() raises:
    assert size_of[GeminiSimplePart]() == 40
    var key_storage = alloc(Layout[ProdexRichStringView](count=3))
    var part_storage = alloc(Layout[GeminiSimplePart](count=2))
    var output_storage = alloc(Layout[Int64](count=3))
    var keys = key_storage.unsafe_ptr()
    var parts = part_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    keys[unsafe_offset=0] = pj_literal(StringSlice("functionCall"))
    keys[unsafe_offset=1] = pj_literal(StringSlice("functionCall"))
    keys[unsafe_offset=2] = pj_literal(StringSlice("extra"))
    parts[unsafe_offset=0] = GeminiSimplePart(UInt64(UInt(Int(keys))), 1, pj_literal(StringSlice("tool")), 49)
    parts[unsafe_offset=1] = GeminiSimplePart(UInt64(UInt(Int(keys)) + 16), 2, pj_literal(StringSlice("other")), 49)
    for index in range(3):
        output[unsafe_offset=index] = -1
    var source = UInt(Int(parts))
    var destination = UInt(Int(output))
    assert gemini_simple_response_plan(2, source, 2, destination, 3) == 4
    assert gemini_simple_response_plan(1, source, 2, destination, 2) == 3
    parts[unsafe_offset=1].flags = 128
    assert gemini_simple_response_plan(1, source, 2, destination, 3) == 1
    for index in range(3):
        assert output[unsafe_offset=index] == -1
    parts[unsafe_offset=1].flags = 49
    assert gemini_simple_response_plan(1, source, 2, destination, 3) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 3
    keys[unsafe_offset=0] = pj_literal(StringSlice("text"))
    for index in range(2):
        parts[unsafe_offset=index] = GeminiSimplePart(UInt64(UInt(Int(keys))), 1, pj_literal(StringSlice("")), 7)
    assert gemini_simple_response_plan(1, source, 2, destination, 3) == 0
    assert output[unsafe_offset=0] == 0
    dealloc(key_storage^)
    dealloc(part_storage^)
    dealloc(output_storage^)
