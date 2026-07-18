use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAtomicReservationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAppendOnlyAuditSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAuditRetentionPurgeSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub event_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteAuditExportQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteRoleBindingMutationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub role_binding_id: RoleBindingId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteServiceIdentityCreateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteUserLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBudgetPolicyUpdateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteTenantLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteVirtualKeySecretReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub virtual_key_id: VirtualKeyId,
    pub display_name: String,
    pub kind: VirtualKeySecretReferenceKind,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteProviderCredentialReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteUsageReconciliationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub ledger_event_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBillingLedgerQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: BillingLedgerSortOrder,
    pub page_limit: u16,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteExpiredReservationRecoverySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyPendingRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyCompletedRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub response_byte_count: usize,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteIdempotencyRecordLookupSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<SqliteStatement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqliteStatement {
    pub name: &'static str,
    pub sql: &'static str,
}

pub const SQLITE_BEGIN_IMMEDIATE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_reservation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_audit_append",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_AUDIT_RETENTION_PURGE_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "begin_immediate_audit_retention_purge",
        sql: "BEGIN IMMEDIATE",
    };

pub const SQLITE_BEGIN_IMMEDIATE_RECONCILIATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_usage_reconciliation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_RECOVERY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_expired_recovery",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_idempotency_pending",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_COMPLETION_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "begin_immediate_idempotency_completion",
        sql: "BEGIN IMMEDIATE",
    };

pub const SQLITE_BEGIN_IMMEDIATE_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_role_binding_mutation",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_service_identity_create",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_user_lifecycle",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_budget_policy_update",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_tenant_lifecycle",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_VIRTUAL_KEY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_virtual_key_secret_reference",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_BEGIN_IMMEDIATE_PROVIDER_CREDENTIAL_STATEMENT: SqliteStatement = SqliteStatement {
    name: "begin_immediate_provider_credential_reference",
    sql: "BEGIN IMMEDIATE",
};

pub const SQLITE_APPEND_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "append_audit_hash_chain_locally",
    sql: r#"
INSERT OR IGNORE INTO prodex_audit_log (
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);
"#,
};

pub const SQLITE_PURGE_AUDIT_RETENTION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "purge_audit_retention_batch_locally",
    sql: r#"
DELETE FROM prodex_audit_log
WHERE tenant_id = ?1
  AND audit_event_id IN (?2);
"#,
};

pub const SQLITE_QUERY_AUDIT_EXPORT_ASC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_audit_export_asc_locally",
    sql: r#"
SELECT
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
FROM prodex_audit_log
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms ASC, audit_event_id ASC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_audit_export_desc_locally",
    sql: r#"
SELECT
    tenant_id,
    audit_event_id,
    previous_digest,
    event_digest,
    occurred_at_unix_ms,
    principal_id,
    action,
    resource_kind,
    resource_id,
    outcome,
    reason_code
FROM prodex_audit_log
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_BILLING_LEDGER_ASC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_billing_ledger_asc_locally",
    sql: r#"
SELECT
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
FROM prodex_usage_ledger
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms ASC, ledger_event_id ASC
LIMIT ?4;
"#,
};

pub const SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT: SqliteStatement = SqliteStatement {
    name: "query_billing_ledger_desc_locally",
    sql: r#"
SELECT
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
FROM prodex_usage_ledger
WHERE tenant_id = ?1
  AND (?2 IS NULL OR occurred_at_unix_ms >= ?2)
  AND (?3 IS NULL OR occurred_at_unix_ms <= ?3)
ORDER BY occurred_at_unix_ms DESC, ledger_event_id DESC
LIMIT ?4;
"#,
};

pub const SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "insert_idempotency_pending_locally",
    sql: r#"
INSERT OR IGNORE INTO prodex_idempotency_records (
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms
) VALUES (?1, ?2, ?3, 'pending', ?4);
"#,
};

pub const SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT: SqliteStatement = SqliteStatement {
    name: "complete_idempotency_record_locally",
    sql: r#"
UPDATE prodex_idempotency_records
SET entry_status = 'completed',
    completed_at_unix_ms = ?4,
    response_body = ?5
WHERE tenant_id = ?1
  AND idempotency_key = ?2
  AND request_fingerprint = ?3
  AND entry_status = 'pending';
"#,
};

pub const SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT: SqliteStatement = SqliteStatement {
    name: "lookup_idempotency_record_locally",
    sql: r#"
SELECT
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms,
    completed_at_unix_ms,
    response_body
FROM prodex_idempotency_records
WHERE tenant_id = ?1
  AND idempotency_key = ?2;
"#,
};

pub const SQLITE_GRANT_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "grant_role_binding_locally",
    sql: r#"
