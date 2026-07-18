use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTenantContextSqlPlan {
    pub tenant_id: TenantId,
    pub statement: PostgresStatement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAtomicReservationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAppendOnlyAuditSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAuditRetentionPurgeSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub event_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresAuditExportQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresRoleBindingMutationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub role_binding_id: RoleBindingId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresServiceIdentityCreateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresUserLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBudgetPolicyUpdateSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTenantLifecycleSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresVirtualKeySecretReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub virtual_key_id: VirtualKeyId,
    pub display_name: String,
    pub kind: VirtualKeySecretReferenceKind,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresProviderCredentialReferenceSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresUsageReconciliationSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub ledger_event_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresBillingLedgerQuerySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub sort_order: BillingLedgerSortOrder,
    pub page_limit: u16,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresExpiredReservationRecoverySqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyPendingRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyCompletedRecordSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub response_byte_count: usize,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresIdempotencyRecordLookupSqlPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<PostgresStatement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresStatement {
    pub name: &'static str,
    pub sql: &'static str,
}

pub const SET_TENANT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "set_tenant_context",
    sql: "SELECT set_config('prodex.tenant_id', $1, true)",
};

pub fn plan_postgres_tenant_context(tenant_id: TenantId) -> PostgresTenantContextSqlPlan {
    PostgresTenantContextSqlPlan {
        tenant_id,
        statement: SET_TENANT_STATEMENT,
    }
}

pub const APPEND_AUDIT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "append_audit_hash_chain",
    sql: r#"
INSERT INTO prodex_audit_log (
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
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, audit_event_id) DO NOTHING
RETURNING tenant_id
"#,
};

pub const PURGE_AUDIT_RETENTION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "purge_audit_retention_batch",
    sql: r#"
DELETE FROM prodex_audit_log
WHERE tenant_id = $1
  AND audit_event_id = ANY($2)
RETURNING tenant_id, audit_event_id
"#,
};

pub const QUERY_AUDIT_EXPORT_ASC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_audit_export_asc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms ASC, audit_event_id ASC
LIMIT $4
"#,
};

pub const QUERY_AUDIT_EXPORT_DESC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_audit_export_desc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC
LIMIT $4
"#,
};

pub const QUERY_BILLING_LEDGER_ASC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_billing_ledger_asc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms ASC, ledger_event_id ASC
LIMIT $4
"#,
};

pub const QUERY_BILLING_LEDGER_DESC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "query_billing_ledger_desc",
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
WHERE tenant_id = $1
  AND ($2::BIGINT IS NULL OR occurred_at_unix_ms >= $2)
  AND ($3::BIGINT IS NULL OR occurred_at_unix_ms <= $3)
ORDER BY occurred_at_unix_ms DESC, ledger_event_id DESC
LIMIT $4
"#,
};

pub const INSERT_IDEMPOTENCY_PENDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "insert_idempotency_pending",
    sql: r#"
INSERT INTO prodex_idempotency_records (
    tenant_id,
    idempotency_key,
    request_fingerprint,
    entry_status,
    started_at_unix_ms
) VALUES ($1, $2, $3, 'pending', $4)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
RETURNING tenant_id
"#,
};

pub const COMPLETE_IDEMPOTENCY_RECORD_STATEMENT: PostgresStatement = PostgresStatement {
    name: "complete_idempotency_record",
    sql: r#"
UPDATE prodex_idempotency_records
SET entry_status = 'completed',
    completed_at_unix_ms = $4,
    response_body = $5
WHERE tenant_id = $1
  AND idempotency_key = $2
  AND request_fingerprint = $3
  AND entry_status = 'pending'
RETURNING tenant_id
"#,
};

pub const LOOKUP_IDEMPOTENCY_RECORD_STATEMENT: PostgresStatement = PostgresStatement {
    name: "lookup_idempotency_record",
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
WHERE tenant_id = $1
  AND idempotency_key = $2
"#,
};

pub const GRANT_ROLE_BINDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "grant_role_binding",
    sql: r#"
INSERT INTO prodex_role_bindings (
    tenant_id,
    role_binding_id,
    principal_id,
    role_name,
    granted_at_unix_ms,
    revoked_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, NULL)
ON CONFLICT (tenant_id, principal_id, role_name) DO UPDATE SET
    role_binding_id = EXCLUDED.role_binding_id,
    granted_at_unix_ms = EXCLUDED.granted_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_role_bindings.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const REVOKE_ROLE_BINDING_STATEMENT: PostgresStatement = PostgresStatement {
    name: "revoke_role_binding",
    sql: r#"
UPDATE prodex_role_bindings
SET revoked_at_unix_ms = $5
WHERE tenant_id = $1
  AND role_binding_id = $2
  AND principal_id = $3
  AND role_name = $4
  AND revoked_at_unix_ms IS NULL
RETURNING tenant_id
"#,
};

