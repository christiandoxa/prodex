from std.memory.alloc import alloc, dealloc, Layout
from super_provider_config import prodex_provider_catalog_uint_v1


def check(text: StringSlice, status: Int64, value: UInt64, cause: UInt64) raises:
    var store = alloc(Layout[UInt64](count=2))
    var output = store.unsafe_ptr()
    var o = UInt(Int(output))
    assert prodex_provider_catalog_uint_v1(1, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()), o) == status
    assert output[unsafe_offset=0] == value
    assert output[unsafe_offset=1] == cause
    output[unsafe_offset=0] = 77
    output[unsafe_offset=1] = 77
    assert prodex_provider_catalog_uint_v1(2, 0, 0, o) == 4
    assert prodex_provider_catalog_uint_v1(1, 0, 1, o) == 1
    assert prodex_provider_catalog_uint_v1(1, 0, -1, o) == 1
    assert output[unsafe_offset=0] == 77 and output[unsafe_offset=1] == 77
    dealloc(store^)


def main() raises:
    check("2", 0, 2, 0)
    check("+0002", 0, 2, 0)
    check("18446744073709551615", 0, 18446744073709551615, 0)
    check("", 11, 0, 0)
    check(" 2", 12, 0, 0)
    check("2　", 12, 0, 0)
    check("0", 14, 0, 0)
    check("+0001", 14, 0, 0)
    check("+", 13, 0, 1)
    check("-0", 13, 0, 1)
    check("é", 13, 0, 1)
    check("18446744073709551616", 13, 0, 2)
    check("18446744073709551616x", 13, 0, 2)
    check("1844674407370955161x6", 13, 0, 1)
