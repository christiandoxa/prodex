from std.memory import Pointer

comptime ARTIFACT_REF_ABI_VERSION: Int64 = 1
comptime ARTIFACT_REF_OK: Int64 = 0
comptime ARTIFACT_REF_INVALID: Int64 = 1
comptime ARTIFACT_REF_CAPACITY: Int64 = 3

comptime OP_ALIAS_VALID: Int64 = 0
comptime OP_ALIAS_DECLARATION: Int64 = 1
comptime OP_ALIAS_REFERENCE: Int64 = 2
comptime OP_REFERENCE: Int64 = 3
comptime OP_ID_VALID: Int64 = 4

comptime MAX_TOKEN_BYTES: Int64 = 1024 * 1024

def byte_at(address: UInt, index: Int64) -> UInt8:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))[unsafe_offset=index]

def trim_punctuation_byte(value: UInt8) -> Bool:
    return value == 34 or value == 39 or value == 96 or value == 58 or value == 59 or value == 46 or value == 44 or value == 33 or value == 63 or value == 40 or value == 91 or value == 123 or value == 60 or value == 41 or value == 93 or value == 125 or value == 62

def trim_bounds(address: UInt, length: Int64) -> Tuple[Int64, Int64]:
    var start: Int64 = 0
    var end = length
    while start < end and trim_punctuation_byte(byte_at(address, start)):
        start += 1
    while end > start and trim_punctuation_byte(byte_at(address, end - 1)):
        end -= 1
    return (start, end)

def literal_at[literal: StaticString](address: UInt, start: Int64, end: Int64) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or start + n > end:
        return False
    var wanted = literal.unsafe_ptr()
    for i in range(n):
        if byte_at(address, start + i) != wanted[unsafe_offset=i]:
            return False
    return True

def ascii_hex(value: UInt8) -> Bool:
    return value >= 48 and value <= 57 or value >= 65 and value <= 70 or value >= 97 and value <= 102

def ascii_digit(value: UInt8) -> Bool:
    return value >= 48 and value <= 57

def alias_end(address: UInt, start: Int64, end: Int64) -> Int64:
    if start >= end or byte_at(address, start) != 64:
        return -1
    var cursor = start + 1
    while cursor < end and ascii_digit(byte_at(address, cursor)):
        cursor += 1
    return cursor if cursor > start + 1 else -1

def canonical_reference_bounds(
    address: UInt, start: Int64, end: Int64
) -> Tuple[Int64, Int64, Int64]:
    var source_start = start
    var prefix_kind: Int64 = 0
    if literal_at["prodex-artifact:"](address, source_start, end):
        source_start += 16
    elif literal_at["psc2:"](address, source_start, end):
        source_start += 5
        prefix_kind = 4
    elif literal_at["psc:"](address, source_start, end):
        source_start += 4
        if literal_at["sc:"](address, source_start, end):
            prefix_kind = 1
        else:
            prefix_kind = 3

    if prefix_kind == 0:
        if literal_at["sc2:"](address, source_start, end):
            prefix_kind = 2
        elif literal_at["sc:"](address, source_start, end):
            prefix_kind = 1
        else:
            return (-1, -1, -1)

    var hex_start = source_start
    if prefix_kind == 1:
        hex_start += 3
    elif prefix_kind == 2:
        hex_start += 4

    var cursor = hex_start
    while cursor < end and ascii_hex(byte_at(address, cursor)):
        cursor += 1
    var required: Int64 = 64 if prefix_kind == 2 or prefix_kind == 4 else 16
    if cursor - hex_start != required:
        return (-1, -1, -1)
    return (prefix_kind, hex_start, cursor)

def write_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var n = Int64(literal.byte_length())
    if written[] + n > capacity:
        return False
    var source = literal.unsafe_ptr()
    for i in range(n):
        output[unsafe_offset=written[] + i] = source[unsafe_offset=i]
    written[] += n
    return True

