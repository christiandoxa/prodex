# Direct compiled profile lifecycle operation and filename-component policy.
from std.memory.alloc import alloc, dealloc, Layout
from profile_export_policy import prodex_profile_export_journal_token_policy_v1


def main() raises:
    var output_store = alloc(Layout[Int64](count=1))
    var output = output_store.unsafe_ptr()
    var answer = UInt(Int(output))
    var allowed = StringSlice("main_token-1")
    var traversal = StringSlice("../secret")
    var op = StringSlice("manage")
    var wrong_case = StringSlice("Manage")

    assert prodex_profile_export_journal_token_policy_v1(
        1, 0, UInt(Int(allowed.unsafe_ptr())), Int64(allowed.byte_length()), answer
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_profile_export_journal_token_policy_v1(
        1, 0, UInt(Int(traversal.unsafe_ptr())), Int64(traversal.byte_length()), answer
    ) == 0
    assert output[unsafe_offset=0] == 0
    assert prodex_profile_export_journal_token_policy_v1(
        1, 1, UInt(Int(op.unsafe_ptr())), Int64(op.byte_length()), answer
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert prodex_profile_export_journal_token_policy_v1(
        1, 1, UInt(Int(wrong_case.unsafe_ptr())), Int64(wrong_case.byte_length()), answer
    ) == 0
    assert output[unsafe_offset=0] == 0

    output[unsafe_offset=0] = 77
    assert prodex_profile_export_journal_token_policy_v1(
        2, 1, UInt(Int(op.unsafe_ptr())), Int64(op.byte_length()), answer
    ) == 4
    assert prodex_profile_export_journal_token_policy_v1(
        1, 2, UInt(Int(op.unsafe_ptr())), Int64(op.byte_length()), answer
    ) == 1
    assert prodex_profile_export_journal_token_policy_v1(
        1, 0, 0, 14, answer
    ) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
