from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime COOKIE_POLICY_ABI_VERSION: Int64 = 1
comptime COOKIE_POLICY_OK: Int64 = 0
comptime COOKIE_POLICY_INVALID: Int64 = 1
comptime COOKIE_POLICY_ABI: Int64 = 4
comptime COOKIE_POLICY_MAX_BYTES: Int64 = 1_048_576

comptime COOKIE_PAIR_SET_COOKIE: Int64 = 0
comptime COOKIE_PAIR_CALLER_NAME: Int64 = 1

comptime COOKIE_ATTR_IGNORE: Int64 = 0
comptime COOKIE_ATTR_SECURE: Int64 = 1
comptime COOKIE_ATTR_PATH: Int64 = 2
comptime COOKIE_ATTR_MAX_AGE: Int64 = 3
comptime COOKIE_ATTR_EXPIRES: Int64 = 4
comptime COOKIE_POLICY_MAX_EVICTION_TIMESTAMPS: Int64 = 4_128


def cookie_ascii_lower(byte: UInt8) -> UInt8:
    if byte >= 65 and byte <= 90:
        return byte + 32
    return byte


def cookie_subview(
    base: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> ProdexRichStringView:
    return ProdexRichStringView(
        base.ptr + UInt(start),
        UInt(end - start),
    )


def cookie_trim_range(
    base: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Int64, Int64]:
    if start < 0 or end < start or end > Int64(base.len):
        return (Int64(-1), Int64(-1))
    var view = cookie_subview(base, start, end)
    var bounds = rich_trim_bounds(view)
    return (start + bounds[0], start + bounds[1])


def cookie_find_byte(
    view: ProdexRichStringView,
    byte: UInt8,
    start: Int64,
    end: Int64,
) -> Int64:
    if start < 0 or end < start or end > Int64(view.len):
        return -1
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if ptr[unsafe_offset=index] == byte:
            return index
    return -1


def cookie_casefold_equals_literal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var wanted_len = Int64(literal.byte_length())
    if end - start != wanted_len:
        return False
    var ptr = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(wanted_len):
        if cookie_ascii_lower(ptr[unsafe_offset=start + index]) != expected[unsafe_offset=index]:
            return False
    return True


def cookie_exact_equals_literal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var wanted_len = Int64(literal.byte_length())
    if end - start != wanted_len:
        return False
    var ptr = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(wanted_len):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def cookie_timestamp_is_earlier(
    candidate_after_epoch: UInt64,
    candidate_seconds: UInt64,
    candidate_nanoseconds: UInt64,
    selected_after_epoch: UInt64,
    selected_seconds: UInt64,
    selected_nanoseconds: UInt64,
) -> Bool:
    if candidate_after_epoch != selected_after_epoch:
        return candidate_after_epoch == 0
    if candidate_seconds != selected_seconds:
        if candidate_after_epoch == 0:
            return candidate_seconds > selected_seconds
        return candidate_seconds < selected_seconds
    if candidate_after_epoch == 0:
        return candidate_nanoseconds > selected_nanoseconds
    return candidate_nanoseconds < selected_nanoseconds


def cookie_name_byte_safe(byte: UInt8) -> Bool:
    return (
        byte == 33
        or byte >= 35 and byte <= 39
        or byte >= 42 and byte <= 43
        or byte >= 45 and byte <= 46
        or byte >= 48 and byte <= 57
        or byte >= 65 and byte <= 90
        or byte >= 94 and byte <= 122
        or byte == 124
        or byte == 126
    )


def cookie_value_byte_safe(byte: UInt8) -> Bool:
    return (
        byte == 0x21
        or byte >= 0x23 and byte <= 0x2B
        or byte >= 0x2D and byte <= 0x3A
        or byte >= 0x3C and byte <= 0x5B
        or byte >= 0x5D and byte <= 0x7E
    )


def cookie_name_range_safe(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    maximum: Int64,
) -> Bool:
    var length = end - start
    if length <= 0 or maximum < 0 or length > maximum:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not cookie_name_byte_safe(ptr[unsafe_offset=index]):
            return False
    return True


def cookie_value_range_safe(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    maximum: Int64,
) -> Bool:
    var length = end - start
    if length < 0 or maximum < 0 or length > maximum:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not cookie_value_byte_safe(ptr[unsafe_offset=index]):
            return False
    return True


@export("prodex_runtime_cookie_pair_plan_v1")
def prodex_runtime_cookie_pair_plan_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    max_name_bytes: Int64,
    max_value_bytes: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return COOKIE_POLICY_ABI
    if (
        mode < COOKIE_PAIR_SET_COOKIE
        or mode > COOKIE_PAIR_CALLER_NAME
        or input_length < 0
        or input_length > COOKIE_POLICY_MAX_BYTES
        or max_name_bytes < 0
        or max_value_bytes < 0
        or (input_length > 0 and input_address == 0)
        or output_address == 0
    ):
        return COOKIE_POLICY_INVALID

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, COOKIE_POLICY_MAX_BYTES):
        return COOKIE_POLICY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(5):
        output[unsafe_offset=index] = 0

    var equals = cookie_find_byte(view, UInt8(61), 0, input_length)
    if equals < 0:
        return COOKIE_POLICY_OK

    var name_bounds = cookie_trim_range(view, 0, equals)
    if not cookie_name_range_safe(
        view, name_bounds[0], name_bounds[1], max_name_bytes
    ):
        return COOKIE_POLICY_OK

    output[unsafe_offset=0] = 1
    output[unsafe_offset=1] = name_bounds[0]
    output[unsafe_offset=2] = name_bounds[1]
    if mode == COOKIE_PAIR_CALLER_NAME:
        return COOKIE_POLICY_OK

    var value_bounds = cookie_trim_range(view, equals + 1, input_length)
    if not cookie_value_range_safe(
        view, value_bounds[0], value_bounds[1], max_value_bytes
    ):
        output[unsafe_offset=0] = 0
        return COOKIE_POLICY_OK
    output[unsafe_offset=3] = value_bounds[0]
    output[unsafe_offset=4] = value_bounds[1]
    return COOKIE_POLICY_OK


