use super::{
    MojoError, MojoIssue, RICH_ABI_VERSION, RichStringView, ensure_rich_abi,
    mojo_mut_pointer_address, mojo_pointer_address, view,
};

/// Deterministic JSON shapes emitted by the Gemini response kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiResponseKernelOperation {
    ResponseCreated = 1,
    ResponseCompleted = 2,
    ResponseIncomplete = 3,
    ResponseMetadata = 4,
    OutputItemAdded = 5,
    OutputItemDone = 6,
    FunctionCallArgumentsDelta = 7,
    OutputTextDelta = 8,
    ReasoningSummaryPartAdded = 9,
    ReasoningSummaryTextDelta = 10,
    TextSource = 11,
    ReasoningSource = 12,
    FunctionCallSource = 13,
    OutputTextContent = 14,
    MessageItem = 15,
    OutputMessageItem = 16,
    ResponseValue = 17,
    FunctionCallItem = 18,
    RawFunctionCallItem = 19,
    AddedFunctionCallItem = 20,
    ChatFunctionCallItem = 21,
    ResponseUsage = 22,
    StreamTextDelta = 23,
    StreamReasoningDelta = 24,
    FunctionCallArgumentsDeltaWithoutSequence = 25,
    BufferedResponse = 26,
    CitationText = 27,
    WebSearchCall = 28,
    StreamAssistantMessage = 29,
    StreamOutputItems = 30,
    ToolSearchCallItem = 31,
    CustomToolCallItem = 32,
}

/// Inputs for one bounded Gemini response or stream JSON shape.
#[derive(Debug, Clone, Copy)]
pub struct GeminiResponseKernelInput<'a> {
    pub operation: GeminiResponseKernelOperation,
    pub sequence_number: u64,
    pub created_at: u64,
    pub summary_index: u64,
    pub prompt_token_count: u64,
    pub candidate_token_count: u64,
    pub total_token_count: u64,
    pub total_token_count_present: i64,
    pub cached_content_token_count: u64,
    pub thoughts_token_count: u64,
    pub tool_use_prompt_token_count: u64,
    pub response_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub usage: Option<&'a str>,
    pub metadata: Option<&'a str>,
    pub signature: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub name: Option<&'a str>,
    pub delta: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub message: Option<&'a str>,
    pub item: Option<&'a str>,
    pub response: Option<&'a str>,
    pub content: Option<&'a str>,
    pub output: Option<&'a str>,
    pub arguments: Option<&'a str>,
    pub created_at_present: bool,
    pub include_empty_usage: bool,
    pub include_empty_metadata: bool,
    pub citations: Option<&'a str>,
    pub reason_present: bool,
}

