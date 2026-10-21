from std.memory.alloc import alloc, dealloc, Layout
from codex_auth_headers import prodex_codex_auth_terminal_token_v1
from rich_types import ProdexRichStringView


def check_token(
    kind: Int64, program_present: Int64, program: StringSlice,
    version: StringSlice, term: StringSlice, expected: StringSlice,
) raises:
    var fields_store = alloc(Layout[ProdexRichStringView](count=3))
    var fields = fields_store.unsafe_ptr()
    fields[unsafe_offset=0] = ProdexRichStringView(
        UInt(Int(program.unsafe_ptr())), UInt(program.byte_length())
    )
    fields[unsafe_offset=1] = ProdexRichStringView(
        UInt(Int(version.unsafe_ptr())), UInt(version.byte_length())
    )
    fields[unsafe_offset=2] = ProdexRichStringView(
        UInt(Int(term.unsafe_ptr())), UInt(term.byte_length())
    )
    var output_store = alloc(Layout[UInt8](count=64))
    var written_store = alloc(Layout[Int64](count=1))
    var output = output_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    assert prodex_codex_auth_terminal_token_v1(
        1, kind, program_present, UInt(Int(fields)), UInt(Int(output)),
        64, UInt(Int(written))
    ) == 0
    assert written[] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    output[] = 77
    written[] = 91
    assert prodex_codex_auth_terminal_token_v1(
        2, kind, program_present, UInt(Int(fields)), UInt(Int(output)),
        64, UInt(Int(written))
    ) == 4
    assert output[] == 77 and written[] == 91
    assert prodex_codex_auth_terminal_token_v1(
        1, 14, program_present, UInt(Int(fields)), UInt(Int(output)),
        64, UInt(Int(written))
    ) == 1
    assert output[] == 77 and written[] == 91
    dealloc(written_store^)
    dealloc(output_store^)
    dealloc(fields_store^)


def main() raises:
    check_token(0, 0, "", "1.2", "", "Apple_Terminal/1.2")
    check_token(1, 0, "", "1.2", "", "Ghostty/1.2")
    check_token(2, 0, "", "1.2", "", "iTerm.app/1.2")
    check_token(3, 0, "", "1.2", "", "WarpTerminal/1.2")
    check_token(4, 0, "", "1.2", "", "vscode/1.2")
    check_token(5, 0, "", "1.2", "", "WezTerm/1.2")
    check_token(6, 0, "", "ignored", "", "kitty")
    check_token(7, 0, "", "ignored", "", "Alacritty")
    check_token(8, 0, "", "1.2", "", "Konsole/1.2")
    check_token(9, 0, "", "ignored", "", "gnome-terminal")
    check_token(10, 0, "", "1.2", "", "VTE/1.2")
    check_token(11, 0, "", "ignored", "", "WindowsTerminal")
    check_token(12, 0, "", "ignored", "", "dumb")
    check_token(13, 0, "", "ignored", "", "unknown")
    check_token(13, 1, "", "", "term", "")
    check_token(13, 1, "", "2", "term", "/2")
    check_token(13, 1, "Program", "2", "term", "Program/2")
    check_token(13, 0, "", "2", "xterm-256color", "xterm-256color")
    check_token(13, 1, "AZaz09-_./", "", "", "AZaz09-_./")
    check_token(13, 1, "a🦀é́\n\0z", "β 1", "", "a_____z/__1")
