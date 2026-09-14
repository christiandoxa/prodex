from std.memory import Pointer


comptime APPLICATION_DATA_PLANE_ABI_VERSION: Int64 = 1
comptime STATUS_OK: Int64 = 0
comptime STATUS_INVALID: Int64 = 1
comptime STATUS_ABI: Int64 = 4

comptime ROUTE_RESPONSES: Int64 = 0
comptime ROUTE_COMPACT: Int64 = 1
comptime ROUTE_WEBSOCKET: Int64 = 2
comptime ROUTE_QUOTA: Int64 = 3
comptime ROUTE_CHAT_COMPLETIONS: Int64 = 4
comptime ROUTE_EMBEDDINGS: Int64 = 5
comptime ROUTE_IMAGES_GENERATIONS: Int64 = 6
comptime ROUTE_IMAGES_EDITS: Int64 = 7
comptime ROUTE_IMAGES_VARIATIONS: Int64 = 8
comptime ROUTE_AUDIO_SPEECH: Int64 = 9
comptime ROUTE_AUDIO_TRANSCRIPTIONS: Int64 = 10
comptime ROUTE_AUDIO_TRANSLATIONS: Int64 = 11
comptime ROUTE_BATCHES: Int64 = 12
comptime ROUTE_BATCH: Int64 = 13
comptime ROUTE_RERANK: Int64 = 14
comptime ROUTE_A2A: Int64 = 15
comptime ROUTE_MESSAGES: Int64 = 16
comptime ROUTE_MODELS: Int64 = 17
comptime ROUTE_MODEL: Int64 = 18
comptime ROUTE_CONTROL_PLANE: Int64 = 19
comptime ROUTE_HEALTH_LIVE: Int64 = 20
comptime ROUTE_HEALTH_READY: Int64 = 21
comptime ROUTE_HEALTH_STARTUP: Int64 = 22
comptime ROUTE_UNKNOWN: Int64 = 23

comptime RUNTIME_ROUTE_RESPONSES: Int64 = 0
comptime RUNTIME_ROUTE_COMPACT: Int64 = 1
comptime RUNTIME_ROUTE_WEBSOCKET: Int64 = 2
comptime RUNTIME_ROUTE_STANDARD: Int64 = 3

comptime ENDPOINT_NONE: Int64 = -1
comptime ENDPOINT_RESPONSES: Int64 = 0
comptime ENDPOINT_RESPONSES_COMPACT: Int64 = 1
comptime ENDPOINT_CHAT_COMPLETIONS: Int64 = 2
comptime ENDPOINT_MESSAGES: Int64 = 3
comptime ENDPOINT_MODELS: Int64 = 4
comptime ENDPOINT_EMBEDDINGS: Int64 = 5
comptime ENDPOINT_IMAGES: Int64 = 6
comptime ENDPOINT_AUDIO: Int64 = 7
comptime ENDPOINT_BATCHES: Int64 = 8
comptime ENDPOINT_RERANK: Int64 = 9
comptime ENDPOINT_A2A: Int64 = 10

comptime ACTION_INVOKE_MODEL: Int64 = 0
comptime ACTION_UPLOAD_CONTENT: Int64 = 2
comptime ACTION_COMPACT_CONTEXT: Int64 = 3

comptime CAPABILITY_RESPONSES_API: UInt64 = 1 << 0
comptime CAPABILITY_STREAMING: UInt64 = 1 << 1
comptime CAPABILITY_TOOLS: UInt64 = 1 << 2
comptime CAPABILITY_VISION: UInt64 = 1 << 3
comptime CAPABILITY_JSON_MODE: UInt64 = 1 << 4
comptime CAPABILITY_REMOTE_COMPACT: UInt64 = 1 << 5
comptime CAPABILITY_WEBSOCKET: UInt64 = 1 << 6

