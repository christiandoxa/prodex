# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_failure_detail_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING
from rich_types import ProdexRichStringView
from ping_failure_detail import prodex_mojo_ping_failure_detail_v1


def main() raises:
    var node_storage = alloc(Layout[ParsedJsonNode](count=3))
    var scratch_storage = alloc(Layout[UInt64](count=6))
    var output_storage = alloc(Layout[UInt8](count=64))
    var meta_storage = alloc(Layout[Int64](count=2))
    var nodes = node_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var meta = meta_storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    var key = StringSlice("type")
    var value = StringSlice("turn.failed")
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_ARRAY, 1, -1, -1, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_OBJECT, 2, -1, 0, empty.copy(), empty.copy(), 0, 0)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_STRING, -1, -1, 1,
        ProdexRichStringView(UInt(Int(key.unsafe_ptr())), UInt(key.byte_length())),
        ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length())), 0, 0)
    var source = UInt64(Int(nodes))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var dest = UInt64(Int(output))
    var metadata = UInt64(Int(meta))
    assert prodex_mojo_ping_failure_detail_v1(1, 0, 0, source, 3, 0, 0, scratch, 3, 0, dest, 64, metadata) == 0
    var expected = StringSlice("turn failed")
    assert meta[unsafe_offset=0] == 1 and meta[unsafe_offset=1] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    output[unsafe_offset=0] = 61
    meta[unsafe_offset=0] = 71
    assert prodex_mojo_ping_failure_detail_v1(1, 0, 0, source, 3, 0, 0, scratch, 3, 0, dest, 1, metadata) == 3
    assert prodex_mojo_ping_failure_detail_v1(2, 0, 0, source, 3, 0, 0, scratch, 3, 0, dest, 64, metadata) == 4
    assert prodex_mojo_ping_failure_detail_v1(1, 1, 0, source, 3, 0, 0, scratch, 3, 0, dest, 64, metadata) == 1
    assert output[unsafe_offset=0] == 61 and meta[unsafe_offset=0] == 71
    nodes[unsafe_offset=0].first_child = 0
    assert prodex_mojo_ping_failure_detail_v1(1, 0, 0, source, 3, 0, 0, scratch, 3, 0, dest, 64, metadata) == 1
    assert output[unsafe_offset=0] == 61 and meta[unsafe_offset=0] == 71
    nodes[unsafe_offset=0].first_child = 1
    var invalid_storage = alloc(Layout[UInt8](count=1))
    var invalid = invalid_storage.unsafe_ptr()
    invalid[unsafe_offset=0] = 255
    nodes[unsafe_offset=2].text = ProdexRichStringView(UInt(Int(invalid)), 1)
    assert prodex_mojo_ping_failure_detail_v1(1, 0, 0, source, 3, 0, 0, scratch, 3, 0, dest, 64, metadata) == 1
    assert output[unsafe_offset=0] == 61 and meta[unsafe_offset=0] == 71
    dealloc(invalid_storage^)
    dealloc(node_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(meta_storage^)
