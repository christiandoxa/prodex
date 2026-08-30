use super::{
    MojoError, RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address, view,
};

const OPENAI_COMPAT_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;
const OPENAI_COMPAT_STATUS_REJECTED: i64 = 5;

/// Deterministic operation implemented by the OpenAI chat-compatibility kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatKernelOperation {
    ValidateRequest = 1,
    ParameterSupport = 2,
    RequestMessage = 3,
    OutputText = 4,
    ResponseUsage = 5,
    SplitToolName = 6,
    RtkArguments = 7,
    StreamEvent = 8,
}

/// Message shape requested from the chat-compatibility kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatMessageKind {
    System = 1,
    User = 2,
    Role = 3,
    FunctionCall = 4,
    FunctionCallOutput = 5,
}

/// Stream event shape requested from the chat-compatibility kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatStreamKind {
    Done = 1,
    TextDelta = 2,
    FunctionCallArgumentsDelta = 3,
}

/// Typed request facts passed to Mojo for compatibility validation.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatValidationInput {
    pub provider: &'static str,
    pub has_messages: bool,
    pub has_response_format: bool,
    pub has_reasoning: bool,
    pub has_previous_response_id: bool,
    pub has_text_format: bool,
    pub n_gt_one: bool,
    pub has_metadata: bool,
    pub has_safety_identifier: bool,
    pub has_web_search_options: bool,
    pub tools_non_function: bool,
    pub tool_choice_invalid: bool,
    pub parallel_tool_calls_false: bool,
    pub has_logprobs: bool,
    pub has_top_logprobs: bool,
    pub has_stop_sequences: bool,
    pub input_custom_tool: bool,
    pub input_non_text: bool,
}

/// Typed message fields passed to Mojo for chat request shaping.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatMessageInput<'a> {
    pub kind: OpenAiCompatMessageKind,
    pub role: Option<&'a str>,
    pub text: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub name: Option<&'a str>,
    pub arguments: Option<&'a str>,
}

/// Typed stream fields passed to Mojo for SSE event shaping.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatStreamInput<'a> {
    pub kind: OpenAiCompatStreamKind,
    pub call_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub delta: Option<&'a str>,
}

/// One unsupported Responses parameter reported by the Mojo kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatParameter {
    pub field: String,
    pub reason: String,
}

/// Error returned by a bounded OpenAI compatibility kernel call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiCompatError {
    Mojo(MojoError),
    Rejected(String),
}

impl From<MojoError> for OpenAiCompatError {
    fn from(error: MojoError) -> Self {
        Self::Mojo(error)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct OpenAiCompatKernelFfiInput {
    operation: i64,
    message_kind: i64,
    stream_kind: i64,
    has_messages: i64,
    has_response_format: i64,
    has_reasoning: i64,
    has_previous_response_id: i64,
    has_text_format: i64,
    n_gt_one: i64,
    has_metadata: i64,
    has_safety_identifier: i64,
    has_web_search_options: i64,
    tools_non_function: i64,
    tool_choice_invalid: i64,
    parallel_tool_calls_false: i64,
    has_logprobs: i64,
    has_top_logprobs: i64,
    has_stop_sequences: i64,
    input_custom_tool: i64,
    input_non_text: i64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    total_tokens_present: i64,
    provider_present: i64,
    role_present: i64,
    text_present: i64,
    call_id_present: i64,
    namespace_present: i64,
    name_present: i64,
    arguments_present: i64,
    delta_present: i64,
    provider: RichStringView,
    role: RichStringView,
    text: RichStringView,
    call_id: RichStringView,
    namespace: RichStringView,
    name: RichStringView,
    arguments: RichStringView,
    delta: RichStringView,
}

const _: () = assert!(std::mem::size_of::<OpenAiCompatKernelFfiInput>() == 384);

unsafe extern "C" {
    fn prodex_mojo_openai_compat_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy)]
struct KernelInput<'a> {
    operation: OpenAiCompatKernelOperation,
    message_kind: Option<OpenAiCompatMessageKind>,
    stream_kind: Option<OpenAiCompatStreamKind>,
    validation: Option<OpenAiCompatValidationInput>,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    total_tokens_present: bool,
    role: Option<&'a str>,
    text: Option<&'a str>,
    call_id: Option<&'a str>,
    namespace: Option<&'a str>,
    name: Option<&'a str>,
    arguments: Option<&'a str>,
    delta: Option<&'a str>,
}

impl<'a> KernelInput<'a> {
    fn empty(operation: OpenAiCompatKernelOperation) -> Self {
        Self {
            operation,
            message_kind: None,
            stream_kind: None,
            validation: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            total_tokens_present: false,
            role: None,
            text: None,
            call_id: None,
            namespace: None,
            name: None,
            arguments: None,
            delta: None,
        }
    }
}

