from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from session_tui_presentation import SessionTuiRow, prodex_session_tui_rows_v1


def check(output: Pointer[mut=True, UInt8, _], length: Int64, expected: StringSlice):
    assert length == Int64(expected.byte_length())
    for index in range(length):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var row_storage = alloc(Layout[SessionTuiRow](count=2))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var rows = row_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    rows[] = SessionTuiRow(0, pj_literal(StringSlice("session-雪")), empty.copy(), empty.copy(), empty.copy(), empty.copy(), empty.copy())
    var input = UInt64(Int(rows))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_session_tui_rows_v1(1, 0, input, 1, 0, target, 4096, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice('[[[["hint_bold","session-雪"],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw","-"],["secondary"," profile "],["raw","-"],["secondary"," provider "],["raw","-"]],[["secondary","cwd "],["raw","-"]],[]]]'))
    assert prodex_session_tui_rows_v1(1, 1, input, 1, 0, target, 4096, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice('[[[["hint_bold","session-雪"],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw","-"],["secondary","  profile "],["raw","-"],["secondary","  provider "],["raw","-"]],[]]]'))
    rows[] = SessionTuiRow(31, pj_literal(StringSlice("session-雪")), pj_literal(StringSlice(" \n雪\t ")), empty.copy(), pj_literal(StringSlice("/home/test-user/project")), empty.copy(), pj_literal(StringSlice("provider\"")))
    assert prodex_session_tui_rows_v1(1, 1, input, 1, 0, target, 4096, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice('[[[["hint_bold","session-雪"],["raw","  "],["primary"," \\n雪\\t "]],[["secondary","updated "],["raw",""],["secondary","  profile "],["raw",""],["secondary","  provider "],["raw","provider\\\""]],[["secondary","cwd "],["raw","/home/test-user/project"]],[]]]'))
    rows[].title = pj_literal(StringSlice("\u0085\u00a0\u3000"))
    rows[].id = empty.copy()
    assert prodex_session_tui_rows_v1(1, 0, input, 1, 0, target, 4096, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice('[[[["hint_bold",""],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw",""],["secondary"," profile "],["raw",""],["secondary"," provider "],["raw","provider\\\""]],[["secondary","cwd "],["raw","/home/test-user/project"]],[]]]'))
    assert prodex_session_tui_rows_v1(1, 0, 0, 0, 0, target, 4096, meta) == 0
    check(output, metadata[unsafe_offset=1], StringSlice("[]"))
    rows[unsafe_offset=1] = rows[].copy()
    assert prodex_session_tui_rows_v1(1, 0, input, 2, 1, 0, 0, meta) == 0
    var required = metadata[unsafe_offset=1]
    assert prodex_session_tui_rows_v1(1, 0, input, 2, 0, target, required, meta) == 0
    assert metadata[unsafe_offset=1] == required
    for index in range(4096):
        output[unsafe_offset=index] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_session_tui_rows_v1(1, 0, input, 2, 0, target, required - 1, meta) == 3
    assert prodex_session_tui_rows_v1(2, 0, input, 2, 0, target, 4096, meta) == 4
    assert prodex_session_tui_rows_v1(1, 2, input, 2, 0, target, 4096, meta) == 1
    assert prodex_session_tui_rows_v1(1, 0, input + 1, 2, 0, target, 4096, meta) == 1
    rows[].presence = 32
    assert prodex_session_tui_rows_v1(1, 0, input, 2, 0, target, 4096, meta) == 1
    for index in range(4096):
        assert output[unsafe_offset=index] == 93
    assert metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(row_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
