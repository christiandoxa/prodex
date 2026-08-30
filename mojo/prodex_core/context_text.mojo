from std.memory import Pointer
from std.sys.info import align_of, size_of


comptime CONTEXT_SIGNAL_COUNTER_COUNT: Int64 = 7
comptime CONTEXT_SIGNAL_ROW_WIDTH: Int64 = 8
comptime CONTEXT_SIGNAL_MAX_LINES: Int64 = 65_536
comptime CONTEXT_SIGNAL_MAX_KEYS: Int64 = 65_536
comptime CONTEXT_TEXT_ABI_VERSION: Int64 = 1
comptime CONTEXT_TEXT_SOURCE_AFTER: Int64 = 0
comptime CONTEXT_TEXT_SOURCE_BEFORE: Int64 = 1
comptime CONTEXT_TEXT_HASH_SEED: UInt64 = 1469598103934665603
comptime CONTEXT_GIT_SEARCH_MAX_BYTES: Int64 = 131_072
comptime CONTEXT_GIT_SEARCH_RESULT_WIDTH: Int64 = 4
comptime CONTEXT_GIT_SEARCH_DIRECT_MATCH: Int64 = 1
comptime CONTEXT_GIT_SEARCH_JSON_MATCH: Int64 = 2
comptime CONTEXT_GIT_SEARCH_JSON_LINE: Int64 = 4
comptime CONTEXT_GIT_SEARCH_HEADING_PATH: Int64 = 8
comptime CONTEXT_GIT_SEARCH_HEADING_MATCH: Int64 = 16
comptime CONTEXT_GIT_SEARCH_CAPACITY: Int64 = -2


@fieldwise_init
struct ProdexStringView(Copyable):
    var ptr: Optional[Pointer[mut=False, UInt8, ImmUntrackedOrigin]]
    var len: UInt


@fieldwise_init
struct ProdexBytesView(Copyable):
    var ptr: Optional[Pointer[mut=False, UInt8, ImmUntrackedOrigin]]
    var len: UInt


@fieldwise_init
struct ContextTextRowsResult(Copyable):
    var abi_version: Int64
    var before_line_count: Int64
    var after_line_count: Int64
    var before_rows_written: Int64
    var key_count: Int64
    var after_signal_line_count: Int64
    var required_before_rows: Int64
    var required_key_capacity: Int64
    var required_hash_capacity: Int64


def context_text_is_utf8_continuation(value: UInt8) -> Bool:
    return value >= 0x80 and value <= 0xBF


