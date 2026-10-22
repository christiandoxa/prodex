from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import pj_literal
from quota_watch_presentation import prodex_quota_tui_span_plan_v1


def check(text: StringSlice, operation: Int64, kind: Int64, label_end: Int64, value_start: Int64, output: UInt64):
    var view = pj_literal(text)
    assert prodex_quota_tui_span_plan_v1(1, operation, UInt64(view.ptr), Int64(view.len), output, 3) == 0
    from std.memory import Pointer
    var result = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(output))
    assert result[unsafe_offset=0] == kind
    assert result[unsafe_offset=1] == label_end
    assert result[unsafe_offset=2] == value_start


def main() raises:
    var storage = alloc(Layout[Int64](count=3))
    var output = storage.unsafe_ptr()
    var address = UInt64(Int(output))
    check(StringSlice(""), 0, 2, 0, 0, address)
    check(StringSlice(""), 1, 0, 0, 0, address)
    check(StringSlice(" -\u2003-\u00a0"), 0, 2, 0, 0, address)
    check(StringSlice("Quota Ready Error"), 0, 1, 0, 0, address)
    check(StringSlice("== x"), 0, 1, 0, 0, address)
    check(StringSlice("==x"), 0, 0, 0, 0, address)
    check(StringSlice(" xprofiles"), 0, 1, 0, 0, address)
    check(StringSlice("Quota"), 0, 0, 0, 0, address)
    check(StringSlice("Quota \0\n"), 0, 1, 0, 0, address)
    check(StringSlice("Status: Error"), 0, 7, 7, 8, address)
    check(StringSlice("Status:\u00a0  Ready "), 0, 7, 7, 11, address)
    check(StringSlice("\u2003Account:\t 雪:more "), 0, 7, 11, 13, address)
    check(StringSlice("Error Ready critical"), 0, 3, 0, 0, address)
    check(StringSlice("Error Ready critical"), 1, 3, 0, 0, address)
    check(StringSlice("Ready critical"), 0, 4, 0, 0, address)
    check(StringSlice("unhealthy"), 0, 4, 0, 0, address)
    check(StringSlice("unhealthy"), 1, 4, 0, 0, address)
    check(StringSlice("thin"), 0, 3, 0, 0, address)
    check(StringSlice("thin"), 1, 0, 0, 0, address)
    check(StringSlice("PROFILE Ready"), 0, 4, 0, 0, address)
    check(StringSlice("PROFILE names"), 0, 5, 0, 0, address)
    check(StringSlice("  workspace: value"), 0, 6, 0, 0, address)
    check(StringSlice("\u001cworkspace: value"), 0, 0, 0, 0, address)
    check(StringSlice("status: value"), 0, 6, 0, 0, address)
    check(StringSlice("press Ready"), 0, 4, 0, 0, address)
    check(StringSlice(" \u2003press q"), 0, 2, 0, 0, address)
    check(StringSlice("雪\0"), 0, 0, 0, 0, address)
    output[unsafe_offset=0] = -5
    output[unsafe_offset=1] = 255
    output[unsafe_offset=2] = -7
    assert prodex_quota_tui_span_plan_v1(2, 0, 0, 0, address, 3) == 4
    assert prodex_quota_tui_span_plan_v1(1, 2, 0, 0, address, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, -1, address, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 1, address, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, address+8, 1, address, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 0, address+1, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 0, 0, 3) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 0, address, -1) == 1
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 0, address, 2) == 3
    assert output[unsafe_offset=0] == -5 and output[unsafe_offset=1] == 255 and output[unsafe_offset=2] == -7
    assert prodex_quota_tui_span_plan_v1(1, 0, 0, 0, address, 3) == 0
    assert output[unsafe_offset=0] == 2 and output[unsafe_offset=1] == 0 and output[unsafe_offset=2] == 0
    dealloc(storage^)
