# Quota rows and profile fields share one presentation policy. Provider-specific
# renderers and Serde acquisition remain the host's existing boundaries.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_OBJECT, JSON_STRING, JSON_ARRAY, JSON_TRUE,
    pj_valid, pj_kind, pj_field, pj_text, pj_literal, pj_trim, pj_is, pj_child, pj_next,
)
from rich_text import (
    rich_view_prefix, rich_trim_bounds, rich_view_valid, rich_codepoint_width,
    rich_codepoint, rich_unicode_space,
)
from rich_types import ProdexRichStringView
from json_sink import JsonSink, js_byte, js_literal, js_string, js_view, js_escaped
from quota_watch_policy import (
    quota_watch_reset_kind, quota_watch_text_starts_at_exact, quota_watch_text_contains_exact,
)


def qwp_value(tree: ParsedJson, key: StringSlice) -> ProdexRichStringView:
    return pj_text(tree, pj_field(tree, 0, key))


def qwp_optional(view: ProdexRichStringView) -> ProdexRichStringView:
    var trimmed = pj_trim(view)
    return trimmed^ if trimmed.len > 0 else pj_literal(StringSlice("-"))


def qwp_kind(tree: ParsedJson) -> Int64:
    var node = pj_field(tree, 0, StringSlice("kind"))
    if pj_is["openai"](tree, node):
        return 0
    if pj_is["copilot"](tree, node):
        return 1
    if pj_is["gemini"](tree, node):
        return 2
    if pj_is["external"](tree, node):
        return 3
    return 4 if pj_is["error"](tree, node) else -1


def qwp_reset(
    tree: ParsedJson, kind: Int64, address: UInt, capacity: Int64,
) -> Tuple[Bool, ProdexRichStringView]:
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(address)),
        capacity, 0, False, False,
    )
    var reset = pj_field(tree, 0, StringSlice("reset"))
    if kind != 4:
        js_literal(Pointer(to=sink), StringSlice("resets: "))
    if pj_kind(tree, reset) == JSON_STRING:
        js_view(Pointer(to=sink), pj_text(tree, reset))
    elif kind != 4:
        js_literal(Pointer(to=sink), StringSlice("unavailable"))
    if kind == 0:
        var credits = pj_field(tree, 0, StringSlice("reset_credit_summary"))
        if pj_kind(tree, credits) == JSON_STRING:
            js_literal(Pointer(to=sink), StringSlice("; reset credits: "))
            js_view(Pointer(to=sink), pj_text(tree, credits))
    return (not sink.failed, ProdexRichStringView(address, UInt(sink.written)))


def qwp_strip_reset(view: ProdexRichStringView) -> ProdexRichStringView:
    if rich_view_prefix["resets: "](view, False):
        return ProdexRichStringView(view.ptr + 8, view.len - 8)
    return view.copy()


def qwp_cell(sink: Pointer[mut=True, JsonSink, _], name: StringSlice, value: ProdexRichStringView):
    js_string(sink, pj_literal(name))
    js_literal(sink, StringSlice(":["))
    js_string(sink, value)
    js_literal(sink, StringSlice("],"))


def qwp_pair(sink: Pointer[mut=True, JsonSink, _], name: StringSlice, value: ProdexRichStringView):
    js_byte(sink, 91)
    js_string(sink, pj_literal(name))
    js_byte(sink, 44)
    js_string(sink, value)
    js_byte(sink, 93)


def qwp_compact_label_end(text: ProdexRichStringView) -> Int64:
    var first = rich_trim_bounds(text)[0]
    for label in [
        StringSlice("Available"), StringSlice("Last Updated"),
        StringSlice("5h remaining pool"), StringSlice("Weekly remaining pool"),
        StringSlice("Remaining pool"), StringSlice("Account"), StringSlice("Plan"),
        StringSlice("Status"), StringSlice("Main"), StringSlice("5h"),
        StringSlice("Weekly"), StringSlice("Reset credits"), StringSlice("Auth"),
        StringSlice("Provider"), StringSlice("Reset"), StringSlice("Project"),
        StringSlice("Bucket"), StringSlice("Quota"), StringSlice("Rate groups"),
        StringSlice("Model groups"), StringSlice("Admin API"),
        StringSlice("Model provider"), StringSlice("Source"),
    ]:
        var end = first + Int64(label.byte_length())
        if quota_watch_text_starts_at_exact(UInt64(text.ptr), Int64(text.len), first, label) and quota_watch_text_starts_at_exact(
            UInt64(text.ptr), Int64(text.len), end, StringSlice(":"),
        ):
            return end + 1
    return -1


