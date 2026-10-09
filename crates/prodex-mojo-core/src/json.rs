//! Typed, borrowed JSON-tree ABI for complete semantic transforms. JSON wire
//! parsing and compatibility serialization remain outside the Mojo bridge.
#[cfg(feature = "mojo-rich")]
mod retry_headers;
#[cfg(feature = "mojo-rich")]
pub use self::retry_headers::runtime_retry_after_json;
mod response_metadata;
pub use self::response_metadata::{
    RuntimeResponseMetadataJsonPlan, runtime_response_event_is_completed,
    runtime_response_metadata_json,
};
mod session_report;
pub use self::session_report::{
    SessionReportOrderKey, SessionReportUpdatePlan, session_report_metadata_json,
    session_report_order, session_report_record_shape, session_report_timestamp_sort_key,
    session_report_update_json,
};
mod session_selector;
pub use self::session_selector::{session_selector_is_full, session_selector_matches};
mod kiro_catalog;
pub use self::kiro_catalog::{
    KiroModelCatalogModel, KiroModelCatalogPlan, kiro_model_catalog_plan,
};

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
    fn prodex_runtime_proxy_request_shape_v1(
        abi: i64,
        raw: u64,
        raw_length: i64,
        session_present: i64,
        output: u64,
    ) -> i64;
    fn prodex_runtime_response_metadata_json_v1(
        abi_version: i64,
        raw_address: u64,
        raw_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_grounding_v1(
        abi_version: i64,
        operation: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        response_id_address: u64,
        response_id_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
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

fn ffi_nodes_with_text(
    nodes: &[JsonNode<'_>],
    raw: &str,
    text_overrides: Option<&[Option<String>]>,
) -> Result<Vec<NodeFfi>, MojoError> {
    if nodes.is_empty() || nodes.len() > i64::MAX as usize / 80 {
        return Err(MojoError::InvalidInput);
    }
    if text_overrides.is_some_and(|overrides| overrides.len() != nodes.len()) {
        return Err(MojoError::InvalidInput);
    }
    if text_overrides.is_some_and(|overrides| {
        overrides
            .iter()
            .zip(nodes)
            .any(|(text, node)| text.is_some() && !matches!(node.kind, JsonKind::Number))
    }) {
        return Err(MojoError::InvalidInput);
    }
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let end = node
                .raw_start
                .checked_add(node.raw_length)
                .ok_or(MojoError::InvalidInput)?;
            raw.get(node.raw_start..end)
                .ok_or(MojoError::InvalidInput)?;
            let text = text_overrides
                .and_then(|overrides| overrides[index].as_deref())
                .unwrap_or(node.text);
            Ok(NodeFfi {
                kind: node.kind as i64,
                first_child: optional_index(node.first_child, nodes.len())?,
                next_sibling: optional_index(node.next_sibling, nodes.len())?,
                parent: optional_index(node.parent, nodes.len())?,
                key: node.key.into(),
                text: text.into(),
                raw_start: signed(node.raw_start)?,
                raw_length: signed(node.raw_length)?,
            })
        })
        .collect()
}

fn ffi_nodes(nodes: &[JsonNode<'_>], raw: &str) -> Result<Vec<NodeFfi>, MojoError> {
    ffi_nodes_with_text(nodes, raw, None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiGroundingOperation {
    CitationText = 0,
    WebSearchCall = 1,
}

pub fn gemini_grounding(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiGroundingOperation,
    response_id: Option<&str>,
) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    let response_id = response_id.unwrap_or_default();
    let capacity = raw
        .len()
        .checked_mul(6)
        .and_then(|value| value.checked_add(response_id.len().saturating_mul(2)))
        .and_then(|value| value.checked_add(512))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity.max(512)];
    let mut written = 0_i64;
    status(unsafe {
        prodex_mojo_gemini_grounding_v1(
            1,
            operation as i64,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            response_id.as_ptr() as u64,
            signed(response_id.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}

fn validated_optional_raw_span(
    start: i64,
    end: i64,
    raw: &str,
) -> Result<Option<(usize, usize)>, MojoError> {
    if start == -1 && end == -1 {
        return Ok(None);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if end <= start || end > raw.len() {
        return Err(MojoError::InvalidOutput);
    }
    let token = raw.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if !token.starts_with('"') || !token.ends_with('"') {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyRequestShapePlan {
    pub requires_previous_response_affinity: bool,
    pub fresh_fallback_shape: Option<i64>,
    pub reconstructable_full_history: bool,
}

pub fn runtime_proxy_request_shape(
    raw: &str,
    session_present: bool,
) -> Result<RuntimeProxyRequestShapePlan, MojoError> {
    let mut output = [-1_i64; 3];
    status(unsafe {
        prodex_runtime_proxy_request_shape_v1(
            1,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            i64::from(session_present),
            output.as_mut_ptr() as u64,
        )
    })?;
    let bool_output = |value: i64| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let fresh_fallback_shape = match output[1] {
        -1 => None,
        0..=3 => Some(output[1]),
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(RuntimeProxyRequestShapePlan {
        requires_previous_response_affinity: bool_output(output[0])?,
        fresh_fallback_shape,
        reconstructable_full_history: bool_output(output[2])?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportMetadataPlan {
    pub type_class: i64,
    pub resume_id: Option<(usize, usize)>,
    pub model: Option<(usize, usize)>,
    pub effort: Option<(usize, usize)>,
    pub thread_name: Option<(usize, usize)>,
    pub cwd: Option<(usize, usize)>,
    pub updated_at: Option<(usize, usize)>,
    pub parent_thread_id: Option<(usize, usize)>,
    pub model_provider: Option<(usize, usize)>,
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

pub(crate) type JsonKernel =
    unsafe extern "C" fn(i64, i64, i64, u64, i64, u64, i64, u64, i64, i64, u64, i64, u64) -> i64;

pub(crate) fn transform_json(
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
