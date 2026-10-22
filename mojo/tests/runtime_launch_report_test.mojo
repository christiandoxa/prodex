from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView, ProdexRichSlice
from rich_text import rich_slice_matches_literal
from parsed_json import pj_literal
from runtime_launch_report import prodex_runtime_launch_report_v1


def main() raises:
    var views_storage = alloc(Layout[ProdexRichStringView](count=10))
    var output_storage = alloc(Layout[UInt8](count=2048))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var views = views_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = metadata_storage.unsafe_ptr()
    var texts = UInt64(Int(views))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    for index in range(10):
        views[unsafe_offset=index] = pj_literal(StringSlice(""))
    views[unsafe_offset=0] = pj_literal(StringSlice("run"))
    views[unsafe_offset=1] = pj_literal(StringSlice("codex"))
    assert prodex_runtime_launch_report_v1(1,0,texts,6,0,0,0,0,0,0,target,2048,meta) == 0
    assert rich_slice_matches_literal["Prodex dry run: launch diagnostics\nFlow: run\nBinary: codex\nProvider: openai\nModel: (codex default)\nCODEX_HOME: <CODEX_HOME>\nRuntime proxy: disabled\nArgs:\n  (none)\nEnv:\n  CODEX_HOME=<CODEX_HOME>\nCodex/TUI not started because --dry-run was set.\n"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    views[unsafe_offset=2] = pj_literal(StringSlice("provider"))
    views[unsafe_offset=3] = pj_literal(StringSlice("model"))
    views[unsafe_offset=4] = pj_literal(StringSlice("[::1]:1"))
    views[unsafe_offset=5] = pj_literal(StringSlice("/v1"))
    views[unsafe_offset=6] = pj_literal(StringSlice("exec"))
    views[unsafe_offset=7] = pj_literal(StringSlice("VISIBLE"))
    views[unsafe_offset=8] = pj_literal(StringSlice("1"))
    views[unsafe_offset=9] = pj_literal(StringSlice("OLD"))
    assert prodex_runtime_launch_report_v1(1,0,texts,10,7,1,1,1,1,0,target,2048,meta) == 0
    assert rich_slice_matches_literal["Prodex dry run: launch diagnostics\nFlow: run\nBinary: codex\nProvider: provider\nModel: model\nCODEX_HOME: <CODEX_HOME>\nRuntime proxy: enabled at http://[::1]:1/v1\nArgs:\n  exec\nEnv:\n  CODEX_HOME=<CODEX_HOME>\n  VISIBLE=1\nRemoved env:\n  OLD\nCodex/TUI not started because --dry-run was set.\n"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    assert prodex_runtime_launch_report_v1(1,0,texts,10,7,1,1,1,0,0,target,2048,meta) == 0
    views[unsafe_offset=0] = pj_literal(StringSlice("base"))
    assert prodex_runtime_launch_report_v1(1,1,texts,1,3,0,0,0,0,0,target,2048,meta) == 0
    assert rich_slice_matches_literal["baseProfile: <configured>\nPresidio redaction: enabled (would start at launch)\n"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    views[unsafe_offset=0] = pj_literal(StringSlice("base\n"))
    views[unsafe_offset=1] = pj_literal(StringSlice(""))
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,0,target,2048,meta) == 0
    assert rich_slice_matches_literal["base\nProfile: (active/default)\n\nPresidio redaction: disabled (not started)\n"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    var required = metadata[unsafe_offset=1]
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,1,0,0,meta) == 0
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == required
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,0,target,required-1,meta) == 3
    assert prodex_runtime_launch_report_v1(2,1,texts,2,4,0,0,0,0,0,target,2048,meta) == 4
    assert prodex_runtime_launch_report_v1(1,2,texts,2,4,0,0,0,0,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,1,texts,2,8,0,0,0,0,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,1,0,0,0,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,0,texts,6,0,1,0,0,0,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,0,texts,6,0,0,0,0,1,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,0,texts,6,4,0,0,0,65536,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,1,texts+1,2,4,0,0,0,0,0,target,2048,meta) == 1
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,0,target,2048,meta+1) == 1
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,1,target,2048,meta) == 1
    output[unsafe_offset=1] = 255
    views[unsafe_offset=0] = ProdexRichStringView(UInt(target+1), 1)
    assert prodex_runtime_launch_report_v1(1,1,texts,2,4,0,0,0,0,0,target,2048,meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(views_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
