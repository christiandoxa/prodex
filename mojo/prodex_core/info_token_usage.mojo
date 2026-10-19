# Token-usage event number/default policy; wire decoding stays at the caller.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from log_number import log_parse_u64_text, log_positive_finite_decimal, log_completed_rate


# Texts: input, cached, output, reasoning, request, generation, reported rate.
# Output: accepted, six numbers, request/generation/rate presence bits, rate bits.
def info_token_usage_numbers(
    kind: Int64, text_address: UInt, text_count: Int64, presence: UInt64,
    output_address: UInt, output_words: Int64,
) -> Int64:
    if kind < 0 or kind > 1 or text_count != 7 or presence > 127 or text_address == 0 or output_address == 0 or output_words < 9 or text_address % 8 != 0 or output_address % 8 != 0:
        return 1
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(text_address))
    for index in range(7):
        if not rich_view_valid(texts[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF) or ((presence & (UInt64(1) << UInt64(index))) == 0 and texts[unsafe_offset=index].len != 0):
            return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(9):
        output[unsafe_offset=index] = 0
    var valid: UInt64 = 0
    for index in range(6):
        if (presence & (UInt64(1) << UInt64(index))) == 0:
            continue
        var text = texts[unsafe_offset=index].copy()
        var parsed = log_parse_u64_text(UInt64(text.ptr), UInt64(text.len))
        if parsed[0]:
            valid |= UInt64(1) << UInt64(index)
            output[unsafe_offset=index + 1] = parsed[1]
    var accepted = (kind == 1 or (valid & 1) != 0) and ((valid & 4) != 0 or (kind == 0 and (presence & 4) == 0))
    if not accepted:
        for index in range(9):
            output[unsafe_offset=index] = 0
        return 0
    var timing_valid = (presence & 64) == 0 or log_positive_finite_decimal(texts[unsafe_offset=6])
    var request_present = (valid & 16) != 0
    var generation_present = (valid & 32) != 0 and output[unsafe_offset=6] > 0 and timing_valid
    var generation = output[unsafe_offset=6] if generation_present else UInt64(0)
    var rate = log_completed_rate(output[unsafe_offset=3], generation)
    output[unsafe_offset=0] = 1
    output[unsafe_offset=6] = generation
    output[unsafe_offset=7] = UInt64(request_present) | (UInt64(generation_present) << 1) | (UInt64(rate[0]) << 2)
    Pointer[mut=True, Float64, MutUntrackedOrigin](unsafe_from_address=Int(output_address + 64))[] = rate[1]
    return 0
