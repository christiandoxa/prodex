# Shared checked number parsing and completed-rate computation for log policies.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr


def log_parse_u64_text(
    address: UInt64,
    length: UInt64,
) -> Tuple[Bool, UInt64]:
    if length == 0 or length > UInt64(0x7FFF_FFFF_FFFF_FFFF) or address == 0:
        return (False, UInt64(0))
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var index: UInt64 = 0
    if source[unsafe_offset=0] == 43:
        index = 1
        if index == length:
            return (False, UInt64(0))
    var value: UInt64 = 0
    while index < length:
        var byte = source[unsafe_offset=Int(index)]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(byte - 48)
        if value > (UInt64(0xFFFFFFFFFFFFFFFF) - digit) // 10:
            return (False, UInt64(0))
        value = value * 10 + digit
        index += 1
    return (True, value)



def log_decimal_significand_order(
    view: ProdexRichStringView, start: Int64, end: Int64, boundary: StringSlice,
) -> Int64:
    var source = rich_view_ptr(view)
    var expected = boundary.unsafe_ptr()
    var position: Int64 = 0
    var boundary_length = Int64(boundary.byte_length())
    for cursor in range(start, end):
        var byte = source[unsafe_offset=cursor]
        if byte == 46:
            continue
        var other = expected[unsafe_offset=position] if position < boundary_length else UInt8(48)
        if byte != other:
            return -1 if byte < other else 1
        position += 1
    while position < boundary_length:
        if expected[unsafe_offset=position] != 48:
            return -1
        position += 1
    return 0


def log_positive_finite_decimal(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var source = rich_view_ptr(view)
    var length = Int64(view.len)
    var cursor: Int64 = 0
    if source[unsafe_offset=cursor] == 43:
        cursor += 1
    var digits: UInt64 = 0
    var integer_digits: UInt64 = 0
    var leading_zeros: UInt64 = 0
    var first_significant: Int64 = -1
    var decimal_point = False
    while cursor < length and source[unsafe_offset=cursor] != 101 and source[unsafe_offset=cursor] != 69:
        var byte = source[unsafe_offset=cursor]
        if byte == 46 and not decimal_point:
            decimal_point = True
        elif byte >= 48 and byte <= 57:
            digits += 1
            if not decimal_point:
                integer_digits += 1
            if first_significant < 0:
                if byte == 48:
                    leading_zeros += 1
                else:
                    first_significant = cursor
        else:
            return False
        cursor += 1
    if digits == 0 or first_significant < 0:
        return False
    var mantissa_end = cursor
    var exponent: UInt64 = 0
    var exponent_negative = False
    if cursor < length:
        cursor += 1
        if cursor < length and (source[unsafe_offset=cursor] == 43 or source[unsafe_offset=cursor] == 45):
            exponent_negative = source[unsafe_offset=cursor] == 45
            cursor += 1
        var exponent_start = cursor
        # Larger exponents cannot be cancelled by this document's mantissa.
        var cap = UInt64(length) + 512
        while cursor < length:
            var byte = source[unsafe_offset=cursor]
            if byte < 48 or byte > 57:
                return False
            var digit = UInt64(byte - 48)
            exponent = cap if exponent > (cap - digit) // 10 else exponent * 10 + digit
            cursor += 1
        if cursor == exponent_start:
            return False
    var negative = integer_digits <= leading_zeros
    var magnitude = leading_zeros - integer_digits + 1 if negative else integer_digits - leading_zeros - 1
    if negative == exponent_negative:
        if exponent > 400 or magnitude > 400:
            return False
        magnitude += exponent
    elif exponent > magnitude:
        magnitude = exponent - magnitude
        negative = exponent_negative
    else:
        magnitude -= exponent
    if negative:
        if magnitude != 324:
            return magnitude < 324
        # Exactly 2^-1075 rounds to zero; the next larger decimal rounds up.
        return log_decimal_significand_order(view, first_significant, mantissa_end,
            StringSlice("24703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328125")) > 0
    if magnitude != 308:
        return magnitude < 308
    # Exactly 2^1024 - 2^970 rounds to infinity.
    return log_decimal_significand_order(view, first_significant, mantissa_end,
        StringSlice("179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497792")) < 0


def log_completed_rate(output_tokens: UInt64, generation_ms: UInt64) -> Tuple[Bool, Float64]:
    if output_tokens == 0 or generation_ms == 0:
        return (False, Float64(0))
    return (True, Float64(output_tokens) * 1000.0 / Float64(generation_ms))