INSERT INTO prodex_role_bindings (
    tenant_id,
    role_binding_id,
    principal_id,
    role_name,
    granted_at_unix_ms,
    revoked_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, NULL)
ON CONFLICT(tenant_id, principal_id, role_name) DO UPDATE SET
    role_binding_id = excluded.role_binding_id,
    granted_at_unix_ms = excluded.granted_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_role_bindings.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_REVOKE_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "revoke_role_binding_locally",
    sql: r#"
UPDATE prodex_role_bindings
SET revoked_at_unix_ms = ?5
WHERE tenant_id = ?1
  AND role_binding_id = ?2
  AND principal_id = ?3
  AND role_name = ?4
  AND revoked_at_unix_ms IS NULL;
"#,
};

pub const SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_service_identity_locally",
    sql: r#"
INSERT INTO prodex_service_identities (
    tenant_id,
    principal_id,
    display_name,
    created_at_unix_ms,
    disabled_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, NULL)
ON CONFLICT(tenant_id, principal_id) DO UPDATE SET
    display_name = excluded.display_name,
    created_at_unix_ms = excluded.created_at_unix_ms,
    disabled_at_unix_ms = NULL
WHERE prodex_service_identities.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_user_lifecycle_locally",
    sql: r#"
INSERT INTO prodex_users (
    tenant_id,
    principal_id,
    external_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms,
    deleted_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?5, NULL)
ON CONFLICT(tenant_id, principal_id) DO UPDATE SET
    external_id = excluded.external_id,
    display_name = excluded.display_name,
    updated_at_unix_ms = excluded.updated_at_unix_ms,
    deleted_at_unix_ms = NULL
WHERE prodex_users.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_DELETE_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "delete_user_lifecycle_locally",
    sql: r#"
UPDATE prodex_users
SET deleted_at_unix_ms = ?5,
    updated_at_unix_ms = ?5
WHERE tenant_id = ?1
  AND principal_id = ?2
  AND external_id = ?3
  AND deleted_at_unix_ms IS NULL;
"#,
};

pub const SQLITE_UPSERT_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_budget_policy_locally",
    sql: r#"
INSERT INTO prodex_budget_policies (
    tenant_id,
    budget_scope,
    max_tokens,
    max_cost_micros,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(tenant_id, budget_scope) DO UPDATE SET
    max_tokens = excluded.max_tokens,
    max_cost_micros = excluded.max_cost_micros,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_budget_policies.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_tenant_lifecycle_locally",
    sql: r#"
INSERT INTO prodex_tenants (
    tenant_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?3)
ON CONFLICT(tenant_id) DO UPDATE SET
    display_name = excluded.display_name,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_tenants.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "upsert_virtual_key_secret_reference_locally",
    sql: r#"
INSERT INTO prodex_virtual_keys (
    tenant_id,
    virtual_key_id,
    principal_id,
    display_name,
    secret_provider,
    secret_name,
    secret_version,
    secret_rotated_at_unix_ms,
    created_at_unix_ms,
    revoked_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL)
ON CONFLICT(tenant_id, virtual_key_id) DO UPDATE SET
    principal_id = excluded.principal_id,
    display_name = excluded.display_name,
    secret_provider = excluded.secret_provider,
    secret_name = excluded.secret_name,
    secret_version = excluded.secret_version,
    secret_rotated_at_unix_ms = excluded.secret_rotated_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_virtual_keys.tenant_id = excluded.tenant_id;
"#,
};

pub const SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT: SqliteStatement =
    SqliteStatement {
        name: "upsert_provider_credential_reference_locally",
        sql: r#"
INSERT INTO prodex_provider_credentials (
    tenant_id,
    provider_credential_id,
    provider_name,
    secret_provider,
    secret_name,
    secret_version,
    rotated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(tenant_id, provider_name) DO UPDATE SET
    provider_credential_id = excluded.provider_credential_id,
    secret_provider = excluded.secret_provider,
    secret_name = excluded.secret_name,
    secret_version = excluded.secret_version,
    rotated_at_unix_ms = excluded.rotated_at_unix_ms
WHERE prodex_provider_credentials.tenant_id = excluded.tenant_id;
"#,
    };

pub const SQLITE_ATOMIC_RESERVATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "reserve_usage_locally",
    sql: r#"
INSERT INTO prodex_budget_counters (
    tenant_id,
    storage_scope,
    virtual_key_id,
    reserved_tokens,
    reserved_cost_micros,
    committed_tokens,
    committed_cost_micros,
    updated_at_unix_ms
) VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)
ON CONFLICT(tenant_id, storage_scope) DO UPDATE SET
    reserved_tokens = reserved_tokens + excluded.reserved_tokens,
    reserved_cost_micros = reserved_cost_micros + excluded.reserved_cost_micros,
    updated_at_unix_ms = excluded.updated_at_unix_ms
