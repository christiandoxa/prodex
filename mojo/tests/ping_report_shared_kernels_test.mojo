from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from ping_protocol import ping_status_label_view, ping_summary_counts
from rich_text import rich_view_ptr


def main() raises:
    var label = ping_status_label_view(2, False)
    var expected = StringSlice("UNEXPECTED_RESPONSE")
    assert label.len == UInt(expected.byte_length())
    for index in range(Int64(label.len)):
        assert rich_view_ptr(label)[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    label = ping_status_label_view(9, True)
    expected = StringSlice("exhausted")
    assert label.len == UInt(expected.byte_length())
    for index in range(Int64(label.len)):
        assert rich_view_ptr(label)[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    var storage = alloc(Layout[Int64](count=240))
    var statuses = storage.unsafe_ptr()
    for index in range(240):
        statuses[unsafe_offset=index] = 93
    for index in range(15):
        statuses[unsafe_offset=index * 16] = Int64(index)
    var counts = ping_summary_counts(statuses, 15, 16)
    assert counts.valid and counts.total == 15 and counts.healthy == 1 and counts.exhausted == 1
    assert counts.auth_failures == 1 and counts.temporary_failures == 5 and counts.other_failures == 7 and counts.pool_usable
    statuses[unsafe_offset=16] = 15
    counts = ping_summary_counts(statuses, 15, 16)
    assert not counts.valid
    counts = ping_summary_counts(statuses, 0, 16)
    assert counts.valid and counts.total == 0 and not counts.pool_usable
    dealloc(storage^)
