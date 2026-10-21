from std.memory.alloc import alloc, dealloc, Layout
from provider_conversation_memory import ConversationMemoryNode, prodex_provider_conversation_memory_v1


def main() raises:
    var store = alloc(Layout[ConversationMemoryNode](count=4))
    var output_store = alloc(Layout[UInt64](count=1))
    var nodes = store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var n = UInt(Int(nodes))
    var o = UInt(Int(output))
    nodes[unsafe_offset=0] = ConversationMemoryNode(5, 0, 0)
    nodes[unsafe_offset=1] = ConversationMemoryNode(3, 3, 0)
    nodes[unsafe_offset=2] = ConversationMemoryNode(1, 0, 0)
    nodes[unsafe_offset=3] = ConversationMemoryNode(4, 1, 2)
    assert prodex_provider_conversation_memory_v1(1, n, 4, 4, 32, 1, 16, 18446744073709551615, o) == 0
    assert output[] == 123
    assert prodex_provider_conversation_memory_v1(1, n, 1, 4, 32, 1, 16, 18446744073709551615, o) == 0
    assert output[] == 4
    assert prodex_provider_conversation_memory_v1(1, n, 4, 18446744073709551615, 18446744073709551615, 1, 16, 18446744073709551615, o) == 0
    assert output[] == 18446744073709551615
    assert prodex_provider_conversation_memory_v1(1, n, 4, 4294967295, 32, 1, 16, 4294967295, o) == 0
    assert output[] == 4294967295
    output[] = 77
    assert prodex_provider_conversation_memory_v1(2, n, 4, 4, 32, 1, 16, 18446744073709551615, o) == 4
    assert prodex_provider_conversation_memory_v1(1, n + 1, 4, 4, 32, 1, 16, 18446744073709551615, o) == 1
    assert prodex_provider_conversation_memory_v1(1, n, 0, 4, 32, 1, 16, 18446744073709551615, o) == 1
    assert prodex_provider_conversation_memory_v1(1, n, 4, 4, 0, 1, 16, 18446744073709551615, o) == 1
    nodes[unsafe_offset=3] = ConversationMemoryNode(99, 0, 0)
    assert prodex_provider_conversation_memory_v1(1, n, 4, 4, 32, 1, 16, 18446744073709551615, o) == 1
    assert output[] == 77
    dealloc(output_store^)
    dealloc(store^)
