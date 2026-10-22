from std.memory.alloc import alloc, dealloc, Layout
from runtime_noncompact_loop import prodex_runtime_noncompact_next_action_v1


def main() raises:
    var field_storage = alloc(Layout[UInt64](count=12))
    var result_storage = alloc(Layout[Int64](count=1))
    var fields = field_storage.unsafe_ptr()
    var result = result_storage.unsafe_ptr()
    var source = UInt(Int(fields))
    var target = UInt(Int(result))
    for index in range(12):
        fields[unsafe_offset=index] = 0
    fields[unsafe_offset=1] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 0
    fields[unsafe_offset=3] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 1
    fields[unsafe_offset=2] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 0
    fields[unsafe_offset=0] = 1
    fields[unsafe_offset=4] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 2
    fields[unsafe_offset=4] = 0
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 3
    fields[unsafe_offset=0] = 3
    fields[unsafe_offset=7] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 4
    fields[unsafe_offset=7] = 0
    fields[unsafe_offset=9] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 5
    fields[unsafe_offset=10] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 6
    fields[unsafe_offset=8] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 7
    fields[unsafe_offset=0] = 4
    fields[unsafe_offset=11] = 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 4
    fields[unsafe_offset=11] = 0
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 0
    assert result[] == 7
    result[] = 83
    assert prodex_runtime_noncompact_next_action_v1(2, source, 12, target) == 4
    assert prodex_runtime_noncompact_next_action_v1(1, source, 11, target) == 1
    assert prodex_runtime_noncompact_next_action_v1(1, 0, 12, target) == 1
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, 0) == 1
    fields[unsafe_offset=11] = 2
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 1
    fields[unsafe_offset=11] = 0
    fields[unsafe_offset=0] = 5
    assert prodex_runtime_noncompact_next_action_v1(1, source, 12, target) == 1
    assert result[] == 83
    dealloc(field_storage^)
    dealloc(result_storage^)
