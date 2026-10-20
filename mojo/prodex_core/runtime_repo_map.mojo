from std.memory import Pointer

comptime REPO_MAP_ABI_VERSION: Int64 = 1
comptime REPO_MAP_OK: Int64 = 0
comptime REPO_MAP_INVALID: Int64 = 1
comptime REPO_MAP_CAPACITY: Int64 = 3
comptime REPO_MAP_MAX_INPUT_BYTES: Int64 = 4 * 1024 * 1024
comptime REPO_MAP_MAX_OUTPUT_BYTES: Int64 = 512

def is_ascii_whitespace(value: UInt8) -> Bool:
    return value == 9 or value >= 10 and value <= 13 or value == 32

def unicode_whitespace_len(
    source: Pointer[mut=False, UInt8, _], length: Int64, start: Int64
) -> Int64:
    if start < 0 or start >= length:
        return 0
    if start + 1 < length and source[unsafe_offset=start] == 194 and (
        source[unsafe_offset=start + 1] == 133 or source[unsafe_offset=start + 1] == 160
    ):
        return 2
    if start + 2 >= length:
        return 0
    var first = source[unsafe_offset=start]
    var second = source[unsafe_offset=start + 1]
    var third = source[unsafe_offset=start + 2]
    if first == 225 and second == 154 and third == 128:
        return 3
    if first == 226 and second == 128 and (
        third >= 128 and third <= 138 or third == 168 or third == 169 or third == 175
    ):
        return 3
    if first == 226 and second == 129 and third == 159:
        return 3
    if first == 227 and second == 128 and third == 128:
        return 3
    return 0

