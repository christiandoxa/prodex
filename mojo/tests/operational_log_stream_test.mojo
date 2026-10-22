from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from operational_log_stream import prodex_mojo_operational_log_stream_v1
from log_load_classification import log_load_event_is_routine


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def main() raises:
    var fs = alloc(Layout[ProdexRichStringView](count=6))
    var os = alloc(Layout[Int64](count=3))
    var ms = alloc(Layout[Int64](count=2))
    var fields = fs.unsafe_ptr()
    var output = os.unsafe_ptr()
    var metadata = ms.unsafe_ptr()
    fields[unsafe_offset=0] = view("switched")
    fields[unsafe_offset=1] = view("true")
    fields[unsafe_offset=2] = view("path")
    fields[unsafe_offset=3] = view("https://example.com/responses?fake=1")
    fields[unsafe_offset=4] = view("ignored")
    fields[unsafe_offset=5] = view("fake")
    var data = UInt(Int(fields))
    var target = UInt(Int(output))
    var meta = UInt(Int(metadata))
    var event = view("profile_commit")
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 0
    assert metadata[unsafe_offset=0] == 4 and metadata[unsafe_offset=1] == 0
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 0 and output[unsafe_offset=2] == 0
    fields[unsafe_offset=1] = view("TRUE")
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 0
    assert metadata[unsafe_offset=0] == 0 and metadata[unsafe_offset=1] == 0
    event = view("profile_inflight_saturated")
    assert log_load_event_is_routine(event)
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 0
    assert metadata[unsafe_offset=0] == 16 and metadata[unsafe_offset=1] == 1
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 2 and output[unsafe_offset=2] == 0
    event = view("runtime_proxy_queue_overloaded")
    assert not log_load_event_is_routine(event)
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 0
    assert metadata[unsafe_offset=0] == 9 and metadata[unsafe_offset=1] == 0
    event = view("stream_payload")
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 0
    assert metadata[unsafe_offset=0] == 0 and metadata[unsafe_offset=1] == 0
    output[unsafe_offset=0] = 73
    metadata[unsafe_offset=0] = 83
    assert prodex_mojo_operational_log_stream_v1(2, event.ptr, Int64(event.len), data, 3, target, 3, meta) == 4
    assert prodex_mojo_operational_log_stream_v1(1, event.ptr, Int64(event.len), data, 3, target, 2, meta) == 2
    assert prodex_mojo_operational_log_stream_v1(1, 0, 1, data, 3, target, 3, meta) == 1
    assert output[unsafe_offset=0] == 73 and metadata[unsafe_offset=0] == 83
    dealloc(fs^)
    dealloc(os^)
    dealloc(ms^)
