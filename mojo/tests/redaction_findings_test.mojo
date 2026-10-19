# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_findings_test.mojo`.
from rich_types import ProdexRichStringView
from redaction_findings import finding_confidence, finding_byte_offset, finding_scalar_count, finding_entity_kind


def main() raises:
    var text = StringSlice("é 🧭東京")
    var view = ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))
    assert finding_scalar_count(view) == 5
    assert finding_byte_offset(view, 0) == 0
    assert finding_byte_offset(view, 2) == 3
    assert finding_byte_offset(view, 3) == 7
    assert finding_byte_offset(view, 5) == 13
    assert finding_confidence(0.0) == 0
    assert finding_confidence(1.0) == 10000
    assert finding_confidence(0.00005) == 1
    assert finding_confidence(0.000049999999999999996) == 0
    var entity = StringSlice("NATIONAL_ID")
    assert finding_entity_kind(ProdexRichStringView(UInt(Int(entity.unsafe_ptr())), UInt(entity.byte_length()))) == 4
    var unknown = StringSlice("CUSTOM_ENTITY")
    assert finding_entity_kind(ProdexRichStringView(UInt(Int(unknown.unsafe_ptr())), UInt(unknown.byte_length()))) == 11
