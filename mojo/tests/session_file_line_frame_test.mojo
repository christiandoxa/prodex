# Direct compiled session JSONL cursor, newline and filename suffix policy.
from std.memory.alloc import alloc, dealloc, Layout
from session_selector import (
    prodex_session_file_line_frame_v1,
    prodex_session_file_compressed_suffix_v1,
)


def main() raises:
    var complete = StringSlice("a\n")
    var unfinished = StringSlice("a")
    var output_store = alloc(Layout[UInt64](count=2))
    var frame = output_store.unsafe_ptr()
    var result_address = UInt(Int(frame))
    assert prodex_session_file_line_frame_v1(
        1, 5, 5, 2, UInt(Int(complete.unsafe_ptr())),
        UInt64(complete.byte_length()), result_address
    ) == 0
    assert frame[unsafe_offset=0] == 3
    assert frame[unsafe_offset=1] == 7
    assert prodex_session_file_line_frame_v1(
        1, 5, 5, 1, UInt(Int(complete.unsafe_ptr())),
        UInt64(complete.byte_length()), result_address
    ) == 0
    assert frame[unsafe_offset=0] == 1
    assert prodex_session_file_line_frame_v1(
        1, 5, 5, 10, UInt(Int(unfinished.unsafe_ptr())),
        UInt64(unfinished.byte_length()), result_address
    ) == 0
    assert frame[unsafe_offset=0] == 2
    assert frame[unsafe_offset=1] == 5
    assert prodex_session_file_line_frame_v1(
        1, 5, 5, 10, 0, 0, result_address
    ) == 0
    assert frame[unsafe_offset=0] == 0
    assert frame[unsafe_offset=1] == 5

    var compressed = StringSlice("rollout.jsonl.zst")
    var invalid = StringSlice("rollout.jsonl.zst.backup")
    assert prodex_session_file_compressed_suffix_v1(
        1, UInt(Int(compressed.unsafe_ptr())), Int64(compressed.byte_length()),
        result_address
    ) == 0
    assert frame[unsafe_offset=0] == 1
    assert prodex_session_file_compressed_suffix_v1(
        1, UInt(Int(invalid.unsafe_ptr())), Int64(invalid.byte_length()),
        result_address
    ) == 0
    assert frame[unsafe_offset=0] == 0
    frame[unsafe_offset=0] = 77
    assert prodex_session_file_line_frame_v1(
        2, 5, 5, 10, 0, 0, result_address
    ) == 4
    assert prodex_session_file_line_frame_v1(
        1, 5, 5, 10, 0, 1, result_address
    ) == 1
    assert prodex_session_file_compressed_suffix_v1(
        2, 0, 0, result_address
    ) == 4
    assert frame[unsafe_offset=0] == 77
    dealloc(output_store^)
