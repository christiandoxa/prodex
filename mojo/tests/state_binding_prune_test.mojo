# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/state_binding_prune_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout
from state_policy import prodex_state_binding_prune_v1


def main() raises:
    var rank_storage = alloc(Layout[Int64](count=8))
    var output_storage = alloc(Layout[Int64](count=5))
    var ranks = rank_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    for index in range(8):
        ranks[unsafe_offset=index] = 0
    ranks[unsafe_offset=1] = 5
    ranks[unsafe_offset=3] = -1
    ranks[unsafe_offset=5] = 5
    ranks[unsafe_offset=6] = 1
    ranks[unsafe_offset=7] = -99
    var source = UInt(Int(ranks))
    var destination = UInt(Int(output))
    for index in range(5):
        output[unsafe_offset=index] = 71
    assert prodex_state_binding_prune_v1(1, 0, 4, 2, 2, destination, 1) == 5
    assert output[unsafe_offset=0] == 71
    assert prodex_state_binding_prune_v1(1, source, 4, 2, 2, destination, 5) == 0
    assert output[unsafe_offset=0] == 2
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 0
    assert output[unsafe_offset=3] == 2
    assert output[unsafe_offset=4] == 3
    dealloc(rank_storage^)
    dealloc(output_storage^)
