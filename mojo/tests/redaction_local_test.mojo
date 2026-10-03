# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_local_test.mojo`.
from std.collections.list import List
from std.memory import Pointer

from redaction_local import prodex_redaction_local_inspection_v1


def test_local_redaction_matches_masking_and_ordered_ranges() raises:
    var input: String = (
        "héllo user@example.test | Bearer token-123 | sk-proj-1234567890 | card"
        " 4111-1111-1111-1111"
    )
    var expected: String = (
        "héllo <redacted> | Bearer <redacted> | <redacted> | card <redacted>"
    )
    var output = List[UInt8]()
    for _ in range(Int(input.byte_length()) + 9 * 8 + 32):
        output.append(0)
    var matches = List[Int64]()
    for _ in range(8 * 3):
        matches.append(-1)
    var match_count: Int64 = 0
    var written: Int64 = 0
    var match_count_pointer = Pointer(to=match_count)
    var written_pointer = Pointer(to=written)

    var status = prodex_redaction_local_inspection_v1(
        1,
        UInt(Int(input.unsafe_ptr())),
        Int64(input.byte_length()),
        -1,
        UInt(Int(output.unsafe_ptr())),
        Int64(len(output)),
        UInt(Int(matches.unsafe_ptr())),
        8,
        UInt(Int(match_count_pointer)),
        UInt(Int(written_pointer)),
    )
    assert status == 0
    assert match_count == 4
    assert written == Int64(expected.byte_length())
    var expected_pointer = expected.unsafe_ptr()
    for index in range(written):
        assert output[index] == expected_pointer[unsafe_offset=index]

    assert matches[0] == 7
    assert matches[1] == 24
    assert matches[2] == 0
    assert matches[3] == 34
    assert matches[4] == 43
    assert matches[5] == 7
    assert matches[6] == 46
    assert matches[7] == 64
    assert matches[8] == 8
    assert matches[9] == 72
    assert matches[10] == 91
    assert matches[11] == 5


def main() raises:
    test_local_redaction_matches_masking_and_ordered_ranges()
