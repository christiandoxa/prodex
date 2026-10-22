from std.memory.alloc import alloc, dealloc, Layout
from super_expose import prodex_mojo_super_expose_dispatch_response_v1


def main() raises:
    var os = alloc(Layout[UInt8](count=64))
    var ms = alloc(Layout[Int64](count=5))
    var output = os.unsafe_ptr()
    var metadata = ms.unsafe_ptr()
    var target = UInt(Int(output))
    var meta = UInt(Int(metadata))
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 0, target, 64, meta) == 0
    assert metadata[unsafe_offset=0] == 0 and metadata[unsafe_offset=1] == 1
    assert metadata[unsafe_offset=4] == 0
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 1, target, 64, meta) == 0
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == 0
    assert metadata[unsafe_offset=2] == 202 and metadata[unsafe_offset=4] == 0
    for decision in range(Int64(2), Int64(9)):
        assert prodex_mojo_super_expose_dispatch_response_v1(1, decision, target, 64, meta) == 0
        assert metadata[unsafe_offset=0] == 2 and metadata[unsafe_offset=2] == 400
        assert metadata[unsafe_offset=1] == Int64(decision >= 4)
        assert metadata[unsafe_offset=3] == Int64(-32601 if decision == 2 else -32600 if decision == 3 else -32602)
        assert metadata[unsafe_offset=4] > 0 and metadata[unsafe_offset=4] <= 64
    var expected = StringSlice("tool arguments must be an object")
    assert metadata[unsafe_offset=4] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    for index in range(64):
        output[unsafe_offset=index] = 73
    for index in range(5):
        metadata[unsafe_offset=index] = 83
    assert prodex_mojo_super_expose_dispatch_response_v1(2, 0, target, 64, meta) == 4
    assert prodex_mojo_super_expose_dispatch_response_v1(1, -1, target, 64, meta) == 1
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 9, target, 64, meta) == 1
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 8, target, 1, meta) == 3
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 8, target, -1, meta) == 1
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 8, 0, 64, meta) == 1
    assert prodex_mojo_super_expose_dispatch_response_v1(1, 8, target, 64, meta + 1) == 1
    for index in range(64):
        assert output[unsafe_offset=index] == 73
    for index in range(5):
        assert metadata[unsafe_offset=index] == 83
    dealloc(os^)
    dealloc(ms^)