comptime MODALITY_TEXT: UInt64 = 1 << 0
comptime MODALITY_IMAGE: UInt64 = 1 << 1
comptime MODALITY_AUDIO: UInt64 = 1 << 2
comptime MODALITY_FILE: UInt64 = 1 << 4

comptime PROVIDER_GEMINI: Int64 = 4
comptime CAPABILITY_STATUS_PARTIAL: Int64 = 4
comptime CAPABILITY_STATUS_UNTESTED: Int64 = 6

comptime QUOTA_READY: Int64 = 0
comptime QUOTA_THIN: Int64 = 1
comptime QUOTA_CRITICAL: Int64 = 2
comptime QUOTA_EXHAUSTED: Int64 = 3
comptime QUOTA_UNKNOWN: Int64 = 4

comptime DISPATCH_STOP: Int64 = 0
comptime DISPATCH_SKIP: Int64 = 1
comptime DISPATCH_PRIMARY: Int64 = 2
comptime DISPATCH_FALLBACK: Int64 = 3

comptime ATTEMPT_SUCCESS: Int64 = 0
comptime ATTEMPT_RETRY: Int64 = 1
comptime ATTEMPT_STOP: Int64 = 2

comptime BINDING_VALID: Int64 = 0
comptime BINDING_MISSING_IDENTITY: Int64 = 1
comptime BINDING_PROVIDER_MISMATCH: Int64 = 2
comptime BINDING_IDENTITY_MISMATCH: Int64 = 3
comptime BINDING_SELECTED_IDENTITY_REQUIRED: Int64 = 4

comptime GOVERNANCE_DISPATCH_ALLOWED: Int64 = 0
comptime GOVERNANCE_DISPATCH_IDENTITY_REQUIRED: Int64 = 1
comptime GOVERNANCE_DISPATCH_UNAVAILABLE: Int64 = 2


@fieldwise_init
struct ApplicationRouteRequestPlanResult(Copyable):
    var abi_version: Int64
    var runtime_route: Int64
    var endpoint: Int64
    var governance_route: Int64
    var governed_action: Int64
    var capability_mask: UInt64
    var modality_mask: UInt64
    var stream_mode: Int64
    var buffered_response_inspectable: Int64
    var compact_dispatch: Int64
    var models_dispatch: Int64


@fieldwise_init
struct ApplicationQuotaHeadroomResult(Copyable):
    var abi_version: Int64
    var present: Int64
    var headroom: Int64


@fieldwise_init
struct ApplicationCandidatePlanResult(Copyable):
    var abi_version: Int64
    var candidate_count: Int64
    var attempt_action: Int64


@fieldwise_init
struct ApplicationPipelineErrorPlanResult(Copyable):
    var abi_version: Int64
    var status: Int64
    var response_kind: Int64


@fieldwise_init
struct ApplicationOperationalProbePlanResult(Copyable):
    var abi_version: Int64
    var method_allowed: Int64
    var probe_kind: Int64
    var ready: Int64
    var status_reason: Int64
    var metric_result: Int64
    var http_status: Int64
    var omit_body: Int64


