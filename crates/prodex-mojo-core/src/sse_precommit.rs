//! Typed boundary for the Mojo-owned SSE precommit wait policy.

use crate::MojoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsePrecommitBoundary {
    Wait,
    DeadlineExceeded,
    ByteLimitExceeded,
    IncompleteEof,
}

unsafe extern "C" {
    fn prodex_runtime_sse_precommit_boundary_v1(
        abi_version: i64,
        buffered_bytes: u64,
        byte_limit: u64,
        elapsed_ms: u64,
        deadline_ms: u64,
        upstream_eof: i64,
    ) -> i64;
}

/// Called only before a commit-ready event; it can wait or fail, never commit.
pub fn held_boundary(
    buffered_bytes: usize,
    byte_limit: usize,
    elapsed_ms: u64,
    deadline_ms: u64,
    upstream_eof: bool,
) -> Result<SsePrecommitBoundary, MojoError> {
    let result = unsafe {
        prodex_runtime_sse_precommit_boundary_v1(
            1,
            u64::try_from(buffered_bytes).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(byte_limit).map_err(|_| MojoError::InvalidInput)?,
            elapsed_ms,
            deadline_ms,
            i64::from(upstream_eof),
        )
    };
    match result {
        0 => Ok(SsePrecommitBoundary::Wait),
        1 => Ok(SsePrecommitBoundary::DeadlineExceeded),
        2 => Ok(SsePrecommitBoundary::ByteLimitExceeded),
        3 => Ok(SsePrecommitBoundary::IncompleteEof),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_sse_boundaries_never_authorize_commit() {
        assert_eq!(
            held_boundary(0, 100, 1_001, 300_000, false),
            Ok(SsePrecommitBoundary::Wait)
        );
        assert_eq!(
            held_boundary(99, 100, 299_999, 300_000, false),
            Ok(SsePrecommitBoundary::Wait)
        );
        assert_eq!(
            held_boundary(0, 100, 300_000, 300_000, false),
            Ok(SsePrecommitBoundary::DeadlineExceeded)
        );
        assert_eq!(
            held_boundary(100, 100, 0, 300_000, false),
            Ok(SsePrecommitBoundary::ByteLimitExceeded)
        );
        assert_eq!(
            held_boundary(1, 100, 0, 300_000, true),
            Ok(SsePrecommitBoundary::IncompleteEof)
        );
        assert_eq!(
            held_boundary(0, 100, 0, 300_000, true),
            Ok(SsePrecommitBoundary::IncompleteEof)
        );
        assert_eq!(
            held_boundary(0, 0, 0, 1, false),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            held_boundary(0, 1, 0, 0, false),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            unsafe { prodex_runtime_sse_precommit_boundary_v1(2, 0, 1, 0, 1, 0) },
            -4
        );
        assert_eq!(
            unsafe { prodex_runtime_sse_precommit_boundary_v1(1, 0, 1, 0, 1, 2) },
            -1
        );
    }
}
