# Shared provider history selection and bounded conversation-cache retention.
from std.memory import Pointer
from index_order import index_order_sort
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_prefix, rich_views_equal, rich_view_compare
from runtime_math import UINT64_MAX, INT64_MAX

comptime CONVERSATION_SCOPE_LIMIT: Int64 = 256
comptime CONVERSATION_TOTAL_LIMIT: Int64 = 2048
comptime CONVERSATION_BYTE_LIMIT: UInt64 = 64 * 1024 * 1024


@fieldwise_init
struct ConversationRow(Copyable):
    var key: ProdexRichStringView
    var sequence: UInt64
    var estimated_bytes: UInt64


def conversation_in_scope(key: ProdexRichStringView, scope: ProdexRichStringView, scoped: Int64) -> Bool:
    if scoped == 0:
        return not rich_view_prefix["\0prodex:"](key, False)
    return key.len >= scope.len and rich_views_equal(ProdexRichStringView(key.ptr, scope.len), scope)


def conversation_rows_valid(rows: Pointer[mut=False, ConversationRow, _], count: Int64) -> Bool:
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if not rich_view_valid(row.key, INT64_MAX):
            return False
        if index > 0 and rich_view_compare(rows[unsafe_offset=index - 1].key, row.key) >= 0:
            return False
    return True


@export("prodex_provider_conversation_history_v1")
def prodex_provider_conversation_history_v1(
    version: Int64, mode: Int64, scope_address: UInt, scope_length: Int64,
    scoped: Int64, rows_address: UInt, count: Int64, mask_address: UInt,
    mask_capacity: Int64, selected_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if mode < 0 or mode > 1 or scoped < 0 or scoped > 1 or scope_length < 0 or count < 0 or count > CONVERSATION_TOTAL_LIMIT:
        return 1
    if count > 0 and (rows_address == 0 or rows_address % 8 != 0 or mask_address == 0):
        return 1
    if selected_address == 0 or selected_address % 8 != 0:
        return 1
    if mask_capacity < count:
        return 3
    var scope = ProdexRichStringView(scope_address, UInt(scope_length))
    if not rich_view_valid(scope, INT64_MAX):
        return 1
    var rows = Pointer[mut=False, ConversationRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    var mask = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(mask_address))
    if not conversation_rows_valid(rows, count):
        return 1
    if mode == 1:
        for index in range(count):
            if mask[unsafe_offset=index] > 1:
                return 1
    var selected: Int64 = -1
    var sequence: UInt64 = 0
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        var eligible = conversation_in_scope(row.key, scope, scoped)
        if mode == 0:
            mask[unsafe_offset=index] = UInt8(eligible)
        elif eligible and mask[unsafe_offset=index] == 1 and (selected < 0 or row.sequence >= sequence):
            selected = index
            sequence = row.sequence
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_address))[] = selected
    return 0


def conversation_insert_sequence(sequence: UInt64) -> UInt64:
    return sequence if sequence == UINT64_MAX else sequence + 1


def conversation_oldest_compare(data: UInt, incoming: Int64, left: Int64, right: Int64) -> Int64:
    var rows = Pointer[mut=False, ConversationRow, ImmUntrackedOrigin](unsafe_from_address=Int(data))
    var a = rows[unsafe_offset=left].copy()
    var b = rows[unsafe_offset=right].copy()
    var a_sequence = conversation_insert_sequence(a.sequence) if left == incoming else a.sequence
    var b_sequence = conversation_insert_sequence(b.sequence) if right == incoming else b.sequence
    if a_sequence != b_sequence:
        return -1 if a_sequence < b_sequence else 1
    return rich_view_compare(a.key, b.key)


@export("prodex_provider_conversation_retention_v1")
def prodex_provider_conversation_retention_v1(
    version: Int64, scope_address: UInt, scope_length: Int64, scoped: Int64,
    rows_address: UInt, count: Int64, keep_address: UInt, keep_capacity: Int64,
    scratch_address: UInt, scratch_capacity: Int64, output_address: UInt,
) abi("C") -> Int64:
    # The final row is the incoming value; its sequence is the host's current counter.
    if version != 1:
        return 4
    if count < 0 or count > CONVERSATION_TOTAL_LIMIT or scoped < 0 or scoped > 1 or scope_length < 0:
        return 1
    if rows_address == 0 or rows_address % 8 != 0 or keep_address == 0 or scratch_address == 0 or scratch_address % 8 != 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    if keep_capacity < count + 1 or scratch_capacity < count + 1:
        return 3
    var scope = ProdexRichStringView(scope_address, UInt(scope_length))
    var rows = Pointer[mut=False, ConversationRow, ImmUntrackedOrigin](unsafe_from_address=Int(rows_address))
    var incoming = rows[unsafe_offset=count].copy()
    if not rich_view_valid(scope, INT64_MAX) or not rich_view_valid(incoming.key, INT64_MAX) or not conversation_rows_valid(rows, count):
        return 1
    var total: UInt64 = 0
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        if row.sequence > incoming.sequence or row.estimated_bytes > CONVERSATION_BYTE_LIMIT:
            return 1
        total += row.estimated_bytes
    if total > CONVERSATION_BYTE_LIMIT:
        return 1

    var keep = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(keep_address))
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var scope_count: Int64 = 0
    var live_count = count
    for index in range(count):
        var row = rows[unsafe_offset=index].copy()
        var retained = not rich_views_equal(row.key, incoming.key)
        keep[unsafe_offset=index] = UInt8(retained)
        if not retained:
            total -= row.estimated_bytes
            live_count -= 1
        elif conversation_in_scope(row.key, scope, scoped):
            scope_count += 1
    keep[unsafe_offset=count] = 0
    var sequence = incoming.sequence
    if incoming.estimated_bytes <= CONVERSATION_BYTE_LIMIT:
        keep[unsafe_offset=count] = 1
        sequence = conversation_insert_sequence(sequence)
        total += incoming.estimated_bytes
        live_count += 1
        if conversation_in_scope(incoming.key, scope, scoped):
            scope_count += 1
        if scope_count > CONVERSATION_SCOPE_LIMIT or live_count > CONVERSATION_TOTAL_LIMIT or total > CONVERSATION_BYTE_LIMIT:
            for index in range(count + 1):
                scratch[unsafe_offset=index] = index
            index_order_sort[conversation_oldest_compare](rows_address, count, scratch, count + 1)
            for ordered in range(count + 1):
                if scope_count <= CONVERSATION_SCOPE_LIMIT:
                    break
                var index = scratch[unsafe_offset=ordered]
                var row = rows[unsafe_offset=index].copy()
                if keep[unsafe_offset=index] == 1 and conversation_in_scope(row.key, scope, scoped):
                    keep[unsafe_offset=index] = 0
                    total -= row.estimated_bytes
                    scope_count -= 1
                    live_count -= 1
            for ordered in range(count + 1):
                if live_count <= CONVERSATION_TOTAL_LIMIT and total <= CONVERSATION_BYTE_LIMIT:
                    break
                var index = scratch[unsafe_offset=ordered]
                if keep[unsafe_offset=index] == 1:
                    keep[unsafe_offset=index] = 0
                    total -= rows[unsafe_offset=index].estimated_bytes
                    live_count -= 1
    output[unsafe_offset=0] = sequence
    output[unsafe_offset=1] = total
    return 0