fn operation_code(operation: OpenAiCompatKernelOperation) -> i64 {
    operation as i64
}

fn message_kind_code(kind: Option<OpenAiCompatMessageKind>) -> i64 {
    kind.map(|kind| kind as i64).unwrap_or(0)
}

fn stream_kind_code(kind: Option<OpenAiCompatStreamKind>) -> i64 {
    kind.map(|kind| kind as i64).unwrap_or(0)
}

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn input_bytes(input: &KernelInput<'_>) -> Result<usize, OpenAiCompatError> {
    [
        input.validation.map(|validation| validation.provider),
        input.role,
        input.text,
        input.call_id,
        input.namespace,
        input.name,
        input.arguments,
        input.delta,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(OpenAiCompatError::Mojo(MojoError::InvalidInput))
    })
}

fn ffi_input(input: &KernelInput<'_>) -> OpenAiCompatKernelFfiInput {
    let validation = input.validation.unwrap_or(OpenAiCompatValidationInput {
        provider: "",
        has_messages: false,
        has_response_format: false,
        has_reasoning: false,
        has_previous_response_id: false,
        has_text_format: false,
        n_gt_one: false,
        has_metadata: false,
        has_safety_identifier: false,
        has_web_search_options: false,
        tools_non_function: false,
        tool_choice_invalid: false,
        parallel_tool_calls_false: false,
        has_logprobs: false,
        has_top_logprobs: false,
        has_stop_sequences: false,
        input_custom_tool: false,
        input_non_text: false,
    });
    OpenAiCompatKernelFfiInput {
        operation: operation_code(input.operation),
        message_kind: message_kind_code(input.message_kind),
        stream_kind: stream_kind_code(input.stream_kind),
        has_messages: i64::from(validation.has_messages),
        has_response_format: i64::from(validation.has_response_format),
        has_reasoning: i64::from(validation.has_reasoning),
        has_previous_response_id: i64::from(validation.has_previous_response_id),
        has_text_format: i64::from(validation.has_text_format),
        n_gt_one: i64::from(validation.n_gt_one),
        has_metadata: i64::from(validation.has_metadata),
        has_safety_identifier: i64::from(validation.has_safety_identifier),
        has_web_search_options: i64::from(validation.has_web_search_options),
        tools_non_function: i64::from(validation.tools_non_function),
        tool_choice_invalid: i64::from(validation.tool_choice_invalid),
        parallel_tool_calls_false: i64::from(validation.parallel_tool_calls_false),
        has_logprobs: i64::from(validation.has_logprobs),
        has_top_logprobs: i64::from(validation.has_top_logprobs),
        has_stop_sequences: i64::from(validation.has_stop_sequences),
        input_custom_tool: i64::from(validation.input_custom_tool),
        input_non_text: i64::from(validation.input_non_text),
        input_tokens: input.input_tokens,
        output_tokens: input.output_tokens,
        total_tokens: input.total_tokens,
        total_tokens_present: i64::from(input.total_tokens_present),
        provider_present: i64::from(input.validation.is_some()),
        role_present: i64::from(input.role.is_some()),
        text_present: i64::from(input.text.is_some()),
        call_id_present: i64::from(input.call_id.is_some()),
        namespace_present: i64::from(input.namespace.is_some()),
        name_present: i64::from(input.name.is_some()),
        arguments_present: i64::from(input.arguments.is_some()),
        delta_present: i64::from(input.delta.is_some()),
        provider: kernel_view(input.validation.map(|validation| validation.provider)),
        role: kernel_view(input.role),
        text: kernel_view(input.text),
        call_id: kernel_view(input.call_id),
        namespace: kernel_view(input.namespace),
        name: kernel_view(input.name),
        arguments: kernel_view(input.arguments),
        delta: kernel_view(input.delta),
    }
}