pub const UPSERT_SERVICE_IDENTITY_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_service_identity",
    sql: r#"
INSERT INTO prodex_service_identities (
    tenant_id,
    principal_id,
    display_name,
    created_at_unix_ms,
    disabled_at_unix_ms
) VALUES ($1, $2, $3, $4, NULL)
ON CONFLICT (tenant_id, principal_id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    created_at_unix_ms = EXCLUDED.created_at_unix_ms,
    disabled_at_unix_ms = NULL
WHERE prodex_service_identities.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_USER_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_user_lifecycle",
    sql: r#"
INSERT INTO prodex_users (
    tenant_id,
    principal_id,
    external_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms,
    deleted_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, $5, NULL)
ON CONFLICT (tenant_id, principal_id) DO UPDATE SET
    external_id = EXCLUDED.external_id,
    display_name = EXCLUDED.display_name,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms,
    deleted_at_unix_ms = NULL
WHERE prodex_users.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const DELETE_USER_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "delete_user_lifecycle",
    sql: r#"
UPDATE prodex_users
SET deleted_at_unix_ms = $5,
    updated_at_unix_ms = $5
WHERE tenant_id = $1
  AND principal_id = $2
  AND external_id = $3
  AND deleted_at_unix_ms IS NULL
RETURNING tenant_id
"#,
};

pub const UPSERT_BUDGET_POLICY_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_budget_policy",
    sql: r#"
INSERT INTO prodex_budget_policies (
    tenant_id,
    budget_scope,
    max_tokens,
    max_cost_micros,
    updated_at_unix_ms
) VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (tenant_id, budget_scope) DO UPDATE SET
    max_tokens = EXCLUDED.max_tokens,
    max_cost_micros = EXCLUDED.max_cost_micros,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
WHERE prodex_budget_policies.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_TENANT_LIFECYCLE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_tenant_lifecycle",
    sql: r#"
INSERT INTO prodex_tenants (
    tenant_id,
    display_name,
    created_at_unix_ms,
    updated_at_unix_ms
) VALUES ($1, $2, $3, $3)
ON CONFLICT (tenant_id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
WHERE prodex_tenants.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_virtual_key_secret_reference",
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
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)
ON CONFLICT (tenant_id, virtual_key_id) DO UPDATE SET
    principal_id = EXCLUDED.principal_id,
    display_name = EXCLUDED.display_name,
    secret_provider = EXCLUDED.secret_provider,
    secret_name = EXCLUDED.secret_name,
    secret_version = EXCLUDED.secret_version,
    secret_rotated_at_unix_ms = EXCLUDED.secret_rotated_at_unix_ms,
    revoked_at_unix_ms = NULL
WHERE prodex_virtual_keys.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "upsert_provider_credential_reference",
    sql: r#"
INSERT INTO prodex_provider_credentials (
    tenant_id,
    provider_credential_id,
    provider_name,
    secret_provider,
    secret_name,
    secret_version,
    rotated_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (tenant_id, provider_name) DO UPDATE SET
    provider_credential_id = EXCLUDED.provider_credential_id,
    secret_provider = EXCLUDED.secret_provider,
    secret_name = EXCLUDED.secret_name,
    secret_version = EXCLUDED.secret_version,
    rotated_at_unix_ms = EXCLUDED.rotated_at_unix_ms
WHERE prodex_provider_credentials.tenant_id = EXCLUDED.tenant_id
RETURNING tenant_id
"#,
};

pub const ATOMIC_RESERVATION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "reserve_usage_atomically",
    sql: r#"
WITH existing AS (
    SELECT tenant_id, reservation_id
    FROM prodex_reservations
    WHERE tenant_id = $1 AND idempotency_key = $2
), upsert_counter AS (
    INSERT INTO prodex_budget_counters (
        tenant_id,
        storage_scope,
        virtual_key_id,
        reserved_tokens,
        reserved_cost_micros,
        committed_tokens,
        committed_cost_micros,
        request_count,
        updated_at_unix_ms
    ) VALUES ($1, $3, $4, $5, $6, 0, 0, 1, $7)
    ON CONFLICT (tenant_id, storage_scope) DO UPDATE SET
        reserved_tokens = prodex_budget_counters.reserved_tokens + EXCLUDED.reserved_tokens,
        reserved_cost_micros = prodex_budget_counters.reserved_cost_micros + EXCLUDED.reserved_cost_micros,
        request_count = prodex_budget_counters.request_count + 1,
        updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
    WHERE NOT EXISTS (SELECT 1 FROM existing)
      AND prodex_budget_counters.tenant_id = EXCLUDED.tenant_id
      AND prodex_budget_counters.reserved_tokens + prodex_budget_counters.committed_tokens + EXCLUDED.reserved_tokens <= $8
      AND prodex_budget_counters.reserved_cost_micros + prodex_budget_counters.committed_cost_micros + EXCLUDED.reserved_cost_micros <= $9
      AND prodex_budget_counters.request_count + 1 <= $10
    RETURNING tenant_id
), reservation_insert AS (
    INSERT INTO prodex_reservations (
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
    )
    SELECT $1, $11, $12, $4, $3, $2, $5, $6, $7, $13
    WHERE EXISTS (SELECT 1 FROM upsert_counter)
    ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
    RETURNING tenant_id
)
INSERT INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT $1, $14, $11, $12, 'reserved', $5, $6, $7
WHERE EXISTS (SELECT 1 FROM reservation_insert)
ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
RETURNING tenant_id
"#,
};

