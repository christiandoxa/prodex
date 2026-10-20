# Direct ABI for trimming the first partial line from a persisted log tail.
from std.memory.alloc import alloc, dealloc, Layout
from log_parser import prodex_mojo_log_tail_boundary_v1


def main() raises:
    var storage = alloc(Layout[Int64](count=1))
    var written = storage.unsafe_ptr()
    var address = UInt(Int(written))
    var sample = StringSlice("partial\r\nnext\n")
    assert prodex_mojo_log_tail_boundary_v1(
        1, UInt(Int(sample.unsafe_ptr())), Int64(sample.byte_length()), 1, address,
    ) == 0
    assert written[unsafe_offset=0] == 9
    assert prodex_mojo_log_tail_boundary_v1(
        1, UInt(Int(sample.unsafe_ptr())), Int64(sample.byte_length()), 0, address,
    ) == 0
    assert written[unsafe_offset=0] == 0
    var missing = StringSlice("no-newline")
    assert prodex_mojo_log_tail_boundary_v1(
        1, UInt(Int(missing.unsafe_ptr())), Int64(missing.byte_length()), 1, address,
    ) == 0
    assert written[unsafe_offset=0] == 0
    written[unsafe_offset=0] = 77
    assert prodex_mojo_log_tail_boundary_v1(
        2, UInt(Int(sample.unsafe_ptr())), Int64(sample.byte_length()), 1, address,
    ) == 4
    assert prodex_mojo_log_tail_boundary_v1(
        1, 0, Int64(sample.byte_length()), 1, address,
    ) == 1
    assert prodex_mojo_log_tail_boundary_v1(
        1, UInt(Int(sample.unsafe_ptr())), Int64(sample.byte_length()), 2, address,
    ) == 1
    assert written[unsafe_offset=0] == 77
    dealloc(storage^)
