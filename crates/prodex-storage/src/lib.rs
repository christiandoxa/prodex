#![forbid(unsafe_code)]
//! Storage boundary contracts for tenant-scoped atomic operations.
//!
//! This crate defines adapter-neutral storage commands and validation helpers.
//! It intentionally contains no database driver, filesystem, network, async
//! runtime, or HTTP dependency. Postgres/Redis/SQLite crates can implement these
//! contracts in separate adapter crates.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    AuditDigest, AuditEnvelope, AuditEvent, AuditExportPlan, AuditRetentionPurgeBatch,
    AuditSortOrder, BudgetLimit, BudgetSnapshot, IdempotencyEntry, IdempotencyKey,
    IdempotencyRecord, IdempotentOperation, LedgerEvent, PrincipalId, ProviderCredentialId,
    ReservationReconciliation, ReservationReconciliationReason, ReservationRecord,
    ReservationRecoveryError, ReservationRequest, Role, RoleBindingId, SecretRef, TenantId,
    UsageAmount, VirtualKeyId, reconcile_reserved_usage, release_expired_reservation,
};

mod accounting;
mod audit;
mod idempotency;
mod identity;
mod policy;
mod secret_reference;
mod tenant_storage_key;
mod topology;
mod verification;

pub use accounting::*;
pub use audit::*;
pub use idempotency::*;
pub use identity::*;
pub use policy::*;
pub use secret_reference::*;
pub use tenant_storage_key::{BudgetStorageScope, TenantStorageKey};
pub use topology::*;
pub use verification::*;
