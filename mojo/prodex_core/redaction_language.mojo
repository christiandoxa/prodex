# Presidio's existing substring keyword scores and configured-candidate priority.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr, rich_codepoint, rich_codepoint_width
from redaction import reaction_view_equals_literal


def language_lowercase_match(
    text: ProdexRichStringView, start: Int64, keyword: StringSlice,
) -> Bool:
    var source = rich_view_ptr(text)
    var expected = keyword.unsafe_ptr()
    var cursor = start
    for position in range(Int64(keyword.byte_length())):
        if cursor >= Int64(text.len):
            return False
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var scalar = rich_codepoint(source, cursor, width)
        cursor += width
        if scalar >= 65 and scalar <= 90:
            scalar += 32
        elif scalar == 0x212A:
            scalar = 107
        elif scalar == 0x0130:
            # Rust lowercases this to i + combining dot; only a final i fits.
            return expected[unsafe_offset=position] == 105 and position + 1 == Int64(keyword.byte_length())
        if scalar != Int64(expected[unsafe_offset=position]):
            return False
    return True


def language_contains(text: ProdexRichStringView, keyword: StringSlice) -> Bool:
    var source = rich_view_ptr(text)
    var cursor: Int64 = 0
    while cursor < Int64(text.len):
        if language_lowercase_match(text, cursor, keyword):
            return True
        cursor += rich_codepoint_width(source[unsafe_offset=cursor])
    return False


@export("prodex_redaction_language_plan_v1")
def prodex_redaction_language_plan_v1(
    abi: Int64, text_address: UInt64, text_length: Int64,
    candidates_address: UInt64, count: Int64, output_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if text_length < 0 or count < 0 or count > 0x7FFFFFFFFFFFFFFF // 16 or output_address == 0 or (count > 0 and candidates_address == 0):
        return 1
    var text = ProdexRichStringView(UInt(text_address), UInt(text_length))
    if not rich_view_valid(text, text_length):
        return 1
    var candidates = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(candidates_address))
    var indonesian: Int64 = -1
    var english: Int64 = -1
    for index in range(count):
        var candidate = candidates[unsafe_offset=index].copy()
        if not rich_view_valid(candidate, 0x7FFFFFFFFFFFFFFF):
            return 1
        if indonesian < 0 and reaction_view_equals_literal(candidate, StringSlice("id")):
            indonesian = index
        if english < 0 and reaction_view_equals_literal(candidate, StringSlice("en")):
            english = index
    var selected: Int64 = -1
    if count > 0:
        selected = 0
    if count > 1:
        var id_score: Int64 = 0
        var en_score: Int64 = 0
        for keyword in [StringSlice("yang"), StringSlice("dan"), StringSlice("di"), StringSlice("ke"), StringSlice("dari"), StringSlice("saya"), StringSlice("kami"), StringSlice("anda"), StringSlice("nomor"), StringSlice("nama"), StringSlice("alamat"), StringSlice("tanggal"), StringSlice("lahir"), StringSlice("dengan"), StringSlice("untuk")]:
            id_score += Int64(language_contains(text, keyword))
        for keyword in [StringSlice("the"), StringSlice("and"), StringSlice("to"), StringSlice("from"), StringSlice("my"), StringSlice("name"), StringSlice("phone"), StringSlice("email"), StringSlice("address"), StringSlice("with"), StringSlice("for"), StringSlice("birth")]:
            en_score += Int64(language_contains(text, keyword))
        if id_score > en_score and indonesian >= 0:
            selected = indonesian
        elif en_score > id_score and english >= 0:
            selected = english
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[] = selected
    return 0
