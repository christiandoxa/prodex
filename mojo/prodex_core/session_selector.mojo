from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid, rich_views_equal
from rich_types import ProdexRichStringView

comptime SESSION_SELECTOR_ABI_VERSION: Int64 = 1
comptime SESSION_SELECTOR_OK: Int64 = 0
comptime SESSION_SELECTOR_INVALID: Int64 = 1
comptime SESSION_REPORT_QUERY_FIELD_COUNT: Int64 = 6
comptime SESSION_STATE_DB_MATCH_NONE: Int64 = 0
comptime SESSION_STATE_DB_MATCH_PREFIX: Int64 = 1
comptime SESSION_STATE_DB_MATCH_EXACT: Int64 = 2
comptime SESSION_STATE_DB_SOURCE_NONE: Int64 = 0
comptime SESSION_STATE_DB_SOURCE_NORMALIZED_THREAD_ID: Int64 = 1
comptime SESSION_STATE_DB_SOURCE_THREAD_ID: Int64 = 2
comptime SESSION_STATE_DB_SOURCE_PATH_STEM: Int64 = 3


@fieldwise_init
struct SessionRecentWorkspaceCandidate(Copyable):
    var modified_epoch: Int64
    var absolute: Int64
    var workspace: ProdexRichStringView


def session_file_name_suffix_length(
    view: ProdexRichStringView,
    literal: StringSlice,
) -> Int64:
    var suffix_length = Int64(literal.byte_length())
    var view_length = Int64(view.len)
    if suffix_length > view_length:
        return -1
    var source = rich_view_ptr(view)
    var suffix = literal.unsafe_ptr()
    var start = view_length - suffix_length
    for index in range(suffix_length):
        if source[unsafe_offset=start + index] != suffix[unsafe_offset=index]:
            return -1
    return suffix_length


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


def session_selector_strip_thread_prefix(
    view: ProdexRichStringView,
) -> ProdexRichStringView:
    if view.len < 7:
        return view.copy()
    var source = rich_view_ptr(view)
    if (
        source[unsafe_offset=0] != 116
        or source[unsafe_offset=1] != 104
        or source[unsafe_offset=2] != 114
        or source[unsafe_offset=3] != 101
        or source[unsafe_offset=4] != 97
        or source[unsafe_offset=5] != 100
        or source[unsafe_offset=6] != 95
    ):
        return view.copy()
    return ProdexRichStringView(view.ptr + UInt(7), view.len - UInt(7))


def session_selector_path_span(
    stem: ProdexRichStringView,
    selector: ProdexRichStringView,
    exact: Bool,
    require_full: Bool,
) -> Tuple[Int64, Int64]:
    var stem_length = Int64(stem.len)
    if require_full:
        if session_selector_is_full_id(stem):
            return (0, stem_length)
    elif session_selector_matches(stem, selector, exact):
        return (0, stem_length)

    var source = rich_view_ptr(stem)
    var start: Int64 = 0
    while start <= stem_length:
        var cursor = start
        var separator_count: Int64 = 0
        var next_start: Int64 = -1
        while cursor < stem_length:
            if source[unsafe_offset=cursor] == 45:
                separator_count += 1
                if separator_count == 1:
                    next_start = cursor + 1
                if separator_count == 4:
                    break
            cursor += 1
        if separator_count < 4:
            break

        var candidate_end = cursor + 1
        while candidate_end < stem_length and source[unsafe_offset=candidate_end] != 45:
            candidate_end += 1
        var candidate = ProdexRichStringView(
            stem.ptr + UInt(start), UInt(candidate_end - start)
        )
        var matched = (
            session_selector_is_full_id(candidate)
            if require_full
            else session_selector_matches(candidate, selector, exact)
        )
        if matched:
            return (start, candidate_end)
        start = next_start
    return (-1, -1)


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


def session_recent_workspace_seen(
    candidates: Pointer[mut=False, SessionRecentWorkspaceCandidate, _],
    output: Pointer[mut=False, Int64, _],
    selected_count: Int64,
    candidate_index: Int64,
) -> Bool:
    var workspace = candidates[unsafe_offset=candidate_index].workspace.copy()
    for selected in range(selected_count):
        var selected_index = output[unsafe_offset=selected + 1]
        if rich_views_equal(
            workspace, candidates[unsafe_offset=selected_index].workspace
        ):
            return True
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


