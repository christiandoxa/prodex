use super::{
    MojoError, RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address,
};

const ANTHROPIC_REQUEST_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Deterministic JSON shapes emitted by the Anthropic request kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicRequestKernelOperation {
    RequestBody = 1,
    Message = 2,
    TextBlock = 3,
    ToolUseBlock = 4,
    ToolResultBlock = 5,
    ToolDeclaration = 6,
    ToolChoice = 7,
    WebSearchTool = 8,
    WebSearchCall = 9,
    ToolUseItem = 10,
    ToolUsage = 11,
    AppendMessage = 12,
    StreamMessageStart = 20,
    StreamTextStart = 21,
    StreamToolStart = 22,
    StreamWebSearchStart = 23,
    StreamThinkingStart = 24,
    StreamTextDelta = 25,
    StreamArgumentsDelta = 26,
    StreamThinkingDelta = 27,
    StreamCompleted = 28,
    StreamError = 29,
}

/// Inputs for one bounded Anthropic wire-shape operation.
#[derive(Debug, Clone, Copy)]
pub struct AnthropicRequestKernelInput<'a> {
    pub operation: AnthropicRequestKernelOperation,
    pub stream: bool,
    pub choice_kind: i64,
    pub created_at: u64,
    pub count: u64,
    pub index: Option<&'a str>,
    pub model: Option<&'a str>,
    pub system: Option<&'a str>,
    pub messages: Option<&'a str>,
    pub max_tokens: Option<&'a str>,
    pub temperature: Option<&'a str>,
    pub top_p: Option<&'a str>,
    pub stop_sequences: Option<&'a str>,
    pub tools: Option<&'a str>,
    pub tool_choice: Option<&'a str>,
    pub role: Option<&'a str>,
    pub blocks: Option<&'a str>,
    pub id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub input: Option<&'a str>,
    pub content: Option<&'a str>,
    pub arguments: Option<&'a str>,
    pub delta: Option<&'a str>,
    pub error: Option<&'a str>,
    pub queries: Option<&'a str>,
    pub allowed_domains: Option<&'a str>,
    pub blocked_domains: Option<&'a str>,
    pub user_location: Option<&'a str>,
    pub max_uses: Option<&'a str>,
    pub tool_use_id: Option<&'a str>,
}

impl<'a> AnthropicRequestKernelInput<'a> {
    pub const fn new(operation: AnthropicRequestKernelOperation) -> Self {
        Self {
            operation,
            stream: false,
            choice_kind: 0,
            created_at: 0,
            count: 0,
            index: None,
            model: None,
            system: None,
            messages: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            role: None,
            blocks: None,
            id: None,
            name: None,
            namespace: None,
            input: None,
            content: None,
            arguments: None,
            delta: None,
            error: None,
            queries: None,
            allowed_domains: None,
            blocked_domains: None,
            user_location: None,
            max_uses: None,
            tool_use_id: None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct AnthropicRequestKernelFfiInput {
    operation: i64,
    stream: i64,
    choice_kind: i64,
    created_at: u64,
    count: u64,
    index: RichStringView,
    model: RichStringView,
    system: RichStringView,
    messages: RichStringView,
    max_tokens: RichStringView,
    temperature: RichStringView,
    top_p: RichStringView,
    stop_sequences: RichStringView,
    tools: RichStringView,
    tool_choice: RichStringView,
    role: RichStringView,
    blocks: RichStringView,
    id: RichStringView,
    name: RichStringView,
    namespace: RichStringView,
    input: RichStringView,
    content: RichStringView,
    arguments: RichStringView,
    delta: RichStringView,
    error: RichStringView,
    queries: RichStringView,
    allowed_domains: RichStringView,
    blocked_domains: RichStringView,
    user_location: RichStringView,
    max_uses: RichStringView,
    tool_use_id: RichStringView,
}

const _: () = assert!(std::mem::size_of::<AnthropicRequestKernelFfiInput>() == 456);

unsafe extern "C" {
    fn prodex_mojo_rich_anthropic_request_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(super::view).unwrap_or_default()
}

fn input_bytes(input: &AnthropicRequestKernelInput<'_>) -> Result<usize, MojoError> {
    [
        input.index,
        input.model,
        input.system,
        input.messages,
        input.max_tokens,
        input.temperature,
        input.top_p,
        input.stop_sequences,
        input.tools,
        input.tool_choice,
        input.role,
        input.blocks,
        input.id,
        input.name,
        input.namespace,
        input.input,
        input.content,
        input.arguments,
        input.delta,
        input.error,
        input.queries,
        input.allowed_domains,
        input.blocked_domains,
        input.user_location,
        input.max_uses,
        input.tool_use_id,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}

/// Runs one bounded Anthropic request, message, tool, web-search, or stream
/// JSON writer in compiled Mojo.
pub fn anthropic_request_kernel(
    input: AnthropicRequestKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > ANTHROPIC_REQUEST_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input_bytes
        .checked_mul(6)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let ffi_input = AnthropicRequestKernelFfiInput {
        operation: input.operation as i64,
        stream: i64::from(input.stream),
        choice_kind: input.choice_kind,
        created_at: input.created_at,
        count: input.count,
        index: kernel_view(input.index),
        model: kernel_view(input.model),
        system: kernel_view(input.system),
        messages: kernel_view(input.messages),
        max_tokens: kernel_view(input.max_tokens),
        temperature: kernel_view(input.temperature),
        top_p: kernel_view(input.top_p),
        stop_sequences: kernel_view(input.stop_sequences),
        tools: kernel_view(input.tools),
        tool_choice: kernel_view(input.tool_choice),
        role: kernel_view(input.role),
        blocks: kernel_view(input.blocks),
        id: kernel_view(input.id),
        name: kernel_view(input.name),
        namespace: kernel_view(input.namespace),
        input: kernel_view(input.input),
        content: kernel_view(input.content),
        arguments: kernel_view(input.arguments),
        delta: kernel_view(input.delta),
        error: kernel_view(input.error),
        queries: kernel_view(input.queries),
        allowed_domains: kernel_view(input.allowed_domains),
        blocked_domains: kernel_view(input.blocked_domains),
        user_location: kernel_view(input.user_location),
        max_uses: kernel_view(input.max_uses),
        tool_use_id: kernel_view(input.tool_use_id),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_rich_anthropic_request_kernel_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(super::status_error(status, 9, 1, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}
