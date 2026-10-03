from std.memory import Pointer

comptime LOG_THROUGHPUT_ABI_VERSION: Int64 = 1
comptime LOG_THROUGHPUT_OK: Int64 = 0
comptime LOG_THROUGHPUT_INVALID: Int64 = 1
comptime LOG_THROUGHPUT_ABI: Int64 = 4
comptime LOG_THROUGHPUT_MIN_SAMPLE_MS: UInt64 = 250


@export("prodex_log_throughput_sample_plan_v1")
def prodex_log_throughput_sample_plan_v1(
    abi_version: Int64,
    previous_present: Int64,
    previous_tokens: UInt64,
    previous_generation_ms: UInt64,
    current_tokens: UInt64,
    current_generation_ms: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if (
        (previous_present != 0 and previous_present != 1)
        or output_address == 0
    ):
        return LOG_THROUGHPUT_INVALID

    var reset = (
        previous_present == 1
        and (
            current_tokens < previous_tokens
            or current_generation_ms < previous_generation_ms
        )
    )
    var append = (
        previous_present == 0
        or reset
        or current_tokens > previous_tokens
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(reset)
    output[unsafe_offset=1] = Int64(append)
    return LOG_THROUGHPUT_OK


def throughput_write_rate(
    valid: Bool,
    rate: Float64,
    valid_address: UInt,
    rate_address: UInt,
) -> Int64:
    if valid_address == 0 or rate_address == 0:
        return LOG_THROUGHPUT_INVALID
    var valid_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(valid_address)
    )
    var rate_output = Pointer[mut=True, Float64, MutUntrackedOrigin](
        unsafe_from_address=Int(rate_address)
    )
    valid_output[] = Int64(valid)
    rate_output[] = rate if valid else 0.0
    return LOG_THROUGHPUT_OK


@export("prodex_log_throughput_completed_rate_v1")
def prodex_log_throughput_completed_rate_v1(
    abi_version: Int64,
    output_tokens: UInt64,
    generation_ms: UInt64,
    valid_address: UInt,
    rate_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if output_tokens == 0 or generation_ms == 0:
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var rate = (
        Float64(output_tokens) * 1000.0 / Float64(generation_ms)
    )
    return throughput_write_rate(
        True, rate, valid_address, rate_address
    )


@export("prodex_log_throughput_stream_rate_v1")
def prodex_log_throughput_stream_rate_v1(
    abi_version: Int64,
    first_tokens: UInt64,
    first_generation_ms: UInt64,
    last_tokens: UInt64,
    last_generation_ms: UInt64,
    valid_address: UInt,
    rate_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_THROUGHPUT_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if (
        last_generation_ms < first_generation_ms
        or last_tokens < first_tokens
    ):
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var elapsed_ms = last_generation_ms - first_generation_ms
    var tokens = last_tokens - first_tokens
    if (
        elapsed_ms < LOG_THROUGHPUT_MIN_SAMPLE_MS
        or tokens == 0
    ):
        return throughput_write_rate(
            False, 0.0, valid_address, rate_address
        )
    var rate = Float64(tokens) * 1000.0 / Float64(elapsed_ms)
    return throughput_write_rate(
        True, rate, valid_address, rate_address
    )


comptime LOG_RETENTION_POLICY_ABI_VERSION: Int64 = 1
comptime LOG_RETENTION_POLICY_BOUNDED_VALUE: Int64 = 1
comptime LOG_RETENTION_POLICY_ROTATION: Int64 = 2
comptime LOG_RETENTION_POLICY_BOUNDED_TEXT: Int64 = 5
comptime LOG_RETENTION_CANDIDATES_EXPIRED: Int64 = 6
comptime LOG_RETENTION_CANDIDATES_OVER_BUDGET: Int64 = 7
comptime LOG_RETENTION_CANDIDATE_STRIDE: Int = 5
comptime LOG_RETENTION_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


def log_retention_parse_u64_text(
    address: UInt64,
    length: UInt64,
) -> Tuple[Bool, UInt64]:
    if length == 0 or length > UInt64(0x7FFF_FFFF_FFFF_FFFF) or address == 0:
        return (False, UInt64(0))
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var index: UInt64 = 0
    if source[unsafe_offset=0] == 43:
        index = 1
        if index == length:
            return (False, UInt64(0))
    var value: UInt64 = 0
    while index < length:
        var byte = source[unsafe_offset=Int(index)]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(byte - 48)
        if value > (LOG_RETENTION_UINT64_MAX - digit) // 10:
            return (False, UInt64(0))
        value = value * 10 + digit
        index += 1
    return (True, value)


@export("prodex_log_retention_policy_v1")
def prodex_log_retention_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input0: UInt64,
    input1: UInt64,
    input2: UInt64,
    input3: UInt64,
    input4: UInt64,
    input5: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_RETENTION_POLICY_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if output_address == 0:
        return LOG_THROUGHPUT_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    if operation == LOG_RETENTION_POLICY_BOUNDED_VALUE:
        # input0 present, input1 value, input2 default, input3 min, input4 max
        if input0 > 1 or input3 > input4:
            return LOG_THROUGHPUT_INVALID
        if input0 == 1 and input1 >= input3 and input1 <= input4:
            output[unsafe_offset=0] = input1
        else:
            output[unsafe_offset=0] = input2
        return LOG_THROUGHPUT_OK

    if operation == LOG_RETENTION_POLICY_ROTATION:
        # current_size, line_len, max_file_bytes
        var current_size = input0
        var line_len = input1
        var max_file_bytes = input2
        var next_size: UInt64
        if LOG_RETENTION_UINT64_MAX - current_size < line_len:
            next_size = LOG_RETENTION_UINT64_MAX
        else:
            next_size = current_size + line_len
        output[unsafe_offset=0] = UInt64(
            current_size > 0 and next_size > max_file_bytes
        )
        output[unsafe_offset=1] = UInt64(line_len > max_file_bytes)
        return LOG_THROUGHPUT_OK

    if operation == LOG_RETENTION_POLICY_BOUNDED_TEXT:
        # input0 address, input1 length, input2 present, input3 default, input4 min, input5 max
        if input2 > 1 or input4 > input5:
            return LOG_THROUGHPUT_INVALID
        if input2 == 0:
            output[unsafe_offset=0] = input3
            return LOG_THROUGHPUT_OK
        var parsed = log_retention_parse_u64_text(input0, input1)
        if parsed[0] and parsed[1] >= input4 and parsed[1] <= input5:
            output[unsafe_offset=0] = parsed[1]
        else:
            output[unsafe_offset=0] = input3
        return LOG_THROUGHPUT_OK

    return LOG_THROUGHPUT_INVALID


def log_retention_candidate_before(
    candidates: Pointer[mut=False, UInt64, _],
    file_names: Pointer[mut=False, UInt8, _],
    left: Int,
    right: Int,
) -> Bool:
    var left_base = left * LOG_RETENTION_CANDIDATE_STRIDE
    var right_base = right * LOG_RETENTION_CANDIDATE_STRIDE
    var left_modified = candidates[unsafe_offset=left_base + 1]
    var right_modified = candidates[unsafe_offset=right_base + 1]
    if left_modified != right_modified:
        return left_modified < right_modified

    var left_start = Int(candidates[unsafe_offset=left_base + 3])
    var right_start = Int(candidates[unsafe_offset=right_base + 3])
    var left_length = Int(candidates[unsafe_offset=left_base + 4])
    var right_length = Int(candidates[unsafe_offset=right_base + 4])
    var common_length = min(left_length, right_length)
    for offset in range(common_length):
        var left_byte = file_names[unsafe_offset=left_start + offset]
        var right_byte = file_names[unsafe_offset=right_start + offset]
        if left_byte != right_byte:
            return left_byte < right_byte
    if left_length != right_length:
        return left_length < right_length
    return left < right


def log_retention_candidate_swap(
    indices: Pointer[mut=True, UInt64, _], left: Int, right: Int
) -> None:
    var selected = indices[unsafe_offset=left]
    indices[unsafe_offset=left] = indices[unsafe_offset=right]
    indices[unsafe_offset=right] = selected


def log_retention_candidate_sift_down(
    candidates: Pointer[mut=False, UInt64, _],
    file_names: Pointer[mut=False, UInt8, _],
    indices: Pointer[mut=True, UInt64, _],
    root_index: Int,
    end: Int,
) -> None:
    var root = root_index
    while True:
        var child = root * 2 + 1
        if child > end:
            break
        if child + 1 <= end and log_retention_candidate_before(
            candidates,
            file_names,
            Int(indices[unsafe_offset=child]),
            Int(indices[unsafe_offset=child + 1]),
        ):
            child += 1
        if not log_retention_candidate_before(
            candidates,
            file_names,
            Int(indices[unsafe_offset=root]),
            Int(indices[unsafe_offset=child]),
        ):
            break
        log_retention_candidate_swap(indices, root, child)
        root = child


def log_retention_candidate_heap_sort(
    candidates: Pointer[mut=False, UInt64, _],
    file_names: Pointer[mut=False, UInt8, _],
    indices: Pointer[mut=True, UInt64, _],
    count: Int,
) -> None:
    var start = count // 2
    while start > 0:
        start -= 1
        log_retention_candidate_sift_down(
            candidates, file_names, indices, start, count - 1
        )
    var end = count
    while end > 1:
        end -= 1
        log_retention_candidate_swap(indices, 0, end)
        log_retention_candidate_sift_down(
            candidates, file_names, indices, 0, end - 1
        )


# Candidate rows are [size, signed-order epoch, removable/unavailable flags, name offset, name length].
# Output holds selected candidate indices with a max-value sentinel, then sort indices.
@export("prodex_log_retention_candidates_v1")
def prodex_log_retention_candidates_v1(
    abi_version: Int64,
    operation: Int64,
    candidate_count: UInt64,
    candidates_address: UInt,
    file_names_address: UInt,
    file_names_length: UInt64,
    policy_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LOG_RETENTION_POLICY_ABI_VERSION:
        return LOG_THROUGHPUT_ABI
    if (
        (
            operation != LOG_RETENTION_CANDIDATES_EXPIRED
            and operation != LOG_RETENTION_CANDIDATES_OVER_BUDGET
        )
        or policy_address == 0
        or file_names_length > UInt64(0x7FFF_FFFF_FFFF_FFFF)
    ):
        return LOG_THROUGHPUT_INVALID
    if candidate_count > UInt64(0x7FFF_FFFF_FFFF_FFFF) // UInt64(10):
        return LOG_THROUGHPUT_INVALID
    if candidate_count > 0 and (
        candidates_address == 0
        or file_names_address == 0
        or output_address == 0
    ):
        return LOG_THROUGHPUT_INVALID
    if candidate_count == 0:
        return LOG_THROUGHPUT_OK

    var candidates = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(candidates_address)
    )
    var file_names = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(file_names_address)
    )
    var policy = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(policy_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var count = Int(candidate_count)
    for index in range(count):
        var base = index * LOG_RETENTION_CANDIDATE_STRIDE
        var flags = candidates[unsafe_offset=base + 2]
        var name_offset = candidates[unsafe_offset=base + 3]
        var name_length = candidates[unsafe_offset=base + 4]
        if operation == LOG_RETENTION_CANDIDATES_EXPIRED and flags > 1:
            return LOG_THROUGHPUT_INVALID
        if operation == LOG_RETENTION_CANDIDATES_OVER_BUDGET and flags > 3:
            return LOG_THROUGHPUT_INVALID
        if (
            name_length == 0
            or name_offset > file_names_length
            or name_length > file_names_length - name_offset
        ):
            return LOG_THROUGHPUT_INVALID
        output[unsafe_offset=index] = (
            UInt64(0) if operation
            == LOG_RETENTION_CANDIDATES_EXPIRED else LOG_RETENTION_UINT64_MAX
        )
        output[unsafe_offset=count + index] = UInt64(index)

    # Policy is [age cutoff, remaining count, max files, bytes, byte budget].
    if operation == LOG_RETENTION_CANDIDATES_EXPIRED:
        var oldest_allowed = policy[unsafe_offset=0]
        for index in range(count):
            var base = index * LOG_RETENTION_CANDIDATE_STRIDE
            var modified = candidates[unsafe_offset=base + 1]
            var removable = candidates[unsafe_offset=base + 2] == 1
            output[unsafe_offset=index] = UInt64(
                modified < oldest_allowed and removable
            )
        return LOG_THROUGHPUT_OK

    var remaining_count = policy[unsafe_offset=1]
    var max_files = policy[unsafe_offset=2]
    var total_bytes = policy[unsafe_offset=3]
    var total_budget = policy[unsafe_offset=4]
    var indices = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address) + count * 8
    )
    log_retention_candidate_heap_sort(candidates, file_names, indices, count)
    var selected_count = 0
    for position in range(count):
        if remaining_count <= max_files and total_bytes <= total_budget:
            break
        var index = Int(indices[unsafe_offset=position])
        var base = index * LOG_RETENTION_CANDIDATE_STRIDE
        var size = candidates[unsafe_offset=base]
        var flags = candidates[unsafe_offset=base + 2]
        var unavailable = flags & UInt64(2) != 0
        var removable = flags & UInt64(1) != 0
        if unavailable or not removable:
            continue
        output[unsafe_offset=selected_count] = UInt64(index)
        selected_count += 1
        if remaining_count > 0:
            remaining_count = remaining_count - 1
        if total_bytes > size:
            total_bytes = total_bytes - size
        else:
            total_bytes = 0
    for position in range(selected_count, count):
        output[unsafe_offset=position] = LOG_RETENTION_UINT64_MAX
    return LOG_THROUGHPUT_OK
