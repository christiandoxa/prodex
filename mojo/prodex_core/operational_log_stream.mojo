from std.memory import Pointer
from std.collections import Array
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_matches_literal
from log_load_classification import log_load_event_is_routine
from observability_labels import (
    operational_event_classify, OPERATIONAL_EVENT_SOURCE_NONE,
    OPERATIONAL_EVENT_SOURCE_ROUTE, OPERATIONAL_EVENT_SOURCE_LOAD, OPERATIONAL_EVENT_SOURCE_ERROR,
)


def operational_stream_load_field(key: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["path"](key, False):
        return 2
    if (rich_view_matches_literal["profile"](key, False) or rich_view_matches_literal["route"](key, False)
        or rich_view_matches_literal["lane"](key, False) or rich_view_matches_literal["transport"](key, False)
        or rich_view_matches_literal["context"](key, False) or rich_view_matches_literal["provider"](key, False)
        or rich_view_matches_literal["model"](key, False) or rich_view_matches_literal["limit"](key, False)
        or rich_view_matches_literal["hard_limit"](key, False) or rich_view_matches_literal["reason"](key, False)):
        return 1
    return 0


@export("prodex_mojo_operational_log_stream_v1")
def prodex_mojo_operational_log_stream_v1(
    abi_version: Int64, event_address: UInt, event_length: Int64,
    fields_address: UInt, field_count: Int64,
    output_address: UInt, output_capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (event_length <= 0 or event_length > 65_536
        or field_count < 0 or field_count > 1_048_576
        or (field_count > 0 and (fields_address == 0 or output_address == 0))
        or output_capacity < 0 or metadata_address == 0):
        return 1
    if output_capacity < field_count:
        return 2
    var event = ProdexRichStringView(event_address, UInt(event_length))
    if not rich_view_valid(event, 65_536):
        return 1
    var fields = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(fields_address))
    var decoded_bytes: UInt = 0
    for index in range(field_count * 2):
        var text = fields[unsafe_offset=index].copy()
        if not rich_view_valid(text, 67_108_864):
            return 1
        decoded_bytes += text.len
        if decoded_bytes > 67_108_864:
            return 1
    var source: Int64 = OPERATIONAL_EVENT_SOURCE_NONE
    var suppressed = (rich_view_matches_literal["stream_payload"](event, False)
        or rich_view_matches_literal["upstream_payload"](event, False)
        or rich_view_matches_literal["token_usage"](event, False)
        or rich_view_matches_literal["token_usage_progress"](event, False))
    if not suppressed:
        var values = Array[ProdexRichStringView, 5](fill=ProdexRichStringView(0, 0))
        var present = Array[Int64, 5](fill=0)
        for index in range(field_count):
            var key = fields[unsafe_offset=index * 2].copy()
            var slot = -1
            if rich_view_matches_literal["tool_surface"](key, False):
                slot = 0
            elif rich_view_matches_literal["continuation"](key, False):
                slot = 1
            elif rich_view_matches_literal["family"](key, False):
                slot = 2
            elif rich_view_matches_literal["decision"](key, False):
                slot = 3
            elif rich_view_matches_literal["switched"](key, False):
                slot = 4
            if slot >= 0 and present[slot] == 0:
                values[slot] = fields[unsafe_offset=index * 2 + 1].copy()
                present[slot] = 1
        for slot in range(4):
            if values[slot].len > 65_536:
                return 1
        var plan = operational_event_classify(event_address, event_length,
            values[0].ptr, Int64(values[0].len), present[0],
            values[1].ptr, Int64(values[1].len), present[1],
            values[2].ptr, Int64(values[2].len), present[2],
            values[3].ptr, Int64(values[3].len), present[3])
        source = plan[0]
        if plan[1] == 0 or source == OPERATIONAL_EVENT_SOURCE_NONE:
            source = OPERATIONAL_EVENT_SOURCE_NONE
        elif source == OPERATIONAL_EVENT_SOURCE_LOAD and not log_load_event_is_routine(event):
            source = OPERATIONAL_EVENT_SOURCE_ERROR
        elif (source == OPERATIONAL_EVENT_SOURCE_ROUTE
            and rich_view_matches_literal["profile_commit"](event, False)
            and not rich_view_matches_literal["true"](values[4], False)):
            source = OPERATIONAL_EVENT_SOURCE_NONE
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var load = source == OPERATIONAL_EVENT_SOURCE_LOAD
    for index in range(field_count):
        output[unsafe_offset=index] = operational_stream_load_field(fields[unsafe_offset=index * 2]) if load else 0
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = source
    metadata[unsafe_offset=1] = Int64(load)
    return 0