def write_range(
    address: UInt,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or written[] + end - start > capacity:
        return False
    for i in range(end - start):
        output[unsafe_offset=written[] + i] = byte_at(address, start + i)
    written[] += end - start
    return True

def write_canonical_id(
    address: UInt,
    prefix_kind: Int64,
    hex_start: Int64,
    hex_end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if prefix_kind == 2 or prefix_kind == 4:
        if not write_literal["sc2:"](output, capacity, written):
            return False
    else:
        if not write_literal["sc:"](output, capacity, written):
            return False
    return write_range(address, hex_start, hex_end, output, capacity, written)

def parse_uint(address: UInt, start: Int64, end: Int64) -> Int64:
    if start >= end:
        return -1
    var value: Int64 = 0
    for i in range(start, end):
        var byte = byte_at(address, i)
        if not ascii_digit(byte):
            return -1
        var digit = Int64(byte) - 48
        if value > 0x7FFFFFFFFFFFFFFF // 10:
            return -1
        value = value * 10 + digit
        if value < 0:
            return -1
    return value

def parse_ranges(
    address: UInt,
    start: Int64,
    end: Int64,
    ranges: Pointer[mut=True, Int64, _],
    range_capacity: Int64,
    range_count: Pointer[mut=True, Int64, _],
) -> Int64:
    range_count[] = 0
    if start >= end:
        return ARTIFACT_REF_OK
    var cursor = start
    var lead = byte_at(address, cursor)
    if lead != 35 and lead != 58 and lead != 63:
        return ARTIFACT_REF_OK
    cursor += 1
    if literal_at["lines="](address, cursor, end):
        cursor += 6

    while cursor < end:
        var segment_end = cursor
        while segment_end < end and byte_at(address, segment_end) != 44:
            segment_end += 1
        var first = cursor
        if first < segment_end and (
            byte_at(address, first) == 76 or byte_at(address, first) == 108
        ):
            first += 1
        var dash: Int64 = -1
        for i in range(first, segment_end):
            if byte_at(address, i) == 45:
                dash = i
                break
        var left_end = dash if dash >= 0 else segment_end
        var right_start = dash + 1 if dash >= 0 else first
        if right_start < segment_end and (
            byte_at(address, right_start) == 76 or byte_at(address, right_start) == 108
        ):
            right_start += 1
        var line_start = parse_uint(address, first, left_end)
        var line_end = parse_uint(address, right_start, segment_end)
        if line_start > 0 and line_end >= line_start:
            if range_count[] >= range_capacity:
                return ARTIFACT_REF_CAPACITY
            ranges[unsafe_offset=range_count[] * 2] = line_start
            ranges[unsafe_offset=range_count[] * 2 + 1] = line_end
            range_count[] += 1
        cursor = segment_end + 1
    return ARTIFACT_REF_OK

@export("prodex_smart_context_artifact_ref_v1")
def prodex_smart_context_artifact_ref_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    ranges_address: UInt,
    range_capacity: Int64,
    range_count_address: UInt,
    meta_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != ARTIFACT_REF_ABI_VERSION
        or operation < OP_ALIAS_VALID or operation > OP_ID_VALID
        or length < 0 or length > MAX_TOKEN_BYTES
        or (length > 0 and address == 0)
        or meta_address == 0
    ):
        return ARTIFACT_REF_INVALID

    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(meta_address)
    )
    for i in range(4):
        meta[unsafe_offset=i] = -1

    var bounds = trim_bounds(address, length)
    var start = bounds[0]
    var end = bounds[1]

    if operation == OP_ALIAS_VALID:
        var alias = alias_end(address, 0, length)
        meta[0] = Int64(alias == length and alias >= 0)
        return ARTIFACT_REF_OK

    if operation == OP_ID_VALID:
        var ref_bounds = canonical_reference_bounds(address, 0, length)
        meta[0] = Int64(
            ref_bounds[0] >= 0
            and (ref_bounds[0] == 1 or ref_bounds[0] == 2)
            and ref_bounds[2] == length
        )
        return ARTIFACT_REF_OK

    if operation == OP_ALIAS_DECLARATION:
        var equals: Int64 = -1
        for i in range(start, end):
            if byte_at(address, i) == 61:
                equals = i
                break
        if equals < 0:
            return ARTIFACT_REF_OK
        var alias = alias_end(address, start, equals)
        if alias != equals:
            return ARTIFACT_REF_OK
        var ref_start = equals + 1
        var ref_end = end
        while ref_start < ref_end and trim_punctuation_byte(byte_at(address, ref_start)):
            ref_start += 1
        while ref_end > ref_start and trim_punctuation_byte(byte_at(address, ref_end - 1)):
            ref_end -= 1
        var ref_bounds = canonical_reference_bounds(address, ref_start, ref_end)
        if ref_bounds[0] < 0:
            return ARTIFACT_REF_OK
        if output_address == 0 or written_address == 0:
            return ARTIFACT_REF_INVALID
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = 0
        if not write_canonical_id(
            address, ref_bounds[0], ref_bounds[1], ref_bounds[2],
            output, output_capacity, written
        ):
            return ARTIFACT_REF_CAPACITY
        meta[0] = start
        meta[1] = equals
        meta[2] = 1
        return ARTIFACT_REF_OK

    if operation == OP_ALIAS_REFERENCE:
        var alias = alias_end(address, start, end)
        if alias < 0:
            return ARTIFACT_REF_OK
        if ranges_address == 0 or range_count_address == 0:
            return ARTIFACT_REF_INVALID
        var ranges = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(ranges_address))
        var range_count = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(range_count_address))
        var status = parse_ranges(address, alias, end, ranges, range_capacity, range_count)
        if status != ARTIFACT_REF_OK:
            return status
        meta[0] = start
        meta[1] = alias
        meta[2] = alias
        meta[3] = end
        return ARTIFACT_REF_OK

    var ref_bounds = canonical_reference_bounds(address, start, end)
    if ref_bounds[0] < 0:
        return ARTIFACT_REF_OK
    if output_address == 0 or written_address == 0 or ranges_address == 0 or range_count_address == 0:
        return ARTIFACT_REF_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    var ranges = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(ranges_address))
    var range_count = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(range_count_address))
    written[] = 0
    if not write_canonical_id(address, ref_bounds[0], ref_bounds[1], ref_bounds[2], output, output_capacity, written):
        return ARTIFACT_REF_CAPACITY
    var status = parse_ranges(address, ref_bounds[2], end, ranges, range_capacity, range_count)
    if status != ARTIFACT_REF_OK:
        return status
    meta[0] = start
    meta[1] = end
    meta[2] = 1
    return ARTIFACT_REF_OK
