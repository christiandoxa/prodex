# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_quota_duration_test.mojo`.
from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from provider_quota_duration import prodex_provider_quota_duration_plan_v1, prodex_provider_quota_duration_finish_v1


def main() raises:
    var text = String("\u20031 . 5h500ms\u3000")
    var number_storage = alloc(Layout[UInt8](count=6))
    var row_storage = alloc(Layout[Int64](count=6))
    var plan_storage = alloc(Layout[Int64](count=3))
    var numbers = number_storage.unsafe_ptr()
    var rows = row_storage.unsafe_ptr()
    var plan = plan_storage.unsafe_ptr()
    var number_address = UInt(Int(numbers))
    var rows_address = UInt(Int(rows))
    var plan_address = UInt(Int(plan))
    plan[unsafe_offset=0] = -1
    assert prodex_provider_quota_duration_plan_v1(1, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()), number_address, 4, rows_address, 2, plan_address) == 3
    assert plan[unsafe_offset=0] == -1
    assert prodex_provider_quota_duration_plan_v1(1, UInt(Int(text.unsafe_ptr())), Int64(text.byte_length()), number_address, 6, rows_address, 2, plan_address) == 0
    assert plan[unsafe_offset=0] == 1 and plan[unsafe_offset=1] == 2 and plan[unsafe_offset=2] == 6
    assert rows[unsafe_offset=0] == 0 and rows[unsafe_offset=1] == 3 and rows[unsafe_offset=2] == 3
    assert rows[unsafe_offset=3] == 3 and rows[unsafe_offset=4] == 6 and rows[unsafe_offset=5] == 1
    var value_storage = alloc(Layout[Float64](count=6))
    var values = value_storage.unsafe_ptr()
    var tags = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(values))
    tags[unsafe_offset=0] = 3
    tags[unsafe_offset=1] = 1
    values[unsafe_offset=2] = 1.5
    tags[unsafe_offset=3] = 1
    tags[unsafe_offset=4] = 1
    values[unsafe_offset=5] = 500.0
    assert prodex_provider_quota_duration_finish_v1(1, UInt(Int(values)), 2, plan_address, 2) == 0
    assert plan[unsafe_offset=0] == 1 and plan[unsafe_offset=1] == 5401
    dealloc(number_storage^)
    dealloc(row_storage^)
    dealloc(plan_storage^)
    dealloc(value_storage^)
