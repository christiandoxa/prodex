# Direct ABI: ascending u32 process IDs with stable equal-PID rows.
# Run: mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_process_pid_order_test.mojo
from std.memory.alloc import alloc, dealloc, Layout
from info_render import prodex_info_process_pid_order_v1


def main() raises:
    var pids_storage = alloc(Layout[UInt64](count=5))
    var output_storage = alloc(Layout[Int64](count=5))
    var pids = pids_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    pids[unsafe_offset=0] = 42
    pids[unsafe_offset=1] = 4294967295
    pids[unsafe_offset=2] = 0
    pids[unsafe_offset=3] = 17
    pids[unsafe_offset=4] = 42
    var input = UInt(Int(pids))
    var dest = UInt(Int(output))
    assert prodex_info_process_pid_order_v1(1, input, 5, dest, 5) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 3
    assert output[unsafe_offset=2] == 0
    assert output[unsafe_offset=3] == 4
    assert output[unsafe_offset=4] == 1

    for index in range(5):
        output[unsafe_offset=index] = 71
    assert prodex_info_process_pid_order_v1(2, input, 5, dest, 5) == 4
    assert prodex_info_process_pid_order_v1(1, input, 5, dest, 4) == 1
    pids[unsafe_offset=4] = 4294967296
    assert prodex_info_process_pid_order_v1(1, input, 5, dest, 5) == 1
    for index in range(5):
        assert output[unsafe_offset=index] == 71
    assert prodex_info_process_pid_order_v1(1, 0, 0, dest, 5) == 0
    dealloc(output_storage^)
    dealloc(pids_storage^)
