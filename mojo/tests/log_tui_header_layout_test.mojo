# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_tui_header_layout_test.mojo`.
from std.collections import List
from log_semantics import prodex_mojo_log_tui_header_layout_v1


def test_header_and_atomic_quota_layout() raises:
    var output = List[UInt64](length=4, fill=0)
    var address = UInt(Int(output.unsafe_ptr()))
    assert prodex_mojo_log_tui_header_layout_v1(1, 0, 80, 10, 9, 1, address) == 0
    assert output == [UInt64(2), UInt64(78), UInt64(67), UInt64(55)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 0, 11, 10, 9, 1, address) == 0
    assert output == [UInt64(0), UInt64(9), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 0, 20, 12, 1, 1, address) == 0
    assert output == [UInt64(2), UInt64(18), UInt64(15), UInt64(1)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 1, 23, 10, 20, 1, address) == 0
    assert output == [UInt64(3), UInt64(3), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 1, 22, 10, 20, 1, address) == 0
    assert output == [UInt64(1), UInt64(0), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 1, 30, 10, 20, 1, address) == 0
    assert output == [UInt64(2), UInt64(0), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 2, 80, 40, 8, 0, address) == 0
    assert output == [UInt64(0), UInt64(30), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 2, 20, 18, 10, 0, address) == 0
    assert output == [UInt64(0), UInt64(0), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(2, 0, 80, 0, 0, 0, address) == 4
    assert output == [UInt64(0), UInt64(0), UInt64(0), UInt64(0)]
    assert prodex_mojo_log_tui_header_layout_v1(1, 1, 80, 0, 8, 0, address) == 1
    assert output == [UInt64(0), UInt64(0), UInt64(0), UInt64(0)]


def main() raises:
    test_header_and_atomic_quota_layout()
