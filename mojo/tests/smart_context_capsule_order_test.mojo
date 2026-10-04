# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/smart_context_capsule_order_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout

from smart_context_capsule_order import (
    SMART_CONTEXT_CAPSULE_ORDER_STATUS_ABI,
    SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID,
    SMART_CONTEXT_CAPSULE_ORDER_STATUS_OK,
    prodex_mojo_smart_context_capsule_order_v1,
)


def test_capsule_order_is_stable_and_reads_borrowed_ids() raises:
    var ids_source = String("highlowsamesameéacost-firstreq-zreq-a")
    var source_bytes = ids_source.unsafe_ptr()
    var ids_allocation = alloc(Layout[UInt8](count=38))
    var ids = ids_allocation.unsafe_ptr()
    for offset in range(38):
        ids[unsafe_offset=offset] = source_bytes[unsafe_offset=offset]

    var id_addresses_allocation = alloc(Layout[UInt64](count=9))
    var id_addresses = id_addresses_allocation.unsafe_ptr()
    for index, offset in [
        (0, 0),
        (1, 4),
        (2, 7),
        (3, 11),
        (4, 15),
        (5, 17),
        (6, 18),
        (7, 28),
        (8, 33),
    ]:
        id_addresses[unsafe_offset=index] = UInt64(
            Int(ids.unsafe_offset(offset))
        )

    var id_lengths_allocation = alloc(Layout[Int64](count=9))
    var id_lengths = id_lengths_allocation.unsafe_ptr()
    for index, length in [
        (0, 4),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 2),
        (5, 1),
        (6, 10),
        (7, 5),
        (8, 5),
    ]:
        id_lengths[unsafe_offset=index] = Int64(length)

    var relevances_allocation = alloc(Layout[Float32](count=9))
    var relevances = relevances_allocation.unsafe_ptr()
    for index, value in [
        (0, 0.9),
        (1, 0.1),
        (2, 0.4),
        (3, 0.4),
        (4, 0.4),
        (5, 0.4),
        (6, 0.4),
        (7, 0.0),
        (8, 0.0),
    ]:
        relevances[unsafe_offset=index] = Float32(value)

    var token_costs_allocation = alloc(Layout[UInt64](count=9))
    var token_costs = token_costs_allocation.unsafe_ptr()
    for index, value in [
        (0, 20),
        (1, 30),
        (2, 5),
        (3, 5),
        (4, 5),
        (5, 5),
        (6, 3),
        (7, 1),
        (8, 1),
    ]:
        token_costs[unsafe_offset=index] = UInt64(value)

    var required_allocation = alloc(Layout[Int64](count=9))
    var required = required_allocation.unsafe_ptr()
    for index, value in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 1),
        (8, 1),
    ]:
        required[unsafe_offset=index] = Int64(value)

    var permutation_allocation = alloc(Layout[Int64](count=9))
    var permutation = permutation_allocation.unsafe_ptr()
    var scratch_allocation = alloc(Layout[Int64](count=9))
    var scratch = scratch_allocation.unsafe_ptr()
    for index in range(9):
        permutation[unsafe_offset=index] = -1
        scratch[unsafe_offset=index] = 0

    var status = prodex_mojo_smart_context_capsule_order_v1(
        1,
        UInt(Int(id_addresses)),
        UInt(Int(id_lengths)),
        UInt(Int(relevances)),
        UInt(Int(token_costs)),
        UInt(Int(required)),
        UInt(Int(permutation)),
        UInt(Int(scratch)),
        9,
    )

    assert status == SMART_CONTEXT_CAPSULE_ORDER_STATUS_OK
    for index, expected in [
        (0, 8),
        (1, 7),
        (2, 0),
        (3, 6),
        (4, 5),
        (5, 2),
        (6, 3),
        (7, 4),
        (8, 1),
    ]:
        assert permutation[unsafe_offset=index] == Int64(expected)

    # The ABI reads borrowed bytes and leaves every source byte unchanged.
    for offset in range(38):
        assert ids[unsafe_offset=offset] == source_bytes[unsafe_offset=offset]
    for index, offset in [
        (0, 0),
        (1, 4),
        (2, 7),
        (3, 11),
        (4, 15),
        (5, 17),
        (6, 18),
        (7, 28),
        (8, 33),
    ]:
        assert id_addresses[unsafe_offset=index] == UInt64(
            Int(ids.unsafe_offset(offset))
        )
    for index, expected in [
        (0, 4),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 2),
        (5, 1),
        (6, 10),
        (7, 5),
        (8, 5),
    ]:
        assert id_lengths[unsafe_offset=index] == Int64(expected)
    for index, expected in [
        (0, 0.9),
        (1, 0.1),
        (2, 0.4),
        (3, 0.4),
        (4, 0.4),
        (5, 0.4),
        (6, 0.4),
        (7, 0.0),
        (8, 0.0),
    ]:
        assert relevances[unsafe_offset=index] == Float32(expected)
    for index, expected in [
        (0, 20),
        (1, 30),
        (2, 5),
        (3, 5),
        (4, 5),
        (5, 5),
        (6, 3),
        (7, 1),
        (8, 1),
    ]:
        assert token_costs[unsafe_offset=index] == UInt64(expected)
    for index, expected in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 1),
        (8, 1),
    ]:
        assert required[unsafe_offset=index] == Int64(expected)

    dealloc(scratch_allocation^)
    dealloc(permutation_allocation^)
    dealloc(required_allocation^)
    dealloc(token_costs_allocation^)
    dealloc(relevances_allocation^)
    dealloc(id_lengths_allocation^)
    dealloc(id_addresses_allocation^)
    dealloc(ids_allocation^)