def qwp_render(
    sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, operation: Int64,
    detail: Bool, kind: Int64, reset: ProdexRichStringView, reset_kind: Int64,
):
    if operation == 2:
        js_literal(sink, StringSlice("true") if qwp_compact_label_end(pj_text(tree, 0)) >= 0 else StringSlice("false"))
        return
    var account = qwp_optional(qwp_value(tree, StringSlice("account")))
    var plan_node = pj_field(tree, 0, StringSlice("plan"))
    if kind == 1 and pj_kind(tree, plan_node) != JSON_STRING:
        plan_node = pj_field(tree, 0, StringSlice("fallback_plan"))
    var plan = qwp_optional(pj_text(tree, plan_node))
    var main = pj_literal(StringSlice("-")) if kind == 4 else qwp_value(tree, StringSlice("main"))
    var status = qwp_value(tree, StringSlice("status"))
    if operation == 1:
        js_byte(sink, 91)
        qwp_pair(sink, StringSlice("Account"), account)
        js_byte(sink, 44)
        qwp_pair(sink, StringSlice("Plan"), plan)
        js_byte(sink, 44)
        qwp_pair(sink, StringSlice("Status"), status)
        js_byte(sink, 44)
        qwp_pair(sink, StringSlice("Main"), main)
        if detail or kind != 0:
            var reset_value = reset.copy()
            if kind == 0:
                while rich_view_prefix["resets: "](reset_value, False):
                    reset_value = qwp_strip_reset(reset_value)
            else:
                var raw_reset = pj_field(tree, 0, StringSlice("reset"))
                reset_value = pj_text(tree, raw_reset) if pj_kind(tree, raw_reset) == JSON_STRING else pj_literal(StringSlice("unavailable"))
            js_byte(sink, 44)
            qwp_pair(sink, StringSlice("Reset"), reset_value)
        if kind == 0 and not detail:
            var credits = pj_field(tree, 0, StringSlice("reset_credit_summary"))
            if pj_kind(tree, credits) == JSON_STRING:
                js_byte(sink, 44)
                qwp_pair(sink, StringSlice("Reset credits"), pj_text(tree, credits))
        js_byte(sink, 44)
        qwp_pair(sink, StringSlice("Last Updated"), qwp_value(tree, StringSlice("updated")))
        js_byte(sink, 93)
        return
    if kind == 0:
        main = qwp_value(tree, StringSlice("compact_main"))
    js_byte(sink, 123)
    qwp_cell(sink, StringSlice("profile"), qwp_value(tree, StringSlice("profile")))
    qwp_cell(sink, StringSlice("current"), pj_literal(
        StringSlice("*") if pj_kind(tree, pj_field(tree, 0, StringSlice("active"))) == JSON_TRUE else StringSlice(""),
    ))
    qwp_cell(sink, StringSlice("auth"), qwp_value(tree, StringSlice("auth")))
    qwp_cell(sink, StringSlice("account"), account)
    qwp_cell(sink, StringSlice("plan"), plan)
    qwp_cell(sink, StringSlice("status"), status)
    qwp_cell(sink, StringSlice("remaining"), main)
    js_literal(sink, StringSlice('"detail":['))
    if detail:
        var windows = reset.copy() if reset_kind == 1 else qwp_strip_reset(reset)
        var credits = ProdexRichStringView(0, 0)
        if reset_kind == 2:
            for cursor in range(Int64(windows.len)):
                if quota_watch_text_starts_at_exact(UInt64(windows.ptr), Int64(windows.len), cursor, StringSlice("; reset credits:")):
                    var start = cursor + Int64(StringSlice("; reset credits:").byte_length())
                    credits = pj_trim(ProdexRichStringView(windows.ptr + UInt(start), windows.len - UInt(start)))
                    windows = ProdexRichStringView(windows.ptr, UInt(cursor))
                    break
        js_byte(sink, 34)
        if reset_kind != 1:
            js_literal(sink, StringSlice("resets: "))
        js_escaped(sink, windows)
        var workspace = pj_trim(qwp_value(tree, StringSlice("workspace_name")))
        if kind == 0 and pj_trim(qwp_value(tree, StringSlice("account"))).len > 0 and (
            workspace.len > 0 or pj_trim(qwp_value(tree, StringSlice("workspace_id"))).len > 0
        ):
            if workspace.len == 0:
                workspace = pj_trim(qwp_value(tree, StringSlice("profile")))
            js_literal(sink, StringSlice(" | workspace: "))
            js_escaped(sink, workspace)
        if reset_kind == 2:
            js_literal(sink, StringSlice(" | reset credits: "))
            js_escaped(sink, credits)
        if kind == 0:
            var additional = pj_field(tree, 0, StringSlice("additional"))
            var item = pj_child(tree, additional) if pj_kind(tree, additional) == JSON_ARRAY else -1
            while item >= 0:
                js_literal(sink, StringSlice(" | "))
                js_escaped(sink, pj_text(tree, item))
                item = pj_next(tree, item)
        js_byte(sink, 34)
    js_literal(sink, StringSlice("]}"))


