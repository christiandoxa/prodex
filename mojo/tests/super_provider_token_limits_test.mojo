# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_provider_token_limits_test.mojo`.
from std.collections.list import List

from super_provider_config import prodex_external_provider_token_limits_v1


def output_pointer(output: List[UInt64]) -> UInt:
    return UInt(Int(output.unsafe_ptr()))


def test_default_and_copilot_limits() raises:
    var output = List[UInt64]()
    output.append(0)
    output.append(0)
    assert (
        prodex_external_provider_token_limits_v1(
            1, 0, 100_000, 90_000, 0, 0, 0, 0, output_pointer(output)
        )
        == 0
    )
    assert output[0] == 100_000
    assert output[1] == 90_000

    assert (
        prodex_external_provider_token_limits_v1(
            1, 1, 922_000, 900_000, 0, 0, 0, 0, output_pointer(output)
        )
        == 0
    )
    assert output[0] == 922_000
    assert output[1] == 875_900


def test_explicit_limits_and_invalid_flags() raises:
    var output = List[UInt64]()
    output.append(0)
    output.append(0)
    assert (
        prodex_external_provider_token_limits_v1(
            1, 1, 272_000, 258_400, 333_333, 1, 999_999, 1, output_pointer(output)
        )
        == 0
    )
    assert output[0] == 333_333
    assert output[1] == 333_332
    assert (
        prodex_external_provider_token_limits_v1(
            1, 2, 100_000, 90_000, 0, 0, 0, 0, output_pointer(output)
        )
        == 1
    )
    assert (
        prodex_external_provider_token_limits_v1(
            2, 0, 100_000, 90_000, 0, 0, 0, 0, output_pointer(output)
        )
        == 4
    )


def main() raises:
    test_default_and_copilot_limits()
    test_explicit_limits_and_invalid_flags()
