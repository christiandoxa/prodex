from std.memory.alloc import alloc, dealloc, Layout
from profile_export_diagnostics import prodex_profile_export_diagnostic_v1


def check(issue: Int64, expected_version: Int64, received: Int64, iterations: Int64, argon2: Int64, expected: StringSlice) raises:
    var storage = alloc(Layout[UInt8](count=256))
    var result_storage = alloc(Layout[Int64](count=1))
    var output = storage.unsafe_ptr()
    var written = result_storage.unsafe_ptr()
    var target = UInt(Int(output))
    var result = UInt(Int(written))
    assert prodex_profile_export_diagnostic_v1(1, issue, expected_version, received, iterations, argon2, target, 256, result) == 0
    assert written[] == Int64(expected.byte_length())
    for index in range(written[]):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    for index in range(256):
        output[unsafe_offset=index] = 73
    written[] = 83
    assert prodex_profile_export_diagnostic_v1(2, issue, expected_version, received, iterations, argon2, target, 256, result) == 4
    assert prodex_profile_export_diagnostic_v1(1, issue, expected_version, received, iterations, argon2, target, 1, result) == 3
    assert prodex_profile_export_diagnostic_v1(1, 5, expected_version, received, iterations, argon2, target, 256, result) == 1
    assert prodex_profile_export_diagnostic_v1(1, issue, -2, received, iterations, argon2, target, 256, result) == 1
    for index in range(256):
        assert output[unsafe_offset=index] == 73
    assert written[] == 83
    dealloc(storage^)
    dealloc(result_storage^)


def main() raises:
    check(6, 2, 77, 23, 99, "profile export PBKDF2 iteration count 23 is outside safe range 50000..=2000000")
    check(7, 2, 77, 23, 99, "unsupported profile export Argon2id version 99")
    check(8, 2, 77, 23, 99, "profile export Argon2id memory cost is outside safe range")
    check(9, 2, 77, 23, 99, "profile export Argon2id iteration count is outside safe range")
    check(10, 2, 77, 23, 99, "profile export Argon2id parallelism is outside safe range")
    check(11, 2, 77, 23, 99, "unsupported profile export format")
    check(12, 2, 77, 23, 99, "unsupported profile export version 77 (supported: 1, 2)")
    check(13, 2, 77, 23, 99, "unsupported profile export cipher")
    check(14, 2, 77, 23, 99, "unsupported profile export KDF")
    check(15, 2, 77, 23, 99, "invalid encrypted export salt encoding")
    check(16, 2, 77, 23, 99, "invalid encrypted export salt length")
    check(17, 2, 77, 23, 99, "invalid encrypted export nonce encoding")
    check(18, 2, 77, 23, 99, "invalid encrypted export nonce length")
    check(19, 2, 77, 23, 99, "invalid encrypted export payload encoding")
    check(20, 2, 77, 23, 99, "invalid encrypted export payload length")
    check(21, 2, 77, 23, 99, "invalid encrypted export payload length")
    check(22, 2, 77, 23, 99, "profile export envelope kind requires version 2 (received 77)")
    check(22, -1, 77, -1, -1, "invalid profile export envelope version")
    check(6, -1, 0, -1, -1, "profile export PBKDF2 iteration count 0 is outside safe range 50000..=2000000")
    check(7, -1, 0, -1, -1, "unsupported profile export Argon2id version 0")
    check(12, -1, 4294967295, -1, -1, "unsupported profile export version 4294967295 (supported: 1, 2)")
