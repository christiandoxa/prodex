# Compiled terminal User-Agent token and Unicode blank-value decisions.
from std.memory.alloc import alloc, dealloc, Layout
from codex_auth_headers import (
    prodex_codex_auth_terminal_token_ascii_v1,
    prodex_codex_auth_nonwhitespace_v1,
    prodex_codex_auth_user_agent_ascii_v1,
)


def main() raises:
    var candidate = StringSlice("A/B-. \t🌍")
    var projected = StringSlice("A/B-.___")
    var result_store = alloc(Layout[UInt8](count=32))
    var length_store = alloc(Layout[Int64](count=1))
    var result = result_store.unsafe_ptr()
    var length = length_store.unsafe_ptr()
    var dst = UInt(Int(result))
    var written = UInt(Int(length))
    var src = UInt(Int(candidate.unsafe_ptr()))
    assert prodex_codex_auth_terminal_token_ascii_v1(
        1, src, Int64(candidate.byte_length()), dst, 32, written
    ) == 0
    assert length[unsafe_offset=0] == Int64(projected.byte_length())
    for index in range(Int64(projected.byte_length())):
        assert result[unsafe_offset=index] == projected.unsafe_ptr()[unsafe_offset=index]

    var blank = StringSlice("  \t ")
    var content = StringSlice("  value ")
    assert prodex_codex_auth_nonwhitespace_v1(
        1, UInt(Int(blank.unsafe_ptr())), Int64(blank.byte_length()), written
    ) == 0
    assert length[unsafe_offset=0] == 0
    assert prodex_codex_auth_nonwhitespace_v1(
        1, UInt(Int(content.unsafe_ptr())), Int64(content.byte_length()), written
    ) == 0
    assert length[unsafe_offset=0] == 1
    assert prodex_codex_auth_nonwhitespace_v1(1, 0, 0, written) == 0
    assert length[unsafe_offset=0] == 0

    # Existing User-Agent path reuses the same byte projection, but permits
    # printable ASCII spaces in its distinct allowed alphabet.
    assert prodex_codex_auth_user_agent_ascii_v1(
        1, src, Int64(candidate.byte_length()), dst, 32, written
    ) == 0
    assert result[unsafe_offset=5] == 32

    length[unsafe_offset=0] = 77
    assert prodex_codex_auth_terminal_token_ascii_v1(
        2, src, Int64(candidate.byte_length()), dst, 32, written
    ) == 4
    assert prodex_codex_auth_terminal_token_ascii_v1(
        1, src, Int64(candidate.byte_length()), dst, 1, written
    ) == 1
    assert prodex_codex_auth_nonwhitespace_v1(2, src, Int64(candidate.byte_length()), written) == 4
    assert length[unsafe_offset=0] == 77
    dealloc(length_store^)
    dealloc(result_store^)
