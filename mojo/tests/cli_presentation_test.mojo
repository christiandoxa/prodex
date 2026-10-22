from std.memory.alloc import alloc, dealloc, Layout
from cli_presentation import prodex_cli_presentation_text_v1, prodex_cli_command_runtime_v1


def check_text(operation: Int64, kind: Int64, expected_length: Int64, expected_hash: UInt64):
    var output_storage = alloc(Layout[UInt8](count=5000))
    var written_storage = alloc(Layout[Int64](count=1))
    var output = output_storage.unsafe_ptr()
    var written = written_storage.unsafe_ptr()
    var target = UInt64(Int(output))
    var length = UInt64(Int(written))
    assert prodex_cli_presentation_text_v1(1, operation, kind, 1, 0, 0, length) == 0
    assert written[] == expected_length
    assert prodex_cli_presentation_text_v1(1, operation, kind, 0, target, 5000, length) == 0
    assert written[] == expected_length
    var hash = UInt64(0xcbf29ce484222325)
    for index in range(expected_length):
        hash = (hash ^ UInt64(output[unsafe_offset=index])) * UInt64(0x100000001b3)
    assert hash == expected_hash
    output[] = 95
    written[] = 71
    assert prodex_cli_presentation_text_v1(1, operation, kind, 0, target, expected_length - 1, length) == 2
    assert output[] == 95 and written[] == 71
    dealloc(output_storage^)
    dealloc(written_storage^)


def main() raises:
    check_text(0, 0, 426, UInt64(0x82460174480663e0))
    check_text(0, 1, 827, UInt64(0x542f61f5b923e4ff))
    check_text(0, 2, 724, UInt64(0xd5f3c28c01183558))
    check_text(0, 3, 1239, UInt64(0x681bf5372c6943af))
    check_text(0, 4, 511, UInt64(0xb3a64c323850321a))
    check_text(0, 5, 980, UInt64(0x248df9759a8e94d3))
    check_text(0, 6, 4483, UInt64(0x9901b75cb564c7d0))
    check_text(0, 7, 230, UInt64(0xfe4dbd8f55073010))
    check_text(0, 8, 355, UInt64(0xc11a6d0aafde817a))
    check_text(1, 0, 7, UInt64(0x8add349e618ba10e))
    assert prodex_cli_command_runtime_v1(1, 0) == 0
    check_text(1, 1, 3, UInt64(0x4c52b0193dcce634))
    assert prodex_cli_command_runtime_v1(1, 1) == 0
    check_text(1, 2, 7, UInt64(0x2a2e8a5afcc8d89a))
    assert prodex_cli_command_runtime_v1(1, 2) == 0
    check_text(1, 3, 4, UInt64(0xf5d6a8c57ab94f05))
    assert prodex_cli_command_runtime_v1(1, 3) == 0
    check_text(1, 4, 6, UInt64(0xc4d5696d6cc12c2f))
    assert prodex_cli_command_runtime_v1(1, 4) == 0
    check_text(1, 5, 3, UInt64(0x125073191daf5431))
    assert prodex_cli_command_runtime_v1(1, 5) == 0
    check_text(1, 6, 7, UInt64(0x3ca2c49553a457d7))
    assert prodex_cli_command_runtime_v1(1, 6) == 0
    check_text(1, 7, 6, UInt64(0xeda3b2552abf2c52))
    assert prodex_cli_command_runtime_v1(1, 7) == 0
    check_text(1, 8, 5, UInt64(0x03c75db6e18f29d2))
    assert prodex_cli_command_runtime_v1(1, 8) == 0
    check_text(1, 9, 6, UInt64(0x2deee7c162e1558b))
    assert prodex_cli_command_runtime_v1(1, 9) == 0
    check_text(1, 10, 6, UInt64(0xa0ad68312a3d0454))
    assert prodex_cli_command_runtime_v1(1, 10) == 0
    check_text(1, 11, 5, UInt64(0x5a60ed143b2fd02b))
    assert prodex_cli_command_runtime_v1(1, 11) == 0
    check_text(1, 12, 6, UInt64(0xbccb6b0bfcbf6bb5))
    assert prodex_cli_command_runtime_v1(1, 12) == 0
    check_text(1, 13, 4, UInt64(0xbf30e00dc53307a9))
    assert prodex_cli_command_runtime_v1(1, 13) == 0
    check_text(1, 14, 3, UInt64(0x89b2a81960c55a4a))
    assert prodex_cli_command_runtime_v1(1, 14) == 1
    check_text(1, 15, 5, UInt64(0xd0d708b40e957634))
    assert prodex_cli_command_runtime_v1(1, 15) == 1
    check_text(1, 16, 7, UInt64(0x9d2717bfe2cfa019))
    assert prodex_cli_command_runtime_v1(1, 16) == 1
    check_text(1, 17, 12, UInt64(0xfd3bf0ff9553d74b))
    assert prodex_cli_command_runtime_v1(1, 17) == 1
    check_text(1, 18, 16, UInt64(0x262fbe6a37be7c49))
    assert prodex_cli_command_runtime_v1(1, 18) == 1
    check_text(1, 19, 18, UInt64(0x9dd11c693511ca3c))
    assert prodex_cli_command_runtime_v1(1, 19) == 0
    check_text(1, 20, 16, UInt64(0xcd2f0c0f538c958f))
    assert prodex_cli_command_runtime_v1(1, 20) == 0
    assert prodex_cli_command_runtime_v1(2, 0) == -2
    assert prodex_cli_command_runtime_v1(1, -1) == -1
    assert prodex_cli_command_runtime_v1(1, 21) == -1
    var output_storage = alloc(Layout[UInt8](count=32))
    var written_storage = alloc(Layout[Int64](count=1))
    var output = output_storage.unsafe_ptr()
    var written = written_storage.unsafe_ptr()
    var target = UInt64(Int(output))
    var length = UInt64(Int(written))
    output[] = 93
    written[] = 77
    assert prodex_cli_presentation_text_v1(2, 0, 0, 0, target, 32, length) == 4
    assert prodex_cli_presentation_text_v1(1, 2, 0, 0, target, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 0, 9, 0, target, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 21, 0, target, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, -1, 0, target, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 2, target, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 1, target, 0, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 0, 0, 32, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 0, target, -1, length) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 0, target, 32, 0) == 1
    assert prodex_cli_presentation_text_v1(1, 1, 0, 0, target, 32, length + 1) == 1
    assert output[] == 93 and written[] == 77
    dealloc(output_storage^)
    dealloc(written_storage^)
