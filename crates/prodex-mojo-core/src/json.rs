//! Typed, borrowed JSON-tree ABI for complete semantic transforms. JSON wire
//! parsing and compatibility serialization remain outside the Mojo bridge.
use crate::MojoError;

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
    if nodes.is_empty() || nodes.len() > i64::MAX as usize / 80 {
        return Err(MojoError::InvalidInput);
    }
    let input = nodes
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
        .collect::<Result<Vec<_>, _>>()?;
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
