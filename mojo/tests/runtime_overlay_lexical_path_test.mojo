# Direct compiled lexical path component stack tests.
# Tags: prefix=0, root=1, current=2, normal=3, parent=4.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_overlay_policy import prodex_runtime_overlay_lexical_path_v1


def main() raises:
    var tags_store = alloc(Layout[Int64](count=8))
    var output_store = alloc(Layout[Int64](count=8))
    var written_store = alloc(Layout[Int64](count=1))
    var tags = tags_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    var p = UInt(Int(tags))
    var o = UInt(Int(output))
    var w = UInt(Int(written))

    # /a/b/../c/.. => /a
    var source = [Int64(1), 3, 3, 4, 3, 4]
    for index in range(6):
        tags[unsafe_offset=index] = source[index]
    assert prodex_runtime_overlay_lexical_path_v1(1, p, 6, o, 8, w) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1

    # ../.. /a/.. => ../..
    tags[unsafe_offset=0] = 4
    tags[unsafe_offset=1] = 4
    tags[unsafe_offset=2] = 3
    tags[unsafe_offset=3] = 4
    assert prodex_runtime_overlay_lexical_path_v1(1, p, 4, o, 8, w) == 0
    assert written[unsafe_offset=0] == 2
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1

    # Bad ABI and bad tag leave existing output unchanged.
    written[unsafe_offset=0] = 77
    assert prodex_runtime_overlay_lexical_path_v1(2, p, 4, o, 8, w) == 4
    tags[unsafe_offset=3] = 9
    assert prodex_runtime_overlay_lexical_path_v1(1, p, 4, o, 8, w) == 1
    assert written[unsafe_offset=0] == 77
    tags[unsafe_offset=3] = 4
    assert prodex_runtime_overlay_lexical_path_v1(1, 0, 0, 0, 0, w) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(written_store^)
    dealloc(output_store^)
    dealloc(tags_store^)