def range_starts_with[literal: StaticString](
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or end < start or start + n > end:
        return False
    var wanted = literal.unsafe_ptr()
    for index in range(n):
        if source[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True

def is_ascii_alnum_or_underscore(value: UInt8) -> Bool:
    return value >= 48 and value <= 57 or value >= 65 and value <= 90 or value >= 97 and value <= 122 or value == 95

def repo_module_like(source: Pointer[mut=False, UInt8, _], length: Int64) -> Bool:
    var line_start: Int64 = 0
    while line_start < length:
        var line_end = line_start
        while line_end < length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        if line_end > line_start and source[unsafe_offset=line_end - 1] == 13:
            line_end -= 1

        var start = line_start
        while start < line_end:
            var value = source[unsafe_offset=start]
            if is_ascii_whitespace(value):
                start += 1
                continue
            var unicode_length = unicode_whitespace_len(source, line_end, start)
            if unicode_length > 0:
                start += unicode_length
                continue
            break

        if start < line_end:
            if source[unsafe_offset=start] == 35 or source[unsafe_offset=start] == 64 or range_starts_with["//"](source, start, line_end):
                pass
            else:
                if range_starts_with["pub(crate) "](source, start, line_end):
                    start += 11
                elif range_starts_with["pub(super) "](source, start, line_end):
                    start += 11
                elif range_starts_with["pub "](source, start, line_end):
                    start += 4

                if range_starts_with["export default "](source, start, line_end):
                    start += 15
                elif range_starts_with["export "](source, start, line_end):
                    start += 7
                elif range_starts_with["async "](source, start, line_end):
                    start += 6

                while start < line_end and not is_ascii_alnum_or_underscore(source[unsafe_offset=start]):
                    start += 1
                var token_end = start
                while token_end < line_end and is_ascii_alnum_or_underscore(source[unsafe_offset=token_end]):
                    token_end += 1
                if token_end > start:
                    return (
                        token_end - start == 3
                        and source[unsafe_offset=start] == 109
                        and source[unsafe_offset=start + 1] == 111
                        and source[unsafe_offset=start + 2] == 100
                    ) or (
                        token_end - start == 5
                        and source[unsafe_offset=start] == 99
                        and source[unsafe_offset=start + 1] == 108
                        and source[unsafe_offset=start + 2] == 97
                        and source[unsafe_offset=start + 3] == 115
                        and source[unsafe_offset=start + 4] == 115
                    )
                return False
        if line_end >= length:
            break
        line_start = line_end + 1
    return False

def repo_module_from_path(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var start: Int64 = 0
    var end = length

    if range_starts_with["a/"](source, start, end):
        start += 2
    if range_starts_with["b/"](source, start, end):
        start += 2
    if range_starts_with["./"](source, start, end):
        start += 2

    while start < end and source[unsafe_offset=start] == 34:
        start += 1
    while end > start and source[unsafe_offset=end - 1] == 34:
        end -= 1

    var dot: Int64 = -1
    var cursor = end
    while cursor > start:
        cursor -= 1
        if source[unsafe_offset=cursor] == 46:
            dot = cursor
            break
    if dot >= 0:
        end = dot

    var part_count: Int64 = 0
    var first_start: Int64 = -1
    var first_end: Int64 = -1
    var last_start: Int64 = -1
    var last_end: Int64 = -1
    cursor = start
    while cursor < end:
        while cursor < end and (
            source[unsafe_offset=cursor] == 47 or source[unsafe_offset=cursor] == 92
        ):
            cursor += 1
        if cursor >= end:
            break
        var part_start = cursor
        while cursor < end and (
            source[unsafe_offset=cursor] != 47 and source[unsafe_offset=cursor] != 92
        ):
            cursor += 1
        var part_end = cursor
        if part_end - part_start == 1 and source[unsafe_offset=part_start] == 46:
            continue
        if part_count == 0:
            first_start = part_start
            first_end = part_end
        last_start = part_start
        last_end = part_end
        part_count += 1

    var skip_first = (
        part_count > 1
        and first_end - first_start == 3
        and source[unsafe_offset=first_start] == 115
        and source[unsafe_offset=first_start + 1] == 114
        and source[unsafe_offset=first_start + 2] == 99
    )
    var effective_count = part_count - Int64(skip_first)
    var tail_len = last_end - last_start
    var tail_special = (
        tail_len == 3
        and (
            (
                source[unsafe_offset=last_start] == 109
                and source[unsafe_offset=last_start + 1] == 111
                and source[unsafe_offset=last_start + 2] == 100
            )
            or (
                source[unsafe_offset=last_start] == 108
                and source[unsafe_offset=last_start + 1] == 105
                and source[unsafe_offset=last_start + 2] == 98
            )
        )
    ) or (
        tail_len == 4
        and source[unsafe_offset=last_start] == 109
        and source[unsafe_offset=last_start + 1] == 97
        and source[unsafe_offset=last_start + 2] == 105
        and source[unsafe_offset=last_start + 3] == 110
    ) or (
        tail_len == 5
        and source[unsafe_offset=last_start] == 105
        and source[unsafe_offset=last_start + 1] == 110
        and source[unsafe_offset=last_start + 2] == 100
        and source[unsafe_offset=last_start + 3] == 101
        and source[unsafe_offset=last_start + 4] == 120
    )
    var skip_last = effective_count > 1 and tail_special

    written[] = 0
    var valid_index: Int64 = 0
    var emitted: Int64 = 0
    cursor = start
    while cursor < end:
        while cursor < end and (
            source[unsafe_offset=cursor] == 47 or source[unsafe_offset=cursor] == 92
        ):
            cursor += 1
        if cursor >= end:
            break
        var part_start = cursor
        while cursor < end and (
            source[unsafe_offset=cursor] != 47 and source[unsafe_offset=cursor] != 92
        ):
            cursor += 1
        var part_end = cursor
        if part_end - part_start == 1 and source[unsafe_offset=part_start] == 46:
            continue
        var omit = (skip_first and valid_index == 0) or (
            skip_last and valid_index == part_count - 1
        )
        if not omit:
            if emitted > 0:
                if written[] + 2 > capacity or written[] + 2 > REPO_MAP_MAX_OUTPUT_BYTES:
                    return REPO_MAP_CAPACITY
                output[unsafe_offset=written[]] = 58
                output[unsafe_offset=written[] + 1] = 58
                written[] += 2
            var n = part_end - part_start
            if n < 0 or written[] + n > capacity or written[] + n > REPO_MAP_MAX_OUTPUT_BYTES:
                return REPO_MAP_CAPACITY
            for offset in range(n):
                output[unsafe_offset=written[] + offset] = source[unsafe_offset=part_start + offset]
            written[] += n
            emitted += 1
        valid_index += 1
    return REPO_MAP_OK

@export("prodex_runtime_repo_module_path_v1")
def prodex_runtime_repo_module_path_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or address == 0
        or length < 0
        or length > REPO_MAP_MAX_INPUT_BYTES
        or output_address == 0
        or output_capacity < 1
        or output_capacity > REPO_MAP_MAX_OUTPUT_BYTES
        or written_address == 0
    ):
        return REPO_MAP_INVALID
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    return repo_module_from_path(source, length, output, output_capacity, written)


comptime REPO_CHUNK_ACCEPT: Int64 = 0
comptime REPO_CHUNK_REJECT_CAPACITY: Int64 = 1
comptime REPO_CHUNK_REJECT_INVALID: Int64 = 2

comptime REPO_DUPLICATE_SKIP: Int64 = 0
comptime REPO_DUPLICATE_APPEND: Int64 = 1
comptime REPO_DUPLICATE_STOP_CAPACITY: Int64 = 2

comptime REPO_SYMBOL_KIND_MODULE: Int64 = 1
comptime REPO_SYMBOL_KIND_SYMBOL: Int64 = 2
comptime REPO_SYMBOL_KIND_TEST: Int64 = 3


@export("prodex_runtime_repo_chunk_plan_v1")
def prodex_runtime_repo_chunk_plan_v1(
    abi_version: Int64,
    current_count: Int64,
    max_count: Int64,
    byte_len: Int64,
    text_len: Int64,
    hash_matches: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or current_count < 0
        or max_count < 0
        or byte_len < 0
        or text_len < 0
        or (hash_matches != 0 and hash_matches != 1)
    ):
        return -1
    if current_count >= max_count:
        return REPO_CHUNK_REJECT_CAPACITY
    if byte_len != text_len or hash_matches == 0:
        return REPO_CHUNK_REJECT_INVALID
    return REPO_CHUNK_ACCEPT


@export("prodex_runtime_repo_duplicate_plan_v1")
def prodex_runtime_repo_duplicate_plan_v1(
    abi_version: Int64,
    occurrence_count: Int64,
    duplicate_count: Int64,
    max_duplicate_count: Int64,
    max_occurrences: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or occurrence_count < 0
        or duplicate_count < 0
        or max_duplicate_count < 0
        or max_occurrences < 0
    ):
        return -1
    if occurrence_count < 2:
        return REPO_DUPLICATE_SKIP
    if duplicate_count >= max_duplicate_count:
        return REPO_DUPLICATE_STOP_CAPACITY
    var occurrences_complete = Int64(occurrence_count <= max_occurrences)
    return REPO_DUPLICATE_APPEND | (occurrences_complete << 8)


@export("prodex_runtime_repo_path_distance_v1")
def prodex_runtime_repo_path_distance_v1(
    abi_version: Int64,
    start: Int64,
    end: Int64,
    line: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or start < 0
        or end < start
        or line < 0
    ):
        return -1
    if start <= line and line <= end:
        return 0
    var left = start - line if start >= line else line - start
    var right = end - line if end >= line else line - end
    return min(left, right)


@export("prodex_runtime_repo_symbol_kind_v1")
def prodex_runtime_repo_symbol_kind_v1(
    abi_version: Int64,
    label_address: UInt,
    label_length: Int64,
    text_address: UInt,
    text_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or label_length < 0
        or text_length < 0
        or (label_length > 0 and label_address == 0)
        or (text_length > 0 and text_address == 0)
    ):
        return -1
    var label = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(label_address)
    )
    if (
        label_length == 11
        and range_starts_with["test_symbol"](label, 0, label_length)
    ):
        return REPO_SYMBOL_KIND_TEST
    if (
        label_length == 6
        and range_starts_with["symbol"](label, 0, label_length)
    ):
        var text = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(text_address)
        )
        if repo_module_like(text, text_length):
            return REPO_SYMBOL_KIND_MODULE
    return REPO_SYMBOL_KIND_SYMBOL


