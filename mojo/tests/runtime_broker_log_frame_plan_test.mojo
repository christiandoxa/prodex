# Compiled broker-log binary framing and line-size policy ABI regression.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_broker_log_frame_plan_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from runtime_broker_continuity import prodex_runtime_broker_log_frame_plan_v1


def main() raises:
    var result_storage = alloc(Layout[Int64](count=5))
    var output = result_storage.unsafe_ptr()
    var result_address = UInt(Int(output))
    var content = StringSlice("first\nsecond\npartial")
    var address = UInt(Int(content.unsafe_ptr()))
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, address, Int64(content.byte_length()), 0, 0, result_address,
    ) == 0
    assert output[unsafe_offset=0] == 6
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 0
    assert output[unsafe_offset=4] == 1

    assert prodex_runtime_broker_log_frame_plan_v1(
        1, address, Int64(content.byte_length()), 1_048_574, 0, result_address,
    ) == 0
    assert output[unsafe_offset=2] == 2
    assert output[unsafe_offset=3] == 1
    assert output[unsafe_offset=4] == 0
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, address, Int64(content.byte_length()), 0, 1, result_address,
    ) == 0
    assert output[unsafe_offset=2] == 0
    assert output[unsafe_offset=3] == 1
    assert output[unsafe_offset=4] == 0

    var partial = StringSlice("incomplete")
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, UInt(Int(partial.unsafe_ptr())), Int64(partial.byte_length()),
        0, 0, result_address,
    ) == 0
    assert output[unsafe_offset=0] == Int64(partial.byte_length())
    assert output[unsafe_offset=1] == 0
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=4] == 0

    for index in range(5):
        output[unsafe_offset=index] = 79
    assert prodex_runtime_broker_log_frame_plan_v1(
        2, address, Int64(content.byte_length()), 0, 0, result_address,
    ) == 4
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, 0, Int64(content.byte_length()), 0, 0, result_address,
    ) == 1
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, address, Int64(content.byte_length()), -1, 0, result_address,
    ) == 1
    assert prodex_runtime_broker_log_frame_plan_v1(
        1, address, Int64(content.byte_length()), 0, 2, result_address,
    ) == 1
    for index in range(5):
        assert output[unsafe_offset=index] == 79
    dealloc(result_storage^)