def valid_flag(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_mojo_application_operational_probe_plan_v1")
def prodex_mojo_application_operational_probe_plan_v1(
    abi_version: Int64,
    route: Int64,
    method: Int64,
    overloaded: Int64,
    draining: Int64,
    credentials_stale: Int64,
    governance_audit_available: Int64,
    governance_policy_available: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        route < ROUTE_RESPONSES
        or route > ROUTE_UNKNOWN
        or method < 0
        or method > 2
        or not valid_flag(overloaded)
        or not valid_flag(draining)
        or not valid_flag(credentials_stale)
        or not valid_flag(governance_audit_available)
        or not valid_flag(governance_policy_available)
        or result_address == 0
    ):
        return STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationOperationalProbePlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = APPLICATION_DATA_PLANE_ABI_VERSION
    var handled = Int64(
        route == ROUTE_HEALTH_LIVE
        or route == ROUTE_HEALTH_READY
        or route == ROUTE_HEALTH_STARTUP
    )
    result[].method_allowed = Int64(method != 2)
    result[].probe_kind = route - ROUTE_HEALTH_LIVE
    result[].ready = 0
    result[].status_reason = 0
    result[].metric_result = 0
    result[].http_status = 200
    result[].omit_body = Int64(method == 1)
    if handled == 0:
        result[].probe_kind = -1
        result[].method_allowed = 0
        result[].omit_body = 0
        return STATUS_OK
    if result[].method_allowed == 0:
        result[].status_reason = 6
        result[].http_status = 405
        return STATUS_OK
    result[].ready = Int64(
        route != ROUTE_HEALTH_READY
        or (
            overloaded == 0
            and draining == 0
            and credentials_stale == 0
            and governance_audit_available == 1
            and governance_policy_available == 1
        )
    )
    if result[].ready == 0:
        result[].http_status = 503
    if result[].ready == 1:
        result[].status_reason = 0
    elif draining == 1:
        result[].status_reason = 1
    elif credentials_stale == 1:
        result[].status_reason = 2
    elif governance_audit_available == 0:
        result[].status_reason = 3
    elif governance_policy_available == 0:
        result[].status_reason = 4
    else:
        result[].status_reason = 5
    if draining == 1:
        result[].metric_result = 1
    elif (
        overloaded == 1
        or credentials_stale == 1
        or governance_audit_available == 0
        or governance_policy_available == 0
    ):
        result[].metric_result = 2
    return STATUS_OK


@export("prodex_mojo_application_pipeline_error_plan_v1")
def prodex_mojo_application_pipeline_error_plan_v1(
    abi_version: Int64,
    error_kind: Int64,
    gateway_error_status: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        error_kind < 0
        or error_kind > 1
        or gateway_error_status < -1
        or gateway_error_status > 4
        or result_address == 0
    ):
        return STATUS_INVALID
    if (
        (error_kind == 0 and gateway_error_status != -1)
        or (error_kind == 1 and gateway_error_status == -1)
    ):
        return STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationPipelineErrorPlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = APPLICATION_DATA_PLANE_ABI_VERSION
    result[].response_kind = error_kind
    if error_kind == 0:
        result[].status = 404
    elif gateway_error_status == 0:
        result[].status = 400
    elif gateway_error_status == 1:
        result[].status = 405
    elif gateway_error_status == 2:
        result[].status = 413
    elif gateway_error_status == 3:
        result[].status = 431
    else:
        result[].status = 500
    return STATUS_OK


def route_is_image(route: Int64) -> Bool:
    return (
        route == ROUTE_IMAGES_GENERATIONS
        or route == ROUTE_IMAGES_EDITS
        or route == ROUTE_IMAGES_VARIATIONS
    )


def route_is_audio(route: Int64) -> Bool:
    return (
        route == ROUTE_AUDIO_SPEECH
        or route == ROUTE_AUDIO_TRANSCRIPTIONS
        or route == ROUTE_AUDIO_TRANSLATIONS
    )


def route_has_file_input(route: Int64) -> Bool:
    return (
        route == ROUTE_IMAGES_EDITS
        or route == ROUTE_IMAGES_VARIATIONS
        or route == ROUTE_AUDIO_TRANSCRIPTIONS
        or route == ROUTE_AUDIO_TRANSLATIONS
    )


def route_is_provider_data_plane(route: Int64) -> Bool:
    return route >= ROUTE_RESPONSES and route <= ROUTE_MODEL and route != ROUTE_QUOTA


def application_runtime_route(route: Int64) -> Int64:
    if route == ROUTE_RESPONSES:
        return RUNTIME_ROUTE_RESPONSES
    if route == ROUTE_COMPACT:
        return RUNTIME_ROUTE_COMPACT
    if route == ROUTE_WEBSOCKET:
        return RUNTIME_ROUTE_WEBSOCKET
    return RUNTIME_ROUTE_STANDARD


def application_provider_endpoint(route: Int64) -> Int64:
    if route == ROUTE_RESPONSES or route == ROUTE_WEBSOCKET:
        return ENDPOINT_RESPONSES
    if route == ROUTE_COMPACT:
        return ENDPOINT_RESPONSES_COMPACT
    if route == ROUTE_CHAT_COMPLETIONS:
        return ENDPOINT_CHAT_COMPLETIONS
    if route == ROUTE_MESSAGES:
        return ENDPOINT_MESSAGES
    if route == ROUTE_MODELS or route == ROUTE_MODEL:
        return ENDPOINT_MODELS
    if route == ROUTE_EMBEDDINGS:
        return ENDPOINT_EMBEDDINGS
    if route_is_image(route):
        return ENDPOINT_IMAGES
    if route_is_audio(route):
        return ENDPOINT_AUDIO
    if route == ROUTE_BATCHES or route == ROUTE_BATCH:
        return ENDPOINT_BATCHES
    if route == ROUTE_RERANK:
        return ENDPOINT_RERANK
    if route == ROUTE_A2A:
        return ENDPOINT_A2A
    return ENDPOINT_NONE


def application_governed_action(route: Int64) -> Int64:
    if route == ROUTE_COMPACT:
        return ACTION_COMPACT_CONTEXT
    if route_has_file_input(route):
        return ACTION_UPLOAD_CONTENT
    return ACTION_INVOKE_MODEL


def application_capability_mask(
    route: Int64, streaming: Int64, tools_present: Int64, vision_required: Int64
) -> UInt64:
    var capabilities: UInt64 = 0
    if (
        route == ROUTE_RESPONSES
        or route == ROUTE_COMPACT
        or route == ROUTE_WEBSOCKET
    ):
        capabilities |= CAPABILITY_RESPONSES_API
    if route == ROUTE_COMPACT:
        capabilities |= CAPABILITY_REMOTE_COMPACT
    if streaming == 1:
        capabilities |= CAPABILITY_STREAMING
    if tools_present == 1:
        capabilities |= CAPABILITY_TOOLS
    if route_is_image(route) or vision_required == 1:
        capabilities |= CAPABILITY_VISION
    if route == ROUTE_WEBSOCKET:
        capabilities |= CAPABILITY_WEBSOCKET
    return capabilities


def application_modality_mask(route: Int64, capabilities: UInt64) -> UInt64:
    var modalities = MODALITY_TEXT
    if route_is_image(route) or capabilities & CAPABILITY_VISION != 0:
        modalities |= MODALITY_IMAGE
    if route_is_audio(route):
        modalities |= MODALITY_AUDIO
    if route_has_file_input(route):
        modalities |= MODALITY_FILE
    return modalities


@export("prodex_mojo_application_route_request_plan_v1")
def prodex_mojo_application_route_request_plan_v1(
    abi_version: Int64,
    route: Int64,
    streaming: Int64,
    tools_present: Int64,
    vision_required: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        route < ROUTE_RESPONSES
        or route > ROUTE_UNKNOWN
        or not valid_flag(streaming)
        or not valid_flag(tools_present)
        or not valid_flag(vision_required)
        or result_address == 0
    ):
        return STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationRouteRequestPlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    var capabilities = application_capability_mask(
        route, streaming, tools_present, vision_required
    )
    result[].abi_version = APPLICATION_DATA_PLANE_ABI_VERSION
    result[].runtime_route = application_runtime_route(route)
    result[].endpoint = application_provider_endpoint(route)
    result[].governance_route = route if route_is_provider_data_plane(route) else -1
    result[].governed_action = application_governed_action(route)
    result[].capability_mask = capabilities
    result[].modality_mask = application_modality_mask(route, capabilities)
    result[].stream_mode = streaming
    result[].buffered_response_inspectable = Int64(
        not route_is_image(route) and not route_is_audio(route)
    )
    result[].compact_dispatch = Int64(route == ROUTE_COMPACT)
    result[].models_dispatch = Int64(route == ROUTE_MODELS or route == ROUTE_MODEL)
    return STATUS_OK


def capability_status_is_executable(status: Int64) -> Bool:
    return status >= 0 and status <= CAPABILITY_STATUS_PARTIAL


@export("prodex_mojo_application_capability_status_executable_v1")
def prodex_mojo_application_capability_status_executable_v1(
    abi_version: Int64,
    status: Int64,
    executable_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        status < 0
        or status > CAPABILITY_STATUS_UNTESTED
        or executable_address == 0
    ):
        return STATUS_INVALID
    var executable = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(executable_address))
    executable[] = Int64(capability_status_is_executable(status))
    return STATUS_OK


