# Candidate policy for the live Gemini stream before any output is committed.
from std.memory import Pointer

from gemini_chat_response import gemini_chat_parts
from gemini_internal_instruction import gemini_instruction_sanitize, gemini_instruction_text_echoes
from gemini_response import gemini_grounding_first_candidate
from gemini_response_media import media_put_content, media_put_special_text
from json_sink import JsonSink
from parsed_json import (
    JSON_ARRAY, JSON_OBJECT, JSON_TRUE, ParsedJson, ParsedJsonNode, pj_child, pj_field,
    pj_kind, pj_next, pj_nonblank, pj_string_field, pj_text, pj_valid,
)
from provider_constraints import gemini_failure_retryable_reason
from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView


@fieldwise_init
struct GeminiPrecommitPlan(Copyable):
    var decision: Int64
    var visible: Bool
    var reasoning: Bool
    var reason: Int64


def gemini_precommit_candidate_plan(
    tree: ParsedJson, corpus: ProdexRichStringView,
    visible: Bool, reasoning: Bool,
    scratch: Pointer[mut=True, UInt8, MutUntrackedOrigin],
) -> GeminiPrecommitPlan:
    var plan = GeminiPrecommitPlan(0, visible, reasoning, -1)
    var feedback = pj_field(tree, 0, StringSlice("promptFeedback"))
    var block = pj_string_field(tree, feedback, StringSlice("blockReason"))
    if pj_field(tree, 0, StringSlice("error")) >= 0 or pj_nonblank(tree, block):
        plan.decision = 1
        return plan.copy()

    var candidate = gemini_grounding_first_candidate(tree)
    var grounding = pj_field(tree, candidate, StringSlice("groundingMetadata"))
    for index in range(2):
        var key = StringSlice("webSearchQueries") if index == 0 else StringSlice("groundingChunks")
        var array = pj_field(tree, grounding, key)
        if pj_kind(tree, array) == JSON_ARRAY and pj_child(tree, array) >= 0:
            plan.visible = True
            plan.decision = 1
            return plan.copy()

    var part = pj_child(tree, gemini_chat_parts(tree, 0))
    var sink = JsonSink(scratch, 0, 0, True, False)
    while part >= 0:
        if pj_kind(tree, part) != JSON_OBJECT:
            plan.decision = -1
            return plan.copy()
        if (
            pj_field(tree, part, StringSlice("functionCall")) >= 0
            or media_put_content(Pointer(to=sink), tree, part)
            or media_put_special_text(Pointer(to=sink), tree, part)
        ):
            plan.visible = True
            break
        var text = pj_text(tree, pj_string_field(tree, part, StringSlice("text")))
        if text.len > 0:
            if pj_kind(tree, pj_field(tree, part, StringSlice("thought"))) == JSON_TRUE:
                plan.reasoning = True
            else:
                var length: Int64 = 0
                var retained = gemini_instruction_sanitize(text, scratch, Pointer(to=length))
                if retained != 2 and not gemini_instruction_text_echoes(text, corpus):
                    plan.visible = True
                    break
        part = pj_next(tree, part)

    if plan.visible:
        plan.decision = 1
        return plan.copy()
    var reason_node = pj_string_field(tree, candidate, StringSlice("finishReason"))
    if not pj_nonblank(tree, reason_node):
        return plan.copy()
    var reason = pj_text(tree, reason_node)
    if gemini_failure_retryable_reason(UInt64(reason.ptr), Int64(reason.len)):
        plan.decision = 2
        plan.reason = reason_node
    elif rich_view_matches_literal["STOP"](reason, False) and not plan.reasoning:
        plan.decision = 2
    else:
        plan.decision = 1
    return plan.copy()


@export("prodex_mojo_gemini_precommit_v1")
def prodex_mojo_gemini_precommit_v1(
    abi: Int64, nodes_address: UInt, nodes_count: Int64,
    raw_address: UInt, raw_length: Int64, corpus_address: UInt, corpus_length: Int64,
    visible: Int64, reasoning: Int64, scratch_address: UInt, scratch_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        nodes_count <= 0 or nodes_address == 0 or nodes_address % 8 != 0
        or raw_length < 0 or (raw_length > 0 and raw_address == 0)
        or corpus_length < 0 or (corpus_length > 0 and corpus_address == 0)
        or visible < 0 or visible > 1 or reasoning < 0 or reasoning > 1
        or scratch_length < 0 or (scratch_length > 0 and scratch_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        nodes_count, ProdexRichStringView(raw_address, UInt(raw_length)),
    )
    var corpus = ProdexRichStringView(corpus_address, UInt(corpus_length))
    if not pj_valid(tree) or not rich_view_valid(corpus, corpus_length):
        return 1
    for index in range(nodes_count):
        if pj_text(tree, index).len > UInt(scratch_length):
            return 1
    var plan = gemini_precommit_candidate_plan(
        tree, corpus, visible == 1, reasoning == 1,
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(scratch_address)),
    )
    if plan.decision < 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = plan.decision
    output[unsafe_offset=1] = Int64(plan.visible)
    output[unsafe_offset=2] = Int64(plan.reasoning)
    output[unsafe_offset=3] = plan.reason
    return 0
