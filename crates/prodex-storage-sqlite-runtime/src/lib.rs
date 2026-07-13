#![forbid(unsafe_code)]
//! Executing SQLite adapters. SQL plans remain in `prodex-storage-sqlite`.

mod governance_repository;

pub use governance_repository::*;
pub use prodex_storage::{
    ApprovalVoteRequest, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceAuditIntegrityHealth, GovernanceOutboxHealth,
    GovernanceRepositoryError, GovernanceRevisionSummary, GovernanceSnapshot,
    GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome,
};