impl<'a> GeminiResponseKernelInput<'a> {
    pub const fn new(operation: GeminiResponseKernelOperation) -> Self {
        Self {
            operation,
            sequence_number: 0,
            created_at: 0,
            summary_index: 0,
            prompt_token_count: 0,
            candidate_token_count: 0,
            total_token_count: 0,
            total_token_count_present: 0,
            cached_content_token_count: 0,
            thoughts_token_count: 0,
            tool_use_prompt_token_count: 0,
            response_id: None,
            call_id: None,
            model: None,
            usage: None,
            metadata: None,
            signature: None,
            namespace: None,
            name: None,
            delta: None,
            reason: None,
            message: None,
            item: None,
            response: None,
            content: None,
            output: None,
            arguments: None,
            created_at_present: false,
            include_empty_usage: false,
            include_empty_metadata: false,
            citations: None,
            reason_present: false,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiResponseKernelFfiInput {
    operation: i64,
    sequence_number: u64,
    created_at: u64,
    summary_index: u64,
    prompt_token_count: u64,
    candidate_token_count: u64,
    total_token_count: u64,
    total_token_count_present: i64,
    cached_content_token_count: u64,
    thoughts_token_count: u64,
    tool_use_prompt_token_count: u64,
    response_id_present: i64,
    call_id_present: i64,
    model_present: i64,
    usage_present: i64,
    metadata_present: i64,
    signature_present: i64,
    namespace_present: i64,
    response_id: RichStringView,
    call_id: RichStringView,
    name: RichStringView,
    delta: RichStringView,
    reason: RichStringView,
    message: RichStringView,
    item: RichStringView,
    metadata: RichStringView,
    response: RichStringView,
    content: RichStringView,
    output: RichStringView,
    model: RichStringView,
    usage: RichStringView,
    signature: RichStringView,
    namespace: RichStringView,
    arguments: RichStringView,
    created_at_present: i64,
    include_empty_usage: i64,
    include_empty_metadata: i64,
    citations: RichStringView,
    reason_present: i64,
}

const _: () = assert!(std::mem::size_of::<GeminiResponseKernelFfiInput>() == 448);

unsafe extern "C" {
    fn prodex_mojo_gemini_response_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

const GEMINI_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;

fn gemini_kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn gemini_kernel_operation(operation: GeminiResponseKernelOperation) -> i64 {
    match operation {
        GeminiResponseKernelOperation::ResponseCreated => 1,
        GeminiResponseKernelOperation::ResponseCompleted => 2,
        GeminiResponseKernelOperation::ResponseIncomplete => 3,
        GeminiResponseKernelOperation::ResponseMetadata => 4,
        GeminiResponseKernelOperation::OutputItemAdded => 5,
        GeminiResponseKernelOperation::OutputItemDone => 6,
        GeminiResponseKernelOperation::FunctionCallArgumentsDelta => 7,
        GeminiResponseKernelOperation::OutputTextDelta => 8,
        GeminiResponseKernelOperation::ReasoningSummaryPartAdded => 9,
        GeminiResponseKernelOperation::ReasoningSummaryTextDelta => 10,
        GeminiResponseKernelOperation::TextSource => 11,
        GeminiResponseKernelOperation::ReasoningSource => 12,
        GeminiResponseKernelOperation::FunctionCallSource => 13,
        GeminiResponseKernelOperation::OutputTextContent => 14,
        GeminiResponseKernelOperation::MessageItem => 15,
        GeminiResponseKernelOperation::OutputMessageItem => 16,
        GeminiResponseKernelOperation::ResponseValue => 17,
        GeminiResponseKernelOperation::FunctionCallItem => 18,
        GeminiResponseKernelOperation::RawFunctionCallItem => 19,
        GeminiResponseKernelOperation::AddedFunctionCallItem => 20,
        GeminiResponseKernelOperation::ChatFunctionCallItem => 21,
        GeminiResponseKernelOperation::ResponseUsage => 22,
        GeminiResponseKernelOperation::StreamTextDelta => 23,
        GeminiResponseKernelOperation::StreamReasoningDelta => 24,
        GeminiResponseKernelOperation::FunctionCallArgumentsDeltaWithoutSequence => 25,
        GeminiResponseKernelOperation::BufferedResponse => 26,
        GeminiResponseKernelOperation::CitationText => 27,
        GeminiResponseKernelOperation::WebSearchCall => 28,
        GeminiResponseKernelOperation::StreamAssistantMessage => 29,
        GeminiResponseKernelOperation::StreamOutputItems => 30,
        GeminiResponseKernelOperation::ToolSearchCallItem => 31,
        GeminiResponseKernelOperation::CustomToolCallItem => 32,
    }
}

fn gemini_kernel_capacity(input: &GeminiResponseKernelInput<'_>) -> Result<usize, MojoError> {
    let views = [
        input.response_id,
        input.call_id,
        input.model,
        input.usage,
        input.metadata,
        input.signature,
        input.namespace,
        input.name,
        input.delta,
        input.reason,
        input.message,
        input.item,
        input.response,
        input.content,
        input.output,
        input.arguments,
        input.citations,
    ];
    let input_bytes = views.iter().flatten().try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })?;
    if input_bytes > GEMINI_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    input_bytes
        .checked_mul(6)
        .and_then(|value| value.checked_add(512))
        .ok_or(MojoError::InvalidInput)
}

/// Runs one bounded Gemini response/stream JSON builder in compiled Mojo.
pub fn gemini_response_kernel(input: GeminiResponseKernelInput<'_>) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let capacity = gemini_kernel_capacity(&input)?;
    let mut output = vec![0_u8; capacity];
    let ffi_input = GeminiResponseKernelFfiInput {
        operation: gemini_kernel_operation(input.operation),
        sequence_number: input.sequence_number,
        created_at: input.created_at,
        summary_index: input.summary_index,
        prompt_token_count: input.prompt_token_count,
        candidate_token_count: input.candidate_token_count,
        total_token_count: input.total_token_count,
        total_token_count_present: input.total_token_count_present,
        cached_content_token_count: input.cached_content_token_count,
        thoughts_token_count: input.thoughts_token_count,
        tool_use_prompt_token_count: input.tool_use_prompt_token_count,
        response_id_present: i64::from(input.response_id.is_some()),
        call_id_present: i64::from(input.call_id.is_some()),
        model_present: i64::from(input.model.is_some()),
        usage_present: i64::from(input.usage.is_some()),
        metadata_present: i64::from(input.metadata.is_some()),
        signature_present: i64::from(input.signature.is_some()),
        namespace_present: i64::from(input.namespace.is_some()),
        response_id: gemini_kernel_view(input.response_id),
        call_id: gemini_kernel_view(input.call_id),
        name: gemini_kernel_view(input.name),
        delta: gemini_kernel_view(input.delta),
        reason: gemini_kernel_view(input.reason),
        message: gemini_kernel_view(input.message),
        item: gemini_kernel_view(input.item),
        metadata: gemini_kernel_view(input.metadata),
        response: gemini_kernel_view(input.response),
        content: gemini_kernel_view(input.content),
        output: gemini_kernel_view(input.output),
        model: gemini_kernel_view(input.model),
        usage: gemini_kernel_view(input.usage),
        signature: gemini_kernel_view(input.signature),
        namespace: gemini_kernel_view(input.namespace),
        arguments: gemini_kernel_view(input.arguments),
        created_at_present: i64::from(input.created_at_present),
        include_empty_usage: i64::from(input.include_empty_usage),
        include_empty_metadata: i64::from(input.include_empty_metadata),
        citations: gemini_kernel_view(input.citations),
        reason_present: i64::from(input.reason_present),
    };
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_response_kernel_v1(
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
                domain: 7,
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
