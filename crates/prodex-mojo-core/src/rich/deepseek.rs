use super::{
    MojoError, MojoIssue, RICH_ABI_VERSION, RichStringView, ensure_rich_abi,
    mojo_mut_pointer_address, mojo_pointer_address, view,
};

/// Deterministic DeepSeek request, response, and stream JSON shapes.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekKernelOperation {
    RequestBody = 1,
    SystemMessage = 2,
    UserMessage = 3,
    Message = 4,
    ToolCallMessage = 5,
    ToolMessage = 6,
    ResponseValue = 7,
    BufferedResponse = 8,
    ResponseCreatedEvent = 9,
    ResponseCompletedEvent = 10,
    OutputItemAddedEvent = 11,
    OutputItemDoneEvent = 12,
    FunctionCallArgumentsDeltaEvent = 13,
    OutputTextDeltaEvent = 14,
    OutputTextItem = 15,
    StreamResponseValue = 16,
    StreamAssistantMessage = 17,
    FunctionCallItem = 18,
    AddedFunctionCallItem = 19,
    ToolSearchItem = 20,
    CustomToolCallItem = 21,
    FunctionCallArgumentsDeltaSource = 22,
    TextDeltaSource = 23,
    SseFunctionCallDelta = 24,
    SseTextDelta = 25,
    ResponseMetadata = 26,
    StrictFunctionSchema = 27,
    PrimitiveRequestFields = 28,
    ReasoningParameters = 29,
    ResponseFormat = 30,
    UserId = 31,
    StreamToolCallDelta = 32,
    StreamChunkMetadata = 33,
    StreamChoiceMetadata = 34,
    StreamChoiceDelta = 35,
    StreamResponseMetadata = 36,
    RawCommonRequest = 37,
    RequestMetadata = 38,
    RawBridgeInputItem = 39,
    ResponseToolCallItem = 40,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekRequestPolicyOperation {
    RequestFields = 1,
    BetaFields = 2,
    ReasoningShape = 3,
    SimpleRequest = 4,
    PrimitiveCore = 5,
    TopLogprobs = 6,
    Stop = 7,
    ToolChoice = 8,
    WebSearchOptions = 9,
    WebSearchContext = 10,
    ToolsShape = 11,
    ResponsesRequestParams = 12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekRequestPolicyPlan {
    pub tag: i64,
    pub detail_start: Option<usize>,
    pub detail_end: Option<usize>,
}

/// Inputs for one bounded DeepSeek JSON transformation.
#[derive(Debug, Clone, Copy)]
pub struct DeepSeekKernelInput<'a> {
    pub operation: DeepSeekKernelOperation,
    pub sequence_number: u64,
    pub created_at: u64,
    pub stream: bool,
    pub response_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub role: Option<&'a str>,
    pub content: Option<&'a str>,
    pub reasoning_content: Option<&'a str>,
    pub name: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub arguments: Option<&'a str>,
    pub signature: Option<&'a str>,
    pub delta: Option<&'a str>,
    pub messages: Option<&'a str>,
    pub tools: Option<&'a str>,
    pub tool_choice: Option<&'a str>,
    pub extra: Option<&'a str>,
    pub output: Option<&'a str>,
    pub usage: Option<&'a str>,
    pub metadata: Option<&'a str>,
    pub item: Option<&'a str>,
    pub response: Option<&'a str>,
    pub tool_calls: Option<&'a str>,
    pub input: Option<&'a str>,
    pub error_code: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

impl<'a> DeepSeekKernelInput<'a> {
    pub const fn new(operation: DeepSeekKernelOperation) -> Self {
        Self {
            operation,
            sequence_number: 0,
            created_at: 0,
            stream: false,
            response_id: None,
            call_id: None,
            model: None,
            role: None,
            content: None,
            reasoning_content: None,
            name: None,
            namespace: None,
            arguments: None,
            signature: None,
            delta: None,
            messages: None,
            tools: None,
            tool_choice: None,
            extra: None,
            output: None,
            usage: None,
            metadata: None,
            item: None,
            response: None,
            tool_calls: None,
            input: None,
            error_code: None,
            error_message: None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DeepSeekKernelFfiInput {
    operation: i64,
    sequence_number: u64,
    created_at: u64,
    stream: i64,
    response_id_present: i64,
    call_id_present: i64,
    model_present: i64,
    role_present: i64,
    content_present: i64,
    reasoning_content_present: i64,
    name_present: i64,
    namespace_present: i64,
    arguments_present: i64,
    signature_present: i64,
    delta_present: i64,
    messages_present: i64,
    tools_present: i64,
    tool_choice_present: i64,
    extra_present: i64,
    output_present: i64,
    usage_present: i64,
    metadata_present: i64,
    item_present: i64,
    response_present: i64,
    tool_calls_present: i64,
    input_present: i64,
    error_code_present: i64,
    error_message_present: i64,
    response_id: RichStringView,
    call_id: RichStringView,
    model: RichStringView,
    role: RichStringView,
    content: RichStringView,
    reasoning_content: RichStringView,
    name: RichStringView,
    namespace: RichStringView,
    arguments: RichStringView,
    signature: RichStringView,
    delta: RichStringView,
    messages: RichStringView,
    tools: RichStringView,
    tool_choice: RichStringView,
    extra: RichStringView,
    output: RichStringView,
    usage: RichStringView,
    metadata: RichStringView,
    item: RichStringView,
    response: RichStringView,
    tool_calls: RichStringView,
    input: RichStringView,
    error_code: RichStringView,
    error_message: RichStringView,
}

const _: () = assert!(std::mem::size_of::<DeepSeekKernelFfiInput>() == 608);

unsafe extern "C" {
    fn prodex_mojo_deepseek_kernel_v2(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_deepseek_request_policy_v1(
        abi_version: i64,
        operation: i64,
        input: u64,
        input_length: i64,
        flag: i64,
        scalar: i64,
        output: u64,
    ) -> i64;
}

pub const DEEPSEEK_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum aggregate input size for bounded large DeepSeek response operations.
pub const DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES: usize = 16 * 1024 * 1024;
const DEEPSEEK_KERNEL_ABI_VERSION: i64 = 2;

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn operation_code(operation: DeepSeekKernelOperation) -> i64 {
    match operation {
        DeepSeekKernelOperation::RequestBody => 1,
        DeepSeekKernelOperation::SystemMessage => 2,
        DeepSeekKernelOperation::UserMessage => 3,
        DeepSeekKernelOperation::Message => 4,
        DeepSeekKernelOperation::ToolCallMessage => 5,
        DeepSeekKernelOperation::ToolMessage => 6,
        DeepSeekKernelOperation::ResponseValue => 7,
        DeepSeekKernelOperation::BufferedResponse => 8,
        DeepSeekKernelOperation::ResponseCreatedEvent => 9,
        DeepSeekKernelOperation::ResponseCompletedEvent => 10,
        DeepSeekKernelOperation::OutputItemAddedEvent => 11,
        DeepSeekKernelOperation::OutputItemDoneEvent => 12,
        DeepSeekKernelOperation::FunctionCallArgumentsDeltaEvent => 13,
        DeepSeekKernelOperation::OutputTextDeltaEvent => 14,
        DeepSeekKernelOperation::OutputTextItem => 15,
        DeepSeekKernelOperation::StreamResponseValue => 16,
        DeepSeekKernelOperation::StreamAssistantMessage => 17,
        DeepSeekKernelOperation::FunctionCallItem => 18,
        DeepSeekKernelOperation::AddedFunctionCallItem => 19,
        DeepSeekKernelOperation::ToolSearchItem => 20,
        DeepSeekKernelOperation::CustomToolCallItem => 21,
        DeepSeekKernelOperation::FunctionCallArgumentsDeltaSource => 22,
        DeepSeekKernelOperation::TextDeltaSource => 23,
        DeepSeekKernelOperation::SseFunctionCallDelta => 24,
        DeepSeekKernelOperation::SseTextDelta => 25,
        DeepSeekKernelOperation::ResponseMetadata => 26,
        DeepSeekKernelOperation::StrictFunctionSchema => 27,
        DeepSeekKernelOperation::PrimitiveRequestFields => 28,
        DeepSeekKernelOperation::ReasoningParameters => 29,
        DeepSeekKernelOperation::ResponseFormat => 30,
        DeepSeekKernelOperation::UserId => 31,
        DeepSeekKernelOperation::StreamToolCallDelta => 32,
        DeepSeekKernelOperation::StreamChunkMetadata => 33,
        DeepSeekKernelOperation::StreamChoiceMetadata => 34,
        DeepSeekKernelOperation::StreamChoiceDelta => 35,
        DeepSeekKernelOperation::StreamResponseMetadata => 36,
        DeepSeekKernelOperation::RawCommonRequest => 37,
        DeepSeekKernelOperation::RequestMetadata => 38,
        DeepSeekKernelOperation::RawBridgeInputItem => 39,
        DeepSeekKernelOperation::ResponseToolCallItem => 40,
    }
}

fn input_bytes(input: &DeepSeekKernelInput<'_>) -> Result<usize, MojoError> {
    let views = [
        input.response_id,
        input.call_id,
        input.model,
        input.role,
        input.content,
        input.reasoning_content,
        input.name,
        input.namespace,
        input.arguments,
        input.signature,
        input.delta,
        input.messages,
        input.tools,
        input.tool_choice,
        input.extra,
        input.output,
        input.usage,
        input.metadata,
        input.item,
        input.response,
        input.tool_calls,
        input.input,
        input.error_code,
        input.error_message,
    ];
    views.iter().flatten().try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}

fn kernel_output_capacity(
    operation: DeepSeekKernelOperation,
    input_bytes: usize,
) -> Result<usize, MojoError> {
    if operation == DeepSeekKernelOperation::UserId {
        return Ok(512 + 2);
    }
    let multiplier = match operation {
        DeepSeekKernelOperation::ResponseToolCallItem
        | DeepSeekKernelOperation::BufferedResponse => 6,
        _ => 8,
    };
    input_bytes
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(2048))
        .ok_or(MojoError::InvalidInput)
}

fn kernel_input_limit(operation: DeepSeekKernelOperation) -> usize {
    match operation {
        DeepSeekKernelOperation::ResponseToolCallItem
        | DeepSeekKernelOperation::BufferedResponse => DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES,
        _ => DEEPSEEK_KERNEL_MAX_BYTES,
    }
}

pub fn deepseek_request_policy(
    operation: DeepSeekRequestPolicyOperation,
    input: &str,
    flag: bool,
    scalar: i64,
) -> Result<DeepSeekRequestPolicyPlan, MojoError> {
    ensure_rich_abi()?;
    let max_input_bytes = match operation {
        DeepSeekRequestPolicyOperation::SimpleRequest => DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES,
        _ => DEEPSEEK_KERNEL_MAX_BYTES,
    };
    if input.len() > max_input_bytes || scalar < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_mojo_deepseek_request_policy_v1(
            RICH_ABI_VERSION,
            operation as i64,
            input.as_ptr() as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(flag),
            scalar,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            4 => MojoError::AbiMismatch,
            1 | 2 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    let (detail_start, detail_end) = match (output[1], output[2]) {
        (-1, -1) => (None, None),
        (start, end) if start >= 0 && end >= start => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            if end > input.len() {
                return Err(MojoError::InvalidOutput);
            }
            (Some(start), Some(end))
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(DeepSeekRequestPolicyPlan {
        tag: output[0],
        detail_start,
        detail_end,
    })
}

/// Runs one bounded DeepSeek request, response, or stream JSON builder in Mojo.
pub fn deepseek_kernel(input: DeepSeekKernelInput<'_>) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > kernel_input_limit(input.operation) {
        return Err(MojoError::InvalidInput);
    }
    let capacity = kernel_output_capacity(input.operation, input_bytes)?;
    let ffi_input = DeepSeekKernelFfiInput {
        operation: operation_code(input.operation),
        sequence_number: input.sequence_number,
        created_at: input.created_at,
        stream: i64::from(input.stream),
        response_id_present: i64::from(input.response_id.is_some()),
        call_id_present: i64::from(input.call_id.is_some()),
        model_present: i64::from(input.model.is_some()),
        role_present: i64::from(input.role.is_some()),
        content_present: i64::from(input.content.is_some()),
        reasoning_content_present: i64::from(input.reasoning_content.is_some()),
        name_present: i64::from(input.name.is_some()),
        namespace_present: i64::from(input.namespace.is_some()),
        arguments_present: i64::from(input.arguments.is_some()),
        signature_present: i64::from(input.signature.is_some()),
        delta_present: i64::from(input.delta.is_some()),
        messages_present: i64::from(input.messages.is_some()),
        tools_present: i64::from(input.tools.is_some()),
        tool_choice_present: i64::from(input.tool_choice.is_some()),
        extra_present: i64::from(input.extra.is_some()),
        output_present: i64::from(input.output.is_some()),
        usage_present: i64::from(input.usage.is_some()),
        metadata_present: i64::from(input.metadata.is_some()),
        item_present: i64::from(input.item.is_some()),
        response_present: i64::from(input.response.is_some()),
        tool_calls_present: i64::from(input.tool_calls.is_some()),
        input_present: i64::from(input.input.is_some()),
        error_code_present: i64::from(input.error_code.is_some()),
        error_message_present: i64::from(input.error_message.is_some()),
        response_id: kernel_view(input.response_id),
        call_id: kernel_view(input.call_id),
        model: kernel_view(input.model),
        role: kernel_view(input.role),
        content: kernel_view(input.content),
        reasoning_content: kernel_view(input.reasoning_content),
        name: kernel_view(input.name),
        namespace: kernel_view(input.namespace),
        arguments: kernel_view(input.arguments),
        signature: kernel_view(input.signature),
        delta: kernel_view(input.delta),
        messages: kernel_view(input.messages),
        tools: kernel_view(input.tools),
        tool_choice: kernel_view(input.tool_choice),
        extra: kernel_view(input.extra),
        output: kernel_view(input.output),
        usage: kernel_view(input.usage),
        metadata: kernel_view(input.metadata),
        item: kernel_view(input.item),
        response: kernel_view(input.response),
        tool_calls: kernel_view(input.tool_calls),
        input: kernel_view(input.input),
        error_code: kernel_view(input.error_code),
        error_message: kernel_view(input.error_message),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_deepseek_kernel_v2(
            DEEPSEEK_KERNEL_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(match status {
            2 => MojoError::Structured(MojoIssue {
                domain: 8,
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

#[cfg(test)]
mod tests {
    use super::{DEEPSEEK_KERNEL_MAX_BYTES, DeepSeekKernelOperation, kernel_output_capacity};

    #[test]
    fn user_id_output_capacity_stays_bounded_for_large_inputs() {
        assert_eq!(
            kernel_output_capacity(DeepSeekKernelOperation::UserId, DEEPSEEK_KERNEL_MAX_BYTES),
            Ok(514)
        );
    }
}
