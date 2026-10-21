# Compiled Codex TERM_PROGRAM aliases and Unicode tmux token spans.
from std.memory.alloc import alloc, dealloc, Layout
from codex_auth_headers import (
    prodex_codex_auth_terminal_name_v1,
    prodex_codex_auth_terminal_parts_v1,
)


def main() raises:
    var result_store = alloc(Layout[Int64](count=4))
    var result = result_store.unsafe_ptr()
    var address = UInt(Int(result))
    var apple = StringSlice(" Apple.Terminal ")
    assert prodex_codex_auth_terminal_name_v1(
        1, 0, UInt(Int(apple.unsafe_ptr())),
        Int64(apple.byte_length()), address
    ) == 0
    assert result[unsafe_offset=0] == 0
    var term = StringSlice("wezterm-mux")
    assert prodex_codex_auth_terminal_name_v1(
        1, 1, UInt(Int(term.unsafe_ptr())),
        Int64(term.byte_length()), address
    ) == 0
    assert result[unsafe_offset=0] == 5
    assert prodex_codex_auth_terminal_name_v1(
        1, 0, UInt(Int(term.unsafe_ptr())),
        Int64(term.byte_length()), address
    ) == 0
    assert result[unsafe_offset=0] == 13

    var parts = StringSlice("  Ghostty 2.0.0 extra ")
    assert prodex_codex_auth_terminal_parts_v1(
        1, UInt(Int(parts.unsafe_ptr())), Int64(parts.byte_length()), address
    ) == 0
    var bytes = parts.unsafe_ptr()
    assert bytes[unsafe_offset=result[unsafe_offset=0]] == 71
    assert bytes[unsafe_offset=result[unsafe_offset=2]] == 50
    assert result[unsafe_offset=0] < result[unsafe_offset=1]
    assert result[unsafe_offset=1] < result[unsafe_offset=2]
    assert result[unsafe_offset=2] < result[unsafe_offset=3]

    result[unsafe_offset=0] = 77
    assert prodex_codex_auth_terminal_name_v1(
        2, 0, UInt(Int(apple.unsafe_ptr())),
        Int64(apple.byte_length()), address
    ) == 4
    assert prodex_codex_auth_terminal_name_v1(
        1, 2, UInt(Int(apple.unsafe_ptr())),
        Int64(apple.byte_length()), address
    ) == 1
    assert prodex_codex_auth_terminal_parts_v1(
        1, 0, 12, address
    ) == 1
    assert result[unsafe_offset=0] == 77
    var invalid_utf8 = alloc(Layout[UInt8](count=1))
    invalid_utf8.unsafe_ptr()[] = 255
    assert prodex_codex_auth_terminal_name_v1(
        1, 0, UInt(Int(invalid_utf8.unsafe_ptr())), 1, address
    ) == 1
    assert prodex_codex_auth_terminal_parts_v1(
        1, UInt(Int(invalid_utf8.unsafe_ptr())), 1, address
    ) == 1
    assert result[unsafe_offset=0] == 77
    assert prodex_codex_auth_terminal_parts_v1(2, 0, 0, address) == 4
    assert prodex_codex_auth_terminal_parts_v1(1, 0, -1, address) == 1
    assert prodex_codex_auth_terminal_name_v1(1, 0, 0, 0, address + 1) == 1
    assert prodex_codex_auth_terminal_parts_v1(1, 0, 0, address + 1) == 1
    assert result[unsafe_offset=0] == 77
    assert prodex_codex_auth_terminal_parts_v1(1, 0, 0, address) == 0
    for index in range(4):
        assert result[unsafe_offset=index] == -1
    dealloc(invalid_utf8^)
    dealloc(result_store^)
