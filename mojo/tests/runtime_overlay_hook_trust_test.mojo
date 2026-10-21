# Read-only compiled hook status selection and config success ABI.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_overlay_policy import prodex_runtime_overlay_hook_trust_policy_v1


def main() raises:
    var untrusted = StringSlice("untrusted")
    var modified = StringSlice("modified")
    var trusted = StringSlice("trusted")
    var uppercase = StringSlice("Untrusted")
    var names = [trusted, untrusted, modified, uppercase, modified]
    var views_store = alloc(Layout[UInt64](count=10))
    var output_store = alloc(Layout[Int64](count=5))
    var written_store = alloc(Layout[Int64](count=1))
    var views = views_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var written = written_store.unsafe_ptr()
    for i in range(5):
        views[unsafe_offset=i * 2] = UInt64(UInt(Int(names[i].unsafe_ptr())))
        views[unsafe_offset=i * 2 + 1] = UInt64(names[i].byte_length())
    var v = UInt(Int(views))
    var o = UInt(Int(output))
    var w = UInt(Int(written))
    assert prodex_runtime_overlay_hook_trust_policy_v1(1, 0, v, 5, o, 5, w) == 0
    assert written[unsafe_offset=0] == 3
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 2
    assert output[unsafe_offset=2] == 4

    var ok = StringSlice("okOverridden")
    views[unsafe_offset=0] = UInt64(UInt(Int(ok.unsafe_ptr())))
    views[unsafe_offset=1] = UInt64(ok.byte_length())
    assert prodex_runtime_overlay_hook_trust_policy_v1(1, 1, v, 1, o, 1, w) == 0
    assert written[unsafe_offset=0] == 1
    assert output[unsafe_offset=0] == 1
    assert prodex_runtime_overlay_hook_trust_policy_v1(1, 0, 0, 0, 0, 0, w) == 0
    assert written[unsafe_offset=0] == 0

    written[unsafe_offset=0] = 77
    assert prodex_runtime_overlay_hook_trust_policy_v1(2, 0, v, 5, o, 5, w) == 4
    assert prodex_runtime_overlay_hook_trust_policy_v1(1, 1, v, 5, o, 5, w) == 1
    views[unsafe_offset=1] = 65_537
    assert prodex_runtime_overlay_hook_trust_policy_v1(1, 0, v, 5, o, 5, w) == 1
    assert written[unsafe_offset=0] == 77
    dealloc(written_store^)
    dealloc(output_store^)
    dealloc(views_store^)