def repo_lexical_less(
    left: Pointer[mut=False, UInt8, _],
    left_length: Int64,
    right: Pointer[mut=False, UInt8, _],
    right_length: Int64,
) -> Bool:
    var common = min(left_length, right_length)
    for index in range(common):
        var left_byte = left[unsafe_offset=index]
        var right_byte = right[unsafe_offset=index]
        if left_byte < right_byte:
            return True
        if left_byte > right_byte:
            return False
    return left_length < right_length


@export("prodex_runtime_repo_entry_replace_v1")
def prodex_runtime_repo_entry_replace_v1(
    abi_version: Int64,
    incoming_order: UInt64,
    current_order: UInt64,
    incoming_id_address: UInt,
    incoming_id_length: Int64,
    current_id_address: UInt,
    current_id_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != REPO_MAP_ABI_VERSION
        or incoming_id_length < 0
        or current_id_length < 0
        or (incoming_id_length > 0 and incoming_id_address == 0)
        or (current_id_length > 0 and current_id_address == 0)
    ):
        return -1
    if incoming_order > current_order:
        return 1
    if incoming_order < current_order:
        return 0
    var incoming = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(incoming_id_address)
    )
    var current = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(current_id_address)
    )
    return Int64(
        repo_lexical_less(
            incoming,
            incoming_id_length,
            current,
            current_id_length,
        )
    )


