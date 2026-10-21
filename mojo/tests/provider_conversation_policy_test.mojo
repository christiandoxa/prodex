from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from provider_conversation_policy import (
    ConversationRow, prodex_provider_conversation_history_v1,
    prodex_provider_conversation_retention_v1,
)


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def main() raises:
    var row_store = alloc(Layout[ConversationRow](count=4))
    var mask_store = alloc(Layout[UInt8](count=4))
    var scratch_store = alloc(Layout[Int64](count=4))
    var output_store = alloc(Layout[UInt64](count=2))
    var rows = row_store.unsafe_ptr()
    var mask = mask_store.unsafe_ptr()
    var scratch = scratch_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var r = UInt(Int(rows))
    var m = UInt(Int(mask))
    var s = UInt(Int(scratch))
    var o = UInt(Int(output))
    var scope = view("\0prodex:1:a:")
    rows[unsafe_offset=0] = ConversationRow(view("\0prodex:1:a:a"), 9, 10)
    rows[unsafe_offset=1] = ConversationRow(view("\0prodex:1:a:z"), 9, 20)
    rows[unsafe_offset=2] = ConversationRow(view("root"), 99, 30)
    assert prodex_provider_conversation_history_v1(1, 0, scope.ptr, Int64(scope.len), 1, r, 3, m, 4, o) == 0
    assert mask[unsafe_offset=0] == 1 and mask[unsafe_offset=1] == 1 and mask[unsafe_offset=2] == 0
    assert prodex_provider_conversation_history_v1(1, 1, scope.ptr, Int64(scope.len), 1, r, 3, m, 4, o) == 0
    assert output[] == 1
    mask[unsafe_offset=1] = 0
    assert prodex_provider_conversation_history_v1(1, 1, scope.ptr, Int64(scope.len), 1, r, 3, m, 4, o) == 0
    assert output[] == 0
    assert prodex_provider_conversation_history_v1(1, 0, 0, 0, 0, r, 3, m, 4, o) == 0
    assert mask[unsafe_offset=0] == 0 and mask[unsafe_offset=1] == 0 and mask[unsafe_offset=2] == 1

    rows[unsafe_offset=3] = ConversationRow(view("root"), 99, 40)
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 3, m, 4, s, 4, o) == 0
    assert mask[unsafe_offset=2] == 0 and mask[unsafe_offset=3] == 1
    assert output[] == 100 and output[unsafe_offset=1] == 70
    rows[unsafe_offset=3] = ConversationRow(view("root"), 99, 64 * 1024 * 1024 + 1)
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 3, m, 4, s, 4, o) == 0
    assert mask[unsafe_offset=2] == 0 and mask[unsafe_offset=3] == 0
    assert output[] == 99 and output[unsafe_offset=1] == 30

    for index in range(4):
        mask[unsafe_offset=index] = 7
        scratch[unsafe_offset=index] = 77
    output[] = 77
    output[unsafe_offset=1] = 77
    assert prodex_provider_conversation_retention_v1(2, 0, 0, 0, r, 3, m, 4, s, 4, o) == 4
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 3, m, 3, s, 4, o) == 3
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 3, m, 4, s, 3, o) == 3
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r + 1, 3, m, 4, s, 4, o) == 1
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 2049, m, 4, s, 4, o) == 1
    assert prodex_provider_conversation_history_v1(1, 1, 0, 0, 0, r, 3, m, 4, o) == 1
    var invalid_store = alloc(Layout[UInt8](count=1))
    invalid_store.unsafe_ptr()[] = 255
    rows[unsafe_offset=0] = ConversationRow(ProdexRichStringView(UInt(Int(invalid_store.unsafe_ptr())), 1), 9, 10)
    assert prodex_provider_conversation_retention_v1(1, 0, 0, 0, r, 3, m, 4, s, 4, o) == 1
    for index in range(4):
        assert mask[unsafe_offset=index] == 7
        assert scratch[unsafe_offset=index] == 77
    assert output[] == 77 and output[unsafe_offset=1] == 77
    dealloc(invalid_store^)
    dealloc(output_store^)
    dealloc(scratch_store^)
    dealloc(mask_store^)
    dealloc(row_store^)
