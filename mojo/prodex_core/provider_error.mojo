
from std.memory import Pointer
from parsed_json import (
    JSON_ARRAY, JSON_OBJECT, JSON_STRING, ParsedJson, ParsedJsonNode, pj_valid,
)
from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr


comptime PROVIDER_ERROR_CLASS_AUTH: Int64 = 0
comptime PROVIDER_ERROR_CLASS_QUOTA: Int64 = 1
comptime PROVIDER_ERROR_CLASS_RATE_LIMIT: Int64 = 2
comptime PROVIDER_ERROR_CLASS_TRANSIENT: Int64 = 3
comptime PROVIDER_ERROR_CLASS_NOT_FOUND: Int64 = 4
comptime PROVIDER_ERROR_CLASS_OTHER: Int64 = 5
comptime PROVIDER_ERROR_REJECTION_ABI: Int64 = 1
comptime PROVIDER_ERROR_REJECTION_MAX_NODES: Int64 = 65537
comptime PROVIDER_ERROR_REJECTION_MAX_RAW_BYTES: Int64 = 1048576
comptime PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES: Int64 = 65536


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


def provider_error_ascii_alphanumeric(value: UInt8) -> Bool:
    return (
        (value >= 48 and value <= 57)
        or (value >= 65 and value <= 90)
        or (value >= 97 and value <= 122)
    )


def provider_error_normalize_member(
    member: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    prefix: Pointer[mut=True, Int64, MutUntrackedOrigin],
) -> Int64:
    var input = rich_view_ptr(member)
    var length: Int64 = 0
    for index in range(Int64(member.len)):
        var value = input[unsafe_offset=index]
        if provider_error_ascii_alphanumeric(value):
            output[unsafe_offset=length] = provider_error_ascii_lower(value)
            length += 1

    if length > 0:
        prefix[unsafe_offset=0] = 0
        var matched: Int64 = 0
        for index in range(Int64(1), length):
            while matched > 0 and output[unsafe_offset=index] != output[unsafe_offset=matched]:
                matched = prefix[unsafe_offset=matched - 1]
            if output[unsafe_offset=index] == output[unsafe_offset=matched]:
                matched += 1
            prefix[unsafe_offset=index] = matched
    return length


def provider_error_view_mentions_member(
    view: ProdexRichStringView,
    needle: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    prefix: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    needle_length: Int64,
) -> Bool:
    if needle_length <= 0 or view.len == 0:
        return False
    var input = rich_view_ptr(view)
    var matched: Int64 = 0
    for index in range(Int64(view.len)):
        var value = input[unsafe_offset=index]
        if not provider_error_ascii_alphanumeric(value):
            continue
        value = provider_error_ascii_lower(value)
        while matched > 0 and value != needle[unsafe_offset=matched]:
            matched = prefix[unsafe_offset=matched - 1]
        if value == needle[unsafe_offset=matched]:
            matched += 1
            if matched == needle_length:
                return True
    return False


