from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_trim_bounds
from quota_watch_policy import quota_watch_text_contains
from json_sink import JsonSink, js_byte, js_literal, js_view


def lrp_line(sink: Pointer[mut=True, JsonSink, _], prefix: StringSlice, value: ProdexRichStringView):
    js_literal(sink, prefix)
    js_view(sink, value)
    js_byte(sink, 10)


def lrp_report(
    sink: Pointer[mut=True, JsonSink, _], texts: Pointer[mut=False, ProdexRichStringView, _],
    flags: UInt64, args_count: Int64, env_count: Int64, removed_count: Int64, proxy_port: Int64,
):
    js_literal(sink, StringSlice("Prodex dry run: launch diagnostics\n"))
    lrp_line(sink, StringSlice("Flow: "), texts[unsafe_offset=0])
    lrp_line(sink, StringSlice("Binary: "), texts[unsafe_offset=1])
    if flags & 1 != 0:
        lrp_line(sink, StringSlice("Provider: "), texts[unsafe_offset=2])
    else:
        js_literal(sink, StringSlice("Provider: openai\n"))
    if flags & 2 != 0:
        lrp_line(sink, StringSlice("Model: "), texts[unsafe_offset=3])
    else:
        js_literal(sink, StringSlice("Model: (codex default)\n"))
    js_literal(sink, StringSlice("CODEX_HOME: <CODEX_HOME>\n"))
    if flags & 4 == 0:
        js_literal(sink, StringSlice("Runtime proxy: disabled\n"))
    elif proxy_port == 0:
        lrp_line(sink, StringSlice("Runtime proxy: would be enabled with mount "), texts[unsafe_offset=5])
    else:
        js_literal(sink, StringSlice("Runtime proxy: enabled at http://"))
        js_view(sink, texts[unsafe_offset=4])
        js_view(sink, texts[unsafe_offset=5])
        js_byte(sink, 10)
    js_literal(sink, StringSlice("Args:\n"))
    if args_count == 0:
        js_literal(sink, StringSlice("  (none)\n"))
    for index in range(args_count):
        lrp_line(sink, StringSlice("  "), texts[unsafe_offset=6 + index])
    js_literal(sink, StringSlice("Env:\n  CODEX_HOME=<CODEX_HOME>\n"))
    var start = 6 + args_count
    for index in range(env_count):
        js_literal(sink, StringSlice("  "))
        js_view(sink, texts[unsafe_offset=start + index * 2])
        js_byte(sink, 61)
        js_view(sink, texts[unsafe_offset=start + index * 2 + 1])
        js_byte(sink, 10)
    if removed_count > 0:
        js_literal(sink, StringSlice("Removed env:\n"))
    start += env_count * 2
    for index in range(removed_count):
        lrp_line(sink, StringSlice("  "), texts[unsafe_offset=start + index])
    js_literal(sink, StringSlice("Codex/TUI not started because --dry-run was set.\n"))


def lrp_completion(
    sink: Pointer[mut=True, JsonSink, _], texts: Pointer[mut=False, ProdexRichStringView, _], flags: UInt64,
):
    js_view(sink, texts[unsafe_offset=0])
    js_literal(sink, StringSlice("Profile: <configured>\n") if flags & 1 != 0 else StringSlice("Profile: (active/default)\n"))
    if flags & 4 != 0:
        lrp_line(sink, StringSlice(""), texts[unsafe_offset=1])
    js_literal(sink, StringSlice("Presidio redaction: "))
    js_literal(sink, StringSlice("enabled (would start at launch)\n") if flags & 2 != 0 else StringSlice("disabled (not started)\n"))


def lrp_emit(
    sink: Pointer[mut=True, JsonSink, _], operation: Int64,
    texts: Pointer[mut=False, ProdexRichStringView, _], flags: UInt64,
    args_count: Int64, env_count: Int64, removed_count: Int64, proxy_port: Int64,
):
    if operation == 0:
        lrp_report(sink, texts, flags, args_count, env_count, removed_count, proxy_port)
    else:
        lrp_completion(sink, texts, flags)