def context_text_is_valid_utf8(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    var index: Int64 = 0
    while index < length:
        var lead = ptr[unsafe_offset=index]
        if lead <= 0x7F:
            index += 1
        elif lead >= 0xC2 and lead <= 0xDF:
            if index + 1 >= length or not context_text_is_utf8_continuation(
                ptr[unsafe_offset=index + 1]
            ):
                return False
            index += 2
        elif lead == 0xE0:
            if index + 2 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0xA0
                or second > 0xBF
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif (lead >= 0xE1 and lead <= 0xEC) or (lead >= 0xEE and lead <= 0xEF):
            if (
                index + 2 >= length
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 1]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif lead == 0xED:
            if index + 2 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x80
                or second > 0x9F
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif lead == 0xF0:
            if index + 3 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x90
                or second > 0xBF
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        elif lead >= 0xF1 and lead <= 0xF3:
            if (
                index + 3 >= length
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 1]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        elif lead == 0xF4:
            if index + 3 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x80
                or second > 0x8F
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        else:
            return False
    return True


def context_text_view_is_valid(view: ProdexStringView) -> Bool:
    if view.len == 0:
        return True
    if not view.ptr or view.len > UInt(9223372036854775807):
        return False
    return context_text_is_valid_utf8(view.ptr.unsafe_value(), Int64(view.len))


def context_text_hash(view: ProdexStringView) -> UInt64:
    var value = CONTEXT_TEXT_HASH_SEED ^ UInt64(view.len)
    if view.len == 0:
        return value
    ref ptr = view.ptr.unsafe_value()
    var text = StringSlice(
        unsafe_from_utf8=Span[UInt8](unsafe_ptr=ptr, length=Int(view.len))
    )
    var text_ptr = text.unsafe_ptr()
    for index in range(Int64(text.byte_length())):
        value = value ^ UInt64(text_ptr[unsafe_offset=index])
        value = (value << 7) | (value >> 57)
        value = value ^ (value >> 17)
    return value


def context_text_views_equal(
    left: ProdexStringView, right: ProdexStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    ref left_ptr = left.ptr.unsafe_value()
    ref right_ptr = right.ptr.unsafe_value()
    var left_text = StringSlice(
        unsafe_from_utf8=Span[UInt8](unsafe_ptr=left_ptr, length=Int(left.len))
    )
    var right_text = StringSlice(
        unsafe_from_utf8=Span[UInt8](
            unsafe_ptr=right_ptr, length=Int(right.len)
        )
    )
    return left_text == right_text


def context_text_key_matches(
    view: ProdexStringView,
    key_id: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    after_views: Pointer[mut=False, ProdexStringView, _],
    key_sources: Pointer[mut=False, Int64, _],
    key_indices: Pointer[mut=False, Int64, _],
) -> Bool:
    var source = key_sources[unsafe_offset=key_id]
    var index = key_indices[unsafe_offset=key_id]
    if source == CONTEXT_TEXT_SOURCE_AFTER:
        return context_text_views_equal(view, after_views[unsafe_offset=index])
    return context_text_views_equal(view, before_views[unsafe_offset=index])


def context_text_intern(
    view: ProdexStringView,
    source: Int64,
    source_index: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    after_views: Pointer[mut=False, ProdexStringView, _],
    hash_slots: Pointer[mut=True, Int64, _],
    key_hashes: Pointer[mut=True, UInt64, _],
    key_sources: Pointer[mut=True, Int64, _],
    key_indices: Pointer[mut=True, Int64, _],
    after_available: Pointer[mut=True, Int64, _],
    key_count: Pointer[mut=True, Int64, _],
    key_capacity: Int64,
    hash_capacity: Int64,
) -> Int64:
    var hash = context_text_hash(view)
    var slot = Int64(hash % UInt64(hash_capacity))
    for _ in range(hash_capacity):
        var key_id = hash_slots[unsafe_offset=slot]
        if key_id < 0:
            key_id = key_count[]
            if key_id >= key_capacity:
                return -1
            hash_slots[unsafe_offset=slot] = key_id
            key_hashes[unsafe_offset=key_id] = hash
            key_sources[unsafe_offset=key_id] = source
            key_indices[unsafe_offset=key_id] = source_index
            after_available[unsafe_offset=key_id] = 0
            key_count[] = key_id + 1
            return key_id
        if key_hashes[
            unsafe_offset=key_id
        ] == hash and context_text_key_matches(
            view,
            key_id,
            before_views,
            after_views,
            key_sources,
            key_indices,
        ):
            return key_id
        slot += 1
        if slot == hash_capacity:
            slot = 0
    return -1


def context_text_required_hash_capacity(key_capacity: Int64) -> Int64:
    var required: Int64 = 1
    var target = key_capacity * 2
    while required < target:
        required *= 2
    return required


comptime CONTEXT_OUTPUT_MAX_BYTES: Int64 = 131_072
comptime CONTEXT_OUTPUT_LABEL_NONE: Int64 = 0
comptime CONTEXT_OUTPUT_LABEL_COVERAGE: Int64 = 1
comptime CONTEXT_OUTPUT_LABEL_GRADLE_TEST: Int64 = 2
comptime CONTEXT_OUTPUT_LABEL_MAVEN_TEST: Int64 = 3
comptime CONTEXT_OUTPUT_LABEL_PACKAGE_INSTALL: Int64 = 4
comptime CONTEXT_OUTPUT_LABEL_DOCKER_BUILDX: Int64 = 5
comptime CONTEXT_OUTPUT_LABEL_BAZEL_TEST: Int64 = 6
comptime CONTEXT_OUTPUT_LABEL_JUNIT_XML: Int64 = 7
comptime CONTEXT_OUTPUT_LABEL_SWIFT_TEST: Int64 = 8
comptime CONTEXT_OUTPUT_LABEL_PLAYWRIGHT: Int64 = 9
comptime CONTEXT_OUTPUT_LABEL_BIOME: Int64 = 10
comptime CONTEXT_OUTPUT_LABEL_OXLINT: Int64 = 11
comptime CONTEXT_OUTPUT_LABEL_COMPILING: Int64 = 12
comptime CONTEXT_OUTPUT_LABEL_CHECKING: Int64 = 13
comptime CONTEXT_OUTPUT_LABEL_FRESH: Int64 = 14
comptime CONTEXT_OUTPUT_LABEL_DOCUMENTING: Int64 = 15
comptime CONTEXT_OUTPUT_LABEL_FORMATTING: Int64 = 16
comptime CONTEXT_OUTPUT_LABEL_CARGO_FIX: Int64 = 17
comptime CONTEXT_OUTPUT_LABEL_GENERATED_DOCS: Int64 = 18
comptime CONTEXT_OUTPUT_LABEL_FINISHED: Int64 = 19
comptime CONTEXT_OUTPUT_LABEL_RUNNING_TARGETS: Int64 = 20
comptime CONTEXT_OUTPUT_LABEL_DOC_TESTS: Int64 = 21
comptime CONTEXT_OUTPUT_LABEL_RUNNING_TESTS: Int64 = 22
comptime CONTEXT_OUTPUT_LABEL_PASSED_TESTS: Int64 = 23
comptime CONTEXT_OUTPUT_LABEL_NEXTEST_PASS: Int64 = 24
comptime CONTEXT_OUTPUT_LABEL_NEXTEST_SUMMARY: Int64 = 25
comptime CONTEXT_OUTPUT_LABEL_TEST_RESULT_OK: Int64 = 26
comptime CONTEXT_OUTPUT_LABEL_TYPECHECK: Int64 = 27
comptime CONTEXT_OUTPUT_LABEL_VITE: Int64 = 28
comptime CONTEXT_OUTPUT_LABEL_NEXT: Int64 = 29
comptime CONTEXT_OUTPUT_LABEL_DOT_PROGRESS: Int64 = 30
comptime CONTEXT_OUTPUT_LABEL_BUN_TEST: Int64 = 31
comptime CONTEXT_OUTPUT_LABEL_CYPRESS: Int64 = 32
comptime CONTEXT_OUTPUT_LABEL_ZIG_TEST: Int64 = 33
comptime CONTEXT_OUTPUT_LABEL_PASSED_SUITES: Int64 = 34
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_OK: Int64 = 35
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_NO_FILES: Int64 = 36
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_RUN: Int64 = 37
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_PAUSE: Int64 = 38
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_CONT: Int64 = 39
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_PASS: Int64 = 40
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_SKIP: Int64 = 41
comptime CONTEXT_OUTPUT_LABEL_GO_TEST_SUMMARY: Int64 = 42
comptime CONTEXT_OUTPUT_LABEL_TEST_SUITES: Int64 = 43
comptime CONTEXT_OUTPUT_LABEL_TEST_CASES: Int64 = 44
comptime CONTEXT_OUTPUT_LABEL_SNAPSHOTS: Int64 = 45
comptime CONTEXT_OUTPUT_LABEL_TEST_FILES: Int64 = 46
comptime CONTEXT_OUTPUT_LABEL_TEST_DURATION: Int64 = 47
comptime CONTEXT_OUTPUT_LABEL_TEST_TIME: Int64 = 48
comptime CONTEXT_OUTPUT_LABEL_TEST_RUNNER: Int64 = 49
comptime CONTEXT_OUTPUT_LABEL_DONE: Int64 = 50
comptime CONTEXT_OUTPUT_LABEL_BUILD_SUCCESS: Int64 = 51
comptime CONTEXT_OUTPUT_LABEL_BUILD_STEPS: Int64 = 52
comptime CONTEXT_OUTPUT_LABEL_BAZEL_STEPS: Int64 = 53
comptime CONTEXT_OUTPUT_LABEL_BAZEL_SUMMARY: Int64 = 54
comptime CONTEXT_OUTPUT_LABEL_NX_SUMMARY: Int64 = 55
comptime CONTEXT_OUTPUT_LABEL_TURBO_SUMMARY: Int64 = 56
comptime CONTEXT_OUTPUT_LABEL_GRADLE_TASKS: Int64 = 57
comptime CONTEXT_OUTPUT_LABEL_MAVEN_SUMMARY: Int64 = 58
comptime CONTEXT_OUTPUT_LABEL_DOCKER_STEPS: Int64 = 59
comptime CONTEXT_OUTPUT_LABEL_DOCKER_COMPOSE: Int64 = 60
comptime CONTEXT_OUTPUT_LABEL_DOCKER_SUMMARY: Int64 = 61
comptime CONTEXT_OUTPUT_LABEL_PLAYWRIGHT_RUNNING: Int64 = 62
comptime CONTEXT_OUTPUT_LABEL_TEST_SUMMARY: Int64 = 63
comptime CONTEXT_OUTPUT_LABEL_PACKAGES_ADDED: Int64 = 64
comptime CONTEXT_OUTPUT_LABEL_PACKAGES_AUDITED: Int64 = 65
comptime CONTEXT_OUTPUT_LABEL_CARGO_INDEX: Int64 = 66
comptime CONTEXT_OUTPUT_LABEL_CARGO_LOCK: Int64 = 67
comptime CONTEXT_OUTPUT_LABEL_CARGO_DOWNLOAD: Int64 = 68
comptime CONTEXT_OUTPUT_LABEL_PACKAGE_PROGRESS: Int64 = 69
comptime CONTEXT_OUTPUT_LABEL_PACKAGES_UP_TO_DATE: Int64 = 70
comptime CONTEXT_OUTPUT_LABEL_PYTHON_PACKAGES: Int64 = 71
comptime CONTEXT_OUTPUT_LABEL_VULNERABILITY: Int64 = 72
comptime CONTEXT_OUTPUT_LABEL_FORMATTER: Int64 = 73
comptime CONTEXT_OUTPUT_LABEL_BUILD_SUMMARY: Int64 = 74
comptime CONTEXT_OUTPUT_LABEL_COMPILE_SUMMARY: Int64 = 75
comptime CONTEXT_OUTPUT_LABEL_PYTEST_PROGRESS: Int64 = 76
comptime CONTEXT_OUTPUT_LABEL_NPM_SCRIPT: Int64 = 77
comptime CONTEXT_OUTPUT_LABEL_PYTEST_COLLECTING: Int64 = 78
comptime CONTEXT_OUTPUT_LABEL_PYTEST_COLLECTED: Int64 = 79

comptime CONTEXT_OUTPUT_RUST_STRONG: Int64 = 1
comptime CONTEXT_OUTPUT_RUST_LOCATION: Int64 = 2
comptime CONTEXT_OUTPUT_RUST_BACKTRACE: Int64 = 4
comptime CONTEXT_OUTPUT_RUST_EXIT: Int64 = 8
comptime CONTEXT_OUTPUT_RUST_NOISE: Int64 = 16
comptime CONTEXT_OUTPUT_CLIPPY: Int64 = 32
comptime CONTEXT_OUTPUT_DIAGNOSTIC_TARGET: Int64 = 64
comptime CONTEXT_OUTPUT_NOISY_KEY: Int64 = 128
comptime CONTEXT_OUTPUT_FAILURE: Int64 = 256
comptime CONTEXT_OUTPUT_WARNING: Int64 = 512
comptime CONTEXT_OUTPUT_DIAGNOSTIC_SUCCESS: Int64 = 1024
comptime CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE: Int64 = 2048
comptime CONTEXT_OUTPUT_TYPESCRIPT: Int64 = 4096
comptime CONTEXT_OUTPUT_ESLINT: Int64 = 8192
comptime CONTEXT_OUTPUT_EXCEPTION: Int64 = 16_384
comptime CONTEXT_OUTPUT_JUNIT_FAILURE: Int64 = 32_768


@fieldwise_init
struct ContextCommandOutputLineResult(Copyable):
    var flags: Int64
    var noisy_label: Int64
    var diagnostic_label: Int64


def context_output_starts[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_text_ascii_starts_at[literal](ptr, start, end)


def context_output_starts_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        end - start == Int64(literal.byte_length())
        and context_text_ascii_starts_exact[literal](ptr, start, end)
    )


def context_output_starts_raw[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    return length <= end - start and context_text_ascii_starts_exact[literal](
        ptr, start, end
    )


def context_output_equals[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    return length == end - start and context_text_ascii_starts_at[literal](
        ptr, start, end
    )


def context_output_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_text_ascii_contains[literal](ptr, start, end)


def context_output_contains_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if start < 0 or end < start or start + length > end:
        return False
    for offset in range(start, end - length + 1):
        if context_text_ascii_starts_exact[literal](ptr, offset, end):
            return True
    return False


def context_output_find_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var length = Int64(literal.byte_length())
    if length == 0:
        return start
    if start < 0 or end < start or start + length > end:
        return -1
    for offset in range(start, end - length + 1):
        if context_text_ascii_starts_exact[literal](ptr, offset, end):
            return offset
    return -1


def context_output_ends[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    return length <= end - start and context_text_ascii_starts_at[literal](
        ptr, end - length, end
    )


def context_output_ends_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    return length <= end - start and context_text_ascii_starts_exact[literal](
        ptr, end - length, end
    )


def context_output_next_word(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 3]:
    var result = InlineArray[Int64, 3](fill=-1)
    var index = cursor
    while index < length:
        var whitespace = context_text_whitespace_width(ptr, index, length)
        if whitespace > 0:
            index += whitespace
        else:
            break
    if index >= length:
        return result^
    var start = index
    while index < length:
        var whitespace = context_text_whitespace_width(ptr, index, length)
        if whitespace > 0:
            break
        index += context_text_codepoint_width(ptr[unsafe_offset=index])
    result[0] = start
    result[1] = index
    result[2] = index
    return result^


def context_output_parse_digits(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if start >= end:
        return -1
    var value: Int64 = 0
    for index in range(start, end):
        var digit = ptr[unsafe_offset=index]
        if digit < 48 or digit > 57:
            return -1
        var number = Int64(digit - 48)
        if value > 922337203685477580 or value * 10 > 9223372036854775807 - number:
            return -1
        value = value * 10 + number
    return value


def context_output_count_after(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var bounds = context_text_trim_bounds(ptr, start, end)
    if bounds[0] >= bounds[1]:
        return -1
    var marker = ptr[unsafe_offset=bounds[0]]
    if marker == 58 or marker == 61 or marker == 40:
        bounds[0] += 1
    else:
        return -1
    bounds = context_text_trim_bounds(ptr, bounds[0], bounds[1])
    var index = bounds[0]
    while index < bounds[1]:
        var value = ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            break
        index += 1
    return context_output_parse_digits(ptr, bounds[0], index)


def context_output_count_before(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var index = end
    while index > start:
        var value = ptr[unsafe_offset=index - 1]
        if (
            value == 32
            or value == 9
            or value == 10
            or value == 11
            or value == 12
            or value == 13
            or value == 44
            or value == 58
            or value == 59
            or value == 40
        ):
            index -= 1
        else:
            break
    var digits_end = index
    while index > start:
        var value = ptr[unsafe_offset=index - 1]
        if value < 48 or value > 57:
            break
        index -= 1
    return context_output_parse_digits(ptr, index, digits_end)


def context_output_count_for_word[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var cursor = start
    var length = Int64(literal.byte_length())
    while cursor < end:
        var found = context_text_ascii_find[literal](ptr, cursor, end)
        if found < 0:
            return -1
        var after = context_output_count_after(ptr, found + length, end)
        if after >= 0:
            return after
        var before = context_output_count_before(ptr, start, found)
        if before >= 0:
            return before
        cursor = found + 1
    return -1


def context_output_has_nonzero_count[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    var length = Int64(literal.byte_length())
    while cursor < end:
        var found = context_text_ascii_find[literal](ptr, cursor, end)
        if found < 0:
            return False
        var count = context_output_count_after(ptr, found + length, end)
        if count < 0:
            count = context_output_count_before(ptr, start, found)
        if count > 0:
            return True
        cursor = found + 1
    return False


def context_output_has_zero_only_count[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    var length = Int64(literal.byte_length())
    var saw_count = False
    while cursor < end:
        var found = context_text_ascii_find[literal](ptr, cursor, end)
        if found < 0:
            break
        var count = context_output_count_after(ptr, found + length, end)
        if count < 0:
            count = context_output_count_before(ptr, start, found)
        if count >= 0:
            saw_count = True
            if count > 0:
                return False
        cursor = found + 1
    return saw_count


def context_output_is_count_word[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var first = context_output_next_word(ptr, end, start)
    if first[0] < 0:
        return False
    var second = context_output_next_word(ptr, end, first[2])
    if second[0] < 0 or second[2] != end:
        return False
    return context_output_parse_digits(ptr, first[0], first[1]) >= 0 and context_output_starts_exact[
        literal
    ](ptr, second[0], second[1])


def context_output_has_ascii_digit(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if value >= 48 and value <= 57:
            return True
    return False


def context_output_all_dots(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if start >= end:
        return False
    for index in range(start, end):
        if ptr[unsafe_offset=index] != 46:
            return False
    return True


def context_output_find_last_byte(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, value: UInt8
) -> Int64:
    var index = end
    while index > start:
        index -= 1
        if ptr[unsafe_offset=index] == value:
            return index
    return -1


def context_output_trim_ascii_punctuation(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=0)
    var first = start
    var last = end
    while first < last:
        var value = ptr[unsafe_offset=first]
        if (
            value == 34
            or value == 39
            or value == 96
            or value == 44
            or value == 59
            or value == 40
            or value == 41
            or value == 91
            or value == 93
            or value == 123
            or value == 125
            or value == 60
            or value == 62
        ):
            first += 1
        else:
            break
    while last > first:
        var value = ptr[unsafe_offset=last - 1]
        if (
            value == 34
            or value == 39
            or value == 96
            or value == 44
            or value == 59
            or value == 40
            or value == 41
            or value == 91
            or value == 93
            or value == 123
            or value == 125
            or value == 60
            or value == 62
        ):
            last -= 1
        else:
            break
    result[0] = first
    result[1] = last
    return result^


def context_output_looks_like_location_path(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var first = start
    var last = end
    while first < last and (
        ptr[unsafe_offset=first] == 60
        or ptr[unsafe_offset=first] == 62
        or ptr[unsafe_offset=first] == 45
        or ptr[unsafe_offset=first] == 58
        or ptr[unsafe_offset=first] == 32
    ):
        first += 1
    while last > first and (
        ptr[unsafe_offset=last - 1] == 60
        or ptr[unsafe_offset=last - 1] == 62
        or ptr[unsafe_offset=last - 1] == 45
        or ptr[unsafe_offset=last - 1] == 58
        or ptr[unsafe_offset=last - 1] == 32
    ):
        last -= 1
    if first >= last:
        return False
    for index in range(first, last):
        var value = ptr[unsafe_offset=index]
        if value == 47 or value == 92:
            return True
    var slash = context_output_find_last_byte(ptr, first, last, 47)
    var name_start = slash + 1 if slash >= first else first
    var dot = context_output_find_last_byte(ptr, name_start, last, 46)
    if dot < name_start or dot + 1 >= last:
        return False
    var extension_length = last - dot - 1
    if extension_length > 12:
        return False
    for index in range(dot + 1, last):
        var value = ptr[unsafe_offset=index]
        if not (
            value >= 48
            and value <= 57
            or value >= 65
            and value <= 90
            or value >= 97
            and value <= 122
            or value == 95
            or value == 45
        ):
            return False
    return True


def context_output_token_has_location(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_output_trim_ascii_punctuation(ptr, start, end)
    var first = bounds[0]
    var last = bounds[1]
    while last > first and ptr[unsafe_offset=last - 1] == 46:
        last -= 1
    if first >= last:
        return False
    var tail_colon = context_output_find_last_byte(ptr, first, last, 58)
    if tail_colon < 0:
        return False
    if context_output_parse_digits(ptr, tail_colon + 1, last) < 0:
        return False
    var middle_colon = context_output_find_last_byte(ptr, first, tail_colon, 58)
    if middle_colon >= 0 and context_output_parse_digits(
        ptr, middle_colon + 1, tail_colon
    ) >= 0:
        return context_output_looks_like_location_path(ptr, first, middle_colon)
    return context_output_looks_like_location_path(ptr, first, tail_colon)


def context_output_paren_location(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var open = context_output_find_last_byte(ptr, start, end, 40)
    if open < 0:
        return False
    var close = open
    while close < end and ptr[unsafe_offset=close] != 41:
        close += 1
    if close >= end:
        return False
    var comma = context_output_find_last_byte(ptr, open + 1, close, 44)
    if comma < 0:
        return False
    return context_output_looks_like_location_path(ptr, start, open)
        and context_output_parse_digits(ptr, open + 1, comma) >= 0
        and context_output_parse_digits(ptr, comma + 1, close) >= 0


def context_output_python_location(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var quote = context_output_find_exact["File \""](ptr, start, end)
    var quote_value: UInt8 = 34
    if quote < 0:
        quote = context_output_find_exact["File '"](ptr, start, end)
        quote_value = 39
    if quote < 0:
        return False
    var path_start = quote + 6
    var path_end = path_start
    while path_end < end and ptr[unsafe_offset=path_end] != quote_value:
        path_end += 1
    if path_end >= end or not context_output_looks_like_location_path(
        ptr, path_start, path_end
    ):
        return False
    var marker = context_text_ascii_find[", line "](ptr, path_end + 1, end)
    if marker < 0:
        return False
    var line_start = marker + 7
    while line_start < end:
        var value = ptr[unsafe_offset=line_start]
        if value < 48 or value > 57:
            break
        line_start += 1
    return line_start > marker + 7


def context_output_has_file_location(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    while cursor < end:
        var word = context_output_next_word(ptr, end, cursor)
        if word[0] < 0:
            break
        if context_output_token_has_location(ptr, word[0], word[1]):
            return True
        cursor = word[2]
    return context_output_python_location(ptr, start, end)
        or context_output_paren_location(ptr, start, end)


def context_output_typescript_diagnostic(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_output_has_file_location(ptr, start, end) and (
        context_output_contains["error ts"](ptr, start, end)
        or context_output_contains["warning ts"](ptr, start, end)
        or context_output_contains[" - error ts"](ptr, start, end)
        or context_output_contains[": error ts"](ptr, start, end)
        or context_output_contains[" - warning ts"](ptr, start, end)
        or context_output_contains[": warning ts"](ptr, start, end)
    )


def context_output_eslint_diagnostic(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_output_has_file_location(ptr, start, end) and (
        context_output_contains["  error  "](ptr, start, end)
        or context_output_contains["  warning  "](ptr, start, end)
        or context_output_contains[": error "](ptr, start, end)
        or context_output_contains[": warning "](ptr, start, end)
        or context_output_contains[" eslint "](ptr, start, end)
    )


def context_output_exception(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if context_output_starts_exact["E   "](ptr, start, end):
        return True
    var colon = context_output_find_last_byte(ptr, start, end, 58)
    if colon < 0:
        return False
    var prefix_end = colon
    while prefix_end > start and context_text_whitespace_width(
        ptr, prefix_end - 1, colon
    ) > 0:
        prefix_end -= 1
    var prefix_start = start
    while prefix_start < prefix_end and context_text_whitespace_width(
        ptr, prefix_start, prefix_end
    ) > 0:
        prefix_start += 1
    var has_space = False
    var cursor = prefix_start
    while cursor < prefix_end:
        var whitespace = context_text_whitespace_width(ptr, cursor, prefix_end)
        if whitespace > 0:
            has_space = True
            break
        cursor += context_text_codepoint_width(ptr[unsafe_offset=cursor])
    if has_space and not context_output_ends_exact["Error"](
        ptr, prefix_start, prefix_end
    ):
        return False
    return (
        context_output_starts_exact["Error"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["AssertionError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["ImportError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["ModuleNotFoundError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["NameError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["RuntimeError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["SyntaxError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["TypeError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["ValueError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["ZeroDivisionError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["ReferenceError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["RangeError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["URIError"](ptr, prefix_start, prefix_end)
        or context_output_starts_exact["EvalError"](ptr, prefix_start, prefix_end)
        or context_output_ends_exact["Error"](ptr, prefix_start, prefix_end)
        or context_output_ends_exact["Exception"](ptr, prefix_start, prefix_end)
    )


def context_output_has_zero_count[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    var length = Int64(literal.byte_length())
    while cursor < end:
        var found = context_text_ascii_find[literal](ptr, cursor, end)
        if found < 0:
            return False
        var count = context_output_count_after(ptr, found + length, end)
        if count < 0:
            count = context_output_count_before(ptr, start, found)
        if count == 0:
            return True
        cursor = found + 1
    return False


def context_output_docker_compose_state(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_contains[" started"](ptr, start, end)
        or context_output_contains[" running"](ptr, start, end)
        or context_output_contains[" healthy"](ptr, start, end)
        or context_output_contains[" created"](ptr, start, end)
        or context_output_contains[" done"](ptr, start, end)
        or context_output_contains[" pulled"](ptr, start, end)
    )


def context_output_is_bun_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["bun test v"](ptr, start, end)
        or context_output_starts["(pass) "](ptr, start, end)
        or (
            context_output_starts["ran "](ptr, start, end)
            and context_output_contains[" tests across "](ptr, start, end)
            and context_output_contains[" file"](ptr, start, end)
        )
        or context_output_has_zero_count["fail"](ptr, start, end)
        or context_output_is_count_word["pass"](ptr, start, end)
        or context_output_contains[" expect() call"](ptr, start, end)
        or context_output_ends_exact[".test.ts:"](ptr, start, end)
        or context_output_ends_exact[".test.tsx:"](ptr, start, end)
        or context_output_ends_exact[".test.js:"](ptr, start, end)
        or context_output_ends_exact[".test.jsx:"](ptr, start, end)
    )


def context_output_is_swift_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["build complete!"](ptr, start, end)
        or (
            (
                context_output_starts["test suite "](ptr, start, end)
                or context_output_contains[" test suite "](ptr, start, end)
            )
            and context_output_contains[" passed at "](ptr, start, end)
        )
        or (
            (
                context_output_starts["test case "](ptr, start, end)
                or context_output_contains[" test case "](ptr, start, end)
            )
            and context_output_contains[" passed ("](ptr, start, end)
        )
        or (
            context_output_starts["executed "](ptr, start, end)
            and context_output_contains[" tests"](ptr, start, end)
            and context_output_contains["with 0 failures"](ptr, start, end)
        )
    )


def context_output_is_zig_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_equals["test"](ptr, start, end)
        or context_output_starts["run test"](ptr, start, end)
        or context_output_contains[" run test"](ptr, start, end)
        or (
            context_output_contains[" zig test "](ptr, start, end)
            and context_output_contains[" passed"](ptr, start, end)
        )
        or context_output_contains[" steps succeeded"](ptr, start, end)
        or context_output_contains[" tests passed"](ptr, start, end)
        or (
            context_output_starts["build summary:"](ptr, start, end)
            and context_output_contains["succeeded"](ptr, start, end)
            and not (
                context_output_has_nonzero_count["failed"](ptr, start, end)
                or context_output_has_nonzero_count["failures"](ptr, start, end)
                or context_output_has_nonzero_count["error"](ptr, start, end)
                or context_output_has_nonzero_count["errors"](ptr, start, end)
            )
        )
    )


def context_output_is_gradle_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["> task "](ptr, start, end)
        and context_output_contains[":test"](ptr, start, end)
        and not context_output_ends[" failed"](ptr, start, end)
        or context_output_contains[" > "](ptr, start, end)
        and (
            context_output_ends[" passed"](ptr, start, end)
            or context_output_ends[" skipped"](ptr, start, end)
        )
        or context_output_starts["test run finished after"](ptr, start, end)
        or context_output_starts["["](ptr, start, end)
        and context_output_contains[" tests successful"](ptr, start, end)
        or context_output_starts["["](ptr, start, end)
        and context_output_contains[" tests skipped"](ptr, start, end)
        or context_output_ends[" tests completed"](ptr, start, end)
        or context_output_contains[" tests completed, 0 failed"](ptr, start, end)
        or context_output_starts_exact["BUILD SUCCESSFUL"](ptr, start, end)
    )


def context_output_is_maven_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if (
        context_output_starts["[info] running "](ptr, start, end)
        or context_output_starts["[info] results:"](ptr, start, end)
        or context_output_starts["[info] surefire report directory:"](ptr, start, end)
        or (
            context_output_starts["[info] tests run:"](ptr, start, end)
            and not (
                context_output_has_nonzero_count["failures"](ptr, start, end)
                or context_output_has_nonzero_count["errors"](ptr, start, end)
            )
        )
        or context_output_starts["[info] t e s t s"](ptr, start, end)
    ):
        return True
    var body_start = start
    if context_output_starts["[info]"](ptr, start, end):
        body_start += 6
        while body_start < end and context_text_whitespace_width(
            ptr, body_start, end
        ) > 0:
            body_start += context_text_whitespace_width(ptr, body_start, end)
        var count = end - body_start
        if count >= 8:
            for index in range(body_start, end):
                var value = ptr[unsafe_offset=index]
                if value != 45 and context_text_whitespace_width(
                    ptr, index, end
                ) == 0:
                    return False
            return True
    return False


def context_output_is_package_install(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["yarn install v"](ptr, start, end)
        or context_output_starts["[1/4] resolving packages"](ptr, start, end)
        or context_output_starts["[2/4] fetching packages"](ptr, start, end)
        or context_output_starts["[3/4] linking dependencies"](ptr, start, end)
        or context_output_starts["[4/4] building fresh packages"](ptr, start, end)
        or context_output_starts["success saved lockfile"](ptr, start, end)
        or context_output_starts["success already up-to-date"](ptr, start, end)
        or context_output_starts["saved lockfile"](ptr, start, end)
        or context_output_starts["bun install v"](ptr, start, end)
        or context_output_starts["resolved, downloaded and extracted"](ptr, start, end)
        or context_output_contains[" packages installed"](ptr, start, end)
        or (
            context_output_starts["scope: all "](ptr, start, end)
            and context_output_contains["workspace project"](ptr, start, end)
        )
        or (
            context_output_starts["done in "](ptr, start, end)
            and context_output_contains[" using pnpm "](ptr, start, end)
        )
    )


def context_output_is_docker_buildx(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_output_starts_raw["#"](ptr, start, end) and (
        context_output_contains[" building with "](ptr, start, end)
        or context_output_contains[" transferring "](ptr, start, end)
        or context_output_contains[" exporting "](ptr, start, end)
        or context_output_contains[" importing cache manifest"](ptr, start, end)
        or context_output_contains[" resolving provenance"](ptr, start, end)
        or context_output_contains[" writing image sha256:"](ptr, start, end)
        or context_output_contains[" naming to "](ptr, start, end)
        or context_output_contains[" pushing layers"](ptr, start, end)
        or context_output_contains[" pushing manifest"](ptr, start, end)
        or context_output_ends[" cached"](ptr, start, end)
    )


def context_output_is_bazel_test(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts_raw["//"](ptr, start, end)
        and (
            context_output_contains[" passed in "](ptr, start, end)
            or context_output_ends[" passed"](ptr, start, end)
        )
        or context_output_starts["executed "](ptr, start, end)
        and context_output_contains[" out of "](ptr, start, end)
        and context_output_contains[" tests"](ptr, start, end)
        and (
            context_output_contains[" tests pass"](ptr, start, end)
            or context_output_contains[" test passes"](ptr, start, end)
        )
        or context_output_starts["info: found "](ptr, start, end)
        and context_output_contains[" test target"](ptr, start, end)
        or context_output_starts["info: "](ptr, start, end)
        and context_output_contains[" processes:"](ptr, start, end)
    )


def context_output_typescript_project(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var first = start
    var last = end
    while first < last and (
        ptr[unsafe_offset=first] == 39
        or ptr[unsafe_offset=first] == 34
        or ptr[unsafe_offset=first] == 96
        or ptr[unsafe_offset=last - 1] == 44
        or ptr[unsafe_offset=last - 1] == 59
    ):
        if ptr[unsafe_offset=first] == 39
            or ptr[unsafe_offset=first] == 34
            or ptr[unsafe_offset=first] == 96:
            first += 1
        else:
            last -= 1
    return context_output_ends_exact["tsconfig.json"](ptr, first, last) or context_output_ends_exact[
        "tsconfig.tsbuildinfo"
    ](ptr, first, last)


def context_output_is_typescript_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["project '"](ptr, start, end)
        and context_output_contains[" is up to date"](ptr, start, end)
        or context_output_starts["building project '"](ptr, start, end)
        or context_output_starts["updating unchanged output timestamps"](ptr, start, end)
        or context_output_starts["projects in this build:"](ptr, start, end)
        or context_output_typescript_project(ptr, start, end)
    )


def context_output_is_vite_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["vite "](ptr, start, end)
        and context_output_contains[" building for production"](ptr, start, end)
        or context_output_equals["transforming..."](ptr, start, end)
        or context_output_equals["rendering chunks..."](ptr, start, end)
        or context_output_equals["computing gzip size..."](ptr, start, end)
        or context_output_contains[" modules transformed"](ptr, start, end)
        or context_output_starts["dist/"](ptr, start, end)
        and (
            context_output_contains[" kb"](ptr, start, end)
            or context_output_contains["gzip:"](ptr, start, end)
        )
        or context_output_starts_raw["✓"](ptr, start, end)
        and context_output_contains["built in "](ptr, start, end)
    )


def context_output_next_route_table(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if start >= end:
        return False
    var first = ptr[unsafe_offset=start]
    var is_table = (
        first == 0xE2
        and start + 2 < end
        and (
            ptr[unsafe_offset=start + 1] == 0x94
            or ptr[unsafe_offset=start + 1] == 0x94
        )
        or first == 0xE2
        and start + 2 < end
        and ptr[unsafe_offset=start + 1] == 0x94
        or first == 0xE2
        and start + 2 < end
        and ptr[unsafe_offset=start + 1] == 0x94
        or first == 0xE2
        and start + 2 < end
        and ptr[unsafe_offset=start + 1] == 0x94
        or first == 0xE2
        and start + 2 < end
        and ptr[unsafe_offset=start + 1] == 0x94
        or first == 0xE2
        and start + 2 < end
        and ptr[unsafe_offset=start + 1] == 0x94
        or first == 43
    )
    if not is_table:
        return False
    return (
        context_output_contains[" kb"](ptr, start, end)
        or context_output_contains["first load js"](ptr, start, end)
        or context_output_contains["static"](ptr, start, end)
    )


def context_output_is_next_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["▲ next.js"](ptr, start, end)
        or context_output_starts["next.js "](ptr, start, end)
        or context_output_starts["creating an optimized production build"](ptr, start, end)
        or context_output_starts["compiled successfully"](ptr, start, end)
        or context_output_contains["compiled successfully"](ptr, start, end)
        or context_output_starts["linting and checking validity of types"](ptr, start, end)
        or context_output_starts["collecting page data"](ptr, start, end)
        or context_output_starts["generating static pages"](ptr, start, end)
        or context_output_starts["finalizing page optimization"](ptr, start, end)
        or context_output_starts["collecting build traces"](ptr, start, end)
        or context_output_starts["route "](ptr, start, end)
        and context_output_contains["first load js"](ptr, start, end)
        or context_output_starts["+ first load js"](ptr, start, end)
        or context_output_contains["first load js shared by all"](ptr, start, end)
        or context_output_next_route_table(ptr, start, end)
    )


def context_output_numeric_column(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var last = end
    if last > start and ptr[unsafe_offset=last - 1] == 37:
        last -= 1
    if start >= last:
        return False
    var saw_digit = False
    for index in range(start, last):
        var value = ptr[unsafe_offset=index]
        if value >= 48 and value <= 57:
            saw_digit = True
        elif value != 46:
            return False
    return saw_digit


def context_output_coverage_row(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if not context_output_contains["%"](ptr, start, end):
        return False
    var first = context_output_next_word(ptr, end, start)
    if first[0] < 0 or not context_output_looks_like_location_path(
        ptr, first[0], first[1]
    ):
        return False
    var cursor = first[2]
    var numeric = 0
    while cursor < end:
        var word = context_output_next_word(ptr, end, cursor)
        if word[0] < 0:
            break
        if context_output_numeric_column(ptr, word[0], word[1]):
            numeric += 1
        cursor = word[2]
    return numeric >= 3


def context_output_is_coverage(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["coverage summary"](ptr, start, end)
        or context_output_starts["all files"](ptr, start, end)
        or context_output_starts["statements"](ptr, start, end)
        or context_output_starts["branches"](ptr, start, end)
        or context_output_starts["functions"](ptr, start, end)
        or context_output_starts["lines"](ptr, start, end)
        or context_output_contains[" coverage: platform "](ptr, start, end)
        or context_output_starts["name "](ptr, start, end)
        and context_output_contains[" stmts "](ptr, start, end)
        and context_output_contains[" cover"](ptr, start, end)
        or context_output_starts["total "](ptr, start, end)
        and context_output_contains["%"](ptr, start, end)
        or context_output_starts["required test coverage"](ptr, start, end)
        and context_output_contains[" reached"](ptr, start, end)
        or context_output_starts["coverage html written"](ptr, start, end)
        or context_output_starts["coverage xml written"](ptr, start, end)
        or context_output_starts["coverage json written"](ptr, start, end)
        or context_output_coverage_row(ptr, start, end)
        or context_output_contains["% stmts"](ptr, start, end)
        or context_output_contains["% branch"](ptr, start, end)
        or context_output_contains["% funcs"](ptr, start, end)
        or context_output_contains["% lines"](ptr, start, end)
        or context_output_contains["uncovered line"](ptr, start, end)
    )


def context_output_is_junit_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        (
            context_output_starts_raw["<testsuite"](ptr, start, end)
            or context_output_starts_raw["<testsuites"](ptr, start, end)
        )
        and context_output_contains["tests="](ptr, start, end)
        and (
            context_output_contains_exact["failures=\"0\""](ptr, start, end)
            or context_output_contains_exact["failures='0'"](ptr, start, end)
            or context_output_contains_exact["failures=0"](ptr, start, end)
        )
        and (
            context_output_contains_exact["errors=\"0\""](ptr, start, end)
            or context_output_contains_exact["errors='0'"](ptr, start, end)
            or context_output_contains_exact["errors=0"](ptr, start, end)
        )
    )


def context_output_xml_nonzero[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    var length = Int64(literal.byte_length())
    while cursor < end:
        var marker = context_text_ascii_find[literal](ptr, cursor, end)
        if marker < 0:
            return False
        var value_start = marker + length
        if value_start < end and (
            ptr[unsafe_offset=value_start] == 34
            or ptr[unsafe_offset=value_start] == 39
        ):
            value_start += 1
        var value_end = value_start
        while value_end < end:
            var value = ptr[unsafe_offset=value_end]
            if value < 48 or value > 57:
                break
            value_end += 1
        if context_output_parse_digits(ptr, value_start, value_end) > 0:
            return True
        cursor = marker + 1
    return False


def context_output_is_junit_failure(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts_raw["<failure"](ptr, start, end)
        or context_output_starts_raw["<error"](ptr, start, end)
        or (
            (
                context_output_starts_raw["<testsuite"](ptr, start, end)
                or context_output_starts_raw["<testsuites"](ptr, start, end)
            )
            and (
                context_output_xml_nonzero["failures="](ptr, start, end)
                or context_output_xml_nonzero["errors="](ptr, start, end)
            )
        )
    )


def context_output_is_playwright_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["running "](ptr, start, end)
        and context_output_contains[" tests using "](ptr, start, end)
        or context_output_contains[" passed ("](ptr, start, end)
        and context_output_has_ascii_digit(ptr, start, end)
        or context_output_starts["slow test file:"](ptr, start, end)
        or context_output_starts_raw["✓"](ptr, start, end)
    )


def context_output_is_cypress_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_contains["all specs passed"](ptr, start, end)
        or context_output_starts["spec"](ptr, start, end)
        or context_output_starts["tests"](ptr, start, end)
        or context_output_starts["passing"](ptr, start, end)
        or context_output_starts_raw["✔"](ptr, start, end)
    )


def context_output_is_biome_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        (
            context_output_starts["checked "](ptr, start, end)
            or context_output_starts["formatted "](ptr, start, end)
            or context_output_starts["linted "](ptr, start, end)
        )
        and context_output_contains[" file"](ptr, start, end)
        and context_output_contains[" in "](ptr, start, end)
        and (
            context_output_contains["no fixes applied"](ptr, start, end)
            or context_output_contains["fixed "](ptr, start, end)
            or context_output_contains["no issues found"](ptr, start, end)
        )
        or context_output_equals["no fixes applied."](ptr, start, end)
        or context_output_starts["fixed "](ptr, start, end)
        and context_output_contains[" file"](ptr, start, end)
    )


def context_output_is_oxlint_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["finished in "](ptr, start, end)
        and context_output_contains[" on "](ptr, start, end)
        and context_output_contains[" file"](ptr, start, end)
        and (
            context_output_contains["0 warning"](ptr, start, end)
            or context_output_contains["0 error"](ptr, start, end)
            or not context_output_contains["warning"](ptr, start, end)
            and not context_output_contains["error"](ptr, start, end)
        )
    )


def context_output_is_pytest_progress(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var first = context_output_next_word(ptr, end, start)
    if first[0] < 0:
        return False
    var length = first[1] - first[0]
    if length < 2:
        return False
    var dots = False
    for index in range(first[0], first[1]):
        var value = ptr[unsafe_offset=index]
        if value == 46:
            dots = True
        elif value != 115 and value != 83 and value != 120 and value != 88:
            return False
    if not dots:
        return False
    var rest = context_output_next_word(ptr, end, first[2])
    if rest[0] < 0:
        return True
    return ptr[unsafe_offset=rest[0]] == 91 and ptr[unsafe_offset=rest[1] - 1] == 93 and context_output_contains[
        "%"
    ](ptr, rest[0], rest[1])


def context_output_is_pytest_success_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_contains[" passed"](ptr, start, end)
        and context_output_contains[" in "](ptr, start, end)
        and not context_output_contains[" failed"](ptr, start, end)
        and not context_output_contains[" error"](ptr, start, end)
        and not context_output_contains["errors"](ptr, start, end)
    )


def context_output_word_count(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var count: Int64 = 0
    var cursor = start
    while cursor < end:
        var word = context_output_next_word(ptr, end, cursor)
        if word[0] < 0:
            break
        count += 1
        cursor = word[2]
    return count


def context_output_rust_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_starts_raw["Compiling "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_COMPILING
    if context_output_starts_raw["Checking "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_CHECKING
    if context_output_starts_raw["Fresh "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_FRESH
    if context_output_starts_raw["Documenting "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_DOCUMENTING
    if context_output_starts_raw["Formatting "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_FORMATTING
    if (
        context_output_starts_raw["Fixing "](ptr, start, end)
        or context_output_starts_raw["Fixed "](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_CARGO_FIX
    if context_output_starts_raw["Generated "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GENERATED_DOCS
    if context_output_starts_raw["Finished "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_FINISHED
    if context_output_starts_raw["Running "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_RUNNING_TARGETS
    if context_output_starts_raw["Doc-tests "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_DOC_TESTS
    if context_output_starts_raw["running "](ptr, start, end) and context_output_ends_exact[
        " tests"
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_RUNNING_TESTS
    if context_output_starts_raw["test "](ptr, start, end)
        and context_output_contains_exact[" ... ok"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PASSED_TESTS
    if context_output_starts_raw["PASS ["](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NEXTEST_PASS
    if context_output_starts_raw["Summary ["](ptr, start, end)
        and context_output_contains_exact[" passed"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NEXTEST_SUMMARY
    if context_output_starts_raw["test result: ok"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_RESULT_OK
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_is_common_success(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["build successful"](ptr, start, end)
        or context_output_contains[" build success"](ptr, start, end)
        or context_output_starts["[info] build success"](ptr, start, end)
        or context_output_contains["actionable tasks:"](ptr, start, end)
        or context_output_contains["actionable task:"](ptr, start, end)
        or context_output_starts["[info] tests run:"](ptr, start, end)
        or context_output_starts["successfully built "](ptr, start, end)
        or context_output_starts["successfully tagged "](ptr, start, end)
        or context_output_starts["info: build completed successfully"](ptr, start, end)
        or context_output_contains["successfully ran target"](ptr, start, end)
        or context_output_contains["successfully ran targets"](ptr, start, end)
        or context_output_starts["tasks:"](ptr, start, end)
        and context_output_contains["successful"](ptr, start, end)
        or context_output_starts["summary ["](ptr, start, end)
        and context_output_contains[" passed"](ptr, start, end)
        or context_output_starts["success: no issues found"](ptr, start, end)
        or context_output_starts["found 0 errors"](ptr, start, end)
        or context_output_starts["all checks passed"](ptr, start, end)
        or context_output_contains["all matched files use prettier code style"](
            ptr, start, end
        )
        or context_output_contains["eslint found no problems"](ptr, start, end)
        or context_output_starts["test files"](ptr, start, end)
        and context_output_contains["passed"](ptr, start, end)
        or context_output_contains[" passed ("](ptr, start, end)
        or context_output_contains["all specs passed"](ptr, start, end)
    )


def context_output_noisy_framework_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_is_coverage(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_COVERAGE
    if context_output_is_gradle_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GRADLE_TEST
    if context_output_is_maven_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_MAVEN_TEST
    if context_output_is_package_install(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGE_INSTALL
    if context_output_is_docker_buildx(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_DOCKER_BUILDX
    if context_output_is_bazel_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_BAZEL_TEST
    if context_output_is_junit_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_JUNIT_XML
    if context_output_is_swift_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_SWIFT_TEST
    if context_output_is_playwright_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PLAYWRIGHT
    if context_output_is_biome_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_BIOME
    if context_output_is_oxlint_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_OXLINT
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_language_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_is_typescript_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TYPECHECK
    if context_output_is_vite_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_VITE
    if context_output_is_next_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NEXT
    if context_output_all_dots(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_DOT_PROGRESS
    if context_output_is_bun_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_BUN_TEST
    if context_output_is_cypress_success(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_CYPRESS
    if context_output_is_zig_test(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_ZIG_TEST
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_go_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_starts_raw["PASS "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PASSED_SUITES
    if context_output_starts["ok "](ptr, start, end) and context_output_word_count(
        ptr, start, end
    ) >= 2:
        return CONTEXT_OUTPUT_LABEL_GO_TEST_OK
    if context_output_starts["? "](ptr, start, end) and context_output_contains[
        "[no test files]"
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_NO_FILES
    if context_output_starts["=== run "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_RUN
    if context_output_starts["=== pause "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_PAUSE
    if context_output_starts["=== cont "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_CONT
    if context_output_starts["--- pass: "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_PASS
    if context_output_starts["--- skip: "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_SKIP
    if context_output_starts_exact["PASS"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GO_TEST_SUMMARY
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_test_summary_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_starts["test suites:"](ptr, start, end)
        and context_output_contains["passed"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_SUITES
    if context_output_starts["tests:"](ptr, start, end)
        and context_output_contains["passed"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_CASES
    if context_output_starts["summary ["](ptr, start, end)
        and context_output_contains[" passed"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NEXTEST_SUMMARY
    if context_output_starts_raw["PASS ["](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NEXTEST_PASS
    if context_output_starts["snapshots:"](ptr, start, end) and (
        context_output_contains["passed"](ptr, start, end)
        or context_output_contains["0 total"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_SNAPSHOTS
    if context_output_starts["test files"](ptr, start, end)
        and context_output_contains["passed"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_FILES
    if context_output_starts["duration"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_DURATION
    if context_output_starts["time:"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_TIME
    if context_output_starts["ran all test suites"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_RUNNER
    if context_output_starts["done in "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_DONE
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_build_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if (
        context_output_starts["build successful"](ptr, start, end)
        or context_output_starts["build success"](ptr, start, end)
        or context_output_starts["[info] build success"](ptr, start, end)
        or context_output_contains[" build success"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_BUILD_SUCCESS
    if context_output_starts["[info] --- "](ptr, start, end) or context_output_starts[
        "> task "
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_BUILD_STEPS
    if (
        context_output_starts["info: analyzed target"](ptr, start, end)
        or context_output_starts["info: found "](ptr, start, end)
        or context_output_starts["info: elapsed time:"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_BAZEL_STEPS
    if (
        context_output_starts["target "](ptr, start, end)
        and context_output_contains["up-to-date"](ptr, start, end)
        or context_output_starts["info: build completed successfully"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_BAZEL_SUMMARY
    if (
        context_output_contains["successfully ran target"](ptr, start, end)
        or context_output_contains["successfully ran targets"](ptr, start, end)
        or context_output_starts["nx successfully ran"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_NX_SUMMARY
    if context_output_starts["tasks:"](ptr, start, end)
        and context_output_contains["successful"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TURBO_SUMMARY
    if context_output_contains["actionable tasks:"](ptr, start, end)
        or context_output_contains["actionable task:"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_GRADLE_TASKS
    if (
        context_output_starts["[info] total time:"](ptr, start, end)
        or context_output_starts["[info] finished at:"](ptr, start, end)
        or context_output_starts["[info] tests run:"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_MAVEN_SUMMARY
    if (
        context_output_starts["=> "](ptr, start, end)
        or context_output_starts["=>=> "](ptr, start, end)
        or context_output_starts_raw["#"](ptr, start, end)
        and (
            context_output_contains[" done "](ptr, start, end)
            or context_output_ends[" done"](ptr, start, end)
        )
    ):
        return CONTEXT_OUTPUT_LABEL_DOCKER_STEPS
    if (
        context_output_starts["[+] running "](ptr, start, end)
        or context_output_starts["container "](ptr, start, end)
        and context_output_docker_compose_state(ptr, start, end)
        or context_output_starts["network "](ptr, start, end)
        and context_output_contains["created"](ptr, start, end)
        or context_output_starts["volume "](ptr, start, end)
        and context_output_contains["created"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_DOCKER_COMPOSE
    if (
        context_output_starts["successfully built "](ptr, start, end)
        or context_output_starts["successfully tagged "](ptr, start, end)
        or context_output_contains["writing image sha256:"](ptr, start, end)
        or context_output_contains["naming to "](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_DOCKER_SUMMARY
    if context_output_starts["running "](ptr, start, end)
        and context_output_contains[" tests using "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PLAYWRIGHT_RUNNING
    if context_output_contains[" passed ("](ptr, start, end)
        and context_output_has_ascii_digit(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_TEST_SUMMARY
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_package_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_starts["added "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGES_ADDED
    if context_output_starts["audited "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGES_AUDITED
    if context_output_starts["updating crates.io index"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_CARGO_INDEX
    if context_output_starts["locking "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_CARGO_LOCK
    if context_output_starts["downloading crates"](ptr, start, end) or context_output_starts[
        "downloaded "
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_CARGO_DOWNLOAD
    if context_output_starts["packages: "](ptr, start, end) or context_output_starts[
        "progress: resolved"
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGE_PROGRESS
    if context_output_starts["lockfile is up to date"](ptr, start, end) or context_output_starts[
        "already up to date"
    ](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGES_UP_TO_DATE
    if (
        context_output_starts["requirement already satisfied"](ptr, start, end)
        or context_output_starts["successfully installed"](ptr, start, end)
        or context_output_starts["installing collected packages"](ptr, start, end)
        or context_output_starts["resolved "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
        or context_output_starts["prepared "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
        or context_output_starts["installed "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_PYTHON_PACKAGES
    if context_output_equals["up to date"](ptr, start, end)
        or context_output_starts["up to date in "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PACKAGES_UP_TO_DATE
    if context_output_starts["found 0 vulnerabilities"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_VULNERABILITY
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_noisy_quality_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if (
        context_output_starts["all files pass"](ptr, start, end)
        or context_output_contains["all matched files use prettier code style"](
            ptr, start, end
        )
        or context_output_contains["eslint found no problems"](ptr, start, end)
        or context_output_starts["all checks passed"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_FORMATTER
    if (
        context_output_starts["success: no issues found"](ptr, start, end)
        or context_output_starts["found 0 errors"](ptr, start, end)
        or context_output_starts["found 0 warnings"](ptr, start, end)
        or context_output_starts["found 0 issues"](ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_TYPECHECK
    if context_output_starts["built in "](ptr, start, end)
        or context_output_contains[" built in "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_BUILD_SUMMARY
    if context_output_starts["compiled successfully"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_COMPILE_SUMMARY
    if (
        context_output_starts["tests/"](ptr, start, end)
        and context_output_contains[" passed"](ptr, start, end)
        or context_output_contains["::test_"](ptr, start, end)
        and context_output_ends[" passed"](ptr, start, end)
        or context_output_is_pytest_success_summary(ptr, start, end)
    ):
        return CONTEXT_OUTPUT_LABEL_PASSED_TESTS
    if context_output_is_pytest_progress(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PYTEST_PROGRESS
    return CONTEXT_OUTPUT_LABEL_NONE


def context_output_only_digits_or_fail(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    while cursor < end:
        var word = context_output_next_word(ptr, end, cursor)
        if word[0] < 0:
            break
        if not context_output_equals["fail"](ptr, word[0], word[1])
            and context_output_parse_digits(ptr, word[0], word[1]) < 0:
            return False
        cursor = word[2]
    return True


def context_output_is_nonzero_fail_count(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var cursor = start
    var length = Int64(4)
    while cursor < end:
        var found = context_text_ascii_find["fail"](ptr, cursor, end)
        if found < 0:
            return False
        var count = context_output_count_after(ptr, found + length, end)
        if count < 0:
            count = context_output_count_before(ptr, start, found)
        if count > 0 and context_output_only_digits_or_fail(ptr, start, end):
            return True
        cursor = found + 1
    return False


def context_output_is_failure(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["build failure"](ptr, start, end)
        or context_output_starts["build failed"](ptr, start, end)
        or context_output_contains["build did not complete successfully"](ptr, start, end)
        or context_output_contains["build did not complete"](ptr, start, end)
        or context_output_starts["info: build failed"](ptr, start, end)
        or context_output_starts["failed:"](ptr, start, end)
        or context_output_starts["fail:"](ptr, start, end)
        or context_output_starts["--- fail:"](ptr, start, end)
        or context_output_starts["(fail) "](ptr, start, end)
        or context_output_starts["failed tests:"](ptr, start, end)
        or context_output_starts["there were failing"](ptr, start, end)
        or context_output_starts["there were test failures"](ptr, start, end)
        or context_output_contains[" test failures"](ptr, start, end)
        or context_output_contains["tests failed"](ptr, start, end)
        or context_output_starts["type error"](ptr, start, end)
        or context_output_contains["failed to compile"](ptr, start, end)
        or context_output_contains["failed to load"](ptr, start, end)
        or context_output_contains["failed with"](ptr, start, end)
        or context_output_contains["execution failed"](ptr, start, end)
        or context_output_contains["failed to solve"](ptr, start, end)
        or context_output_contains["executor failed running"](ptr, start, end)
        or context_output_starts["> task "](ptr, start, end)
        and context_output_ends[" failed"](ptr, start, end)
        or context_output_starts_raw["//"](ptr, start, end)
        and context_output_contains[" failed"](ptr, start, end)
        or context_output_starts_raw["#"](ptr, start, end)
        and context_output_contains[" error"](ptr, start, end)
        or context_output_starts["err_pnpm_"](ptr, start, end)
        or context_output_starts["required test coverage"](ptr, start, end)
        and context_output_contains["not reached"](ptr, start, end)
        or context_output_is_nonzero_fail_count(ptr, start, end)
        or context_output_is_junit_failure(ptr, start, end)
        or context_output_has_nonzero_count["failed"](ptr, start, end)
        or context_output_has_nonzero_count["failures"](ptr, start, end)
        or context_output_has_nonzero_count["failing"](ptr, start, end)
        or context_output_has_nonzero_count["fails"](ptr, start, end)
        or context_output_has_nonzero_count["error"](ptr, start, end)
        or context_output_has_nonzero_count["errors"](ptr, start, end)
    )


def context_output_is_warning(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if start >= end or context_output_has_zero_only_count["warning"](ptr, start, end)
        or context_output_has_zero_only_count["warnings"](ptr, start, end):
        return False
    return (
        context_output_starts["warning"](ptr, start, end)
        or context_output_starts["warn "](ptr, start, end)
        or context_output_starts["warn:"](ptr, start, end)
        or context_output_starts["[warning]"](ptr, start, end)
        or context_output_starts["[warn]"](ptr, start, end)
        or context_output_starts["npm warn"](ptr, start, end)
        or context_output_starts["pnpm warn"](ptr, start, end)
        or context_output_starts["yarn warning"](ptr, start, end)
        or context_output_starts["bun warning"](ptr, start, end)
        or context_output_has_nonzero_count["warning"](ptr, start, end)
        or context_output_has_nonzero_count["warnings"](ptr, start, end)
        or context_output_contains[" warning "](ptr, start, end)
        or context_output_contains[" warnings"](ptr, start, end)
        or context_output_contains["with warnings"](ptr, start, end)
        or context_output_contains["compiled with warning"](ptr, start, end)
        or context_output_contains["compiled with warnings"](ptr, start, end)
        or context_output_contains["warning ts"](ptr, start, end)
        or context_output_contains[": warning ts"](ptr, start, end)
        or context_output_contains[" - warning ts"](ptr, start, end)
    )


def context_output_is_diagnostic_success_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["test suites:"](ptr, start, end)
        or context_output_starts["tests:"](ptr, start, end)
        or context_output_starts["snapshots:"](ptr, start, end)
        or context_output_starts["test files"](ptr, start, end)
        or context_output_starts["ran all test suites"](ptr, start, end)
        or context_output_starts["found 0 vulnerabilities"](ptr, start, end)
        or context_output_is_junit_success(ptr, start, end)
        or context_output_contains[" passed in "](ptr, start, end)
        or context_output_is_common_success(ptr, start, end)
    )


def context_output_is_diagnostic_failure_summary(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["test suites:"](ptr, start, end)
        and context_output_has_nonzero_count["failed"](ptr, start, end)
        or context_output_starts["tests:"](ptr, start, end)
        and context_output_has_nonzero_count["failed"](ptr, start, end)
        or context_output_contains[" failed, "](ptr, start, end)
        and context_output_has_nonzero_count["failed"](ptr, start, end)
        or context_output_starts["failed "](ptr, start, end)
        and not (
            context_output_has_zero_only_count["failed"](ptr, start, end)
            or context_output_has_zero_only_count["failures"](ptr, start, end)
        )
        or context_output_starts["error summary"](ptr, start, end)
        and not (
            context_output_has_zero_only_count["error"](ptr, start, end)
            or context_output_has_zero_only_count["errors"](ptr, start, end)
        )
        or context_output_is_junit_failure(ptr, start, end)
    )


def context_output_is_noisy_key(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts["test suites:"](ptr, start, end)
        or context_output_starts["tests:"](ptr, start, end)
        or context_output_starts["snapshots:"](ptr, start, end)
        or context_output_starts["test files"](ptr, start, end)
        or context_output_starts["summary ["](ptr, start, end)
        and context_output_contains[" passed"](ptr, start, end)
        or context_output_starts["ran all test suites"](ptr, start, end)
        or context_output_starts["done in "](ptr, start, end)
        or context_output_starts["added "](ptr, start, end)
        or context_output_starts["audited "](ptr, start, end)
        or context_output_starts["packages: "](ptr, start, end)
        or context_output_starts["build successful"](ptr, start, end)
        or context_output_contains[" build success"](ptr, start, end)
        or context_output_starts["[info] build success"](ptr, start, end)
        or context_output_starts["info: build completed successfully"](ptr, start, end)
        or context_output_contains["successfully ran target"](ptr, start, end)
        or context_output_contains["successfully ran targets"](ptr, start, end)
        or context_output_starts["tasks:"](ptr, start, end)
        and context_output_contains["successful"](ptr, start, end)
        or context_output_contains["actionable tasks:"](ptr, start, end)
        or context_output_contains["actionable task:"](ptr, start, end)
        or context_output_starts["[info] tests run:"](ptr, start, end)
        or context_output_starts["successfully built "](ptr, start, end)
        or context_output_starts["successfully tagged "](ptr, start, end)
        or context_output_starts["[+] running "](ptr, start, end)
        or context_output_starts["container "](ptr, start, end)
        and context_output_docker_compose_state(ptr, start, end)
        or context_output_contains["writing image sha256:"](ptr, start, end)
        or context_output_contains["naming to "](ptr, start, end)
        or context_output_is_docker_buildx(ptr, start, end)
        or context_output_contains["all matched files use prettier code style"](
            ptr, start, end
        )
        or context_output_contains["eslint found no problems"](ptr, start, end)
        or context_output_starts["found 0 vulnerabilities"](ptr, start, end)
        or context_output_equals["up to date"](ptr, start, end)
        or context_output_starts["up to date in "](ptr, start, end)
        or context_output_starts["lockfile is up to date"](ptr, start, end)
        or context_output_starts["already up to date"](ptr, start, end)
        or context_output_is_package_install(ptr, start, end)
        or context_output_starts["successfully installed"](ptr, start, end)
        or context_output_starts["resolved "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
        or context_output_starts["prepared "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
        or context_output_starts["installed "](ptr, start, end)
        and context_output_contains[" package"](ptr, start, end)
        or context_output_starts["all files pass"](ptr, start, end)
        or context_output_starts["all checks passed"](ptr, start, end)
        or context_output_starts["success: no issues found"](ptr, start, end)
        or context_output_starts["found 0 errors"](ptr, start, end)
        or context_output_starts["found 0 issues"](ptr, start, end)
        or context_output_starts["built in "](ptr, start, end)
        or context_output_contains[" passed in "](ptr, start, end)
        or context_output_is_bazel_test(ptr, start, end)
        or context_output_is_common_success(ptr, start, end)
        or context_output_starts["compiled successfully"](ptr, start, end)
        or context_output_is_coverage(ptr, start, end)
        or context_output_is_junit_success(ptr, start, end)
        or context_output_is_playwright_success(ptr, start, end)
        or context_output_is_cypress_success(ptr, start, end)
    )


def context_output_noisy_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_is_failure(ptr, start, end) and not context_output_is_diagnostic_failure_summary(
        ptr, start, end
    ) or context_output_is_warning(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NONE
    var label = context_output_noisy_framework_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_rust_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_noisy_language_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_noisy_go_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_noisy_test_summary_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_noisy_build_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    label = context_output_noisy_package_label(ptr, start, end)
    if label != CONTEXT_OUTPUT_LABEL_NONE:
        return label
    return context_output_noisy_quality_label(ptr, start, end)


def context_output_diagnostic_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_output_starts_raw["> "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_NPM_SCRIPT
    if context_output_starts["collecting "](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PYTEST_COLLECTING
    if context_output_starts["collected "](ptr, start, end)
        and context_output_contains[" item"](ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PYTEST_COLLECTED
    if context_output_is_pytest_progress(ptr, start, end):
        return CONTEXT_OUTPUT_LABEL_PYTEST_PROGRESS
    return context_output_noisy_label(ptr, start, end)


def context_output_rust_strong(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts_raw["error["](ptr, start, end)
        or context_output_starts_raw["error:"](ptr, start, end)
        or context_output_starts_raw["warning["](ptr, start, end)
        or context_output_starts_raw["warning:"](ptr, start, end)
        or (
            context_output_contains[": error"](ptr, start, end)
            and context_output_has_file_location(ptr, start, end)
        )
        or (
            context_output_contains[": warning"](ptr, start, end)
            and context_output_has_file_location(ptr, start, end)
        )
        or (
            context_output_starts_raw["test "](ptr, start, end)
            and context_output_contains_exact[" ... FAILED"](ptr, start, end)
        )
        or context_output_starts_raw["---- "](ptr, start, end)
        and context_output_ends_exact[" ----"](ptr, start, end)
        or context_output_contains_exact["panicked at "](ptr, start, end)
        or context_output_contains_exact["panicked at:"](ptr, start, end)
        or context_output_starts_raw["test result: FAILED"](ptr, start, end)
        or context_output_starts_raw["failures:"](ptr, start, end)
        or context_output_starts_raw["error: aborting"](ptr, start, end)
    )


def context_output_rust_location(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_starts_raw["--> "](ptr, start, end)
        or context_output_starts_raw["::: "](ptr, start, end)
        or context_output_starts_raw["at "](ptr, start, end)
        and context_output_has_file_location(ptr, start, end)
        or context_output_has_file_location(ptr, start, end)
    )


def context_output_rust_backtrace(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_output_equals["stack backtrace:"](ptr, start, end) or context_output_equals[
        "Backtrace:"
    ](ptr, start, end)


def context_output_rust_exit(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_output_contains["exit status"](ptr, start, end)
        or context_output_contains["exit code"](ptr, start, end)
        or context_output_contains["exit_status"](ptr, start, end)
        or context_output_contains["process didn't exit successfully"](ptr, start, end)
    )


def context_output_line_semantics(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> ContextCommandOutputLineResult:
    var result = ContextCommandOutputLineResult(0, 0, 0)
    var bounds = context_text_trim_bounds(ptr, 0, length)
    var start = bounds[0]
    var end = bounds[1]
    if start >= end:
        return result^

    var rust_label = context_output_rust_label(ptr, start, end)
    var rust_strong = context_output_rust_strong(ptr, start, end)
    var rust_location = context_output_rust_location(ptr, start, end)
    var rust_backtrace = context_output_rust_backtrace(ptr, start, end)
    var rust_exit = context_output_rust_exit(ptr, start, end)
    if rust_strong:
        result.flags |= CONTEXT_OUTPUT_RUST_STRONG
    if rust_location:
        result.flags |= CONTEXT_OUTPUT_RUST_LOCATION
    if rust_backtrace:
        result.flags |= CONTEXT_OUTPUT_RUST_BACKTRACE
    if rust_exit:
        result.flags |= CONTEXT_OUTPUT_RUST_EXIT
    if rust_label != CONTEXT_OUTPUT_LABEL_NONE:
        result.flags |= CONTEXT_OUTPUT_RUST_NOISE
    if context_output_contains_exact["clippy::"](ptr, start, end) or context_output_contains_exact[
        "cargo clippy"
    ](ptr, start, end):
        result.flags |= CONTEXT_OUTPUT_CLIPPY

    var typescript = context_output_typescript_diagnostic(ptr, start, end)
    var eslint = context_output_eslint_diagnostic(ptr, start, end)
    var exception = context_output_exception(ptr, start, end)
    var junit_failure = context_output_is_junit_failure(ptr, start, end)
    if typescript:
        result.flags |= CONTEXT_OUTPUT_TYPESCRIPT
    if eslint:
        result.flags |= CONTEXT_OUTPUT_ESLINT
    if exception:
        result.flags |= CONTEXT_OUTPUT_EXCEPTION
    if junit_failure:
        result.flags |= CONTEXT_OUTPUT_JUNIT_FAILURE
    if typescript or eslint or exception or junit_failure:
        result.flags |= CONTEXT_OUTPUT_DIAGNOSTIC_TARGET

    var failure = context_output_is_failure(ptr, start, end)
    var warning = context_output_is_warning(ptr, start, end)
    if failure:
        result.flags |= CONTEXT_OUTPUT_FAILURE
    if warning:
        result.flags |= CONTEXT_OUTPUT_WARNING
    if context_output_is_noisy_key(ptr, start, end):
        result.flags |= CONTEXT_OUTPUT_NOISY_KEY
    if context_output_is_diagnostic_success_summary(ptr, start, end):
        result.flags |= CONTEXT_OUTPUT_DIAGNOSTIC_SUCCESS
    if context_output_is_diagnostic_failure_summary(ptr, start, end):
        result.flags |= CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE
    result.noisy_label = context_output_noisy_label(ptr, start, end)
    result.diagnostic_label = context_output_diagnostic_label(ptr, start, end)
    return result^


comptime CONTEXT_METADATA_MAX_BYTES: Int64 = 16_384
comptime CONTEXT_METADATA_NO_KIND: Int64 = -1


def context_metadata_token_byte(value: UInt8) -> Bool:
    return (
        value >= 48
        and value <= 57
        or value >= 65
        and value <= 90
        or value >= 97
        and value <= 122
        or value == 45
        or value == 95
        or value == 46
        or value == 47
        or value == 43
    )


def context_metadata_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def context_metadata_next_token(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 3]:
    var bounds = InlineArray[Int64, 3](fill=-1)
    var index = cursor
    while index < length and not context_metadata_token_byte(
        ptr[unsafe_offset=index]
    ):
        index += 1
    if index >= length:
        return bounds^
    bounds[0] = index
    while index < length and context_metadata_token_byte(
        ptr[unsafe_offset=index]
    ):
        index += 1
    bounds[1] = index
    bounds[2] = index
    return bounds^


def context_metadata_command_bounds(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var bounds = InlineArray[Int64, 2](fill=0)
    var command_start = start
    var index = end
    while index > start:
        index -= 1
        if ptr[unsafe_offset=index] == 47:
            command_start = index + 1
            break
    var command_end = end
    if command_end - command_start >= 4:
        var suffix = command_end - 4
        if (
            context_metadata_lower(ptr[unsafe_offset=suffix]) == 46
            and context_metadata_lower(ptr[unsafe_offset=suffix + 1]) == 101
            and context_metadata_lower(ptr[unsafe_offset=suffix + 2]) == 120
            and context_metadata_lower(ptr[unsafe_offset=suffix + 3]) == 101
        ):
            command_end = suffix
    bounds[0] = command_start
    bounds[1] = command_end
    return bounds^


def context_metadata_token_equals[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var length = bounds[1] - bounds[0]
    if length != Int64(literal.byte_length()):
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if (
            context_metadata_lower(ptr[unsafe_offset=bounds[0] + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_metadata_token_ends_with[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var suffix_length = Int64(literal.byte_length())
    var length = bounds[1] - bounds[0]
    if length < suffix_length:
        return False
    var expected = literal.unsafe_ptr()
    var offset = bounds[1] - suffix_length
    for index in range(suffix_length):
        if (
            context_metadata_lower(ptr[unsafe_offset=offset + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_metadata_token_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var needle_length = Int64(literal.byte_length())
    var length = bounds[1] - bounds[0]
    if needle_length == 0:
        return True
    if length < needle_length:
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(length - needle_length + 1):
        var matched = True
        for index in range(needle_length):
            if (
                context_metadata_lower(ptr[unsafe_offset=bounds[0] + offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def context_metadata_token_is_option_or_shell_glue(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    if bounds[1] <= bounds[0]:
        return True
    var first = context_metadata_lower(ptr[unsafe_offset=bounds[0]])
    if first == 45 or first == 43:
        return True
    return (
        context_metadata_token_equals["cmd"](ptr, start, end)
        or context_metadata_token_equals["command"](ptr, start, end)
        or context_metadata_token_equals["args"](ptr, start, end)
        or context_metadata_token_equals["arguments"](ptr, start, end)
        or context_metadata_token_equals["metadata"](ptr, start, end)
        or context_metadata_token_equals["name"](ptr, start, end)
        or context_metadata_token_equals["tool"](ptr, start, end)
        or context_metadata_token_equals["tool_name"](ptr, start, end)
        or context_metadata_token_equals["shell"](ptr, start, end)
        or context_metadata_token_equals["bash"](ptr, start, end)
        or context_metadata_token_equals["sh"](ptr, start, end)
        or context_metadata_token_equals["zsh"](ptr, start, end)
        or context_metadata_token_equals["fish"](ptr, start, end)
        or context_metadata_token_equals["powershell"](ptr, start, end)
        or context_metadata_token_equals["pwsh"](ptr, start, end)
        or context_metadata_token_equals["python"](ptr, start, end)
        or context_metadata_token_equals["python3"](ptr, start, end)
        or context_metadata_token_equals["py"](ptr, start, end)
        or context_metadata_token_equals["node"](ptr, start, end)
        or context_metadata_token_equals["npx"](ptr, start, end)
        or context_metadata_token_equals["bunx"](ptr, start, end)
        or context_metadata_token_equals["uv"](ptr, start, end)
        or context_metadata_token_equals["uvx"](ptr, start, end)
        or context_metadata_token_equals["poetry"](ptr, start, end)
        or context_metadata_token_equals["pipenv"](ptr, start, end)
        or context_metadata_token_equals["exec_command"](ptr, start, end)
        or context_metadata_token_equals["function_call"](ptr, start, end)
        or context_metadata_token_equals["function_call_output"](ptr, start, end)
        or context_metadata_token_equals["shell_call"](ptr, start, end)
        or context_metadata_token_equals["shell_call_output"](ptr, start, end)
        or context_metadata_token_equals["true"](ptr, start, end)
        or context_metadata_token_equals["false"](ptr, start, end)
        or context_metadata_token_equals["null"](ptr, start, end)
    )


def context_metadata_token_option_takes_value(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_metadata_token_equals["-c"](ptr, start, end)
        or context_metadata_token_equals["-m"](ptr, start, end)
        or context_metadata_token_equals["-p"](ptr, start, end)
        or context_metadata_token_equals["--config"](ptr, start, end)
        or context_metadata_token_equals["--git-dir"](ptr, start, end)
        or context_metadata_token_equals["--work-tree"](ptr, start, end)
        or context_metadata_token_equals["--manifest-path"](ptr, start, end)
        or context_metadata_token_equals["--package"](ptr, start, end)
        or context_metadata_token_equals["--bin"](ptr, start, end)
        or context_metadata_token_equals["--example"](ptr, start, end)
        or context_metadata_token_equals["--target"](ptr, start, end)
        or context_metadata_token_equals["--project"](ptr, start, end)
        or context_metadata_token_equals["--cwd"](ptr, start, end)
        or context_metadata_token_equals["--prefix"](ptr, start, end)
        or context_metadata_token_equals["--directory"](ptr, start, end)
    )


def context_metadata_subcommand_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if not context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
    return result^


def context_metadata_package_script_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var saw_run = False
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            continue
        if context_metadata_token_equals["run"](ptr, token[0], token[1]) or context_metadata_token_equals[
            "run-script"
        ](ptr, token[0], token[1]):
            saw_run = True
            continue
        if (
            context_metadata_token_equals["test"](ptr, token[0], token[1])
            or context_metadata_token_equals["t"](ptr, token[0], token[1])
            or context_metadata_token_equals["typecheck"](ptr, token[0], token[1])
            or context_metadata_token_equals["type-check"](ptr, token[0], token[1])
            or context_metadata_token_equals["tsc"](ptr, token[0], token[1])
            or context_metadata_token_equals["check"](ptr, token[0], token[1])
            or saw_run
            and (
                context_metadata_token_contains["test"](ptr, token[0], token[1])
                or context_metadata_token_contains["typecheck"](ptr, token[0], token[1])
            )
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
        return result^
    return result^


def context_metadata_package_install_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            continue
        if (
            context_metadata_token_equals["install"](ptr, token[0], token[1])
            or context_metadata_token_equals["i"](ptr, token[0], token[1])
            or context_metadata_token_equals["ci"](ptr, token[0], token[1])
            or context_metadata_token_equals["add"](ptr, token[0], token[1])
            or context_metadata_token_equals["update"](ptr, token[0], token[1])
            or context_metadata_token_equals["upgrade"](ptr, token[0], token[1])
            or context_metadata_token_equals["sync"](ptr, token[0], token[1])
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
        return result^
    return result^


def context_metadata_direct_kind(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if (
        context_metadata_token_equals["rg"](ptr, start, end)
        or context_metadata_token_equals["ripgrep"](ptr, start, end)
        or context_metadata_token_equals["grep"](ptr, start, end)
        or context_metadata_token_equals["egrep"](ptr, start, end)
        or context_metadata_token_equals["fgrep"](ptr, start, end)
    ):
        return 6
    if (
        context_metadata_token_equals["ls"](ptr, start, end)
        or context_metadata_token_equals["find"](ptr, start, end)
        or context_metadata_token_equals["tree"](ptr, start, end)
    ):
        return 7
    if (
        context_metadata_token_equals["pytest"](ptr, start, end)
        or context_metadata_token_equals["py.test"](ptr, start, end)
        or context_metadata_token_equals["tsc"](ptr, start, end)
        or context_metadata_token_equals["ruff"](ptr, start, end)
        or context_metadata_token_equals["mypy"](ptr, start, end)
        or context_metadata_token_equals["biome"](ptr, start, end)
        or context_metadata_token_equals["oxlint"](ptr, start, end)
        or context_metadata_token_equals["eslint"](ptr, start, end)
        or context_metadata_token_equals["playwright"](ptr, start, end)
        or context_metadata_token_equals["cypress"](ptr, start, end)
        or context_metadata_token_ends_with["-tsc"](ptr, start, end)
        or context_metadata_token_ends_with["_tsc"](ptr, start, end)
    ):
        return 4
    if (
        context_metadata_token_equals["bazel"](ptr, start, end)
        or context_metadata_token_equals["bazelisk"](ptr, start, end)
        or context_metadata_token_equals["nx"](ptr, start, end)
        or context_metadata_token_equals["turbo"](ptr, start, end)
        or context_metadata_token_equals["pip"](ptr, start, end)
        or context_metadata_token_equals["pip3"](ptr, start, end)
        or context_metadata_token_equals["uv"](ptr, start, end)
        or context_metadata_token_equals["nyc"](ptr, start, end)
        or context_metadata_token_equals["c8"](ptr, start, end)
        or context_metadata_token_equals["vite"](ptr, start, end)
        or context_metadata_token_equals["next"](ptr, start, end)
        or context_metadata_token_equals["docker-compose"](ptr, start, end)
    ):
        return 9
    return -1


def context_metadata_kind_for_token(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    start: Int64,
    end: Int64,
    next_cursor: Int64,
) -> Int64:
    var direct = context_metadata_direct_kind(ptr, start, end)
    if direct >= 0:
        return direct

    var subcommand = context_metadata_subcommand_after(
        ptr, length, next_cursor
    )
    if (
        context_metadata_token_equals["gradle"](ptr, start, end)
        or context_metadata_token_equals["gradlew"](ptr, start, end)
    ) and subcommand[0] >= 0 and (
        context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["check"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["mvn"](ptr, start, end)
        or context_metadata_token_equals["mvnw"](ptr, start, end)
    ) and subcommand[0] >= 0 and (
        context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["verify"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["package"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["install"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["journalctl"](ptr, start, end)
        or context_metadata_token_equals["tail"](ptr, start, end)
        or context_metadata_token_equals["kubectl"](ptr, start, end)
        and subcommand[0] >= 0
        and context_metadata_token_equals["logs"](ptr, subcommand[0], subcommand[1])
    ):
        return 8
    if context_metadata_token_equals["go"](ptr, start, end) and subcommand[0] >= 0 and (
        context_metadata_token_equals["vet"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
    ):
        return 4
    if context_metadata_token_equals["cargo"](ptr, start, end) and subcommand[0] >= 0:
        if (
            context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["check"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["clippy"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["doc"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["nextest"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fmt"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fix"](ptr, subcommand[0], subcommand[1])
        ):
            return 3
        if (
            context_metadata_token_equals["update"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["install"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fetch"](ptr, subcommand[0], subcommand[1])
        ):
            return 9
    if context_metadata_token_equals["git"](ptr, start, end) and subcommand[0] >= 0:
        if context_metadata_token_equals["status"](ptr, subcommand[0], subcommand[1]):
            return 1
        if (
            context_metadata_token_equals["diff"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["show"](ptr, subcommand[0], subcommand[1])
        ):
            return 2
        if context_metadata_token_equals["log"](ptr, subcommand[0], subcommand[1]):
            return 5
        if context_metadata_token_equals["grep"](ptr, subcommand[0], subcommand[1]):
            return 6
        if context_metadata_token_equals["ls-files"](ptr, subcommand[0], subcommand[1]):
            return 7
    if context_metadata_token_equals["docker"](ptr, start, end) and subcommand[0] >= 0 and (
        context_metadata_token_equals["compose"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["buildx"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["pull"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["npm"](ptr, start, end)
        or context_metadata_token_equals["pnpm"](ptr, start, end)
        or context_metadata_token_equals["yarn"](ptr, start, end)
        or context_metadata_token_equals["bun"](ptr, start, end)
    ):
        var script = context_metadata_package_script_after(
            ptr, length, next_cursor
        )
        if script[0] >= 0:
            return 4
        var install = context_metadata_package_install_after(
            ptr, length, next_cursor
        )
        if install[0] >= 0:
            return 9
    return -1


def context_metadata_kind(view: ProdexStringView) -> Int64:
    var ptr = view.ptr.unsafe_value()
    var length = Int64(view.len)
    var cursor: Int64 = 0
    while cursor < length:
        var token = context_metadata_next_token(ptr, length, cursor)
        if token[0] < 0:
            break
        var kind = context_metadata_kind_for_token(
            ptr, length, token[0], token[1], token[2]
        )
        if kind >= 0:
            return kind
        cursor = token[2]
    return CONTEXT_METADATA_NO_KIND


comptime CONTEXT_CI_RESULT_WIDTH: Int64 = 7
comptime CONTEXT_CI_MARKER: Int64 = 1
comptime CONTEXT_CI_ANNOTATION: Int64 = 2
comptime CONTEXT_CI_JOB: Int64 = 4
comptime CONTEXT_CI_STEP: Int64 = 8
comptime CONTEXT_CI_EXIT_CODE: Int64 = 16
comptime CONTEXT_CI_FAILURE_TEXT: Int64 = 32


def context_text_codepoint_width(value: UInt8) -> Int64:
    if value <= 0x7F:
        return 1
    if value <= 0xDF:
        return 2
    if value <= 0xEF:
        return 3
    return 4


def context_text_whitespace_width(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    if index >= end:
        return 0
    var value = ptr[unsafe_offset=index]
    if (
        value == 9
        or value == 10
        or value == 11
        or value == 12
        or value == 13
        or value == 32
    ):
        return 1
    if value == 0xC2 and index + 1 < end:
        var second = ptr[unsafe_offset=index + 1]
        if second == 0x85 or second == 0xA0:
            return 2
    if value == 0xE1 and index + 2 < end:
        if (
            ptr[unsafe_offset=index + 1] == 0x9A
            and ptr[unsafe_offset=index + 2] == 0x80
        ):
            return 3
    if value == 0xE2 and index + 2 < end:
        var second = ptr[unsafe_offset=index + 1]
        var third = ptr[unsafe_offset=index + 2]
        if second == 0x80 and (third >= 0x80 and third <= 0x8A or third >= 0xA8 and third <= 0xA9 or third == 0xAF):
            return 3
        if second == 0x81 and third == 0x9F:
            return 3
    if (
        value == 0xE3
        and index + 2 < end
        and ptr[unsafe_offset=index + 1] == 0x80
        and ptr[unsafe_offset=index + 2] == 0x80
    ):
        return 3
    return 0


def context_text_trim_bounds(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var bounds = InlineArray[Int64, 2](fill=0)
    var first = end
    var last = start
    var index = start
    while index < end:
        var whitespace = context_text_whitespace_width(ptr, index, end)
        if whitespace > 0:
            index += whitespace
            continue
        if first == end:
            first = index
        index += context_text_codepoint_width(ptr[unsafe_offset=index])
        last = index
    if first == end:
        bounds[0] = end
        bounds[1] = end
    else:
        bounds[0] = first
        bounds[1] = last
    return bounds^


def context_text_ascii_starts_at[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if (
            context_metadata_lower(ptr[unsafe_offset=start + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_text_ascii_starts_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def context_text_ascii_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if (
                context_metadata_lower(ptr[unsafe_offset=offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def context_text_ascii_find[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var length = Int64(literal.byte_length())
    if length == 0:
        return start
    if start < 0 or end < start or start + length > end:
        return -1
    var expected = literal.unsafe_ptr()
    for offset in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if (
                context_metadata_lower(ptr[unsafe_offset=offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return offset
    return -1


def context_ci_first_integer_token(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var index = start
    var token_start: Int64 = -1
    var started = False
    var has_digit = False
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value >= 48 and value <= 57 or not started and value == 45:
            if not started:
                token_start = index
                started = True
            if value >= 48 and value <= 57:
                has_digit = True
            index += 1
        elif started:
            break
        else:
            index += 1
    if has_digit:
        result[0] = token_start
        result[1] = index
    return result^


def context_ci_exit_code_span(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var marker = context_text_ascii_find["exit code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 9, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["exit status"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 11, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["exited with code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 16, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["failed with code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 16, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["code"](ptr, 0, length)
    if marker >= 0:
        return context_ci_first_integer_token(ptr, marker + 4, length)^
    return result^


def context_ci_line_semantics(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, output: Pointer[mut=True, Int64, _]
) -> None:
    var trimmed = context_text_trim_bounds(ptr, 0, length)
    var start = trimmed[0]
    var end = trimmed[1]
    if start >= end:
        return

    if (
        context_text_ascii_starts_at["##[group]"](ptr, start, end)
        or context_text_ascii_starts_at["##[endgroup]"](ptr, start, end)
        or context_text_ascii_starts_at["##[error]"](ptr, start, end)
        or context_text_ascii_starts_at["::error"](ptr, start, end)
        or context_text_ascii_starts_at["current runner version:"](ptr, start, end)
        or context_text_ascii_starts_at["runner name:"](ptr, start, end)
        or context_text_ascii_starts_at["runner os:"](ptr, start, end)
        or context_text_ascii_starts_at["prepare workflow directory"](ptr, start, end)
        or context_text_ascii_starts_at["prepare all required actions"](ptr, start, end)
        or context_text_ascii_starts_at["complete job"](ptr, start, end)
        or context_text_ascii_starts_at["set up job"](ptr, start, end)
        or context_text_ascii_contains["actions/checkout"](ptr, start, end)
        or context_text_ascii_contains["/_actions/"](ptr, start, end)
        or context_text_ascii_contains["github actions"](ptr, start, end)
        or context_text_ascii_contains["process completed with exit code"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_MARKER
    if (
        context_text_ascii_starts_at["##[error]"](ptr, start, end)
        or context_text_ascii_starts_at["::error"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_ANNOTATION
    if (
        context_text_ascii_contains["process completed with exit code"](ptr, start, end)
        or context_text_ascii_contains["failed with exit code"](ptr, start, end)
        or context_text_ascii_contains["exited with code"](ptr, start, end)
        or context_text_ascii_contains["exit status"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_FAILURE_TEXT

    var job_prefix: Int64 = -1
    if context_text_ascii_starts_at["job:"](ptr, start, end):
        job_prefix = 4
    elif context_text_ascii_starts_at["job name:"](ptr, start, end):
        job_prefix = 9
    elif context_text_ascii_starts_at["workflow job:"](ptr, start, end):
        job_prefix = 13
    elif context_text_ascii_starts_at["failed job:"](ptr, start, end):
        job_prefix = 11
    if job_prefix >= 0:
        var job = context_text_trim_bounds(ptr, start + job_prefix, end)
        if job[0] < job[1]:
            output[unsafe_offset=0] |= CONTEXT_CI_JOB
            output[unsafe_offset=1] = job[0]
            output[unsafe_offset=2] = job[1]
    elif context_text_ascii_starts_at["job "](ptr, start, end) and context_text_ascii_contains[" failed"](ptr, start, end):
        output[unsafe_offset=0] |= CONTEXT_CI_JOB
        output[unsafe_offset=1] = start
        output[unsafe_offset=2] = end

    var body_start = start
    if context_text_ascii_starts_exact["##[group]"](ptr, body_start, end):
        body_start += 9
    var body = context_text_trim_bounds(ptr, body_start, end)
    var step_prefix: Int64 = -1
    if context_text_ascii_starts_at["run "](ptr, body[0], body[1]):
        step_prefix = 4
    elif context_text_ascii_starts_at["step:"](ptr, body[0], body[1]):
        step_prefix = 5
    elif context_text_ascii_starts_at["failed step:"](ptr, body[0], body[1]):
        step_prefix = 12
    if step_prefix >= 0:
        var step = context_text_trim_bounds(ptr, body[0] + step_prefix, body[1])
        if step[0] < step[1]:
            output[unsafe_offset=0] |= CONTEXT_CI_STEP
            output[unsafe_offset=3] = step[0]
            output[unsafe_offset=4] = step[1]

    var exit_code = context_ci_exit_code_span(ptr, length)
    if exit_code[0] >= 0:
        output[unsafe_offset=0] |= CONTEXT_CI_EXIT_CODE
        output[unsafe_offset=5] = exit_code[0]
        output[unsafe_offset=6] = exit_code[1]


def context_search_ascii_starts_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def context_search_ascii_contains_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if start < 0 or end < start or start + length > end:
        return False
    for offset in range(start, end - length + 1):
        if context_search_ascii_starts_exact[literal](ptr, offset, end):
            return True
    return False


def context_search_ascii_find_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var length = Int64(literal.byte_length())
    if length == 0:
        return start
    if start < 0 or end < start or start + length > end:
        return -1
    for offset in range(start, end - length + 1):
        if context_search_ascii_starts_exact[literal](ptr, offset, end):
            return offset
    return -1


def context_search_find_byte(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, value: UInt8
) -> Int64:
    for index in range(start, end):
        if ptr[unsafe_offset=index] == value:
            return index
    return -1


def context_search_all_digits(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            return False
    return True


def context_search_parse_i64_digits(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if start >= end or not context_search_all_digits(ptr, start, end):
        return -1
    var value: Int64 = 0
    for index in range(start, end):
        var digit = Int64(ptr[unsafe_offset=index] - 48)
        if value > 922337203685477580 or value * 10 > 9223372036854775807 - digit:
            return -1
        value = value * 10 + digit
    return value


def context_search_whitespace_count(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var count: Int64 = 0
    var index = start
    while index < end:
        var whitespace = context_text_whitespace_width(ptr, index, end)
        if whitespace > 0:
            count += 1
            if count > 1:
                return count
            index += whitespace
            continue
        index += context_text_codepoint_width(ptr[unsafe_offset=index])
    return count


def context_search_path_like(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_search_find_byte(ptr, start, end, 47) >= 0
        or context_search_find_byte(ptr, start, end, 92) >= 0
        or context_search_find_byte(ptr, start, end, 46) >= 0
    )


def context_search_bare_path_entry(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if start >= end or context_search_whitespace_count(ptr, start, end) > 0:
        return False
    var first = ptr[unsafe_offset=start]
    if first == 45 or first == 123 or first == 91:
        return False
    if context_search_find_byte(ptr, start, end, 58) >= 0:
        return False
    if (
        context_search_ascii_starts_exact["Cargo.toml"](ptr, start, end)
        or context_search_ascii_starts_exact["Cargo.lock"](ptr, start, end)
        or context_search_ascii_starts_exact["Makefile"](ptr, start, end)
        or context_search_ascii_starts_exact["README"](ptr, start, end)
        or context_search_ascii_starts_exact["README.md"](ptr, start, end)
        or context_search_ascii_starts_exact["LICENSE"](ptr, start, end)
        or context_search_ascii_starts_exact["AGENTS.md"](ptr, start, end)
        or context_search_ascii_starts_exact[".gitignore"](ptr, start, end)
    ):
        return True
    var dot: Int64 = -1
    for index in range(start, end):
        if ptr[unsafe_offset=index] == 46:
            dot = index
    if dot < start or dot + 1 >= end or end - dot - 1 > 12:
        return False
    for index in range(dot + 1, end):
        var value = ptr[unsafe_offset=index]
        if not (
            value >= 48
            and value <= 57
            or value >= 65
            and value <= 90
            or value >= 97
            and value <= 122
            or value == 95
            or value == 45
        ):
            return False
    return True


def context_search_file_list_candidate(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if (
        context_search_ascii_starts_exact["./"](ptr, start, end)
        or ptr[unsafe_offset=start] == 47
        or context_search_find_byte(ptr, start, end, 47) >= 0
        or context_search_find_byte(ptr, start, end, 92) >= 0
        or context_search_ascii_starts_exact["|-- "](ptr, start, end)
        or context_search_ascii_starts_exact["`-- "](ptr, start, end)
        or context_search_ascii_starts_exact["├── "](ptr, start, end)
        or context_search_ascii_starts_exact["└── "](ptr, start, end)
    ):
        return True
    return context_search_bare_path_entry(ptr, start, end)


def context_search_windows_drive_prefix(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start < 3:
        return False
    var first = ptr[unsafe_offset=start]
    return (
        (first >= 65 and first <= 90 or first >= 97 and first <= 122)
        and ptr[unsafe_offset=start + 1] == 58
        and (ptr[unsafe_offset=start + 2] == 47 or ptr[unsafe_offset=start + 2] == 92)
    )


def context_search_heading_path_bounds(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var trimmed = context_text_trim_bounds(ptr, 0, length)
    var start = trimmed[0]
    var end = trimmed[1]
    if start >= end or context_search_ascii_starts_exact["--"](ptr, start, end):
        return result^
    if (
        context_search_find_byte(ptr, start, end, 58) >= 0
        and not (
            context_search_windows_drive_prefix(ptr, start, end)
            and context_search_find_byte(ptr, start + 2, end, 58) < 0
        )
    ):
        return result^
    if (
        context_search_ascii_contains_exact["://"](ptr, start, end)
        or context_search_whitespace_count(ptr, start, end) > 1
        or not context_search_file_list_candidate(ptr, start, end)
        or not context_search_path_like(ptr, start, end)
    ):
        return result^
    result[0] = start
    result[1] = end
    return result^


def context_search_last_marker_end[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var length = Int64(literal.byte_length())
    var found: Int64 = -1
    if start < 0 or end < start or start + length > end:
        return found
    for index in range(start, end - length + 1):
        if context_search_ascii_starts_exact[literal](ptr, index, end):
            found = index + length
    return found


def context_search_copy_normalized_path(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var trimmed = context_text_trim_bounds(ptr, start, end)
    var marker_end: Int64 = -1
    var candidate = context_search_last_marker_end["|-- "](ptr, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["`-- "](ptr, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["├── "](ptr, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["└── "](ptr, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    var suffix_start = trimmed[0] if marker_end < 0 else marker_end
    var suffix = context_text_trim_bounds(ptr, suffix_start, trimmed[1])
    var written: Int64 = 0
    for index in range(suffix[0], suffix[1]):
        if written >= capacity:
            return -1
        var value = ptr[unsafe_offset=index]
        if value == 92:
            value = 47
        output[unsafe_offset=written] = value
        written += 1
    return written


def context_search_copy_trimmed_span(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var bounds = context_text_trim_bounds(ptr, start, end)
    var length = bounds[1] - bounds[0]
    if length > capacity:
        return -1
    for index in range(length):
        output[unsafe_offset=index] = ptr[unsafe_offset=bounds[0] + index]
    return length


def context_search_json_field_bounds[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var marker = context_search_ascii_find_exact[literal](ptr, start, end)
    if marker < 0:
        return result^
    var colon = context_search_find_byte(
        ptr, marker + Int64(literal.byte_length()), end, 58
    )
    if colon < 0:
        return result^
    var value_start = colon + 1
    while value_start < end:
        var whitespace = context_text_whitespace_width(ptr, value_start, end)
        if whitespace == 0:
            break
        value_start += whitespace
    if value_start >= end or ptr[unsafe_offset=value_start] != 34:
        return result^
    value_start += 1
    var cursor = value_start
    var escaped = False
    while cursor < end:
        var value = ptr[unsafe_offset=cursor]
        if escaped:
            escaped = False
            cursor += context_text_codepoint_width(value)
        elif value == 92:
            escaped = True
            cursor += 1
        elif value == 34:
            result[0] = value_start
            result[1] = cursor
            return result^
        else:
            cursor += context_text_codepoint_width(value)
    return result^


def context_search_json_value_equals[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var expected = literal.unsafe_ptr()
    var expected_length = Int64(literal.byte_length())
    var expected_index: Int64 = 0
    var cursor = start
    while cursor < end:
        var value = ptr[unsafe_offset=cursor]
        if value == 92:
            cursor += 1
            if cursor >= end:
                return False
            value = ptr[unsafe_offset=cursor]
            if value == 110:
                value = 10
                cursor += 1
            elif value == 114:
                value = 13
                cursor += 1
            elif value == 116:
                value = 9
                cursor += 1
            elif value == 34 or value == 92:
                cursor += 1
            else:
                var width = context_text_codepoint_width(value)
                if expected_index + width > expected_length:
                    return False
                for index in range(width):
                    if ptr[unsafe_offset=cursor + index] != expected[unsafe_offset=expected_index + index]:
                        return False
                expected_index += width
                cursor += width
                continue
        else:
            var width = context_text_codepoint_width(value)
            if expected_index + width > expected_length:
                return False
            for index in range(width):
                if ptr[unsafe_offset=cursor + index] != expected[unsafe_offset=expected_index + index]:
                    return False
            expected_index += width
            cursor += width
            continue
        if expected_index >= expected_length or value != expected[unsafe_offset=expected_index]:
            return False
        expected_index += 1
    return expected_index == expected_length


def context_search_copy_json_value(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    var written: Int64 = 0
    var cursor = start
    while cursor < end:
        var value = ptr[unsafe_offset=cursor]
        if value == 92:
            cursor += 1
            if cursor >= end:
                return -1
            value = ptr[unsafe_offset=cursor]
            if value == 110:
                value = 10
                cursor += 1
            elif value == 114:
                value = 13
                cursor += 1
            elif value == 116:
                value = 9
                cursor += 1
            elif value == 34 or value == 92:
                cursor += 1
            else:
                var width = context_text_codepoint_width(value)
                if written + width > capacity:
                    return -1
                for index in range(width):
                    output[unsafe_offset=written + index] = ptr[unsafe_offset=cursor + index]
                written += width
                cursor += width
                continue
        else:
            var width = context_text_codepoint_width(value)
            if written + width > capacity:
                return -1
            for index in range(width):
                output[unsafe_offset=written + index] = ptr[unsafe_offset=cursor + index]
            written += width
            cursor += width
            continue
        if written >= capacity:
            return -1
        output[unsafe_offset=written] = value
        written += 1
    return written


def context_search_trim_output(
    output: Pointer[mut=True, UInt8, _], length: Int64
) -> Int64:
    var bounds = context_text_trim_bounds(output, 0, length)
    var trimmed_length = bounds[1] - bounds[0]
    for index in range(trimmed_length):
        output[unsafe_offset=index] = output[unsafe_offset=bounds[0] + index]
    return trimmed_length


def context_search_normalize_output_path(
    output: Pointer[mut=True, UInt8, _], written: Int64, capacity: Int64
) -> Int64:
    var trimmed = context_text_trim_bounds(output, 0, written)
    var marker_end: Int64 = -1
    var candidate = context_search_last_marker_end["|-- "](output, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["`-- "](output, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["├── "](output, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    candidate = context_search_last_marker_end["└── "](output, trimmed[0], trimmed[1])
    if candidate > marker_end:
        marker_end = candidate
    var suffix_start = trimmed[0] if marker_end < 0 else marker_end
    var suffix = context_text_trim_bounds(output, suffix_start, trimmed[1])
    var normalized_length = suffix[1] - suffix[0]
    if normalized_length > capacity:
        return -1
    for index in range(normalized_length):
        var value = output[unsafe_offset=suffix[0] + index]
        if value == 92:
            value = 47
        output[unsafe_offset=index] = value
    return normalized_length


def context_search_plain_result(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    path_output: Pointer[mut=True, UInt8, _],
    path_capacity: Int64,
    text_output: Pointer[mut=True, UInt8, _],
    text_capacity: Int64,
) -> InlineArray[Int64, 4]:
    var result = InlineArray[Int64, 4](fill=-1)
    result[0] = 0
    var separator_start: Int64 = 0
    if context_search_windows_drive_prefix(ptr, 0, length):
        separator_start = 2
    var separator = context_search_find_byte(ptr, separator_start, length, 58)
    if separator < 0:
        return result^
    var path = context_text_trim_bounds(ptr, 0, separator)
    var rest_start = separator + 1
    var rest = context_text_trim_bounds(ptr, rest_start, length)
    if path[0] >= path[1] or rest[0] >= rest[1]:
        return result^
    var second = context_search_find_byte(ptr, rest_start, length, 58)
    var text_start = rest_start
    var line_number: Int64 = -1
    if second < 0:
        if not context_search_path_like(ptr, path[0], path[1]):
            return result^
    elif context_search_all_digits(ptr, rest_start, second):
        line_number = context_search_parse_i64_digits(ptr, rest_start, second)
        text_start = second + 1
        var column = context_search_find_byte(ptr, text_start, length, 58)
        if column >= 0 and context_search_all_digits(ptr, text_start, column):
            text_start = column + 1
    elif not context_search_path_like(ptr, path[0], path[1]):
        return result^
    var path_length = context_search_copy_normalized_path(
        ptr, path[0], path[1], path_output, path_capacity
    )
    var text_length = context_search_copy_trimmed_span(
        ptr, text_start, length, text_output, text_capacity
    )
    if path_length < 0 or text_length < 0:
        result[0] = CONTEXT_GIT_SEARCH_CAPACITY
        return result^
    result[0] = CONTEXT_GIT_SEARCH_DIRECT_MATCH
    result[1] = line_number
    result[2] = path_length
    result[3] = text_length
    return result^


def context_search_json_result(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    path_output: Pointer[mut=True, UInt8, _],
    path_capacity: Int64,
    text_output: Pointer[mut=True, UInt8, _],
    text_capacity: Int64,
) -> InlineArray[Int64, 4]:
    var result = InlineArray[Int64, 4](fill=-1)
    result[0] = 0
    var trimmed = context_text_trim_bounds(ptr, 0, length)
    if (
        not context_search_ascii_starts_exact["{"](ptr, trimmed[0], trimmed[1])
        or not context_search_ascii_contains_exact["\"type\""](ptr, trimmed[0], trimmed[1])
        or not (
            context_search_ascii_contains_exact["\"data\""](ptr, trimmed[0], trimmed[1])
            or context_search_ascii_contains_exact["\"path\""](ptr, trimmed[0], trimmed[1])
        )
    ):
        return result^
    result[0] = CONTEXT_GIT_SEARCH_JSON_LINE
    var type = context_search_json_field_bounds["\"type\""](ptr, 0, length)
    if type[0] < 0 or not context_search_json_value_equals["match"](ptr, type[0], type[1]):
        return result^
    var path_marker = context_search_ascii_find_exact["\"path\""](ptr, 0, length)
    if path_marker < 0:
        return result^
    var path_start = path_marker + 6
    var path = context_search_json_field_bounds["\"text\""](ptr, path_start, length)
    if path[0] < 0:
        path = context_search_json_field_bounds["\"path\""](ptr, path_start, length)
    if path[0] < 0:
        return result^
    var path_length = context_search_copy_json_value(
        ptr, path[0], path[1], path_output, path_capacity
    )
    if path_length < 0:
        result[0] = CONTEXT_GIT_SEARCH_CAPACITY
        return result^
    path_length = context_search_normalize_output_path(
        path_output, path_length, path_capacity
    )
    if path_length < 0:
        result[0] = CONTEXT_GIT_SEARCH_CAPACITY
        return result^
    var text_length: Int64 = 0
    var lines_marker = context_search_ascii_find_exact["\"lines\""](ptr, 0, length)
    if lines_marker >= 0:
        var lines = context_search_json_field_bounds["\"text\""](
            ptr, lines_marker + 7, length
        )
        if lines[0] >= 0:
            text_length = context_search_copy_json_value(
                ptr, lines[0], lines[1], text_output, text_capacity
            )
            if text_length < 0:
                result[0] = CONTEXT_GIT_SEARCH_CAPACITY
                return result^
            text_length = context_search_trim_output(text_output, text_length)
    result[0] = CONTEXT_GIT_SEARCH_JSON_LINE | CONTEXT_GIT_SEARCH_JSON_MATCH
    result[1] = context_search_json_number["\"line_number\""](ptr, 0, length)
    result[2] = path_length
    result[3] = text_length
    return result^


def context_search_json_number[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var marker = context_search_ascii_find_exact[literal](ptr, start, end)
    if marker < 0:
        return -1
    var colon = context_search_find_byte(
        ptr, marker + Int64(literal.byte_length()), end, 58
    )
    if colon < 0:
        return -1
    var value_start = colon + 1
    while value_start < end:
        var whitespace = context_text_whitespace_width(ptr, value_start, end)
        if whitespace == 0:
            break
        value_start += whitespace
    var value_end = value_start
    while value_end < end:
        var value = ptr[unsafe_offset=value_end]
        if value < 48 or value > 57:
            break
        value_end += 1
    return context_search_parse_i64_digits(ptr, value_start, value_end)


def context_search_heading_match_result(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    heading_present: Bool,
    text_output: Pointer[mut=True, UInt8, _],
    text_capacity: Int64,
) -> InlineArray[Int64, 4]:
    var result = InlineArray[Int64, 4](fill=-1)
    result[0] = 0
    if not heading_present:
        return result^
    var leading = context_text_trim_bounds(ptr, 0, length)
    var start = leading[0]
    var separator = context_search_find_byte(ptr, start, length, 58)
    if separator < 0 or not context_search_all_digits(ptr, start, separator):
        return result^
    var text_start = separator + 1
    var column = context_search_find_byte(ptr, text_start, length, 58)
    if column >= 0 and context_search_all_digits(ptr, text_start, column):
        text_start = column + 1
    var text_length = context_search_copy_trimmed_span(
        ptr, text_start, length, text_output, text_capacity
    )
    if text_length < 0:
        result[0] = CONTEXT_GIT_SEARCH_CAPACITY
        return result^
    result[0] = CONTEXT_GIT_SEARCH_HEADING_MATCH
    result[1] = context_search_parse_i64_digits(ptr, start, separator)
    result[3] = text_length
    return result^


def context_search_classify_line(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    heading_present: Bool,
    path_output: Pointer[mut=True, UInt8, _],
    path_capacity: Int64,
    text_output: Pointer[mut=True, UInt8, _],
    text_capacity: Int64,
) -> InlineArray[Int64, 4]:
    var result = context_search_plain_result(
        ptr, length, path_output, path_capacity, text_output, text_capacity
    )
    if heading_present:
        result = context_search_heading_match_result(
            ptr, length, True, text_output, text_capacity
        )
        if result[0] == CONTEXT_GIT_SEARCH_HEADING_MATCH or result[0] == CONTEXT_GIT_SEARCH_CAPACITY:
            return result^
        result[0] = 0
    if result[0] == CONTEXT_GIT_SEARCH_DIRECT_MATCH or result[0] == CONTEXT_GIT_SEARCH_CAPACITY:
        return result^
    result = context_search_json_result(
        ptr, length, path_output, path_capacity, text_output, text_capacity
    )
    if result[0] != 0:
        return result^
    var heading = context_search_heading_path_bounds(ptr, length)
    if heading[0] >= 0:
        var path_length = context_search_copy_normalized_path(
            ptr, heading[0], heading[1], path_output, path_capacity
        )
        if path_length < 0:
            result[0] = CONTEXT_GIT_SEARCH_CAPACITY
            return result^
        result[0] = CONTEXT_GIT_SEARCH_HEADING_PATH
        result[2] = path_length
        return result^
    return context_search_heading_match_result(
        ptr, length, heading_present, text_output, text_capacity
    )^


def context_text_line_counts(
    counts: Pointer[mut=False, Int64, _], line: Int64
) -> InlineArray[Int64, 7]:
    var offset = line * CONTEXT_SIGNAL_COUNTER_COUNT
    var values = InlineArray[Int64, 7](fill=0)
    values[0] = counts[unsafe_offset=offset]
    values[1] = counts[unsafe_offset=offset + 1]
    values[2] = counts[unsafe_offset=offset + 2]
    values[3] = counts[unsafe_offset=offset + 3]
    values[4] = counts[unsafe_offset=offset + 4]
    values[5] = counts[unsafe_offset=offset + 5]
    values[6] = counts[unsafe_offset=offset + 6]
    return values^


def context_text_counts_have_signal(values: InlineArray[Int64, 7]) -> Bool:
    return (
        values[0] > 0
        or values[1] > 0
        or values[2] > 0
        or values[3] > 0
        or values[4] > 0
        or values[5] > 0
        or values[6] > 0
    )


comptime CONTEXT_GEMINI_GLOB_MAX_BYTES: Int64 = 131_072


def context_gemini_glob_component_bounds(
    view: ProdexStringView, cursor: Int64
) -> InlineArray[Int64, 3]:
    var bounds = InlineArray[Int64, 3](fill=-1)
    var length = Int64(view.len)
    if cursor < 0 or cursor > length:
        return bounds^
    var start = cursor
    ref ptr = view.ptr.unsafe_value()
    var end = start
    while end < length and ptr[unsafe_offset=end] != 47:
        end += 1
    bounds[0] = start
    bounds[1] = end
    bounds[2] = end + 1 if end < length else length + 1
    return bounds^


def context_gemini_glob_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def context_gemini_glob_segment_matches(
    pattern: ProdexStringView,
    pattern_bounds: InlineArray[Int64, 3],
    text: ProdexStringView,
    text_bounds: InlineArray[Int64, 3],
) -> Bool:
    ref pattern_ptr = pattern.ptr.unsafe_value()
    ref text_ptr = text.ptr.unsafe_value()
    var pattern_index = pattern_bounds[0]
    var text_index = text_bounds[0]
    var pattern_end = pattern_bounds[1]
    var text_end = text_bounds[1]
    var star: Int64 = -1
    var star_text = text_index
    while text_index < text_end:
        if pattern_index < pattern_end:
            var value = pattern_ptr[unsafe_offset=pattern_index]
            if value == 42:
                star = pattern_index
                pattern_index += 1
                star_text = text_index
                continue
            if value == 63 or (
                context_gemini_glob_ascii_lower(value)
                == context_gemini_glob_ascii_lower(
                    text_ptr[unsafe_offset=text_index]
                )
            ):
                pattern_index += 1
                text_index += 1
                continue
            if star < 0:
                return False
        elif star < 0:
            return False
        pattern_index = star + 1
        star_text += 1
        text_index = star_text
    while pattern_index < pattern_end and pattern_ptr[unsafe_offset=pattern_index] == 42:
        pattern_index += 1
    return pattern_index == pattern_end


def context_gemini_glob_component_is_double_star(
    view: ProdexStringView, bounds: InlineArray[Int64, 3]
) -> Bool:
    return (
        bounds[1] - bounds[0] == 2
        and view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 42
        and view.ptr.unsafe_value()[unsafe_offset=bounds[0] + 1] == 42
    )


def context_gemini_glob_matches(
    pattern: ProdexStringView, path: ProdexStringView
) -> Bool:
    var pattern_length = Int64(pattern.len)
    var path_length = Int64(path.len)
    var pattern_cursor: Int64 = 0
    var path_cursor: Int64 = 0
    var star_pattern_cursor: Int64 = -1
    var star_path_cursor: Int64 = -1
    while path_cursor <= path_length:
        if pattern_cursor <= pattern_length:
            var pattern_bounds = context_gemini_glob_component_bounds(
                pattern, pattern_cursor
            )
            if context_gemini_glob_component_is_double_star(
                pattern, pattern_bounds
            ):
                star_pattern_cursor = pattern_bounds[2]
                pattern_cursor = pattern_bounds[2]
                star_path_cursor = path_cursor
                continue
            var path_bounds = context_gemini_glob_component_bounds(path, path_cursor)
            if context_gemini_glob_segment_matches(pattern, pattern_bounds, path, path_bounds):
                pattern_cursor = pattern_bounds[2]
                path_cursor = path_bounds[2]
                continue

        if star_pattern_cursor < 0 or star_path_cursor > path_length:
            return False
        var star_bounds = context_gemini_glob_component_bounds(path, star_path_cursor)
        star_path_cursor = star_bounds[2]
        path_cursor = star_path_cursor
        pattern_cursor = star_pattern_cursor
    while pattern_cursor <= pattern_length:
        var bounds = context_gemini_glob_component_bounds(pattern, pattern_cursor)
        if not context_gemini_glob_component_is_double_star(pattern, bounds):
            return False
        pattern_cursor = bounds[2]
    return True


@export("prodex_mojo_text_abi_version")
def prodex_mojo_text_abi_version() abi("C") -> Int64:
    return CONTEXT_TEXT_ABI_VERSION


@export("prodex_mojo_text_abi_layout")
def prodex_mojo_text_abi_layout(
    output: Pointer[mut=True, UInt64, _], output_count: Int64
) abi("C") -> Int64:
    if output_count != 12:
        return 1
    output[unsafe_offset=0] = UInt64(size_of[ProdexStringView]())
    output[unsafe_offset=1] = UInt64(align_of[ProdexStringView]())
    output[unsafe_offset=2] = UInt64(
        reflect[ProdexStringView].field_offset[name="ptr"]()
    )
    output[unsafe_offset=3] = UInt64(
        reflect[ProdexStringView].field_offset[name="len"]()
    )
    output[unsafe_offset=4] = UInt64(size_of[ProdexBytesView]())
    output[unsafe_offset=5] = UInt64(align_of[ProdexBytesView]())
    output[unsafe_offset=6] = UInt64(
        reflect[ProdexBytesView].field_offset[name="ptr"]()
    )
    output[unsafe_offset=7] = UInt64(
        reflect[ProdexBytesView].field_offset[name="len"]()
    )
    output[unsafe_offset=8] = UInt64(size_of[ContextTextRowsResult]())
    output[unsafe_offset=9] = UInt64(align_of[ContextTextRowsResult]())
    output[unsafe_offset=10] = UInt64(
        reflect[ContextTextRowsResult].field_offset[name="abi_version"]()
    )
    output[unsafe_offset=11] = UInt64(
        reflect[ContextTextRowsResult].field_offset[
            name="required_hash_capacity"
        ]()
    )
    return 0


@export("prodex_context_classify_command_output_line_v1")
def prodex_context_classify_command_output_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    result: Pointer[mut=True, ContextCommandOutputLineResult, _],
) abi("C") -> Int64:
    result[] = ContextCommandOutputLineResult(0, 0, 0)
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    var view = line[].copy()
    if view.len > UInt(CONTEXT_OUTPUT_MAX_BYTES):
        return 1
    if not context_text_view_is_valid(view):
        return 2
    result[] = context_output_line_semantics(
        view.ptr.unsafe_value(), Int64(view.len)
    )
    return 0


@export("prodex_context_classify_git_search_line_v1")
def prodex_context_classify_git_search_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    heading_path: Pointer[mut=False, ProdexStringView, _],
    heading_present: Int64,
    path_output: Pointer[mut=True, UInt8, _],
    path_output_capacity: Int64,
    text_output: Pointer[mut=True, UInt8, _],
    text_output_capacity: Int64,
    output: Pointer[mut=True, Int64, _],
    output_count: Int64,
) abi("C") -> Int64:
    if output_count != CONTEXT_GIT_SEARCH_RESULT_WIDTH:
        return 1
    for index in range(CONTEXT_GIT_SEARCH_RESULT_WIDTH):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if (
        heading_present < 0
        or heading_present > 1
        or path_output_capacity < 1
        or path_output_capacity > CONTEXT_GIT_SEARCH_MAX_BYTES
        or text_output_capacity < 1
        or text_output_capacity > CONTEXT_GIT_SEARCH_MAX_BYTES
    ):
        return 1
    var line_view = line[].copy()
    var heading_view = heading_path[].copy()
    if (
        line_view.len > UInt(CONTEXT_GIT_SEARCH_MAX_BYTES)
        or heading_present == 1
        and heading_view.len > UInt(CONTEXT_GIT_SEARCH_MAX_BYTES)
    ):
        return 1
    if not context_text_view_is_valid(line_view) or (
        heading_present == 1 and not context_text_view_is_valid(heading_view)
    ):
        return 2
    var result = context_search_classify_line(
        line_view.ptr.unsafe_value(),
        Int64(line_view.len),
        heading_present == 1,
        path_output,
        path_output_capacity,
        text_output,
        text_output_capacity,
    )
    if result[0] == CONTEXT_GIT_SEARCH_CAPACITY:
        return 3
    output[unsafe_offset=0] = result[0]
    output[unsafe_offset=1] = result[1]
    output[unsafe_offset=2] = result[2]
    output[unsafe_offset=3] = result[3]
    return 0


@export("prodex_context_classify_command_metadata_v1")
def prodex_context_classify_command_metadata_v1(
    abi_version: Int64,
    metadata: Pointer[mut=False, ProdexStringView, _],
    output_kind: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output_kind[] = CONTEXT_METADATA_NO_KIND
    var view = metadata[].copy()
    if view.len > UInt(CONTEXT_METADATA_MAX_BYTES):
        return 1
    if not context_text_view_is_valid(view):
        return 2
    if view.len == 0:
        return 0
    output_kind[] = context_metadata_kind(view)
    return 0


@export("prodex_context_gemini_glob_matches_v1")
def prodex_context_gemini_glob_matches_v1(
    abi_version: Int64,
    pattern: Pointer[mut=False, ProdexStringView, _],
    path: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output[] = 0
    var pattern_view = pattern[].copy()
    var path_view = path[].copy()
    if (
        pattern_view.len > UInt(CONTEXT_GEMINI_GLOB_MAX_BYTES)
        or path_view.len > UInt(CONTEXT_GEMINI_GLOB_MAX_BYTES)
    ):
        return 1
    if not context_text_view_is_valid(pattern_view) or not context_text_view_is_valid(
        path_view
    ):
        return 2
    if context_gemini_glob_matches(pattern_view, path_view):
        output[] = 1
    return 0


@export("prodex_context_classify_ci_line_v1")
def prodex_context_classify_ci_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
    output_count: Int64,
) abi("C") -> Int64:
    if output_count != CONTEXT_CI_RESULT_WIDTH:
        return 1
    for index in range(CONTEXT_CI_RESULT_WIDTH):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    var view = line[].copy()
    if not context_text_view_is_valid(view):
        return 2
    if view.len == 0:
        return 0
    context_ci_line_semantics(view.ptr.unsafe_value(), Int64(view.len), output)
    return 0


@export("prodex_context_classify_dot_reporter_success_line_v1")
def prodex_context_classify_dot_reporter_success_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output[] = 0
    var view = line[].copy()
    if not context_text_view_is_valid(view):
        return 2
    if view.len < 4:
        return 0
    ref ptr = view.ptr.unsafe_value()
    for index in range(Int64(view.len)):
        if ptr[unsafe_offset=index] != 46:
            return 0
    output[] = 1
    return 0


def context_success_prefix[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_text_ascii_starts_at[literal](ptr, start, end)


def context_success_exact_prefix[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_text_ascii_starts_exact[literal](ptr, start, end)


def context_success_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return end - start == Int64(literal.byte_length()) and context_success_prefix[literal](ptr, start, end)


def context_success_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return context_text_ascii_contains[literal](ptr, start, end)


def context_success_ends[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    return end - start >= length and context_text_ascii_starts_at[literal](ptr, end - length, end)


def context_success_all_dots(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if end - start < 4:
        return False
    for index in range(start, end):
        if ptr[unsafe_offset=index] != 46:
            return False
    return True


def context_success_has_digit(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    for index in range(start, end):
        if ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            return True
    return False


def context_success_label(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if context_success_exact_prefix["Compiling "](ptr, start, end):
        return 1
    if context_success_exact_prefix["Checking "](ptr, start, end):
        return 2
    if context_success_exact_prefix["Fresh "](ptr, start, end):
        return 3
    if context_success_exact_prefix["Documenting "](ptr, start, end):
        return 4
    if context_success_exact_prefix["Formatting "](ptr, start, end):
        return 5
    if context_success_exact_prefix["Fixing "](ptr, start, end) or context_success_exact_prefix["Fixed "](ptr, start, end):
        return 6
    if context_success_exact_prefix["Generated "](ptr, start, end):
        return 7
    if context_success_exact_prefix["Finished "](ptr, start, end) and not context_success_prefix["finished in "](ptr, start, end):
        return 8
    if context_success_exact_prefix["Running "](ptr, start, end):
        return 9
    if context_success_exact_prefix["Doc-tests "](ptr, start, end):
        return 10
    if context_success_prefix["running "](ptr, start, end) and context_success_ends[" tests"](ptr, start, end):
        return 11
    if context_success_prefix["test "](ptr, start, end) and context_success_contains[" ... ok"](ptr, start, end):
        return 12
    if context_success_prefix["pass ["](ptr, start, end):
        return 13
    if context_success_prefix["summary ["](ptr, start, end) and context_success_contains[" passed"](ptr, start, end):
        return 14
    if context_success_prefix["test result: ok"](ptr, start, end):
        return 15
    if context_success_prefix["project '"](ptr, start, end) and context_success_contains[" is up to date"](ptr, start, end):
        return 16
    if context_success_prefix["building project '"](ptr, start, end) or context_success_prefix["updating unchanged output timestamps"](ptr, start, end) or context_success_prefix["projects in this build:"](ptr, start, end):
        return 16
    if context_success_prefix["vite "](ptr, start, end) and context_success_contains[" building for production"](ptr, start, end):
        return 17
    if context_success_exact["transforming..."](ptr, start, end) or context_success_exact["rendering chunks..."](ptr, start, end) or context_success_exact["computing gzip size..."](ptr, start, end) or context_success_contains[" modules transformed"](ptr, start, end):
        return 17
    if context_success_prefix["next.js "](ptr, start, end) or context_success_prefix["creating an optimized production build"](ptr, start, end) or context_success_contains["compiled successfully"](ptr, start, end) or context_success_prefix["linting and checking validity of types"](ptr, start, end) or context_success_prefix["collecting page data"](ptr, start, end) or context_success_prefix["generating static pages"](ptr, start, end) or context_success_prefix["finalizing page optimization"](ptr, start, end) or context_success_prefix["collecting build traces"](ptr, start, end):
        return 18
    if context_success_all_dots(ptr, start, end):
        return 19
    if context_success_prefix["bun test v"](ptr, start, end) or context_success_prefix["(pass) "](ptr, start, end):
        return 20
    if context_success_prefix["pass "](ptr, start, end):
        return 23
    if context_success_prefix["ok "](ptr, start, end) and end - start > 3:
        return 24
    if context_success_prefix["? "](ptr, start, end) and context_success_contains["[no test files]"](ptr, start, end):
        return 25
    if context_success_prefix["=== run "](ptr, start, end):
        return 26
    if context_success_prefix["=== pause "](ptr, start, end):
        return 27
    if context_success_prefix["=== cont "](ptr, start, end):
        return 28
    if context_success_prefix["--- pass: "](ptr, start, end):
        return 29
    if context_success_prefix["--- skip: "](ptr, start, end):
        return 30
    if context_success_exact["pass"](ptr, start, end):
        return 31
    if context_success_prefix["test suites:"](ptr, start, end) and context_success_contains["passed"](ptr, start, end):
        return 32
    if context_success_prefix["tests:"](ptr, start, end) and context_success_contains["passed"](ptr, start, end):
        return 33
    if context_success_prefix["snapshots:"](ptr, start, end) and (context_success_contains["passed"](ptr, start, end) or context_success_contains["0 total"](ptr, start, end)):
        return 34
    if context_success_prefix["test files"](ptr, start, end) and context_success_contains["passed"](ptr, start, end):
        return 35
    if context_success_prefix["duration"](ptr, start, end):
        return 36
    if context_success_prefix["time:"](ptr, start, end):
        return 37
    if context_success_prefix["ran all test suites"](ptr, start, end):
        return 38
    if context_success_prefix["done in "](ptr, start, end):
        return 39
    if context_success_prefix["added "](ptr, start, end) and context_success_contains[" package"](ptr, start, end):
        return 40
    if context_success_prefix["audited "](ptr, start, end) and context_success_contains[" package"](ptr, start, end):
        return 41
    if context_success_prefix["updating crates.io index"](ptr, start, end):
        return 42
    if context_success_prefix["locking "](ptr, start, end) and context_success_contains[" package"](ptr, start, end):
        return 43
    if context_success_prefix["downloading crates"](ptr, start, end) or context_success_prefix["downloaded "](ptr, start, end):
        return 44
    if context_success_prefix["packages: "](ptr, start, end) or context_success_prefix["progress: resolved"](ptr, start, end):
        return 45
    if context_success_prefix["lockfile is up to date"](ptr, start, end) or context_success_prefix["already up to date"](ptr, start, end):
        return 46
    if context_success_prefix["requirement already satisfied"](ptr, start, end) or context_success_prefix["successfully installed"](ptr, start, end) or context_success_prefix["installing collected packages"](ptr, start, end):
        return 47
    if context_success_prefix["found 0 vulnerabilities"](ptr, start, end):
        return 48
    if context_success_prefix["all files pass"](ptr, start, end) or context_success_prefix["all checks passed"](ptr, start, end):
        return 49
    if context_success_prefix["compiled successfully"](ptr, start, end):
        return 50
    if context_success_prefix["built in "](ptr, start, end) or context_success_contains[" built in "](ptr, start, end):
        return 51
    if context_success_prefix["tests/"](ptr, start, end) and context_success_contains[" passed"](ptr, start, end):
        return 52
    if context_success_prefix["collected "](ptr, start, end) and context_success_contains[" item"](ptr, start, end):
        return 53
    if context_success_prefix["coverage summary"](ptr, start, end) or context_success_prefix["all files"](ptr, start, end) or context_success_prefix["statements"](ptr, start, end) or context_success_prefix["branches"](ptr, start, end) or context_success_prefix["functions"](ptr, start, end) or context_success_prefix["lines"](ptr, start, end) or context_success_prefix["coverage html written"](ptr, start, end) or context_success_prefix["coverage xml written"](ptr, start, end) or context_success_prefix["coverage json written"](ptr, start, end):
        return 54
    if context_success_prefix["> task "](ptr, start, end) and context_success_contains[":test"](ptr, start, end) and not context_success_ends[" failed"](ptr, start, end):
        return 55
    if context_success_contains[" tests successful"](ptr, start, end) or context_success_contains[" tests skipped"](ptr, start, end) or context_success_ends[" tests completed"](ptr, start, end) or context_success_contains[" tests completed, 0 failed"](ptr, start, end) or context_success_exact["build successful"](ptr, start, end):
        return 55
    if context_success_prefix["[info] running "](ptr, start, end) or context_success_prefix["[info] results:"](ptr, start, end) or context_success_prefix["[info] surefire report directory:"](ptr, start, end):
        return 56
    if context_success_prefix["[info] tests run:"](ptr, start, end) and (context_success_contains["failures: 0"](ptr, start, end) or context_success_contains["errors: 0"](ptr, start, end)):
        return 56
    if context_success_prefix["yarn install v"](ptr, start, end) or context_success_prefix["[1/4] resolving packages"](ptr, start, end) or context_success_prefix["[2/4] fetching packages"](ptr, start, end) or context_success_prefix["[3/4] linking dependencies"](ptr, start, end) or context_success_prefix["[4/4] building fresh packages"](ptr, start, end) or context_success_prefix["success saved lockfile"](ptr, start, end) or context_success_prefix["success already up-to-date"](ptr, start, end) or context_success_prefix["saved lockfile"](ptr, start, end) or context_success_prefix["bun install v"](ptr, start, end) or context_success_contains[" packages installed"](ptr, start, end):
        return 57
    if context_success_prefix["#"](ptr, start, end) and (context_success_contains[" building with "](ptr, start, end) or context_success_contains[" transferring "](ptr, start, end) or context_success_contains[" exporting "](ptr, start, end) or context_success_contains[" resolving provenance"](ptr, start, end) or context_success_contains[" cached"](ptr, start, end)):
        return 58
    if context_success_prefix["//"](ptr, start, end) and (context_success_contains[" passed in "](ptr, start, end) or context_success_ends[" passed"](ptr, start, end)):
        return 59
    if (context_success_prefix["<testsuite"](ptr, start, end) or context_success_prefix["<testsuites"](ptr, start, end)) and context_success_contains["tests="](ptr, start, end) and context_success_contains["failures=\"0\""](ptr, start, end) and context_success_contains["errors=\"0\""](ptr, start, end):
        return 60
    if context_success_prefix["build complete!"](ptr, start, end) or context_success_prefix["test suite "](ptr, start, end) and context_success_contains[" passed at "](ptr, start, end) or context_success_prefix["test case "](ptr, start, end) and context_success_contains[" passed ("](ptr, start, end) or context_success_prefix["executed "](ptr, start, end) and context_success_contains[" tests"](ptr, start, end) and context_success_contains["with 0 failures"](ptr, start, end):
        return 61
    if context_success_prefix["running "](ptr, start, end) and context_success_contains[" tests using "](ptr, start, end) or context_success_prefix["slow test file:"](ptr, start, end) or context_success_contains[" passed ("](ptr, start, end) and context_success_has_digit(ptr, start, end):
        return 62
    if context_success_prefix["checked "](ptr, start, end) or context_success_prefix["formatted "](ptr, start, end) or context_success_prefix["linted "](ptr, start, end) or context_success_exact["no fixes applied."](ptr, start, end) or context_success_prefix["fixed "](ptr, start, end) and context_success_contains[" file"](ptr, start, end):
        return 63
    if context_success_prefix["finished in "](ptr, start, end) and context_success_contains[" on "](ptr, start, end) and context_success_contains[" file"](ptr, start, end):
        return 64
    if context_success_prefix["build successful"](ptr, start, end) or context_success_prefix["build success"](ptr, start, end) or context_success_prefix["[info] build success"](ptr, start, end) or context_success_contains[" build success"](ptr, start, end):
        return 65
    if context_success_contains["actionable tasks:"](ptr, start, end) or context_success_contains["actionable task:"](ptr, start, end):
        return 66
    if context_success_prefix["[info] total time:"](ptr, start, end) or context_success_prefix["[info] finished at:"](ptr, start, end):
        return 67
    if context_success_prefix["=> "](ptr, start, end) or context_success_prefix["=>=> "](ptr, start, end):
        return 68
    if context_success_prefix["successfully built "](ptr, start, end) or context_success_prefix["successfully tagged "](ptr, start, end) or context_success_contains["writing image sha256:"](ptr, start, end) or context_success_contains["naming to "](ptr, start, end):
        return 69
    if context_success_prefix["info: build completed successfully"](ptr, start, end):
        return 70
    if context_success_contains["successfully ran target"](ptr, start, end) or context_success_prefix["nx successfully ran"](ptr, start, end):
        return 71
    if context_success_prefix["tasks:"](ptr, start, end) and context_success_contains["successful"](ptr, start, end):
        return 72
    if context_success_prefix["found 0 errors"](ptr, start, end) or context_success_prefix["found 0 warnings"](ptr, start, end) or context_success_prefix["found 0 issues"](ptr, start, end):
        return 16
    return 0


@export("prodex_context_classify_noisy_success_line_v1")
def prodex_context_classify_noisy_success_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output[] = -1
    var view = line[].copy()
    if not context_text_view_is_valid(view):
        return 2
    var bounds = context_text_trim_bounds(view.ptr.unsafe_value(), 0, Int64(view.len))
    if bounds[1] > bounds[0]:
        output[] = context_success_label(view.ptr.unsafe_value(), bounds[0], bounds[1])
    return 0


@export("prodex_context_prepare_signal_rows_v1")
def prodex_context_prepare_signal_rows_v1(
    abi_version: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    before_counts: Pointer[mut=False, Int64, _],
    before_count: Int64,
    after_views: Pointer[mut=False, ProdexStringView, _],
    after_counts: Pointer[mut=False, Int64, _],
    after_count: Int64,
    before_rows: Pointer[mut=True, Int64, _],
    before_rows_capacity: Int64,
    after_available: Pointer[mut=True, Int64, _],
    key_capacity: Int64,
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    key_hashes: Pointer[mut=True, UInt64, _],
    key_sources: Pointer[mut=True, Int64, _],
    key_indices: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, ContextTextRowsResult, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if (
        before_count < 0
        or before_count > CONTEXT_SIGNAL_MAX_LINES
        or after_count < 0
        or after_count > CONTEXT_SIGNAL_MAX_LINES
        or key_capacity < 0
        or key_capacity > CONTEXT_SIGNAL_MAX_KEYS
        or hash_capacity < 1
    ):
        return 1

    var required_before_rows = before_count * CONTEXT_SIGNAL_ROW_WIDTH
    var total_lines = before_count + after_count
    var required_key_capacity = total_lines
    if required_key_capacity > CONTEXT_SIGNAL_MAX_KEYS:
        required_key_capacity = CONTEXT_SIGNAL_MAX_KEYS
    var required_hash_capacity = context_text_required_hash_capacity(
        required_key_capacity
    )
    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        0,
        0,
        0,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    if (
        before_rows_capacity < required_before_rows
        or key_capacity < required_key_capacity
        or hash_capacity < required_hash_capacity
    ):
        return 1

    for line in range(before_count):
        if not context_text_view_is_valid(before_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                before_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2
    for line in range(after_count):
        if not context_text_view_is_valid(after_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                after_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2

    for slot in range(hash_capacity):
        hash_slots[unsafe_offset=slot] = -1
    var key_count: Int64 = 0
    var after_signal_line_count: Int64 = 0

    for line in range(after_count):
        var line_counts = context_text_line_counts(after_counts, line)
        if not context_text_counts_have_signal(line_counts):
            continue
        var key_id = context_text_intern(
            after_views[unsafe_offset=line],
            CONTEXT_TEXT_SOURCE_AFTER,
            line,
            before_views,
            after_views,
            hash_slots,
            key_hashes,
            key_sources,
            key_indices,
            after_available,
            Pointer(to=key_count),
            key_capacity,
            hash_capacity,
        )
        if key_id < 0:
            return 3
        if after_available[unsafe_offset=key_id] == 9223372036854775807:
            return 2
        after_available[unsafe_offset=key_id] += 1
        after_signal_line_count += 1

    for line in range(before_count):
        var line_counts = context_text_line_counts(before_counts, line)
        var key_id: Int64 = -1
        if context_text_counts_have_signal(line_counts):
            key_id = context_text_intern(
                before_views[unsafe_offset=line],
                CONTEXT_TEXT_SOURCE_BEFORE,
                line,
                before_views,
                after_views,
                hash_slots,
                key_hashes,
                key_sources,
                key_indices,
                after_available,
                Pointer(to=key_count),
                key_capacity,
                hash_capacity,
            )
            if key_id < 0:
                return 3
        before_rows[unsafe_offset=line * CONTEXT_SIGNAL_ROW_WIDTH] = key_id
        var row = line * CONTEXT_SIGNAL_ROW_WIDTH
        before_rows[unsafe_offset=row + 1] = line_counts[0]
        before_rows[unsafe_offset=row + 2] = line_counts[1]
        before_rows[unsafe_offset=row + 3] = line_counts[2]
        before_rows[unsafe_offset=row + 4] = line_counts[3]
        before_rows[unsafe_offset=row + 5] = line_counts[4]
        before_rows[unsafe_offset=row + 6] = line_counts[5]
        before_rows[unsafe_offset=row + 7] = line_counts[6]

    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        required_before_rows,
        key_count,
        after_signal_line_count,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    return 0
