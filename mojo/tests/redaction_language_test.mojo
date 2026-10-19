# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_language_test.mojo`.
from rich_types import ProdexRichStringView
from redaction_language import language_contains, prodex_redaction_language_plan_v1


def check(text: StringSlice, keyword: StringSlice, expected: Bool) raises:
    var source = ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))
    assert language_contains(source, keyword) == expected


def main() raises:
    check(StringSlice("NAMA DAN ALAMAT SAYA"), StringSlice("nama"), True)
    check(StringSlice("Dİ"), StringSlice("di"), True)
    check(StringSlice("İD"), StringSlice("id"), False)
    check(StringSlice("KE"), StringSlice("ke"), True)
    check(StringSlice("KAMI"), StringSlice("kami"), True)
    check(StringSlice("the phone address"), StringSlice("phone"), True)
    check(StringSlice("🧭東京"), StringSlice("di"), False)
    check(StringSlice(""), StringSlice("dan"), False)
    assert prodex_redaction_language_plan_v1(2, 0, 0, 0, 0, 0) == 4
