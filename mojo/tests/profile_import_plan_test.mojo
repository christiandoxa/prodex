# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_import_plan_test.mojo`.
from std.collections.list import List
from std.memory import Pointer

from profile_export_policy import (
    PROFILE_IMPORT_PLAN_DUPLICATE_NAME,
    PROFILE_IMPORT_PLAN_LOOKUP_IDENTITY,
    PROFILE_IMPORT_PLAN_OK,
    PROFILE_IMPORT_PLAN_PROVIDER_MISMATCH,
    ProdexRichStringView,
    prodex_profile_import_plan_v1,
)


def view(address: UInt, length: Int) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def call_plan(
    count: Int64,
    names: List[ProdexRichStringView],
    keys: List[ProdexRichStringView],
    lookup_names: List[ProdexRichStringView],
    flags: List[Int64],
    scratch: List[Int64],
    output: List[Int64],
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    return prodex_profile_import_plan_v1(
        1,
        count,
        UInt(Int(names.unsafe_ptr())),
        UInt(Int(keys.unsafe_ptr())),
        UInt(Int(lookup_names.unsafe_ptr())),
        UInt(Int(flags.unsafe_ptr())),
        UInt(Int(scratch.unsafe_ptr())),
        count * 3,
        UInt(Int(output.unsafe_ptr())),
        count * 5,
        UInt(Int(written)),
    )


def test_profile_import_planner_requests_first_external_lookup() raises:
    var first = String("first")
    var second = String("second")
    var account = String("account:acct-main")
    var names = List[ProdexRichStringView]()
    var keys = List[ProdexRichStringView]()
    var lookup_names = List[ProdexRichStringView]()
    var flags = List[Int64]()
    names.append(view(UInt(Int(first.unsafe_ptr())), Int(first.byte_length())))
    names.append(view(UInt(Int(second.unsafe_ptr())), Int(second.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    lookup_names.append(ProdexRichStringView(0, 0))
    lookup_names.append(ProdexRichStringView(0, 0))
    flags.extend([1, -1, 1, 1, -1, 1])
    var scratch = List[Int64]()
    var output = List[Int64]()
    for _ in range(6):
        scratch.append(0)
    for _ in range(10):
        output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(
        2, names, keys, lookup_names, flags, scratch, output, written_pointer
    )
    assert status == PROFILE_IMPORT_PLAN_LOOKUP_IDENTITY
    assert written == 1
    assert output[0] == 0


def test_profile_import_planner_stages_and_rewrites_duplicate_identity() raises:
    var first = String("first")
    var second = String("second")
    var account = String("account:acct-main")
    var names = List[ProdexRichStringView]()
    var keys = List[ProdexRichStringView]()
    var lookup_names = List[ProdexRichStringView]()
    var flags = List[Int64]()
    names.append(view(UInt(Int(first.unsafe_ptr())), Int(first.byte_length())))
    names.append(view(UInt(Int(second.unsafe_ptr())), Int(second.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    lookup_names.append(ProdexRichStringView(0, 0))
    lookup_names.append(ProdexRichStringView(0, 0))
    flags.extend([1, -1, 2, 1, -1, 1])
    var scratch = List[Int64]()
    var output = List[Int64]()
    for _ in range(6):
        scratch.append(0)
    for _ in range(10):
        output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(
        2, names, keys, lookup_names, flags, scratch, output, written_pointer
    )
    assert status == PROFILE_IMPORT_PLAN_OK
    assert written == 10
    assert output[1] == 1
    assert output[2] == 0
    assert output[6] == 2
    assert output[7] == 0
    assert output[9] == 0


def test_profile_import_planner_reuses_external_identity_match() raises:
    var first = String("first")
    var second = String("second")
    var account = String("account:acct-main")
    var existing = String("existing")
    var names = List[ProdexRichStringView]()
    var keys = List[ProdexRichStringView]()
    var lookup_names = List[ProdexRichStringView]()
    var flags = List[Int64]()
    names.append(view(UInt(Int(first.unsafe_ptr())), Int(first.byte_length())))
    names.append(view(UInt(Int(second.unsafe_ptr())), Int(second.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    keys.append(view(UInt(Int(account.unsafe_ptr())), Int(account.byte_length())))
    lookup_names.append(view(UInt(Int(existing.unsafe_ptr())), Int(existing.byte_length())))
    lookup_names.append(ProdexRichStringView(0, 0))
    flags.extend([1, -1, 3, 1, -1, 1])
    var scratch = List[Int64]()
    var output = List[Int64]()
    for _ in range(6):
        scratch.append(0)
    for _ in range(10):
        output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(
        2, names, keys, lookup_names, flags, scratch, output, written_pointer
    )
    assert status == PROFILE_IMPORT_PLAN_OK
    assert written == 10
    assert output[1] == 0
    assert output[3] == 1
    assert output[4] == 0
    assert output[6] == 0
    assert output[8] == 1
    assert output[9] == 0


def test_profile_import_planner_rejects_duplicate_names() raises:
    var first = String("same")
    var second = String("same")
    var names = List[ProdexRichStringView]()
    var keys = List[ProdexRichStringView]()
    var lookup_names = List[ProdexRichStringView]()
    var flags = List[Int64]()
    names.append(view(UInt(Int(first.unsafe_ptr())), Int(first.byte_length())))
    names.append(view(UInt(Int(second.unsafe_ptr())), Int(second.byte_length())))
    keys.append(ProdexRichStringView(0, 0))
    keys.append(ProdexRichStringView(0, 0))
    lookup_names.append(ProdexRichStringView(0, 0))
    lookup_names.append(ProdexRichStringView(0, 0))
    flags.extend([0, -1, 0, 0, -1, 0])
    var scratch = List[Int64]()
    var output = List[Int64]()
    for _ in range(6):
        scratch.append(0)
    for _ in range(10):
        output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(
        2, names, keys, lookup_names, flags, scratch, output, written_pointer
    )
    assert status == PROFILE_IMPORT_PLAN_DUPLICATE_NAME
    assert written == 1
    assert output[0] == 1


def test_profile_import_planner_rejects_provider_mismatch() raises:
    var name = String("main")
    var names = List[ProdexRichStringView]()
    var keys = List[ProdexRichStringView]()
    var lookup_names = List[ProdexRichStringView]()
    var flags = List[Int64]()
    names.append(view(UInt(Int(name.unsafe_ptr())), Int(name.byte_length())))
    keys.append(ProdexRichStringView(0, 0))
    lookup_names.append(ProdexRichStringView(0, 0))
    flags.extend([1, 0, 0])
    var scratch = List[Int64]()
    var output = List[Int64]()
    for _ in range(3):
        scratch.append(0)
    for _ in range(5):
        output.append(-1)
    var written: Int64 = 0
    var written_pointer = Pointer(to=written)

    var status = call_plan(
        1, names, keys, lookup_names, flags, scratch, output, written_pointer
    )
    assert status == PROFILE_IMPORT_PLAN_PROVIDER_MISMATCH
    assert written == 1
    assert output[0] == 0


def main() raises:
    test_profile_import_planner_requests_first_external_lookup()
    test_profile_import_planner_stages_and_rewrites_duplicate_identity()
    test_profile_import_planner_reuses_external_identity_match()
    test_profile_import_planner_rejects_duplicate_names()
    test_profile_import_planner_rejects_provider_mismatch()
