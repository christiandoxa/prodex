# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_import_auth_update_test.mojo`.
from std.collections.list import List
from std.memory import Pointer

from profile_export_policy import (
    PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH,
    PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH_EMAIL,
    ProdexRichStringView,
    prodex_profile_import_auth_update_target_v1,
)


def call_plan(
    target: String,
    existing: List[ProdexRichStringView],
    incoming_email_present: Int64,
    output: List[Int64],
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    return prodex_profile_import_auth_update_target_v1(
        1,
        UInt(Int(target.unsafe_ptr())),
        Int64(target.byte_length()),
        UInt(Int(existing.unsafe_ptr())),
        Int64(existing.__len__()),
        incoming_email_present,
        UInt(Int(output.unsafe_ptr())),
        Int64(output.__len__()),
        UInt(Int(written)),
    )


def test_auth_update_plan_selects_first_target_and_action() raises:
    var target = String("main")
    var other = String("other")
    var main = String("main")
    var names = List[ProdexRichStringView]()
    names.append(
        ProdexRichStringView(UInt(Int(other.unsafe_ptr())), UInt(other.byte_length()))
    )
    names.append(
        ProdexRichStringView(UInt(Int(main.unsafe_ptr())), UInt(main.byte_length()))
    )
    var output = List[Int64]()
    output.append(-1)
    output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(target, names, 0, output, written_pointer)
    assert status == 0
    assert written == 2
    assert output[0] == PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH
    assert output[1] == 1

    status = call_plan(target, names, 1, output, written_pointer)
    assert status == 0
    assert output[0] == PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH_EMAIL
    assert output[1] == 1


def main() raises:
    test_auth_update_plan_selects_first_target_and_action()
