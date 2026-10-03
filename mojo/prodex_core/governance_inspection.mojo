from std.memory import Pointer
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

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
        var minimum = governance_finding_minimum_classification(
            values[unsafe_offset=index]
        )
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
comptime GOVERNANCE_MAX_FINDINGS: Int64 = 256
comptime GOVERNANCE_MAX_TAGS: Int64 = 32
comptime GOVERNANCE_MAX_REASON_CODES: Int64 = 32


def governance_copy_label[
    label: StaticString
](output_address: UInt, output_capacity: Int64, written_address: UInt,) -> Bool:
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
            ok = governance_copy_label["public"](
                output_address, output_capacity, written_address
            )
        elif value == 1:
            ok = governance_copy_label["internal"](
                output_address, output_capacity, written_address
            )
        elif value == 2:
            ok = governance_copy_label["confidential"](
                output_address, output_capacity, written_address
            )
        elif value == 3:
            ok = governance_copy_label["restricted"](
                output_address, output_capacity, written_address
            )
        else:
            return GOVERNANCE_INSPECTION_INVALID
    else:
        if value == 0:
            ok = governance_copy_label["full"](
                output_address, output_capacity, written_address
            )
        elif value == 1:
            ok = governance_copy_label["partial"](
                output_address, output_capacity, written_address
            )
        elif value == 2:
            ok = governance_copy_label["unsupported"](
                output_address, output_capacity, written_address
            )
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
        GOVERNANCE_MAX_LOCATION_PATH_BYTES if kind
        == GOVERNANCE_TEXT_CONTENT_LOCATION else GOVERNANCE_MAX_TOKEN_BYTES
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
        and max_findings <= UInt64(GOVERNANCE_MAX_FINDINGS)
        and max_tags >= 1
        and max_tags <= UInt64(GOVERNANCE_MAX_TAGS)
        and max_reason_codes >= 1
        and max_reason_codes <= UInt64(GOVERNANCE_MAX_REASON_CODES)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(valid)
    return GOVERNANCE_INSPECTION_OK


comptime GOVERNANCE_INSPECTION_ORDER_ABI_VERSION: Int64 = 1
comptime GOVERNANCE_INSPECTION_ORDER_ABI: Int64 = 4


@fieldwise_init
struct GovernanceFindingOrderKey(Copyable):
    var field_path: ProdexRichStringView
    var start_byte: Int64
    var end_byte: Int64
    var kind: Int64
    var detector_id: ProdexRichStringView
    var confidence_basis_points: Int64


def governance_inspection_view_compare(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
) -> Int64:
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    var common = Int64(left.len)
    if right.len < left.len:
        common = Int64(right.len)
    for index in range(common):
        if left_bytes[unsafe_offset=index] < right_bytes[unsafe_offset=index]:
            return -1
        if left_bytes[unsafe_offset=index] > right_bytes[unsafe_offset=index]:
            return 1
    if left.len < right.len:
        return -1
    if left.len > right.len:
        return 1
    return 0


def governance_finding_order_before(
    keys: Pointer[mut=False, GovernanceFindingOrderKey, ImmUntrackedOrigin],
    left_index: Int64,
    right_index: Int64,
) -> Bool:
    var left = keys[unsafe_offset=left_index].copy()
    var right = keys[unsafe_offset=right_index].copy()
    var compared = governance_inspection_view_compare(
        left.field_path, right.field_path
    )
    if compared != 0:
        return compared < 0
    if left.start_byte != right.start_byte:
        return left.start_byte < right.start_byte
    if left.end_byte != right.end_byte:
        return left.end_byte < right.end_byte
    if left.kind != right.kind:
        return left.kind < right.kind
    compared = governance_inspection_view_compare(
        left.detector_id, right.detector_id
    )
    if compared != 0:
        return compared < 0
    return left.confidence_basis_points < right.confidence_basis_points


def governance_inspection_order_swap(
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    left: Int64,
    right: Int64,
) -> None:
    var value = indices[unsafe_offset=left]
    indices[unsafe_offset=left] = indices[unsafe_offset=right]
    indices[unsafe_offset=right] = value


