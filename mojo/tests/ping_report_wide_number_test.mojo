from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from json_sink import JsonSink, js_u128


def check(value: UInt128, expected: StringSlice) raises:
    var storage = alloc(Layout[UInt8](count=80))
    var output = storage.unsafe_ptr()
    for index in range(80):
        output[unsafe_offset=index] = 93
    var target = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output))
    var sink = JsonSink(target, 80, 0, False, False)
    js_u128(Pointer(to=sink), value)
    assert not sink.failed and sink.written == Int64(expected.byte_length())
    for index in range(sink.written):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    assert output[unsafe_offset=sink.written] == 93
    dealloc(storage^)


def main() raises:
    check(0, StringSlice("0"))
    check(18_446_744_073_709_551_615, StringSlice("18446744073709551615"))
    check(18_446_744_073_709_551_616, StringSlice("18446744073709551616"))
    check(340_282_366_920_938_463_463_374_607_431_768_211_455, StringSlice("340282366920938463463374607431768211455"))
