from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SESSION_SELECTOR_ABI_VERSION: Int64 = 1
comptime SESSION_SELECTOR_OK: Int64 = 0
comptime SESSION_SELECTOR_INVALID: Int64 = 1
comptime SESSION_REPORT_QUERY_FIELD_COUNT: Int64 = 6


def session_selector_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def session_selector_is_full_id(view: ProdexRichStringView) -> Bool:
    if view.len != 36:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(36):
        if index == 8 or index == 13 or index == 18 or index == 23:
            if ptr[unsafe_offset=index] != 45:
                return False
        else:
            var value = ptr[unsafe_offset=index]
            if not (
                value >= 48 and value <= 57
                or value >= 65 and value <= 70
                or value >= 97 and value <= 102
            ):
                return False
    return True


def session_selector_matches(
    id: ProdexRichStringView,
    selector: ProdexRichStringView,
    exact: Bool,
) -> Bool:
    if exact and id.len != selector.len:
        return False
    if selector.len > id.len:
        return False
    var left = rich_view_ptr(id)
    var right = rich_view_ptr(selector)
    for index in range(Int64(selector.len)):
        if session_selector_ascii_lower(left[unsafe_offset=index]) != session_selector_ascii_lower(right[unsafe_offset=index]):
            return False
    return True


def session_report_query_contains(
    field: ProdexRichStringView,
    query: ProdexRichStringView,
) -> Bool:
    """Search one already-normalized report field for the query bytes.

    The Rust adapter applies Unicode ``str::to_lowercase`` before this call,
    because that is the public session-filter contract. Mojo owns the bounded
    field scan and therefore never needs to duplicate Rust's Unicode tables.
    """
    if query.len == 0:
        return True
    if query.len > field.len:
        return False
    var source = rich_view_ptr(field)
    var wanted = rich_view_ptr(query)
    var last_start = Int64(field.len - query.len)
    var start: Int64 = 0
    while start <= last_start:
        var matched = True
        for offset in range(Int64(query.len)):
            if source[unsafe_offset=start + offset] != wanted[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
        start += 1
    return False


@export("prodex_session_selector_is_full_v1")
def prodex_session_selector_is_full_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, 0x7FFFFFFFFFFFFFFF):
        return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(session_selector_is_full_id(value))
    return SESSION_SELECTOR_OK


@export("prodex_session_selector_matches_v1")
def prodex_session_selector_matches_v1(
    abi_version: Int64,
    id_address: UInt,
    id_length: Int64,
    selector_address: UInt,
    selector_length: Int64,
    exact: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or id_length < 0
        or selector_length < 0
        or (exact != 0 and exact != 1)
        or output_address == 0
        or (id_length > 0 and id_address == 0)
        or (selector_length > 0 and selector_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var id = ProdexRichStringView(id_address, UInt(id_length))
    var selector = ProdexRichStringView(selector_address, UInt(selector_length))
    if not rich_view_valid(id, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(
        selector, 0x7FFFFFFFFFFFFFFF
    ):
        return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(
        session_selector_matches(id, selector, exact == 1)
    )
    return SESSION_SELECTOR_OK


@export("prodex_session_selector_match_plan_v1")
def prodex_session_selector_match_plan_v1(
    abi_version: Int64,
    ids_address: UInt,
    ids_count: Int64,
    selector_address: UInt,
    selector_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or ids_count < 0
        or ids_count > (0x7FFFFFFFFFFFFFFF - 2) // 2
        or selector_length < 0
        or output_address == 0
        or output_capacity < 2 + ids_count * 2
        or (ids_count > 0 and ids_address == 0)
        or (selector_length > 0 and selector_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var selector = ProdexRichStringView(selector_address, UInt(selector_length))
    if not rich_view_valid(selector, 0x7FFFFFFFFFFFFFFF):
        return SESSION_SELECTOR_INVALID
    var ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(ids_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var exact_count: Int64 = 0
    var prefix_count: Int64 = 0
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    for index in range(ids_count):
        var id = ids[unsafe_offset=index].copy()
        if not rich_view_valid(id, 0x7FFFFFFFFFFFFFFF):
            return SESSION_SELECTOR_INVALID
        if session_selector_matches(id, selector, True):
            output[unsafe_offset=2 + exact_count] = index
            exact_count += 1
        if session_selector_matches(id, selector, False):
            output[unsafe_offset=2 + ids_count + prefix_count] = index
            prefix_count += 1
    output[unsafe_offset=0] = exact_count
    output[unsafe_offset=1] = prefix_count
    return SESSION_SELECTOR_OK


@export("prodex_session_report_query_matches_v1")
def prodex_session_report_query_matches_v1(
    abi_version: Int64,
    query_address: UInt,
    query_length: Int64,
    fields_address: UInt,
    fields_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or query_length < 0
        or fields_count != SESSION_REPORT_QUERY_FIELD_COUNT
        or output_address == 0
        or (query_length > 0 and query_address == 0)
        or (fields_count > 0 and fields_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var query = ProdexRichStringView(query_address, UInt(query_length))
    if not rich_view_valid(query, 0x7FFFFFFFFFFFFFFF):
        return SESSION_SELECTOR_INVALID
    var fields = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(fields_address))
    for index in range(SESSION_REPORT_QUERY_FIELD_COUNT):
        var field = fields[unsafe_offset=index].copy()
        if not rich_view_valid(field, 0x7FFFFFFFFFFFFFFF):
            return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    for index in range(SESSION_REPORT_QUERY_FIELD_COUNT):
        if session_report_query_contains(fields[unsafe_offset=index], query):
            output[unsafe_offset=0] = 1
            break
    return SESSION_SELECTOR_OK
