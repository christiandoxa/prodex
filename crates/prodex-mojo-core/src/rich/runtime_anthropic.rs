use super::{
    MojoError, RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address, view,
};

const RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Deterministic Anthropic runtime JSON and SSE shapes.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAnthropicKernelOperation {
    SseEvent = 1,
    MessageSse = 2,
    ResponseMessage = 3,
    Usage = 4,
    InputText = 5,
    ImagePart = 6,
    FunctionCall = 7,
    FunctionCallOutput = 8,
    ShellToolResult = 9,
    ComputerToolResult = 10,
    ToolUseBlock = 11,
    McpCallBlocks = 12,
    McpApprovalBlock = 13,
    McpListToolsBlock = 14,
    ServerToolBlock = 15,
    ThinkingBlock = 16,
    TextBlock = 17,
}

pub const RUNTIME_ANTHROPIC_FLAG_ERROR: i64 = 1;
pub const RUNTIME_ANTHROPIC_FLAG_MAX_OUTPUT_LENGTH: i64 = 2;
pub const RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS: i64 = 4;

/// Inputs for one bounded Anthropic runtime wire-shape operation.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeAnthropicKernelInput<'a> {
    pub operation: RuntimeAnthropicKernelOperation,
    pub index: u64,
    pub flags: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub web_search_requests: u64,
    pub web_fetch_requests: u64,
    pub code_execution_requests: u64,
    pub tool_search_requests: u64,
    pub max_output_length: u64,
    pub id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub block_type: Option<&'a str>,
    pub server_name: Option<&'a str>,
    pub text: Option<&'a str>,
    pub input: Option<&'a str>,
    pub output: Option<&'a str>,
    pub content: Option<&'a str>,
    pub usage: Option<&'a str>,
    pub stop_reason: Option<&'a str>,
    pub message: Option<&'a str>,
}

impl<'a> RuntimeAnthropicKernelInput<'a> {
    pub const fn new(operation: RuntimeAnthropicKernelOperation) -> Self {
        Self {
            operation,
            index: 0,
            flags: 0,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: 0,
            web_search_requests: 0,
            web_fetch_requests: 0,
            code_execution_requests: 0,
            tool_search_requests: 0,
            max_output_length: 0,
            id: None,
            name: None,
            block_type: None,
            server_name: None,
            text: None,
            input: None,
            output: None,
            content: None,
            usage: None,
            stop_reason: None,
            message: None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RuntimeAnthropicKernelFfiInput {
    operation: i64,
    index: u64,
    flags: i64,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    max_output_length: u64,
    max_output_length_present: i64,
    id_present: i64,
    name_present: i64,
    block_type_present: i64,
    server_name_present: i64,
    text_present: i64,
    input_present: i64,
    output_present: i64,
    content_present: i64,
    usage_present: i64,
    stop_reason_present: i64,
    message_present: i64,
    id: RichStringView,
    name: RichStringView,
    block_type: RichStringView,
    server_name: RichStringView,
    text: RichStringView,
    input: RichStringView,
    output: RichStringView,
    content: RichStringView,
    usage: RichStringView,
    stop_reason: RichStringView,
    message: RichStringView,
}

const _: () = assert!(std::mem::size_of::<RuntimeAnthropicKernelFfiInput>() == 360);

unsafe extern "C" {
    fn prodex_mojo_rich_runtime_anthropic_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn input_bytes(input: &RuntimeAnthropicKernelInput<'_>) -> Result<usize, MojoError> {
    [
        input.id,
        input.name,
        input.block_type,
        input.server_name,
        input.text,
        input.input,
        input.output,
        input.content,
        input.usage,
        input.stop_reason,
        input.message,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}

/// Runs one bounded Anthropic runtime wire-shape operation in compiled Mojo.
pub fn runtime_anthropic_kernel(
    input: RuntimeAnthropicKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    // ponytail: fixed 16x writer headroom; add a sizing pass if new shapes exceed it.
    let capacity = input_bytes
        .checked_mul(16)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let ffi_input = RuntimeAnthropicKernelFfiInput {
        operation: input.operation as i64,
        index: input.index,
        flags: input.flags,
        input_tokens: input.input_tokens,
        output_tokens: input.output_tokens,
        cached_tokens: input.cached_tokens,
        web_search_requests: input.web_search_requests,
        web_fetch_requests: input.web_fetch_requests,
        code_execution_requests: input.code_execution_requests,
        tool_search_requests: input.tool_search_requests,
        max_output_length: input.max_output_length,
        max_output_length_present: i64::from(
            input.flags & RUNTIME_ANTHROPIC_FLAG_MAX_OUTPUT_LENGTH != 0,
        ),
        id_present: i64::from(input.id.is_some()),
        name_present: i64::from(input.name.is_some()),
        block_type_present: i64::from(input.block_type.is_some()),
        server_name_present: i64::from(input.server_name.is_some()),
        text_present: i64::from(input.text.is_some()),
        input_present: i64::from(input.input.is_some()),
        output_present: i64::from(input.output.is_some()),
        content_present: i64::from(input.content.is_some()),
        usage_present: i64::from(input.usage.is_some()),
        stop_reason_present: i64::from(input.stop_reason.is_some()),
        message_present: i64::from(input.message.is_some()),
        id: kernel_view(input.id),
        name: kernel_view(input.name),
        block_type: kernel_view(input.block_type),
        server_name: kernel_view(input.server_name),
        text: kernel_view(input.text),
        input: kernel_view(input.input),
        output: kernel_view(input.output),
        content: kernel_view(input.content),
        usage: kernel_view(input.usage),
        stop_reason: kernel_view(input.stop_reason),
        message: kernel_view(input.message),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_rich_runtime_anthropic_kernel_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(super::status_error(status, 9, 2, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}
