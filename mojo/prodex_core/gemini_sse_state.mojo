from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_matches_literal, rich_trim_bounds


comptime GEMINI_RESPONSE_STATE_ABI_VERSION: Int64 = 1
comptime GEMINI_RESPONSE_STATE_STATUS_OK: Int64 = 0
comptime GEMINI_RESPONSE_STATE_STATUS_INVALID: Int64 = 1

comptime GEMINI_RESPONSE_PART_REASONING: Int64 = 1
comptime GEMINI_RESPONSE_PART_VISIBLE_TEXT: Int64 = 2
comptime GEMINI_RESPONSE_PART_SPECIAL_TEXT: Int64 = 4
comptime GEMINI_RESPONSE_PART_MEDIA: Int64 = 8
comptime GEMINI_RESPONSE_PART_NATIVE: Int64 = 16
comptime GEMINI_RESPONSE_PART_IMAGE: Int64 = 32
comptime GEMINI_RESPONSE_PART_FUNCTION: Int64 = 64
comptime GEMINI_RESPONSE_PART_FLUSH_PENDING: Int64 = 128

# The translator sends one compact fact word per content part.  Keeping the
# wire bits here makes the batch operation a single semantic authority while
# Rust remains responsible for acquiring the JSON facts.
comptime GEMINI_RESPONSE_PART_FLAG_HAS_TEXT: Int64 = 1
comptime GEMINI_RESPONSE_PART_FLAG_IS_THOUGHT: Int64 = 2
comptime GEMINI_RESPONSE_PART_FLAG_HAS_VISIBLE_TEXT: Int64 = 4
comptime GEMINI_RESPONSE_PART_FLAG_HAS_SPECIAL_TEXT: Int64 = 8
comptime GEMINI_RESPONSE_PART_FLAG_HAS_MEDIA: Int64 = 16
comptime GEMINI_RESPONSE_PART_FLAG_HAS_VIDEO_METADATA: Int64 = 32
comptime GEMINI_RESPONSE_PART_FLAG_HAS_IMAGE_GENERATION: Int64 = 64
comptime GEMINI_RESPONSE_PART_FLAG_HAS_FUNCTION_CALL: Int64 = 128
comptime GEMINI_RESPONSE_PART_FLAG_COMMAND_OUTPUT_ONLY: Int64 = 256
comptime GEMINI_RESPONSE_PART_FLAG_FORCED_OUTPUT: Int64 = 512
comptime GEMINI_RESPONSE_PART_FLAG_INTERNAL_ECHO: Int64 = 1024
comptime GEMINI_RESPONSE_PART_FLAG_SUPPRESS_VISIBLE_TEXT: Int64 = 2048
comptime GEMINI_RESPONSE_PART_FLAG_MASK: Int64 = 4095
comptime GEMINI_RESPONSE_PART_BATCH_MAX: Int64 = 65_536
comptime GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING: Int64 = 1
comptime GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS: Int64 = 2


comptime SIMPLE_OBJECT: UInt64 = 1
comptime SIMPLE_TEXT_PRESENT: UInt64 = 2
comptime SIMPLE_TEXT_STRING: UInt64 = 4
comptime SIMPLE_THOUGHT: UInt64 = 8
comptime SIMPLE_FUNCTION_PRESENT: UInt64 = 16
comptime SIMPLE_FUNCTION_OBJECT: UInt64 = 32
comptime SIMPLE_TEXT_KEYS: Int64 = 0
comptime SIMPLE_MEDIA_KEYS: Int64 = 1
comptime SIMPLE_FUNCTION_KEYS: Int64 = 2


@fieldwise_init
struct GeminiSimplePart(Copyable):
    var keys_address: UInt64
    var key_count: Int64
    var name: ProdexRichStringView
    var flags: UInt64


def gemini_simple_keys(part: GeminiSimplePart, mode: Int64) -> Bool:
    var keys = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(part.keys_address))
    for index in range(part.key_count):
        var key = keys[unsafe_offset=index].copy()
        if mode == SIMPLE_TEXT_KEYS:
            if not (rich_view_matches_literal["text"](key, False) or rich_view_matches_literal["thought"](key, False)):
                return False
        elif mode == SIMPLE_MEDIA_KEYS:
            if not (rich_view_matches_literal["inlineData"](key, False) or rich_view_matches_literal["inline_data"](key, False)
                or rich_view_matches_literal["fileData"](key, False) or rich_view_matches_literal["file_data"](key, False)
                or rich_view_matches_literal["executableCode"](key, False) or rich_view_matches_literal["codeExecutionResult"](key, False)
                or rich_view_matches_literal["videoMetadata"](key, False)):
                return False
        elif not (rich_view_matches_literal["functionCall"](key, False) or rich_view_matches_literal["thoughtSignature"](key, False)):
            return False
    return True


