# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/smart_context_request_plan_test.mojo`.
from std.memory.alloc import alloc, dealloc, Layout

from smart_context_policy import (
    prodex_smart_context_body_admission_plan_v1,
    prodex_smart_context_body_shape_plan_v1,
    prodex_smart_context_rewrite_outcome_plan_v1,
    prodex_smart_context_telemetry_label_v1,
)
from smart_context_artifact_ref import prodex_smart_context_artifact_ref_v1


def test_request_plan_edges() raises:
    var reason_allocation = alloc(Layout[Int64](count=1))
    var reason = reason_allocation.unsafe_ptr()

    var status = prodex_smart_context_body_admission_plan_v1(
        1, 0, 0, 1, 1, 0, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 3

    status = prodex_smart_context_body_admission_plan_v1(
        1, 18446744073709551615, 0, 1, 1, 0, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 6

    status = prodex_smart_context_body_admission_plan_v1(
        1, 18446744073709551615, 0, 1, 1, 1, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 0

    status = prodex_smart_context_body_shape_plan_v1(
        1, 0, 1, 0, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 1

    status = prodex_smart_context_body_shape_plan_v1(
        1, 1, 0, 1, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 2

    status = prodex_smart_context_body_shape_plan_v1(
        1, 1, 1, 0, 0, 0, UInt(Int(reason))
    )
    assert status == 0
    assert reason[] == 4

    var outcome_allocation = alloc(Layout[Int64](count=1))
    var outcome = outcome_allocation.unsafe_ptr()
    status = prodex_smart_context_rewrite_outcome_plan_v1(
        1,
        18446744073709551615,
        0,
        0,
        0,
        0,
        0,
        UInt(Int(outcome)),
    )
    assert status == 0
    assert outcome[] == 1

    var output_allocation = alloc(Layout[UInt8](count=128))
    var output = output_allocation.unsafe_ptr()
    var written_allocation = alloc(Layout[Int64](count=1))
    var written = written_allocation.unsafe_ptr()
    status = prodex_smart_context_telemetry_label_v1(
        1,
        4,
        1 | (1 << 5),
        UInt(Int(output)),
        128,
        UInt(Int(written)),
    )
    assert status == 0
    assert written[] == 23
    assert output[unsafe_offset=0] == 116  # t
    assert output[unsafe_offset=11] == 44  # comma

    var marker = String("unicode café psc:0123456789abcdef")
    var marker_meta_allocation = alloc(Layout[Int64](count=4))
    var marker_meta = marker_meta_allocation.unsafe_ptr()
    status = prodex_smart_context_artifact_ref_v1(
        1,
        5,
        UInt(Int(marker.unsafe_ptr())),
        Int64(marker.byte_length()),
        0,
        0,
        0,
        0,
        1,
        0,
        UInt(Int(marker_meta)),
    )
    assert status == 0
    assert marker_meta[unsafe_offset=0] == 1

    dealloc(marker_meta_allocation^)
    dealloc(written_allocation^)
    dealloc(output_allocation^)
    dealloc(outcome_allocation^)
    dealloc(reason_allocation^)


def main() raises:
    test_request_plan_edges()
