use super::{
    MojoError, MojoIssue, RICH_ABI_VERSION, RichStringView, ensure_rich_abi,
    mojo_mut_pointer_address, mojo_pointer_address, view,
};

/// Deterministic Kiro request, response, and stream JSON shapes.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroKernelOperation {
    RequestBody = 1,
    PromptSection = 2,
    ResponseMessageItem = 3,
    ResponseFunctionCallItem = 4,
    ResponseFunctionCallOutputItem = 5,
    LegacyFunctionTool = 6,
    LegacyToolChoice = 7,
    ChatCompletionResponse = 8,
    AnthropicToolUseBlock = 9,
    AnthropicResponse = 10,
    ChatCompletionChunk = 11,
    ChatRoleDelta = 12,
    ChatEmptyDelta = 13,
    ChatTextDelta = 14,
    ChatReasoningDelta = 15,
    ChatToolCallDelta = 16,
    OutputTextDeltaEvent = 17,
    ResponseCreatedEvent = 18,
    OutputItemAddedEvent = 19,
    OutputItemDoneEvent = 20,
    ResponseCompletedEvent = 21,
    ResponseFailedEvent = 22,
    ResponseIncompleteEvent = 23,
    ToolCallArgumentsDeltaChatValue = 24,
    UsageUpdate = 25,
    StreamToolArguments = 26,
    FinishReason = 27,
    ChatToolCallItem = 28,
}

/// Inputs for one bounded Kiro JSON or text transformation.
#[derive(Debug, Clone, Copy)]
pub struct KiroKernelInput<'a> {
    pub operation: KiroKernelOperation,
    pub sequence_number: u64,
    pub created_at: u64,
    pub request_id: u64,
    pub used: u64,
    pub size: u64,
    pub include_role: bool,
    pub has_tool_calls: bool,
    pub response_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub role: Option<&'a str>,
    pub content: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub arguments: Option<&'a str>,
    pub input: Option<&'a str>,
    pub output: Option<&'a str>,
    pub tool_calls: Option<&'a str>,
    pub requested_model: Option<&'a str>,
    pub metadata: Option<&'a str>,
    pub finish_reason: Option<&'a str>,
    pub status: Option<&'a str>,
    pub error: Option<&'a str>,
    pub extra: Option<&'a str>,
    pub incomplete_reason: Option<&'a str>,
}

/// Selects the Kiro request surface whose capability rules are being checked.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroRequestValidationMode {
    ChatCompletions = 1,
    Responses = 2,
}

/// JSON facts extracted by Rust before Mojo applies Kiro capability policy.
///
/// Rust owns JSON parsing and request mutation. Mojo owns the ordered decision
/// about whether a parsed request asks for an unsupported capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KiroRequestValidationInput {
    pub mode: KiroRequestValidationMode,
    pub flags: u64,
    pub detail: i64,
    pub allow_token_limit: bool,
}

/// Ordered Kiro request capability decision returned by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KiroRequestValidationPlan {
    pub reason: i64,
    pub detail: i64,
    pub detail_is_invalid: bool,
}

impl KiroRequestValidationPlan {
    pub const REASON_NONE: i64 = 0;
    pub const REASON_CHAT_RESPONSE_FORMAT: i64 = 1;
    pub const REASON_CHAT_CHOICE_COUNT: i64 = 2;
    pub const REASON_CHAT_STOP: i64 = 3;
    pub const REASON_CHAT_TEMPERATURE: i64 = 4;
    pub const REASON_CHAT_TOP_P: i64 = 5;
    pub const REASON_CHAT_PRESENCE_PENALTY: i64 = 6;
    pub const REASON_CHAT_FREQUENCY_PENALTY: i64 = 7;
    pub const REASON_CHAT_SEED: i64 = 8;
    pub const REASON_CHAT_PARALLEL_TOOL_CALLS: i64 = 9;
    pub const REASON_TOKEN_LIMIT: i64 = 10;
    pub const REASON_GENERATION_CONTROL: i64 = 11;
    pub const REASON_RESPONSE_STOP: i64 = 12;
    pub const REASON_LOGPROBS: i64 = 13;
    pub const REASON_TOP_LOGPROBS: i64 = 14;
    pub const REASON_RESPONSE_FORMAT: i64 = 15;
    pub const REASON_TOOL_CHOICE: i64 = 16;
    pub const REASON_TOOLS: i64 = 17;
    pub const REASON_WEB_SEARCH: i64 = 18;
    pub const REASON_REASONING_EFFORT: i64 = 19;
}

