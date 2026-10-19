from std.collections import Array


@fieldwise_init
struct InfoDecimal(Copyable):
    var bytes: Array[UInt8, 312]
    var length: Int


# Exact IEEE-754 scaling and ties-to-even preserve Rust's fixed-one-decimal text.
def info_decimal_one(bits: UInt64) -> InfoDecimal:
    var text = InfoDecimal(Array[UInt8, 312](fill=0), 0)
    var bytes = text.bytes.unsafe_ptr()
    var exponent = Int64((bits >> UInt64(52)) & UInt64(2047))
    var significand = bits & UInt64(0x000fffffffffffff)
    var negative = (bits >> UInt64(63)) != 0
    if exponent == 2047:
        var label = StringSlice("NaN" if significand != 0 else "inf")
        if negative and significand == 0:
            bytes[unsafe_offset=0] = 45
            text.length = 1
        var source = label.unsafe_ptr()
        for index in range(Int(label.byte_length())):
            bytes[unsafe_offset=text.length] = source[unsafe_offset=index]
            text.length += 1
        return text^
    if negative:
        bytes[unsafe_offset=0] = 45
        text.length = 1
    var shift: Int64 = -1074
    if exponent != 0:
        significand |= UInt64(1) << UInt64(52)
        shift = exponent - 1075
    var scaled = significand * UInt64(10)
    if shift < 0:
        var places = -shift
        if places >= 64:
            scaled = 0
        else:
            var half = UInt64(1) << UInt64(places - 1)
            var remainder = scaled & ((UInt64(1) << UInt64(places)) - UInt64(1))
            scaled >>= UInt64(places)
            if remainder > half or (remainder == half and (scaled & UInt64(1)) != 0):
                scaled += 1
        shift = 0
    # The largest finite scaled f64 has at most 1028 bits / 310 decimal digits.
    var digits = Array[UInt8, 310](fill=0)
    var decimal_digits = digits.unsafe_ptr()
    var count = 0
    while scaled > 0:
        decimal_digits[unsafe_offset=count] = UInt8(scaled % UInt64(10))
        scaled //= UInt64(10)
        count += 1
    count = max(count, 1)
    for _ in range(shift):
        var carry: UInt8 = 0
        for index in range(count):
            var value = decimal_digits[unsafe_offset=index] * UInt8(2) + carry
            decimal_digits[unsafe_offset=index] = value % UInt8(10)
            carry = value // UInt8(10)
        if carry != 0:
            decimal_digits[unsafe_offset=count] = carry
            count += 1
    if count == 1:
        bytes[unsafe_offset=text.length] = 48
        text.length += 1
    else:
        var index = count - 1
        while index > 0:
            bytes[unsafe_offset=text.length] = decimal_digits[unsafe_offset=index] + UInt8(48)
            text.length += 1
            index -= 1
    bytes[unsafe_offset=text.length] = 46
    bytes[unsafe_offset=text.length + 1] = decimal_digits[unsafe_offset=0] + UInt8(48)
    text.length += 2
    return text^
