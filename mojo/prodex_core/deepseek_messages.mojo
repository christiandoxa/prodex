from std.memory import Pointer
from rich_types import ProdexRichStringView
from parsed_json import ParsedJson, ParsedJsonNode, pj_valid
from json_sink import JsonSink
from deepseek_message_common import ds_message, ds_normalize_thinking, DS_MESSAGE_ASSISTANT
from deepseek_message_adjacency import ds_repair_adjacency
from deepseek_message_metadata import ds_merge_metadata


@export("prodex_mojo_deepseek_messages_v1")
def prodex_mojo_deepseek_messages_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, nodes_count: Int64,
    raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64,
    metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if operation < 0 or operation > 3 or flag != 0 or measuring < 0 or measuring > 1 or raw_length < 0:
        return 1
    if capacity < 0 or metadata_address == 0 or nodes_address == 0 or scratch_address == 0 or scratch_count < nodes_count:
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree):
        return 1
    # The shared bridge supplies 16 bytes per node. Adjacency uses the first
    # Int64 half for sorted output-node indices and the second for emitted IDs.
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address))
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var present = True
    if operation == 0:
        present = ds_normalize_thinking(Pointer(to=sink), tree, scratch)
    elif operation == 1:
        ds_message(Pointer(to=sink), tree, 0, DS_MESSAGE_ASSISTANT, scratch, 0)
    elif operation == 2:
        present = ds_repair_adjacency(Pointer(to=sink), tree, scratch)
    else:
        present = ds_merge_metadata(Pointer(to=sink), tree)
    if sink.failed:
        return 3
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    meta[unsafe_offset=0] = Int64(present)
    meta[unsafe_offset=1] = sink.written
    return 0
