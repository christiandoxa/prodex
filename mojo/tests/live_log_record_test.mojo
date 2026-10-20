# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/live_log_record_test.mojo`.
from std.collections.list import List

from live_log_record import (
    LIVE_LOG_RECORD_CAPACITY,
    LIVE_LOG_RECORD_INVALID,
    LIVE_LOG_RECORD_OK,
    prodex_live_log_json_plan_v1,
    prodex_live_log_plain_text_truncate_v1,
    prodex_live_log_record_over_limit_v1,
    prodex_live_log_snapshot_plan_v1,
    prodex_live_log_string_clip_end_v1,
    prodex_live_log_retention_trim_v1,
    prodex_live_log_oldest_path_v1,
)

comptime EXPECTED_LIVE_LOG_RECORD_MAX_BYTES: Int64 = 128 * 1024


def test_record_limit_and_serialized_json_fallback() raises:
    var over_limit = List[Int64]()
    over_limit.append(-1)
    assert (
        prodex_live_log_record_over_limit_v1(
            1,
            EXPECTED_LIVE_LOG_RECORD_MAX_BYTES,
            UInt(Int(over_limit.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert over_limit[0] == 0
    assert (
        prodex_live_log_record_over_limit_v1(
            1,
            EXPECTED_LIVE_LOG_RECORD_MAX_BYTES + 1,
            UInt(Int(over_limit.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert over_limit[0] == 1

    var json_plan = List[Int64]()
    for _ in range(4):
        json_plan.append(-1)
    assert (
        prodex_live_log_json_plan_v1(
            1,
            EXPECTED_LIVE_LOG_RECORD_MAX_BYTES,
            UInt(Int(json_plan.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert json_plan[0] == 0
    assert json_plan[1] == 0
    assert json_plan[2] == 0
    assert json_plan[3] == 0
    assert (
        prodex_live_log_json_plan_v1(
            1,
            EXPECTED_LIVE_LOG_RECORD_MAX_BYTES + 1,
            UInt(Int(json_plan.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert json_plan[0] == 1
    assert json_plan[1] == 1
    assert json_plan[2] == 1
    assert json_plan[3] == 1


def test_nested_string_clip_keeps_ascii_and_unicode_character_boundaries() raises:
    var ascii = List[UInt8]()
    for _ in range(8 * 1024 + 1):
        ascii.append(UInt8(97))
    var end = List[Int64]()
    end.append(-1)
    assert (
        prodex_live_log_string_clip_end_v1(
            1,
            UInt(Int(ascii.unsafe_ptr())),
            Int64(len(ascii)),
            UInt(Int(end.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert end[0] == 8 * 1024

    var unicode = List[UInt8]()
    for _ in range(8 * 1024 - 1):
        unicode.append(UInt8(117))
    unicode.append(UInt8(195))
    unicode.append(UInt8(169))
    unicode.append(UInt8(120))
    end[0] = -1
    assert (
        prodex_live_log_string_clip_end_v1(
            1,
            UInt(Int(unicode.unsafe_ptr())),
            Int64(len(unicode)),
            UInt(Int(end.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert end[0] == 8 * 1024 + 1


def test_plain_text_truncation_writes_exact_tail_and_checks_capacity() raises:
    var input = List[UInt8]()
    for _ in range(Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES) + 1):
        input.append(UInt8(110))
    var output = List[UInt8]()
    for _ in range(Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES)):
        output.append(0)
    var written = List[Int64]()
    written.append(-1)

    assert (
        prodex_live_log_plain_text_truncate_v1(
            1,
            UInt(Int(input.unsafe_ptr())),
            Int64(len(input)),
            UInt(Int(output.unsafe_ptr())),
            Int64(len(output)),
            UInt(Int(written.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    var prefix_length = Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES - 32)
    var tail = StringSlice(" …[truncated]\n")
    var tail_pointer = tail.unsafe_ptr()
    assert written[0] == Int64(prefix_length) + Int64(tail.byte_length())
    for index in range(prefix_length):
        assert output[index] == UInt8(110)
    for index in range(Int(tail.byte_length())):
        assert (
            output[prefix_length + index] == tail_pointer[unsafe_offset=index]
        )

    assert (
        prodex_live_log_plain_text_truncate_v1(
            1,
            UInt(Int(input.unsafe_ptr())),
            Int64(len(input)),
            UInt(Int(output.unsafe_ptr())),
            1,
            UInt(Int(written.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_CAPACITY
    )


def test_plain_text_truncation_keeps_unicode_character_crossing_prefix_limit() raises:
    var input = List[UInt8]()
    for _ in range(Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES - 33)):
        input.append(UInt8(97))
    input.append(UInt8(195))
    input.append(UInt8(169))
    for _ in range(64):
        input.append(UInt8(120))
    var output = List[UInt8]()
    for _ in range(Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES)):
        output.append(0)
    var written = List[Int64]()
    written.append(-1)

    assert (
        prodex_live_log_plain_text_truncate_v1(
            1,
            UInt(Int(input.unsafe_ptr())),
            Int64(len(input)),
            UInt(Int(output.unsafe_ptr())),
            Int64(len(output)),
            UInt(Int(written.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    var prefix_length = Int(EXPECTED_LIVE_LOG_RECORD_MAX_BYTES - 31)
    var tail = StringSlice(" …[truncated]\n")
    var tail_pointer = tail.unsafe_ptr()
    assert output[prefix_length - 2] == UInt8(195)
    assert output[prefix_length - 1] == UInt8(169)
    for index in range(Int(tail.byte_length())):
        assert (
            output[prefix_length + index] == tail_pointer[unsafe_offset=index]
        )
    assert written[0] == Int64(prefix_length) + Int64(tail.byte_length())


def test_snapshot_plan_pages_after_cursor_and_rejects_unsorted_input() raises:
    var sequences = List[UInt64]()
    sequences.append(7)
    sequences.append(9)
    sequences.append(11)
    var output = List[UInt64]()
    for _ in range(4):
        output.append(UInt64(0xFFFFFFFFFFFFFFFF))
    assert (
        prodex_live_log_snapshot_plan_v1(
            1,
            UInt(Int(sequences.unsafe_ptr())),
            3,
            7,
            99,
            2,
            20,
            3,
            UInt(Int(output.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_OK
    )
    assert output[0] == 1
    assert output[1] == 2
    assert output[2] == 11
    assert output[3] == 3

    sequences[1] = 6
    assert (
        prodex_live_log_snapshot_plan_v1(
            1,
            UInt(Int(sequences.unsafe_ptr())),
            3,
            0,
            1,
            2,
            20,
            3,
            UInt(Int(output.unsafe_ptr())),
        )
        == LIVE_LOG_RECORD_INVALID
    )


def test_retention_thresholds_and_stable_oldest_path() raises:
    var decision = List[Int64]()
    decision.append(-7)
    var result = UInt(Int(decision.unsafe_ptr()))
    assert prodex_live_log_retention_trim_v1(1, 0, 512, 2 * 1024 * 1024, 0, result) == 0
    assert decision[0] == 0
    assert prodex_live_log_retention_trim_v1(1, 0, 513, 0, 0, result) == 0
    assert decision[0] == 1
    assert prodex_live_log_retention_trim_v1(1, 1, 2048, 8 * 1024 * 1024, 64, result) == 0
    assert decision[0] == 0
    assert prodex_live_log_retention_trim_v1(1, 1, 2048, 8 * 1024 * 1024, 65, result) == 0
    assert decision[0] == 1
    decision[0] = 71
    assert prodex_live_log_retention_trim_v1(2, 0, 512, 0, 0, result) == 4
    assert decision[0] == 71
    assert prodex_live_log_retention_trim_v1(1, 2, 512, 0, 0, result) == 1
    assert decision[0] == 71

    var values = List[UInt64]()
    values.append(9)
    values.append(7)
    values.append(7)
    values.append(12)
    assert prodex_live_log_oldest_path_v1(
        1, UInt(Int(values.unsafe_ptr())), 4, result,
    ) == 0
    assert decision[0] == 1
    decision[0] = 83
    assert prodex_live_log_oldest_path_v1(
        1, UInt(Int(values.unsafe_ptr())), 0, result,
    ) == 1
    assert decision[0] == 83


def main() raises:
    test_record_limit_and_serialized_json_fallback()
    test_nested_string_clip_keeps_ascii_and_unicode_character_boundaries()
    test_plain_text_truncation_writes_exact_tail_and_checks_capacity()
    test_plain_text_truncation_keeps_unicode_character_crossing_prefix_limit()
    test_snapshot_plan_pages_after_cursor_and_rejects_unsorted_input()
    test_retention_thresholds_and_stable_oldest_path()