def cookie_parse_i64(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Bool, Int64]:
    if start >= end:
        return (False, Int64(0))
    var ptr = rich_view_ptr(view)
    var negative = ptr[unsafe_offset=start] == 45
    var cursor = start + Int64(negative)
    if cursor >= end:
        return (False, Int64(0))

    var magnitude: UInt64 = 0
    var positive_limit = UInt64(9_223_372_036_854_775_807)
    var negative_limit = UInt64(9_223_372_036_854_775_808)
    var limit = negative_limit if negative else positive_limit
    for index in range(cursor, end):
        var byte = ptr[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return (False, Int64(0))
        var digit = UInt64(byte - 48)
        if magnitude > limit // UInt64(10) or (
            magnitude == limit // UInt64(10)
            and digit > limit % UInt64(10)
        ):
            return (False, Int64(0))
        magnitude = magnitude * UInt64(10) + digit

    if negative:
        if magnitude == negative_limit:
            return (True, -9_223_372_036_854_775_808)
        return (True, -Int64(magnitude))
    return (True, Int64(magnitude))


@export("prodex_runtime_cookie_attribute_plan_v1")
def prodex_runtime_cookie_attribute_plan_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    max_path_bytes: Int64,
    max_age_seen: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return COOKIE_POLICY_ABI
    if (
        input_length < 0
        or input_length > COOKIE_POLICY_MAX_BYTES
        or max_path_bytes < 0
        or (max_age_seen != 0 and max_age_seen != 1)
        or (input_length > 0 and input_address == 0)
        or output_address == 0
    ):
        return COOKIE_POLICY_INVALID

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, COOKIE_POLICY_MAX_BYTES):
        return COOKIE_POLICY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    var whole = cookie_trim_range(view, 0, input_length)
    if cookie_casefold_equals_literal(
        view, whole[0], whole[1], StringSlice("secure")
    ):
        output[unsafe_offset=0] = COOKIE_ATTR_SECURE
        return COOKIE_POLICY_OK

    var equals = cookie_find_byte(view, UInt8(61), whole[0], whole[1])
    if equals < 0:
        return COOKIE_POLICY_OK
    var name_bounds = cookie_trim_range(view, whole[0], equals)
    var value_bounds = cookie_trim_range(view, equals + 1, whole[1])

    if cookie_casefold_equals_literal(
        view, name_bounds[0], name_bounds[1], StringSlice("path")
    ):
        var value_len = value_bounds[1] - value_bounds[0]
        if value_len <= 0 or value_len > max_path_bytes:
            return COOKIE_POLICY_OK
        var ptr = rich_view_ptr(view)
        if ptr[unsafe_offset=value_bounds[0]] != 47:
            return COOKIE_POLICY_OK
        for index in range(value_bounds[0], value_bounds[1]):
            var byte = ptr[unsafe_offset=index]
            if byte == 13 or byte == 10:
                return COOKIE_POLICY_OK
        output[unsafe_offset=0] = COOKIE_ATTR_PATH
        output[unsafe_offset=1] = value_bounds[0]
        output[unsafe_offset=2] = value_bounds[1]
        return COOKIE_POLICY_OK

    if cookie_casefold_equals_literal(
        view, name_bounds[0], name_bounds[1], StringSlice("max-age")
    ):
        var parsed = cookie_parse_i64(view, value_bounds[0], value_bounds[1])
        if parsed[0]:
            output[unsafe_offset=0] = COOKIE_ATTR_MAX_AGE
            output[unsafe_offset=3] = parsed[1]
        return COOKIE_POLICY_OK

    if (
        max_age_seen == 0
        and cookie_casefold_equals_literal(
            view, name_bounds[0], name_bounds[1], StringSlice("expires")
        )
    ):
        output[unsafe_offset=0] = COOKIE_ATTR_EXPIRES
        output[unsafe_offset=1] = value_bounds[0]
        output[unsafe_offset=2] = value_bounds[1]
    return COOKIE_POLICY_OK


