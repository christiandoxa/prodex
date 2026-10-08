from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PROFILE_IDENTITY_ABI_VERSION: Int64 = 1
comptime PROFILE_IDENTITY_OK: Int64 = 0
comptime PROFILE_IDENTITY_INVALID: Int64 = 1
comptime PROFILE_IDENTITY_CAPACITY: Int64 = 2
comptime PROFILE_IDENTITY_ABI: Int64 = 4

comptime PROFILE_IDENTITY_NORMALIZE_EMAIL: Int64 = 0
comptime PROFILE_IDENTITY_NORMALIZE_ACCOUNT: Int64 = 1
comptime PROFILE_IDENTITY_PROFILE_NAME: Int64 = 2
comptime PROFILE_IDENTITY_CANONICAL_KEY: Int64 = 3
comptime PROFILE_IDENTITY_VALIDATE_PROFILE_NAME: Int64 = 4
comptime PROFILE_IDENTITY_ADD_PROFILE_SOURCE: Int64 = 5
comptime PROFILE_IDENTITY_SHOULD_ACTIVATE: Int64 = 6
comptime PROFILE_IDENTITY_OTHER_EMAIL_NAME: Int64 = 7
comptime PROFILE_IDENTITY_FIND_MATCH: Int64 = 8
comptime PROFILE_IDENTITY_REMOVE_TARGETS: Int64 = 9
comptime PROFILE_IDENTITY_DELETE_HOME: Int64 = 10
comptime PROFILE_IDENTITY_SANITIZE_SLUG: Int64 = 11
comptime PROFILE_IDENTITY_TRIMMED_EQUAL: Int64 = 12
comptime PROFILE_IDENTITY_TRIMMED_CASEFOLD_EQUAL: Int64 = 13
comptime PROFILE_IDENTITY_OPTIONAL_CASEFOLD_EQUAL: Int64 = 14
comptime PROFILE_IDENTITY_OPTIONAL_CASEFOLD_WILDCARD: Int64 = 15
comptime PROFILE_IDENTITY_OPTIONAL_NONEMPTY_CASEFOLD_EQUAL: Int64 = 16
comptime PROFILE_IDENTITY_FIRST_PRESENT_SOURCE: Int64 = 17
comptime PROFILE_IDENTITY_REMOVED_ACTIVE_CHOICE: Int64 = 18
comptime PROFILE_IDENTITY_NAME_CANDIDATE: Int64 = 19

comptime PROFILE_MANAGEMENT_STATUS_ABI_VERSION: Int64 = 1
comptime PROFILE_MANAGEMENT_STATUS_OK: Int64 = 0
comptime PROFILE_MANAGEMENT_STATUS_INVALID: Int64 = 1
comptime PROFILE_MANAGEMENT_STATUS_CAPACITY: Int64 = 2
comptime PROFILE_MANAGEMENT_STATUS_ABI: Int64 = 4
comptime PROFILE_MANAGEMENT_STATUS_MAX_PROFILES: Int64 = 256

comptime PROFILE_MANAGEMENT_SCREEN_NO_ACTIVE: Int64 = 0
comptime PROFILE_MANAGEMENT_SCREEN_ONLY_PROFILE: Int64 = 1
comptime PROFILE_MANAGEMENT_SCREEN_ACTIVE: Int64 = 2

comptime PROFILE_PRIMARY_PRESENT: Int64 = 1
comptime PROFILE_SECONDARY_PRESENT: Int64 = 2
comptime PROFILE_RECORD_EMAIL_PRESENT: Int64 = 1
comptime PROFILE_RECORD_ACCOUNT_PRESENT: Int64 = 2

@fieldwise_init
struct ProfileIdentityRecord(Copyable, Movable):
    var email_address: UInt64
    var email_length: UInt64
    var account_address: UInt64
    var account_length: UInt64
    var flags: Int64

@fieldwise_init
struct ProfileRemovalRecord(Copyable, Movable):
    var name_address: UInt64
    var name_length: UInt64
    var managed: Int64


def profile_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def profile_record_email(record: ProfileIdentityRecord) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(record.email_address), UInt(record.email_length))


def profile_record_account(record: ProfileIdentityRecord) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(record.account_address), UInt(record.account_length))


