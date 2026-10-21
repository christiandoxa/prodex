# Offline compiled legacy Caveman SHA-256 paragraph selector ABI.
from std.memory.alloc import alloc, dealloc, Layout
from optional_tools_policy import prodex_optional_tool_legacy_instruction_paragraphs_v1


def main() raises:
    var digests_store = alloc(Layout[UInt8](count=96))
    var indices_store = alloc(Layout[Int64](count=3))
    var written_store = alloc(Layout[Int64](count=1))
    var digests = digests_store.unsafe_ptr()
    var indices = indices_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    var historical = [
        UInt8(0xa0), 0x7e, 0x8d, 0x51, 0x67, 0xe4, 0x54, 0xf7,
        0x63, 0x7e, 0xb0, 0xe3, 0x52, 0x30, 0xa8, 0x49,
        0x87, 0xc5, 0x37, 0xae, 0x98, 0xd6, 0xf7, 0x95,
        0xd2, 0x46, 0x33, 0x8b, 0x65, 0x28, 0x10, 0xf1,
    ]
    for index in range(96):
        digests[unsafe_offset=index] = 0
    for index in range(32):
        digests[unsafe_offset=32 + index] = historical[index]
        digests[unsafe_offset=64 + index] = historical[index]
    digests[unsafe_offset=95] = 0xf0

    var source = UInt(Int(digests))
    var output = UInt(Int(indices))
    var written_address = UInt(Int(written))
    assert prodex_optional_tool_legacy_instruction_paragraphs_v1(
        1, source, 3, output, 3, written_address
    ) == 0
    assert written[unsafe_offset=0] == 2
    assert indices[unsafe_offset=0] == 0
    assert indices[unsafe_offset=1] == 2

    written[unsafe_offset=0] = 77
    assert prodex_optional_tool_legacy_instruction_paragraphs_v1(
        2, source, 3, output, 3, written_address
    ) == 4
    assert prodex_optional_tool_legacy_instruction_paragraphs_v1(
        1, source, 3, output, 2, written_address
    ) == 3
    assert prodex_optional_tool_legacy_instruction_paragraphs_v1(
        1, 0, 3, output, 3, written_address
    ) == 1
    assert written[unsafe_offset=0] == 77
    assert prodex_optional_tool_legacy_instruction_paragraphs_v1(
        1, 0, 0, 0, 0, written_address
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(written_store^)
    dealloc(indices_store^)
    dealloc(digests_store^)
