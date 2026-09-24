from std.memory import Pointer

comptime TELEMETRY_LABEL_ABI_VERSION: Int64 = 1
comptime TELEMETRY_LABEL_STATUS_OK: Int64 = 0
comptime TELEMETRY_LABEL_STATUS_INVALID: Int64 = 1
comptime TELEMETRY_LABEL_STATUS_ABI: Int64 = 4
comptime TELEMETRY_LABEL_MAX_BYTES: Int64 = 128


def telemetry_label_ascii_graphic(
    value: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    if length <= 0 or length > TELEMETRY_LABEL_MAX_BYTES:
        return False
    for index in range(length):
        var byte = Int64(value[unsafe_offset=index])
        if byte < 33 or byte > 126:
            return False
    return True


def telemetry_label_key_contains(
    key: Pointer[mut=False, UInt8, _], key_length: Int64, literal: StringSlice
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if key_length < literal_length:
        return False
    var expected = literal.unsafe_ptr()
    for start in range(key_length - literal_length + 1):
        var matches = True
        for offset in range(literal_length):
            var byte = Int64(key[unsafe_offset=start + offset])
            if byte >= 65 and byte <= 90:
                byte += 32
            if byte == 45 or byte == 46:
                byte = 95
            if byte != Int64(expected[unsafe_offset=offset]):
                matches = False
                break
        if matches:
            return True
    return False


def telemetry_label_key_invalid(
    key: Pointer[mut=False, UInt8, _], key_length: Int64
) -> Bool:
    if not telemetry_label_ascii_graphic(key, key_length):
        return True
    return (
        telemetry_label_key_contains(key, key_length, StringSlice("tenant_id"))
        or telemetry_label_key_contains(key, key_length, StringSlice("user_id"))
        or telemetry_label_key_contains(key, key_length, StringSlice("principal_id"))
        or telemetry_label_key_contains(key, key_length, StringSlice("request_id"))
        or telemetry_label_key_contains(key, key_length, StringSlice("call_id"))
        or telemetry_label_key_contains(key, key_length, StringSlice("virtual_key"))
        or telemetry_label_key_contains(key, key_length, StringSlice("api_key"))
        or telemetry_label_key_contains(key, key_length, StringSlice("prompt"))
    )


def telemetry_label_hex_byte(value: Int64) -> Bool:
    return (
        (value >= 48 and value <= 57)
        or (value >= 65 and value <= 70)
        or (value >= 97 and value <= 102)
    )


def telemetry_label_value_is_uuid(
    value: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    if length != 36:
        return False
    for index in range(length):
        var byte = Int64(value[unsafe_offset=index])
        if index == 8 or index == 13 or index == 18 or index == 23:
            if byte != 45:
                return False
        elif not telemetry_label_hex_byte(byte):
            return False
    return True


def telemetry_label_value_is_hex_id(
    value: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    if length != 32:
        return False
    for index in range(length):
        if not telemetry_label_hex_byte(Int64(value[unsafe_offset=index])):
            return False
    return True


@export("prodex_mojo_observability_metric_label_validate_v1")
def prodex_mojo_observability_metric_label_validate_v1(
    abi_version: Int64,
    key_address: UInt,
    key_length: Int64,
    value_address: UInt,
    value_length: Int64,
    output_tag_address: UInt,
) abi("C") -> Int64:
    if abi_version != TELEMETRY_LABEL_ABI_VERSION:
        return TELEMETRY_LABEL_STATUS_ABI
    if (
        key_address == 0
        or key_length < 0
        or value_address == 0
        or value_length < 0
        or output_tag_address == 0
    ):
        return TELEMETRY_LABEL_STATUS_INVALID
    var key = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(key_address)
    )
    var value = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(value_address)
    )
    var output_tag = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_tag_address)
    )
    if telemetry_label_key_invalid(key, key_length):
        output_tag[] = 1
    elif not telemetry_label_ascii_graphic(value, value_length) or telemetry_label_value_is_uuid(
        value, value_length
    ) or telemetry_label_value_is_hex_id(value, value_length):
        output_tag[] = 2
    else:
        output_tag[] = 0
    return TELEMETRY_LABEL_STATUS_OK
