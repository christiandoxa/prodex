from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime REACTION_ABI_VERSION: Int64 = 1
comptime REACTION_OK: Int64 = 0
comptime REACTION_INVALID: Int64 = 1
comptime REACTION_CAPACITY: Int64 = 2
comptime REACTION_ABI: Int64 = 4

comptime REACTION_REDACTED = StringSlice("<redacted>")


def reaction_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def reaction_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def reaction_ascii_alphanumeric(value: UInt8) -> Bool:
    return (
        value >= 48 and value <= 57
        or value >= 65 and value <= 90
        or value >= 97 and value <= 122
    )


def reaction_ascii_alphabetic(value: UInt8) -> Bool:
    return value >= 65 and value <= 90 or value >= 97 and value <= 122


def reaction_ascii_hexdigit(value: UInt8) -> Bool:
    return (
        value >= 48 and value <= 57
        or value >= 65 and value <= 70
        or value >= 97 and value <= 102
    )


def reaction_ascii_whitespace(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32


def reaction_writer_byte(
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


def reaction_writer_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if written[] < 0 or length > capacity - written[]:
        return False
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def reaction_writer_range(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or end - start > capacity - written[]:
        return False
    for index in range(start, end):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def reaction_normalized_key_length(
    source: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    var count: Int64 = 0
    for index in range(length):
        if reaction_ascii_alphanumeric(source[unsafe_offset=index]):
            count += 1
    return count


def reaction_normalized_key_matches(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    literal: StringSlice,
) -> Bool:
    if reaction_normalized_key_length(source, length) != Int64(literal.byte_length()):
        return False
    var target = literal.unsafe_ptr()
    var target_index: Int64 = 0
    for index in range(length):
        var value = source[unsafe_offset=index]
        if reaction_ascii_alphanumeric(value):
            if reaction_ascii_lower(value) != target[unsafe_offset=target_index]:
                return False
            target_index += 1
    return True


def reaction_normalized_key_suffix(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var normalized_length = reaction_normalized_key_length(source, length)
    var target_length = Int64(literal.byte_length())
    if normalized_length < target_length:
        return False
    var skip = normalized_length - target_length
    var target = literal.unsafe_ptr()
    var normalized_index: Int64 = 0
    var target_index: Int64 = 0
    for index in range(length):
        var value = source[unsafe_offset=index]
        if reaction_ascii_alphanumeric(value):
            if normalized_index >= skip:
                if reaction_ascii_lower(value) != target[unsafe_offset=target_index]:
                    return False
                target_index += 1
            normalized_index += 1
    return target_index == target_length


def reaction_normalized_key_substring_matches(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    normalized_start: Int64,
    literal: StringSlice,
) -> Bool:
    var target = literal.unsafe_ptr()
    var target_length = Int64(literal.byte_length())
    var normalized_index: Int64 = 0
    var target_index: Int64 = 0
    for index in range(length):
        var value = source[unsafe_offset=index]
        if reaction_ascii_alphanumeric(value):
            if normalized_index >= normalized_start and target_index < target_length:
                if reaction_ascii_lower(value) != target[unsafe_offset=target_index]:
                    return False
                target_index += 1
                if target_index == target_length:
                    return True
            normalized_index += 1
    return False


def reaction_normalized_key_contains(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var normalized_length = reaction_normalized_key_length(source, length)
    var target_length = Int64(literal.byte_length())
    if target_length == 0:
        return True
    if normalized_length < target_length:
        return False
    for start in range(normalized_length - target_length + 1):
        if reaction_normalized_key_substring_matches(source, length, start, literal):
            return True
    return False


def reaction_key_sensitive_ptr(
    source: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    return (
        reaction_normalized_key_matches(source, length, StringSlice("authorization"))
        or reaction_normalized_key_matches(source, length, StringSlice("apikey"))
        or reaction_normalized_key_matches(source, length, StringSlice("xapikey"))
        or reaction_normalized_key_matches(source, length, StringSlice("authkey"))
        or reaction_normalized_key_matches(source, length, StringSlice("cookie"))
        or reaction_normalized_key_matches(source, length, StringSlice("setcookie"))
        or reaction_normalized_key_matches(source, length, StringSlice("token"))
        or reaction_normalized_key_matches(source, length, StringSlice("accesstoken"))
        or reaction_normalized_key_matches(source, length, StringSlice("refreshtoken"))
        or reaction_normalized_key_matches(source, length, StringSlice("idtoken"))
        or reaction_normalized_key_matches(source, length, StringSlice("secret"))
        or reaction_normalized_key_matches(source, length, StringSlice("password"))
        or reaction_normalized_key_matches(source, length, StringSlice("credential"))
        or reaction_normalized_key_matches(source, length, StringSlice("credentials"))
        or reaction_normalized_key_matches(source, length, StringSlice("email"))
        or reaction_normalized_key_matches(source, length, StringSlice("githublogin"))
        or reaction_normalized_key_matches(source, length, StringSlice("profilearn"))
        or reaction_normalized_key_matches(source, length, StringSlice("profilenameupstream"))
        or reaction_normalized_key_matches(source, length, StringSlice("starturl"))
        or reaction_normalized_key_matches(source, length, StringSlice("accountid"))
        or reaction_normalized_key_matches(source, length, StringSlice("chatgptaccountid"))
        or reaction_normalized_key_matches(source, length, StringSlice("proxyauthorization"))
        or reaction_normalized_key_suffix(source, length, StringSlice("token"))
        or reaction_normalized_key_contains(source, length, StringSlice("apikey"))
        or reaction_normalized_key_contains(source, length, StringSlice("secret"))
        or reaction_normalized_key_contains(source, length, StringSlice("password"))
        or reaction_normalized_key_contains(source, length, StringSlice("cookie"))
        or reaction_normalized_key_contains(source, length, StringSlice("credential"))
    )


@export("prodex_redaction_key_sensitive_v1")
def prodex_redaction_key_sensitive_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REACTION_ABI_VERSION
        or input_length < 0
        or (input_length > 0 and input_address == 0)
    ):
        return -1
    var input = reaction_view(input_address, input_length)
    if not rich_view_valid(input, input_length):
        return -1
    return Int64(reaction_key_sensitive_ptr(rich_view_ptr(input), input_length))


def reaction_starts_with_ignore_ascii_case(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    index: Int64,
    literal: StringSlice,
) -> Bool:
    var target_length = Int64(literal.byte_length())
    if index < 0 or target_length > length - index:
        return False
    var target = literal.unsafe_ptr()
    for offset in range(target_length):
        if reaction_ascii_lower(source[unsafe_offset=index + offset]) != reaction_ascii_lower(
            target[unsafe_offset=offset]
        ):
            return False
    return True


def reaction_ascii_boundary_before(
    source: Pointer[mut=False, UInt8, _], index: Int64
) -> Bool:
    return index == 0 or not reaction_ascii_alphanumeric(source[unsafe_offset=index - 1])


def reaction_ascii_boundary_after(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Bool:
    return index >= length or not reaction_ascii_alphanumeric(source[unsafe_offset=index])


def reaction_skip_ascii_whitespace(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Int64:
    var cursor = index
    while cursor < length and reaction_ascii_whitespace(source[unsafe_offset=cursor]):
        cursor += 1
    return cursor


def reaction_secret_token_end(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Int64:
    var cursor = index
    while cursor < length:
        var value = source[unsafe_offset=cursor]
        if (
            reaction_ascii_whitespace(value)
            or value == 34
            or value == 39
            or value == 44
            or value == 125
            or value == 93
            or value == 41
            or value == 59
            or value == 38
        ):
            break
        cursor += 1
    return cursor


def reaction_field_name_start(value: UInt8) -> Bool:
    return reaction_ascii_alphabetic(value) or value == 95 or value == 45


def reaction_field_name_byte(value: UInt8) -> Bool:
    return reaction_ascii_alphanumeric(value) or value == 95 or value == 45


def reaction_parse_field_name(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    index: Int64,
    key_start: Pointer[mut=True, Int64, _],
    key_end: Pointer[mut=True, Int64, _],
    after_key: Pointer[mut=True, Int64, _],
) -> Bool:
    if index < 0 or index >= length:
        return False
    var first = source[unsafe_offset=index]
    if first == 34 or first == 39:
        var cursor = index + 1
        key_start[] = cursor
        while cursor < length:
            var value = source[unsafe_offset=cursor]
            if value == first:
                key_end[] = cursor
                after_key[] = cursor + 1
                return True
            if value == 92:
                cursor += 2
            else:
                cursor += 1
        return False
    if not reaction_field_name_start(first):
        return False
    var cursor = index + 1
    while cursor < length and reaction_field_name_byte(source[unsafe_offset=cursor]):
        cursor += 1
    key_start[] = index
    key_end[] = cursor
    after_key[] = cursor
    return True


def reaction_raw_contains_cookie(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var literal = StringSlice("cookie")
    var target = literal.unsafe_ptr()
    var target_length = Int64(literal.byte_length())
    if end - start < target_length:
        return False
    for candidate in range(start, end - target_length + 1):
        var matched = True
        for offset in range(target_length):
            if reaction_ascii_lower(source[unsafe_offset=candidate + offset]) != target[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
    return False


def reaction_authorization_scheme_length(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Int64:
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("Bearer")):
        return 6
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("Basic")):
        return 5
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("Token")):
        return 5
    return 0


def reaction_sensitive_key_value_pass(
    input: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var index: Int64 = 0
    while index < length:
        var key_start: Int64 = 0
        var key_end: Int64 = 0
        var after_key: Int64 = 0
        var parsed = reaction_parse_field_name(
            source, length, index, Pointer(to=key_start), Pointer(to=key_end), Pointer(to=after_key)
        )
        if parsed:
            var separator = reaction_skip_ascii_whitespace(source, length, after_key)
            if separator < length and (
                source[unsafe_offset=separator] == 58 or source[unsafe_offset=separator] == 61
            ):
                if reaction_key_sensitive_ptr(
                    Pointer[mut=False, UInt8, ImmUntrackedOrigin](
                        unsafe_from_address=Int(input.ptr) + Int(key_start)
                    ),
                    key_end - key_start,
                ):
                    var value_start = reaction_skip_ascii_whitespace(source, length, separator + 1)
                    if not reaction_writer_range(source, index, value_start, output, capacity, written):
                        return -1
                    if reaction_raw_contains_cookie(source, key_start, key_end):
                        var value_end = value_start
                        while value_end < length and source[unsafe_offset=value_end] != 13 and source[unsafe_offset=value_end] != 10:
                            value_end += 1
                        if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                            return -1
                        index = value_end
                        continue

                    if value_start >= length:
                        if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                            return -1
                        index = value_start
                        continue
                    var first = source[unsafe_offset=value_start]
                    if first == 34 or first == 39:
                        var cursor = value_start + 1
                        while cursor < length:
                            var value = source[unsafe_offset=cursor]
                            if value == first:
                                cursor += 1
                                break
                            if value == 92:
                                cursor += 2
                            else:
                                cursor += 1
                        if not reaction_writer_byte(output, capacity, written, first):
                            return -1
                        if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                            return -1
                        if not reaction_writer_byte(output, capacity, written, first):
                            return -1
                        index = min(cursor, length)
                        continue

                    var scheme_length = reaction_authorization_scheme_length(
                        source, length, value_start
                    )
                    if (
                        scheme_length > 0
                        and value_start + scheme_length < length
                        and reaction_ascii_whitespace(
                            source[unsafe_offset=value_start + scheme_length]
                        )
                    ):
                        var token_start = reaction_skip_ascii_whitespace(
                            source, length, value_start + scheme_length
                        )
                        var token_end = reaction_secret_token_end(source, length, token_start)
                        if token_end > token_start:
                            if not reaction_writer_range(
                                source,
                                value_start,
                                value_start + scheme_length,
                                output,
                                capacity,
                                written,
                            ):
                                return -1
                            if not reaction_writer_literal(
                                output, capacity, written, StringSlice(" ")
                            ) or not reaction_writer_literal(
                                output, capacity, written, REACTION_REDACTED
                            ):
                                return -1
                            index = token_end
                            continue

                    var cursor = value_start
                    while cursor < length:
                        var value = source[unsafe_offset=cursor]
                        if (
                            reaction_ascii_whitespace(value)
                            or value == 34
                            or value == 39
                            or value == 44
                            or value == 38
                            or value == 59
                            or value == 125
                            or value == 93
                            or value == 41
                        ):
                            break
                        cursor += 1
                    if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                        return -1
                    index = cursor
                    continue
                if not reaction_writer_range(source, index, after_key, output, capacity, written):
                    return -1
                index = after_key
                continue
            if source[unsafe_offset=index] != 34 and source[unsafe_offset=index] != 39:
                if not reaction_writer_range(source, index, after_key, output, capacity, written):
                    return -1
                index = after_key
                continue
        if not reaction_writer_byte(output, capacity, written, source[unsafe_offset=index]):
            return -1
        index += 1
    return written[]


def reaction_authorization_pass(
    input: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var index: Int64 = 0
    while index < length:
        var scheme_length = reaction_authorization_scheme_length(source, length, index)
        if (
            scheme_length > 0
            and reaction_ascii_boundary_before(source, index)
            and reaction_ascii_boundary_after(source, length, index + scheme_length)
        ):
            var whitespace = reaction_skip_ascii_whitespace(source, length, index + scheme_length)
            if whitespace > index + scheme_length:
                var token_end = reaction_secret_token_end(source, length, whitespace)
                if token_end > whitespace:
                    if not reaction_writer_range(source, index, whitespace, output, capacity, written):
                        return -1
                    if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                        return -1
                    index = token_end
                    continue
        if not reaction_writer_byte(output, capacity, written, source[unsafe_offset=index]):
            return -1
        index += 1
    return written[]


def reaction_api_prefix_length(
    source: Pointer[mut=False, UInt8, _], length: Int64, index: Int64
) -> Int64:
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk-proj-")):
        return 8
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk-ant-")):
        return 7
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk-live-")):
        return 8
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk_test_")):
        return 8
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk_live_")):
        return 8
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk-")):
        return 3
    if reaction_starts_with_ignore_ascii_case(source, length, index, StringSlice("sk_")):
        return 3
    return 0


def reaction_write_api_prefix(
    prefix_length: Int64,
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    index: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    # Rust writes the canonical prefix constant selected by the first matching rule.
    if prefix_length == 8 and reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-proj-")
    ):
        return reaction_writer_literal(output, capacity, written, StringSlice("sk-proj-"))
    if prefix_length == 7:
        return reaction_writer_literal(output, capacity, written, StringSlice("sk-ant-"))
    if prefix_length == 8 and reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-live-")
    ):
        return reaction_writer_literal(output, capacity, written, StringSlice("sk-live-"))
    if prefix_length == 8 and reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk_test_")
    ):
        return reaction_writer_literal(output, capacity, written, StringSlice("sk_test_"))
    if prefix_length == 8:
        return reaction_writer_literal(output, capacity, written, StringSlice("sk_live_"))
    if reaction_starts_with_ignore_ascii_case(
        source, length, index, StringSlice("sk-")
    ):
        return reaction_writer_literal(output, capacity, written, StringSlice("sk-"))
    return reaction_writer_literal(output, capacity, written, StringSlice("sk_"))


def reaction_api_key_pass(
    input: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var index: Int64 = 0
    while index < length:
        if reaction_ascii_boundary_before(source, index):
            var prefix_length = reaction_api_prefix_length(source, length, index)
            if prefix_length > 0:
                var token_end = reaction_secret_token_end(source, length, index)
                if token_end >= index + prefix_length + 8:
                    if not reaction_write_api_prefix(
                        prefix_length, source, length, index, output, capacity, written
                    ) or not reaction_writer_literal(
                        output, capacity, written, REACTION_REDACTED
                    ):
                        return -1
                    index = token_end
                    continue
        if not reaction_writer_byte(output, capacity, written, source[unsafe_offset=index]):
            return -1
        index += 1
    return written[]


@export("prodex_redaction_secret_like_v1")
def prodex_redaction_secret_like_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    scratch_one_address: UInt,
    scratch_two_address: UInt,
    output_address: UInt,
    capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != REACTION_ABI_VERSION:
        return REACTION_ABI
    if (
        input_length < 0
        or capacity < 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
        or capacity == 0
        or scratch_one_address == 0
        or scratch_two_address == 0
        or output_address == 0
    ):
        return REACTION_INVALID
    var input = reaction_view(input_address, input_length)
    if not rich_view_valid(input, input_length):
        return REACTION_INVALID
    var scratch_one = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_one_address)
    )
    var scratch_two = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_two_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var first_length = reaction_sensitive_key_value_pass(input, scratch_one, capacity)
    if first_length < 0:
        return REACTION_CAPACITY
    var second_length = reaction_authorization_pass(
        ProdexRichStringView(UInt(scratch_one_address), UInt(first_length)),
        scratch_two,
        capacity,
    )
    if second_length < 0:
        return REACTION_CAPACITY
    var output_length = reaction_api_key_pass(
        ProdexRichStringView(UInt(scratch_two_address), UInt(second_length)),
        output,
        capacity,
    )
    if output_length < 0:
        return REACTION_CAPACITY
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = output_length
    return REACTION_OK


def reaction_token_byte(value: UInt8) -> Bool:
    return (
        reaction_ascii_alphanumeric(value)
        or value == 46
        or value == 95
        or value == 37
        or value == 43
        or value == 45
        or value == 64
    )


def reaction_email_token_should_redact(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var at: Int64 = -1
    for index in range(start, end):
        if source[unsafe_offset=index] == 64:
            at = index
            break
    if at <= start or at + 1 >= end:
        return False
    var domain_start = at + 1
    var part_start = domain_start
    var has_dot = False
    for index in range(domain_start, end):
        if source[unsafe_offset=index] == 46:
            has_dot = True
            if index == part_start:
                return False
            part_start = index + 1
    return has_dot and part_start < end


def reaction_email_pass(
    input: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var index: Int64 = 0
    while index < length:
        if reaction_token_byte(source[unsafe_offset=index]):
            var start = index
            while index < length and reaction_token_byte(source[unsafe_offset=index]):
                index += 1
            if reaction_email_token_should_redact(source, start, index):
                if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                    return -1
            elif not reaction_writer_range(source, start, index, output, capacity, written):
                return -1
            continue
        if not reaction_writer_byte(output, capacity, written, source[unsafe_offset=index]):
            return -1
        index += 1
    return written[]


def reaction_uuid_end(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var end = start + 36
    if start < 0 or end > length:
        return -1
    if (
        start > 0 and reaction_ascii_alphanumeric(source[unsafe_offset=start - 1])
        or end < length and reaction_ascii_alphanumeric(source[unsafe_offset=end])
    ):
        return -1
    for index in range(36):
        var value = source[unsafe_offset=start + Int64(index)]
        if index == 8 or index == 13 or index == 18 or index == 23:
            if value != 45:
                return -1
        elif not reaction_ascii_hexdigit(value):
            return -1
    return end


def reaction_digit_group_byte(value: UInt8) -> Bool:
    return value >= 48 and value <= 57 or value == 32 or value == 45


def reaction_digit_group_end(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    var end = start
    while end < length and reaction_digit_group_byte(source[unsafe_offset=end]):
        if end > start and reaction_uuid_end(source, length, end) >= 0:
            break
        end += 1
    return end


def reaction_long_digit_pass(
    input: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var source = rich_view_ptr(input)
    var length = Int64(input.len)
    var written_storage: Int64 = 0
    var written = Pointer(to=written_storage)
    var index: Int64 = 0
    while index < length:
        var uuid_end = reaction_uuid_end(source, length, index)
        if uuid_end >= 0:
            if not reaction_writer_range(source, index, uuid_end, output, capacity, written):
                return -1
            index = uuid_end
            continue
        if reaction_digit_group_byte(source[unsafe_offset=index]):
            var start = index
            index = reaction_digit_group_end(source, length, start)
            var digit_count: Int64 = 0
            for cursor in range(start, index):
                var value = source[unsafe_offset=cursor]
                if value >= 48 and value <= 57:
                    digit_count += 1
            if digit_count >= 13 and digit_count <= 19:
                if not reaction_writer_literal(output, capacity, written, REACTION_REDACTED):
                    return -1
            elif not reaction_writer_range(source, start, index, output, capacity, written):
                return -1
            continue
        if not reaction_writer_byte(output, capacity, written, source[unsafe_offset=index]):
            return -1
        index += 1
    return written[]


@export("prodex_redaction_gateway_extra_v1")
def prodex_redaction_gateway_extra_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    scratch_address: UInt,
    output_address: UInt,
    capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != REACTION_ABI_VERSION:
        return REACTION_ABI
    if (
        input_length < 0
        or capacity <= 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
        or scratch_address == 0
        or output_address == 0
    ):
        return REACTION_INVALID
    var input = reaction_view(input_address, input_length)
    if not rich_view_valid(input, input_length):
        return REACTION_INVALID
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var email_length = reaction_email_pass(input, scratch, capacity)
    if email_length < 0:
        return REACTION_CAPACITY
    var output_length = reaction_long_digit_pass(
        ProdexRichStringView(UInt(scratch_address), UInt(email_length)),
        output,
        capacity,
    )
    if output_length < 0:
        return REACTION_CAPACITY
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = output_length
    return REACTION_OK


comptime REACTION_JSON_INSPECT_SCHEMA_ONLY: Int64 = 0
comptime REACTION_JSON_INSPECT_DIRECT_STRINGS: Int64 = 1
comptime REACTION_JSON_INSPECT_ALL_STRINGS: Int64 = 2

comptime REACTION_JSON_SENSITIVE_NONE: Int64 = 0
comptime REACTION_JSON_SENSITIVE_PRIVATE_KEY: Int64 = 1
comptime REACTION_JSON_SENSITIVE_API_KEY: Int64 = 2
comptime REACTION_JSON_SENSITIVE_ACCESS_TOKEN: Int64 = 3
comptime REACTION_JSON_SENSITIVE_PASSWORD: Int64 = 4


def reaction_view_equals_literal(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    if Int64(view.len) != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var target = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if source[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True


def reaction_presidio_inspect_mode(view: ProdexRichStringView) -> Int64:
    if (
        reaction_view_equals_literal(view, StringSlice("arguments"))
        or reaction_view_equals_literal(view, StringSlice("output"))
    ):
        return REACTION_JSON_INSPECT_ALL_STRINGS
    if (
        reaction_view_equals_literal(view, StringSlice("content"))
        or reaction_view_equals_literal(view, StringSlice("input"))
        or reaction_view_equals_literal(view, StringSlice("instructions"))
        or reaction_view_equals_literal(view, StringSlice("prompt"))
        or reaction_view_equals_literal(view, StringSlice("text"))
    ):
        return REACTION_JSON_INSPECT_DIRECT_STRINGS
    return REACTION_JSON_INSPECT_SCHEMA_ONLY


def reaction_presidio_known_metadata(view: ProdexRichStringView) -> Bool:
    return (
        reaction_view_equals_literal(view, StringSlice("background"))
        or reaction_view_equals_literal(view, StringSlice("call_id"))
        or reaction_view_equals_literal(view, StringSlice("conversation"))
        or reaction_view_equals_literal(view, StringSlice("id"))
        or reaction_view_equals_literal(view, StringSlice("include"))
        or reaction_view_equals_literal(view, StringSlice("max_completion_tokens"))
        or reaction_view_equals_literal(view, StringSlice("max_output_tokens"))
        or reaction_view_equals_literal(view, StringSlice("model"))
        or reaction_view_equals_literal(view, StringSlice("name"))
        or reaction_view_equals_literal(view, StringSlice("parallel_tool_calls"))
        or reaction_view_equals_literal(view, StringSlice("previous_response_id"))
        or reaction_view_equals_literal(view, StringSlice("prompt_cache_key"))
        or reaction_view_equals_literal(view, StringSlice("reasoning"))
        or reaction_view_equals_literal(view, StringSlice("response_format"))
        or reaction_view_equals_literal(view, StringSlice("role"))
        or reaction_view_equals_literal(view, StringSlice("server_label"))
        or reaction_view_equals_literal(view, StringSlice("service_tier"))
        or reaction_view_equals_literal(view, StringSlice("store"))
        or reaction_view_equals_literal(view, StringSlice("stream"))
        or reaction_view_equals_literal(view, StringSlice("temperature"))
        or reaction_view_equals_literal(view, StringSlice("top_k"))
        or reaction_view_equals_literal(view, StringSlice("top_p"))
        or reaction_view_equals_literal(view, StringSlice("truncation"))
        or reaction_view_equals_literal(view, StringSlice("type"))
        or reaction_view_equals_literal(view, StringSlice("user"))
        or reaction_view_equals_literal(view, StringSlice("verbosity"))
    )


def reaction_presidio_unsupported_modality(view: ProdexRichStringView) -> Bool:
    return (
        reaction_view_equals_literal(view, StringSlice("audio"))
        or reaction_view_equals_literal(view, StringSlice("audio_url"))
        or reaction_view_equals_literal(view, StringSlice("file"))
        or reaction_view_equals_literal(view, StringSlice("image"))
        or reaction_view_equals_literal(view, StringSlice("image_url"))
        or reaction_view_equals_literal(view, StringSlice("input_audio"))
        or reaction_view_equals_literal(view, StringSlice("input_file"))
        or reaction_view_equals_literal(view, StringSlice("input_image"))
        or reaction_view_equals_literal(view, StringSlice("video"))
    )


def reaction_presidio_sensitive_kind(view: ProdexRichStringView) -> Int64:
    var source = rich_view_ptr(view)
    var length = Int64(view.len)
    if not reaction_key_sensitive_ptr(source, length):
        return REACTION_JSON_SENSITIVE_NONE
    if reaction_normalized_key_contains(
        source, length, StringSlice("privatekey")
    ):
        return REACTION_JSON_SENSITIVE_PRIVATE_KEY
    if reaction_normalized_key_contains(source, length, StringSlice("apikey")):
        return REACTION_JSON_SENSITIVE_API_KEY
    if (
        reaction_normalized_key_contains(source, length, StringSlice("token"))
        or reaction_normalized_key_matches(
            source, length, StringSlice("authorization")
        )
    ):
        return REACTION_JSON_SENSITIVE_ACCESS_TOKEN
    return REACTION_JSON_SENSITIVE_PASSWORD


@export("prodex_redaction_json_field_plan_v1")
def prodex_redaction_json_field_plan_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != REACTION_ABI_VERSION
        or input_length < 0
        or (input_length > 0 and input_address == 0)
        or output_address == 0
    ):
        return REACTION_INVALID
    var view = reaction_view(input_address, input_length)
    if not rich_view_valid(view, input_length):
        return REACTION_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = reaction_presidio_inspect_mode(view)
    output[unsafe_offset=1] = reaction_presidio_sensitive_kind(view)
    output[unsafe_offset=2] = Int64(reaction_presidio_known_metadata(view))
    output[unsafe_offset=3] = Int64(reaction_presidio_unsupported_modality(view))
    output[unsafe_offset=4] = Int64(
        reaction_view_equals_literal(view, StringSlice("tools"))
    )
    return REACTION_OK


comptime PRESIDIO_TRANSPORT_POLICY_ABI_VERSION: Int64 = 1
comptime PRESIDIO_TRANSPORT_POLICY_FAIL_CLOSED: Int64 = 0
comptime PRESIDIO_TRANSPORT_POLICY_LOCAL_REQUIRED: Int64 = 1
comptime PRESIDIO_TRANSPORT_POLICY_COVERAGE_DENIED: Int64 = 2


@export("prodex_redaction_presidio_transport_policy_v1")
def prodex_redaction_presidio_transport_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    input4: Int64,
) abi("C") -> Int64:
    if (
        abi_version != PRESIDIO_TRANSPORT_POLICY_ABI_VERSION
        or operation < PRESIDIO_TRANSPORT_POLICY_FAIL_CLOSED
        or operation > PRESIDIO_TRANSPORT_POLICY_COVERAGE_DENIED
    ):
        return -1
    for value in [input0, input1, input2, input3, input4]:
        if value != 0 and value != 1:
            return -1

    if operation == PRESIDIO_TRANSPORT_POLICY_FAIL_CLOSED:
        return Int64(
            input0 == 1
            or input1 == 1
            or input2 == 1
            or input3 == 1
            or input4 == 1
        )
    if operation == PRESIDIO_TRANSPORT_POLICY_LOCAL_REQUIRED:
        # input0 is rollout_off; all other inputs retain their positive meaning.
        return Int64(
            input0 == 0
            or input1 == 1
            or input2 == 1
            or input3 == 1
        )
    # coverage denied: input0 fail_closed, input1 coverage_full.
    return Int64(input0 == 1 and input1 == 0)
