from std.memory import Pointer

comptime OPERATIONAL_METRICS_ABI_VERSION: Int64 = 1
comptime OPERATIONAL_METRICS_STATUS_OK: Int64 = 0
comptime OPERATIONAL_METRICS_STATUS_INVALID: Int64 = 1
comptime OPERATIONAL_METRICS_STATUS_CAPACITY: Int64 = 2
comptime OPERATIONAL_METRICS_STATUS_ABI: Int64 = 4
comptime OPERATIONAL_METRICS_MAX_NAME_BYTES: Int64 = 128


def operational_metrics_name_ends_with(
    name: Pointer[mut=False, UInt8, _], name_length: Int64, suffix: StringSlice
) -> Bool:
    var suffix_length = Int64(suffix.byte_length())
    if name_length < suffix_length:
        return False
    var expected = suffix.unsafe_ptr()
    for offset in range(suffix_length):
        if name[unsafe_offset=name_length - suffix_length + offset] != expected[
            unsafe_offset=offset
        ]:
            return False
    return True


@export("prodex_mojo_operational_histogram_bounds_v1")
def prodex_mojo_operational_histogram_bounds_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPERATIONAL_METRICS_ABI_VERSION:
        return OPERATIONAL_METRICS_STATUS_ABI
    if (
        name_address == 0
        or name_length < 0
        or name_length > OPERATIONAL_METRICS_MAX_NAME_BYTES
        or output_address == 0
        or output_capacity < 0
        or output_count_address == 0
    ):
        return OPERATIONAL_METRICS_STATUS_INVALID

    var name = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(name_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )

    if operational_metrics_name_ends_with(name, name_length, StringSlice("_microseconds")):
        if output_capacity < 16:
            return OPERATIONAL_METRICS_STATUS_CAPACITY
        output[0] = 100
        output[1] = 250
        output[2] = 500
        output[3] = 1_000
        output[4] = 2_500
        output[5] = 5_000
        output[6] = 10_000
        output[7] = 25_000
        output[8] = 50_000
        output[9] = 100_000
        output[10] = 250_000
        output[11] = 500_000
        output[12] = 1_000_000
        output[13] = 5_000_000
        output[14] = 30_000_000
        output[15] = 120_000_000
        output_count[] = 16
    else:
        if output_capacity < 15:
            return OPERATIONAL_METRICS_STATUS_CAPACITY
        output[0] = 1
        output[1] = 2
        output[2] = 5
        output[3] = 10
        output[4] = 25
        output[5] = 50
        output[6] = 100
        output[7] = 250
        output[8] = 500
        output[9] = 1_000
        output[10] = 2_500
        output[11] = 5_000
        output[12] = 10_000
        output[13] = 30_000
        output[14] = 120_000
        output_count[] = 15
    return OPERATIONAL_METRICS_STATUS_OK


# All histogram accumulation decisions live here; Rust owns synchronization and storage.
def operational_histogram_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    var maximum = UInt64(18446744073709551615)
    if right > maximum - left:
        return maximum
    return left + right


@export("prodex_mojo_operational_histogram_observe_v1")
def prodex_mojo_operational_histogram_observe_v1(
    abi_version: Int64,
    observation: UInt64,
    bounds_address: UInt,
    counts_address: UInt,
    bucket_count: Int64,
    count_address: UInt,
    sum_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPERATIONAL_METRICS_ABI_VERSION:
        return OPERATIONAL_METRICS_STATUS_ABI
    if (
        bounds_address == 0
        or counts_address == 0
        or bucket_count < 0
        or bucket_count > 16
        or count_address == 0
        or sum_address == 0
    ):
        return OPERATIONAL_METRICS_STATUS_INVALID
    var bounds = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(bounds_address)
    )
    var counts = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(counts_address)
    )
    var count = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(count_address)
    )
    var total = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(sum_address)
    )
    count[] = operational_histogram_saturating_add(count[], 1)
    total[] = operational_histogram_saturating_add(total[], observation)
    for index in range(bucket_count):
        if observation <= bounds[index]:
            counts[index] = operational_histogram_saturating_add(counts[index], 1)
    return OPERATIONAL_METRICS_STATUS_OK
