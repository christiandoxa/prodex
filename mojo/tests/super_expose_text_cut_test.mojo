from std.memory.alloc import alloc, dealloc, Layout
from super_expose_exec import prodex_mojo_super_expose_exec_validation_v1
from launch_config import prodex_launch_instruction_text_cut_v1


def main() raises:
    var output_store = alloc(Layout[Int64](count=2))
    var invalid_store = alloc(Layout[UInt8](count=1))
    var output = output_store.unsafe_ptr()
    var invalid = invalid_store.unsafe_ptr()
    var o = UInt(Int(output))
    var text = StringSlice("aé雪🙂z")
    var t = UInt(Int(text.unsafe_ptr()))
    var expected = [0,1,1,3,3,3,6,6,6,6,10,11,11]
    for maximum in range(13):
        assert prodex_mojo_super_expose_exec_validation_v1(1,8,t,11,0,0,UInt64(maximum),o) == 0
        assert output[unsafe_offset=0] == Int64(expected[maximum]) and output[unsafe_offset=1] == 0
        assert prodex_launch_instruction_text_cut_v1(1,t,11,Int64(maximum),o) == 0
        assert output[unsafe_offset=0] == Int64(expected[maximum])
    assert prodex_mojo_super_expose_exec_validation_v1(1,8,t,11,0,0,18446744073709551615,o) == 0
    assert output[unsafe_offset=0] == 11
    assert prodex_mojo_super_expose_exec_validation_v1(1,8,0,0,0,0,0,o) == 0
    assert output[unsafe_offset=0] == 0
    invalid[unsafe_offset=0] = 255
    output[unsafe_offset=0] = 71
    output[unsafe_offset=1] = 71
    assert prodex_mojo_super_expose_exec_validation_v1(1,8,UInt(Int(invalid)),1,0,0,1,o) == 1
    assert prodex_mojo_super_expose_exec_validation_v1(2,8,t,11,0,0,1,o) == 4
    assert prodex_mojo_super_expose_exec_validation_v1(1,99,t,11,0,0,1,o) == 1
    assert prodex_mojo_super_expose_exec_validation_v1(1,8,t,-1,0,0,1,o) == 1
    assert prodex_mojo_super_expose_exec_validation_v1(1,8,t,11,0,0,1,o+1) == 1
    assert output[unsafe_offset=0] == 71 and output[unsafe_offset=1] == 71
    dealloc(invalid_store^)
    dealloc(output_store^)
