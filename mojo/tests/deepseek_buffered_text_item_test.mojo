# Shared compiled text-item writer is used by stream and buffered responses.
from std.memory.alloc import alloc, dealloc, Layout
from std.memory import Pointer
from deepseek import DeepSeekResponseWriter, deepseek_write_output_text_item
from rich_types import ProdexRichStringView


def main() raises:
    var output_storage = alloc(Layout[UInt8](count=256))
    var buffer = output_storage.unsafe_ptr()
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(UInt(Int(buffer)))
    )
    var text = StringSlice('雪\n"quoted"')
    var view = ProdexRichStringView(
        UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()),
    )
    var writer = DeepSeekResponseWriter(output, 256, 0, False)
    assert deepseek_write_output_text_item(Pointer(to=writer), view)
    var expected = StringSlice(
        '{"type":"message","role":"assistant","content":[{"type":"output_text","text":"雪\\n\\"quoted\\""}]}'
    )
    assert writer.written == Int64(expected.byte_length())
    for index in range(Int(expected.byte_length())):
        assert buffer[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]

    var small = DeepSeekResponseWriter(output, 8, 0, False)
    assert not deepseek_write_output_text_item(Pointer(to=small), view)
    assert small.written <= 8
    dealloc(output_storage^)
