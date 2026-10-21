# Proxy-bypass token trimming, stable deduplication and original-range planning.
# Rust owns environment reads, URL parsing and final OsString materialization.
from std.memory import Pointer
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime LAUNCH_PROXY_BYPASS_ABI: Int64 = 1
comptime LAUNCH_PROXY_BYPASS_MAX_VIEWS: Int64 = 4096
comptime LAUNCH_PROXY_BYPASS_MAX_BYTES: Int64 = 1_048_576
comptime LAUNCH_PROXY_BYPASS_MAX_TOKENS: Int64 = 16384


def launch_proxy_ascii_lower(byte: UInt8) -> UInt8:
    if byte >= 65 and byte <= 90:
        return byte + 32
    return byte


def launch_proxy_token_equal(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if launch_proxy_ascii_lower(left_ptr[unsafe_offset=index]) != launch_proxy_ascii_lower(
            right_ptr[unsafe_offset=index]
        ):
            return False
    return True


@export("prodex_launch_proxy_bypass_plan_v1")
def prodex_launch_proxy_bypass_plan_v1(
    abi_version: Int64,
    sources_address: UInt,
    source_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != LAUNCH_PROXY_BYPASS_ABI:
        return 4
    if (
        source_count < 0 or source_count > LAUNCH_PROXY_BYPASS_MAX_VIEWS
        or output_capacity < 0 or output_capacity > LAUNCH_PROXY_BYPASS_MAX_TOKENS
        or (source_count > 0 and (sources_address == 0 or sources_address % 8 != 0))
        or (output_capacity > 0 and (output_address == 0 or output_address % 8 != 0))
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    var source_words = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(sources_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var total_bytes: Int64 = 0
    for source_index in range(source_count):
        var source = ProdexRichStringView(
            UInt(source_words[unsafe_offset=source_index * 2]),
            UInt(source_words[unsafe_offset=source_index * 2 + 1]),
        )
        if not rich_view_valid(source, LAUNCH_PROXY_BYPASS_MAX_BYTES):
            return 1
        if Int64(source.len) > LAUNCH_PROXY_BYPASS_MAX_BYTES - total_bytes:
            return 1
        total_bytes += Int64(source.len)

    var selected_count: Int64 = 0
    for source_index in range(source_count):
        var source = ProdexRichStringView(
            UInt(source_words[unsafe_offset=source_index * 2]),
            UInt(source_words[unsafe_offset=source_index * 2 + 1]),
        )
        var ptr = rich_view_ptr(source)
        var token_start: Int64 = 0
        for cursor in range(Int64(source.len) + 1):
            if cursor != Int64(source.len) and ptr[unsafe_offset=cursor] != 44:
                continue
            var raw_token = ProdexRichStringView(
                source.ptr + UInt(token_start), UInt(cursor - token_start)
            )
            var trim = rich_trim_bounds(raw_token)
            var start = token_start + trim[0]
            var length = trim[1] - trim[0]
            if length > 0:
                var token = ProdexRichStringView(
                    source.ptr + UInt(start), UInt(length)
                )
                var exists = False
                for prior in range(selected_count):
                    var prior_source_index = output[unsafe_offset=prior * 3]
                    var prior_source = ProdexRichStringView(
                        UInt(source_words[unsafe_offset=prior_source_index * 2]),
                        UInt(source_words[unsafe_offset=prior_source_index * 2 + 1]),
                    )
                    var prior_start = output[unsafe_offset=prior * 3 + 1]
                    var prior_length = output[unsafe_offset=prior * 3 + 2]
                    var previous = ProdexRichStringView(
                        prior_source.ptr + UInt(prior_start), UInt(prior_length)
                    )
                    if launch_proxy_token_equal(previous, token):
                        exists = True
                        break
                if not exists:
                    if selected_count >= output_capacity:
                        return 3
                    output[unsafe_offset=selected_count * 3] = source_index
                    output[unsafe_offset=selected_count * 3 + 1] = start
                    output[unsafe_offset=selected_count * 3 + 2] = length
                    selected_count += 1
            token_start = cursor + 1
    written[unsafe_offset=0] = selected_count
    return 0
