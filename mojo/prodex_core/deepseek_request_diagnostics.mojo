# Provider-neutral diagnostic rendering for compiled DeepSeek request plans.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from json_sink import JsonSink, js_literal, js_view
from parsed_json import pj_literal


def diagnostic_parts(operation: Int64, tag: Int64) -> Tuple[Bool, Bool, ProdexRichStringView, ProdexRichStringView, Bool]:
    var prefix = pj_literal(StringSlice(""))
    var suffix = pj_literal(StringSlice(""))
    var detail = False
    var valid = True
    if operation != 1 and operation != 2 and operation != 3 and operation != 5 and operation != 6 and operation != 7 and operation != 8 and operation != 13 and operation != 14:
        return (False, False, prefix^, suffix^, False)
    if tag == 0 or operation == 3 and tag == 10 or operation == 13 and (tag == 1 or tag == 2) or operation == 14 and tag == 256:
        return (True, False, prefix^, suffix^, False)
    if operation == 1:
        if tag == 1:
            prefix = pj_literal(StringSlice("frequency_penalty is deprecated and is not forwarded by Prodex"))
        elif tag == 2:
            prefix = pj_literal(StringSlice("presence_penalty is deprecated and is not forwarded by Prodex"))
        elif tag >= 10 and tag <= 16:
            var fields = [pj_literal(StringSlice("n")), pj_literal(StringSlice("seed")), pj_literal(StringSlice("service_tier")), pj_literal(StringSlice("prediction")), pj_literal(StringSlice("logit_bias")), pj_literal(StringSlice("functions")), pj_literal(StringSlice("function_call"))]
            prefix = fields[Int(tag - 10)].copy()
            suffix = pj_literal(StringSlice(" is not supported by this Responses adapter"))
        elif tag == 20:
            prefix = pj_literal(StringSlice("include must be an array"))
        elif tag == 21:
            prefix = pj_literal(StringSlice("store must be a boolean"))
        elif tag == 22:
            prefix = pj_literal(StringSlice("background responses are not supported by this Responses adapter"))
        elif tag == 23:
            prefix = pj_literal(StringSlice("background must be a boolean"))
        elif tag == 24:
            prefix = pj_literal(StringSlice("truncation=auto is not supported by this Responses adapter"))
        elif tag == 25:
            prefix = pj_literal(StringSlice("truncation `"))
            suffix = pj_literal(StringSlice("` is not supported"))
            detail = True
        elif tag == 26:
            prefix = pj_literal(StringSlice("truncation must be a string"))
        elif tag == 27:
            prefix = pj_literal(StringSlice("max_tool_calls is not supported by this Responses adapter"))
        elif tag == 28:
            prefix = pj_literal(StringSlice("text must be an object"))
        elif tag == 29:
            prefix = pj_literal(StringSlice("text."))
            suffix = pj_literal(StringSlice(" is not supported by this Responses adapter"))
            detail = True
        elif tag == 30:
            prefix = pj_literal(StringSlice("does not expose a compatible parallel_tool_calls=false control"))
        elif tag == 31:
            prefix = pj_literal(StringSlice("parallel_tool_calls must be a boolean"))
        elif tag == 32:
            prefix = pj_literal(StringSlice("stream_options must be an object"))
        elif tag == 33:
            prefix = pj_literal(StringSlice("stream_options requires stream=true"))
        elif tag == 34:
            prefix = pj_literal(StringSlice("stream_options."))
            suffix = pj_literal(StringSlice(" is not supported"))
            detail = True
        elif tag == 35:
            prefix = pj_literal(StringSlice("streaming adapter requires stream_options.include_usage=true"))
        elif tag == 36:
            prefix = pj_literal(StringSlice("stream_options.include_usage must be a boolean"))
        elif tag == 37:
            prefix = pj_literal(StringSlice("modalities must be an array"))
        elif tag == 38:
            prefix = pj_literal(StringSlice("Responses adapter only supports text modality; audio/image/video modalities are not supported"))
        elif tag == 39:
            prefix = pj_literal(StringSlice("Responses adapter does not support audio output"))
        else:
            valid = False
    elif operation == 2:
        if tag == 1:
            prefix = pj_literal(StringSlice("chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter"))
        elif tag == 2:
            prefix = pj_literal(StringSlice("FIM suffix completion requires the beta /completions endpoint, which is outside this Responses adapter"))
        elif tag == 3:
            prefix = pj_literal(StringSlice("prompt completions require the beta /completions endpoint, which is outside this Responses adapter"))
        else:
            valid = False
    elif operation == 3:
        if tag == 1:
            prefix = pj_literal(StringSlice("reasoning must be an object"))
        elif tag == 2:
            prefix = pj_literal(StringSlice("reasoning."))
            suffix = pj_literal(StringSlice(" is not supported by this Responses adapter"))
            detail = True
        elif tag == 3:
            prefix = pj_literal(StringSlice("reasoning.effort must be a string"))
        elif tag == 4:
            prefix = pj_literal(StringSlice("reasoning_effort must be a string"))
        else:
            valid = False
    elif operation == 5:
        if tag >= 1 and tag <= 6:
            var fields = [pj_literal(StringSlice("temperature must be a number")), pj_literal(StringSlice("top_p must be a number")), pj_literal(StringSlice("max_output_tokens must be a positive integer")), pj_literal(StringSlice("max_tokens must be a positive integer")), pj_literal(StringSlice("max_completion_tokens must be a positive integer")), pj_literal(StringSlice("logprobs must be a boolean"))]
            prefix = fields[Int(tag - 1)].copy()
        else:
            valid = False
    elif operation == 6:
        if tag >= 1 and tag <= 3:
            var fields = [pj_literal(StringSlice("top_logprobs must be an integer")), pj_literal(StringSlice("top_logprobs must be <= 20")), pj_literal(StringSlice("top_logprobs requires logprobs=true"))]
            prefix = fields[Int(tag - 1)].copy()
        else:
            valid = False
    elif operation == 7:
        if tag >= 1 and tag <= 3:
            var fields = [pj_literal(StringSlice("stop must be a string or array of strings")), pj_literal(StringSlice("supports at most 16 stop sequences")), pj_literal(StringSlice("stop sequences must be strings"))]
            prefix = fields[Int(tag - 1)].copy()
        else:
            valid = False
    elif operation == 8:
        if tag == 1 or tag == 4:
            prefix = pj_literal(StringSlice("tool_choice string `")) if tag == 1 else pj_literal(StringSlice("tool_choice type `"))
            suffix = pj_literal(StringSlice("` is not supported"))
            detail = True
        elif tag == 2:
            prefix = pj_literal(StringSlice("tool_choice must be a string or object"))
        elif tag == 3:
            prefix = pj_literal(StringSlice("named tool_choice requires a function name"))
        else:
            valid = False
    elif operation == 13:
        if tag == 3:
            prefix = pj_literal(StringSlice("response_format must include a type"))
        elif tag == 4:
            prefix = pj_literal(StringSlice("response_format type `"))
            suffix = pj_literal(StringSlice("` is not supported"))
            detail = True
        else:
            valid = False
    else:
        if tag >= 1 and tag <= 4:
            var fields = [pj_literal(StringSlice("request metadata must be an object")), pj_literal(StringSlice("client_metadata must be an object")), pj_literal(StringSlice("prompt_cache_key must be a string")), pj_literal(StringSlice("prompt_cache_retention must be a string"))]
            prefix = fields[Int(tag - 1)].copy()
        else:
            valid = False
    return (valid, valid, prefix^, suffix^, detail)