def governance_finding_order_sift_down(
    keys: Pointer[mut=False, GovernanceFindingOrderKey, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    root_index: Int64,
    end: Int64,
) -> None:
    var root = root_index
    while True:
        var child = root * 2 + 1
        if child > end:
            break
        if child + 1 <= end and governance_finding_order_before(
            keys,
            indices[unsafe_offset=child],
            indices[unsafe_offset=child + 1],
        ):
            child += 1
        if not governance_finding_order_before(
            keys,
            indices[unsafe_offset=root],
            indices[unsafe_offset=child],
        ):
            break
        governance_inspection_order_swap(indices, root, child)
        root = child


def governance_finding_order_sort(
    keys: Pointer[mut=False, GovernanceFindingOrderKey, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    count: Int64,
) -> None:
    var start = count // 2
    while start > 0:
        start -= 1
        governance_finding_order_sift_down(keys, indices, start, count - 1)
    var end = count
    while end > 1:
        end -= 1
        governance_inspection_order_swap(indices, 0, end)
        governance_finding_order_sift_down(keys, indices, 0, end - 1)


def governance_view_order_sift_down(
    values: Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    root_index: Int64,
    end: Int64,
) -> None:
    var root = root_index
    while True:
        var child = root * 2 + 1
        if child > end:
            break
        if (
            child + 1 <= end
            and governance_inspection_view_compare(
                values[unsafe_offset=indices[unsafe_offset=child]],
                values[unsafe_offset=indices[unsafe_offset=child + 1]],
            )
            < 0
        ):
            child += 1
        if (
            governance_inspection_view_compare(
                values[unsafe_offset=indices[unsafe_offset=root]],
                values[unsafe_offset=indices[unsafe_offset=child]],
            )
            >= 0
        ):
            break
        governance_inspection_order_swap(indices, root, child)
        root = child


def governance_view_order_sort(
    values: Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    count: Int64,
) -> None:
    var start = count // 2
    while start > 0:
        start -= 1
        governance_view_order_sift_down(values, indices, start, count - 1)
    var end = count
    while end > 1:
        end -= 1
        governance_inspection_order_swap(indices, 0, end)
        governance_view_order_sift_down(values, indices, 0, end - 1)


def governance_view_order_deduplicate(
    values: Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin],
    indices: Pointer[mut=True, Int64, MutUntrackedOrigin],
    count: Int64,
) -> Int64:
    var written: Int64 = 0
    for index in range(count):
        var source_index = indices[unsafe_offset=index]
        if (
            written == 0
            or governance_inspection_view_compare(
                values[unsafe_offset=indices[unsafe_offset=written - 1]],
                values[unsafe_offset=source_index],
            )
            != 0
        ):
            indices[unsafe_offset=written] = source_index
            written += 1
    return written


def governance_order_views_valid(
    values: Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin],
    count: Int64,
    maximum_bytes: Int64,
) -> Bool:
    for index in range(count):
        if not rich_view_valid(
            values[unsafe_offset=index].copy(), maximum_bytes
        ):
            return False
    return True


@export("prodex_mojo_governance_inspection_order_v1")
def prodex_mojo_governance_inspection_order_v1(
    abi_version: Int64,
    findings_address: UInt,
    findings_count: Int64,
    tags_address: UInt,
    tags_count: Int64,
    reason_codes_address: UInt,
    reason_codes_count: Int64,
    finding_order_address: UInt,
    tag_order_address: UInt,
    tag_count_address: UInt,
    reason_code_order_address: UInt,
    reason_code_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != GOVERNANCE_INSPECTION_ORDER_ABI_VERSION:
        return GOVERNANCE_INSPECTION_ORDER_ABI
    if (
        findings_count < 0
        or tags_count < 0
        or reason_codes_count < 0
        or tag_count_address == 0
        or reason_code_count_address == 0
        or (
            findings_count > 0
            and (findings_address == 0 or finding_order_address == 0)
        )
        or (tags_count > 0 and (tags_address == 0 or tag_order_address == 0))
        or (
            reason_codes_count > 0
            and (reason_codes_address == 0 or reason_code_order_address == 0)
        )
    ):
        return GOVERNANCE_INSPECTION_INVALID
    if (
        findings_count > GOVERNANCE_MAX_FINDINGS
        or tags_count > GOVERNANCE_MAX_TAGS
        or reason_codes_count > GOVERNANCE_MAX_REASON_CODES
    ):
        return 2

    var findings = Pointer[
        mut=False, GovernanceFindingOrderKey, ImmUntrackedOrigin
    ](unsafe_from_address=Int(findings_address))
    var tags = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(tags_address)
    )
    var reason_codes = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(reason_codes_address))
    var finding_order = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(finding_order_address)
    )
    var tag_order = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(tag_order_address)
    )
    var reason_code_order = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(reason_code_order_address)
    )
    var tag_count_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(tag_count_address)
    )
    var reason_code_count_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(reason_code_count_address)
    )
    tag_count_output[] = 0
    reason_code_count_output[] = 0

    for index in range(findings_count):
        var key = findings[unsafe_offset=index].copy()
        if (
            not rich_view_valid(
                key.field_path, GOVERNANCE_MAX_LOCATION_PATH_BYTES
            )
            or not rich_view_valid(key.detector_id, GOVERNANCE_MAX_TOKEN_BYTES)
            or key.start_byte < 0
            or key.end_byte < key.start_byte
            or key.end_byte > 4294967295
            or governance_finding_minimum_classification(key.kind) < 0
            or key.confidence_basis_points < 0
            or key.confidence_basis_points > 10000
        ):
            return GOVERNANCE_INSPECTION_INVALID
        finding_order[unsafe_offset=index] = index
    if not governance_order_views_valid(
        tags, tags_count, GOVERNANCE_MAX_TOKEN_BYTES
    ):
        return GOVERNANCE_INSPECTION_INVALID
    if not governance_order_views_valid(
        reason_codes, reason_codes_count, GOVERNANCE_MAX_TOKEN_BYTES
    ):
        return GOVERNANCE_INSPECTION_INVALID

    if findings_count > 1:
        governance_finding_order_sort(findings, finding_order, findings_count)
    for index in range(tags_count):
        tag_order[unsafe_offset=index] = index
    if tags_count > 1:
        governance_view_order_sort(tags, tag_order, tags_count)
    tag_count_output[] = governance_view_order_deduplicate(
        tags, tag_order, tags_count
    )

    for index in range(reason_codes_count):
        reason_code_order[unsafe_offset=index] = index
    if reason_codes_count > 1:
        governance_view_order_sort(
            reason_codes, reason_code_order, reason_codes_count
        )
    reason_code_count_output[] = governance_view_order_deduplicate(
        reason_codes, reason_code_order, reason_codes_count
    )
    return GOVERNANCE_INSPECTION_OK