@export("prodex_runtime_cookie_default_path_v1")
def prodex_runtime_cookie_default_path_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return COOKIE_POLICY_ABI
    if (
        input_length < 0
        or input_length > COOKIE_POLICY_MAX_BYTES
        or (input_length > 0 and input_address == 0)
        or output_address == 0
    ):
        return COOKIE_POLICY_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, COOKIE_POLICY_MAX_BYTES):
        return COOKIE_POLICY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    if input_length <= 0:
        return COOKIE_POLICY_OK
    var ptr = rich_view_ptr(view)
    if ptr[unsafe_offset=0] != 47:
        return COOKIE_POLICY_OK

    var last: Int64 = -1
    for index in range(input_length):
        if ptr[unsafe_offset=index] == 47:
            last = index
    if last <= 0:
        return COOKIE_POLICY_OK
    output[unsafe_offset=0] = 1
    output[unsafe_offset=1] = last
    return COOKIE_POLICY_OK


@export("prodex_runtime_cookie_path_matches_v1")
def prodex_runtime_cookie_path_matches_v1(
    abi_version: Int64,
    request_address: UInt,
    request_length: Int64,
    cookie_address: UInt,
    cookie_length: Int64,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return -4
    if (
        request_length < 0
        or cookie_length < 0
        or request_length > COOKIE_POLICY_MAX_BYTES
        or cookie_length > COOKIE_POLICY_MAX_BYTES
        or (request_length > 0 and request_address == 0)
        or (cookie_length > 0 and cookie_address == 0)
    ):
        return -2
    var request = ProdexRichStringView(request_address, UInt(request_length))
    var cookie = ProdexRichStringView(cookie_address, UInt(cookie_length))
    if (
        not rich_view_valid(request, COOKIE_POLICY_MAX_BYTES)
        or not rich_view_valid(cookie, COOKIE_POLICY_MAX_BYTES)
    ):
        return -2
    if request_length < cookie_length:
        return 0
    var request_ptr = rich_view_ptr(request)
    var cookie_ptr = rich_view_ptr(cookie)
    for index in range(cookie_length):
        if request_ptr[unsafe_offset=index] != cookie_ptr[unsafe_offset=index]:
            return 0
    if request_length == cookie_length:
        return 1
    if cookie_length > 0 and cookie_ptr[unsafe_offset=cookie_length - 1] == 47:
        return 1
    return Int64(request_ptr[unsafe_offset=cookie_length] == 47)


@export("prodex_runtime_cookie_scheme_secure_v1")
def prodex_runtime_cookie_scheme_secure_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return -4
    if (
        input_length < 0
        or input_length > COOKIE_POLICY_MAX_BYTES
        or (input_length > 0 and input_address == 0)
    ):
        return -2
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, COOKIE_POLICY_MAX_BYTES):
        return -2
    return Int64(
        cookie_exact_equals_literal(view, 0, input_length, StringSlice("https"))
        or cookie_exact_equals_literal(view, 0, input_length, StringSlice("wss"))
    )


