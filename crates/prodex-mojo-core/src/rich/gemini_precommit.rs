//! Borrowed Gemini pre-commit plans; Mojo alone classifies candidate output.
use super::{MojoError, ensure_rich_abi};
use crate::json::{JsonKind, JsonNode, ffi_nodes, signed, status};

/// Output already observed in the uncommitted upstream stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeminiPrecommitState {
    pub visible_output: bool,
    pub reasoning_output: bool,
}

/// Whether the caller should keep buffering, commit, or retry before commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeminiPrecommitDecision<'a> {
    Continue,
    Commit,
    RetryableInvalid(&'a str),
}

/// Validated decision and resulting stream observations from one response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeminiPrecommitPlan<'a> {
    pub decision: GeminiPrecommitDecision<'a>,
    pub state: GeminiPrecommitState,
}

/// Joined SSE data and whether Serde acquired any continuing response values.
pub struct GeminiPrecommitFrame<'a> {
    pub data: &'a str,
    pub parsed_any: bool,
}

unsafe extern "C" {
    fn prodex_mojo_gemini_precommit_frame_v1(
        abi: i64,
        address: u64,
        length: i64,
        parsed_any: i64,
        visible: i64,
        reasoning: i64,
    ) -> i64;
    fn prodex_mojo_gemini_precommit_v1(
        abi: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        corpus: u64,
        corpus_length: i64,
        visible: i64,
        reasoning: i64,
        scratch: u64,
        scratch_length: i64,
        output: u64,
    ) -> i64;
}

/// Classifies terminal markers and malformed frames after native JSON acquisition.
pub fn gemini_precommit_frame_decision(
    frame: GeminiPrecommitFrame<'_>,
    state: GeminiPrecommitState,
) -> Result<GeminiPrecommitDecision<'static>, MojoError> {
    ensure_rich_abi()?;
    // SAFETY: the UTF-8 data is borrowed for this synchronous scalar-return ABI.
    let tag = unsafe {
        prodex_mojo_gemini_precommit_frame_v1(
            1,
            frame.data.as_ptr() as u64,
            signed(frame.data.len())?,
            i64::from(frame.parsed_any),
            i64::from(state.visible_output),
            i64::from(state.reasoning_output),
        )
    };
    match tag {
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        0..=2 => decode_decision(&[], tag, -1),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classifies a normalized response using caller-owned JSON and sanitizer scratch.
pub fn gemini_precommit_plan<'a>(
    nodes: &[JsonNode<'a>],
    raw: &str,
    corpus: &str,
    state: GeminiPrecommitState,
) -> Result<GeminiPrecommitPlan<'a>, MojoError> {
    ensure_rich_abi()?;
    let input = ffi_nodes(nodes, raw)?;
    let capacity = nodes.iter().map(|node| node.text.len()).max().unwrap_or(0);
    let mut scratch = Vec::<u8>::new();
    scratch
        .try_reserve_exact(capacity)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(capacity, 0);
    let mut output = [-1_i64; 4];
    // SAFETY: borrowed nodes/raw/corpus and distinct scratch/output storage stay
    // alive for this synchronous call; Mojo validates the tree before traversal.
    status(unsafe {
        prodex_mojo_gemini_precommit_v1(
            1,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            corpus.as_ptr() as u64,
            signed(corpus.len())?,
            i64::from(state.visible_output),
            i64::from(state.reasoning_output),
            scratch.as_mut_ptr() as u64,
            signed(scratch.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;
    let boolean = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    Ok(GeminiPrecommitPlan {
        decision: decode_decision(nodes, output[0], output[3])?,
        state: GeminiPrecommitState {
            visible_output: boolean(output[1])?,
            reasoning_output: boolean(output[2])?,
        },
    })
}

fn decode_decision<'a>(
    nodes: &[JsonNode<'a>],
    tag: i64,
    reason: i64,
) -> Result<GeminiPrecommitDecision<'a>, MojoError> {
    Ok(match (tag, reason) {
        (0, -1) => GeminiPrecommitDecision::Continue,
        (1, -1) => GeminiPrecommitDecision::Commit,
        (2, -1) => GeminiPrecommitDecision::RetryableInvalid("gemini_empty_response"),
        (2, index) => {
            let node = usize::try_from(index)
                .ok()
                .and_then(|index| nodes.get(index))
                .filter(|node| matches!(node.kind, JsonKind::String))
                .ok_or(MojoError::InvalidOutput)?;
            GeminiPrecommitDecision::RetryableInvalid(node.text)
        }
        _ => return Err(MojoError::InvalidOutput),
    })
}

#[cfg(test)]
#[path = "gemini_precommit_tests.rs"]
mod tests;
