from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import pj_literal
from runtime_launch_report import prodex_runtime_launch_tui_plan_v1


def check(text: StringSlice, operation: Int64, first: Int64, label_end: Int64, value_start: Int64, color: Int64, target: UInt64):
    from std.memory import Pointer
    var view = pj_literal(text)
    assert prodex_runtime_launch_tui_plan_v1(1, operation, UInt64(view.ptr), Int64(view.len), target, 4) == 0
    var output = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(target))
    assert output[unsafe_offset=0] == first
    assert output[unsafe_offset=1] == label_end
    assert output[unsafe_offset=2] == value_start
    assert output[unsafe_offset=3] == color


def main() raises:
    var storage = alloc(Layout[Int64](count=4))
    var bytes_storage = alloc(Layout[UInt8](count=32))
    var output = storage.unsafe_ptr()
    var bytes = bytes_storage.unsafe_ptr()
    var target = UInt64(Int(output))
    var source = UInt64(Int(bytes))
    check(StringSlice(""), 0, 0, 0, 0, 0, target)
    check(StringSlice("Ready"), 0, 0, 0, 0, 0, target)
    check(StringSlice("Proxy: disabled:"), 0, 1, 0, 0, 0, target)
    check(StringSlice("Proxy: disabled enabled"), 0, 2, 6, 7, 1, target)
    check(StringSlice("pRoXy: x"), 0, 2, 6, 7, 2, target)
    check(StringSlice("BINARY: enabled"), 0, 2, 7, 8, 2, target)
    check(StringSlice("Binary: opaque"), 0, 2, 7, 8, 3, target)
    check(StringSlice("CODEX_HOME: value"), 0, 2, 11, 12, 3, target)
    check(StringSlice("Command: removed"), 0, 2, 8, 9, 1, target)
    check(StringSlice("Model: token"), 0, 2, 6, 7, 2, target)
    check(StringSlice("Label:\u2003雪  "), 0, 2, 6, 9, 0, target)
    check(StringSlice("\u2003Label:\t 雪"), 0, 2, 9, 11, 0, target)
    check(StringSlice("Label:\u001c雪"), 0, 2, 6, 6, 0, target)
    check(StringSlice(""), 1, 8, 0, 0, 0, target)
    check(StringSlice("a\nb\nc\nd"), 1, 9, 0, 0, 0, target)
    check(StringSlice("\r\n\r\n\r\n\r\n"), 1, 9, 0, 0, 0, target)
    for index in range(32):
        bytes[unsafe_offset=index] = 10
    assert prodex_runtime_launch_tui_plan_v1(1,1,source,26,target,4) == 0 and output[] == 31
    assert prodex_runtime_launch_tui_plan_v1(1,1,source,27,target,4) == 0 and output[] == 32
    assert prodex_runtime_launch_tui_plan_v1(1,1,source,32,target,4) == 0 and output[] == 32
    for index in range(4):
        output[unsafe_offset=index] = 77
    assert prodex_runtime_launch_tui_plan_v1(2,0,0,0,target,4) == 4
    assert prodex_runtime_launch_tui_plan_v1(1,2,0,0,target,4) == 1
    assert prodex_runtime_launch_tui_plan_v1(1,0,0,-1,target,4) == 1
    assert prodex_runtime_launch_tui_plan_v1(1,0,0,1,target,4) == 1
    assert prodex_runtime_launch_tui_plan_v1(1,0,0,0,target+1,4) == 1
    assert prodex_runtime_launch_tui_plan_v1(1,0,0,0,target,3) == 3
    bytes[] = 255
    assert prodex_runtime_launch_tui_plan_v1(1,0,source,1,target,4) == 1
    for index in range(4):
        assert output[unsafe_offset=index] == 77
    dealloc(storage^)
    dealloc(bytes_storage^)
