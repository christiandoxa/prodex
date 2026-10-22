from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from json_sink import JsonSink, js_byte, js_literal, js_escaped
from session_tui_presentation import stp_literal, stp_span


def sb_emit(sink: Pointer[mut=True, JsonSink, _], operation: Int64, texts_address: UInt64, presence: UInt64):
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(texts_address))
    js_byte(sink, 91)
    if operation == 0:
        stp_literal(sink, StringSlice("title"), StringSlice(" PRODEX STATUS "))
        js_literal(sink, StringSlice(',["accent"," profile '))
        if presence & 1 != 0:
            js_escaped(sink, texts[unsafe_offset=0])
        else:
            js_literal(sink, StringSlice("loading"))
        js_literal(sink, StringSlice(' "],["secondary"," '))
        js_literal(sink, StringSlice("refreshing") if presence & 2 != 0 else StringSlice("live"))
        js_literal(sink, StringSlice(" · "))
        if presence & 1 != 0:
            js_escaped(sink, texts[unsafe_offset=1])
        else:
            js_literal(sink, StringSlice("waiting for first snapshot"))
        js_literal(sink, StringSlice(' "]'))
    elif presence == 1:
        stp_literal(sink, StringSlice("error"), StringSlice(" refresh error: "))
        js_byte(sink, 44)
        stp_span(sink, StringSlice("muted"), texts[unsafe_offset=0])
    else:
        stp_literal(sink, StringSlice("hint_bold"), StringSlice(" q/esc "))
        js_byte(sink, 44)
        stp_literal(sink, StringSlice("raw"), StringSlice("quit  "))
        js_byte(sink, 44)
        stp_literal(sink, StringSlice("hint_bold"), StringSlice("r "))
        js_byte(sink, 44)
        stp_literal(sink, StringSlice("raw"), StringSlice("refresh  "))
        js_byte(sink, 44)
        stp_literal(sink, StringSlice("muted"), StringSlice("NET shows Prodex socket queues; disk rates come from /proc/<pid>/io"))
    js_byte(sink, 93)


@export("prodex_status_bar_v1")
def prodex_status_bar_v1(
    abi: Int64, operation: Int64, texts_address: UInt64, text_count: Int64, presence: UInt64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 1 or text_count < 0 or text_count > 2
        or texts_address % 8 != 0 or (text_count > 0 and texts_address == 0)
        or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    if operation == 0:
        if presence > 3 or text_count != Int64(2 if presence & 1 != 0 else 0):
            return 1
    elif presence > 1 or text_count != Int64(presence):
        return 1
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(texts_address))
    for index in range(text_count):
        if not rich_view_valid(texts[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    sb_emit(Pointer(to=sink), operation, texts_address, presence)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        sb_emit(Pointer(to=sink), operation, texts_address, presence)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
