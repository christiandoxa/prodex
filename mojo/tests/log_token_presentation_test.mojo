from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING, JSON_NUMBER, JSON_TRUE, pj_literal, pj_equal
from rich_types import ProdexRichStringView
from log_token_presentation import prodex_log_token_presentation_v1


def main() raises:
    var raw = StringSlice('[{"timestamp":"time","request":18446744073709551615,"profile":"profile-a","transport":"sse","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0},"15.0 t/s","— t/s"]')
    var node_storage = alloc(Layout[ParsedJsonNode](count=15))
    var scratch_storage = alloc(Layout[Int64](count=30))
    var output_storage = alloc(Layout[UInt8](count=4096))
    var meta_storage = alloc(Layout[Int64](count=2))
    var nodes = node_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var metadata = meta_storage.unsafe_ptr()
    var empty = pj_literal(StringSlice(""))
    nodes[unsafe_offset=0] = ParsedJsonNode(JSON_ARRAY, 1, -1, -1, empty.copy(), empty.copy(), 0, 288)
    nodes[unsafe_offset=1] = ParsedJsonNode(JSON_OBJECT, 2, 13, 0, empty.copy(), empty.copy(), 1, 265)
    nodes[unsafe_offset=2] = ParsedJsonNode(JSON_STRING, -1, 3, 1, pj_literal(StringSlice('timestamp')), pj_literal(StringSlice('time')), 14, 6)
    nodes[unsafe_offset=3] = ParsedJsonNode(JSON_NUMBER, -1, 4, 1, pj_literal(StringSlice('request')), empty.copy(), 31, 20)
    nodes[unsafe_offset=4] = ParsedJsonNode(JSON_STRING, -1, 5, 1, pj_literal(StringSlice('profile')), pj_literal(StringSlice('profile-a')), 62, 11)
    nodes[unsafe_offset=5] = ParsedJsonNode(JSON_STRING, -1, 6, 1, pj_literal(StringSlice('transport')), pj_literal(StringSlice('sse')), 86, 5)
    nodes[unsafe_offset=6] = ParsedJsonNode(JSON_STRING, -1, 7, 1, pj_literal(StringSlice('source')), pj_literal(StringSlice('responses')), 101, 11)
    nodes[unsafe_offset=7] = ParsedJsonNode(JSON_NUMBER, -1, 8, 1, pj_literal(StringSlice('input_tokens')), empty.copy(), 128, 20)
    nodes[unsafe_offset=8] = ParsedJsonNode(JSON_NUMBER, -1, 9, 1, pj_literal(StringSlice('cached_input_tokens')), empty.copy(), 171, 1)
    nodes[unsafe_offset=9] = ParsedJsonNode(JSON_NUMBER, -1, 10, 1, pj_literal(StringSlice('output_tokens')), empty.copy(), 189, 2)
    nodes[unsafe_offset=10] = ParsedJsonNode(JSON_NUMBER, -1, 11, 1, pj_literal(StringSlice('reasoning_tokens')), empty.copy(), 211, 1)
    nodes[unsafe_offset=11] = ParsedJsonNode(JSON_NUMBER, -1, 12, 1, pj_literal(StringSlice('generation_ms')), empty.copy(), 229, 4)
    nodes[unsafe_offset=12] = ParsedJsonNode(JSON_NUMBER, -1, -1, 1, pj_literal(StringSlice('output_tokens_per_second')), empty.copy(), 261, 4)
    nodes[unsafe_offset=13] = ParsedJsonNode(JSON_STRING, -1, 14, 0, empty.copy(), pj_literal(StringSlice('15.0 t/s')), 267, 10)
    nodes[unsafe_offset=14] = ParsedJsonNode(JSON_STRING, -1, -1, 0, empty.copy(), pj_literal(StringSlice('— t/s')), 278, 9)
    var input = UInt64(Int(nodes))
    var source = UInt64(Int(raw.unsafe_ptr()))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output))
    var meta = UInt64(Int(metadata))
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,15,1,0,0,meta) == 0
    var required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,15,0,target,required-1,meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,15,0,target,4096,meta) == 0
    assert pj_equal(ProdexRichStringView(UInt(target), UInt(metadata[unsafe_offset=1])), pj_literal(StringSlice('{"event":"token_usage","fields":{"request":"18446744073709551615","transport":"sse","profile":"profile-a","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0},"timestamp":"time","request":18446744073709551615,"profile":"profile-a","transport":"sse","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0}')))
    assert prodex_log_token_presentation_v1(1,1,0,input,15,source,288,scratch,15,1,0,0,meta) == 0
    required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_log_token_presentation_v1(1,1,0,input,15,source,288,scratch,15,0,target,required-1,meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_log_token_presentation_v1(1,1,0,input,15,source,288,scratch,15,0,target,4096,meta) == 0
    assert pj_equal(ProdexRichStringView(UInt(target), UInt(metadata[unsafe_offset=1])), pj_literal(StringSlice('[["profile","profile-a"],["request","18446744073709551615"],["transport","sse"],["source","responses"],["input","18446744073709551615"],["cache","0"],["output","15"],["reasoning","2"],["generation","1000ms"],["avg_output","15.0 t/s"]]')))
    assert prodex_log_token_presentation_v1(1,2,0,input,15,source,288,scratch,15,1,0,0,meta) == 0
    required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_log_token_presentation_v1(1,2,0,input,15,source,288,scratch,15,0,target,required-1,meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_log_token_presentation_v1(1,2,0,input,15,source,288,scratch,15,0,target,4096,meta) == 0
    assert pj_equal(ProdexRichStringView(UInt(target), UInt(metadata[unsafe_offset=1])), pj_literal(StringSlice('[[["Muted","time"],["Raw"," "],["Token","TOKENS"]],[["Muted","profile "],["Primary","profile-a"],["Muted"," request "],["Primary","18446744073709551615"],["Muted"," transport "],["Primary","sse"],["Muted"," source "],["Primary","responses"]],[["Muted","input "],["Metric","18446744073709551615"],["Muted"," cache "],["Accent","0"],["Muted"," output "],["Metric","15"],["Muted"," reasoning "],["Tool","2"],["Muted"," generation "],["Metric","1000ms"]],[["Muted","average "],["Metric","15.0 t/s"]]]')))
    assert prodex_log_token_presentation_v1(1,3,0,input,15,source,288,scratch,15,1,0,0,meta) == 0
    required = metadata[unsafe_offset=1]
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    output[] = 93
    assert prodex_log_token_presentation_v1(1,3,0,input,15,source,288,scratch,15,0,target,required-1,meta) == 3
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    assert prodex_log_token_presentation_v1(1,3,0,input,15,source,288,scratch,15,0,target,4096,meta) == 0
    assert pj_equal(ProdexRichStringView(UInt(target), UInt(metadata[unsafe_offset=1])), pj_literal(StringSlice('"time 18446744073709551615 sse profile-a responses 18446744073709551615 0 15 2"')))
    output[] = 93
    metadata[unsafe_offset=0] = 77
    metadata[unsafe_offset=1] = 88
    assert prodex_log_token_presentation_v1(2,0,0,input,15,source,288,scratch,15,0,target,4096,meta) == 4
    assert prodex_log_token_presentation_v1(1,4,0,input,15,source,288,scratch,15,0,target,4096,meta) == 1
    assert prodex_log_token_presentation_v1(1,0,1,input,15,source,288,scratch,15,0,target,4096,meta) == 1
    assert prodex_log_token_presentation_v1(1,0,0,input+1,15,source,288,scratch,15,0,target,4096,meta) == 1
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,15,0,target,4096,meta+1) == 1
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,14,0,target,4096,meta) == 1
    nodes[unsafe_offset=3].kind = JSON_TRUE
    assert prodex_log_token_presentation_v1(1,0,0,input,15,source,288,scratch,15,0,target,4096,meta) == 1
    assert output[] == 93 and metadata[unsafe_offset=0] == 77 and metadata[unsafe_offset=1] == 88
    dealloc(node_storage^)
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(meta_storage^)
