# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_transcript_context_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING
from rich_types import ProdexRichStringView
from log_transcript_context import prodex_mojo_log_transcript_context_v1


def main() raises:
    var nodes_storage = alloc(Layout[ParsedJsonNode](count=2))
    var output_storage = alloc(Layout[UInt8](count=64))
    var scratch_storage = alloc(Layout[UInt64](count=4))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var nodes = nodes_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var key = StringSlice("model")
    var value = StringSlice("test")
    var empty = ProdexRichStringView(0, 0)
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_OBJECT, 1, -1, -1, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_STRING, -1, -1, 0,
        ProdexRichStringView(UInt(Int(key.unsafe_ptr())), UInt(key.byte_length())),
        ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length())), 0, 0)
    var destination = UInt64(Int(output))
    var source = UInt64(Int(nodes))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var meta = UInt64(Int(metadata))
    assert prodex_mojo_log_transcript_context_v1(1, 1, 0, source, 2, 0, 0, scratch, 2, 0, destination, 64, meta) == 0
    var expected = StringSlice("[[\"turn-context\",\"model=test\"]]")
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    output[unsafe_offset=0] = 61
    metadata[unsafe_offset=0] = 71
    assert prodex_mojo_log_transcript_context_v1(1, 1, 0, source, 2, 0, 0, scratch, 2, 0, destination, 1, meta) == 3
    assert output[unsafe_offset=0] == 61 and metadata[unsafe_offset=0] == 71
    dealloc(nodes_storage^)
    dealloc(output_storage^)
    dealloc(scratch_storage^)
    dealloc(metadata_storage^)
