from std.memory.alloc import alloc, dealloc, Layout
from runtime_broker_admin import prodex_runtime_broker_snapshot_query_v1


def check(text: StringSlice, issue: UInt64, after: UInt64, limit: UInt64) raises:
    var store = alloc(Layout[UInt64](count=3))
    var output = store.unsafe_ptr()
    var target = UInt(Int(output))
    assert prodex_runtime_broker_snapshot_query_v1(
        1, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()), 64, target
    ) == 0
    assert output[unsafe_offset=0] == issue
    assert output[unsafe_offset=1] == after
    assert output[unsafe_offset=2] == limit
    for index in range(3):
        output[unsafe_offset=index] = 77
    assert prodex_runtime_broker_snapshot_query_v1(2, 0, 0, 64, target) == 4
    assert prodex_runtime_broker_snapshot_query_v1(1, 0, 1, 64, target) == 1
    assert prodex_runtime_broker_snapshot_query_v1(1, 0, 0, 0, target) == 1
    assert prodex_runtime_broker_snapshot_query_v1(1, 0, 0, 64, target + 1) == 1
    for index in range(3):
        assert output[unsafe_offset=index] == 77
    dealloc(store^)


def main() raises:
    check("/snapshot", 0, 0, 64)
    check("/snapshot?after=+0007&limit=12&limit=4", 0, 7, 4)
    check("/snapshot?after=18446744073709551615", 0, 18_446_744_073_709_551_615, 64)
    check("/snapshot?unknown=a=b&&", 0, 0, 64)
    check("/snapshot?unknown", 1, 0, 0)
    check("/snapshot?after=18446744073709551616", 2, 0, 0)
    check("/snapshot?after=%31", 2, 0, 0)
    check("/snapshot?limit=1=2", 3, 0, 0)
    check("/snapshot?limit=0&after=nope", 4, 0, 0)
