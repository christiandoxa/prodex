//! Typed, borrowed JSON-tree ABI for complete semantic transforms. JSON wire
//! parsing and compatibility serialization remain outside the Mojo bridge.
use crate::MojoError;

/// Maximum UTF-8 byte length accepted for a provider error body or member.
pub const PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES: usize = 65_536;
const PROVIDER_ERROR_REJECTION_MAX_RAW_BYTES: usize = 1_048_576;
const PROVIDER_ERROR_REJECTION_MAX_NODES: usize = PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES + 1;

#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum JsonKind {
    Null,
    False,
    True,
    Number,
    String,
    Array,
    Object,
}

#[derive(Clone, Copy, Debug)]
pub struct JsonNode<'a> {
    pub kind: JsonKind,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
    pub parent: Option<usize>,
    pub key: &'a str,
    pub text: &'a str,
    pub raw_start: usize,
    pub raw_length: usize,
}

#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum ChatToolOperation {
    Tools,
    Choice,
    WebSearchOptions,
    WithoutWebSearch,
    FlattenName,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct StringView {
    address: u64,
    length: u64,
}
impl From<&str> for StringView {
    fn from(value: &str) -> Self {
        Self {
            address: value.as_ptr() as u64,
            length: value.len() as u64,
        }
    }
}
#[repr(C)]
struct NodeFfi {
    kind: i64,
    first_child: i64,
    next_sibling: i64,
    parent: i64,
    key: StringView,
    text: StringView,
    raw_start: i64,
    raw_length: i64,
}
const _: () = {
    assert!(std::mem::size_of::<NodeFfi>() == 80);
    assert!(std::mem::align_of::<NodeFfi>() == 8);
    assert!(std::mem::size_of::<StringView>() == 16);
    assert!(std::mem::size_of::<usize>() == 8);
};