def profile_management_screen_status(
    active_profile_present: Int64,
    profile_count: Int64,
) -> Int64:
    if active_profile_present == 1:
        return PROFILE_MANAGEMENT_SCREEN_ACTIVE
    if profile_count == 1:
        return PROFILE_MANAGEMENT_SCREEN_ONLY_PROFILE
    return PROFILE_MANAGEMENT_SCREEN_NO_ACTIVE


def profile_management_row_status(
    active: Int64,
    managed: Int64,
    identity_present: Int64,
) -> Int64:
    return active | (managed << 1) | (identity_present << 2)


def profile_lower_ascii(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def profile_write_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def profile_write_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    if written[] < 0 or Int64(literal.byte_length()) > capacity - written[]:
        return False
    var source = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def profile_write_normalized_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    lowercase: Bool,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end - start > capacity - written[]:
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if lowercase:
            value = profile_lower_ascii(value)
        output[unsafe_offset=written[]] = value
        written[] += 1
    return True


def profile_write_normalized(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    lowercase: Bool,
) -> Bool:
    var bounds = rich_trim_bounds(view)
    return profile_write_normalized_range(
        view, bounds[0], bounds[1], output, capacity, written, lowercase
    )


def profile_normalized_equal(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
    lowercase: Bool,
) -> Bool:
    var left_bounds = rich_trim_bounds(left)
    var right_bounds = rich_trim_bounds(right)
    var left_length = left_bounds[1] - left_bounds[0]
    var right_length = right_bounds[1] - right_bounds[0]
    if left_length != right_length:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(left_length):
        var left_value = left_ptr[unsafe_offset=left_bounds[0] + index]
        var right_value = right_ptr[unsafe_offset=right_bounds[0] + index]
        if lowercase:
            left_value = profile_lower_ascii(left_value)
            right_value = profile_lower_ascii(right_value)
        if left_value != right_value:
            return False
    return True


def profile_sanitize_slug(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    fallback: StringSlice,
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var source = rich_view_ptr(view)
    var output_start = written[]
    var index = bounds[0]
    while index < bounds[1]:
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var value: UInt8 = 45
        if width == 1:
            var byte = profile_lower_ascii(source[unsafe_offset=index])
            if (
                byte >= 97 and byte <= 122
                or byte >= 48 and byte <= 57
                or byte == 46
                or byte == 95
                or byte == 45
            ):
                value = byte
            elif byte == 64:
                value = 95
        if not profile_write_byte(output, capacity, written, value):
            return False
        index += width

    var lower = output_start
    while lower < written[]:
        var value = output[unsafe_offset=lower]
        if value != 46 and value != 95 and value != 45:
            break
        lower += 1
    var upper = written[]
    while upper > lower:
        var value = output[unsafe_offset=upper - 1]
        if value != 46 and value != 95 and value != 45:
            break
        upper -= 1

    if lower == upper:
        written[] = output_start
        return profile_write_literal(output, capacity, written, fallback)

    var length = upper - lower
    for offset in range(length):
        output[unsafe_offset=output_start + offset] = output[unsafe_offset=lower + offset]
    written[] = output_start + length
    return True


def profile_name_from_email(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return profile_sanitize_slug(
        view,
        output,
        capacity,
        written,
        StringSlice("profile"),
    )


def profile_write_positive_decimal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: Int64,
) -> Bool:
    if value <= 0:
        return False
    var divisor: Int64 = 1
    while value // divisor >= 10:
        divisor *= 10
    while divisor > 0:
        var digit = (value // divisor) % 10
        if not profile_write_byte(output, capacity, written, UInt8(digit + 48)):
            return False
        divisor //= 10
    return True


def profile_name_candidate(
    base: ProdexRichStringView,
    fallback: ProdexRichStringView,
    attempt: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var base_bounds = rich_trim_bounds(base)
    var selected = base.copy()
    if base_bounds[1] == base_bounds[0]:
        selected = fallback.copy()
    if not profile_write_normalized_range(
        selected, 0, Int64(selected.len), output, capacity, written, False
    ):
        return False
    if attempt == 0:
        return True
    return profile_write_byte(output, capacity, written, 45) and profile_write_positive_decimal(
        output, capacity, written, attempt + 1
    )


def profile_name_base_email_derived(
    view: ProdexRichStringView, end: Int64
) -> Bool:
    if end <= 0 or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    var underscore: Int64 = -1
    var cursor = end
    while cursor > 0:
        cursor -= 1
        if ptr[unsafe_offset=cursor] == 95:
            underscore = cursor
            break
    if underscore <= 0 or underscore + 1 >= end:
        return False

    var domain_start = underscore + 1
    var label_start = domain_start
    var has_dot = False
    for index in range(domain_start, end):
        var value = ptr[unsafe_offset=index]
        if value == 46:
            has_dot = True
            if (
                index == label_start
                or ptr[unsafe_offset=label_start] == 45
                or ptr[unsafe_offset=index - 1] == 45
            ):
                return False
            label_start = index + 1
        elif not (
            value >= 48 and value <= 57
            or value >= 65 and value <= 90
            or value >= 97 and value <= 122
            or value == 45
        ):
            return False

    if (
        not has_dot
        or label_start >= end
        or ptr[unsafe_offset=label_start] == 45
        or ptr[unsafe_offset=end - 1] == 45
    ):
        return False
    return True


def profile_stripped_unique_suffix_end(view: ProdexRichStringView) -> Int64:
    var end = Int64(view.len)
    var ptr = rich_view_ptr(view)
    var dash: Int64 = -1
    var cursor = end
    while cursor > 0:
        cursor -= 1
        if ptr[unsafe_offset=cursor] == 45:
            dash = cursor
            break
    if dash < 0 or dash + 1 >= end:
        return end
    for index in range(dash + 1, end):
        var value = ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            return end
    if profile_name_base_email_derived(view, dash):
        return dash
    return end


def profile_output_equals_view_range(
    output: Pointer[mut=True, UInt8, _],
    output_length: Int64,
    view: ProdexRichStringView,
    end: Int64,
) -> Bool:
    if output_length != end:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(end):
        if output[unsafe_offset=index] != ptr[unsafe_offset=index]:
            return False
    return True


def profile_validate_name(view: ProdexRichStringView) -> Int64:
    var length = Int64(view.len)
    if length == 0:
        return 1
    var ptr = rich_view_ptr(view)
    for index in range(length):
        var value = ptr[unsafe_offset=index]
        if value == 47 or value == 92:
            return 2
    if length == 1 and ptr[unsafe_offset=0] == 46:
        return 3
    if length == 2 and ptr[unsafe_offset=0] == 46 and ptr[unsafe_offset=1] == 46:
        return 3
    for index in range(length):
        var value = ptr[unsafe_offset=index]
        if not (
            value >= 48 and value <= 57
            or value >= 65 and value <= 90
            or value >= 97 and value <= 122
            or value == 45
            or value == 95
            or value == 46
        ):
            return 4
    return 0


def profile_add_source(flags: Int64) -> Int64:
    var external = (flags & 1) != 0
    var copy_from = (flags & 2) != 0
    var copy_current = (flags & 4) != 0
    if external and (copy_from or copy_current):
        return 10
    if copy_from and copy_current:
        return 11
    if external:
        return 0
    if copy_current:
        return 2
    if copy_from:
        return 1
    return 3


def profile_record_valid(record: ProfileIdentityRecord) -> Bool:
    if record.email_length > UInt64(0x7FFFFFFFFFFFFFFF) or record.account_length > UInt64(0x7FFFFFFFFFFFFFFF):
        return False
    if record.flags & PROFILE_RECORD_EMAIL_PRESENT:
        if not rich_view_valid(profile_record_email(record), Int64(record.email_length)):
            return False
    if record.flags & PROFILE_RECORD_ACCOUNT_PRESENT:
        if not rich_view_valid(profile_record_account(record), Int64(record.account_length)):
            return False
    return True


def profile_find_match(
    primary: ProdexRichStringView,
    secondary: ProdexRichStringView,
    flags: Int64,
    records_address: UInt,
    record_count: Int64,
) -> Int64:
    var records = Pointer[mut=False, ProfileIdentityRecord, ImmUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    var account_present = (flags & PROFILE_PRIMARY_PRESENT) != 0
    var email_present = (flags & PROFILE_SECONDARY_PRESENT) != 0

    if account_present:
        for index in range(record_count):
            var record = records[unsafe_offset=index].copy()
            if not profile_record_valid(record):
                return -2
            if (
                record.flags & PROFILE_RECORD_ACCOUNT_PRESENT
                and profile_normalized_equal(
                    profile_record_account(record), primary, False
                )
            ):
                if not email_present:
                    return index
                if (
                    record.flags & PROFILE_RECORD_EMAIL_PRESENT
                    and profile_normalized_equal(
                        profile_record_email(record), secondary, True
                    )
                ):
                    return index

    if not email_present:
        return -1

    var match_index: Int64 = -1
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if not profile_record_valid(record):
            return -2
        if (
            not (record.flags & PROFILE_RECORD_ACCOUNT_PRESENT)
            and record.flags & PROFILE_RECORD_EMAIL_PRESENT
            and profile_normalized_equal(
                profile_record_email(record), secondary, True
            )
        ):
            if match_index >= 0:
                return -1
            match_index = index
    return match_index



def profile_views_equal_exact(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def profile_removal_record_view(record: ProfileRemovalRecord) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(record.name_address), UInt(record.name_length))


def profile_removal_record_valid(record: ProfileRemovalRecord) -> Bool:
    if record.name_length > UInt64(0x7FFFFFFFFFFFFFFF):
        return False
    return rich_view_valid(
        profile_removal_record_view(record), Int64(record.name_length)
    )


def profile_remove_targets(
    requested: ProdexRichStringView,
    flags: Int64,
    records_address: UInt,
    record_count: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var remove_all = (flags & 1) != 0
    var requested_present = (flags & 2) != 0
    var delete_home = (flags & 4) != 0
    var records = Pointer[mut=False, ProfileRemovalRecord, ImmUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )

    if remove_all:
        if delete_home:
            var external_count: Int64 = 0
            for index in range(record_count):
                var record = records[unsafe_offset=index].copy()
                if not profile_removal_record_valid(record):
                    return -20
                if record.managed == 0:
                    if external_count > 0:
                        if not profile_write_literal(
                            output, output_capacity, written, StringSlice(", ")
                        ):
                            return -21
                    var view = profile_removal_record_view(record)
                    if not profile_write_normalized_range(
                        view,
                        0,
                        Int64(view.len),
                        output,
                        output_capacity,
                        written,
                        False,
                    ):
                        return -21
                    external_count += 1
            if external_count > 0:
                return -12
        else:
            for index in range(record_count):
                if not profile_removal_record_valid(
                    records[unsafe_offset=index].copy()
                ):
                    return -20
        return -2

    if not requested_present:
        return -10
    for index in range(record_count):
        var record = records[unsafe_offset=index].copy()
        if not profile_removal_record_valid(record):
            return -20
        if profile_views_equal_exact(
            profile_removal_record_view(record), requested
        ):
            return index
    return -11


def profile_optional_casefold_equal(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
    left_present: Bool,
    right_present: Bool,
) -> Bool:
    if left_present != right_present:
        return False
    if not left_present:
        return True
    return profile_normalized_equal(left, right, True)


def profile_optional_casefold_wildcard(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
    left_present: Bool,
    right_present: Bool,
) -> Bool:
    if not left_present or not right_present:
        return True
    return profile_normalized_equal(left, right, True)


def profile_optional_nonempty_casefold_equal(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
    left_present: Bool,
    right_present: Bool,
) -> Bool:
    var left_bounds = rich_trim_bounds(left)
    var right_bounds = rich_trim_bounds(right)
    var left_effective = left_present and left_bounds[1] > left_bounds[0]
    var right_effective = right_present and right_bounds[1] > right_bounds[0]
    if left_effective != right_effective:
        return False
    if not left_effective:
        return True
    return profile_normalized_equal(left, right, True)


@export("prodex_mojo_profile_identity_v1")
def prodex_mojo_profile_identity_v1(
    abi_version: Int64,
    operation: Int64,
    primary_address: UInt,
    primary_length: Int64,
    secondary_address: UInt,
    secondary_length: Int64,
    flags: Int64,
    records_address: UInt,
    record_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_IDENTITY_ABI_VERSION:
        return PROFILE_IDENTITY_ABI
    if (
        operation < 0
        or operation > PROFILE_IDENTITY_NAME_CANDIDATE
        or primary_length < 0
        or secondary_length < 0
        or record_count < 0
        or output_capacity < 0
        or written_address == 0
        or result_address == 0
    ):
        return PROFILE_IDENTITY_INVALID
    if (
        primary_length > 0 and primary_address == 0
        or secondary_length > 0 and secondary_address == 0
        or record_count > 0 and records_address == 0
        or output_capacity > 0 and output_address == 0
    ):
        return PROFILE_IDENTITY_INVALID

    var primary = profile_view(primary_address, primary_length)
    var secondary = profile_view(secondary_address, secondary_length)
    if (
        not rich_view_valid(primary, primary_length)
        or not rich_view_valid(secondary, secondary_length)
    ):
        return PROFILE_IDENTITY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    written[] = 0
    result[] = 0

    if operation == PROFILE_IDENTITY_NORMALIZE_EMAIL:
        if not profile_write_normalized(
            primary, output, output_capacity, written, True
        ):
            return PROFILE_IDENTITY_CAPACITY
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_NORMALIZE_ACCOUNT:
        if not profile_write_normalized(
            primary, output, output_capacity, written, False
        ):
            return PROFILE_IDENTITY_CAPACITY
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_PROFILE_NAME:
        if not profile_name_from_email(
            primary, output, output_capacity, written
        ):
            return PROFILE_IDENTITY_CAPACITY
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_NAME_CANDIDATE:
        if flags < 0 or flags == 9_223_372_036_854_775_807:
            return PROFILE_IDENTITY_INVALID
        if not profile_name_candidate(
            primary, secondary, flags, output, output_capacity, written
        ):
            return PROFILE_IDENTITY_CAPACITY
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_CANONICAL_KEY:
        var account_bounds = rich_trim_bounds(primary)
        var email_bounds = rich_trim_bounds(secondary)
        var account_present = (
            flags & PROFILE_PRIMARY_PRESENT
            and account_bounds[1] > account_bounds[0]
        )
        var email_present = (
            flags & PROFILE_SECONDARY_PRESENT
            and email_bounds[1] > email_bounds[0]
        )
        if not account_present and not email_present:
            result[] = 0
            return PROFILE_IDENTITY_OK
        if account_present:
            if not profile_write_literal(
                output, output_capacity, written, StringSlice("account:")
            ) or not profile_write_normalized_range(
                primary,
                account_bounds[0],
                account_bounds[1],
                output,
                output_capacity,
                written,
                False,
            ):
                return PROFILE_IDENTITY_CAPACITY
        if email_present:
            if account_present:
                if not profile_write_literal(
                    output, output_capacity, written, StringSlice("|email:")
                ):
                    return PROFILE_IDENTITY_CAPACITY
            elif not profile_write_literal(
                output, output_capacity, written, StringSlice("email:")
            ):
                return PROFILE_IDENTITY_CAPACITY
            if not profile_write_normalized_range(
                secondary,
                email_bounds[0],
                email_bounds[1],
                output,
                output_capacity,
                written,
                True,
            ):
                return PROFILE_IDENTITY_CAPACITY
        result[] = 1
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_VALIDATE_PROFILE_NAME:
        result[] = profile_validate_name(primary)
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_ADD_PROFILE_SOURCE:
        result[] = profile_add_source(flags)
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_SHOULD_ACTIVATE:
        result[] = Int64(not (flags & 1) or (flags & 2))
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_OTHER_EMAIL_NAME:
        if not profile_name_from_email(
            secondary, output, output_capacity, written
        ):
            return PROFILE_IDENTITY_CAPACITY
        var base_end = profile_stripped_unique_suffix_end(primary)
        var derived = profile_name_base_email_derived(primary, base_end)
        result[] = Int64(
            derived
            and not profile_output_equals_view_range(
                output, written[], primary, base_end
            )
        )
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_FIND_MATCH:
        var match_index = profile_find_match(
            primary, secondary, flags, records_address, record_count
        )
        if match_index == -2:
            return PROFILE_IDENTITY_INVALID
        result[] = match_index
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_REMOVE_TARGETS:
        var remove_result = profile_remove_targets(
            primary,
            flags,
            records_address,
            record_count,
            output,
            output_capacity,
            written,
        )
        if remove_result == -20:
            return PROFILE_IDENTITY_INVALID
        if remove_result == -21:
            return PROFILE_IDENTITY_CAPACITY
        result[] = remove_result
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_DELETE_HOME:
        var managed = (flags & 1) != 0
        var delete_home = (flags & 2) != 0
        if not delete_home:
            result[] = 0
        elif managed:
            result[] = 1
        else:
            result[] = 2
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_SANITIZE_SLUG:
        if not profile_sanitize_slug(
            primary,
            output,
            output_capacity,
            written,
            StringSlice("api_key"),
        ):
            return PROFILE_IDENTITY_CAPACITY
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_TRIMMED_EQUAL:
        result[] = Int64(profile_normalized_equal(primary, secondary, False))
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_TRIMMED_CASEFOLD_EQUAL:
        result[] = Int64(profile_normalized_equal(primary, secondary, True))
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_FIRST_PRESENT_SOURCE:
        if flags < 0 or flags > 255:
            return PROFILE_IDENTITY_INVALID
        result[] = -1
        for index in range(8):
            if flags & (Int64(1) << Int64(index)) != 0:
                result[] = Int64(index)
                break
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_REMOVED_ACTIVE_CHOICE:
        if flags < 0 or flags > 7:
            return PROFILE_IDENTITY_INVALID
        var current_present = (flags & 1) != 0
        var current_removed = (flags & 2) != 0
        var remaining_present = (flags & 4) != 0
        if current_present and not current_removed:
            result[] = 1
        elif current_removed and remaining_present:
            result[] = 2
        else:
            result[] = 0
        return PROFILE_IDENTITY_OK

    var primary_present = (flags & PROFILE_PRIMARY_PRESENT) != 0
    var secondary_present = (flags & PROFILE_SECONDARY_PRESENT) != 0
    if operation == PROFILE_IDENTITY_OPTIONAL_CASEFOLD_EQUAL:
        result[] = Int64(
            profile_optional_casefold_equal(
                primary,
                secondary,
                primary_present,
                secondary_present,
            )
        )
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_OPTIONAL_CASEFOLD_WILDCARD:
        result[] = Int64(
            profile_optional_casefold_wildcard(
                primary,
                secondary,
                primary_present,
                secondary_present,
            )
        )
        return PROFILE_IDENTITY_OK

    if operation == PROFILE_IDENTITY_OPTIONAL_NONEMPTY_CASEFOLD_EQUAL:
        result[] = Int64(
            profile_optional_nonempty_casefold_equal(
                primary,
                secondary,
                primary_present,
                secondary_present,
            )
        )
        return PROFILE_IDENTITY_OK

    return PROFILE_IDENTITY_INVALID


@export("prodex_profile_management_status_v1")
def prodex_profile_management_status_v1(
    abi_version: Int64,
    active_profile_present: Int64,
    profile_count: Int64,
    flags_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_MANAGEMENT_STATUS_ABI_VERSION:
        return PROFILE_MANAGEMENT_STATUS_ABI
    if (
        (active_profile_present != 0 and active_profile_present != 1)
        or profile_count < 0
        or profile_count > PROFILE_MANAGEMENT_STATUS_MAX_PROFILES
        or output_capacity < 0
        or written_address == 0
        or (profile_count > 0 and flags_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return PROFILE_MANAGEMENT_STATUS_INVALID

    var required_capacity = 1 + profile_count
    if output_capacity < required_capacity:
        return PROFILE_MANAGEMENT_STATUS_CAPACITY

    var flags = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(flags_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    # Validate the complete input before writing any output.
    for index in range(profile_count):
        var active = flags[unsafe_offset=index * 3]
        var managed = flags[unsafe_offset=index * 3 + 1]
        var identity_present = flags[unsafe_offset=index * 3 + 2]
        if (
            (active != 0 and active != 1)
            or (managed != 0 and managed != 1)
            or (identity_present != 0 and identity_present != 1)
        ):
            return PROFILE_MANAGEMENT_STATUS_INVALID

    output[unsafe_offset=0] = profile_management_screen_status(
        active_profile_present,
        profile_count,
    )
    for index in range(profile_count):
        var active = flags[unsafe_offset=index * 3]
        var managed = flags[unsafe_offset=index * 3 + 1]
        var identity_present = flags[unsafe_offset=index * 3 + 2]
        output[unsafe_offset=1 + index] = profile_management_row_status(
            active,
            managed,
            identity_present,
        )
    written[] = required_capacity
    return PROFILE_MANAGEMENT_STATUS_OK
