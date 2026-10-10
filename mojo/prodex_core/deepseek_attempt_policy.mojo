# DeepSeek pre-commit attempt and first-event retry policy.
#
# Rust supplies bounded attempt facts and applies the selected effect.  This
# kernel owns the retry budget and the model-before-credential precedence.

comptime DEEPSEEK_ATTEMPT_ABI_VERSION: Int64 = 1

comptime DEEPSEEK_ATTEMPT_RETURN: Int64 = 0
comptime DEEPSEEK_ATTEMPT_NEXT_MODEL: Int64 = 1
comptime DEEPSEEK_ATTEMPT_NEXT_CREDENTIAL: Int64 = 2

comptime DEEPSEEK_ATTEMPT_NATIVE_FIRST_EVENT: Int64 = 0
comptime DEEPSEEK_ATTEMPT_ERROR: Int64 = 1

def deepseek_attempt_bool_is_valid(value: Int64) -> Bool:
    return value == 0 or value == 1

def deepseek_attempt_index_is_valid(index: Int64, count: Int64) -> Bool:
    return count > 0 and index >= 0 and index < count

@export("prodex_deepseek_first_event_retry_allowed_v1")
def prodex_deepseek_first_event_retry_allowed_v1(
    abi_version: Int64,
    attempted_retries: Int64,
    first_event_committed: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_ATTEMPT_ABI_VERSION:
        return -4
    if attempted_retries < 0 or not deepseek_attempt_bool_is_valid(first_event_committed):
        return -1
    # DeepSeek permits one pre-commit first-event recovery.  Once output has
    # committed, transport transparency forbids rotating the stream.
    return Int64(first_event_committed == 0 and attempted_retries < 1)

@export("prodex_deepseek_attempt_action_v1")
def prodex_deepseek_attempt_action_v1(
    abi_version: Int64,
    attempt_kind: Int64,
    attempted_first_event_retries: Int64,
    first_event_committed: Int64,
    model_index: Int64,
    model_count: Int64,
    credential_index: Int64,
    credential_count: Int64,
    model_retry_allowed: Int64,
    credential_retry_allowed: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_ATTEMPT_ABI_VERSION:
        return -4
    if (
        (attempt_kind != DEEPSEEK_ATTEMPT_NATIVE_FIRST_EVENT and attempt_kind != DEEPSEEK_ATTEMPT_ERROR)
        or attempted_first_event_retries < 0
        or not deepseek_attempt_bool_is_valid(first_event_committed)
        or not deepseek_attempt_index_is_valid(model_index, model_count)
        or not deepseek_attempt_index_is_valid(credential_index, credential_count)
        or not deepseek_attempt_bool_is_valid(model_retry_allowed)
        or not deepseek_attempt_bool_is_valid(credential_retry_allowed)
    ):
        return -1

    if attempt_kind == DEEPSEEK_ATTEMPT_NATIVE_FIRST_EVENT:
        if first_event_committed == 1 or attempted_first_event_retries >= 1:
            return DEEPSEEK_ATTEMPT_RETURN
        if model_retry_allowed == 1 and model_index + 1 < model_count:
            return DEEPSEEK_ATTEMPT_NEXT_MODEL
        if credential_retry_allowed == 1 and credential_index + 1 < credential_count:
            return DEEPSEEK_ATTEMPT_NEXT_CREDENTIAL
        return DEEPSEEK_ATTEMPT_RETURN

    if model_retry_allowed == 1 and model_index + 1 < model_count:
        return DEEPSEEK_ATTEMPT_NEXT_MODEL
    if credential_retry_allowed == 1 and credential_index + 1 < credential_count:
        return DEEPSEEK_ATTEMPT_NEXT_CREDENTIAL
    return DEEPSEEK_ATTEMPT_RETURN