@export("prodex_quota_watch_presentation_v1")
def prodex_quota_watch_presentation_v1(
    abi: Int64, operation: Int64, detail: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 2 or detail < 0 or detail > 1
        or nodes_address == 0 or count <= 0 or count > 1048576
        or raw_address == 0 or raw_length < 0 or raw_length > 67108864
        or scratch_address == 0 or scratch_count < 0 or scratch_count > 0x7FFFFFFFFFFFFFFF // 16
        or scratch_count * 16 < raw_length + 128 or metadata_address == 0
        or measuring < 0 or measuring > 1 or capacity < 0
        or measuring == 0 and output_address == 0
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != (JSON_STRING if operation == 2 else JSON_OBJECT):
        return 1
    var kind = -1 if operation == 2 else qwp_kind(tree)
    if operation != 2 and (kind < 0 or operation == 1 and kind == 4):
        return 1
    var reset = ProdexRichStringView(0, 0)
    var reset_kind: Int64 = 0
    if operation != 2:
        var value = qwp_reset(tree, kind, UInt(scratch_address), scratch_count * 16)
        if not value[0]:
            return 3
        reset = value[1].copy()
        reset_kind = quota_watch_reset_kind(UInt64(reset.ptr), Int64(reset.len))
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, True, False,
    )
    qwp_render(Pointer(to=sink), tree, operation, detail == 1, kind, reset, reset_kind)
    if sink.failed or measuring == 0 and sink.written > capacity:
        return 3
    if measuring == 0:
        sink.written = 0
        sink.measuring = False
        qwp_render(Pointer(to=sink), tree, operation, detail == 1, kind, reset, reset_kind)
        if sink.failed:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0


@fieldwise_init
struct QuotaTuiSpanPlan(Copyable):
    var kind: Int64
    var label_end: Int64
    var value_start: Int64


def qwp_status_tone(text: ProdexRichStringView) -> Int64:
    for word in [StringSlice("Blocked"), StringSlice("Error")]:
        if quota_watch_text_contains_exact(UInt64(text.ptr), Int64(text.len), word):
            return 3
    for word in [StringSlice("Ready"), StringSlice("healthy")]:
        if quota_watch_text_contains_exact(UInt64(text.ptr), Int64(text.len), word):
            return 4
    return 0


def qwp_separator(text: ProdexRichStringView) -> Bool:
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(text.ptr))
    var cursor: Int64 = 0
    while cursor < Int64(text.len):
        var width = rich_codepoint_width(bytes[unsafe_offset=cursor])
        var value = rich_codepoint(bytes, cursor, width)
        if value != 45 and not rich_unicode_space(value):
            return False
        cursor += width
    return True


def qwp_human_span_plan(text: ProdexRichStringView) -> QuotaTuiSpanPlan:
    if (
        rich_view_prefix["== "](text, False) or rich_view_prefix["Quota "](text, False)
        or quota_watch_text_starts_at_exact(UInt64(text.ptr), Int64(text.len), Int64(text.len) - 8, StringSlice("profiles"))
    ):
        return QuotaTuiSpanPlan(1, 0, 0)
    var label_end = qwp_compact_label_end(text)
    if label_end >= 0:
        var value = ProdexRichStringView(text.ptr + UInt(label_end), text.len - UInt(label_end))
        return QuotaTuiSpanPlan(7, label_end, label_end + rich_trim_bounds(value)[0])
    if qwp_separator(text):
        return QuotaTuiSpanPlan(2, 0, 0)
    var status_tone = qwp_status_tone(text)
    if status_tone != 0:
        return QuotaTuiSpanPlan(status_tone, 0, 0)
    for word in [StringSlice("thin"), StringSlice("critical"), StringSlice("exhausted")]:
        if quota_watch_text_contains_exact(UInt64(text.ptr), Int64(text.len), word):
            return QuotaTuiSpanPlan(3, 0, 0)
    if rich_view_prefix["PROFILE"](text, False):
        return QuotaTuiSpanPlan(5, 0, 0)
    var first = rich_trim_bounds(text)[0]
    for label in [StringSlice("workspace:"), StringSlice("error:"), StringSlice("resets:"), StringSlice("status:"), StringSlice("reset credits:")]:
        if quota_watch_text_starts_at_exact(UInt64(text.ptr), Int64(text.len), first, label):
            return QuotaTuiSpanPlan(6, 0, 0)
    if quota_watch_text_starts_at_exact(UInt64(text.ptr), Int64(text.len), first, StringSlice("press ")):
        return QuotaTuiSpanPlan(2, 0, 0)
    return QuotaTuiSpanPlan(0, 0, 0)


@export("prodex_quota_tui_span_plan_v1")
def prodex_quota_tui_span_plan_v1(
    abi: Int64, operation: Int64, text_address: UInt64, text_length: Int64,
    output_address: UInt64, output_capacity: Int64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 1 or text_length < 0
        or output_address == 0 or output_address % 8 != 0 or output_capacity < 0
    ):
        return 1
    var text = ProdexRichStringView(UInt(text_address), UInt(text_length))
    if not rich_view_valid(text, 0x7FFFFFFFFFFFFFFF):
        return 1
    if output_capacity < 3:
        return 3
    var plan = qwp_human_span_plan(text) if operation == 0 else QuotaTuiSpanPlan(qwp_status_tone(text), 0, 0)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = plan.kind
    output[unsafe_offset=1] = plan.label_end
    output[unsafe_offset=2] = plan.value_start
    return 0
