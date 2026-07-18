#![forbid(unsafe_code)]
//! SQLite storage plans for local compatibility and tests.
//!
//! SQLite remains useful for local/single-node compatibility, but migration DDL
//! must be planned explicitly and kept out of request-serving open paths. This
//! crate is driver-free and owns SQL text only; adapter crates decide when and
//! how to execute it.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    AuditSortOrder, BudgetLimit, IdempotencyKey, PrincipalId, ProviderCredentialId,
    ReservationRequest, Role, RoleBindingId, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AtomicReservationCommand, AuditExportQueryCommand,
    AuditRetentionPurgeCommand, BillingLedgerQueryCommand, BillingLedgerSortOrder,
    BudgetPolicyUpdateCommand, ExpiredReservationRecoveryCommand,
    IdempotencyCompletedRecordCommand, IdempotencyPendingRecordCommand,
    IdempotencyRecordLookupCommand, ProviderCredentialReferenceCommand, RoleBindingMutationCommand,
    RoleBindingMutationKind, ServiceIdentityCreateCommand, TenantLifecycleCommand,
    TenantLifecycleKind, TenantStorageKey, UsageReconciliationCommand, UserLifecycleCommand,
    UserLifecycleKind, VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
};

mod enterprise_governance_migration;
pub use enterprise_governance_migration::{
    LOCAL_APPROVAL_TERMINATION_REASON_MIGRATION, LOCAL_AUDIT_LEGAL_HOLD_MIGRATION,
    LOCAL_ENTERPRISE_GOVERNANCE_HARDENING_MIGRATION, LOCAL_ENTERPRISE_GOVERNANCE_MIGRATION,
    LOCAL_GOVERNANCE_LIFECYCLE_MIGRATION, LOCAL_GOVERNANCE_SESSION_INDEX_MIGRATION,
    LOCAL_GOVERNANCE_SESSION_PROVIDER_REVISIONS_MIGRATION,
    LOCAL_RESERVATION_STORAGE_SCOPE_MIGRATION, LOCAL_SESSION_REVOCATION_EPOCH_MIGRATION,
};
pub use prodex_storage::{
    ApprovalVoteRequest, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceAuditIntegrityHealth, GovernanceOutboxHealth,
    GovernanceRepositoryError, GovernanceRevisionSummary, GovernanceSnapshot,
    GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome,
};

mod migration;
mod plans;
mod sql;

pub use migration::*;
pub use plans::*;
pub use sql::*;
