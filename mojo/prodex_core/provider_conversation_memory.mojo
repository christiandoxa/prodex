# Numeric Serde allocation facts; provider JSON acquisition stays in Rust.
from std.memory import Pointer
from parsed_json import JSON_NULL, JSON_FALSE, JSON_TRUE, JSON_NUMBER, JSON_STRING, JSON_ARRAY, JSON_OBJECT


@fieldwise_init
struct ConversationMemoryNode(Copyable):
    var kind: Int64
    var key_bytes: UInt64
    var string_bytes: UInt64


@export("prodex_provider_conversation_memory_v1")
def prodex_provider_conversation_memory_v1(
    version: Int64, nodes_address: UInt, count: Int64, storage_key_bytes: UInt64,
    value_bytes: UInt64, bool_bytes: UInt64, number_bytes: UInt64,
    maximum: UInt64, output_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if count <= 0 or count > 9223372036854775807 // 24 or nodes_address == 0 or nodes_address % 8 != 0:
        return 1
    if value_bytes == 0 or bool_bytes == 0 or number_bytes == 0 or maximum == 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    var nodes = Pointer[mut=False, ConversationMemoryNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address))
    var root = nodes[].copy()
    if root.kind != JSON_ARRAY or root.key_bytes != 0 or root.string_bytes != 0:
        return 1
    # The synthetic messages array is not a Serde Value owned by the cache.
    # The ABI count bound keeps the complete nonnegative sum within UInt128.
    var total = UInt128(storage_key_bytes)
    for index in range(Int64(1), count):
        var node = nodes[unsafe_offset=index].copy()
        if node.kind < JSON_NULL or node.kind > JSON_OBJECT or (node.kind != JSON_STRING and node.string_bytes != 0):
            return 1
        total += UInt128(value_bytes) + UInt128(node.key_bytes)
        if node.kind == JSON_FALSE or node.kind == JSON_TRUE:
            total += UInt128(bool_bytes)
        elif node.kind == JSON_NUMBER:
            total += UInt128(number_bytes)
        elif node.kind == JSON_STRING:
            total += UInt128(node.string_bytes)
    Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))[] = UInt64(min(total, UInt128(maximum)))
    return 0