impl KiroRequestValidationInput {
    pub const FLAG_CHAT_RESPONSE_FORMAT: u64 = 1 << 0;
    pub const FLAG_CHAT_CHOICE_COUNT: u64 = 1 << 1;
    pub const FLAG_CHAT_STOP: u64 = 1 << 2;
    pub const FLAG_CHAT_TEMPERATURE: u64 = 1 << 3;
    pub const FLAG_CHAT_TOP_P: u64 = 1 << 4;
    pub const FLAG_CHAT_PRESENCE_PENALTY: u64 = 1 << 5;
    pub const FLAG_CHAT_FREQUENCY_PENALTY: u64 = 1 << 6;
    pub const FLAG_CHAT_SEED: u64 = 1 << 7;
    pub const FLAG_CHAT_PARALLEL_TOOL_CALLS: u64 = 1 << 8;
    pub const FLAG_TOKEN_LIMIT: u64 = 1 << 9;
    pub const FLAG_TOKEN_LIMIT_INVALID: u64 = 1 << 10;
    pub const FLAG_GENERATION_CONTROL: u64 = 1 << 11;
    pub const FLAG_RESPONSE_STOP: u64 = 1 << 12;
    pub const FLAG_LOGPROBS_UNSUPPORTED: u64 = 1 << 13;
    pub const FLAG_LOGPROBS_INVALID: u64 = 1 << 14;
    pub const FLAG_TOP_LOGPROBS: u64 = 1 << 15;
    pub const FLAG_RESPONSE_FORMAT: u64 = 1 << 16;
    pub const FLAG_TOOL_CHOICE: u64 = 1 << 17;
    pub const FLAG_TOOLS: u64 = 1 << 18;
    pub const FLAG_WEB_SEARCH: u64 = 1 << 19;
    pub const FLAG_REASONING_EFFORT: u64 = 1 << 20;
    pub const FLAG_MASK: u64 = (1 << 21) - 1;
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KiroRequestValidationFfiInput {
    mode: i64,
    flags: u64,
    detail: i64,
    allow_token_limit: i64,
}

const _: () = assert!(std::mem::size_of::<KiroRequestValidationFfiInput>() == 32);

impl<'a> KiroKernelInput<'a> {
    pub const fn new(operation: KiroKernelOperation) -> Self {
        Self {
            operation,
            sequence_number: 0,
            created_at: 0,
            request_id: 0,
            used: 0,
            size: 0,
            include_role: false,
            has_tool_calls: false,
            response_id: None,
            model: None,
            role: None,
            content: None,
            reason: None,
            call_id: None,
            name: None,
            arguments: None,
            input: None,
            output: None,
            tool_calls: None,
            requested_model: None,
            metadata: None,
            finish_reason: None,
            status: None,
            error: None,
            extra: None,
            incomplete_reason: None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KiroKernelFfiInput {
    operation: i64,
    sequence_number: u64,
    created_at: u64,
    request_id: u64,
    used: u64,
    size: u64,
    include_role: i64,
    has_tool_calls: i64,
    response_id_present: i64,
    model_present: i64,
    role_present: i64,
    content_present: i64,
    reason_present: i64,
    call_id_present: i64,
    name_present: i64,
    arguments_present: i64,
    input_present: i64,
    output_present: i64,
    tool_calls_present: i64,
    requested_model_present: i64,
    metadata_present: i64,
    finish_reason_present: i64,
    status_present: i64,
    error_present: i64,
    extra_present: i64,
    incomplete_reason_present: i64,
    response_id: RichStringView,
    model: RichStringView,
    role: RichStringView,
    content: RichStringView,
    reason: RichStringView,
    call_id: RichStringView,
    name: RichStringView,
    arguments: RichStringView,
    input: RichStringView,
    output: RichStringView,
    tool_calls: RichStringView,
    requested_model: RichStringView,
    metadata: RichStringView,
    finish_reason: RichStringView,
    status: RichStringView,
    error: RichStringView,
    extra: RichStringView,
    incomplete_reason: RichStringView,
}

const _: () = assert!(std::mem::size_of::<KiroKernelFfiInput>() == 496);

unsafe extern "C" {
    fn prodex_mojo_kiro_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_kiro_request_validation_v1(abi_version: i64, input: u64, output: u64) -> i64;
}

const KIRO_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn operation_code(operation: KiroKernelOperation) -> i64 {
    match operation {
        KiroKernelOperation::RequestBody => 1,
        KiroKernelOperation::PromptSection => 2,
        KiroKernelOperation::ResponseMessageItem => 3,
        KiroKernelOperation::ResponseFunctionCallItem => 4,
        KiroKernelOperation::ResponseFunctionCallOutputItem => 5,
        KiroKernelOperation::LegacyFunctionTool => 6,
        KiroKernelOperation::LegacyToolChoice => 7,
        KiroKernelOperation::ChatCompletionResponse => 8,
        KiroKernelOperation::AnthropicToolUseBlock => 9,
        KiroKernelOperation::AnthropicResponse => 10,
        KiroKernelOperation::ChatCompletionChunk => 11,
        KiroKernelOperation::ChatRoleDelta => 12,
        KiroKernelOperation::ChatEmptyDelta => 13,
        KiroKernelOperation::ChatTextDelta => 14,
        KiroKernelOperation::ChatReasoningDelta => 15,
        KiroKernelOperation::ChatToolCallDelta => 16,
        KiroKernelOperation::OutputTextDeltaEvent => 17,
        KiroKernelOperation::ResponseCreatedEvent => 18,
        KiroKernelOperation::OutputItemAddedEvent => 19,
        KiroKernelOperation::OutputItemDoneEvent => 20,
        KiroKernelOperation::ResponseCompletedEvent => 21,
        KiroKernelOperation::ResponseFailedEvent => 22,
        KiroKernelOperation::ResponseIncompleteEvent => 23,
        KiroKernelOperation::ToolCallArgumentsDeltaChatValue => 24,
        KiroKernelOperation::UsageUpdate => 25,
        KiroKernelOperation::StreamToolArguments => 26,
        KiroKernelOperation::FinishReason => 27,
        KiroKernelOperation::ChatToolCallItem => 28,
    }
}

fn input_bytes(input: &KiroKernelInput<'_>) -> Result<usize, MojoError> {
    [
        input.response_id,
        input.model,
        input.role,
        input.content,
        input.reason,
        input.call_id,
        input.name,
        input.arguments,
        input.input,
        input.output,
        input.tool_calls,
        input.requested_model,
        input.metadata,
        input.finish_reason,
        input.status,
        input.error,
        input.extra,
        input.incomplete_reason,
    ]
    .iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}

/// Runs one bounded Kiro request, response, or stream transformation in Mojo.
pub fn kiro_kernel(input: KiroKernelInput<'_>) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > KIRO_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input_bytes
        .checked_mul(8)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let ffi_input = KiroKernelFfiInput {
        operation: operation_code(input.operation),
        sequence_number: input.sequence_number,
        created_at: input.created_at,
        request_id: input.request_id,
        used: input.used,
        size: input.size,
        include_role: i64::from(input.include_role),
        has_tool_calls: i64::from(input.has_tool_calls),
        response_id_present: i64::from(input.response_id.is_some()),
        model_present: i64::from(input.model.is_some()),
        role_present: i64::from(input.role.is_some()),
        content_present: i64::from(input.content.is_some()),
        reason_present: i64::from(input.reason.is_some()),
        call_id_present: i64::from(input.call_id.is_some()),
        name_present: i64::from(input.name.is_some()),
        arguments_present: i64::from(input.arguments.is_some()),
        input_present: i64::from(input.input.is_some()),
        output_present: i64::from(input.output.is_some()),
        tool_calls_present: i64::from(input.tool_calls.is_some()),
        requested_model_present: i64::from(input.requested_model.is_some()),
        metadata_present: i64::from(input.metadata.is_some()),
        finish_reason_present: i64::from(input.finish_reason.is_some()),
        status_present: i64::from(input.status.is_some()),
        error_present: i64::from(input.error.is_some()),
        extra_present: i64::from(input.extra.is_some()),
        incomplete_reason_present: i64::from(input.incomplete_reason.is_some()),
        response_id: kernel_view(input.response_id),
        model: kernel_view(input.model),
        role: kernel_view(input.role),
        content: kernel_view(input.content),
        reason: kernel_view(input.reason),
        call_id: kernel_view(input.call_id),
        name: kernel_view(input.name),
        arguments: kernel_view(input.arguments),
        input: kernel_view(input.input),
        output: kernel_view(input.output),
        tool_calls: kernel_view(input.tool_calls),
        requested_model: kernel_view(input.requested_model),
        metadata: kernel_view(input.metadata),
        finish_reason: kernel_view(input.finish_reason),
        status: kernel_view(input.status),
        error: kernel_view(input.error),
        extra: kernel_view(input.extra),
        incomplete_reason: kernel_view(input.incomplete_reason),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_kiro_kernel_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(match status {
            2 => MojoError::Structured(MojoIssue {
                domain: 9,
                kind: 1,
                field: 0,
                object_index: -1,
                byte_offset: 0,
                byte_length: 1,
                expected: 0,
            }),
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}

/// Apply the authoritative Kiro request capability policy in Mojo.
pub fn kiro_validate_request(
    input: KiroRequestValidationInput,
) -> Result<KiroRequestValidationPlan, MojoError> {
    ensure_rich_abi()?;
    if input.flags & !KiroRequestValidationInput::FLAG_MASK != 0
        || !(-1..=2).contains(&input.detail)
    {
        return Err(MojoError::InvalidInput);
    }
    let ffi_input = KiroRequestValidationFfiInput {
        mode: input.mode as i64,
        flags: input.flags,
        detail: input.detail,
        allow_token_limit: i64::from(input.allow_token_limit),
    };
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_kiro_request_validation_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            4 => MojoError::AbiMismatch,
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(KiroRequestValidationPlan::REASON_NONE
        ..=KiroRequestValidationPlan::REASON_REASONING_EFFORT)
        .contains(&output[0])
        || !(-1..=2).contains(&output[1])
        || !matches!(output[2], 0 | 1)
        || (matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_GENERATION_CONTROL
        ) && output[1] < 0)
        || (!matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_GENERATION_CONTROL
        ) && output[1] != -1)
        || (!matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_LOGPROBS
        ) && output[2] != 0)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(KiroRequestValidationPlan {
        reason: output[0],
        detail: output[1],
        detail_is_invalid: output[2] == 1,
    })
}