WHERE prodex_budget_counters.tenant_id = excluded.tenant_id
  AND prodex_budget_counters.reserved_tokens + prodex_budget_counters.committed_tokens + excluded.reserved_tokens <= ?7
  AND prodex_budget_counters.reserved_cost_micros + prodex_budget_counters.committed_cost_micros + excluded.reserved_cost_micros <= ?8;

INSERT OR IGNORE INTO prodex_reservations (
    tenant_id,
    reservation_id,
    call_id,
    virtual_key_id,
    storage_scope,
    idempotency_key,
    reserved_tokens,
    reserved_cost_micros,
    created_at_unix_ms,
    expires_at_unix_ms
) VALUES (?1, ?9, ?10, ?3, ?2, ?11, ?4, ?5, ?6, ?12);

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) VALUES (?1, ?13, ?9, ?10, 'reserved', ?4, ?5, ?6);
"#,
};

pub const SQLITE_RECONCILE_USAGE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "reconcile_usage_locally",
    sql: r#"
UPDATE prodex_budget_counters
SET reserved_tokens = reserved_tokens - ?4,
    reserved_cost_micros = reserved_cost_micros - ?5,
    committed_tokens = committed_tokens + ?6,
    committed_cost_micros = committed_cost_micros + ?7,
    updated_at_unix_ms = ?8
WHERE tenant_id = ?1
  AND storage_scope = ?9
  AND reserved_tokens >= ?4
  AND reserved_cost_micros >= ?5
  AND EXISTS (
      SELECT 1
      FROM prodex_reservations
      WHERE tenant_id = ?1
        AND reservation_id = ?2
        AND call_id = ?3
        AND committed_at_unix_ms IS NULL
  );

UPDATE prodex_reservations
SET committed_at_unix_ms = ?8,
    released_at_unix_ms = CASE WHEN ?10 > 0 OR ?11 > 0 THEN ?8 ELSE released_at_unix_ms END
WHERE tenant_id = ?1
  AND reservation_id = ?2
  AND call_id = ?3
  AND committed_at_unix_ms IS NULL;

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) VALUES (?1, ?12, ?2, ?3, 'committed', ?6, ?7, ?8);

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT ?1, ?13, ?2, ?3, 'released', ?10, ?11, ?8
WHERE ?10 > 0 OR ?11 > 0;
"#,
};

pub const SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "recover_expired_reservation_locally",
    sql: r#"
UPDATE prodex_budget_counters
SET reserved_tokens = reserved_tokens - ?5,
    reserved_cost_micros = reserved_cost_micros - ?6,
    updated_at_unix_ms = ?4
WHERE tenant_id = ?1
  AND storage_scope = ?7
  AND reserved_tokens >= ?5
  AND reserved_cost_micros >= ?6
  AND EXISTS (
      SELECT 1
      FROM prodex_reservations
      WHERE tenant_id = ?1
        AND reservation_id = ?2
        AND call_id = ?3
        AND committed_at_unix_ms IS NULL
        AND released_at_unix_ms IS NULL
        AND expires_at_unix_ms <= ?4
  );

UPDATE prodex_reservations
SET released_at_unix_ms = ?4
WHERE tenant_id = ?1
  AND reservation_id = ?2
  AND call_id = ?3
  AND committed_at_unix_ms IS NULL
  AND released_at_unix_ms IS NULL
  AND expires_at_unix_ms <= ?4
  AND changes() = 1;

INSERT OR IGNORE INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
) SELECT ?1, ?8, ?2, ?3, 'released', ?5, ?6, ?4
WHERE changes() = 1;
"#,
};

pub const SQLITE_COMMIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_reservation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_AUDIT_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_audit_append",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_RECONCILIATION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_usage_reconciliation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_RECOVERY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_expired_recovery",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_IDEMPOTENCY_PENDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_idempotency_pending",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_IDEMPOTENCY_COMPLETION_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_idempotency_completion",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_ROLE_BINDING_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_role_binding_mutation",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_SERVICE_IDENTITY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_service_identity_create",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_USER_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_user_lifecycle",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_BUDGET_POLICY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_budget_policy_update",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_TENANT_LIFECYCLE_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_tenant_lifecycle",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_VIRTUAL_KEY_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_virtual_key_secret_reference",
    sql: "COMMIT",
};

pub const SQLITE_COMMIT_PROVIDER_CREDENTIAL_STATEMENT: SqliteStatement = SqliteStatement {
    name: "commit_provider_credential_reference",
    sql: "COMMIT",
};