def gemini_simple_response_plan(
    version: Int64, parts_address: UInt, count: Int64, output_address: UInt, output_words: Int64,
) -> Int64:
    if version != 1:
        return 4
    if count < 0 or count > 0x7FFFFFFFFFFFFFFF // 40 or (count > 0 and parts_address == 0) or output_address == 0:
        return 1
    if output_words < count + 1:
        return 3
    var parts = Pointer[mut=False, GeminiSimplePart, ImmUntrackedOrigin](unsafe_from_address=Int(parts_address))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var message_only = True
    for index in range(count):
        var part = parts[unsafe_offset=index].copy()
        var flags = part.flags
        if flags > 63 or part.key_count < 0 or part.key_count > 0x7FFFFFFFFFFFFFFF // 16 or (part.key_count > 0 and part.keys_address == 0):
            return 1
        if ((flags & SIMPLE_OBJECT) == 0 and (flags != 0 or part.key_count != 0)) or ((flags & SIMPLE_TEXT_STRING) != 0 and (flags & SIMPLE_TEXT_PRESENT) == 0) or ((flags & SIMPLE_FUNCTION_OBJECT) != 0 and (flags & SIMPLE_FUNCTION_PRESENT) == 0):
            return 1
        if part.name.len > 0 and (flags & SIMPLE_FUNCTION_OBJECT) == 0:
            return 1
        if not rich_view_valid(part.name, 0x7FFFFFFFFFFFFFFF):
            return 2
        var keys = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(part.keys_address))
        for key in range(part.key_count):
            if not rich_view_valid(keys[unsafe_offset=key], 0x7FFFFFFFFFFFFFFF):
                return 2
        if (flags & SIMPLE_OBJECT) == 0 or (flags & SIMPLE_FUNCTION_PRESENT) != 0 or not gemini_simple_keys(part, SIMPLE_TEXT_KEYS if (flags & SIMPLE_TEXT_STRING) != 0 else SIMPLE_MEDIA_KEYS):
            message_only = False
    if message_only:
        output[] = 0
        return 0
    for index in range(count):
        var part = parts[unsafe_offset=index].copy()
        var flags = part.flags
        var action: Int64 = 0
        if (flags & SIMPLE_TEXT_PRESENT) != 0 and (flags & SIMPLE_THOUGHT) == 0:
            action = 0
        elif (flags & SIMPLE_FUNCTION_OBJECT) == 0:
            action = 1 if (flags & SIMPLE_OBJECT) != 0 and gemini_simple_keys(part, SIMPLE_TEXT_KEYS) else 0
        else:
            var name_bounds = rich_trim_bounds(part.name)
            if name_bounds[0] != name_bounds[1]:
                # Extra keys reject only after the host callback has observed this call.
                action = 2 if gemini_simple_keys(part, SIMPLE_FUNCTION_KEYS) else 3
        output[unsafe_offset=index + 1] = action
    output[] = 1
    return 0


def gemini_response_state_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def gemini_response_part_plan(
    has_text: Int64,
    is_thought: Int64,
    has_visible_text: Int64,
    has_special_text: Int64,
    has_media: Int64,
    has_video_metadata: Int64,
    has_image_generation: Int64,
    has_function_call: Int64,
    command_output_only: Int64,
    forced_output: Int64,
    internal_instruction_echo: Int64,
    suppress_visible_text: Int64,
    output_actions: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        not gemini_response_state_flag_valid(has_text)
        or not gemini_response_state_flag_valid(is_thought)
        or not gemini_response_state_flag_valid(has_visible_text)
        or not gemini_response_state_flag_valid(has_special_text)
        or not gemini_response_state_flag_valid(has_media)
        or not gemini_response_state_flag_valid(has_video_metadata)
        or not gemini_response_state_flag_valid(has_image_generation)
        or not gemini_response_state_flag_valid(has_function_call)
        or not gemini_response_state_flag_valid(command_output_only)
        or not gemini_response_state_flag_valid(forced_output)
        or not gemini_response_state_flag_valid(internal_instruction_echo)
        or not gemini_response_state_flag_valid(suppress_visible_text)
    ):
        return GEMINI_RESPONSE_STATE_STATUS_INVALID

    var actions: Int64 = 0
    if has_text == 1 and is_thought == 1:
        actions |= GEMINI_RESPONSE_PART_REASONING
    elif (
        has_visible_text == 1
        and command_output_only == 0
        and forced_output == 0
        and internal_instruction_echo == 0
        and suppress_visible_text == 0
    ):
        actions |= GEMINI_RESPONSE_PART_VISIBLE_TEXT

    if has_special_text == 1 and command_output_only == 0 and forced_output == 0:
        actions |= GEMINI_RESPONSE_PART_SPECIAL_TEXT
    if has_media == 1:
        actions |= GEMINI_RESPONSE_PART_MEDIA | GEMINI_RESPONSE_PART_NATIVE
    if has_video_metadata == 1:
        actions |= GEMINI_RESPONSE_PART_NATIVE
    if has_image_generation == 1:
        actions |= GEMINI_RESPONSE_PART_IMAGE
    if has_function_call == 1 and forced_output == 0:
        actions |= GEMINI_RESPONSE_PART_FUNCTION | GEMINI_RESPONSE_PART_FLUSH_PENDING

    output_actions[] = actions
    return GEMINI_RESPONSE_STATE_STATUS_OK


