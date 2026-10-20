# Direct compiled runtime SSE buffered-event state transitions.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_sse_event_state_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from sse_line import prodex_runtime_sse_event_state_v1


def main() raises:
    var result_storage = alloc(Layout[Int64](count=1))
    var marker_storage = alloc(Layout[UInt8](count=24))
    var result = result_storage.unsafe_ptr()
    var marker = marker_storage.unsafe_ptr()
    var literal = StringSlice("prodex-invalid-sse-data")
    marker[unsafe_offset=0] = 0
    for index in range(Int(literal.byte_length())):
        marker[unsafe_offset=index + 1] = literal.unsafe_ptr()[unsafe_offset=index]
    var marker_address = UInt(Int(marker))
    var result_address = UInt(Int(result))
    assert prodex_runtime_sse_event_state_v1(
        1, 0, 0, 0, 0, 1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert prodex_runtime_sse_event_state_v1(
        1, 1, 0, 0, 0, 1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 5
    assert prodex_runtime_sse_event_state_v1(
        1, 0, 1, marker_address, 24, 1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert prodex_runtime_sse_event_state_v1(
        1, 2, 1, marker_address, 24, 1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 1
    assert prodex_runtime_sse_event_state_v1(
        1, 1, 1, marker_address, 24, 1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 4
    assert prodex_runtime_sse_event_state_v1(
        1, 1, 1, marker_address, 24, 0, result_address
    ) == 0
    assert result[unsafe_offset=0] == 3

    var normal = StringSlice("valid-line")
    assert prodex_runtime_sse_event_state_v1(
        1, 0, 1, UInt(Int(normal.unsafe_ptr())), Int64(normal.byte_length()),
        1, result_address
    ) == 0
    assert result[unsafe_offset=0] == 2

    result[unsafe_offset=0] = 91
    assert prodex_runtime_sse_event_state_v1(
        2, 0, 1, marker_address, 24, 1, result_address
    ) == 4
    assert prodex_runtime_sse_event_state_v1(
        1, 3, 1, marker_address, 24, 1, result_address
    ) == 1
    assert prodex_runtime_sse_event_state_v1(
        1, 0, 1, 0, 24, 1, result_address
    ) == 1
    assert prodex_runtime_sse_event_state_v1(
        1, 0, 1, marker_address, 24, 2, result_address
    ) == 1
    assert result[unsafe_offset=0] == 91
    dealloc(marker_storage^)
    dealloc(result_storage^)
