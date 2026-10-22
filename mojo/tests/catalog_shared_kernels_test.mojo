from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from rich_catalog import catalog_merge_additional, RICH_STATUS_OK, RICH_STATUS_CAPACITY
from provider_constraints import provider_copilot_prompt_token_limit, prodex_provider_scalar_policy_v1
from parsed_json import pj_literal


def main() raises:
    var prompt = pj_literal(StringSlice(" \tGpT-5.4\n"))
    assert provider_copilot_prompt_token_limit(prompt) == 922000
    assert provider_copilot_prompt_token_limit(pj_literal(StringSlice("unknown"))) == -1
    var high = pj_literal(StringSlice(" HIGH "))
    assert prodex_provider_scalar_policy_v1(1, 0, high.ptr, Int64(high.len)) == 4
    var enabled = pj_literal(StringSlice("on"))
    assert prodex_provider_scalar_policy_v1(1, 2, enabled.ptr, Int64(enabled.len)) == 1
    assert prodex_provider_scalar_policy_v1(2, 1, prompt.ptr, Int64(prompt.len)) == -3
    var views_storage = alloc(Layout[ProdexRichStringView](count=5))
    var output_storage = alloc(Layout[Int64](count=5))
    var views = views_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    views[unsafe_offset=0] = pj_literal(StringSlice(" A "))
    views[unsafe_offset=1] = pj_literal(StringSlice("a"))
    views[unsafe_offset=2] = pj_literal(StringSlice("\u3000"))
    views[unsafe_offset=3] = pj_literal(StringSlice("β"))
    views[unsafe_offset=4] = pj_literal(StringSlice("Β"))
    for index in range(5):
        output[unsafe_offset=index] = 93
    var ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(views))
    var owners = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(output))
    var merged = catalog_merge_additional(ids, 0, ids, owners, 0, ids, 5, output, 5)
    assert merged.status == RICH_STATUS_OK and merged.count == 3
    assert output[unsafe_offset=0] == 0 and output[unsafe_offset=1] == 3 and output[unsafe_offset=2] == 4
    assert output[unsafe_offset=3] == 93 and output[unsafe_offset=4] == 93
    merged = catalog_merge_additional(ids, 0, ids, owners, 0, ids, 5, output, 1)
    assert merged.status == RICH_STATUS_CAPACITY and merged.count == 1
    for index in range(5):
        output[unsafe_offset=index] = 93
    merged = catalog_merge_additional(ids, 0, ids, owners, 0, ids, 5, output, 0)
    assert merged.status == RICH_STATUS_CAPACITY and merged.count == 0 and output[] == 93
    merged = catalog_merge_additional(ids, 0, ids, owners, 0, ids, 0, output, 0)
    assert merged.status == RICH_STATUS_OK and merged.count == 0 and output[] == 93
    dealloc(views_storage^)
    dealloc(output_storage^)