def gemini_response_part_batch_plan(
    abi_version: Int64,
    inputs_address: UInt,
    input_count: Int64,
    actions_address: UInt,
    function_flags_address: UInt,
    grounding_present: Int64,
    citations_present: Int64,
    tail_actions_address: UInt,
) -> Int64:
    if abi_version != GEMINI_RESPONSE_STATE_ABI_VERSION:
        return 4
    if (
        input_count < 0
        or input_count > GEMINI_RESPONSE_PART_BATCH_MAX
        or grounding_present < 0
        or grounding_present > 1
        or citations_present < 0
        or citations_present > 1
        or tail_actions_address == 0
        or input_count > 0
        and (inputs_address == 0 or actions_address == 0 or function_flags_address == 0)
    ):
        return GEMINI_RESPONSE_STATE_STATUS_INVALID

    var tail_actions: Int64 = 0
    if grounding_present == 1:
        tail_actions |= GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING
    if citations_present == 1:
        tail_actions |= GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS

    if input_count > 0:
        var inputs = Pointer[mut=False, Int64, ImmUntrackedOrigin](
            unsafe_from_address=Int(inputs_address)
        )
        var actions = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(actions_address)
        )
        var function_flags = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(function_flags_address)
        )
        var index: Int64 = 0
        while index < input_count:
            var flags = inputs[unsafe_offset=index]
            if flags < 0 or flags & (~GEMINI_RESPONSE_PART_FLAG_MASK) != 0:
                return GEMINI_RESPONSE_STATE_STATUS_INVALID
            var planned_actions: Int64 = 0
            var status = gemini_response_part_plan(
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_TEXT) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_IS_THOUGHT) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_VISIBLE_TEXT) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_SPECIAL_TEXT) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_MEDIA) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_VIDEO_METADATA) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_IMAGE_GENERATION) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_HAS_FUNCTION_CALL) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_COMMAND_OUTPUT_ONLY) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_FORCED_OUTPUT) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_INTERNAL_ECHO) != 0),
                Int64((flags & GEMINI_RESPONSE_PART_FLAG_SUPPRESS_VISIBLE_TEXT) != 0),
                Pointer(to=planned_actions),
            )
            if status != GEMINI_RESPONSE_STATE_STATUS_OK:
                return status
            actions[unsafe_offset=index] = planned_actions
            function_flags[unsafe_offset=index] = (
                49 if (flags & GEMINI_RESPONSE_PART_FLAG_HAS_FUNCTION_CALL) != 0 else 48
            )
            index += 1

    var tail = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(tail_actions_address)
    )
    tail[] = tail_actions
    return GEMINI_RESPONSE_STATE_STATUS_OK


comptime GEMINI_COMPLETION_GUARDRAIL_NONE: Int64 = 0
comptime GEMINI_COMPLETION_GUARDRAIL_EMPTY: Int64 = 1
comptime GEMINI_COMPLETION_GUARDRAIL_TOOL_INTENT: Int64 = 2
comptime GEMINI_COMPLETION_GUARDRAIL_WAIT: Int64 = 3
comptime GEMINI_COMPLETION_GUARDRAIL_UNVERIFIED_SUCCESS: Int64 = 4


def gemini_response_once_event_plan(
    already_emitted: Int64,
    content_present: Int64,
    requires_content: Int64,
    output_emit: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        not gemini_response_state_flag_valid(already_emitted)
        or not gemini_response_state_flag_valid(content_present)
        or not gemini_response_state_flag_valid(requires_content)
    ):
        return GEMINI_RESPONSE_STATE_STATUS_INVALID
    output_emit[] = Int64(
        already_emitted == 0
        and (requires_content == 0 or content_present == 1)
    )
    return GEMINI_RESPONSE_STATE_STATUS_OK


def gemini_completion_guardrail_plan(
    output_is_empty: Int64,
    reasoning_is_empty: Int64,
    tool_calls_empty: Int64,
    tool_intent_present: Int64,
    wait_or_poll_present: Int64,
    unverified_success: Int64,
    output_action: Pointer[mut=True, Int64, _],
) -> Int64:
    for value in [
        output_is_empty,
        reasoning_is_empty,
        tool_calls_empty,
        tool_intent_present,
        wait_or_poll_present,
        unverified_success,
    ]:
        if not gemini_response_state_flag_valid(value):
            return GEMINI_RESPONSE_STATE_STATUS_INVALID

    var action = GEMINI_COMPLETION_GUARDRAIL_NONE
    if output_is_empty == 1 and reasoning_is_empty == 1:
        action = GEMINI_COMPLETION_GUARDRAIL_EMPTY
    elif tool_calls_empty == 1 and tool_intent_present == 1:
        action = GEMINI_COMPLETION_GUARDRAIL_TOOL_INTENT
    elif tool_calls_empty == 1 and wait_or_poll_present == 1:
        action = GEMINI_COMPLETION_GUARDRAIL_WAIT
    elif tool_calls_empty == 1 and unverified_success == 1:
        action = GEMINI_COMPLETION_GUARDRAIL_UNVERIFIED_SUCCESS
    output_action[] = action
    return GEMINI_RESPONSE_STATE_STATUS_OK