def test_capsule_order_validates_each_borrowed_id_span() raises:
    var id_addresses_allocation = alloc(Layout[UInt64](count=1))
    var id_addresses = id_addresses_allocation.unsafe_ptr()
    id_addresses[unsafe_offset=0] = 0
    var id_lengths_allocation = alloc(Layout[Int64](count=1))
    var id_lengths = id_lengths_allocation.unsafe_ptr()
    id_lengths[unsafe_offset=0] = -1
    var relevances_allocation = alloc(Layout[Float32](count=1))
    var relevances = relevances_allocation.unsafe_ptr()
    relevances[unsafe_offset=0] = 0.0
    var token_costs_allocation = alloc(Layout[UInt64](count=1))
    var token_costs = token_costs_allocation.unsafe_ptr()
    token_costs[unsafe_offset=0] = 1
    var required_allocation = alloc(Layout[Int64](count=1))
    var required = required_allocation.unsafe_ptr()
    required[unsafe_offset=0] = 0
    var output_allocation = alloc(Layout[Int64](count=1))
    var output = output_allocation.unsafe_ptr()
    output[unsafe_offset=0] = -1
    var scratch_allocation = alloc(Layout[Int64](count=1))
    var scratch = scratch_allocation.unsafe_ptr()
    scratch[unsafe_offset=0] = 0

    assert (
        prodex_mojo_smart_context_capsule_order_v1(
            1,
            0,
            0,
            0,
            0,
            0,
            UInt(Int(output)),
            UInt(Int(scratch)),
            65_538,
        )
        == SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID
    )
    assert output[unsafe_offset=0] == -1
    assert scratch[unsafe_offset=0] == 0

    assert (
        prodex_mojo_smart_context_capsule_order_v1(
            2,
            UInt(Int(id_addresses)),
            UInt(Int(id_lengths)),
            UInt(Int(relevances)),
            UInt(Int(token_costs)),
            UInt(Int(required)),
            UInt(Int(output)),
            UInt(Int(scratch)),
            1,
        )
        == SMART_CONTEXT_CAPSULE_ORDER_STATUS_ABI
    )
    assert (
        prodex_mojo_smart_context_capsule_order_v1(
            1,
            UInt(Int(id_addresses)),
            UInt(Int(id_lengths)),
            UInt(Int(relevances)),
            UInt(Int(token_costs)),
            UInt(Int(required)),
            UInt(Int(output)),
            UInt(Int(scratch)),
            1,
        )
        == SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID
    )
    assert output[unsafe_offset=0] == -1
    assert scratch[unsafe_offset=0] == 0
    assert id_addresses[unsafe_offset=0] == 0
    assert id_lengths[unsafe_offset=0] == -1

    id_lengths[unsafe_offset=0] = 1
    assert (
        prodex_mojo_smart_context_capsule_order_v1(
            1,
            UInt(Int(id_addresses)),
            UInt(Int(id_lengths)),
            UInt(Int(relevances)),
            UInt(Int(token_costs)),
            UInt(Int(required)),
            UInt(Int(output)),
            UInt(Int(scratch)),
            1,
        )
        == SMART_CONTEXT_CAPSULE_ORDER_STATUS_INVALID
    )
    assert output[unsafe_offset=0] == -1
    assert scratch[unsafe_offset=0] == 0

    id_lengths[unsafe_offset=0] = 0
    assert (
        prodex_mojo_smart_context_capsule_order_v1(
            1,
            UInt(Int(id_addresses)),
            UInt(Int(id_lengths)),
            UInt(Int(relevances)),
            UInt(Int(token_costs)),
            UInt(Int(required)),
            UInt(Int(output)),
            UInt(Int(scratch)),
            1,
        )
        == SMART_CONTEXT_CAPSULE_ORDER_STATUS_OK
    )
    assert output[unsafe_offset=0] == 0
    assert id_addresses[unsafe_offset=0] == 0
    assert id_lengths[unsafe_offset=0] == 0

    dealloc(scratch_allocation^)
    dealloc(output_allocation^)
    dealloc(required_allocation^)
    dealloc(token_costs_allocation^)
    dealloc(relevances_allocation^)
    dealloc(id_lengths_allocation^)
    dealloc(id_addresses_allocation^)


def main() raises:
    test_capsule_order_is_stable_and_reads_borrowed_ids()
    test_capsule_order_validates_each_borrowed_id_span()
