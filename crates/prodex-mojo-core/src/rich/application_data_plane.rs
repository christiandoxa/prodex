use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimeRoute {
    Responses,
    Compact,
    WebSocket,
    Standard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderEndpoint {
    Responses,
    ResponsesCompact,
    ChatCompletions,
    Messages,
    Models,
    Embeddings,
    Images,
    Audio,
    Batches,
    Rerank,
    A2a,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationGovernedAction {
    InvokeModel,
    UploadContent,
    CompactContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationRouteRequestPlan {
    pub runtime_route: ApplicationRuntimeRoute,
    pub endpoint: Option<ApplicationProviderEndpoint>,
    pub governance_route: Option<ApplicationRouteKind>,
    pub governed_action: ApplicationGovernedAction,
    pub capability_mask: u64,
    pub modality_mask: u64,
    pub streaming: bool,
    pub buffered_response_inspectable: bool,
    pub compact_dispatch: bool,
    pub models_dispatch: bool,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderKind {
    OpenAi = 0,
    Anthropic = 1,
    Copilot = 2,
    DeepSeek = 3,
    Gemini = 4,
    Kiro = 5,
    Local = 6,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCapabilityStatus {
    Native = 0,
    Translated = 1,
    Passthrough = 2,
    Emulated = 3,
    Partial = 4,
    Unsupported = 5,
    Untested = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCapabilitiesInput {
    pub provider: ApplicationProviderKind,
    pub responses: ApplicationProviderCapabilityStatus,
    pub compact: ApplicationProviderCapabilityStatus,
    pub images: ApplicationProviderCapabilityStatus,
    pub supports_streaming: bool,
    pub catalog_vision: bool,
    pub catalog_tools: bool,
    pub catalog_json_mode: bool,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationQuotaWindowStatus {
    Ready = 0,
    Thin = 1,
    Critical = 2,
    Exhausted = 3,
    Unknown = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationCandidateAttempt {
    Stop,
    Skip,
    Primary,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationCandidatePlan {
    pub candidate_count: usize,
    pub attempt: ApplicationCandidateAttempt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAttemptResult {
    Success,
    Retry,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBindingDecision {
    Valid,
    MissingIdentity,
    ProviderMismatch,
    IdentityMismatch,
    SelectedIdentityRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationBindingInput {
    pub continuation_bound: bool,
    pub bound_identity_present: bool,
    pub provider_matches: bool,
    pub selected_identity_present: bool,
    pub identity_matches: bool,
    pub selected_identity_optional: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationGovernanceDispatchDecision {
    Allowed,
    IdentityRequired,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationGovernanceDispatchInput {
    pub tenant_bound: bool,
    pub anonymous_compatibility_allowed: bool,
    pub enforcing: bool,
    pub mandatory_audit: bool,
    pub policy_allows: bool,
    pub routing_present: bool,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationGatewayErrorStatus {
    BadRequest = 0,
    MethodNotAllowed = 1,
    PayloadTooLarge = 2,
    RequestHeaderFieldsTooLarge = 3,
    InternalServerError = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationPipelineErrorInput {
    UnknownRoute,
    Gateway(ApplicationGatewayErrorStatus),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationPipelineErrorPlan {
    pub status: u16,
    pub gateway_response: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationRouteRequestPlanResult {
    abi_version: i64,
    runtime_route: i64,
    endpoint: i64,
    governance_route: i64,
    governed_action: i64,
    capability_mask: u64,
    modality_mask: u64,
    stream_mode: i64,
    buffered_response_inspectable: i64,
    compact_dispatch: i64,
    models_dispatch: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationQuotaHeadroomResult {
    abi_version: i64,
    present: i64,
    headroom: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationCandidatePlanResult {
    abi_version: i64,
    candidate_count: i64,
    attempt_action: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationPipelineErrorPlanResult {
    abi_version: i64,
    status: i64,
    response_kind: i64,
}

const _: () = assert!(std::mem::size_of::<ApplicationRouteRequestPlanResult>() == 88);
const _: () = assert!(std::mem::size_of::<ApplicationQuotaHeadroomResult>() == 24);
const _: () = assert!(std::mem::size_of::<ApplicationCandidatePlanResult>() == 24);
const _: () = assert!(std::mem::size_of::<ApplicationPipelineErrorPlanResult>() == 24);

unsafe extern "C" {
    fn prodex_mojo_application_pipeline_error_plan_v1(
        abi_version: i64,
        error_kind: i64,
        gateway_error_status: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_application_route_request_plan_v1(
        abi_version: i64,
        route: i64,
        streaming: i64,
        tools_present: i64,
        vision_required: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_application_provider_capabilities_v1(
        abi_version: i64,
        provider: i64,
        response_status: i64,
        compact_status: i64,
        image_status: i64,
        supports_streaming: i64,
        catalog_vision: i64,
        catalog_tools: i64,
        catalog_json_mode: i64,
        capability_mask: u64,
    ) -> i64;
    fn prodex_mojo_application_capability_status_executable_v1(
        abi_version: i64,
        status: i64,
        executable: u64,
    ) -> i64;
    fn prodex_mojo_application_normalized_load_v1(
        abi_version: i64,
        active: u64,
        limit: u64,
        scale: u64,
        load: u64,
    ) -> i64;
    fn prodex_mojo_application_quota_headroom_v1(
        abi_version: i64,
        status: i64,
        remaining_percent: i64,
        reset_at: i64,
        now: i64,
        scale: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_application_output_tokens_v1(
        abi_version: i64,
        max_output_present: i64,
        max_output_tokens: u64,
        max_completion_present: i64,
        max_completion_tokens: u64,
        max_tokens_present: i64,
        max_tokens: u64,
        selected_present: u64,
        selected_tokens: u64,
    ) -> i64;
    fn prodex_mojo_application_candidate_plan_v1(
        abi_version: i64,
        hard_continuation: i64,
        fallback_count: i64,
        attempt_index: i64,
        primary_available: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_application_attempt_result_plan_v1(
        abi_version: i64,
        transport_succeeded: i64,
        retryable_response: i64,
        retry_allowed: i64,
        result_action: u64,
    ) -> i64;
    fn prodex_mojo_application_binding_plan_v1(
        abi_version: i64,
        continuation_bound: i64,
        bound_identity_present: i64,
        provider_matches: i64,
        selected_identity_present: i64,
        identity_matches: i64,
        selected_identity_optional: i64,
        binding_decision: u64,
    ) -> i64;
    fn prodex_mojo_application_governance_dispatch_plan_v1(
        abi_version: i64,
        tenant_bound: i64,
        anonymous_compatibility_allowed: i64,
        enforcing: i64,
        mandatory_audit: i64,
        policy_allows: i64,
        routing_present: i64,
        governance_decision: u64,
    ) -> i64;
}

pub fn plan_application_pipeline_error(
    input: ApplicationPipelineErrorInput,
) -> Result<ApplicationPipelineErrorPlan, MojoError> {
    ensure_rich_abi()?;
    let (error_kind, gateway_error_status) = match input {
        ApplicationPipelineErrorInput::UnknownRoute => (0, -1),
        ApplicationPipelineErrorInput::Gateway(status) => (1, status as i64),
    };
    let mut result = ApplicationPipelineErrorPlanResult::default();
    let status = unsafe {
        prodex_mojo_application_pipeline_error_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            error_kind,
            gateway_error_status,
            mojo_mut_pointer_address(&mut result),
        )
    };
    check_status(status)?;
    if result.abi_version != APPLICATION_DATA_PLANE_ABI_VERSION {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationPipelineErrorPlan {
        status: u16::try_from(result.status).map_err(|_| MojoError::InvalidOutput)?,
        gateway_response: decode_bool(result.response_kind)?,
    })
}

pub fn application_provider_capability_is_executable(
    status: ApplicationProviderCapabilityStatus,
) -> Result<bool, MojoError> {
    call_scalar(|output| unsafe {
        prodex_mojo_application_capability_status_executable_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            status as i64,
            output,
        )
    })
    .and_then(decode_bool)
}

pub fn plan_application_route_request(
    route: ApplicationRouteKind,
    streaming: bool,
    tools_present: bool,
    vision_required: bool,
) -> Result<ApplicationRouteRequestPlan, MojoError> {
    ensure_rich_abi()?;
    let mut result = ApplicationRouteRequestPlanResult::default();
    let status = unsafe {
        prodex_mojo_application_route_request_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            route as i64,
            i64::from(streaming),
            i64::from(tools_present),
            i64::from(vision_required),
            mojo_mut_pointer_address(&mut result),
        )
    };
    check_status(status)?;
    if result.abi_version != APPLICATION_DATA_PLANE_ABI_VERSION {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationRouteRequestPlan {
        runtime_route: decode_runtime_route(result.runtime_route)?,
        endpoint: decode_endpoint(result.endpoint)?,
        governance_route: decode_governance_route(result.governance_route)?,
        governed_action: decode_governed_action(result.governed_action)?,
        capability_mask: validate_mask(result.capability_mask, 0x7f)?,
        modality_mask: validate_mask(result.modality_mask, 0x17)?,
        streaming: decode_bool(result.stream_mode)?,
        buffered_response_inspectable: decode_bool(result.buffered_response_inspectable)?,
        compact_dispatch: decode_bool(result.compact_dispatch)?,
        models_dispatch: decode_bool(result.models_dispatch)?,
    })
}

pub fn plan_application_provider_capabilities(
    input: ApplicationProviderCapabilitiesInput,
) -> Result<u64, MojoError> {
    ensure_rich_abi()?;
    let mut capability_mask = 0_u64;
    let status = unsafe {
        prodex_mojo_application_provider_capabilities_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            input.provider as i64,
            input.responses as i64,
            input.compact as i64,
            input.images as i64,
            i64::from(input.supports_streaming),
            i64::from(input.catalog_vision),
            i64::from(input.catalog_tools),
            i64::from(input.catalog_json_mode),
            mojo_mut_pointer_address(&mut capability_mask),
        )
    };
    check_status(status)?;
    validate_mask(capability_mask, 0x7f)
}

pub fn application_normalized_load(
    active: usize,
    limit: usize,
    scale: u16,
) -> Result<u16, MojoError> {
    ensure_rich_abi()?;
    let mut load = 0_u64;
    let status = unsafe {
        prodex_mojo_application_normalized_load_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            u64::try_from(active).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            u64::from(scale),
            mojo_mut_pointer_address(&mut load),
        )
    };
    check_status(status)?;
    u16::try_from(load).map_err(|_| MojoError::InvalidOutput)
}

pub fn application_quota_headroom(
    status: ApplicationQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
    scale: u16,
) -> Result<Option<u16>, MojoError> {
    ensure_rich_abi()?;
    let mut result = ApplicationQuotaHeadroomResult::default();
    let status = unsafe {
        prodex_mojo_application_quota_headroom_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            status as i64,
            remaining_percent,
            reset_at,
            now,
            i64::from(scale),
            mojo_mut_pointer_address(&mut result),
        )
    };
    check_status(status)?;
    if result.abi_version != APPLICATION_DATA_PLANE_ABI_VERSION {
        return Err(MojoError::InvalidOutput);
    }
    match decode_bool(result.present)? {
        true => Ok(Some(
            u16::try_from(result.headroom).map_err(|_| MojoError::InvalidOutput)?,
        )),
        false if result.headroom == 0 => Ok(None),
        false => Err(MojoError::InvalidOutput),
    }
}

pub fn select_application_output_tokens(
    values: [Option<u64>; 3],
) -> Result<Option<u32>, MojoError> {
    ensure_rich_abi()?;
    let mut selected_present = -1_i64;
    let mut selected_tokens = 0_u64;
    let status = unsafe {
        prodex_mojo_application_output_tokens_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            i64::from(values[0].is_some()),
            values[0].unwrap_or_default(),
            i64::from(values[1].is_some()),
            values[1].unwrap_or_default(),
            i64::from(values[2].is_some()),
            values[2].unwrap_or_default(),
            mojo_mut_pointer_address(&mut selected_present),
            mojo_mut_pointer_address(&mut selected_tokens),
        )
    };
    check_status(status)?;
    match decode_bool(selected_present)? {
        true => Ok(Some(
            u32::try_from(selected_tokens).map_err(|_| MojoError::InvalidOutput)?,
        )),
        false if selected_tokens == 0 => Ok(None),
        false => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_application_candidate(
    hard_continuation: bool,
    fallback_count: usize,
    attempt_index: usize,
    primary_available: bool,
) -> Result<ApplicationCandidatePlan, MojoError> {
    ensure_rich_abi()?;
    let mut result = ApplicationCandidatePlanResult::default();
    let status = unsafe {
        prodex_mojo_application_candidate_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            i64::from(hard_continuation),
            i64::try_from(fallback_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(attempt_index).map_err(|_| MojoError::InvalidInput)?,
            i64::from(primary_available),
            mojo_mut_pointer_address(&mut result),
        )
    };
    check_status(status)?;
    if result.abi_version != APPLICATION_DATA_PLANE_ABI_VERSION || result.candidate_count < 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationCandidatePlan {
        candidate_count: usize::try_from(result.candidate_count)
            .map_err(|_| MojoError::InvalidOutput)?,
        attempt: match result.attempt_action {
            0 => ApplicationCandidateAttempt::Stop,
            1 => ApplicationCandidateAttempt::Skip,
            2 => ApplicationCandidateAttempt::Primary,
            3 => ApplicationCandidateAttempt::Fallback,
            _ => return Err(MojoError::InvalidOutput),
        },
    })
}

pub fn plan_application_attempt_result(
    transport_succeeded: bool,
    retryable_response: bool,
    retry_allowed: bool,
) -> Result<ApplicationAttemptResult, MojoError> {
    let result = call_scalar(|output| unsafe {
        prodex_mojo_application_attempt_result_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            i64::from(transport_succeeded),
            i64::from(retryable_response),
            i64::from(retry_allowed),
            output,
        )
    })?;
    match result {
        0 => Ok(ApplicationAttemptResult::Success),
        1 => Ok(ApplicationAttemptResult::Retry),
        2 => Ok(ApplicationAttemptResult::Stop),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_application_binding(
    input: ApplicationBindingInput,
) -> Result<ApplicationBindingDecision, MojoError> {
    let result = call_scalar(|output| unsafe {
        prodex_mojo_application_binding_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            i64::from(input.continuation_bound),
            i64::from(input.bound_identity_present),
            i64::from(input.provider_matches),
            i64::from(input.selected_identity_present),
            i64::from(input.identity_matches),
            i64::from(input.selected_identity_optional),
            output,
        )
    })?;
    match result {
        0 => Ok(ApplicationBindingDecision::Valid),
        1 => Ok(ApplicationBindingDecision::MissingIdentity),
        2 => Ok(ApplicationBindingDecision::ProviderMismatch),
        3 => Ok(ApplicationBindingDecision::IdentityMismatch),
        4 => Ok(ApplicationBindingDecision::SelectedIdentityRequired),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_application_governance_dispatch(
    input: ApplicationGovernanceDispatchInput,
) -> Result<ApplicationGovernanceDispatchDecision, MojoError> {
    let result = call_scalar(|output| unsafe {
        prodex_mojo_application_governance_dispatch_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            i64::from(input.tenant_bound),
            i64::from(input.anonymous_compatibility_allowed),
            i64::from(input.enforcing),
            i64::from(input.mandatory_audit),
            i64::from(input.policy_allows),
            i64::from(input.routing_present),
            output,
        )
    })?;
    match result {
        0 => Ok(ApplicationGovernanceDispatchDecision::Allowed),
        1 => Ok(ApplicationGovernanceDispatchDecision::IdentityRequired),
        2 => Ok(ApplicationGovernanceDispatchDecision::Unavailable),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn call_scalar(call: impl FnOnce(u64) -> i64) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    let mut output = -1_i64;
    check_status(call(mojo_mut_pointer_address(&mut output)))?;
    Ok(output)
}

pub(super) fn check_status(status: i64) -> Result<(), MojoError> {
    if status == 0 {
        Ok(())
    } else {
        Err(status_error(status, 12, 0, 0, 0))
    }
}

fn validate_mask(value: u64, allowed: u64) -> Result<u64, MojoError> {
    if value & !allowed == 0 {
        Ok(value)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub(super) fn decode_bool(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_runtime_route(value: i64) -> Result<ApplicationRuntimeRoute, MojoError> {
    match value {
        0 => Ok(ApplicationRuntimeRoute::Responses),
        1 => Ok(ApplicationRuntimeRoute::Compact),
        2 => Ok(ApplicationRuntimeRoute::WebSocket),
        3 => Ok(ApplicationRuntimeRoute::Standard),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_endpoint(value: i64) -> Result<Option<ApplicationProviderEndpoint>, MojoError> {
    Ok(Some(match value {
        -1 => return Ok(None),
        0 => ApplicationProviderEndpoint::Responses,
        1 => ApplicationProviderEndpoint::ResponsesCompact,
        2 => ApplicationProviderEndpoint::ChatCompletions,
        3 => ApplicationProviderEndpoint::Messages,
        4 => ApplicationProviderEndpoint::Models,
        5 => ApplicationProviderEndpoint::Embeddings,
        6 => ApplicationProviderEndpoint::Images,
        7 => ApplicationProviderEndpoint::Audio,
        8 => ApplicationProviderEndpoint::Batches,
        9 => ApplicationProviderEndpoint::Rerank,
        10 => ApplicationProviderEndpoint::A2a,
        _ => return Err(MojoError::InvalidOutput),
    }))
}

fn decode_governance_route(value: i64) -> Result<Option<ApplicationRouteKind>, MojoError> {
    Ok(Some(match value {
        -1 => return Ok(None),
        0 => ApplicationRouteKind::Responses,
        1 => ApplicationRouteKind::Compact,
        2 => ApplicationRouteKind::WebSocket,
        4 => ApplicationRouteKind::ChatCompletions,
        5 => ApplicationRouteKind::Embeddings,
        6 => ApplicationRouteKind::ImagesGenerations,
        7 => ApplicationRouteKind::ImagesEdits,
        8 => ApplicationRouteKind::ImagesVariations,
        9 => ApplicationRouteKind::AudioSpeech,
        10 => ApplicationRouteKind::AudioTranscriptions,
        11 => ApplicationRouteKind::AudioTranslations,
        12 => ApplicationRouteKind::Batches,
        13 => ApplicationRouteKind::Batch,
        14 => ApplicationRouteKind::Rerank,
        15 => ApplicationRouteKind::A2a,
        16 => ApplicationRouteKind::Messages,
        17 => ApplicationRouteKind::Models,
        18 => ApplicationRouteKind::Model,
        _ => return Err(MojoError::InvalidOutput),
    }))
}

fn decode_governed_action(value: i64) -> Result<ApplicationGovernedAction, MojoError> {
    match value {
        0 => Ok(ApplicationGovernedAction::InvokeModel),
        2 => Ok(ApplicationGovernedAction::UploadContent),
        3 => Ok(ApplicationGovernedAction::CompactContext),
        _ => Err(MojoError::InvalidOutput),
    }
}
