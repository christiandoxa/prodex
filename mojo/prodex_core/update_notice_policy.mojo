from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime UPDATE_NOTICE_ABI_VERSION: Int64 = 1
comptime UPDATE_NOTICE_OK: Int64 = 0
comptime UPDATE_NOTICE_INVALID: Int64 = 1
comptime UPDATE_NOTICE_ABI: Int64 = 4
comptime UPDATE_NOTICE_MAX_TEXT_BYTES: Int64 = 1_048_576

comptime UPDATE_NOTICE_INSTALL_CHANNEL: Int64 = 0
comptime UPDATE_NOTICE_EMIT: Int64 = 1
comptime UPDATE_NOTICE_CACHE_FRESH: Int64 = 2
comptime UPDATE_NOTICE_RELEASE_VERSION_VALID: Int64 = 3
comptime UPDATE_NOTICE_RELEASE_VERSION_COMPARE: Int64 = 4
comptime UPDATE_NOTICE_UPDATE_DECISION: Int64 = 5

comptime UPDATE_NOTICE_RELEASE_VERSION_TOTAL_ORDER: Int64 = 0
comptime UPDATE_NOTICE_RELEASE_VERSION_PRECEDENCE: Int64 = 1
comptime UPDATE_NOTICE_RELEASE_VERSION_INVALID: Int64 = -2


@fieldwise_init
struct UpdateNoticeReleaseVersion(Copyable):
    var valid: Bool
    var major: UInt64
    var minor: UInt64
    var patch: UInt64
    var pre_start: Int64
    var pre_end: Int64
    var build_start: Int64
    var build_end: Int64
    var has_pre: Bool


def update_notice_invalid_release_version() -> UpdateNoticeReleaseVersion:
    return UpdateNoticeReleaseVersion(
        False,
        UInt64(0),
        UInt64(0),
        UInt64(0),
        Int64(0),
        Int64(0),
        Int64(0),
        Int64(0),
        False,
    )