def diagnostic_render(
    sink: Pointer[mut=True, JsonSink, _], provider: ProdexRichStringView,
    detail: ProdexRichStringView, prefix: ProdexRichStringView, suffix: ProdexRichStringView, dynamic: Bool,
):
    js_view(sink, provider)
    js_literal(sink, StringSlice(" "))
    js_view(sink, prefix)
    if dynamic:
        js_view(sink, detail)
    js_view(sink, suffix)


@export("prodex_deepseek_request_diagnostic_v1")
def prodex_deepseek_request_diagnostic_v1(
    abi: Int64, operation: Int64, tag: Int64,
    provider_address: UInt64, provider_length: Int64,
    detail_address: UInt64, detail_length: Int64,
    output_address: UInt64, capacity: Int64, metadata_address: UInt64, metadata_count: Int64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if provider_length < 0 or detail_length < 0 or capacity < 0 or output_address == 0 or metadata_address == 0 or metadata_count < 2:
        return 1
    var provider = ProdexRichStringView(UInt(provider_address), UInt(provider_length))
    var detail = ProdexRichStringView(UInt(detail_address), UInt(detail_length))
    if not rich_view_valid(provider, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(detail, 0x7FFFFFFFFFFFFFFF):
        return 1
    var parts = diagnostic_parts(operation, tag)
    if not parts[0]:
        return 1
    var sink = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        capacity, 0, True, False,
    )
    if parts[1]:
        diagnostic_render(Pointer(to=sink), provider, detail, parts[2], parts[3], parts[4])
    if sink.failed or sink.written > capacity:
        return 3
    sink.measuring = False
    sink.written = 0
    if parts[1]:
        diagnostic_render(Pointer(to=sink), provider, detail, parts[2], parts[3], parts[4])
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = Int64(parts[1])
    metadata[unsafe_offset=1] = sink.written
    return 0
