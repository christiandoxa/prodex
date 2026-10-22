from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView, ProdexRichSlice
from rich_text import rich_slice_matches_literal
from parsed_json import pj_literal
from status_bar_presentation import prodex_status_bar_v1


def main() raises:
    var views_storage=alloc(Layout[ProdexRichStringView](count=2))
    var output_storage=alloc(Layout[UInt8](count=4096))
    var metadata_storage=alloc(Layout[Int64](count=2))
    var views=views_storage.unsafe_ptr()
    var output=output_storage.unsafe_ptr()
    var metadata=metadata_storage.unsafe_ptr()
    views[unsafe_offset=0]=pj_literal(StringSlice("test-profile"))
    views[unsafe_offset=1]=pj_literal(StringSlice("synthetic-time"))
    var texts=UInt64(Int(views))
    var target=UInt64(Int(output))
    var meta=UInt64(Int(metadata))
    assert prodex_status_bar_v1(1,0,0,0,0,0,target,4096,meta)==0
    assert rich_slice_matches_literal["[[\"title\",\" PRODEX STATUS \"],[\"accent\",\" profile loading \"],[\"secondary\",\" live · waiting for first snapshot \"]]"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    assert prodex_status_bar_v1(1,0,texts,2,3,0,target,4096,meta)==0
    assert rich_slice_matches_literal["[[\"title\",\" PRODEX STATUS \"],[\"accent\",\" profile test-profile \"],[\"secondary\",\" refreshing · synthetic-time \"]]"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    var required=metadata[unsafe_offset=1]
    output[]=93
    metadata[unsafe_offset=0]=77
    metadata[unsafe_offset=1]=88
    assert prodex_status_bar_v1(1,0,texts,2,3,0,target,required-1,meta)==3
    assert prodex_status_bar_v1(2,0,texts,2,3,0,target,4096,meta)==4
    assert prodex_status_bar_v1(1,2,texts,2,3,0,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,1,3,0,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,2,4,0,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts+1,2,3,0,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,2,3,0,target,4096,meta+1)==1
    assert prodex_status_bar_v1(1,0,texts,2,3,1,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,2,3,2,target,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,2,3,0,0,4096,meta)==1
    assert prodex_status_bar_v1(1,0,texts,2,3,0,target,-1,meta)==1
    assert prodex_status_bar_v1(1,1,texts,1,0,0,target,4096,meta)==1
    assert prodex_status_bar_v1(1,1,texts,1,2,0,target,4096,meta)==1
    assert output[]==93 and metadata[unsafe_offset=0]==77 and metadata[unsafe_offset=1]==88
    assert prodex_status_bar_v1(1,1,0,0,0,0,target,4096,meta)==0
    assert rich_slice_matches_literal["[[\"hint_bold\",\" q/esc \"],[\"raw\",\"quit  \"],[\"hint_bold\",\"r \"],[\"raw\",\"refresh  \"],[\"muted\",\"NET shows Prodex socket queues; disk rates come from /proc/<pid>/io\"]]"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    views[unsafe_offset=0]=pj_literal(StringSlice(""))
    assert prodex_status_bar_v1(1,1,texts,1,1,0,target,4096,meta)==0
    assert rich_slice_matches_literal["[[\"error\",\" refresh error: \"],[\"muted\",\"\"]]"](output, ProdexRichSlice(0, metadata[unsafe_offset=1]), False)
    var empty_error_length=metadata[unsafe_offset=1]
    assert prodex_status_bar_v1(1,1,texts,1,1,1,0,0,meta)==0
    assert metadata[unsafe_offset=0]==1 and metadata[unsafe_offset=1]==empty_error_length
    output[]=93
    output[unsafe_offset=1]=255
    metadata[unsafe_offset=0]=77
    metadata[unsafe_offset=1]=88
    views[unsafe_offset=0]=ProdexRichStringView(UInt(target+1), 1)
    assert prodex_status_bar_v1(1,1,texts,1,1,0,target,4096,meta)==1
    views[unsafe_offset=0]=ProdexRichStringView(0, 1)
    assert prodex_status_bar_v1(1,1,texts,1,1,0,target,4096,meta)==1
    assert output[]==93 and metadata[unsafe_offset=0]==77 and metadata[unsafe_offset=1]==88
    dealloc(views_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)
