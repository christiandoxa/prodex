# Direct compiled profile lifecycle operation and filename-component policy.
from std.memory.alloc import alloc, dealloc, Layout
from profile_export_policy import (
    prodex_profile_export_journal_token_policy_v1,
    prodex_profile_export_journal_filename_plan_v1,
)


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

    var lifecycle = StringSlice("login-good-token.json")
    var missing_token = StringSlice("login.json")
    var bad_operation = StringSlice("logout-token.json")
    var unsafe_suffix = StringSlice("login-../.json")
    var result_store = alloc(Layout[Int64](count=3))
    var result = result_store.unsafe_ptr()
    var result_address = UInt(Int(result))
    assert prodex_profile_export_journal_filename_plan_v1(
        1, 1, UInt(Int(lifecycle.unsafe_ptr())),
        Int64(lifecycle.byte_length()), result_address
    ) == 0
    assert result[unsafe_offset=0] == 0
    assert prodex_profile_export_journal_filename_plan_v1(
        1, 1, UInt(Int(missing_token.unsafe_ptr())),
        Int64(missing_token.byte_length()), result_address
    ) == 0
    assert result[unsafe_offset=0] == 2
    assert prodex_profile_export_journal_filename_plan_v1(
        1, 1, UInt(Int(bad_operation.unsafe_ptr())),
        Int64(bad_operation.byte_length()), result_address
    ) == 0
    assert result[unsafe_offset=0] == 3
    assert result[unsafe_offset=2] == 6
    assert prodex_profile_export_journal_filename_plan_v1(
        1, 1, UInt(Int(unsafe_suffix.unsafe_ptr())),
        Int64(unsafe_suffix.byte_length()), result_address
    ) == 0
    assert result[unsafe_offset=0] == 4
    result[unsafe_offset=0] = 77
    assert prodex_profile_export_journal_filename_plan_v1(
        2, 1, UInt(Int(lifecycle.unsafe_ptr())),
        Int64(lifecycle.byte_length()), result_address
    ) == 4
    assert prodex_profile_export_journal_filename_plan_v1(
        1, 3, UInt(Int(lifecycle.unsafe_ptr())),
        Int64(lifecycle.byte_length()), result_address
    ) == 1
    assert result[unsafe_offset=0] == 77
    dealloc(result_store^)

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
