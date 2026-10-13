from std.memory import Pointer

comptime STATUS_RESOURCE_ABI_VERSION: Int64 = 1
comptime STATUS_RESOURCE_OK: Int64 = 0
comptime STATUS_RESOURCE_INVALID: Int64 = 1
comptime STATUS_RESOURCE_ABI: Int64 = 4
comptime STATUS_RESOURCE_PROCESS_CPU: Int64 = 0
comptime STATUS_RESOURCE_SYSTEM_CPU: Int64 = 1
comptime STATUS_RESOURCE_KIB_FIELD: Int64 = 2
comptime STATUS_RESOURCE_U64_FIELD: Int64 = 3
comptime STATUS_RESOURCE_NETWORK_QUEUES: Int64 = 4
comptime STATUS_RESOURCE_SOCKET_INODE: Int64 = 5
comptime STATUS_RESOURCE_MAX: Int64 = 9_223_372_036_854_775_807


def status_resource_ptr(address: UInt64) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))


def status_resource_space(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32


def status_resource_skip_space(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], length: Int64, index: Int64
) -> Int64:
    var cursor = index
    while cursor < length and status_resource_space(source[unsafe_offset=cursor]):
        cursor += 1
    return cursor


def status_resource_decimal(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], start: Int64, end: Int64
) -> Tuple[Bool, Int64]:
    if start < 0 or end <= start:
        return (False, 0)
    var value: Int64 = 0
    var cursor = start
    while cursor < end:
        var byte = source[unsafe_offset=cursor]
        if byte < 48 or byte > 57:
            return (False, 0)
        var digit = Int64(byte - 48)
        if value > (STATUS_RESOURCE_MAX - digit) // 10:
            return (False, 0)
        value = value * 10 + digit
        cursor += 1
    return (True, value)


def status_resource_token(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    length: Int64,
    cursor: Int64,
) -> Tuple[Bool, Int64, Int64, Int64]:
    var start = status_resource_skip_space(source, length, cursor)
    if start >= length:
        return (False, start, start, start)
    var end = start
    while end < length and not status_resource_space(source[unsafe_offset=end]):
        end += 1
    return (True, start, end, end)


def status_resource_process_cpu(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], length: Int64
) -> Tuple[Bool, Int64]:
    var close: Int64 = -1
    var cursor: Int64 = 0
    while cursor < length:
        if source[unsafe_offset=cursor] == 41:
            close = cursor
        cursor += 1
    if close < 0:
        return (False, 0)
    cursor = close + 1
    var field: Int64 = 0
    var first: Int64 = 0
    var second: Int64 = 0
    var first_present = False
    var second_present = False
    while True:
        var token = status_resource_token(source, length, cursor)
        if not token[0]:
            break
        cursor = token[3]
        if field == 11 or field == 12:
            var parsed = status_resource_decimal(source, token[1], token[2])
            if not parsed[0]:
                return (False, 0)
            if field == 11:
                first = parsed[1]
                first_present = True
            else:
                second = parsed[1]
                second_present = True
        field += 1
        if field > 12:
            break
    if not first_present or not second_present:
        return (False, 0)
    if first > STATUS_RESOURCE_MAX - second:
        return (True, STATUS_RESOURCE_MAX)
    return (True, first + second)


def status_resource_system_cpu(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], length: Int64
) -> Tuple[Bool, Int64]:
    var line_end: Int64 = 0
    while line_end < length and source[unsafe_offset=line_end] != 10:
        line_end += 1
    if line_end < 3 or source[unsafe_offset=0] != 99 or source[unsafe_offset=1] != 112 or source[unsafe_offset=2] != 117:
        return (False, 0)
    if line_end == 3 or not status_resource_space(source[unsafe_offset=3]):
        return (False, 0)
    var cursor: Int64 = 3
    var total: Int64 = 0
    var count: Int64 = 0
    while count < 8:
        var token = status_resource_token(source, line_end, cursor)
        if not token[0]:
            break
        var parsed = status_resource_decimal(source, token[1], token[2])
        if not parsed[0]:
            return (False, 0)
        if parsed[1] > STATUS_RESOURCE_MAX - total:
            total = STATUS_RESOURCE_MAX
        else:
            total += parsed[1]
        cursor = token[3]
        count += 1
    return (True, total)


