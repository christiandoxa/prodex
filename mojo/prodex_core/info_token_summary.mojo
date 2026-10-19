# Grouping and saturating token totals for acquired completed-usage events.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_compare, rich_view_order_sort
from provider_usage import usage_saturating_add


def info_token_usage_summary(
    profiles_address: UInt, counts_address: UInt, count: Int64,
    output_address: UInt, output_words: Int64,
) -> Int64:
    if count < 0 or count > (0x7FFFFFFFFFFFFFFF - 48) // 56 or output_words < 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    if count > 0 and (profiles_address == 0 or counts_address == 0 or profiles_address % 8 != 0 or counts_address % 8 != 0):
        return 1
    if output_words < 6 + count * 7:
        return 3
    var profiles = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(profiles_address))
    for index in range(count):
        if not rich_view_valid(profiles[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var counts = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(counts_address))
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var order = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + UInt(6 + count * 6) * 8))
    for index in range(count):
        order[unsafe_offset=index] = index
    rich_view_order_sort(profiles, order, count)
    for index in range(6):
        output[unsafe_offset=index] = 0
    var start: Int64 = 0
    var groups: Int64 = 0
    while start < count:
        var first = order[unsafe_offset=start]
        var label = profiles[unsafe_offset=first].copy()
        var end = start + 1
        while end < count and rich_view_compare(label, profiles[unsafe_offset=order[unsafe_offset=end]]) == 0:
            end += 1
        var base = 6 + groups * 6
        output[unsafe_offset=base] = UInt64(first)
        output[unsafe_offset=base + 1] = UInt64(end - start)
        for column in range(Int64(4)):
            var total: UInt64 = 0
            for position in range(start, end):
                total = usage_saturating_add(total, counts[unsafe_offset=order[unsafe_offset=position] * 4 + column])
            output[unsafe_offset=base + 2 + column] = total
            output[unsafe_offset=2 + column] = usage_saturating_add(output[unsafe_offset=2 + column], total)
        groups += 1
        start = end
    output[unsafe_offset=0] = UInt64(groups)
    output[unsafe_offset=1] = UInt64(count)
    return 0
