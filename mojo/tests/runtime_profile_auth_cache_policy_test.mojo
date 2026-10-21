# Direct versioned runtime auth cache and failure-scoring policy regression.
from std.memory.alloc import alloc, dealloc, Layout
from runtime_health import prodex_runtime_profile_auth_cache_policy_v1


def main() raises:
    var inputs_store = alloc(Layout[Int64](count=4))
    var output_store = alloc(Layout[Int64](count=1))
    var inputs = inputs_store.unsafe_ptr()
    var output = output_store.unsafe_ptr()
    var source = UInt(Int(inputs))
    var target = UInt(Int(output))

    # Warm, exact TTL, clock rollback and proactive refresh, respectively.
    inputs[unsafe_offset=0] = 129
    inputs[unsafe_offset=1] = 100
    inputs[unsafe_offset=2] = 30
    inputs[unsafe_offset=3] = 0
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 0, source, 4, target) == 0
    assert output[unsafe_offset=0] == 0
    inputs[unsafe_offset=0] = 130
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 0, source, 4, target) == 0
    assert output[unsafe_offset=0] == 1
    inputs[unsafe_offset=0] = 80
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 0, source, 4, target) == 0
    assert output[unsafe_offset=0] == 1
    inputs[unsafe_offset=0] = 100
    inputs[unsafe_offset=3] = 1
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 0, source, 4, target) == 0
    assert output[unsafe_offset=0] == 1

    # 401 receives stronger score; all other HTTP statuses receive alternate.
    inputs[unsafe_offset=0] = 401
    inputs[unsafe_offset=1] = 0
    inputs[unsafe_offset=2] = 12
    inputs[unsafe_offset=3] = 2
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 1, source, 4, target) == 0
    assert output[unsafe_offset=0] == 12
    inputs[unsafe_offset=0] = 403
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 1, source, 4, target) == 0
    assert output[unsafe_offset=0] == 2
    inputs[unsafe_offset=1] = 25
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 1, source, 4, target) == 0
    assert output[unsafe_offset=0] == 25

    # ABI and out-of-domain input cannot mutate the result.
    output[unsafe_offset=0] = 77
    assert prodex_runtime_profile_auth_cache_policy_v1(2, 1, source, 4, target) == 4
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 3, source, 4, target) == 1
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 1, source, 3, target) == 1
    inputs[unsafe_offset=0] = 65536
    assert prodex_runtime_profile_auth_cache_policy_v1(1, 1, source, 4, target) == 1
    assert output[unsafe_offset=0] == 77
    dealloc(output_store^)
    dealloc(inputs_store^)
