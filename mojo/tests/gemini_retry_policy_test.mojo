# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_retry_policy_test.mojo`.
from std.collections.list import List

from provider_constraints import prodex_gemini_retry_policy_v1


def call_policy(
    operation: Int64,
    retry_index: Int64,
    header_present: Int64,
    header_ms: UInt64,
    body_present: Int64,
    body_ms: UInt64,
    delay_ms: UInt64,
    output: List[UInt64],
) -> Int64:
    return prodex_gemini_retry_policy_v1(
        1,
        operation,
        retry_index,
        header_present,
        header_ms,
        body_present,
        body_ms,
        delay_ms,
        UInt(Int(output.unsafe_ptr())),
    )


def test_delay_schedule_selection_and_bounds() raises:
    var output = List[UInt64]()
    output.append(0)
    assert call_policy(0, 0, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 5_000
    assert call_policy(0, 1, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 10_000
    assert call_policy(0, 8, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 30_000

    assert call_policy(0, 0, 1, 3_000, 1, 7_000, 0, output) == 0
    assert output[0] == 7_000
    assert call_policy(0, 0, 1, 0, 1, 0, 0, output) == 0
    assert output[0] == 5_000
    assert (
        call_policy(
            0,
            0,
            1,
            UInt64(0xFFFFFFFFFFFFFFFF),
            0,
            0,
            0,
            output,
        )
        == 0
    )
    assert output[0] == 300_000

    # The retry window is an explicit Mojo outcome, so Rust cannot recompute it.
    assert call_policy(0, 9, 0, 0, 0, 0, 0, output) == 2


def test_inline_and_invalid_stream_policy_boundaries() raises:
    var output = List[UInt64]()
    output.append(0)
    assert call_policy(1, 0, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 0
    assert call_policy(1, 0, 0, 0, 0, 0, 30_000, output) == 0
    assert output[0] == 1
    assert call_policy(1, 0, 0, 0, 0, 0, 30_001, output) == 0
    assert output[0] == 0

    assert call_policy(2, 0, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 1_000
    assert call_policy(2, 3, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 8_000
    assert call_policy(2, 99, 0, 0, 0, 0, 0, output) == 0
    assert output[0] == 256_000


def test_policy_rejects_bad_abi_and_inputs() raises:
    var output = List[UInt64]()
    output.append(0)
    assert (
        prodex_gemini_retry_policy_v1(
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            UInt(Int(output.unsafe_ptr())),
        )
        == 4
    )
    assert call_policy(99, 0, 0, 0, 0, 0, 0, output) == 1
    assert call_policy(0, 0, 2, 0, 0, 0, 0, output) == 1
    assert (
        prodex_gemini_retry_policy_v1(1, 0, 0, 0, 0, 0, 0, 0, UInt(0))
        == 1
    )


def main() raises:
    test_delay_schedule_selection_and_bounds()
    test_inline_and_invalid_stream_policy_boundaries()
    test_policy_rejects_bad_abi_and_inputs()
