from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_LOG_LEVEL_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_LEVEL_MAX_BYTES: Int64 = 4096


def log_view_part_matches(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, part: StringSlice
) -> Bool:
    var right = part.unsafe_ptr()
    for index in range(Int64(part.byte_length())):
        if ptr[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def log_view_starts_sequence2(
    view: ProdexRichStringView, first: StringSlice, second: StringSlice
) -> Bool:
    var total = Int64(first.byte_length()) + Int64(second.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, Int64(first.byte_length()), second
    )


def log_view_starts_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var total = first_length + second_length + Int64(third.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, first_length, second
    ) and log_view_part_matches(ptr, first_length + second_length, third)


def log_view_contains_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var total = first_length + second_length + third_length
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if log_view_part_matches(ptr, start, first) and log_view_part_matches(
            ptr, start + first_length, second
        ) and log_view_part_matches(
            ptr, start + first_length + second_length, third
        ):
            return True
    return False


def log_view_contains_sequence5(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    fifth: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var fourth_length = Int64(fourth.byte_length())
    var total = (
        first_length
        + second_length
        + third_length
        + fourth_length
        + Int64(fifth.byte_length())
    )
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if not log_view_part_matches(ptr, start, first):
            continue
        var second_start = start + first_length
        if not log_view_part_matches(ptr, second_start, second):
            continue
        var third_start = second_start + second_length
        if not log_view_part_matches(ptr, third_start, third):
            continue
        var fourth_start = third_start + third_length
        if not log_view_part_matches(ptr, fourth_start, fourth):
            continue
        if log_view_part_matches(ptr, fourth_start + fourth_length, fifth):
            return True
    return False


def log_view_contains_json_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\":\""),
        level,
        StringSlice("\""),
    ) or log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\": \""),
        level,
        StringSlice("\""),
    )


def log_view_contains_kv_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence3(view, field, StringSlice("="), level) or log_view_contains_sequence3(
        view, field, StringSlice(": "), level
    )


def log_view_level_matches(view: ProdexRichStringView, level_id: Int64) -> Bool:
    if level_id < 1 or level_id > 7:
        return False
    var level = StringSlice("fatal")
    if level_id == 2:
        level = StringSlice("error")
    elif level_id == 3:
        level = StringSlice("warn")
    elif level_id == 4:
        level = StringSlice("warning")
    elif level_id == 5:
        level = StringSlice("info")
    elif level_id == 6:
        level = StringSlice("debug")
    elif level_id == 7:
        level = StringSlice("trace")

    for field in [StringSlice("level"), StringSlice("severity"), StringSlice("status")]:
        if log_view_contains_json_level(view, field, level) or log_view_contains_kv_level(
            view, field, level
        ):
            return True
    return log_view_starts_sequence2(view, level, StringSlice(" ")) or log_view_starts_sequence2(
        view, level, StringSlice(":")
    ) or log_view_starts_sequence2(view, level, StringSlice("\t")) or log_view_starts_sequence3(
        view, StringSlice("["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(" ")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(":")
    ) or log_view_contains_sequence3(
        view, StringSlice(" ["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice("\t")
    )


@export("prodex_mojo_log_level_classify_v1")
def prodex_mojo_log_level_classify_v1(
    abi_version: Int64, event_address: UInt, level_address: UInt
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_LOG_LEVEL_ABI_VERSION
        or event_address == 0
        or level_address == 0
    ):
        return 1
    var event_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(event_address))
    var event = event_ptr[].copy()
    if not rich_view_valid(event, PRODEX_LOG_LEVEL_MAX_BYTES):
        return 2

    var level = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(level_address)
    )
    level[] = 0
    var candidate: Int64 = 1
    while candidate <= 7:
        if log_view_level_matches(event, candidate):
            if candidate >= 4:
                level[] = candidate - 1
            else:
                level[] = candidate
            return 0
        candidate += 1
    return 0
