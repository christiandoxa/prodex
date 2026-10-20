# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/quota_source_summary_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from quota_pool import prodex_quota_status_summary_v2, prodex_quota_pool_report_overview_v1
from runtime_math import INT64_MAX


def test_compiled_quota_report_overview_preserves_latest_failed_timestamp() raises:
    var input_storage = alloc(Layout[Int64](count=6))
    var output_storage = alloc(Layout[Int64](count=4))
    var inputs = input_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    inputs[unsafe_offset=0] = -20
    inputs[unsafe_offset=1] = 1
    inputs[unsafe_offset=2] = 99
    inputs[unsafe_offset=3] = 0
    inputs[unsafe_offset=4] = 10
    inputs[unsafe_offset=5] = 1
    var source = UInt(Int(inputs))
    var destination = UInt(Int(output))
    for index in range(4):
        output[unsafe_offset=index] = 71
    assert prodex_quota_pool_report_overview_v1(2, source, 3, destination, 4) == 4
    assert prodex_quota_pool_report_overview_v1(1, source, 3, destination, 3) == 3
    for index in range(4):
        assert output[unsafe_offset=index] == 71
    assert prodex_quota_pool_report_overview_v1(1, source, 3, destination, 4) == 0
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 99
    inputs[unsafe_offset=5] = 2
    output[unsafe_offset=0] = 71
    assert prodex_quota_pool_report_overview_v1(1, source, 3, destination, 4) == 1
    assert output[unsafe_offset=0] == 71
    dealloc(input_storage^)
    dealloc(output_storage^)


def main() raises:
    var input_storage = alloc(Layout[Int64](count=45))
    var output_storage = alloc(Layout[Int64](count=12))
    var inputs = input_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(45):
        inputs[unsafe_offset=index] = 0
    for index in range(12):
        output[unsafe_offset=index] = 71
    for row in range(3):
        inputs[unsafe_offset=row * 15] = 1
        inputs[unsafe_offset=row * 15 + 2] = 1
    inputs[unsafe_offset=1] = 1
    inputs[unsafe_offset=3] = 80
    inputs[unsafe_offset=4] = 1
    inputs[unsafe_offset=5] = 20
    inputs[unsafe_offset=9] = 5
    inputs[unsafe_offset=10] = 1
    inputs[unsafe_offset=11] = 1
    inputs[unsafe_offset=27] = 70
    inputs[unsafe_offset=28] = 1
    inputs[unsafe_offset=29] = INT64_MAX
    inputs[unsafe_offset=31] = 1
    inputs[unsafe_offset=39] = 99
    inputs[unsafe_offset=40] = 1
    var source = UInt(Int(inputs))
    var destination = UInt(Int(output))
    assert prodex_quota_status_summary_v2(1, source, destination, 3, 12) == 4
    assert prodex_quota_status_summary_v2(2, source, destination, 3, 11) == 3
    for index in range(12):
        assert output[unsafe_offset=index] == 71
    assert prodex_quota_status_summary_v2(2, source, destination, 3, 12) == 0
    assert output[unsafe_offset=0] == 3
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 1
    assert output[unsafe_offset=3] == 80
    assert output[unsafe_offset=4] == 20
    assert output[unsafe_offset=6] == 1
    assert output[unsafe_offset=7] == 70
    assert output[unsafe_offset=9] == 0
    assert output[unsafe_offset=10] == 1
    assert output[unsafe_offset=11] == 1
    dealloc(input_storage^)
    dealloc(output_storage^)
    test_compiled_quota_report_overview_preserves_latest_failed_timestamp()
