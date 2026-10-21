from std.memory.alloc import alloc, dealloc, Layout
from super_expose_run_policy import prodex_super_run_event_push_v1, prodex_super_run_events_page_v1


def main() raises:
    var sequences_store = alloc(Layout[UInt64](count=300))
    var indices_store = alloc(Layout[Int64](count=300))
    var output_store = alloc(Layout[UInt64](count=2))
    var sequences = sequences_store.unsafe_ptr()
    var indices = indices_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var s = UInt(Int(sequences))
    var i = UInt(Int(indices))
    var o = UInt(Int(output))
    for index in range(300):
        sequences[unsafe_offset=index] = UInt64(index)
        indices[unsafe_offset=index] = 77
    assert prodex_super_run_event_push_v1(1, 256, 18446744073709551615, o) == 0
    assert output[unsafe_offset=0] == 18446744073709551615 and output[unsafe_offset=1] == 1
    assert prodex_super_run_event_push_v1(1, 300, 10, o) == 0
    assert output[unsafe_offset=0] == 11 and output[unsafe_offset=1] == 45
    assert prodex_super_run_events_page_v1(1, s, 300, 0, 300, 18446744073709551615, i, 300, o) == 0
    assert output[unsafe_offset=0] == 256 and output[unsafe_offset=1] == 0
    assert indices[unsafe_offset=0] == 1 and indices[unsafe_offset=255] == 256
    sequences[unsafe_offset=0] = 9
    sequences[unsafe_offset=1] = 1
    sequences[unsafe_offset=2] = 9
    sequences[unsafe_offset=3] = 18446744073709551615
    assert prodex_super_run_events_page_v1(1, s, 4, 8, 18446744073709551615, 3, i, 300, o) == 0
    assert output[unsafe_offset=0] == 3 and output[unsafe_offset=1] == 0
    assert indices[unsafe_offset=0] == 0 and indices[unsafe_offset=1] == 2 and indices[unsafe_offset=2] == 3
    assert prodex_super_run_events_page_v1(1, 0, 0, 98, 100, 0, 0, 0, o) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 1
    assert prodex_super_run_events_page_v1(1, s, 4, 18446744073709551615, 0, 9, i, 300, o) == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 0
    output[unsafe_offset=0] = 71
    output[unsafe_offset=1] = 71
    indices[unsafe_offset=0] = 77
    assert prodex_super_run_event_push_v1(2, 0, 0, o) == 4
    assert prodex_super_run_event_push_v1(1, -1, 0, o) == 1
    assert prodex_super_run_events_page_v1(2, s, 4, 0, 0, 9, i, 300, o) == 4
    assert prodex_super_run_events_page_v1(1, s+1, 4, 0, 0, 9, i, 300, o) == 1
    assert prodex_super_run_events_page_v1(1, s, 4, 0, 0, 9, i, 3, o) == 3
    assert prodex_super_run_events_page_v1(1, s, -1, 0, 0, 9, i, 300, o) == 1
    assert output[unsafe_offset=0] == 71 and output[unsafe_offset=1] == 71 and indices[unsafe_offset=0] == 77
    dealloc(output_store^)
    dealloc(indices_store^)
    dealloc(sequences_store^)
