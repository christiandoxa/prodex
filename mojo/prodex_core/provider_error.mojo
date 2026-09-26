
from std.memory import Pointer


comptime PROVIDER_ERROR_CLASS_AUTH: Int64 = 0
comptime PROVIDER_ERROR_CLASS_QUOTA: Int64 = 1
comptime PROVIDER_ERROR_CLASS_RATE_LIMIT: Int64 = 2
comptime PROVIDER_ERROR_CLASS_TRANSIENT: Int64 = 3
comptime PROVIDER_ERROR_CLASS_NOT_FOUND: Int64 = 4
comptime PROVIDER_ERROR_CLASS_OTHER: Int64 = 5


def provider_error_ascii_space(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32


def provider_error_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def provider_error_space_width(address: UInt, offset: Int64, end: Int64) -> Int64:
    if address == 0 or offset < 0 or offset >= end:
        return 0
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var first = ptr[unsafe_offset=offset]
    if provider_error_ascii_space(first):
        return 1

    var remaining = end - offset
    if remaining >= 2:
        var second = ptr[unsafe_offset=offset + 1]
        if first == 194 and (second == 133 or second == 160):
            return 2
    if remaining >= 3:
        var second = ptr[unsafe_offset=offset + 1]
        var third = ptr[unsafe_offset=offset + 2]
        if (
            (first == 225 and second == 154 and third == 128)
            or (
                first == 226
                and second == 128
                and (
                    (third >= 128 and third <= 138)
                    or third == 168
                    or third == 169
                    or third == 175
                )
            )
            or (first == 226 and second == 129 and third == 159)
            or (first == 227 and second == 128 and third == 128)
        ):
            return 3
    return 0


def provider_error_trim_bounds(address: UInt, length: Int64) -> Tuple[Int64, Int64]:
    var start: Int64 = 0
    var end = length
    if address == 0 or length <= 0:
        return (start, end)
    var width = provider_error_space_width(address, start, end)
    while width > 0:
        start += width
        width = provider_error_space_width(address, start, end)
    while end > start:
        width = provider_error_space_width(address, end - 1, end)
        if width == 0 and end - start >= 2:
            width = provider_error_space_width(address, end - 2, end)
        if width == 0 and end - start >= 3:
            width = provider_error_space_width(address, end - 3, end)
        if width == 0:
            break
        end -= width
    return (start, end)


def provider_error_equals_ci(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var bounds = provider_error_trim_bounds(address, length)
    var expected = Int64(literal.byte_length())
    if bounds[1] - bounds[0] != expected:
        return False
    if expected == 0:
        return True
    if address == 0:
        return False
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = literal.unsafe_ptr()
    for index in range(expected):
        if provider_error_ascii_lower(ptr[unsafe_offset=bounds[0] + index]) != target[unsafe_offset=index]:
            return False
    return True


def provider_error_contains_ci(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var bounds = provider_error_trim_bounds(address, length)
    var expected = Int64(literal.byte_length())
    if expected == 0:
        return True
    if address == 0 or bounds[1] - bounds[0] < expected:
        return False
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = literal.unsafe_ptr()
    var start = bounds[0]
    while start <= bounds[1] - expected:
        var matched = True
        for offset in range(expected):
            if provider_error_ascii_lower(ptr[unsafe_offset=start + offset]) != target[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
        start += 1
    return False


@export("prodex_provider_error_classify_v1")
def prodex_provider_error_classify_v1(
    status: Int64,
    status_present: Int64,
    code_address: UInt,
    code_length: Int64,
    code_present: Int64,
    text_address: UInt,
    text_length: Int64,
    text_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        status_present < 0
        or status_present > 1
        or code_present < 0
        or code_present > 1
        or text_present < 0
        or text_present > 1
        or code_length < 0
        or text_length < 0
        or (code_present == 1 and code_length > 0 and code_address == 0)
        or (text_present == 1 and text_length > 0 and text_address == 0)
        or output_address == 0
    ):
        return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var classification = PROVIDER_ERROR_CLASS_OTHER
    var cooldown: Int64 = 0

    if (
        (status_present == 1 and (status == 401 or status == 403))
        or (
            code_present == 1
            and (
                provider_error_equals_ci(code_address, code_length, StringSlice("unauthenticated"))
                or provider_error_equals_ci(code_address, code_length, StringSlice("invalid_api_key"))
                or provider_error_equals_ci(code_address, code_length, StringSlice("authentication_error"))
            )
        )
    ):
        classification = PROVIDER_ERROR_CLASS_AUTH
    elif (
        code_present == 1
        and (
            provider_error_equals_ci(code_address, code_length, StringSlice("insufficient_quota"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("credit_balance_exhausted"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("organization_spend_limit_exceeded"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("project_spend_limit_exceeded"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("quota_exhausted"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("quota_exceeded"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("resource_exhausted"))
        )
    ):
        classification = PROVIDER_ERROR_CLASS_QUOTA
        cooldown = 300_000
    elif (
        code_present == 1
        and (
            provider_error_equals_ci(code_address, code_length, StringSlice("rate_limit_exceeded"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("rate_limit_exceeded_error"))
            or provider_error_equals_ci(code_address, code_length, StringSlice("slow_down"))
        )
    ):
        classification = PROVIDER_ERROR_CLASS_RATE_LIMIT
        cooldown = 60_000
    elif (
        (status_present == 1 and status == 404)
        or (
            code_present == 1
            and provider_error_equals_ci(code_address, code_length, StringSlice("model_not_supported"))
        )
        or (
            text_present == 1
            and provider_error_contains_ci(text_address, text_length, StringSlice("model is not supported"))
        )
    ):
        classification = PROVIDER_ERROR_CLASS_NOT_FOUND
    elif (
        (
            status_present == 1
            and (status == 500 or status == 502 or status == 503 or status == 504)
        )
        or (
            text_present == 1
            and provider_error_contains_ci(text_address, text_length, StringSlice("overloaded"))
        )
    ):
        classification = PROVIDER_ERROR_CLASS_TRANSIENT
        cooldown = 10_000

    output[unsafe_offset=0] = classification
    output[unsafe_offset=1] = cooldown
    return 0
