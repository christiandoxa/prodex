# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_broker_continuity_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout

from runtime_broker_continuity import prodex_runtime_broker_continuation_metrics_v1


def test_continuation_metrics_reduces_lifecycle_rows() raises:
    # Each row is kind, state, failures, not-found streak, then three
    # present/timestamp pairs for not-found, verified, and touched events.
    var records_allocation = alloc(Layout[Int64](count=40))
    var records = records_allocation.unsafe_ptr()
    for index, value in enumerate(
        [
            0,
            0,
            2,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            1,
            3,
            4,
            0,
            0,
            1,
            80,
            0,
            0,
            2,
            2,
            4,
            5,
            1,
            70,
            0,
            0,
            0,
            0,
            2,
            3,
            1,
            2,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    ):
        records[unsafe_offset=index] = Int64(value)

    var output_allocation = alloc(Layout[Int64](count=13))
    var output = output_allocation.unsafe_ptr()
    for index in range(13):
        output[unsafe_offset=index] = -1

    var status = prodex_runtime_broker_continuation_metrics_v1(
            1,
            UInt(Int(records)),
            4,
            100,
            10,
            UInt(Int(output)),
        )
    assert status == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 1
    assert output[unsafe_offset=4] == 2
    assert output[unsafe_offset=5] == 3
    assert output[unsafe_offset=6] == 5
    assert output[unsafe_offset=7] == 1
    assert output[unsafe_offset=8] == 4
    assert output[unsafe_offset=9] == 7
    assert output[unsafe_offset=10] == 0
    assert output[unsafe_offset=11] == 1
    assert output[unsafe_offset=12] == 0
    dealloc(output_allocation^)
    dealloc(records_allocation^)


def test_continuation_metrics_rejects_unknown_state_before_output() raises:
    var records_allocation = alloc(Layout[Int64](count=20))
    var records = records_allocation.unsafe_ptr()
    for index, value in enumerate(
        [
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            9,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    ):
        records[unsafe_offset=index] = Int64(value)

    var output_allocation = alloc(Layout[Int64](count=13))
    var output = output_allocation.unsafe_ptr()
    for index in range(13):
        output[unsafe_offset=index] = -9

    assert (
        prodex_runtime_broker_continuation_metrics_v1(
            1,
            UInt(Int(records)),
            2,
            100,
            10,
            UInt(Int(output)),
        )
        == -1
    )
    for index in range(13):
        assert output[unsafe_offset=index] == -9
    dealloc(output_allocation^)
    dealloc(records_allocation^)


def test_continuation_metrics_rejects_unbounded_state() raises:
    var output_allocation = alloc(Layout[Int64](count=13))
    var output = output_allocation.unsafe_ptr()
    for index in range(13):
        output[unsafe_offset=index] = -1
    assert (
        prodex_runtime_broker_continuation_metrics_v1(
            1,
            0,
            65537,
            100,
            10,
            UInt(Int(output)),
        )
        == -1
    )
    dealloc(output_allocation^)


def main() raises:
    test_continuation_metrics_reduces_lifecycle_rows()
    test_continuation_metrics_rejects_unknown_state_before_output()
    test_continuation_metrics_rejects_unbounded_state()