def provider_error_key_equals_ci(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var length = Int64(literal.byte_length())
    if Int64(view.len) != length or view.ptr == 0:
        return False
    var input = rich_view_ptr(view)
    var target = literal.unsafe_ptr()
    for index in range(length):
        if provider_error_ascii_lower(input[unsafe_offset=index]) != target[unsafe_offset=index]:
            return False
    return True


def provider_error_has_rejection_marker(view: ProdexRichStringView) -> Bool:
    return (
        provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unsupported"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("not supported"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("does not support"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unknown_parameter"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unknown parameter"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unknown_field"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unknown field"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unknown name"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unrecognized"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("unexpected"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("not allowed"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("invalid_argument"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("invalid argument"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("invalid_parameter"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("invalid parameter"))
        or provider_error_contains_ci(view.ptr, Int64(view.len), StringSlice("extra inputs are not permitted"))
    )


def provider_error_array_mentions_member(
    tree: ParsedJson,
    root: Int64,
    needle: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    prefix: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    needle_length: Int64,
) -> Bool:
    var node = root
    while node >= 0:
        var kind = tree.nodes[unsafe_offset=node].kind
        if kind == JSON_STRING and provider_error_view_mentions_member(
            tree.nodes[unsafe_offset=node].text, needle, prefix, needle_length
        ):
            return True
        if kind == JSON_ARRAY and tree.nodes[unsafe_offset=node].first_child >= 0:
            node = tree.nodes[unsafe_offset=node].first_child
            continue
        while node != root and tree.nodes[unsafe_offset=node].next_sibling < 0:
            node = tree.nodes[unsafe_offset=node].parent
        if node == root:
            break
        node = tree.nodes[unsafe_offset=node].next_sibling
    return False


def provider_error_array_has_rejection_marker(tree: ParsedJson, root: Int64) -> Bool:
    var node = root
    while node >= 0:
        var kind = tree.nodes[unsafe_offset=node].kind
        if kind == JSON_STRING and provider_error_has_rejection_marker(
            tree.nodes[unsafe_offset=node].text
        ):
            return True
        if kind == JSON_ARRAY and tree.nodes[unsafe_offset=node].first_child >= 0:
            node = tree.nodes[unsafe_offset=node].first_child
            continue
        while node != root and tree.nodes[unsafe_offset=node].next_sibling < 0:
            node = tree.nodes[unsafe_offset=node].parent
        if node == root:
            break
        node = tree.nodes[unsafe_offset=node].next_sibling
    return False


def provider_error_object_rejects_member(
    tree: ParsedJson,
    object: Int64,
    needle: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    prefix: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    needle_length: Int64,
) -> Bool:
    var identifies = False
    var rejects = False
    var field = tree.nodes[unsafe_offset=object].first_child
    while field >= 0:
        var key = tree.nodes[unsafe_offset=field].key.copy()
        var value_kind = tree.nodes[unsafe_offset=field].kind
        if provider_error_view_mentions_member(key, needle, prefix, needle_length):
            identifies = True
        if (
            provider_error_key_equals_ci(key, StringSlice("param"))
            or provider_error_key_equals_ci(key, StringSlice("parameter"))
            or provider_error_key_equals_ci(key, StringSlice("field"))
            or provider_error_key_equals_ci(key, StringSlice("name"))
            or provider_error_key_equals_ci(key, StringSlice("path"))
            or provider_error_key_equals_ci(key, StringSlice("loc"))
            or provider_error_key_equals_ci(key, StringSlice("location"))
        ) and (value_kind == JSON_STRING or value_kind == JSON_ARRAY) and provider_error_array_mentions_member(
            tree, field, needle, prefix, needle_length
        ):
            identifies = True
        if (
            provider_error_key_equals_ci(key, StringSlice("code"))
            or provider_error_key_equals_ci(key, StringSlice("status"))
            or provider_error_key_equals_ci(key, StringSlice("type"))
            or provider_error_key_equals_ci(key, StringSlice("message"))
            or provider_error_key_equals_ci(key, StringSlice("detail"))
            or provider_error_key_equals_ci(key, StringSlice("reason"))
        ) and (value_kind == JSON_STRING or value_kind == JSON_ARRAY) and provider_error_array_has_rejection_marker(
            tree, field
        ):
            rejects = True
        if identifies and rejects:
            return True
        field = tree.nodes[unsafe_offset=field].next_sibling
    return False


@export("prodex_provider_error_rejects_member_v1")
def prodex_provider_error_rejects_member_v1(
    abi: Int64,
    nodes_address: UInt64,
    nodes_count: Int64,
    raw_address: UInt64,
    raw_length: Int64,
    member_address: UInt64,
    member_length: Int64,
    needle_address: UInt64,
    needle_capacity: Int64,
    prefix_address: UInt64,
    prefix_capacity: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi != PROVIDER_ERROR_REJECTION_ABI:
        return 4
    if (
        nodes_address == 0
        or nodes_count <= 0
        or nodes_count > PROVIDER_ERROR_REJECTION_MAX_NODES
        or raw_length < 0
        or raw_length > PROVIDER_ERROR_REJECTION_MAX_RAW_BYTES
        or (raw_length > 0 and raw_address == 0)
        or member_length < 0
        or member_length > PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES
        or (member_length > 0 and member_address == 0)
        or needle_address == 0
        or needle_capacity < member_length
        or prefix_address == 0
        or prefix_capacity < member_length
        or output_address == 0
    ):
        return 1

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    var member = ProdexRichStringView(UInt(member_address), UInt(member_length))
    if not pj_valid(tree) or not rich_view_valid(member, PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES):
        return 1

    var needle = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(needle_address)
    )
    var prefix_mut = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(prefix_address)
    )
    var needle_length = provider_error_normalize_member(member, needle, prefix_mut)
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = needle_length
    if needle_length == 0:
        return 0

    var needle_read = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(needle_address)
    )
    var prefix_read = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(prefix_address)
    )
    for index in range(tree.count):
        var kind = tree.nodes[unsafe_offset=index].kind
        if kind == JSON_STRING and provider_error_view_mentions_member(
            tree.nodes[unsafe_offset=index].text,
            needle_read,
            prefix_read,
            needle_length,
        ) and provider_error_has_rejection_marker(tree.nodes[unsafe_offset=index].text):
            output[unsafe_offset=0] = 1
            return 0
        if kind == JSON_OBJECT and provider_error_object_rejects_member(
            tree, index, needle_read, prefix_read, needle_length
        ):
            output[unsafe_offset=0] = 1
            return 0
    return 0


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