# Projection priority for artifacts: newest order first, lexical artifact ID
# ascending on exact order ties, stable original index on exact equality.
# The kernel uses the existing repo-map lexical byte comparator, not a
# second policy implementation in Rust.
comptime REPO_ARTIFACT_ORDER_MAX: Int64 = 65_536
comptime REPO_ARTIFACT_ORDER_WORDS: Int64 = 3


def repo_artifact_order_compare(
    rows: Pointer[mut=False, UInt64, _], left: Int64, right: Int64,
) -> Int64:
    var l = left * REPO_ARTIFACT_ORDER_WORDS
    var r = right * REPO_ARTIFACT_ORDER_WORDS
    var left_order = rows[unsafe_offset=l]
    var right_order = rows[unsafe_offset=r]
    if left_order != right_order:
        return -1 if left_order > right_order else 1
    var left_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows[unsafe_offset=l + 1])
    )
    var right_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows[unsafe_offset=r + 1])
    )
    var left_len = Int64(rows[unsafe_offset=l + 2])
    var right_len = Int64(rows[unsafe_offset=r + 2])
    if repo_lexical_less(left_ptr, left_len, right_ptr, right_len):
        return -1
    if repo_lexical_less(right_ptr, right_len, left_ptr, left_len):
        return 1
    if left != right:
        return -1 if left < right else 1
    return 0


def repo_artifact_order_heap_down(
    rows: Pointer[mut=False, UInt64, _],
    indices: Pointer[mut=True, Int64, _],
    first: Int64, count: Int64,
):
    var root = first
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        var swap = root
        if repo_artifact_order_compare(
            rows, indices[unsafe_offset=swap], indices[unsafe_offset=child],
        ) < 0:
            swap = child
        if child + 1 < count and repo_artifact_order_compare(
            rows, indices[unsafe_offset=swap], indices[unsafe_offset=child + 1],
        ) < 0:
            swap = child + 1
        if swap == root:
            return
        var saved = indices[unsafe_offset=root]
        indices[unsafe_offset=root] = indices[unsafe_offset=swap]
        indices[unsafe_offset=swap] = saved
        root = swap


@export("prodex_runtime_repo_artifact_order_v1")
def prodex_runtime_repo_artifact_order_v1(
    abi_version: Int64,
    rows_address: UInt, count: Int64,
    output_address: UInt, output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != REPO_MAP_ABI_VERSION:
        return 4
    if (
        count < 0 or count > REPO_ARTIFACT_ORDER_MAX
        or (count > 0 and (rows_address == 0 or rows_address % 8 != 0))
        or output_address == 0 or output_address % 8 != 0
        or output_capacity < count
    ):
        return REPO_MAP_INVALID
    var rows = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows_address)
    )
    for index in range(count):
        var length = rows[unsafe_offset=index * REPO_ARTIFACT_ORDER_WORDS + 2]
        var address = rows[unsafe_offset=index * REPO_ARTIFACT_ORDER_WORDS + 1]
        if (
            length > UInt64(9223372036854775807)
            or (length > 0 and address == 0)
        ):
            return REPO_MAP_INVALID
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(count):
        indices[unsafe_offset=index] = index
    var start = count // 2
    while start > 0:
        start -= 1
        repo_artifact_order_heap_down(rows, indices, start, count)
    var end = count
    while end > 1:
        end -= 1
        var saved = indices[unsafe_offset=0]
        indices[unsafe_offset=0] = indices[unsafe_offset=end]
        indices[unsafe_offset=end] = saved
        repo_artifact_order_heap_down(rows, indices, 0, end)
    return REPO_MAP_OK


# Persisted Smart Context artifact eviction: oldest first, then lowest
# lexical artifact ID; remove until both item-count and tracked byte caps
# are satisfied. This replaces the host's repeated Rust min_by scan.
comptime REPO_ARTIFACT_EVICTION_ROW_WORDS: Int64 = 4
comptime REPO_ARTIFACT_EVICTION_MAX: Int64 = 1_048_576