@export("prodex_runtime_cookie_host_normalize_v1")
def prodex_runtime_cookie_host_normalize_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return COOKIE_POLICY_ABI
    if (
        input_length < 0
        or input_length > COOKIE_POLICY_MAX_BYTES
        or output_capacity < 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return COOKIE_POLICY_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, COOKIE_POLICY_MAX_BYTES):
        return COOKIE_POLICY_INVALID
    var bounds = rich_trim_bounds(view)
    var ptr = rich_view_ptr(view)
    var start = bounds[0]
    var end = bounds[1]
    while start < end and ptr[unsafe_offset=start] == 46:
        start += 1
    while end > start and ptr[unsafe_offset=end - 1] == 46:
        end -= 1

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if start == end:
        return COOKIE_POLICY_OK
    var length = end - start
    if length > output_capacity:
        return COOKIE_POLICY_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(length):
        output[unsafe_offset=index] = cookie_ascii_lower(
            ptr[unsafe_offset=start + index]
        )
    written[] = length
    return COOKIE_POLICY_OK


@export("prodex_runtime_cookie_oldest_timestamp_index_v1")
def prodex_runtime_cookie_oldest_timestamp_index_v1(
    abi_version: Int64,
    timestamps_address: UInt,
    count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != COOKIE_POLICY_ABI_VERSION:
        return COOKIE_POLICY_ABI
    if (
        count < 0
        or count > COOKIE_POLICY_MAX_EVICTION_TIMESTAMPS
        or (count > 0 and timestamps_address == 0)
        or output_address == 0
    ):
        return COOKIE_POLICY_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = -1
    if count == 0:
        return COOKIE_POLICY_OK

    var timestamps = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(timestamps_address)
    )
    var selected_index: Int64 = -1
    var selected_after_epoch: UInt64 = 0
    var selected_seconds: UInt64 = 0
    var selected_nanoseconds: UInt64 = 0
    for index in range(count):
        var offset = index * 3
        var after_epoch = timestamps[unsafe_offset=offset]
        var seconds = timestamps[unsafe_offset=offset + 1]
        var nanoseconds = timestamps[unsafe_offset=offset + 2]
        if after_epoch > 1 or nanoseconds >= UInt64(1_000_000_000):
            return COOKIE_POLICY_INVALID
        if selected_index < 0 or cookie_timestamp_is_earlier(
            after_epoch,
            seconds,
            nanoseconds,
            selected_after_epoch,
            selected_seconds,
            selected_nanoseconds,
        ):
            selected_index = index
            selected_after_epoch = after_epoch
            selected_seconds = seconds
            selected_nanoseconds = nanoseconds

    # Strict comparison keeps the first candidate when timestamps tie.
    output[] = selected_index
    return COOKIE_POLICY_OK