pub const RECONCILE_USAGE_STATEMENT: PostgresStatement = PostgresStatement {
    name: "reconcile_usage_atomically",
    sql: r#"
WITH locked_reservation AS (
    SELECT tenant_id, reservation_id, call_id, virtual_key_id
    FROM prodex_reservations
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND call_id = $3
      AND committed_at_unix_ms IS NULL
    FOR UPDATE
), update_counter AS (
    UPDATE prodex_budget_counters
    SET reserved_tokens = reserved_tokens - $4,
        reserved_cost_micros = reserved_cost_micros - $5,
        committed_tokens = committed_tokens + $6,
        committed_cost_micros = committed_cost_micros + $7,
        updated_at_unix_ms = $8
    WHERE tenant_id = $1
      AND storage_scope = $9
      AND reserved_tokens >= $4
      AND reserved_cost_micros >= $5
      AND EXISTS (SELECT 1 FROM locked_reservation)
    RETURNING tenant_id
), mark_reservation AS (
    UPDATE prodex_reservations
    SET committed_at_unix_ms = $8,
        released_at_unix_ms = CASE WHEN $10::BIGINT > 0 OR $11::BIGINT > 0 THEN $8 ELSE released_at_unix_ms END
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND EXISTS (SELECT 1 FROM update_counter)
    RETURNING tenant_id
), committed_ledger AS (
    INSERT INTO prodex_usage_ledger (
        tenant_id,
        ledger_event_id,
        reservation_id,
        call_id,
        event_kind,
        tokens,
        cost_micros,
        occurred_at_unix_ms
    )
    SELECT $1, $12, $2, $3, 'committed', $6, $7, $8
    WHERE EXISTS (SELECT 1 FROM mark_reservation)
    ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
    RETURNING tenant_id
), released_ledger AS (
    INSERT INTO prodex_usage_ledger (
        tenant_id,
        ledger_event_id,
        reservation_id,
        call_id,
        event_kind,
        tokens,
        cost_micros,
        occurred_at_unix_ms
    )
    SELECT $1, $13, $2, $3, 'released', $10, $11, $8
    WHERE EXISTS (SELECT 1 FROM mark_reservation)
      AND ($10::BIGINT > 0 OR $11::BIGINT > 0)
    ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
    RETURNING tenant_id
)
SELECT tenant_id FROM mark_reservation
"#,
};

pub const RECOVER_EXPIRED_RESERVATION_STATEMENT: PostgresStatement = PostgresStatement {
    name: "recover_expired_reservation",
    sql: r#"
WITH locked_reservation AS (
    SELECT tenant_id, reservation_id, call_id, reserved_tokens, reserved_cost_micros
    FROM prodex_reservations
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND call_id = $3
      AND committed_at_unix_ms IS NULL
      AND released_at_unix_ms IS NULL
      AND expires_at_unix_ms <= $4
    FOR UPDATE
), update_counter AS (
    UPDATE prodex_budget_counters
    SET reserved_tokens = reserved_tokens - $5,
        reserved_cost_micros = reserved_cost_micros - $6,
        updated_at_unix_ms = $4
    WHERE tenant_id = $1
      AND storage_scope = $7
      AND reserved_tokens >= $5
      AND reserved_cost_micros >= $6
      AND EXISTS (SELECT 1 FROM locked_reservation)
    RETURNING tenant_id
), mark_reservation AS (
    UPDATE prodex_reservations
    SET released_at_unix_ms = $4
    WHERE tenant_id = $1
      AND reservation_id = $2
      AND EXISTS (SELECT 1 FROM update_counter)
    RETURNING tenant_id
)
INSERT INTO prodex_usage_ledger (
    tenant_id,
    ledger_event_id,
    reservation_id,
    call_id,
    event_kind,
    tokens,
    cost_micros,
    occurred_at_unix_ms
)
SELECT $1, $8, $2, $3, 'released', $5, $6, $4
WHERE EXISTS (SELECT 1 FROM mark_reservation)
ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING
RETURNING tenant_id
"#,
};
