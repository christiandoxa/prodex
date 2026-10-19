# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_decimal_test.mojo`.
from info_decimal import info_decimal_one


def check(bits: UInt64, expected: StringSlice):
    var text = info_decimal_one(bits)
    assert text.length == Int(expected.byte_length())
    var source = expected.unsafe_ptr()
    for index in range(text.length):
        assert text.bytes[index] == source[unsafe_offset=index]


def main():
    check(0x3ff4000000000000, StringSlice("1.2"))
    check(0xbff4000000000000, StringSlice("-1.2"))
    check(0x3ff2666666666666, StringSlice("1.1"))
    check(0x8000000000000000, StringSlice("-0.0"))
    check(1, StringSlice("0.0"))
    check(0x7ff0000000000000, StringSlice("inf"))
    check(0xfff0000000000000, StringSlice("-inf"))
    check(0xfff8000000000001, StringSlice("NaN"))
