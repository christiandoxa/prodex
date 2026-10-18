//! External quota selection and detail projection from canonical Serde wire spans.
use super::{JsonNode, transform_json};
use crate::MojoError;

/// Chooses one provider's bounded quota presentation contract.
#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum ExternalQuotaProjection {
    Agy = 0,
    Anthropic = 1,
    DeepSeekBalance = 2,
    LocalModels = 3,
}

unsafe extern "C" {
    fn prodex_external_quota_projection_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// AGY returns `{info, amount}`; `amount` names an original numeric JSON pointer
/// and suffix for the host's fixed-decimal formatter. Anthropic returns `{main, details}`.
/// DeepSeek accepts its balance body. Local models accepts `{value, account, models_url}`.
/// These two operations return the complete external quota record.
/// Returns `None` only when AGY's account array is empty.
pub fn external_quota_projection(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: ExternalQuotaProjection,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_external_quota_projection_v1,
    )
}

#[cfg(test)]
#[path = "external_quota_tests.rs"]
mod tests;