def status_resource_key_matches(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
    key: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    key_length: Int64,
) -> Bool:
    if end - start != key_length or key_length <= 0:
        return False
    for index in range(key_length):
        if source[unsafe_offset=start + index] != key[unsafe_offset=index]:
            return False
    return True


def status_resource_field(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    length: Int64,
    key: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    key_length: Int64,
    multiply_kib: Bool,
) -> Tuple[Bool, Int64]:
    var line_start: Int64 = 0
    while line_start < length:
        var line_end = line_start
        while line_end < length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        var colon: Int64 = line_start
        while colon < line_end and source[unsafe_offset=colon] != 58:
            colon += 1
        if colon < line_end and status_resource_key_matches(source, line_start, colon, key, key_length):
            var value_start = status_resource_skip_space(source, line_end, colon + 1)
            var value_end = value_start
            if multiply_kib:
                while value_end < line_end and not status_resource_space(source[unsafe_offset=value_end]):
                    value_end += 1
            else:
                value_end = line_end
                while value_end > value_start and status_resource_space(source[unsafe_offset=value_end - 1]):
                    value_end -= 1
            var parsed = status_resource_decimal(source, value_start, value_end)
            if not parsed[0]:
                return (False, 0)
            if multiply_kib:
                if parsed[1] > STATUS_RESOURCE_MAX // 1024:
                    return (True, STATUS_RESOURCE_MAX)
                return (True, parsed[1] * 1024)
            return parsed
        if line_end >= length:
            break
        line_start = line_end + 1
    return (False, 0)


def status_resource_hex_digit(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 55)
    if value >= 97 and value <= 102:
        return Int64(value - 87)
    return -1


def status_resource_hex(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], start: Int64, end: Int64
) -> Tuple[Bool, Int64]:
    if start >= end:
        return (False, 0)
    var value: Int64 = 0
    for index in range(start, end):
        var digit = status_resource_hex_digit(source[unsafe_offset=index])
        if digit < 0:
            return (False, 0)
        if value > (STATUS_RESOURCE_MAX - digit) // 16:
            return (True, STATUS_RESOURCE_MAX)
        value = value * 16 + digit
    return (True, value)


def status_resource_contains(
    value: Int64,
    inodes: Pointer[mut=False, UInt64, ImmUntrackedOrigin],
    inode_count: Int64,
) -> Bool:
    if value < 0 or inode_count < 0:
        return False
    for index in range(inode_count):
        if inodes[unsafe_offset=index] == UInt64(value):
            return True
    return False


def status_resource_network(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    length: Int64,
    inodes: Pointer[mut=False, UInt64, ImmUntrackedOrigin],
    inode_count: Int64,
) -> Tuple[Bool, Int64, Int64]:
    var line_start: Int64 = 0
    var line_number: Int64 = 0
    var rx: Int64 = 0
    var tx: Int64 = 0
    while line_start < length:
        var line_end = line_start
        while line_end < length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        if line_number > 0:
            var cursor = line_start
            var field: Int64 = 0
            var queue_start: Int64 = -1
            var queue_end: Int64 = -1
            var inode: Int64 = -1
            while True:
                var token = status_resource_token(source, line_end, cursor)
                if not token[0]:
                    break
                cursor = token[3]
                if field == 4:
                    queue_start = token[1]
                    queue_end = token[2]
                elif field == 9:
                    var parsed = status_resource_decimal(source, token[1], token[2])
                    if parsed[0]:
                        inode = parsed[1]
                field += 1
            if inode >= 0 and queue_start >= 0 and status_resource_contains(inode, inodes, inode_count):
                var colon = queue_start
                while colon < queue_end and source[unsafe_offset=colon] != 58:
                    colon += 1
                if colon < queue_end:
                    var tx_value = status_resource_hex(source, queue_start, colon)
                    var rx_value = status_resource_hex(source, colon + 1, queue_end)
                    if tx_value[0] and rx_value[0]:
                        if rx_value[1] > STATUS_RESOURCE_MAX - rx:
                            rx = STATUS_RESOURCE_MAX
                        else:
                            rx += rx_value[1]
                        if tx_value[1] > STATUS_RESOURCE_MAX - tx:
                            tx = STATUS_RESOURCE_MAX
                        else:
                            tx += tx_value[1]
        line_number += 1
        if line_end >= length:
            break
        line_start = line_end + 1
    return (True, rx, tx)



