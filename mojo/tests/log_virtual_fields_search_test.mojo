# Direct versioned KMP search over nine borrowed virtual log fields.
from std.memory.alloc import alloc, dealloc, Layout
from log_semantics import prodex_mojo_log_virtual_fields_contains_v1


def main() raises:
    var main = StringSlice("Main")
    var empty = StringSlice("")
    var transport = StringSlice("websocket")
    var payload = StringSlice("🧭 alpha BABABAC")
    var fields = [
        main, empty, transport, empty, empty, empty, empty, empty, payload
    ]
    var views_store = alloc(Layout[UInt64](count=18))
    var scratch_store = alloc(Layout[Int64](count=64))
    var result_store = alloc(Layout[Int64](count=1))
    var views = views_store.unsafe_ptr()
    var scratch = scratch_store.unsafe_ptr()
    var result = result_store.unsafe_ptr()
    for index in range(9):
        views[unsafe_offset=index * 2] = UInt64(UInt(Int(fields[index].unsafe_ptr())))
        views[unsafe_offset=index * 2 + 1] = UInt64(fields[index].byte_length())
    var v = UInt(Int(views))
    var k = UInt(Int(scratch))
    var answer = UInt(Int(result))
    var combined = StringSlice("MAIN  WEBSOCKET")
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 9, UInt(Int(combined.unsafe_ptr())),
        Int64(combined.byte_length()), k, 64, answer
    ) == 0
    assert result[unsafe_offset=0] == 1
    var repeating = StringSlice("ababac")
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 9, UInt(Int(repeating.unsafe_ptr())),
        Int64(repeating.byte_length()), k, 64, answer
    ) == 0
    assert result[unsafe_offset=0] == 1
    var missing = StringSlice("MAIN WEBSOCKET")
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 9, UInt(Int(missing.unsafe_ptr())),
        Int64(missing.byte_length()), k, 64, answer
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 9, 0, 0, 0, 0, answer
    ) == 0
    assert result[unsafe_offset=0] == 1

    result[unsafe_offset=0] = 77
    assert prodex_mojo_log_virtual_fields_contains_v1(
        2, v, 9, 0, 0, 0, 0, answer
    ) == 4
    # Three-field search retains exact join boundaries.
    var three_fields = StringSlice("MAIN  WEBSOCKET")
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 3, UInt(Int(three_fields.unsafe_ptr())),
        Int64(three_fields.byte_length()), k, 64, answer
    ) == 0
    assert result[unsafe_offset=0] == 1
    result[unsafe_offset=0] = 77
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 0, 0, 0, 0, 0, answer
    ) == 1
    views[unsafe_offset=1] = UInt64(1_048_577)
    assert prodex_mojo_log_virtual_fields_contains_v1(
        1, v, 9, 0, 0, 0, 0, answer
    ) == 1
    assert result[unsafe_offset=0] == 77
    dealloc(result_store^)
    dealloc(scratch_store^)
    dealloc(views_store^)
