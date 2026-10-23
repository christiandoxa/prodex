//! Borrowed-tree adapter for Presidio image and managed-container validation.
use super::{JsonNode, transform_json};
use crate::MojoError;

/// Selects the trust-boundary contract evaluated over the supplied observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum PresidioDockerValidation {
    Image,
    Container,
}

unsafe extern "C" {
    fn prodex_presidio_docker_validation_v1(
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

/// Return the compiled policy's diagnostic, or `None` for accepted observations.
/// Image inputs contain image/service/minimum/latest strings; container inputs
/// contain container/name/host_port/minimum observations. No wire spans are read.
pub fn presidio_docker_validation_error(
    nodes: &[JsonNode<'_>],
    validation: PresidioDockerValidation,
) -> Result<Option<String>, MojoError> {
    transform_json(
        nodes,
        "",
        validation as i64,
        false,
        prodex_presidio_docker_validation_v1,
    )?
    .map(|bytes| String::from_utf8(bytes).map_err(|_| MojoError::InvalidOutput))
    .transpose()
}
