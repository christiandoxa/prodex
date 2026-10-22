from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichSlice
from rich_text import rich_slice_matches_literal
from parsed_json import pj_literal
from status_token_presentation import StatusTokenPanelInput, prodex_status_token_panel_v1


def main() raises:
    var row_storage = alloc(Layout[StatusTokenPanelInput](count=1))
    var row = row_storage.unsafe_ptr()
    row[] = StatusTokenPanelInput(3, 10, 2, 3, 1, 7, 4, 2,
        pj_literal(StringSlice("runtime")), pj_literal(StringSlice("active")),
        pj_literal(StringSlice("first")), pj_literal(StringSlice("last")))
    var scratch_storage = alloc(Layout[UInt8](count=128))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var input = UInt64(Int(row))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_status_token_panel_v1(1, input, scratch, 128, 0, target, 4096, meta) == 0
    assert rich_slice_matches_literal["{\"lines\":[[[\"secondary\",\"total \"],[\"metric\",\"13\"],[\"muted\",\"  7 event(s) / 4 log(s)\"]],[[\"raw\",\"in 10  cached 2  out 3  reasoning 1\"]],[[\"secondary\",\"token efficiency \"],[\"accent\",\"cache hit 20.0% · output share 23.1%\"]],[[\"secondary\",\"active/config \"],[\"accent\",\"runtime / active\"],[\"muted\",\"  2 profile(s)\"]]],\"history_title\":\" recent events first → last \"}"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    var required = metadata[unsafe_offset=1]
    assert prodex_status_token_panel_v1(1, 0, scratch, 128, 0, target, 4096, meta) == 0
    assert rich_slice_matches_literal["{\"lines\":[[[\"raw\",\"loading token history…\"]]],\"history_title\":\"\"}"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_status_token_panel_v1(1, input, scratch, 128, 0, target, required - 1, meta) == 3
    assert prodex_status_token_panel_v1(2, input, scratch, 128, 0, target, 4096, meta) == 4
    assert prodex_status_token_panel_v1(1, input, scratch, 127, 0, target, 4096, meta) == 1
    assert prodex_status_token_panel_v1(1, input, 0, 128, 0, target, 4096, meta) == 1
    row[].presence = 4
    assert prodex_status_token_panel_v1(1, input, scratch, 128, 0, target, 4096, meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(scratch_storage^)
    dealloc(row_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
