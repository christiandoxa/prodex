# Direct ABI validation: Mojo owns SSE data framing and CRLF normalization.
# Run with mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_usage_sse_scan_test.mojo.
from std.memory.alloc import alloc, dealloc, Layout
from provider_usage import prodex_provider_usage_sse_scan_v1


def main() raises:
    var input = StringSlice("data: first\r\n\r\ndata: [DONE]\n\n")
    var records_storage = alloc(Layout[Int64](count=12))
    var count_storage = alloc(Layout[Int64](count=1))
    var output = records_storage.unsafe_ptr()
    var count = count_storage.unsafe_ptr()
    var source = UInt(Int(input.unsafe_ptr()))
    var result = UInt(Int(output))
    var written = UInt(Int(count))
    count[unsafe_offset=0] = -1
    assert prodex_provider_usage_sse_scan_v1(
        1, source, Int64(input.byte_length()), 0, 0, 1, written,
    ) == 0
    assert count[unsafe_offset=0] == 4

    for index in range(12):
        output[unsafe_offset=index] = 77
    assert prodex_provider_usage_sse_scan_v1(
        1, source, Int64(input.byte_length()), result, 3, 0, written,
    ) == 3
    assert count[unsafe_offset=0] == 4
    for index in range(12):
        assert output[unsafe_offset=index] == 77

    assert prodex_provider_usage_sse_scan_v1(
        1, source, Int64(input.byte_length()), result, 4, 0, written,
    ) == 0
    assert count[unsafe_offset=0] == 4
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 6
    assert output[unsafe_offset=2] == 11
    assert output[unsafe_offset=3] == 2
    assert output[unsafe_offset=4] == 0
    assert output[unsafe_offset=5] == 0
    assert output[unsafe_offset=6] == 1
    assert output[unsafe_offset=7] == 21
    assert output[unsafe_offset=8] == 27
    assert output[unsafe_offset=9] == 2
    assert output[unsafe_offset=10] == 0
    assert output[unsafe_offset=11] == 0

    count[unsafe_offset=0] = 91
    assert prodex_provider_usage_sse_scan_v1(
        2, source, Int64(input.byte_length()), 0, 0, 1, written,
    ) == 4
    assert prodex_provider_usage_sse_scan_v1(
        1, source, Int64(input.byte_length()), 0, 0, 2, written,
    ) == 1
    assert count[unsafe_offset=0] == 91
    dealloc(count_storage^)
    dealloc(records_storage^)