@export("prodex_session_selector_path_span_v1")
def prodex_session_selector_path_span_v1(
    abi_version: Int64,
    stem_address: UInt,
    stem_length: Int64,
    selector_address: UInt,
    selector_length: Int64,
    exact: Int64,
    require_full: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or stem_length < 0
        or selector_length < 0
        or (exact != 0 and exact != 1)
        or (require_full != 0 and require_full != 1)
        or output_address == 0
        or (stem_length > 0 and stem_address == 0)
        or (selector_length > 0 and selector_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var stem = ProdexRichStringView(stem_address, UInt(stem_length))
    var selector = ProdexRichStringView(selector_address, UInt(selector_length))
    if not rich_view_valid(stem, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(
        selector, 0x7FFFFFFFFFFFFFFF
    ):
        return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var span = session_selector_path_span(
        stem, selector, exact == 1, require_full == 1
    )
    output[unsafe_offset=0] = span[0]
    output[unsafe_offset=1] = span[1]
    return SESSION_SELECTOR_OK


@export("prodex_session_state_db_row_match_plan_v1")
def prodex_session_state_db_row_match_plan_v1(
    abi_version: Int64,
    thread_id_address: UInt,
    thread_id_length: Int64,
    path_stem_address: UInt,
    path_stem_length: Int64,
    path_stem_present: Int64,
    selector_address: UInt,
    selector_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or thread_id_length < 0
        or path_stem_length < 0
        or selector_length < 0
        or (path_stem_present != 0 and path_stem_present != 1)
        or (path_stem_present == 0 and path_stem_length != 0)
        or output_address == 0
        or (thread_id_length > 0 and thread_id_address == 0)
        or (path_stem_length > 0 and path_stem_address == 0)
        or (selector_length > 0 and selector_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var thread_id = ProdexRichStringView(
        thread_id_address, UInt(thread_id_length)
    )
    var path_stem = ProdexRichStringView(path_stem_address, UInt(path_stem_length))
    var selector = ProdexRichStringView(selector_address, UInt(selector_length))
    if (
        not rich_view_valid(thread_id, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(path_stem, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(selector, 0x7FFFFFFFFFFFFFFF)
    ):
        return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var normalized_thread_id = session_selector_strip_thread_prefix(thread_id)
    var path_matches = path_stem_present == 1
    var thread_id_has_prefix = normalized_thread_id.len != thread_id.len

    var match_kind = SESSION_STATE_DB_MATCH_NONE
    if session_selector_matches(thread_id, selector, True) or session_selector_matches(
        normalized_thread_id, selector, True
    ) or (
        path_matches
        and session_selector_path_span(path_stem, selector, True, False)[0] >= 0
    ):
        match_kind = SESSION_STATE_DB_MATCH_EXACT
    elif session_selector_matches(thread_id, selector, False) or session_selector_matches(
        normalized_thread_id, selector, False
    ) or (
        path_matches
        and session_selector_path_span(path_stem, selector, False, False)[0] >= 0
    ):
        match_kind = SESSION_STATE_DB_MATCH_PREFIX

    var session_id_source = SESSION_STATE_DB_SOURCE_NONE
    if thread_id_has_prefix and session_selector_is_full_id(normalized_thread_id) and session_selector_matches(
        normalized_thread_id, selector, False
    ):
        session_id_source = SESSION_STATE_DB_SOURCE_NORMALIZED_THREAD_ID
    elif session_selector_is_full_id(thread_id) and session_selector_matches(
        thread_id, selector, False
    ):
        session_id_source = SESSION_STATE_DB_SOURCE_THREAD_ID
    elif path_matches and session_selector_path_span(path_stem, selector, False, False)[0] >= 0:
        session_id_source = SESSION_STATE_DB_SOURCE_PATH_STEM

    output[unsafe_offset=0] = match_kind
    output[unsafe_offset=1] = session_id_source
    return SESSION_SELECTOR_OK


@export("prodex_session_recent_workspace_plan_v1")
def prodex_session_recent_workspace_plan_v1(
    abi_version: Int64,
    candidates_address: UInt,
    candidates_count: Int64,
    max_workspaces: Int64,
    output_address: UInt,
    output_capacity: Int64,
    scratch_address: UInt,
    scratch_capacity: Int64,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or candidates_count < 0
        or max_workspaces < 0
        or output_address == 0
        or output_capacity < 1
        or scratch_capacity < candidates_count
        or candidates_count > 0 and candidates_address == 0
        or candidates_count > 0 and scratch_address == 0
    ):
        return SESSION_SELECTOR_INVALID
    var selected_limit = max_workspaces
    if selected_limit > candidates_count:
        selected_limit = candidates_count
    if output_capacity < selected_limit + 1:
        return SESSION_SELECTOR_INVALID

    var candidates = Pointer[
        mut=False, SessionRecentWorkspaceCandidate, ImmUntrackedOrigin
    ](unsafe_from_address=Int(candidates_address))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    for index in range(candidates_count):
        var candidate = candidates[unsafe_offset=index].copy()
        if (
            (candidate.absolute != 0 and candidate.absolute != 1)
            or not rich_view_valid(candidate.workspace, 0x7FFFFFFFFFFFFFFF)
        ):
            return SESSION_SELECTOR_INVALID
        scratch[unsafe_offset=index] = 0
    output[unsafe_offset=0] = 0

    var selected_count: Int64 = 0
    while selected_count < selected_limit:
        var best_index: Int64 = -1
        for index in range(candidates_count):
            if scratch[unsafe_offset=index] == 1:
                continue
            if best_index < 0 or candidates[unsafe_offset=index].modified_epoch > candidates[
                unsafe_offset=best_index
            ].modified_epoch:
                best_index = index
        if best_index < 0:
            break
        scratch[unsafe_offset=best_index] = 1
        if candidates[unsafe_offset=best_index].absolute != 1:
            continue
        if session_recent_workspace_seen(
            candidates, output, selected_count, best_index
        ):
            continue
        output[unsafe_offset=selected_count + 1] = best_index
        selected_count += 1
    output[unsafe_offset=0] = selected_count
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


@export("prodex_session_file_name_policy_v1")
def prodex_session_file_name_policy_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_SELECTOR_ABI_VERSION
        or name_length < 0
        or output_address == 0
        or (name_length > 0 and name_address == 0)
    ):
        return SESSION_SELECTOR_INVALID
    var name = ProdexRichStringView(name_address, UInt(name_length))
    if not rich_view_valid(name, 0x7FFFFFFFFFFFFFFF):
        return SESSION_SELECTOR_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var stem_length = Int64(name.len)
    var is_metadata = False
    var suffix_length = session_file_name_suffix_length(name, StringSlice(".jsonl.zst"))
    if suffix_length < 0:
        suffix_length = session_file_name_suffix_length(name, StringSlice(".jsonl"))
    if suffix_length < 0:
        suffix_length = session_file_name_suffix_length(name, StringSlice(".json"))
    if suffix_length >= 0:
        is_metadata = True
        stem_length -= suffix_length
    output[unsafe_offset=0] = Int64(is_metadata)
    output[unsafe_offset=1] = stem_length
    return SESSION_SELECTOR_OK

# Session report filter ownership. Host supplies path-equality and
# Unicode-lowercased query/fields, while Mojo chooses all filter gates.
# facts: has_dir, dir_match, include_subagents, is_subagent,
#        report_profile_present, filter_profile_present, query_present.
# views: report profile, requested profile, search query, six search fields.
comptime SESSION_REPORT_FILTER_ABI_VERSION: Int64 = 1
comptime SESSION_REPORT_FILTER_FACT_COUNT: Int64 = 7
comptime SESSION_REPORT_FILTER_VIEW_COUNT: Int64 = 9
comptime SESSION_REPORT_FILTER_MAX_TEXT_BYTES: Int64 = 1_048_576


@export("prodex_session_report_filter_v1")
def prodex_session_report_filter_v1(
    abi_version: Int64,
    facts_address: UInt,
    facts_count: Int64,
    views_address: UInt,
    views_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SESSION_REPORT_FILTER_ABI_VERSION:
        return 4
    if (
        facts_count != SESSION_REPORT_FILTER_FACT_COUNT
        or views_count != SESSION_REPORT_FILTER_VIEW_COUNT
        or facts_address == 0 or facts_address % 8 != 0
        or views_address == 0 or views_address % 8 != 0
        or output_address == 0 or output_address % 8 != 0
    ):
        return SESSION_SELECTOR_INVALID
    var facts = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(views_address)
    )
    for index in range(SESSION_REPORT_FILTER_FACT_COUNT):
        if facts[unsafe_offset=index] < 0 or facts[unsafe_offset=index] > 1:
            return SESSION_SELECTOR_INVALID
    for index in range(SESSION_REPORT_FILTER_VIEW_COUNT):
        if not rich_view_valid(
            views[unsafe_offset=index], SESSION_REPORT_FILTER_MAX_TEXT_BYTES
        ):
            return SESSION_SELECTOR_INVALID

    var matches = True
    if facts[unsafe_offset=0] == 1 and facts[unsafe_offset=1] == 0:
        matches = False
    if facts[unsafe_offset=2] == 0 and facts[unsafe_offset=3] == 1:
        matches = False
    if facts[unsafe_offset=5] == 1:
        if facts[unsafe_offset=4] == 0 or not rich_views_equal(
            views[unsafe_offset=0], views[unsafe_offset=1]
        ):
            matches = False
    if facts[unsafe_offset=6] == 1:
        var query = views[unsafe_offset=2].copy()
        var bounds = rich_trim_bounds(query)
        if bounds[1] > bounds[0]:
            query.ptr += UInt(bounds[0])
            query.len = UInt(bounds[1] - bounds[0])
            var found = False
            for index in range(Int64(3), SESSION_REPORT_FILTER_VIEW_COUNT):
                if session_report_query_contains(views[unsafe_offset=index], query):
                    found = True
                    break
            if not found:
                matches = False
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = Int64(matches)
    return SESSION_SELECTOR_OK
