# Official tunnel-client version selection; process probing stays with Rust.
from std.collections import Array
from std.memory import Pointer
from rich_text import rich_trim_bounds, rich_view_prefix, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from update_notice_policy import update_notice_release_numeric

def super_expose_ascii_trim_bounds(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Array[Int64, 2]:
    var left = start
    var right = end
    var ptr = rich_view_ptr(view)
    while left < right:
        var byte = ptr[unsafe_offset=left]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            left += 1
        else:
            break
    while right > left:
        var byte = ptr[unsafe_offset=right - 1]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            right -= 1
        else:
            break
    var bounds = Array[Int64, 2](fill=0)
    bounds[0] = left
    bounds[1] = right
    return bounds^

def super_expose_ascii_digit(byte: UInt8) -> Bool:
    return byte >= 48 and byte <= 57

def super_expose_ascii_hex(byte: UInt8) -> Bool:
    return (
        (byte >= 48 and byte <= 57)
        or (byte >= 97 and byte <= 102)
        or (byte >= 65 and byte <= 70)
    )

def super_expose_tunnel_client_version_line_valid(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end <= start:
        return False
    var ptr = rich_view_ptr(view)
    var cursor = start
    for component in range(3):
        var digits: Int64 = 0
        while cursor < end and super_expose_ascii_digit(ptr[unsafe_offset=cursor]):
            cursor += 1
            digits += 1
        if digits == 0:
            return False
        if component < 2:
            if cursor >= end or ptr[unsafe_offset=cursor] != 46:
                return False
            cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 43:
        return False
    cursor += 1
    var first_sha = cursor
    for _ in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        cursor += 1
    var marker = StringSlice(" (git sha: ")
    var marker_len = Int64(marker.byte_length())
    if not rich_view_prefix[" (git sha: "](
        ProdexRichStringView(view.ptr + UInt(cursor), UInt(end - cursor)), False
    ):
        return False
    cursor += marker_len
    var second_sha = cursor
    for index in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        var sha_index = Int64(index)
        if ptr[unsafe_offset=first_sha + sha_index] != ptr[unsafe_offset=second_sha + sha_index]:
            return False
        cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 41:
        return False
    cursor += 1
    return cursor == end

def super_expose_tunnel_client_version_output_valid(
    view: ProdexRichStringView,
) -> Bool:
    var length = Int64(view.len)
    var line_start: Int64 = 0
    var cursor: Int64 = 0
    while cursor <= length:
        if cursor == length or rich_view_ptr(view)[unsafe_offset=cursor] == 10:
            var bounds = super_expose_ascii_trim_bounds(view, line_start, cursor)
            if super_expose_tunnel_client_version_line_valid(
                view, bounds[0], bounds[1]
            ):
                return True
            line_start = cursor + 1
        cursor += 1
    return False


# Tunnel probes require an exact stable core; release notices allow prefixes and metadata.
def super_expose_tunnel_stable_core(
    view: ProdexRichStringView,
) -> Tuple[Bool, UInt64, UInt64, UInt64]:
    var source = rich_view_ptr(view)
    var end = Int64(view.len)
    var major = update_notice_release_numeric(source, 0, end)
    if not major[0] or major[2] >= end or source[unsafe_offset=major[2]] != 46:
        return (False, UInt64(0), UInt64(0), UInt64(0))
    var minor = update_notice_release_numeric(source, major[2] + 1, end)
    if not minor[0] or minor[2] >= end or source[unsafe_offset=minor[2]] != 46:
        return (False, UInt64(0), UInt64(0), UInt64(0))
    var patch = update_notice_release_numeric(source, minor[2] + 1, end)
    if not patch[0] or patch[2] != end:
        return (False, UInt64(0), UInt64(0), UInt64(0))
    return (True, major[1], minor[1], patch[1])


@export("prodex_super_expose_tunnel_supported_version_v1")
def prodex_super_expose_tunnel_supported_version_v1(
    abi_version: Int64, input_address: UInt, input_length: Int64,
    minimum_address: UInt, minimum_length: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        input_length < 0 or input_length > 16_384 or minimum_length < 0
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    var minimum_view = ProdexRichStringView(minimum_address, UInt(minimum_length))
    if not rich_view_valid(view, 16_384) or not rich_view_valid(minimum_view, 64):
        return 1
    var minimum = super_expose_tunnel_stable_core(minimum_view)
    if not minimum[0]:
        return 1
    var selected_start: Int64 = 0
    var selected_end: Int64 = 0
    # Preserve the existing format gate independently of first supported-line selection.
    if super_expose_tunnel_client_version_output_valid(view):
        var source = rich_view_ptr(view)
        var cursor: Int64 = 0
        var line_start: Int64 = 0
        while cursor <= input_length:
            if cursor == input_length or source[unsafe_offset=cursor] == 10:
                var line = ProdexRichStringView(view.ptr + UInt(line_start), UInt(cursor - line_start))
                var bounds = rich_trim_bounds(line)
                var start = line_start + bounds[0]
                var end = line_start + bounds[1]
                if super_expose_tunnel_client_version_line_valid(view, start, end):
                    var version_end = start
                    while version_end < end and source[unsafe_offset=version_end] != 43:
                        version_end += 1
                    var version = super_expose_tunnel_stable_core(
                        ProdexRichStringView(view.ptr + UInt(start), UInt(version_end - start))
                    )
                    if version[0] and (
                        version[1] > minimum[1]
                        or version[1] == minimum[1] and version[2] > minimum[2]
                        or version[1] == minimum[1] and version[2] == minimum[2] and version[3] >= minimum[3]
                    ):
                        selected_start = start
                        selected_end = version_end
                        break
                line_start = cursor + 1
            cursor += 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = Int64(1) if selected_end > selected_start else Int64(0)
    output[unsafe_offset=1] = selected_start
    output[unsafe_offset=2] = selected_end
    return 0
