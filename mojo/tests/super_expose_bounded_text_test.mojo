from std.memory.alloc import alloc, dealloc, Layout
from super_expose_exec import prodex_mojo_super_expose_exec_validation_v1


def main() raises:
    var output_store = alloc(Layout[Int64](count=2))
    var bytes_store = alloc(Layout[UInt8](count=2))
    var output = output_store.unsafe_ptr()
    var data = bytes_store.unsafe_ptr()
    var o = UInt(Int(output))
    var d = UInt(Int(data))
    var value = StringSlice("é")
    var v = UInt(Int(value.unsafe_ptr()))
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,v,2,0,0,1,o) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=1] == 0
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,v,2,0,0,2,o) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,v,2,0,0,18446744073709551615,o) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,0,0,0,0,256,o) == 0
    assert output[unsafe_offset=0] == 2
    data[unsafe_offset=0] = 0
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,d,1,0,0,256,o) == 0
    assert output[unsafe_offset=0] == 2
    data[unsafe_offset=0] = 0xC2
    data[unsafe_offset=1] = 0x85
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,d,2,0,0,256,o) == 0
    assert output[unsafe_offset=0] == 2
    data[unsafe_offset=0] = 255
    output[unsafe_offset=0] = 71
    output[unsafe_offset=1] = 71
    assert prodex_mojo_super_expose_exec_validation_v1(1,9,d,1,0,0,256,o) == 1
    assert prodex_mojo_super_expose_exec_validation_v1(2,9,v,2,0,0,256,o) == 4
    assert prodex_mojo_super_expose_exec_validation_v1(1,99,v,2,0,0,256,o) == 1
    assert output[unsafe_offset=0] == 71 and output[unsafe_offset=1] == 71
    dealloc(bytes_store^)
    dealloc(output_store^)
