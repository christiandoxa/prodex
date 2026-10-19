# Grouped provider and capability summaries use caller-owned sorting scratch.
from std.memory import Pointer
from std.collections import Array
from rich_types import ProdexRichStringView
from rich_text import rich_view_order_sort, rich_view_compare
from json_sink import JsonSink, js_literal, js_view, js_byte, js_u64


def info_provider_groups(
    sink: Pointer[mut=True, JsonSink, _], views: Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin],
    order: Pointer[mut=True, Int64, MutUntrackedOrigin], count: Int64,
):
    rich_view_order_sort(views, order, count)
    var start: Int64 = 0
    while start < count:
        var end = start + 1
        var label = views[unsafe_offset=order[unsafe_offset=start]].copy()
        while end < count and rich_view_compare(label, views[unsafe_offset=order[unsafe_offset=end]]) == 0:
            end += 1
        if start > 0:
            js_literal(sink, StringSlice(", "))
        js_view(sink, label)
        js_byte(sink, 61)
        js_u64(sink, UInt64(end - start))
        start = end


def info_provider_render(
    capabilities: Bool, unsigned_address: UInt, text_address: UInt, count: Int64,
    output_address: UInt, capacity: Int64, written_address: UInt,
) -> Int64:
    if count > 0x7FFFFFFFFFFFFFFF // 48 or output_address == 0 or (count > 0 and output_address % 8 != 0):
        return 1
    var routes = Array[UInt64, 4](fill=0)
    var openai_format: UInt64 = 0
    if capabilities and count > 0:
        var facts = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(unsigned_address))
        for index in range(count):
            var route = facts[unsafe_offset=index * 2]
            var openai = facts[unsafe_offset=index * 2 + 1]
            if route > 3 or openai > 1:
                return 1
            routes[Int(route)] += 1
            openai_format += openai
    var scratch_bytes = count * 24
    if capacity < scratch_bytes:
        return 3
    var front_capacity = capacity if count == 0 else (capacity - scratch_bytes) & -8
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), front_capacity, 0, False, False)
    if count == 0:
        js_literal(Pointer(to=sink), StringSlice("none"))
    else:
        # The aligned tail is excluded from the text writer's capacity.
        var views = Pointer[mut=True, ProdexRichStringView, MutUntrackedOrigin](unsafe_from_address=Int(output_address + UInt(front_capacity)))
        var order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + UInt(front_capacity + count * 16)))
        var labels = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(text_address))
        for index in range(count):
            views[unsafe_offset=index] = labels[unsafe_offset=index].copy()
            order[unsafe_offset=index] = Int64(index)
        if capabilities:
            js_literal(Pointer(to=sink), StringSlice("routes native="))
            js_u64(Pointer(to=sink), routes[0])
            js_literal(Pointer(to=sink), StringSlice(", adapter="))
            js_u64(Pointer(to=sink), routes[1])
            js_literal(Pointer(to=sink), StringSlice(", external-cli="))
            js_u64(Pointer(to=sink), routes[2])
            js_literal(Pointer(to=sink), StringSlice(", unsupported="))
            js_u64(Pointer(to=sink), routes[3])
            js_literal(Pointer(to=sink), StringSlice("; openai-format="))
            js_u64(Pointer(to=sink), openai_format)
            js_literal(Pointer(to=sink), StringSlice("; quota "))
        info_provider_groups(Pointer(to=sink), views, order, count)
    if sink.failed:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = sink.written
    return 0
