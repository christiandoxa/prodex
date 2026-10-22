from std.memory.alloc import alloc, dealloc, Layout
from std.sys.info import size_of
from parsed_json import pj_literal
from runtime_doctor_bindings import (
    DoctorBindingDisplayInput, DoctorBindingDisplaySource,
    prodex_mojo_runtime_doctor_binding_display_v1,
)


def main() raises:
    assert size_of[DoctorBindingDisplaySource]() == 88
    assert size_of[DoctorBindingDisplayInput]() == 392
    var empty = DoctorBindingDisplaySource(0, 0, 0, 0, 0, 0, 0, 0, pj_literal(""), 0)
    var sources = Array[DoctorBindingDisplaySource, 4](fill=empty)
    sources[0] = DoctorBindingDisplaySource(1, 2, 3, 4, 10, 2, 7, 1, pj_literal("東京"), 10)
    var input_storage = alloc(Layout[DoctorBindingDisplayInput](count=1))
    var input = input_storage.unsafe_ptr()
    input[] = DoctorBindingDisplayInput(1, pj_literal(""), 2, 1, sources^)
    var output_storage = alloc(Layout[UInt8](count=2048))
    var output = output_storage.unsafe_ptr()
    var result_storage = alloc(Layout[Int64](count=1))
    var written = result_storage.unsafe_ptr()
    var address = UInt(Int(input))
    var target = UInt(Int(output))
    var result = UInt(Int(written))
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address, target, 2048, result) == 0
    var expected = StringSlice("active= profiles=2 selected=1 | state r=1 s=2 t=3 sid=4 total=10 profiles=2 top=東京=10 | runtime r=0 s=0 t=0 sid=0 total=0 profiles=0 top=- | journal r=0 s=0 t=0 sid=0 total=0 profiles=0 top=- | merged r=0 s=0 t=0 sid=0 total=0 profiles=0 top=- | missing=7")
    assert written[] == Int64(expected.byte_length())
    for index in range(written[]):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    for index in range(2048):
        output[unsafe_offset=index] = 73
    written[] = 83
    assert prodex_mojo_runtime_doctor_binding_display_v1(2, address, target, 2048, result) == 4
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address + 1, target, 2048, result) == 1
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address, target, -1, result) == 1
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address, target, 1, result) == 3
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address, 0, 2048, result) == 1
    input[].active_present = 2
    assert prodex_mojo_runtime_doctor_binding_display_v1(1, address, target, 2048, result) == 1
    for index in range(2048):
        assert output[unsafe_offset=index] == 73
    assert written[] == 83
    dealloc(input_storage^)
    dealloc(output_storage^)
    dealloc(result_storage^)
