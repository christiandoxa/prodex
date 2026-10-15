# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/rich_route_test.mojo`.
from std.collections.list import List
from std.memory.alloc import alloc, dealloc, Layout

from rich_route import prodex_runtime_provider_models_response_plan_v1
from rich_types import ProdexRichStringView


def view(value: String) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def call_plan(
    method: String,
    route_kind: Int64,
    catalog_state: Int64,
    model_present: Int64,
    output: List[Int64],
) -> Int64:
    var method_allocation = alloc(Layout[ProdexRichStringView](count=1))
    var method_pointer = method_allocation.unsafe_ptr()
    method_pointer[] = view(method)
    var status = prodex_runtime_provider_models_response_plan_v1(
        6,
        UInt(Int(method_pointer)),
        route_kind,
        catalog_state,
        model_present,
        UInt(Int(output.unsafe_ptr())),
    )
    dealloc(method_allocation^)
    return status


def test_provider_models_response_plan_matrix() raises:
    for method, route, catalog, present, expected in [
        ("GET", 5, 1, 0, 2),
        ("get", 6, 1, 1, 3),
        ("GET", 6, 1, 0, 4),
        ("GET", 5, 2, 0, 1),
        ("POST", 5, 1, 0, 0),
        ("GET", 0, 1, 0, 0),
        ("GET", 5, 3, 0, 5),
    ]:
        var output = List[Int64]()
        output.append(-1)
        var status = call_plan(
            method, Int64(route), Int64(catalog), Int64(present), output
        )
        assert status == 0
        assert output[0] == Int64(expected)


def test_provider_models_response_plan_rejects_invalid_facts() raises:
    var output = List[Int64]()
    output.append(-1)
    assert call_plan("GET", 7, 1, 0, output) == 1
    assert call_plan("GET", 5, 1, 1, output) == 1
    assert call_plan("GET", 5, 4, 0, output) == 1


def main() raises:
    test_provider_models_response_plan_matrix()
    test_provider_models_response_plan_rejects_invalid_facts()
