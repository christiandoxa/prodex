# Provider aliases and native-profile membership extend the quota-watch owner.
from std.memory import Pointer
from json_sink import JsonSink, js_byte, js_literal
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from quota_watch_policy import (
    quota_watch_ascii_lower, WATCH_FILTER_ALL, WATCH_FILTER_OPENAI,
    WATCH_FILTER_GEMINI, WATCH_FILTER_ANTHROPIC, WATCH_FILTER_COPILOT,
    WATCH_FILTER_KIRO, WATCH_FILTER_DEEPSEEK, WATCH_FILTER_LOCAL, WATCH_FILTER_AGY,
)


def qpf_normalized_byte(value: UInt8) -> UInt8:
    return UInt8(45) if value == 95 else quota_watch_ascii_lower(value)


def qpf_alias(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    if Int64(view.len) != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if qpf_normalized_byte(source[unsafe_offset=index]) != expected[unsafe_offset=index]:
            return False
    return True


def qpf_kind(view: ProdexRichStringView) -> Int64:
    if view.len == 0 or qpf_alias(view, StringSlice("all")):
        return WATCH_FILTER_ALL
    if qpf_alias(view, StringSlice("openai")) or qpf_alias(view, StringSlice("chatgpt")) or qpf_alias(view, StringSlice("codex")) or qpf_alias(view, StringSlice("openai-codex")):
        return WATCH_FILTER_OPENAI
    if qpf_alias(view, StringSlice("gemini")) or qpf_alias(view, StringSlice("google")) or qpf_alias(view, StringSlice("google-gemini")):
        return WATCH_FILTER_GEMINI
    if qpf_alias(view, StringSlice("anthropic")) or qpf_alias(view, StringSlice("claude")) or qpf_alias(view, StringSlice("anthropic-claude")):
        return WATCH_FILTER_ANTHROPIC
    if qpf_alias(view, StringSlice("copilot")) or qpf_alias(view, StringSlice("github-copilot")) or qpf_alias(view, StringSlice("github")):
        return WATCH_FILTER_COPILOT
    if qpf_alias(view, StringSlice("kiro")) or qpf_alias(view, StringSlice("kiro-cli")):
        return WATCH_FILTER_KIRO
    if qpf_alias(view, StringSlice("deepseek")):
        return WATCH_FILTER_DEEPSEEK
    if qpf_alias(view, StringSlice("local")) or qpf_alias(view, StringSlice("openai-compatible")):
        return WATCH_FILTER_LOCAL
    if qpf_alias(view, StringSlice("agy")) or qpf_alias(view, StringSlice("anti-gravity")):
        return WATCH_FILTER_AGY
    return -1


def qpf_diagnostic(sink: Pointer[mut=True, JsonSink, _], view: ProdexRichStringView):
    js_literal(sink, StringSlice("invalid quota provider filter '"))
    var source = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        js_byte(sink, qpf_normalized_byte(source[unsafe_offset=index]))
    js_literal(sink, StringSlice("'; supported values are all, openai, gemini, anthropic, claude, copilot, kiro, deepseek, local, agy"))


@export("prodex_quota_watch_provider_filter_parse_v1")
def prodex_quota_watch_provider_filter_parse_v1(
    abi_version: Int64, input_address: UInt, input_length: Int64,
    measuring: Int64, output_address: UInt, capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        input_length < 0 or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return 1
    var bounds = rich_trim_bounds(view)
    var trimmed = ProdexRichStringView(view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))
    var kind = qpf_kind(trimmed)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    if kind == -1:
        qpf_diagnostic(Pointer(to=sink), trimmed)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        if kind == -1:
            qpf_diagnostic(Pointer(to=sink), trimmed)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = kind
    metadata[unsafe_offset=1] = required
    return 0


@export("prodex_quota_watch_profile_filter_matches_v1")
def prodex_quota_watch_profile_filter_matches_v1(
    abi_version: Int64, filter_kind: Int64, profile_kind: Int64, result_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        filter_kind < WATCH_FILTER_ALL or filter_kind > WATCH_FILTER_AGY
        or (profile_kind < WATCH_FILTER_OPENAI or profile_kind > WATCH_FILTER_KIRO)
           and profile_kind != WATCH_FILTER_AGY
        or result_address == 0 or result_address % 8 != 0
    ):
        return 1
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(result_address))
    result[] = Int64(1) if filter_kind == WATCH_FILTER_ALL or filter_kind == profile_kind else Int64(0)
    return 0