def update_notice_release_numeric(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Tuple[Bool, UInt64, Int64]:
    if start >= end:
        return (False, UInt64(0), start)
    var first = ptr[unsafe_offset=start]
    if first < 48 or first > 57:
        return (False, UInt64(0), start)
    var value = UInt64(0)
    var index = start
    while index < end:
        var digit = ptr[unsafe_offset=index]
        if digit < 48 or digit > 57:
            break
        if index > start and first == 48:
            return (False, UInt64(0), index)
        var next_digit = UInt64(digit - 48)
        if value > (UInt64(0xFFFFFFFFFFFFFFFF) - next_digit) / UInt64(10):
            return (False, UInt64(0), index)
        value = value * UInt64(10) + next_digit
        index += 1
    return (True, value, index)


def update_notice_release_identifier_list(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    prerelease: Bool,
) -> Tuple[Bool, Int64]:
    if start >= end:
        return (False, start)
    var index = start
    var segment_start = start
    var segment_has_nondigit = False
    while index < end:
        var value = ptr[unsafe_offset=index]
        if (
            (value >= 48 and value <= 57)
            or (value >= 65 and value <= 90)
            or (value >= 97 and value <= 122)
            or value == 45
        ):
            if value < 48 or value > 57:
                segment_has_nondigit = True
            index += 1
            continue
        if value == 46:
            if index == segment_start:
                return (False, index)
            if (
                prerelease
                and not segment_has_nondigit
                and index - segment_start > 1
                and ptr[unsafe_offset=segment_start] == 48
            ):
                return (False, index)
            segment_start = index + 1
            segment_has_nondigit = False
            index += 1
            continue
        if value == 43 and prerelease:
            if index == segment_start:
                return (False, index)
            if (
                not segment_has_nondigit
                and index - segment_start > 1
                and ptr[unsafe_offset=segment_start] == 48
            ):
                return (False, index)
            return (True, index)
        return (False, index)
    if index == segment_start:
        return (False, index)
    if (
        prerelease
        and not segment_has_nondigit
        and index - segment_start > 1
        and ptr[unsafe_offset=segment_start] == 48
    ):
        return (False, index)
    return (True, index)


def update_notice_parse_release_version(
    view: ProdexRichStringView,
) -> UpdateNoticeReleaseVersion:
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    if start >= end:
        return update_notice_invalid_release_version()
    var ptr = rich_view_ptr(view)
    if ptr[unsafe_offset=start] == 118:
        start += 1
        if start >= end:
            return update_notice_invalid_release_version()

    var major_result = update_notice_release_numeric(ptr, start, end)
    if not major_result[0] or major_result[2] >= end:
        return update_notice_invalid_release_version()
    if ptr[unsafe_offset=major_result[2]] != 46:
        return update_notice_invalid_release_version()
    var minor_result = update_notice_release_numeric(
        ptr, major_result[2] + 1, end
    )
    if not minor_result[0] or minor_result[2] >= end:
        return update_notice_invalid_release_version()
    if ptr[unsafe_offset=minor_result[2]] != 46:
        return update_notice_invalid_release_version()
    var patch_result = update_notice_release_numeric(
        ptr, minor_result[2] + 1, end
    )
    if not patch_result[0]:
        return update_notice_invalid_release_version()

    var index = patch_result[2]
    var pre_start: Int64 = 0
    var pre_end: Int64 = 0
    var has_pre = False
    var build_start: Int64 = 0
    var build_end: Int64 = 0
    if index < end and ptr[unsafe_offset=index] == 45:
        has_pre = True
        pre_start = index + 1
        var pre_result = update_notice_release_identifier_list(
            ptr, pre_start, end, True
        )
        if not pre_result[0]:
            return update_notice_invalid_release_version()
        pre_end = pre_result[1]
        index = pre_end
    if index < end and ptr[unsafe_offset=index] == 43:
        build_start = index + 1
        var build_result = update_notice_release_identifier_list(
            ptr, build_start, end, False
        )
        if not build_result[0] or build_result[1] != end:
            return update_notice_invalid_release_version()
        build_end = end
        index = end
    if index != end:
        return update_notice_invalid_release_version()
    return UpdateNoticeReleaseVersion(
        True,
        major_result[1],
        minor_result[1],
        patch_result[1],
        pre_start,
        pre_end,
        build_start,
        build_end,
        has_pre,
    )


def update_notice_release_identifier_compare(
    left_ptr: Pointer[mut=False, UInt8, _],
    left_start: Int64,
    left_end: Int64,
    right_ptr: Pointer[mut=False, UInt8, _],
    right_start: Int64,
    right_end: Int64,
    build_metadata: Bool,
) -> Int64:
    var left_numeric = True
    for index in range(left_start, left_end):
        var value = left_ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            left_numeric = False
            break
    var right_numeric = True
    for index in range(right_start, right_end):
        var value = right_ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            right_numeric = False
            break
    if left_numeric and not right_numeric:
        return -1
    if not left_numeric and right_numeric:
        return 1

    var left_compare_start = left_start
    var right_compare_start = right_start
    if left_numeric and right_numeric and build_metadata:
        while (
            left_compare_start < left_end
            and left_ptr[unsafe_offset=left_compare_start] == 48
        ):
            left_compare_start += 1
        while (
            right_compare_start < right_end
            and right_ptr[unsafe_offset=right_compare_start] == 48
        ):
            right_compare_start += 1
        var left_significant_length = left_end - left_compare_start
        var right_significant_length = right_end - right_compare_start
        if left_significant_length < right_significant_length:
            return -1
        if left_significant_length > right_significant_length:
            return 1
    elif left_numeric and right_numeric:
        var left_length = left_end - left_start
        var right_length = right_end - right_start
        if left_length < right_length:
            return -1
        if left_length > right_length:
            return 1
    else:
        left_compare_start = left_start
        right_compare_start = right_start

    var left_index = left_compare_start
    var right_index = right_compare_start
    while left_index < left_end and right_index < right_end:
        var left_value = left_ptr[unsafe_offset=left_index]
        var right_value = right_ptr[unsafe_offset=right_index]
        if left_value < right_value:
            return -1
        if left_value > right_value:
            return 1
        left_index += 1
        right_index += 1
    if left_numeric and right_numeric and build_metadata:
        var left_original_length = left_end - left_start
        var right_original_length = right_end - right_start
        if left_original_length < right_original_length:
            return -1
        if left_original_length > right_original_length:
            return 1
        return 0
    if left_index < left_end:
        return 1
    if right_index < right_end:
        return -1
    return 0


def update_notice_release_identifier_list_compare(
    left_ptr: Pointer[mut=False, UInt8, _],
    left_start: Int64,
    left_end: Int64,
    right_ptr: Pointer[mut=False, UInt8, _],
    right_start: Int64,
    right_end: Int64,
    build_metadata: Bool,
) -> Int64:
    var left_index = left_start
    var right_index = right_start
    while True:
        var left_segment_end = left_index
        while (
            left_segment_end < left_end
            and left_ptr[unsafe_offset=left_segment_end] != 46
        ):
            left_segment_end += 1
        var right_segment_end = right_index
        while (
            right_segment_end < right_end
            and right_ptr[unsafe_offset=right_segment_end] != 46
        ):
            right_segment_end += 1
        var ordering = update_notice_release_identifier_compare(
            left_ptr,
            left_index,
            left_segment_end,
            right_ptr,
            right_index,
            right_segment_end,
            build_metadata,
        )
        if ordering != 0:
            return ordering
        var left_more = left_segment_end < left_end
        var right_more = right_segment_end < right_end
        if not left_more:
            return Int64(right_more) * -1
        if not right_more:
            return 1
        left_index = left_segment_end + 1
        right_index = right_segment_end + 1


def update_notice_release_version_compare(
    left_view: ProdexRichStringView,
    right_view: ProdexRichStringView,
    include_build_metadata: Bool,
) -> Int64:
    var left = update_notice_parse_release_version(left_view)
    var right = update_notice_parse_release_version(right_view)
    if not left.valid or not right.valid:
        return UPDATE_NOTICE_RELEASE_VERSION_INVALID
    if left.major < right.major:
        return -1
    if left.major > right.major:
        return 1
    if left.minor < right.minor:
        return -1
    if left.minor > right.minor:
        return 1
    if left.patch < right.patch:
        return -1
    if left.patch > right.patch:
        return 1
    if left.has_pre and not right.has_pre:
        return -1
    if not left.has_pre and right.has_pre:
        return 1
    if left.has_pre:
        var pre_order = update_notice_release_identifier_list_compare(
            rich_view_ptr(left_view),
            left.pre_start,
            left.pre_end,
            rich_view_ptr(right_view),
            right.pre_start,
            right.pre_end,
            False,
        )
        if pre_order != 0:
            return pre_order
    if include_build_metadata:
        var left_build_length = left.build_end - left.build_start
        var right_build_length = right.build_end - right.build_start
        if left_build_length == 0 and right_build_length != 0:
            return -1
        if left_build_length != 0 and right_build_length == 0:
            return 1
        if left_build_length != 0:
            var build_order = update_notice_release_identifier_list_compare(
                rich_view_ptr(left_view),
                left.build_start,
                left.build_end,
                rich_view_ptr(right_view),
                right.build_start,
                right.build_end,
                True,
            )
            if build_order != 0:
                return build_order
    return 0


def update_notice_path_byte(value: UInt8) -> UInt8:
    return 47 if value == 92 else value


def update_notice_path_contains[
    literal: StaticString
](view: ProdexRichStringView,) -> Bool:
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
            if (
                update_notice_path_byte(source[unsafe_offset=start + index])
                != needle[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def update_notice_path_suffix[
    literal: StaticString
](view: ProdexRichStringView,) -> Bool:
    var needle_length = Int64(literal.byte_length())
    if Int64(view.len) < needle_length:
        return False
    var source = rich_view_ptr(view)
    var needle = literal.unsafe_ptr()
    var start = Int64(view.len) - needle_length
    for index in range(needle_length):
        if (
            update_notice_path_byte(source[unsafe_offset=start + index])
            != needle[unsafe_offset=index]
        ):
            return False
    return True


def update_notice_equals[
    literal: StaticString
](view: ProdexRichStringView,) -> Bool:
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
        or operation > UPDATE_NOTICE_UPDATE_DECISION
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
    if not rich_view_valid(
        text0, UPDATE_NOTICE_MAX_TEXT_BYTES
    ) or not rich_view_valid(text1, UPDATE_NOTICE_MAX_TEXT_BYTES):
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
        if update_notice_path_contains["/node_modules/@christiandoxa/prodex-"](
            text1
        ) or update_notice_path_contains[
            "/node_modules/@christiandoxa/prodex/"
        ](
            text1
        ):
            output[] = 1
            return UPDATE_NOTICE_OK
        if update_notice_path_suffix["/.cargo/bin/prodex"](
            text1
        ) or update_notice_path_suffix["/.cargo/bin/prodex.exe"](text1):
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

    if operation == UPDATE_NOTICE_RELEASE_VERSION_VALID:
        output[] = Int64(update_notice_parse_release_version(text0).valid)
        return UPDATE_NOTICE_OK

    if operation == UPDATE_NOTICE_UPDATE_DECISION:
        var ordering = update_notice_release_version_compare(text0, text1, False)
        if ordering == UPDATE_NOTICE_RELEASE_VERSION_INVALID:
            return UPDATE_NOTICE_INVALID
        output[] = ordering + 1
        return UPDATE_NOTICE_OK

    if operation == UPDATE_NOTICE_RELEASE_VERSION_COMPARE:
        if (
            tag0 != UPDATE_NOTICE_RELEASE_VERSION_TOTAL_ORDER
            and tag0 != UPDATE_NOTICE_RELEASE_VERSION_PRECEDENCE
        ):
            return UPDATE_NOTICE_INVALID
        output[] = update_notice_release_version_compare(
            text0,
            text1,
            tag0 == UPDATE_NOTICE_RELEASE_VERSION_TOTAL_ORDER,
        )
        return UPDATE_NOTICE_OK

    # tag0: cache TTL seconds, flag0: cached/current release sources are equal.
    if operation != UPDATE_NOTICE_CACHE_FRESH:
        return UPDATE_NOTICE_INVALID
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
