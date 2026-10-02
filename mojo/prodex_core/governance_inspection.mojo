from std.memory import Pointer

comptime GOVERNANCE_INSPECTION_ABI_VERSION: Int64 = 6
comptime GOVERNANCE_INSPECTION_OK: Int64 = 0
comptime GOVERNANCE_INSPECTION_INVALID: Int64 = 1
comptime GOVERNANCE_INSPECTION_ABI: Int64 = 4

def governance_finding_minimum_classification(kind: Int64) -> Int64:
    if (kind >= 0 and kind <= 3) or kind == 11:
        return 2
    if kind >= 4 and kind <= 10:
        return 3
    return -1

@export("prodex_mojo_governance_finding_classification_v1")
def prodex_mojo_governance_finding_classification_v1(
    abi_version: Int64,
    mode: Int64,
    values_address: UInt,
    value_count: Int64,
    classification: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != GOVERNANCE_INSPECTION_ABI_VERSION:
        return GOVERNANCE_INSPECTION_ABI
    if mode < 0 or mode > 1 or value_count < 0 or value_count > 256:
        return GOVERNANCE_INSPECTION_INVALID
    if output_address == 0 or (value_count > 0 and values_address == 0):
        return GOVERNANCE_INSPECTION_INVALID

    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0

    if mode == 0:
        if value_count != 1:
            return GOVERNANCE_INSPECTION_INVALID
        var minimum = governance_finding_minimum_classification(values[0])
        if minimum < 0:
            return GOVERNANCE_INSPECTION_INVALID
        output[] = minimum
        return GOVERNANCE_INSPECTION_OK

    if classification < 0 or classification > 3:
        return GOVERNANCE_INSPECTION_INVALID
    for index in range(value_count):
        var minimum = governance_finding_minimum_classification(values[unsafe_offset=index])
        if minimum < 0:
            return GOVERNANCE_INSPECTION_INVALID
        if classification < minimum:
            output[] = 1
            return GOVERNANCE_INSPECTION_OK
    return GOVERNANCE_INSPECTION_OK

comptime GOVERNANCE_LABEL_CLASSIFICATION: Int64 = 0
comptime GOVERNANCE_LABEL_COVERAGE: Int64 = 1
comptime GOVERNANCE_TEXT_CONTENT_LOCATION: Int64 = 0
comptime GOVERNANCE_TEXT_TOKEN: Int64 = 1
comptime GOVERNANCE_MAX_TOKEN_BYTES: Int64 = 128
comptime GOVERNANCE_MAX_LOCATION_PATH_BYTES: Int64 = 256

def governance_copy_label[label: StaticString](
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Bool:
    var n = Int64(label.byte_length())
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = n
    if output_capacity < n:
        return False
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var source = label.unsafe_ptr()
    for index in range(n):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    return True

@export("prodex_mojo_governance_label_v1")
def prodex_mojo_governance_label_v1(
    abi_version: Int64,
    kind: Int64,
    value: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != GOVERNANCE_INSPECTION_ABI_VERSION
        or kind < GOVERNANCE_LABEL_CLASSIFICATION
        or kind > GOVERNANCE_LABEL_COVERAGE
        or value < 0
        or output_capacity < 0
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return GOVERNANCE_INSPECTION_INVALID

    var ok = False
    if kind == GOVERNANCE_LABEL_CLASSIFICATION:
        if value == 0:
            ok = governance_copy_label["public"](output_address, output_capacity, written_address)
        elif value == 1:
            ok = governance_copy_label["internal"](output_address, output_capacity, written_address)
        elif value == 2:
            ok = governance_copy_label["confidential"](output_address, output_capacity, written_address)
        elif value == 3:
            ok = governance_copy_label["restricted"](output_address, output_capacity, written_address)
        else:
            return GOVERNANCE_INSPECTION_INVALID
    else:
        if value == 0:
            ok = governance_copy_label["full"](output_address, output_capacity, written_address)
        elif value == 1:
            ok = governance_copy_label["partial"](output_address, output_capacity, written_address)
        elif value == 2:
            ok = governance_copy_label["unsupported"](output_address, output_capacity, written_address)
        else:
            return GOVERNANCE_INSPECTION_INVALID
    return GOVERNANCE_INSPECTION_OK if ok else 2

@export("prodex_mojo_governance_coverage_combine_v1")
def prodex_mojo_governance_coverage_combine_v1(
    abi_version: Int64,
    left: Int64,
    right: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != GOVERNANCE_INSPECTION_ABI_VERSION
        or left < 0
        or left > 2
        or right < 0
        or right > 2
        or output_address == 0
    ):
        return GOVERNANCE_INSPECTION_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if left == 0 and right == 0:
        output[] = 0
    elif left == 2 and right == 2:
        output[] = 2
    else:
        output[] = 1
    return GOVERNANCE_INSPECTION_OK

def governance_ascii_alphanumeric(byte: UInt8) -> Bool:
    return (
        (byte >= UInt8(48) and byte <= UInt8(57))
        or (byte >= UInt8(65) and byte <= UInt8(90))
        or (byte >= UInt8(97) and byte <= UInt8(122))
    )

def governance_text_allowed(kind: Int64, byte: UInt8) -> Bool:
    if governance_ascii_alphanumeric(byte):
        return True
    if kind == GOVERNANCE_TEXT_CONTENT_LOCATION:
        return (
            byte == UInt8(36)
            or byte == UInt8(46)
            or byte == UInt8(95)
            or byte == UInt8(45)
            or byte == UInt8(42)
            or byte == UInt8(91)
            or byte == UInt8(93)
        )
    return (
        byte == UInt8(46)
        or byte == UInt8(95)
        or byte == UInt8(45)
        or byte == UInt8(58)
        or byte == UInt8(47)
    )

@export("prodex_mojo_governance_text_valid_v1")
def prodex_mojo_governance_text_valid_v1(
    abi_version: Int64,
    kind: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != GOVERNANCE_INSPECTION_ABI_VERSION
        or kind < GOVERNANCE_TEXT_CONTENT_LOCATION
        or kind > GOVERNANCE_TEXT_TOKEN
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return GOVERNANCE_INSPECTION_INVALID
    var limit = (
        GOVERNANCE_MAX_LOCATION_PATH_BYTES
        if kind == GOVERNANCE_TEXT_CONTENT_LOCATION
        else GOVERNANCE_MAX_TOKEN_BYTES
    )
    var valid = length > 0 and length <= limit
    if valid:
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(address)
        )
        for index in range(length):
            if not governance_text_allowed(kind, source[unsafe_offset=index]):
                valid = False
                break
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(valid)
    return GOVERNANCE_INSPECTION_OK

@export("prodex_mojo_governance_limits_valid_v1")
def prodex_mojo_governance_limits_valid_v1(
    abi_version: Int64,
    max_detectors: UInt64,
    max_findings: UInt64,
    max_tags: UInt64,
    max_reason_codes: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != GOVERNANCE_INSPECTION_ABI_VERSION or output_address == 0:
        return GOVERNANCE_INSPECTION_INVALID
    var valid = (
        max_detectors >= 1
        and max_detectors <= 8
        and max_findings >= 1
        and max_findings <= 256
        and max_tags >= 1
        and max_tags <= 32
        and max_reason_codes >= 1
        and max_reason_codes <= 32
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(valid)
    return GOVERNANCE_INSPECTION_OK
