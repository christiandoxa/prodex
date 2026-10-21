# Direct compiled workspace trust planning ABI and size-budget regression.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_overlay_policy import prodex_runtime_overlay_trust_entry_plan_v1


def main() raises:
    var a = StringSlice("/a")
    var b = StringSlice("/b")
    var c = StringSlice("/c")
    var names = [a, b, a, c, b]
    var lens = [Int64(10), Int64(20), Int64(10), Int64(30), Int64(20)]
    var records_store = alloc(Layout[UInt64](count=15))
    var output_store = alloc(Layout[Int64](count=5))
    var scratch_store = alloc(Layout[Int64](count=10))
    var written_store = alloc(Layout[Int64](count=1))
    var records = records_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var scratch = scratch_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    for index in range(5):
        records[unsafe_offset=index * 3] = UInt64(UInt(Int(names[index].unsafe_ptr())))
        records[unsafe_offset=index * 3 + 1] = UInt64(names[index].byte_length())
        records[unsafe_offset=index * 3 + 2] = UInt64(lens[index])
    var r = UInt(Int(records))
    var o = UInt(Int(output))
    var s = UInt(Int(scratch))
    var w = UInt(Int(written))
    assert prodex_runtime_overlay_trust_entry_plan_v1(
        1, r, 5, 42, o, 5, s, 10, w
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1

    assert prodex_runtime_overlay_trust_entry_plan_v1(
        1, r, 5, 10, o, 5, s, 10, w
    ) == 0
    assert written[unsafe_offset=0] == 1
    assert output[unsafe_offset=0] == 0
    written[unsafe_offset=0] = 79
    assert prodex_runtime_overlay_trust_entry_plan_v1(
        2, r, 5, 42, o, 5, s, 10, w
    ) == 4
    assert prodex_runtime_overlay_trust_entry_plan_v1(
        1, r, 5, 42, o, 5, s, 9, w
    ) == 1
    assert written[unsafe_offset=0] == 79
    records[unsafe_offset=1] = UInt64(1_048_577)
    assert prodex_runtime_overlay_trust_entry_plan_v1(
        1, r, 5, 42, o, 5, s, 10, w
    ) == 1
    assert written[unsafe_offset=0] == 79
    records[unsafe_offset=1] = UInt64(a.byte_length())
    assert prodex_runtime_overlay_trust_entry_plan_v1(
        1, 0, 0, 42, 0, 0, 0, 0, w
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(written_store^)
    dealloc(scratch_store^)
    dealloc(output_store^)
    dealloc(records_store^)
