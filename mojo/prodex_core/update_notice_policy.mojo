from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime UPDATE_NOTICE_ABI_VERSION: Int64 = 1
comptime UPDATE_NOTICE_OK: Int64 = 0
comptime UPDATE_NOTICE_INVALID: Int64 = 1
comptime UPDATE_NOTICE_ABI: Int64 = 4
comptime UPDATE_NOTICE_MAX_TEXT_BYTES: Int64 = 1_048_576

comptime UPDATE_NOTICE_INSTALL_CHANNEL: Int64 = 0
comptime UPDATE_NOTICE_EMIT: Int64 = 1
comptime UPDATE_NOTICE_CACHE_FRESH: Int64 = 2


def update_notice_path_byte(value: UInt8) -> UInt8:
    return 47 if value == 92 else value


def update_notice_path_contains[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    var needle_length = Int64(literal.byte_length())
    if needle_length == 0:
        return True
    if Int64(view.len) < needle_length:
        return False
    var source = rich_view_ptr(view)
    var needle = literal.unsafe_ptr()
    for start in range(Int64(view.len) - needle_length + 1):
        var matched = True
        for index in range(needle_length):
            if update_notice_path_byte(source[unsafe_offset=start + index]) != needle[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def update_notice_path_suffix[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    var needle_length = Int64(literal.byte_length())
    if Int64(view.len) < needle_length:
        return False
    var source = rich_view_ptr(view)
    var needle = literal.unsafe_ptr()
    var start = Int64(view.len) - needle_length
    for index in range(needle_length):
        if update_notice_path_byte(source[unsafe_offset=start + index]) != needle[unsafe_offset=index]:
            return False
    return True


def update_notice_equals[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    if Int64(view.len) != Int64(literal.byte_length()):
        return False
    if view.len == 0:
        return literal.byte_length() == 0
    var source = rich_view_ptr(view)
    var literal_ptr = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if source[unsafe_offset=index] != literal_ptr[unsafe_offset=index]:
            return False
    return True


@export("prodex_update_notice_policy_v1")
def prodex_update_notice_policy_v1(
    abi_version: Int64,
    operation: Int64,
    text0_address: UInt,
    text0_length: Int64,
    text1_address: UInt,
    text1_length: Int64,
    tag0: Int64,
    flag0: Int64,
    flag1: Int64,
    signed0: Int64,
    signed1: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != UPDATE_NOTICE_ABI_VERSION:
        return UPDATE_NOTICE_ABI
    if (
        operation < UPDATE_NOTICE_INSTALL_CHANNEL
        or operation > UPDATE_NOTICE_CACHE_FRESH
        or text0_length < 0
        or text1_length < 0
        or text0_length > UPDATE_NOTICE_MAX_TEXT_BYTES
        or text1_length > UPDATE_NOTICE_MAX_TEXT_BYTES
        or (text0_length > 0 and text0_address == 0)
        or (text1_length > 0 and text1_address == 0)
        or (flag0 != 0 and flag0 != 1)
        or (flag1 != 0 and flag1 != 1)
        or output_address == 0
    ):
        return UPDATE_NOTICE_INVALID

    var text0 = ProdexRichStringView(text0_address, UInt(text0_length))
    var text1 = ProdexRichStringView(text1_address, UInt(text1_length))
    if (
        not rich_view_valid(text0, UPDATE_NOTICE_MAX_TEXT_BYTES)
        or not rich_view_valid(text1, UPDATE_NOTICE_MAX_TEXT_BYTES)
    ):
        return UPDATE_NOTICE_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = -1

    if operation == UPDATE_NOTICE_INSTALL_CHANNEL:
        # tag0: npm package-name present
        if tag0 == 1 and update_notice_equals["@christiandoxa/prodex"](text0):
            output[] = 1
            return UPDATE_NOTICE_OK
        if (
            update_notice_path_contains["/node_modules/@christiandoxa/prodex-"](text1)
            or update_notice_path_contains["/node_modules/@christiandoxa/prodex/"](text1)
        ):
            output[] = 1
            return UPDATE_NOTICE_OK
        if (
            update_notice_path_suffix["/.cargo/bin/prodex"](text1)
            or update_notice_path_suffix["/.cargo/bin/prodex.exe"](text1)
        ):
            output[] = 2
            return UPDATE_NOTICE_OK
        output[] = 0
        return UPDATE_NOTICE_OK

    if operation == UPDATE_NOTICE_EMIT:
        # tag0: 0 other, 1 doctor, 2 update, 3 quota.
        if tag0 == 2:
            output[] = 0
        elif tag0 == 1:
            output[] = Int64(flag0 == 0 and flag1 == 0)
        elif tag0 == 3:
            output[] = Int64(flag0 == 0)
        elif tag0 == 0:
            output[] = 1
        else:
            return UPDATE_NOTICE_INVALID
        return UPDATE_NOTICE_OK

    # tag0: cache TTL seconds, flag0: cached/current release sources are equal.
    if tag0 < 0:
        return UPDATE_NOTICE_INVALID
    var age: Int64 = 0
    if signed1 > 0 and signed0 < -9223372036854775808 + signed1:
        age = -9223372036854775808
    elif signed1 < 0 and signed0 > 9223372036854775807 + signed1:
        age = 9223372036854775807
    else:
        age = signed0 - signed1
    output[] = Int64(flag0 == 1 and age < tag0)
    return UPDATE_NOTICE_OK
