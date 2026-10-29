from rich_fallback import runtime_error_scan_body
from rich_types import ProdexRichStringView


def classify(body: String, status: Int64) -> Int64:
    var view = ProdexRichStringView(UInt(Int(body.unsafe_ptr())), UInt(body.byte_length()))
    return runtime_error_scan_body(view, 0, status).source_kind


def main() raises:
    var policy = '{"error":{"code":"policy_capacity_unavailable"}}'
    var unrelated = '{"metadata":{"code":"policy_capacity_unavailable"},"error":{"code":"server_is_overloaded"}}'
    var nested = '{"error":{"details":{"code":"policy_capacity_unavailable"}}}'
    var overwritten = '{"error":{"code":"policy_capacity_unavailable"},"error":{"code":"server_is_overloaded"}}'
    assert classify(policy, 503) == 6
    assert classify(unrelated, 503) == 4
    assert classify(nested, 503) == 5
    assert classify(overwritten, 503) == 4
    assert classify(policy, 500) == 5
