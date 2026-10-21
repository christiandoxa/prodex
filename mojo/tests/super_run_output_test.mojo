from std.memory.alloc import alloc, dealloc, Layout
from super_expose_run_policy import SuperRunOutputInput, prodex_super_run_output_plan_v1, prodex_super_run_event_push_v1


def main() raises:
    var input_store = alloc(Layout[SuperRunOutputInput](count=1))
    var output_store = alloc(Layout[UInt64](count=9))
    var markers_store = alloc(Layout[UInt8](count=256))
    var input = input_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var markers = markers_store.unsafe_ptr()
    var i = UInt(Int(input))
    var o = UInt(Int(output))
    var m = UInt64(UInt(Int(markers)))
    var text = StringSlice("🦀tail")
    var t = UInt64(UInt(Int(text.unsafe_ptr())))
    for index in range(256):
        markers[unsafe_offset=index] = 0
    input[] = SuperRunOutputInput(t,8,262142,0,18446744073709551615,m,256,0,8192,262144)
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=0] == 8 and output[unsafe_offset=1] == 0 and output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 1 and output[unsafe_offset=4] == 18446744073709551615 and output[unsafe_offset=5] == 1
    assert output[unsafe_offset=6] == 1 and output[unsafe_offset=7] == 18446744073709551615 and output[unsafe_offset=8] == 1
    markers[unsafe_offset=1] = 1
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=6] == 0
    markers[unsafe_offset=1] = 0
    markers[unsafe_offset=0] = 1
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=6] == 1
    input[].event_is_marker = 1
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=6] == 0
    input[] = SuperRunOutputInput(0,0,18446744073709551615,0,3,0,0,0,8192,262144)
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=2] == 0 and output[unsafe_offset=3] == 0 and output[unsafe_offset=6] == 0
    input[] = SuperRunOutputInput(t,8,262140,0,7,m,0,0,8192,262144)
    assert prodex_super_run_output_plan_v1(1,i,o) == 0
    assert output[unsafe_offset=1] == 4 and output[unsafe_offset=2] == 1
    for index in range(9):
        output[unsafe_offset=index] = 71
    input[].event_is_marker = 2
    assert prodex_super_run_output_plan_v1(1,i,o) == 1
    input[].event_is_marker = 0
    input[].marker_count = 256
    markers[unsafe_offset=255] = 2
    assert prodex_super_run_output_plan_v1(1,i,o) == 1
    assert prodex_super_run_output_plan_v1(2,i,o) == 4
    assert prodex_super_run_output_plan_v1(1,i+1,o) == 1
    for index in range(9):
        assert output[unsafe_offset=index] == 71
    dealloc(markers_store^)
    dealloc(output_store^)
    dealloc(input_store^)
