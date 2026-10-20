# Mojo duration normalization direct ABI regression.
from std.memory.alloc import alloc, dealloc, Layout
from quota import prodex_quota_window_duration_seconds_v1


def main() raises:
    var storage = alloc(Layout[Int64](count=2))
    var result = storage.unsafe_ptr()
    var dest = UInt(Int(result))
    assert prodex_quota_window_duration_seconds_v1(1, 0, 0, 1, 300, dest) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == 18000
    assert prodex_quota_window_duration_seconds_v1(1, 1, -7, 1, 300, dest) == 0
    assert result[unsafe_offset=0] == 1
    assert result[unsafe_offset=1] == -7
    assert prodex_quota_window_duration_seconds_v1(1, 0, 0, 1, 153722867280912931, dest) == 0
    assert result[unsafe_offset=0] == 0
    assert result[unsafe_offset=1] == 0
    result[unsafe_offset=0] = 99
    result[unsafe_offset=1] = 88
    assert prodex_quota_window_duration_seconds_v1(2, 0, 0, 1, 300, dest) == 4
    assert prodex_quota_window_duration_seconds_v1(1, 2, 0, 1, 300, dest) == 1
    assert result[unsafe_offset=0] == 99
    assert result[unsafe_offset=1] == 88
    dealloc(storage^)
