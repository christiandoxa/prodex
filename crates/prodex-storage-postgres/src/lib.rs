#![forbid(unsafe_code)]
//! PostgreSQL storage plans for tenant-scoped accounting.
//!
//! This crate is intentionally driver-free. It owns versioned SQL text and
//! request-path SQL plans, while external migrators and adapter crates decide
//! how to execute them. Gateway open/request paths must use prepared DML plans
//! only; DDL lives in explicit migration plans.

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

mod governance_sql;
mod migration_catalog;
pub use governance_sql::*;
pub use migration_catalog::*;

mod migration;
mod plans;
mod sql;

pub use migration::*;
pub use plans::*;
pub use sql::*;
