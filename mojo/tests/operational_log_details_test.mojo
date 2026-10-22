from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from operational_log_details import OperationalDetailText, prodex_mojo_operational_log_details_v1


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def verify(fields: Pointer[mut=True, OperationalDetailText, _], output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _], expected: StringSlice):
    var title = view("Event")
    assert prodex_mojo_operational_log_details_v1(1, title.ptr, 5, UInt(Int(fields)), 4, 1, 0, 0, UInt(Int(written))) == 0
    assert written[] == Int64(expected.byte_length())
    assert prodex_mojo_operational_log_details_v1(1, title.ptr, 5, UInt(Int(fields)), 4, 0, UInt(Int(output)), 256, UInt(Int(written))) == 0
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var fs = alloc(Layout[OperationalDetailText](count=4))
    var os = alloc(Layout[UInt8](count=256))
    var ws = alloc(Layout[Int64](count=1))
    var fields = fs.unsafe_ptr()
    var output = os.unsafe_ptr()
    var written = ws.unsafe_ptr()
    fields[unsafe_offset=0] = OperationalDetailText(view("model"), view("raw"), view("-"), 0)
    fields[unsafe_offset=1] = OperationalDetailText(view("left"), view("-"), view("-"), 1)
    fields[unsafe_offset=2] = OperationalDetailText(view("path"), view("raw"), view("/responses"), 2)
    fields[unsafe_offset=3] = OperationalDetailText(view("model"), view("é"), view("é"), 0)
    verify(fields, output, written, "Event  left=-% · path=/responses · model=é")
    fields[unsafe_offset=3].original = view("a\u0085b")
    verify(fields, output, written, "Event  left=-% · path=/responses")
    fields[unsafe_offset=1].prepared = view("12345678901234567890123456789012345")
    verify(fields, output, written, "Event  left=12345678901234567890123456789012…% · path=/responses")
    output[unsafe_offset=0] = 73
    written[] = 83
    var data = UInt(Int(fields))
    var target = UInt(Int(output))
    var meta = UInt(Int(written))
    var title = view("Event")
    assert prodex_mojo_operational_log_details_v1(2, title.ptr, 5, data, 4, 0, target, 256, meta) == 4
    assert prodex_mojo_operational_log_details_v1(1, title.ptr, 5, data, 65, 0, target, 256, meta) == 1
    assert prodex_mojo_operational_log_details_v1(1, title.ptr, 5, data, 4, 0, target, 1, meta) == 2
    fields[unsafe_offset=0].format_kind = 3
    assert prodex_mojo_operational_log_details_v1(1, title.ptr, 5, data, 4, 0, target, 256, meta) == 1
    assert output[unsafe_offset=0] == 73 and written[] == 83
    dealloc(fs^)
    dealloc(os^)
    dealloc(ws^)