@export("prodex_mojo_application_provider_capabilities_v1")
def prodex_mojo_application_provider_capabilities_v1(
    abi_version: Int64,
    provider: Int64,
    response_status: Int64,
    compact_status: Int64,
    image_status: Int64,
    supports_streaming: Int64,
    catalog_vision: Int64,
    catalog_tools: Int64,
    catalog_json_mode: Int64,
    capability_mask_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        provider < 0
        or provider > 6
        or response_status < 0
        or response_status > CAPABILITY_STATUS_UNTESTED
        or compact_status < 0
        or compact_status > CAPABILITY_STATUS_UNTESTED
        or image_status < 0
        or image_status > CAPABILITY_STATUS_UNTESTED
        or not valid_flag(supports_streaming)
        or not valid_flag(catalog_vision)
        or not valid_flag(catalog_tools)
        or not valid_flag(catalog_json_mode)
        or capability_mask_address == 0
    ):
        return STATUS_INVALID
    var capabilities = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(capability_mask_address))
    capabilities[] = 0
    var response_executable = capability_status_is_executable(response_status)
    if response_executable:
        capabilities[] |= CAPABILITY_RESPONSES_API
        if supports_streaming == 1:
            capabilities[] |= CAPABILITY_STREAMING
    if capability_status_is_executable(compact_status):
        capabilities[] |= CAPABILITY_REMOTE_COMPACT
    if capability_status_is_executable(image_status) or catalog_vision == 1:
        capabilities[] |= CAPABILITY_VISION
    if catalog_tools == 1:
        capabilities[] |= CAPABILITY_TOOLS
    if catalog_json_mode == 1:
        capabilities[] |= CAPABILITY_JSON_MODE
    if provider == PROVIDER_GEMINI:
        capabilities[] |= CAPABILITY_WEBSOCKET
    return STATUS_OK


