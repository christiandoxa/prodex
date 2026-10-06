from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SHARED_ATTACHMENT_ABI_VERSION: Int64 = 1
comptime SHARED_ATTACHMENT_OK: Int64 = 0
comptime SHARED_ATTACHMENT_INVALID: Int64 = 1
comptime SHARED_ATTACHMENT_ABI: Int64 = 4
comptime SHARED_ATTACHMENT_MAX_BYTES: Int64 = 67_108_864

comptime SHARED_ATTACHMENT_IMAGE_TAG_RANGE: Int64 = 0
comptime SHARED_ATTACHMENT_CLIPBOARD_RANGE: Int64 = 1
comptime SHARED_ATTACHMENT_ATTACHMENT_RANGE: Int64 = 2
comptime SHARED_ATTACHMENT_CLIPBOARD_NAME: Int64 = 3
comptime SHARED_ATTACHMENT_PERSISTABLE_NAME: Int64 = 4
comptime SHARED_ATTACHMENT_ROLLOUT_NAME: Int64 = 5


def shared_bytes_match(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or length < 0 or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def shared_find(
    view: ProdexRichStringView,
    literal: StringSlice,
    start: Int64,
) -> Int64:
    var length = Int64(literal.byte_length())
    var end = Int64(view.len)
    if start < 0 or start > end or length > end - start:
        return -1
    var source = rich_view_ptr(view)
    for index in range(start, end - length + 1):
        if shared_bytes_match(source, index, end, literal):
            return index
    return -1


def shared_prefix(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    return shared_bytes_match(rich_view_ptr(view), 0, Int64(view.len), literal)


def shared_suffix(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var length = Int64(literal.byte_length())
    if length > Int64(view.len):
        return False
    return shared_bytes_match(
        rich_view_ptr(view), Int64(view.len) - length, Int64(view.len), literal
    )


def shared_equal(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    return Int64(view.len) == Int64(literal.byte_length()) and shared_prefix(view, literal)


def shared_path_byte(byte: UInt8) -> Bool:
    return not (
        byte == 34
        or byte == 39
        or byte == 60
        or byte == 62
        or byte == 40
        or byte == 41
        or byte == 91
        or byte == 93
        or byte == 123
        or byte == 125
        or byte == 44
        or byte == 59
        or byte == 32
        or byte == 9
        or byte == 13
        or byte == 10
    )


def shared_path_continues(view: ProdexRichStringView, index: Int64) -> Bool:
    var length = Int64(view.len)
    if index < 0 or index >= length:
        return False
    var source = rich_view_ptr(view)
    if source[unsafe_offset=index] == 92:
        if index > 0 and source[unsafe_offset=index - 1] == 92:
            return True
        var end = index
        while end < length and source[unsafe_offset=end] == 92:
            end += 1
        var slash_count = end - index
        var json_escape = False
        if end < length:
            var byte = source[unsafe_offset=end]
            json_escape = (
                byte == 34
                or byte == 98
                or byte == 102
                or byte == 110
                or byte == 114
                or byte == 116
                or byte == 117
            )
        if slash_count % 2 == 1 and json_escape:
            return False
    return shared_path_byte(source[unsafe_offset=index])


def shared_expand_path(
    view: ProdexRichStringView,
    marker_start: Int64,
    marker_length: Int64,
    lower_bound: Int64,
) -> Tuple[Bool, Int64, Int64]:
    var source = rich_view_ptr(view)
    var path_start = marker_start
    while path_start > lower_bound and shared_path_byte(source[unsafe_offset=path_start - 1]):
        path_start -= 1

    var path_end = marker_start + marker_length
    while path_end < Int64(view.len) and shared_path_continues(view, path_end):
        path_end += 1
    while path_end > marker_start and source[unsafe_offset=path_end - 1] == 46:
        path_end -= 1

    var present = (
        path_start < marker_start
        and path_end > marker_start + marker_length
    )
    return (present, path_start, path_end)


def shared_image_prefix_start(
    view: ProdexRichStringView, escaped: Bool
) -> Int64:
    var source = rich_view_ptr(view)
    var length = Int64(view.len)
    var pattern_length: Int64 = 7 if escaped else 6
    if length < pattern_length:
        return -1
    for start in range(length - pattern_length + 1):
        if (
            source[unsafe_offset=start] == 112
            and source[unsafe_offset=start + 1] == 97
            and source[unsafe_offset=start + 2] == 116
            and source[unsafe_offset=start + 3] == 104
            and source[unsafe_offset=start + 4] == 61
        ):
            if escaped:
                if (
                    source[unsafe_offset=start + 5] == 92
                    and source[unsafe_offset=start + 6] == 34
                ):
                    return start
            elif source[unsafe_offset=start + 5] == 34:
                return start
    return -1


def shared_image_tag_range(
    view: ProdexRichStringView,
) -> Tuple[Bool, Int64, Int64]:
    var source = rich_view_ptr(view)
    var prefix_start = shared_image_prefix_start(view, True)
    var escaped = prefix_start >= 0
    var prefix_length: Int64 = 7
    if not escaped:
        prefix_start = shared_image_prefix_start(view, False)
        if prefix_start < 0:
            return (False, Int64(0), Int64(0))
        prefix_length = 6

    var path_start = prefix_start + prefix_length
    var path_end = path_start
    if escaped:
        while path_end + 1 < Int64(view.len):
            if source[unsafe_offset=path_end] == 92 and source[unsafe_offset=path_end + 1] == 34:
                return (True, path_start, path_end)
            path_end += 1
    else:
        while path_end < Int64(view.len):
            if source[unsafe_offset=path_end] == 34:
                return (True, path_start, path_end)
            path_end += 1
    return (False, Int64(0), Int64(0))


def shared_scan_cursor(view: ProdexRichStringView, cursor: Int64) -> Int64:
    var length = Int64(view.len)
    if cursor < 0 or cursor >= length:
        return cursor
    var source = rich_view_ptr(view)
    if source[unsafe_offset=cursor] != 92 or cursor + 1 >= length:
        return cursor
    var escaped = source[unsafe_offset=cursor + 1]
    if escaped == 117 and cursor + 6 <= length:
        return cursor + 6
    if (
        escaped == 34
        or escaped == 98
        or escaped == 102
        or escaped == 110
        or escaped == 114
        or escaped == 116
    ):
        return cursor + 2
    return cursor


def shared_attachment_marker(
    view: ProdexRichStringView,
    cursor: Int64,
) -> Tuple[Bool, Int64, Int64]:
    var marker0 = StringSlice("/attachments/")
    var marker1 = StringSlice("/attachments\\")
    var marker2 = StringSlice("\\attachments/")
    var marker3 = StringSlice("\\attachments\\")
    var best: Int64 = -1
    for candidate in [
        shared_find(view, marker0, cursor),
        shared_find(view, marker1, cursor),
        shared_find(view, marker2, cursor),
        shared_find(view, marker3, cursor),
    ]:
        if candidate >= 0 and (best < 0 or candidate < best):
            best = candidate
    if best < 0:
        return (False, Int64(0), Int64(0))
    return (True, best, Int64(marker0.byte_length()))


@export("prodex_shared_attachment_policy_v1")
def prodex_shared_attachment_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
    cursor: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SHARED_ATTACHMENT_ABI_VERSION:
        return SHARED_ATTACHMENT_ABI
    if (
        operation < SHARED_ATTACHMENT_IMAGE_TAG_RANGE
        or operation > SHARED_ATTACHMENT_ROLLOUT_NAME
        or input_length < 0
        or input_length > SHARED_ATTACHMENT_MAX_BYTES
        or cursor < 0
        or cursor > input_length
        or (input_length > 0 and input_address == 0)
        or output_address == 0
    ):
        return SHARED_ATTACHMENT_INVALID

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, SHARED_ATTACHMENT_MAX_BYTES):
        return SHARED_ATTACHMENT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = 0
    output[1] = 0
    output[2] = 0

    if operation == SHARED_ATTACHMENT_IMAGE_TAG_RANGE:
        var result = shared_image_tag_range(view)
        output[0] = Int64(result[0])
        output[1] = result[1]
        output[2] = result[2]
        return SHARED_ATTACHMENT_OK

    if operation == SHARED_ATTACHMENT_CLIPBOARD_RANGE:
        var marker = StringSlice("codex-clipboard-")
        var scan_cursor = shared_scan_cursor(view, cursor)
        var marker_start = shared_find(view, marker, scan_cursor)
        if marker_start < 0:
            return SHARED_ATTACHMENT_OK
        var result = shared_expand_path(
            view, marker_start, Int64(marker.byte_length()), scan_cursor
        )
        output[0] = Int64(result[0])
        output[1] = result[1]
        output[2] = result[2]
        return SHARED_ATTACHMENT_OK

    if operation == SHARED_ATTACHMENT_ATTACHMENT_RANGE:
        var scan_cursor = shared_scan_cursor(view, cursor)
        var marker = shared_attachment_marker(view, scan_cursor)
        if not marker[0]:
            return SHARED_ATTACHMENT_OK
        var result = shared_expand_path(view, marker[1], marker[2], scan_cursor)
        output[0] = Int64(result[0])
        output[1] = result[1]
        output[2] = result[2]
        return SHARED_ATTACHMENT_OK

    if operation == SHARED_ATTACHMENT_CLIPBOARD_NAME:
        output[0] = Int64(
            shared_prefix(view, StringSlice("codex-clipboard-"))
        )
        return SHARED_ATTACHMENT_OK

    if operation == SHARED_ATTACHMENT_PERSISTABLE_NAME:
        output[0] = Int64(
            shared_prefix(view, StringSlice("pasted-text-"))
            or shared_prefix(view, StringSlice("image-"))
            or shared_equal(view, StringSlice("goal-objective.md"))
        )
        return SHARED_ATTACHMENT_OK

    output[0] = Int64(
        shared_suffix(view, StringSlice(".jsonl"))
        or shared_suffix(view, StringSlice(".jsonl.zst"))
    )
    return SHARED_ATTACHMENT_OK
