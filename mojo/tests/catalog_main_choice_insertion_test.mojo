# Direct main catalog insertion policy with first-match position and ABI rejection.
from std.memory.alloc import alloc, dealloc, Layout
from rich_catalog import prodex_mojo_rich_catalog_insertion_index_v1


def main() raises:
    var tags_storage = alloc(Layout[Int64](count=5))
    var output_storage = alloc(Layout[Int64](count=1))
    var tags = tags_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    tags[unsafe_offset=0] = 1
    tags[unsafe_offset=1] = 4
    tags[unsafe_offset=2] = 0
    tags[unsafe_offset=3] = 4
    tags[unsafe_offset=4] = 1
    var source = UInt(Int(tags))
    var result = UInt(Int(output))
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 0, source, 5, result,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 1, source, 5, result,
    ) == 0
    assert output[unsafe_offset=0] == 3

    output[unsafe_offset=0] = 79
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        5, 0, source, 5, result,
    ) == 4
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, -1, source, 5, result,
    ) == 1
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 0, 0, 5, result,
    ) == 1
    tags[unsafe_offset=4] = 99
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 0, source, 5, result,
    ) == 1
    assert output[unsafe_offset=0] == 79

    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 0, 0, 0, result,
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_mojo_rich_catalog_insertion_index_v1(
        6, 1, 0, 0, result,
    ) == 0
    assert output[unsafe_offset=0] == 0
    dealloc(output_storage^)
    dealloc(tags_storage^)
