from std.memory import Pointer


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
