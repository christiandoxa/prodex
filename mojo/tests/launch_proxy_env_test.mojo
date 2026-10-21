# Direct versioned proxy bypass normalization ABI contract.
from std.memory.alloc import alloc, dealloc, Layout
from launch_proxy_env import prodex_launch_proxy_bypass_plan_v1


def main() raises:
    var first = String(" Example.COM, localhost ,, ::1 ")
    var second = String("example.com,Internal.local, LOCALHOST ,")
    var words_storage = alloc(Layout[UInt64](count=4))
    var output_storage = alloc(Layout[Int64](count=18))
    var written_storage = alloc(Layout[Int64](count=1))
    var words = words_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var written = written_storage.unsafe_ptr()
    words[unsafe_offset=0] = UInt64(UInt(Int(first.unsafe_ptr())))
    words[unsafe_offset=1] = UInt64(first.byte_length())
    words[unsafe_offset=2] = UInt64(UInt(Int(second.unsafe_ptr())))
    words[unsafe_offset=3] = UInt64(second.byte_length())
    var words_address = UInt(Int(words))
    var output_address = UInt(Int(output))
    var written_address = UInt(Int(written))

    assert prodex_launch_proxy_bypass_plan_v1(
        1, words_address, 2, output_address, 6, written_address
    ) == 0
    assert written[unsafe_offset=0] == 4
    assert output[unsafe_offset=0] == 0
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 11
    assert output[unsafe_offset=3] == 0
    assert output[unsafe_offset=6] == 0
    assert output[unsafe_offset=9] == 1
    assert prodex_launch_proxy_bypass_plan_v1(
        1, words_address, 2, output_address, 2, written_address
    ) == 3
    assert prodex_launch_proxy_bypass_plan_v1(
        2, words_address, 2, output_address, 6, written_address
    ) == 4
    assert prodex_launch_proxy_bypass_plan_v1(
        1, 0, 2, output_address, 6, written_address
    ) == 1
    assert prodex_launch_proxy_bypass_plan_v1(
        1, 0, 0, 0, 0, written_address
    ) == 0
    assert written[unsafe_offset=0] == 0
    dealloc(written_storage^)
    dealloc(output_storage^)
    dealloc(words_storage^)