@export("prodex_mojo_application_normalized_load_v1")
def prodex_mojo_application_normalized_load_v1(
    abi_version: Int64,
    active: UInt64,
    limit: UInt64,
    scale: UInt64,
    load_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if scale == 0 or load_address == 0:
        return STATUS_INVALID
    var load = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(load_address))
    if limit == 0 or active >= limit:
        load[] = scale
    elif active > 18446744073709551615 // scale:
        load[] = scale
    else:
        load[] = min((active * scale) // limit, scale)
    return STATUS_OK


@export("prodex_mojo_application_quota_headroom_v1")
def prodex_mojo_application_quota_headroom_v1(
    abi_version: Int64,
    status: Int64,
    remaining_percent: Int64,
    reset_at: Int64,
    now: Int64,
    scale: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        status < QUOTA_READY
        or status > QUOTA_UNKNOWN
        or scale <= 0
        or result_address == 0
    ):
        return STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationQuotaHeadroomResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result[].abi_version = APPLICATION_DATA_PLANE_ABI_VERSION
    result[].present = 0
    result[].headroom = 0
    if status == QUOTA_READY or status == QUOTA_THIN or status == QUOTA_CRITICAL:
        var bounded_remaining = remaining_percent
        if bounded_remaining < 0:
            bounded_remaining = 0
        elif bounded_remaining > 100:
            bounded_remaining = 100
        result[].present = 1
        result[].headroom = bounded_remaining * (scale // 100)
    elif status == QUOTA_EXHAUSTED and reset_at > now:
        result[].present = 1
    return STATUS_OK


@export("prodex_mojo_application_output_tokens_v1")
def prodex_mojo_application_output_tokens_v1(
    abi_version: Int64,
    max_output_present: Int64,
    max_output_tokens: UInt64,
    max_completion_present: Int64,
    max_completion_tokens: UInt64,
    max_tokens_present: Int64,
    max_tokens: UInt64,
    selected_present_address: UInt,
    selected_tokens_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        not valid_flag(max_output_present)
        or not valid_flag(max_completion_present)
        or not valid_flag(max_tokens_present)
        or selected_present_address == 0
        or selected_tokens_address == 0
    ):
        return STATUS_INVALID
    var selected_present = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(selected_present_address))
    var selected_tokens = Pointer[
        mut=True, UInt64, MutUntrackedOrigin
    ](unsafe_from_address=Int(selected_tokens_address))
    selected_present[] = 0
    selected_tokens[] = 0
    if max_output_present == 1 and max_output_tokens <= 4294967295:
        selected_present[] = 1
        selected_tokens[] = max_output_tokens
    elif max_completion_present == 1 and max_completion_tokens <= 4294967295:
        selected_present[] = 1
        selected_tokens[] = max_completion_tokens
    elif max_tokens_present == 1 and max_tokens <= 4294967295:
        selected_present[] = 1
        selected_tokens[] = max_tokens
    return STATUS_OK


def dispatch_candidate_count(hard_continuation: Int64, fallback_count: Int64) -> Int64:
    if hard_continuation == 1:
        return 1
    return fallback_count + 1


def dispatch_attempt_action(
    attempt_index: Int64, candidate_count: Int64, primary_available: Int64
) -> Int64:
    if attempt_index < 0 or attempt_index >= candidate_count:
        return DISPATCH_STOP
    if attempt_index == 0 and primary_available == 0:
        return DISPATCH_SKIP
    if attempt_index == 0:
        return DISPATCH_PRIMARY
    return DISPATCH_FALLBACK


def dispatch_result_action(
    transport_succeeded: Int64, retryable_response: Int64, retry_allowed: Int64
) -> Int64:
    if retry_allowed == 1 and (
        transport_succeeded == 0 or retryable_response == 1
    ):
        return ATTEMPT_RETRY
    if transport_succeeded == 1:
        return ATTEMPT_SUCCESS
    return ATTEMPT_STOP


def dispatch_binding_decision(
    continuation_bound: Int64,
    bound_identity_present: Int64,
    provider_matches: Int64,
    selected_identity_present: Int64,
    identity_matches: Int64,
    selected_identity_optional: Int64,
) -> Int64:
    if continuation_bound == 0:
        return BINDING_VALID
    if bound_identity_present == 0:
        return BINDING_MISSING_IDENTITY
    if provider_matches == 0:
        return BINDING_PROVIDER_MISMATCH
    if selected_identity_present == 1:
        if identity_matches == 1:
            return BINDING_VALID
        return BINDING_IDENTITY_MISMATCH
    if selected_identity_optional == 0:
        return BINDING_SELECTED_IDENTITY_REQUIRED
    return BINDING_VALID


def dispatch_governance_decision(
    tenant_bound: Int64,
    anonymous_compatibility_allowed: Int64,
    enforcing: Int64,
    mandatory_audit: Int64,
    policy_allows: Int64,
    routing_present: Int64,
) -> Int64:
    if tenant_bound == 0:
        if anonymous_compatibility_allowed == 1:
            return GOVERNANCE_DISPATCH_ALLOWED
        return GOVERNANCE_DISPATCH_IDENTITY_REQUIRED
    if enforcing == 1 and (
        mandatory_audit == 0 or policy_allows == 0 or routing_present == 0
    ):
        return GOVERNANCE_DISPATCH_UNAVAILABLE
    return GOVERNANCE_DISPATCH_ALLOWED


@export("prodex_mojo_application_candidate_plan_v1")
def prodex_mojo_application_candidate_plan_v1(
    abi_version: Int64,
    hard_continuation: Int64,
    fallback_count: Int64,
    attempt_index: Int64,
    primary_available: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        not valid_flag(hard_continuation)
        or fallback_count < 0
        or fallback_count > 255
        or not valid_flag(primary_available)
        or result_address == 0
    ):
        return STATUS_INVALID
    var result = Pointer[
        mut=True, ApplicationCandidatePlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    var candidate_count = dispatch_candidate_count(hard_continuation, fallback_count)
    result[].abi_version = APPLICATION_DATA_PLANE_ABI_VERSION
    result[].candidate_count = candidate_count
    result[].attempt_action = dispatch_attempt_action(
        attempt_index, candidate_count, primary_available
    )
    return STATUS_OK


@export("prodex_mojo_application_attempt_result_plan_v1")
def prodex_mojo_application_attempt_result_plan_v1(
    abi_version: Int64,
    transport_succeeded: Int64,
    retryable_response: Int64,
    retry_allowed: Int64,
    result_action_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if (
        not valid_flag(transport_succeeded)
        or not valid_flag(retryable_response)
        or not valid_flag(retry_allowed)
        or result_action_address == 0
    ):
        return STATUS_INVALID
    var result_action = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_action_address))
    result_action[] = dispatch_result_action(
        transport_succeeded, retryable_response, retry_allowed
    )
    return STATUS_OK


@export("prodex_mojo_application_binding_plan_v1")
def prodex_mojo_application_binding_plan_v1(
    abi_version: Int64,
    continuation_bound: Int64,
    bound_identity_present: Int64,
    provider_matches: Int64,
    selected_identity_present: Int64,
    identity_matches: Int64,
    selected_identity_optional: Int64,
    binding_decision_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if binding_decision_address == 0:
        return STATUS_INVALID
    if (
        not valid_flag(continuation_bound)
        or not valid_flag(bound_identity_present)
        or not valid_flag(provider_matches)
        or not valid_flag(selected_identity_present)
        or not valid_flag(identity_matches)
        or not valid_flag(selected_identity_optional)
    ):
        return STATUS_INVALID
    var binding_decision = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(binding_decision_address))
    binding_decision[] = dispatch_binding_decision(
        continuation_bound,
        bound_identity_present,
        provider_matches,
        selected_identity_present,
        identity_matches,
        selected_identity_optional,
    )
    return STATUS_OK


@export("prodex_mojo_application_governance_dispatch_plan_v1")
def prodex_mojo_application_governance_dispatch_plan_v1(
    abi_version: Int64,
    tenant_bound: Int64,
    anonymous_compatibility_allowed: Int64,
    enforcing: Int64,
    mandatory_audit: Int64,
    policy_allows: Int64,
    routing_present: Int64,
    governance_decision_address: UInt,
) abi("C") -> Int64:
    if abi_version != APPLICATION_DATA_PLANE_ABI_VERSION:
        return STATUS_ABI
    if governance_decision_address == 0:
        return STATUS_INVALID
    if (
        not valid_flag(tenant_bound)
        or not valid_flag(anonymous_compatibility_allowed)
        or not valid_flag(enforcing)
        or not valid_flag(mandatory_audit)
        or not valid_flag(policy_allows)
        or not valid_flag(routing_present)
    ):
        return STATUS_INVALID
    var governance_decision = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(governance_decision_address))
    governance_decision[] = dispatch_governance_decision(
        tenant_bound,
        anonymous_compatibility_allowed,
        enforcing,
        mandatory_audit,
        policy_allows,
        routing_present,
    )
    return STATUS_OK
