from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_trim_bounds, rich_unicode_space, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime CONTEXT_BLOB_NOISE_ABI_VERSION: Int64 = 1
comptime CONTEXT_BLOB_NOISE_STATUS_OK: Int64 = 0
comptime CONTEXT_BLOB_NOISE_STATUS_INVALID: Int64 = 1
comptime CONTEXT_BLOB_NOISE_STATUS_UTF8: Int64 = 2
comptime CONTEXT_BLOB_NOISE_STATUS_ABI: Int64 = 4
comptime CONTEXT_BLOB_NOISE_OUTPUT_FIELDS: Int64 = 22

comptime OUT_ABI: Int64 = 0
comptime OUT_PRIMARY_BINARY: Int64 = 1
comptime OUT_BINARY_PRESENT: Int64 = 2
comptime OUT_BINARY_FIRST_LINE: Int64 = 3
comptime OUT_BINARY_SCORE: Int64 = 4
comptime OUT_BINARY_SUSPICIOUS: Int64 = 5
comptime OUT_BINARY_NUL: Int64 = 6
comptime OUT_BINARY_REPLACEMENT: Int64 = 7
comptime OUT_BASE64_PRESENT: Int64 = 8
comptime OUT_BASE64_LINE: Int64 = 9
comptime OUT_BASE64_BYTES: Int64 = 10
comptime OUT_BASE64_SCORE: Int64 = 11
comptime OUT_MINIFIED_PRESENT: Int64 = 12
comptime OUT_MINIFIED_KIND: Int64 = 13
comptime OUT_MINIFIED_SCORE: Int64 = 14
comptime OUT_MINIFIED_MAX_LINE_BYTES: Int64 = 15
comptime OUT_LOCK_PRESENT: Int64 = 16
comptime OUT_LOCK_KIND: Int64 = 17
comptime OUT_LOCK_SCORE: Int64 = 18
comptime OUT_LOCK_LINE: Int64 = 19
comptime OUT_LOCK_VALUE: Int64 = 20
comptime OUT_NON_EMPTY_LINES: Int64 = 21

comptime LINE_PRIMARY_BASE64: Int64 = 1
comptime LINE_PRIMARY_MINIFIED: Int64 = 2


def blob_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def blob_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value


def blob_is_base64(value: UInt8) -> Bool:
    return (
        value >= 65 and value <= 90
        or value >= 97 and value <= 122
        or value >= 48 and value <= 57
        or value == 43 or value == 47 or value == 61 or value == 45 or value == 95
    )


def blob_ascii_contains(view: ProdexRichStringView, start: Int64, end: Int64, needle: StringSlice) -> Bool:
    var length = Int64(needle.byte_length())
    if length == 0:
        return True
    if start < 0 or end < start or end - start < length:
        return False
    var expected = needle.unsafe_ptr()
    for offset in range(end - start - length + 1):
        var matches = True
        for index in range(length):
            if blob_ascii_lower(blob_byte(view, start + offset + index)) != blob_ascii_lower(expected[unsafe_offset=index]):
                matches = False
                break
        if matches:
            return True
    return False


def blob_raw_count(view: ProdexRichStringView, start: Int64, end: Int64, needle: StringSlice) -> Int64:
    var length = Int64(needle.byte_length())
    if length == 0 or start < 0 or end < start or end - start < length:
        return 0
    var expected = needle.unsafe_ptr()
    var count: Int64 = 0
    var offset: Int64 = 0
    while offset <= end - start - length:
        var matches = True
        for index in range(length):
            if blob_byte(view, start + offset + index) != expected[unsafe_offset=index]:
                matches = False
                break
        if matches:
            count += 1
            offset += length
        else:
            offset += 1
    return count


def blob_line_end(view: ProdexRichStringView, start: Int64) -> Int64:
    var end = start
    while end < Int64(view.len) and blob_byte(view, end) != 10:
        end += 1
    if end > start and blob_byte(view, end - 1) == 13:
        return end - 1
    return end


def blob_next_line_start(view: ProdexRichStringView, start: Int64) -> Int64:
    var end = start
    while end < Int64(view.len) and blob_byte(view, end) != 10:
        end += 1
    return end + 1 if end < Int64(view.len) else Int64(view.len)


