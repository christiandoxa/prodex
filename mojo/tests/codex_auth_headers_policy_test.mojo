# Direct compiled Codex version candidate and scalar ASCII User-Agent ABI.
from std.memory.alloc import alloc, dealloc, Layout
from codex_auth_headers import (
    prodex_codex_auth_probe_version_span_v1,
    prodex_codex_auth_user_agent_ascii_v1,
)


def main() raises:
    var candidate = StringSlice("codex-cli 0.123.4 5.6.7,")
    var span_store = alloc(Layout[Int64](count=2))
    var span = span_store.unsafe_ptr()
    var span_address = UInt(Int(span))
    assert prodex_codex_auth_probe_version_span_v1(
        1, UInt(Int(candidate.unsafe_ptr())),
        Int64(candidate.byte_length()), span_address
    ) == 0
    var bytes = candidate.unsafe_ptr()
    assert span[unsafe_offset=0] >= 0
    assert bytes[unsafe_offset=span[unsafe_offset=0]] == 53
    assert bytes[unsafe_offset=span[unsafe_offset=1] - 1] == 55

    var unsupported = StringSlice("codex-cli v1.2.3")
    assert prodex_codex_auth_probe_version_span_v1(
        1, UInt(Int(unsupported.unsafe_ptr())),
        Int64(unsupported.byte_length()), span_address
    ) == 0
    assert span[unsafe_offset=0] == -1
    assert span[unsafe_offset=1] == -1

    var header = StringSlice("A🚀\tβ\n")
    var sanitized = StringSlice("A____")
    var result_store = alloc(Layout[UInt8](count=32))
    var length_store = alloc(Layout[Int64](count=1))
    var result = result_store.unsafe_ptr()
    var length = length_store.unsafe_ptr()
    assert prodex_codex_auth_user_agent_ascii_v1(
        1, UInt(Int(header.unsafe_ptr())), Int64(header.byte_length()),
        UInt(Int(result)), 32, UInt(Int(length))
    ) == 0
    assert length[unsafe_offset=0] == Int64(sanitized.byte_length())
    for index in range(Int64(sanitized.byte_length())):
        assert result[unsafe_offset=index] == sanitized.unsafe_ptr()[unsafe_offset=index]

    length[unsafe_offset=0] = 77
    assert prodex_codex_auth_user_agent_ascii_v1(
        2, UInt(Int(header.unsafe_ptr())), Int64(header.byte_length()),
        UInt(Int(result)), 32, UInt(Int(length))
    ) == 4
    assert prodex_codex_auth_user_agent_ascii_v1(
        1, UInt(Int(header.unsafe_ptr())), Int64(header.byte_length()),
        UInt(Int(result)), 1, UInt(Int(length))
    ) == 1
    assert length[unsafe_offset=0] == 77
    dealloc(length_store^)
    dealloc(result_store^)
    dealloc(span_store^)
