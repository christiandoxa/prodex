use super::{MojoError, ensure_rich_abi};
use crate::json::{JsonKernel, JsonNode, transform_json};

/// Deterministic Gemini response-part projections emitted by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiResponseMediaOperation {
    Content = 0,
    SpecialText = 1,
    ImageGeneration = 2,
}

const GEMINI_RESPONSE_MEDIA_MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

unsafe extern "C" {
    fn prodex_mojo_gemini_response_media_v1(
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

/// Runs one bounded Gemini response-part projection in compiled Mojo.
pub fn gemini_response_media(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiResponseMediaOperation,
) -> Result<Option<Vec<u8>>, MojoError> {
    ensure_rich_abi()?;
    if raw.len() > GEMINI_RESPONSE_MEDIA_MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let kernel: JsonKernel = prodex_mojo_gemini_response_media_v1;
    transform_json(nodes, raw, operation as i64, false, kernel)
}
