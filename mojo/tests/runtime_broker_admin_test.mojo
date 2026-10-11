# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_broker_admin_test.mojo`.
from std.collections.list import List

from runtime_broker_admin import (
    prodex_runtime_broker_admin_method_v1,
    prodex_runtime_broker_admin_value_v1,
)


def test_admin_methods() raises:
    var post = StringSlice("POST")
    var get = StringSlice("GET")
    var output = List[Int64]()
    output.append(-1)

    assert (
        prodex_runtime_broker_admin_method_v1(
            1,
            0,
            UInt(Int(post.unsafe_ptr())),
            Int64(post.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 1

    assert (
        prodex_runtime_broker_admin_method_v1(
            1,
            1,
            UInt(Int(post.unsafe_ptr())),
            Int64(post.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 1

    assert (
        prodex_runtime_broker_admin_method_v1(
            1,
            0,
            UInt(Int(get.unsafe_ptr())),
            Int64(get.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 0

    assert (
        prodex_runtime_broker_admin_method_v1(
            2,
            0,
            UInt(Int(post.unsafe_ptr())),
            Int64(post.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == -1
    )


def test_admin_values_trim_and_bound_bytes() raises:
    var unicode_value = StringSlice("\u2003work\u2003")
    var output = List[Int64]()
    for _ in range(3):
        output.append(-1)
    assert (
        prodex_runtime_broker_admin_value_v1(
            1,
            0,
            UInt(Int(unicode_value.unsafe_ptr())),
            Int64(unicode_value.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 1
    assert output[1] == 3
    assert output[2] == 7

    var empty = StringSlice(" \t")
    assert (
        prodex_runtime_broker_admin_value_v1(
            1,
            1,
            UInt(Int(empty.unsafe_ptr())),
            Int64(empty.byte_length()),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 0

    var exact = List[UInt8]()
    for _ in range(256):
        exact.append(UInt8(97))
    assert (
        prodex_runtime_broker_admin_value_v1(
            1,
            0,
            UInt(Int(exact.unsafe_ptr())),
            Int64(len(exact)),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 1
    assert output[1] == 0
    assert output[2] == 256

    exact.append(UInt8(97))
    assert (
        prodex_runtime_broker_admin_value_v1(
            1,
            0,
            UInt(Int(exact.unsafe_ptr())),
            Int64(len(exact)),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 1
    assert output[2] == 257

    assert (
        prodex_runtime_broker_admin_value_v1(
            1,
            1,
            UInt(Int(exact.unsafe_ptr())),
            Int64(len(exact)),
            UInt(Int(output.unsafe_ptr())),
        )
        == 0
    )
    assert output[0] == 0


def main() raises:
    test_admin_methods()
    test_admin_values_trim_and_bound_bytes()