fn error_for_status(status: i64) -> OpenAiCompatError {
    OpenAiCompatError::Mojo(match status {
        2 => MojoError::Structured(super::MojoIssue {
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
    })
}

fn run_kernel(input: KernelInput<'_>) -> Result<Vec<u8>, OpenAiCompatError> {
    ensure_rich_abi().map_err(OpenAiCompatError::Mojo)?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > OPENAI_COMPAT_KERNEL_MAX_BYTES {
        return Err(OpenAiCompatError::Mojo(MojoError::InvalidInput));
    }
    let capacity = input_bytes
        .checked_mul(8)
        .and_then(|value| value.checked_add(8_192))
        .ok_or(OpenAiCompatError::Mojo(MojoError::InvalidInput))?;
    let ffi_input = ffi_input(&input);
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_openai_compat_kernel_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len())
                .map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidInput))?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    let written =
        usize::try_from(written).map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidOutput))?;
    if written > output.len() {
        return Err(OpenAiCompatError::Mojo(MojoError::InvalidOutput));
    }
    output.truncate(written);
    if status == 0 {
        return Ok(output);
    }
    if status == OPENAI_COMPAT_STATUS_REJECTED {
        return Err(OpenAiCompatError::Rejected(
            String::from_utf8(output)
                .map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidOutput))?,
        ));
    }
    Err(error_for_status(status))
}

/// Validates the already-decoded Responses request facts in Mojo.
pub fn openai_compat_validate_request(
    input: OpenAiCompatValidationInput,
) -> Result<(), OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::ValidateRequest);
    kernel.validation = Some(input);
    run_kernel(kernel).map(|_| ())
}

/// Returns the compatibility parameter report generated by Mojo.
pub fn openai_compat_supported_params(
    provider: &'static str,
) -> Result<Vec<OpenAiCompatParameter>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::ParameterSupport);
    kernel.validation = Some(OpenAiCompatValidationInput {
        provider,
        has_messages: false,
        has_response_format: false,
        has_reasoning: false,
        has_previous_response_id: false,
        has_text_format: false,
        n_gt_one: false,
        has_metadata: false,
        has_safety_identifier: false,
        has_web_search_options: false,
        tools_non_function: false,
        tool_choice_invalid: false,
        parallel_tool_calls_false: false,
        has_logprobs: false,
        has_top_logprobs: false,
        has_stop_sequences: false,
        input_custom_tool: false,
        input_non_text: false,
    });
    let output = run_kernel(kernel)?;
    output
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.is_empty())
        .map(|record| {
            let separator = record
                .iter()
                .position(|byte| *byte == 0)
                .ok_or(OpenAiCompatError::Mojo(MojoError::InvalidOutput))?;
            let field = String::from_utf8(record[..separator].to_vec())
                .map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidOutput))?;
            let reason = String::from_utf8(record[separator + 1..].to_vec())
                .map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidOutput))?;
            Ok(OpenAiCompatParameter { field, reason })
        })
        .collect()
}

/// Builds one chat-completions message object in Mojo.
pub fn openai_compat_request_message(
    input: OpenAiCompatMessageInput<'_>,
) -> Result<Vec<u8>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::RequestMessage);
    kernel.message_kind = Some(input.kind);
    kernel.role = input.role;
    kernel.text = input.text;
    kernel.call_id = input.call_id;
    kernel.namespace = input.namespace;
    kernel.name = input.name;
    kernel.arguments = input.arguments;
    run_kernel(kernel)
}

/// Builds one Responses output-text content object in Mojo.
pub fn openai_compat_output_text(text: &str) -> Result<Vec<u8>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::OutputText);
    kernel.text = Some(text);
    run_kernel(kernel)
}

/// Builds the normalized Responses usage object in Mojo.
pub fn openai_compat_response_usage(
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    total_tokens_present: bool,
) -> Result<Vec<u8>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::ResponseUsage);
    kernel.input_tokens = input_tokens;
    kernel.output_tokens = output_tokens;
    kernel.total_tokens = total_tokens;
    kernel.total_tokens_present = total_tokens_present;
    run_kernel(kernel)
}

/// Splits a flat tool name into the Responses namespace/name JSON object.
pub fn openai_compat_split_tool_name(name: &str) -> Result<Vec<u8>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::SplitToolName);
    kernel.name = Some(name);
    run_kernel(kernel)
}

/// Applies the chat-compatibility RTK command rewrite in Mojo.
pub fn openai_compat_rtk_arguments(
    name: &str,
    arguments: &str,
) -> Result<String, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::RtkArguments);
    kernel.name = Some(name);
    kernel.arguments = Some(arguments);
    String::from_utf8(run_kernel(kernel)?)
        .map_err(|_| OpenAiCompatError::Mojo(MojoError::InvalidOutput))
}

/// Builds one normalized Responses SSE event in Mojo.
pub fn openai_compat_stream_event(
    input: OpenAiCompatStreamInput<'_>,
) -> Result<Vec<u8>, OpenAiCompatError> {
    let mut kernel = KernelInput::empty(OpenAiCompatKernelOperation::StreamEvent);
    kernel.stream_kind = Some(input.kind);
    kernel.call_id = input.call_id;
    kernel.name = input.name;
    kernel.delta = input.delta;
    run_kernel(kernel)
}