def blob_codepoint_count(view: ProdexRichStringView, start: Int64, end: Int64) -> Int64:
    var count: Int64 = 0
    var index = start
    var ptr = rich_view_ptr(view)
    while index < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if width <= 0 or index + width > end:
            return -1
        count += 1
        index += width
    return count


def blob_whitespace_count(view: ProdexRichStringView, start: Int64, end: Int64) -> Int64:
    var count: Int64 = 0
    var index = start
    var ptr = rich_view_ptr(view)
    while index < end:
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if width <= 0 or index + width > end:
            return -1
        if rich_unicode_space(rich_codepoint(ptr, index, width)):
            count += 1
        index += width
    return count


def blob_primary_base64(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    var length = end - start
    if length <= 0:
        return False
    var base64: Int64 = 0
    var upper = False
    var lower = False
    var digit = False
    for index in range(start, end):
        var value = blob_byte(view, index)
        if blob_is_base64(value):
            base64 += 1
        if value >= 65 and value <= 90:
            upper = True
        elif value >= 97 and value <= 122:
            lower = True
        elif value >= 48 and value <= 57:
            digit = True
    return base64 * 100 >= length * 95 and upper and lower and digit


def blob_primary_minified(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    var punctuation: Int64 = 0
    for index in range(start, end):
        var value = blob_byte(view, index)
        if value == 123 or value == 125 or value == 91 or value == 93 or value == 58 or value == 44 or value == 59:
            punctuation += 1
    var spaces = blob_whitespace_count(view, start, end)
    return spaces >= 0 and punctuation >= 32 and spaces * 80 < end - start


def blob_base64_score(view: ProdexRichStringView, start: Int64, end: Int64, min_bytes: Int64, min_unique: Int64) -> Int64:
    var left = start
    var right = end
    while left < right:
        var value = blob_byte(view, left)
        if value == 34 or value == 39 or value == 96 or value == 44 or value == 59:
            left += 1
        else:
            break
    while right > left:
        var value = blob_byte(view, right - 1)
        if value == 34 or value == 39 or value == 96 or value == 44 or value == 59:
            right -= 1
        else:
            break
    var length = right - left
    if length < min_bytes:
        return -1
    var seen_low: UInt64 = 0
    var seen_high: UInt64 = 0
    var base64: Int64 = 0
    var upper: Int64 = 0
    var lower: Int64 = 0
    var digit: Int64 = 0
    var symbol: Int64 = 0
    for index in range(left, right):
        var value = blob_byte(view, index)
        if value >= 128:
            return -1
        if blob_is_base64(value):
            base64 += 1
            if value < 64:
                seen_low |= UInt64(1) << UInt64(value)
            else:
                seen_high |= UInt64(1) << UInt64(value - 64)
            if value >= 65 and value <= 90:
                upper += 1
            elif value >= 97 and value <= 122:
                lower += 1
            elif value >= 48 and value <= 57:
                digit += 1
            else:
                symbol += 1
    var unique: Int64 = 0
    var low = seen_low
    var high = seen_high
    while low != 0:
        unique += Int64(low & 1)
        low >>= 1
    while high != 0:
        unique += Int64(high & 1)
        high >>= 1
    var ratio = base64 * 100 // max(length, Int64(1))
    var classes: Int64 = 0
    if upper > 0: classes += 1
    if lower > 0: classes += 1
    if digit > 0: classes += 1
    if symbol > 0: classes += 1
    if ratio < 96 or unique < min_unique or classes < 2:
        return -1
    return min(Int64(100), Int64(70) + (ratio - 96) * 8)


def blob_update_base64_span(view: ProdexRichStringView, start: Int64, end: Int64, output: Pointer[mut=True, Int64, _], line: Int64):
    var score = blob_base64_score(view, start, end, 160, 16)
    if score >= 0 and end - start > output[unsafe_offset=OUT_BASE64_BYTES]:
        output[unsafe_offset=OUT_BASE64_PRESENT] = 1
        output[unsafe_offset=OUT_BASE64_LINE] = line
        output[unsafe_offset=OUT_BASE64_BYTES] = end - start
        output[unsafe_offset=OUT_BASE64_SCORE] = score


def blob_scan_binary(view: ProdexRichStringView, output: Pointer[mut=True, Int64, _]):
    var total_chars = blob_codepoint_count(view, 0, Int64(view.len))
    if total_chars < 0:
        return
    var index: Int64 = 0
    var line: Int64 = 1
    var first_line: Int64 = -1
    var suspicious: Int64 = 0
    var nul: Int64 = 0
    var replacement: Int64 = 0
    var ptr = rich_view_ptr(view)
    while index < Int64(view.len):
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        var cp = rich_codepoint(ptr, index, width)
        if cp == 0:
            output[unsafe_offset=OUT_PRIMARY_BINARY] = 1
            nul += 1
            suspicious += 1
            if first_line < 0: first_line = line
        elif cp == 0xFFFD:
            output[unsafe_offset=OUT_PRIMARY_BINARY] = 1
            replacement += 1
            suspicious += 1
            if first_line < 0: first_line = line
        elif (cp >= 0 and cp < 32 or cp == 127) and cp != 9 and cp != 10 and cp != 13:
            suspicious += 1
            if first_line < 0: first_line = line
        if cp == 10:
            line += 1
        index += width
    if total_chars < 16:
        return
    var ratio = suspicious * 100 // max(total_chars, Int64(1))
    if nul > 0 or replacement >= 2 or suspicious >= 4 and ratio >= 2:
        output[unsafe_offset=OUT_BINARY_PRESENT] = 1
        output[unsafe_offset=OUT_BINARY_FIRST_LINE] = first_line
        output[unsafe_offset=OUT_BINARY_SCORE] = min(Int64(100), Int64(60) + ratio)
        output[unsafe_offset=OUT_BINARY_SUSPICIOUS] = suspicious
        output[unsafe_offset=OUT_BINARY_NUL] = nul
        output[unsafe_offset=OUT_BINARY_REPLACEMENT] = replacement


def blob_scan_lines(view: ProdexRichStringView, line_flags: Pointer[mut=True, Int64, _], line_count: Int64, output: Pointer[mut=True, Int64, _]):
    var start: Int64 = 0
    var line: Int64 = 0
    var block_start: Int64 = 0
    var block_lines: Int64 = 0
    var block_bytes: Int64 = 0
    var block_score: Int64 = 0
    var cargo_packages: Int64 = 0
    var cargo_checksums: Int64 = 0
    var non_empty: Int64 = 0
    var vendor_lines: Int64 = 0
    var first_vendor: Int64 = -1
    var max_trimmed: Int64 = 0
    while line < line_count:
        var end = blob_line_end(view, start)
        var bounds = rich_trim_bounds(ProdexRichStringView(view.ptr + UInt(start), UInt(end - start)))
        var left = start + bounds[0]
        var right = start + bounds[1]
        var length = right - left
        if length > 0:
            non_empty += 1
        if length > max_trimmed:
            max_trimmed = length
        var flags: Int64 = 0
        if length >= 512 and blob_primary_base64(view, left, right):
            flags |= LINE_PRIMARY_BASE64
        if length >= 800 and blob_primary_minified(view, left, right):
            flags |= LINE_PRIMARY_MINIFIED
        line_flags[unsafe_offset=line] = flags

        var span_start: Int64 = -1
        var index = left
        while index < right:
            if blob_is_base64(blob_byte(view, index)):
                if span_start < 0: span_start = index
            elif span_start >= 0:
                blob_update_base64_span(view, span_start, index, output, line + 1)
                span_start = -1
            index += 1
        if span_start >= 0:
            blob_update_base64_span(view, span_start, right, output, line + 1)

        var score = blob_base64_score(view, left, right, 56, 12)
        if score >= 0:
            if block_lines == 0: block_start = line + 1
            block_lines += 1
            block_bytes += length
            block_score = max(block_score, score)
        else:
            if block_lines >= 4 and block_bytes >= 240 and block_bytes > output[unsafe_offset=OUT_BASE64_BYTES]:
                output[unsafe_offset=OUT_BASE64_PRESENT] = 1
                output[unsafe_offset=OUT_BASE64_LINE] = block_start
                output[unsafe_offset=OUT_BASE64_BYTES] = block_bytes
                output[unsafe_offset=OUT_BASE64_SCORE] = block_score
            block_lines = 0
            block_bytes = 0
            block_score = 0

        if length == 11 and blob_ascii_contains(view, left, right, StringSlice("[[package]]")):
            cargo_packages += 1
        if right - left >= 11:
            var prefix = StringSlice("checksum = ").unsafe_ptr()
            var prefix_ok = True
            for p in range(11):
                if blob_byte(view, left + Int64(p)) != prefix[unsafe_offset=p]: prefix_ok = False
            if prefix_ok: cargo_checksums += 1

        if (
            blob_ascii_contains(view, left, right, StringSlice("node_modules/"))
            or blob_ascii_contains(view, left, right, StringSlice("vendor/"))
            or blob_ascii_contains(view, left, right, StringSlice("third_party/"))
            or blob_ascii_contains(view, left, right, StringSlice(".cargo/registry/"))
            or blob_ascii_contains(view, left, right, StringSlice(".pnpm/"))
            or blob_ascii_contains(view, left, right, StringSlice(".yarn/cache/"))
        ):
            vendor_lines += 1
            if first_vendor < 0: first_vendor = line + 1

        start = blob_next_line_start(view, start)
        line += 1
    if block_lines >= 4 and block_bytes >= 240 and block_bytes > output[unsafe_offset=OUT_BASE64_BYTES]:
        output[unsafe_offset=OUT_BASE64_PRESENT] = 1
        output[unsafe_offset=OUT_BASE64_LINE] = block_start
        output[unsafe_offset=OUT_BASE64_BYTES] = block_bytes
        output[unsafe_offset=OUT_BASE64_SCORE] = block_score

    output[unsafe_offset=OUT_MINIFIED_MAX_LINE_BYTES] = max_trimmed
    output[unsafe_offset=OUT_NON_EMPTY_LINES] = non_empty
    if cargo_packages >= 4 and cargo_checksums >= 2:
        output[unsafe_offset=OUT_LOCK_PRESENT] = 1
        output[unsafe_offset=OUT_LOCK_KIND] = 1
        output[unsafe_offset=OUT_LOCK_SCORE] = 95
        output[unsafe_offset=OUT_LOCK_LINE] = 1
        output[unsafe_offset=OUT_LOCK_VALUE] = cargo_packages
    elif vendor_lines >= 8 and vendor_lines * 100 >= max(non_empty, Int64(1)) * 40:
        output[unsafe_offset=OUT_LOCK_PRESENT] = 1
        output[unsafe_offset=OUT_LOCK_KIND] = 4
        output[unsafe_offset=OUT_LOCK_SCORE] = 85
        output[unsafe_offset=OUT_LOCK_LINE] = first_vendor
        output[unsafe_offset=OUT_LOCK_VALUE] = vendor_lines


def blob_scan_minified_and_lock(input: ProdexRichStringView, output: Pointer[mut=True, Int64, _]):
    var bounds = rich_trim_bounds(input)
    var left = bounds[0]
    var right = bounds[1]
    var length = right - left
    if length >= 512 and not (output[unsafe_offset=OUT_NON_EMPTY_LINES] > 4 and output[unsafe_offset=OUT_MINIFIED_MAX_LINE_BYTES] < 512):
        var chars = blob_codepoint_count(input, left, right)
        var whitespace = blob_whitespace_count(input, left, right)
        if chars > 0 and whitespace >= 0 and whitespace * 100 <= chars * 5:
            var punctuation: Int64 = 0
            for index in range(left, right):
                var value = blob_byte(input, index)
                if value == 123 or value == 125 or value == 91 or value == 93 or value == 40 or value == 41 or value == 58 or value == 44 or value == 59:
                    punctuation += 1
            if punctuation * 100 >= chars * 8:
                var json = False
                if right > left:
                    var first = blob_byte(input, left)
                    var last = blob_byte(input, right - 1)
                    json = (first == 123 and last == 125 or first == 91 and last == 93) and blob_raw_count(input, left, right, StringSlice(":")) >= 8 and blob_raw_count(input, left, right, StringSlice(",")) >= 8 and blob_raw_count(input, left, right, StringSlice("\"")) >= 16
                if json:
                    output[unsafe_offset=OUT_MINIFIED_PRESENT] = 1
                    output[unsafe_offset=OUT_MINIFIED_KIND] = 1
                    output[unsafe_offset=OUT_MINIFIED_SCORE] = 92
                else:
                    var markers = blob_raw_count(input, left, right, StringSlice("function(")) + blob_raw_count(input, left, right, StringSlice("=>")) + blob_raw_count(input, left, right, StringSlice("module.exports")) + blob_raw_count(input, left, right, StringSlice("exports."))
                    var semicolons = blob_raw_count(input, left, right, StringSlice(";"))
                    var braces = blob_raw_count(input, left, right, StringSlice("{")) + blob_raw_count(input, left, right, StringSlice("}"))
                    var parens = blob_raw_count(input, left, right, StringSlice("(")) + blob_raw_count(input, left, right, StringSlice(")"))
                    if markers >= 2 and (semicolons >= 8 or braces + parens >= 32):
                        output[unsafe_offset=OUT_MINIFIED_PRESENT] = 1
                        output[unsafe_offset=OUT_MINIFIED_KIND] = 2
                        output[unsafe_offset=OUT_MINIFIED_SCORE] = 88

    if output[unsafe_offset=OUT_LOCK_PRESENT] == 0:
        if blob_ascii_contains(input, 0, Int64(input.len), StringSlice("\"lockfileversion\"")) and (blob_ascii_contains(input, 0, Int64(input.len), StringSlice("\"packages\"")) or blob_ascii_contains(input, 0, Int64(input.len), StringSlice("\"dependencies\""))):
            output[unsafe_offset=OUT_LOCK_PRESENT] = 1
            output[unsafe_offset=OUT_LOCK_KIND] = 2
            output[unsafe_offset=OUT_LOCK_SCORE] = 95
            output[unsafe_offset=OUT_LOCK_LINE] = 1
        elif blob_ascii_contains(input, 0, Int64(input.len), StringSlice("# yarn lockfile")) or blob_ascii_contains(input, 0, Int64(input.len), StringSlice("pnpm-lock.yaml")) or blob_ascii_contains(input, 0, Int64(input.len), StringSlice("lockfileversion:")) and (blob_ascii_contains(input, 0, Int64(input.len), StringSlice("importers:")) or blob_ascii_contains(input, 0, Int64(input.len), StringSlice("packages:"))):
            output[unsafe_offset=OUT_LOCK_PRESENT] = 1
            output[unsafe_offset=OUT_LOCK_KIND] = 3
            output[unsafe_offset=OUT_LOCK_SCORE] = 90
            output[unsafe_offset=OUT_LOCK_LINE] = 1


@export("prodex_context_blob_noise_analyze_v1")
def prodex_context_blob_noise_analyze_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    normalized_address: UInt,
    normalized_length: Int64,
    line_flags_address: UInt,
    line_count: Int64,
    output_address: UInt,
    output_count: Int64,
) abi("C") -> Int64:
    if abi_version != CONTEXT_BLOB_NOISE_ABI_VERSION:
        return CONTEXT_BLOB_NOISE_STATUS_ABI
    if input_length < 0 or normalized_length < 0 or line_count < 0 or output_count != CONTEXT_BLOB_NOISE_OUTPUT_FIELDS or output_address == 0:
        return CONTEXT_BLOB_NOISE_STATUS_INVALID
    if input_length > 0 and input_address == 0 or normalized_length > 0 and normalized_address == 0 or line_count > 0 and line_flags_address == 0:
        return CONTEXT_BLOB_NOISE_STATUS_INVALID
    var input = ProdexRichStringView(input_address, UInt(input_length))
    var normalized = ProdexRichStringView(normalized_address, UInt(normalized_length))
    if not rich_view_valid(input, 9223372036854775807) or not rich_view_valid(normalized, 9223372036854775807):
        return CONTEXT_BLOB_NOISE_STATUS_UTF8
    var flags = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(line_flags_address))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(output_count):
        output[unsafe_offset=index] = 0
    output[unsafe_offset=OUT_ABI] = CONTEXT_BLOB_NOISE_ABI_VERSION
    output[unsafe_offset=OUT_BINARY_FIRST_LINE] = -1
    output[unsafe_offset=OUT_BASE64_LINE] = -1
    output[unsafe_offset=OUT_LOCK_LINE] = -1
    blob_scan_binary(input, output)
    blob_scan_lines(normalized, flags, line_count, output)
    blob_scan_minified_and_lock(input, output)
    return CONTEXT_BLOB_NOISE_STATUS_OK