unsafe extern "C" {
    fn prodex_mojo_chat_tools_v1(
        abi: i64,
        operation: i64,
        thinking: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
    fn prodex_mojo_openai_chat_request_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
    fn prodex_mojo_openai_chat_response_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
    fn prodex_mojo_anthropic_chat_request_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
    fn prodex_provider_error_rejects_member_v1(
        abi: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        member: u64,
        member_length: i64,
        normalized_member: u64,
        normalized_member_capacity: i64,
        prefix: u64,
        prefix_capacity: i64,
        output: u64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}
fn optional_index(value: Option<usize>, count: usize) -> Result<i64, MojoError> {
    match value {
        None => Ok(-1),
        Some(value) if value < count => signed(value),
        _ => Err(MojoError::InvalidInput),
    }
}
fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn ffi_nodes(nodes: &[JsonNode<'_>], raw: &str) -> Result<Vec<NodeFfi>, MojoError> {
    if nodes.is_empty() || nodes.len() > i64::MAX as usize / 80 {
        return Err(MojoError::InvalidInput);
    }
    nodes
        .iter()
        .map(|node| {
            let end = node
                .raw_start
                .checked_add(node.raw_length)
                .ok_or(MojoError::InvalidInput)?;
            raw.get(node.raw_start..end)
                .ok_or(MojoError::InvalidInput)?;
            Ok(NodeFfi {
                kind: node.kind as i64,
                first_child: optional_index(node.first_child, nodes.len())?,
                next_sibling: optional_index(node.next_sibling, nodes.len())?,
                parent: optional_index(node.parent, nodes.len())?,
                key: node.key.into(),
                text: node.text.into(),
                raw_start: signed(node.raw_start)?,
                raw_length: signed(node.raw_length)?,
            })
        })
        .collect()
}

/// Apply the provider error request-member policy to a Serde-built tree.
pub fn provider_error_rejects_member(
    nodes: &[JsonNode<'_>],
    raw: &str,
    member: &str,
) -> Result<bool, MojoError> {
    if nodes.len() > PROVIDER_ERROR_REJECTION_MAX_NODES
        || raw.len() > PROVIDER_ERROR_REJECTION_MAX_RAW_BYTES
        || member.len() > PROVIDER_ERROR_REJECTION_MAX_INPUT_BYTES
    {
        return Err(MojoError::InvalidInput);
    }
    let input = ffi_nodes(nodes, raw)?;
    let mut normalized_member: Vec<u8> = Vec::new();
    normalized_member
        .try_reserve_exact(member.len())
        .map_err(|_| MojoError::Capacity)?;
    normalized_member.resize(member.len(), 0);
    let mut prefix: Vec<i64> = Vec::new();
    prefix
        .try_reserve_exact(member.len())
        .map_err(|_| MojoError::Capacity)?;
    prefix.resize(member.len(), 0);
    let mut output = [0_i64; 2];
    let result = unsafe {
        prodex_provider_error_rejects_member_v1(
            1,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            member.as_ptr() as u64,
            signed(member.len())?,
            normalized_member.as_mut_ptr() as u64,
            signed(normalized_member.len())?,
            prefix.as_mut_ptr() as u64,
            signed(prefix.len())?,
            output.as_mut_ptr() as u64,
        )
    };
    status(result)?;
    if !matches!(output[0], 0 | 1) || output[1] < 0 || output[1] as usize > member.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output[0] == 1)
}

pub fn transform_chat_tools(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: ChatToolOperation,
    thinking: bool,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(
        nodes,
        raw,
        operation as i64,
        thinking,
        prodex_mojo_chat_tools_v1,
    )
}

pub(super) type JsonKernel =
    unsafe extern "C" fn(i64, i64, i64, u64, i64, u64, i64, u64, i64, i64, u64, i64, u64) -> i64;

pub(super) fn transform_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: i64,
    flag: bool,
    kernel: JsonKernel,
) -> Result<Option<Vec<u8>>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    let mut scratch = vec![StringView::default(); nodes.len()];
    let mut metadata = [-1_i64; 2];
    let mut invoke = |output: Option<&mut [u8]>| -> Result<(bool, usize), MojoError> {
        let (measuring, address, capacity) = match output {
            Some(output) => (0, output.as_mut_ptr() as u64, signed(output.len())?),
            None => (1, 0, 0),
        };
        // SAFETY: all pointer/length pairs refer to immutable input or distinct
        // caller-owned output/scratch/meta allocations for the synchronous call.
        // The kernel validates indices and parent relationships before traversal.
        status(unsafe {
            kernel(
                1,
                operation,
                i64::from(flag),
                input.as_ptr() as u64,
                signed(input.len())?,
                raw.as_ptr() as u64,
                signed(raw.len())?,
                scratch.as_mut_ptr() as u64,
                signed(scratch.len())?,
                measuring,
                address,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        })?;
        let present = match metadata[0] {
            0 => false,
            1 => true,
            _ => return Err(MojoError::InvalidOutput),
        };
        let length = usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?;
        Ok((present, length))
    };
    let (present, required) = invoke(None)?;
    if !present {
        return Ok(None);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(required)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(required, 0);
    if invoke(Some(&mut output))? != (true, required) {
        return Err(MojoError::InvalidOutput);
    }
    std::str::from_utf8(&output).map_err(|_| MojoError::InvalidOutput)?;
    Ok(Some(output))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiChatRequestTransform {
    Body(Vec<u8>),
    Rejected(String),
}

pub fn transform_openai_chat_request(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<OpenAiChatRequestTransform, MojoError> {
    let output = transform_json(nodes, raw, 0, false, prodex_mojo_openai_chat_request_v1)?
        .ok_or(MojoError::InvalidOutput)?;
    let Some((&tag, payload)) = output.split_first() else {
        return Err(MojoError::InvalidOutput);
    };
    match tag {
        b'S' => Ok(OpenAiChatRequestTransform::Body(payload.to_vec())),
        b'E' => String::from_utf8(payload.to_vec())
            .map(OpenAiChatRequestTransform::Rejected)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Convert a parsed chat completion response to the Responses JSON body.
pub fn transform_openai_chat_response(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Vec<u8>, MojoError> {
    transform_json(nodes, raw, 0, false, prodex_mojo_openai_chat_response_v1)?
        .ok_or(MojoError::InvalidOutput)
}

/// Convert one parsed chat completion SSE event to a Responses event.
pub fn transform_openai_chat_stream_event(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(nodes, raw, 1, false, prodex_mojo_openai_chat_response_v1)
}

/// Convert one parsed DeepSeek chat-completion SSE event to a Responses event.
pub fn transform_deepseek_chat_stream_event(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(nodes, raw, 2, false, prodex_mojo_openai_chat_response_v1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicChatRequestTransform {
    Body(Vec<u8>),
    Degraded {
        body: Vec<u8>,
        context_size: &'static str,
    },
    Rejected(String),
}

pub fn transform_anthropic_chat_request(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<AnthropicChatRequestTransform, MojoError> {
    let output = transform_json(nodes, raw, 0, false, prodex_mojo_anthropic_chat_request_v1)?
        .ok_or(MojoError::InvalidOutput)?;
    let Some((&tag, payload)) = output.split_first() else {
        return Err(MojoError::InvalidOutput);
    };
    match tag {
        b'S' => Ok(AnthropicChatRequestTransform::Body(payload.to_vec())),
        b'D' if payload.len() >= 2 => {
            let context_size = match payload[0] {
                b'1' => "low",
                b'2' => "medium",
                b'3' => "high",
                _ => return Err(MojoError::InvalidOutput),
            };
            Ok(AnthropicChatRequestTransform::Degraded {
                body: payload[1..].to_vec(),
                context_size,
            })
        }
        b'E' => String::from_utf8(payload.to_vec())
            .map(AnthropicChatRequestTransform::Rejected)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod provider_error_tests {
    use super::{JsonKind, JsonNode, provider_error_rejects_member};

    #[test]
    fn provider_error_member_kernel_matches_expected_object_fields() {
        let nodes = [
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(1),
                next_sibling: None,
                parent: None,
                key: "",
                text: "",
                raw_start: 0,
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(2),
                parent: Some(0),
                key: "code",
                text: "UNKNOWN_PARAMETER",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(0),
                key: "param",
                text: "WEB_search-options",
                raw_start: 0,
                raw_length: 0,
            },
        ];

        assert_eq!(
            provider_error_rejects_member(&nodes, "{}", "webSearchOptions"),
            Ok(true)
        );
    }
}