def repo_artifact_eviction_compare(
    rows: Pointer[mut=False, UInt64, _], left: Int64, right: Int64,
) -> Int64:
    var l = left * REPO_ARTIFACT_EVICTION_ROW_WORDS
    var r = right * REPO_ARTIFACT_EVICTION_ROW_WORDS
    var left_order = rows[unsafe_offset=l]
    var right_order = rows[unsafe_offset=r]
    if left_order != right_order:
        return -1 if left_order < right_order else 1
    var a = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows[unsafe_offset=l + 1])
    )
    var b = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows[unsafe_offset=r + 1])
    )
    var left_len = Int64(rows[unsafe_offset=l + 2])
    var right_len = Int64(rows[unsafe_offset=r + 2])
    if repo_lexical_less(a, left_len, b, right_len):
        return -1
    if repo_lexical_less(b, right_len, a, left_len):
        return 1
    if left != right:
        return -1 if left < right else 1
    return 0


def repo_artifact_eviction_heap_down(
    rows: Pointer[mut=False, UInt64, _],
    indices: Pointer[mut=True, Int64, _],
    first: Int64, count: Int64,
):
    var root = first
    while root * 2 + 1 < count:
        var child = root * 2 + 1
        var swap = root
        if repo_artifact_eviction_compare(
            rows, indices[unsafe_offset=swap], indices[unsafe_offset=child],
        ) < 0:
            swap = child
        if child + 1 < count and repo_artifact_eviction_compare(
            rows, indices[unsafe_offset=swap], indices[unsafe_offset=child + 1],
        ) < 0:
            swap = child + 1
        if swap == root:
            return
        var saved = indices[unsafe_offset=root]
        indices[unsafe_offset=root] = indices[unsafe_offset=swap]
        indices[unsafe_offset=swap] = saved
        root = swap


@export("prodex_runtime_repo_artifact_eviction_v1")
def prodex_runtime_repo_artifact_eviction_v1(
    abi_version: Int64,
    rows_address: UInt, count: Int64,
    max_count: Int64, total_bytes: UInt64, max_bytes: UInt64,
    scratch_address: UInt, scratch_capacity: Int64,
    output_address: UInt, output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != REPO_MAP_ABI_VERSION:
        return 4
    if (
        count < 0 or count > REPO_ARTIFACT_EVICTION_MAX or max_count < 0
        or (count > 0 and (rows_address == 0 or rows_address % 8 != 0))
        or scratch_address == 0 or scratch_address % 8 != 0
        or output_address == 0 or output_address % 8 != 0
        or scratch_capacity < count or output_capacity < count
        or written_address == 0 or written_address % 8 != 0
    ):
        return REPO_MAP_INVALID
    var rows = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows_address)
    )
    for index in range(count):
        var address = rows[unsafe_offset=index * REPO_ARTIFACT_EVICTION_ROW_WORDS + 1]
        var length = rows[unsafe_offset=index * REPO_ARTIFACT_EVICTION_ROW_WORDS + 2]
        if length > UInt64(9223372036854775807) or (length > 0 and address == 0):
            return REPO_MAP_INVALID

    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    for index in range(count):
        indices[unsafe_offset=index] = index
    var start = count // 2
    while start > 0:
        start -= 1
        repo_artifact_eviction_heap_down(rows, indices, start, count)
    var end = count
    while end > 1:
        end -= 1
        var saved = indices[unsafe_offset=0]
        indices[unsafe_offset=0] = indices[unsafe_offset=end]
        indices[unsafe_offset=end] = saved
        repo_artifact_eviction_heap_down(rows, indices, 0, end)

    var selected = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var chosen = Int64(0)
    var tracked_bytes = total_bytes
    var remaining = count
    for index in range(count):
        if remaining <= max_count and tracked_bytes <= max_bytes:
            break
        var original = indices[unsafe_offset=index]
        selected[unsafe_offset=chosen] = original
        chosen += 1
        remaining -= 1
        var bytes = rows[unsafe_offset=original * REPO_ARTIFACT_EVICTION_ROW_WORDS + 3]
        tracked_bytes = 0 if bytes >= tracked_bytes else tracked_bytes - bytes
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = chosen
    return REPO_MAP_OK