@export("prodex_runtime_launch_report_v1")
def prodex_runtime_launch_report_v1(
    abi: Int64, operation: Int64, texts_address: UInt64, text_count: Int64, flags: UInt64,
    args_count: Int64, env_count: Int64, removed_count: Int64, proxy_port: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 1 or flags > 7
        or texts_address == 0 or texts_address % 8 != 0
        or text_count < 1 or text_count > 0x7FFFFFFFFFFFFFFF // 16
        or args_count < 0 or env_count < 0 or removed_count < 0
        or proxy_port < 0 or proxy_port > 65535
        or metadata_address == 0 or metadata_address % 8 != 0
        or measuring < 0 or measuring > 1 or capacity < 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    if operation == 0:
        if text_count < 6 or (flags & 4 == 0 and proxy_port != 0):
            return 1
        var remaining = text_count - 6
        if args_count > remaining:
            return 1
        remaining -= args_count
        if env_count > remaining // 2:
            return 1
        remaining -= env_count * 2
        if removed_count != remaining:
            return 1
    elif (
        text_count != Int64(2 if flags & 4 != 0 else 1)
        or args_count != 0 or env_count != 0 or removed_count != 0 or proxy_port != 0
    ):
        return 1
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(texts_address))
    for index in range(text_count):
        if not rich_view_valid(texts[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    lrp_emit(Pointer(to=sink), operation, texts, flags, args_count, env_count, removed_count, proxy_port)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        lrp_emit(Pointer(to=sink), operation, texts, flags, args_count, env_count, removed_count, proxy_port)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0


@fieldwise_init
struct RuntimeLaunchTuiPlan(Copyable):
    var kind: Int64
    var label_end: Int64
    var value_start: Int64
    var color: Int64


def lrp_tui_value_color(label: ProdexRichStringView, value: ProdexRichStringView) -> Int64:
    for word in [StringSlice("disabled"), StringSlice("removed")]:
        if quota_watch_text_contains(UInt64(value.ptr), Int64(value.len), word):
            return 1
    for word in [StringSlice("enabled"), StringSlice("ready"), StringSlice("ok")]:
        if quota_watch_text_contains(UInt64(value.ptr), Int64(value.len), word):
            return 2
    if quota_watch_text_contains(UInt64(label.ptr), Int64(label.len), StringSlice("proxy")):
        return 2
    for word in [StringSlice("command"), StringSlice("binary"), StringSlice("codex_home")]:
        if quota_watch_text_contains(UInt64(label.ptr), Int64(label.len), word):
            return 3
    return 0


def lrp_tui_line_plan(text: ProdexRichStringView) -> RuntimeLaunchTuiPlan:
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(text.ptr))
    if text.len > 0 and bytes[unsafe_offset=Int64(text.len) - 1] == 58:
        return RuntimeLaunchTuiPlan(1, 0, 0, 0)
    for index in range(Int64(text.len)):
        if bytes[unsafe_offset=index] != 58:
            continue
        var label = ProdexRichStringView(text.ptr, UInt(index))
        var value = ProdexRichStringView(text.ptr + UInt(index + 1), text.len - UInt(index + 1))
        return RuntimeLaunchTuiPlan(2, index + 1, index + 1 + rich_trim_bounds(value)[0], lrp_tui_value_color(label, value))
    return RuntimeLaunchTuiPlan(0, 0, 0, 0)


def lrp_tui_height(text: ProdexRichStringView) -> Int64:
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(text.ptr))
    var rows: Int64 = 0
    for index in range(Int64(text.len)):
        if bytes[unsafe_offset=index] == 10:
            rows += 1
            if rows >= 27:
                return 32
    if text.len > 0 and bytes[unsafe_offset=Int64(text.len) - 1] != 10:
        rows += 1
    if rows < 3:
        return 8
    if rows >= 27:
        return 32
    return rows + 5


@export("prodex_runtime_launch_tui_plan_v1")
def prodex_runtime_launch_tui_plan_v1(
    abi: Int64, operation: Int64, text_address: UInt64, text_length: Int64,
    output_address: UInt64, output_capacity: Int64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 1 or text_length < 0 or output_address == 0 or output_address % 8 != 0 or output_capacity < 0:
        return 1
    var text = ProdexRichStringView(UInt(text_address), UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 1
    if output_capacity < 4:
        return 3
    var plan = lrp_tui_line_plan(text) if operation == 0 else RuntimeLaunchTuiPlan(lrp_tui_height(text), 0, 0, 0)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = plan.kind
    output[unsafe_offset=1] = plan.label_end
    output[unsafe_offset=2] = plan.value_start
    output[unsafe_offset=3] = plan.color
    return 0
