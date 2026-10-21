from std.memory.alloc import alloc, dealloc, Layout
from shared_attachment_policy import prodex_shared_image_tag_paths_v1


def main() raises:
    var output_store = alloc(Layout[UInt64](count=4))
    var metadata_store = alloc(Layout[Int64](count=1))
    var output = output_store.unsafe_ptr()
    var required = metadata_store.unsafe_ptr()
    var target = UInt(Int(output))
    var metadata = UInt(Int(required))
    var text = StringSlice("<image path=\"one\"> <image path=\"two\">")
    var source = UInt(Int(text.unsafe_ptr()))
    var length = Int64(text.byte_length())
    for index in range(4):
        output[unsafe_offset=index] = 77
    required[] = 91
    assert prodex_shared_image_tag_paths_v1(2, 0, source, length, target, 2, metadata) == 4
    assert required[] == 91 and output[] == 77
    assert prodex_shared_image_tag_paths_v1(1, 2, source, length, target, 2, metadata) == 1
    assert required[] == 91 and output[] == 77
    assert prodex_shared_image_tag_paths_v1(1, 0, source, length, target + 1, 2, metadata) == 1
    assert required[] == 91 and output[] == 77
    assert prodex_shared_image_tag_paths_v1(1, 0, source, length, target, 1, metadata) == 3
    assert required[] == 2
    for index in range(4):
        assert output[unsafe_offset=index] == 77
    assert prodex_shared_image_tag_paths_v1(1, 0, source, length, target, 2, metadata) == 0
    assert output[unsafe_offset=0] == 13 and output[unsafe_offset=1] == 16
    assert output[unsafe_offset=2] == 32 and output[unsafe_offset=3] == 35
    var nested = StringSlice("<image path=\"outer\" <image path=\"inner\">>")
    assert prodex_shared_image_tag_paths_v1(
        1, 0, UInt(Int(nested.unsafe_ptr())), Int64(nested.byte_length()), target, 2, metadata
    ) == 0
    assert required[] == 1
    assert prodex_shared_image_tag_paths_v1(
        1, 1, UInt(Int(nested.unsafe_ptr())), Int64(nested.byte_length()), target, 2, metadata
    ) == 0
    assert required[] == 2
    assert prodex_shared_image_tag_paths_v1(1, 0, 0, 0, 0, 0, metadata) == 0
    assert required[] == 0
    dealloc(metadata_store^)
    dealloc(output_store^)
