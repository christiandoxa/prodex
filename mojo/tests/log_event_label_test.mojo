from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from operational_log_details import prodex_mojo_log_event_label_v1


def verify(source: StringSlice, expected: StringSlice,
    output: Pointer[mut=True, UInt8, _], written: Pointer[mut=True, Int64, _]):
    var address = UInt(Int(source.unsafe_ptr()))
    var length = Int64(source.byte_length())
    var target = UInt(Int(output))
    var meta = UInt(Int(written))
    assert prodex_mojo_log_event_label_v1(1, address, length, 1, 0, 0, meta) == 0
    assert written[] == Int64(expected.byte_length())
    assert prodex_mojo_log_event_label_v1(1, address, length, 0, target, 128, meta) == 0
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var os = alloc(Layout[UInt8](count=128))
    var ws = alloc(Layout[Int64](count=1))
    var output = os.unsafe_ptr()
    var written = ws.unsafe_ptr()
    verify("request", "REQUEST", output, written)
    verify("event", "EVENT", output, written)
    verify("ws", "WEBSOCKET", output, written)
    verify("turn-context", "MODEL", output, written)
    verify("tool-call:", "TOOL CALL", output, written)
    verify("tool-call:é", "TOOL CALL", output, written)
    verify("REQUEST", "stream REQUEST", output, written)
    verify("未知", "stream 未知", output, written)
    verify("", "stream ", output, written)
    output[unsafe_offset=0] = 73
    written[] = 83
    var source = StringSlice("request")
    var address = UInt(Int(source.unsafe_ptr()))
    var target = UInt(Int(output))
    var meta = UInt(Int(written))
    assert prodex_mojo_log_event_label_v1(2, address, 7, 0, target, 128, meta) == 4
    assert prodex_mojo_log_event_label_v1(1, address, 7, 0, target, 1, meta) == 2
    assert prodex_mojo_log_event_label_v1(1, address, 7, 2, target, 128, meta) == 1
    source = StringSlice("é")
    assert prodex_mojo_log_event_label_v1(1, UInt(Int(source.unsafe_ptr())), 1, 0, target, 128, meta) == 1
    assert output[unsafe_offset=0] == 73 and written[] == 83
    dealloc(os^)
    dealloc(ws^)