def status_resource_socket_inode(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], length: Int64
) -> Tuple[Bool, Int64]:
    if length < 10 or source[unsafe_offset=0] != 115 or source[unsafe_offset=1] != 111 or source[unsafe_offset=2] != 99 or source[unsafe_offset=3] != 107 or source[unsafe_offset=4] != 101 or source[unsafe_offset=5] != 116 or source[unsafe_offset=6] != 58 or source[unsafe_offset=7] != 91 or source[unsafe_offset=length - 1] != 93:
        return (False, 0)
    return status_resource_decimal(source, 8, length - 1)


@export("prodex_status_resource_parser_v1")
def prodex_status_resource_parser_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt64,
    input_length: Int64,
    auxiliary_address: UInt64,
    auxiliary_length: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi_version != STATUS_RESOURCE_ABI_VERSION or input_length < 0 or auxiliary_length < 0 or output_address == 0:
        return STATUS_RESOURCE_INVALID
    if operation < STATUS_RESOURCE_PROCESS_CPU or operation > STATUS_RESOURCE_SOCKET_INODE:
        return STATUS_RESOURCE_INVALID
    if input_length > 0 and input_address == 0:
        return STATUS_RESOURCE_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    var source = status_resource_ptr(input_address)
    if operation == STATUS_RESOURCE_PROCESS_CPU:
        var result = status_resource_process_cpu(source, input_length)
        output[unsafe_offset=0] = Int64(result[0])
        output[unsafe_offset=1] = result[1]
    elif operation == STATUS_RESOURCE_SYSTEM_CPU:
        var result = status_resource_system_cpu(source, input_length)
        output[unsafe_offset=0] = Int64(result[0])
        output[unsafe_offset=1] = result[1]
    elif operation == STATUS_RESOURCE_KIB_FIELD or operation == STATUS_RESOURCE_U64_FIELD:
        if auxiliary_length == 0 or auxiliary_address == 0:
            return STATUS_RESOURCE_INVALID
        var key = status_resource_ptr(auxiliary_address)
        var result = status_resource_field(source, input_length, key, auxiliary_length, operation == STATUS_RESOURCE_KIB_FIELD)
        output[unsafe_offset=0] = Int64(result[0])
        output[unsafe_offset=1] = result[1]
    elif operation == STATUS_RESOURCE_NETWORK_QUEUES:
        if auxiliary_length < 0 or auxiliary_length > 1024 or auxiliary_length > 0 and auxiliary_address == 0:
            return STATUS_RESOURCE_INVALID
        var inodes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(auxiliary_address))
        var network = status_resource_network(source, input_length, inodes, auxiliary_length)
        output[unsafe_offset=0] = Int64(network[0])
        output[unsafe_offset=1] = network[1]
        output[unsafe_offset=2] = network[2]
    else:
        var result = status_resource_socket_inode(source, input_length)
        output[unsafe_offset=0] = Int64(result[0])
        output[unsafe_offset=1] = result[1]
    return STATUS_RESOURCE_OK
