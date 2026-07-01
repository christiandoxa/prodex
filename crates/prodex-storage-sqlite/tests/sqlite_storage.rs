use prodex_domain::{
    AuditAction, AuditDigest, AuditExportFormat, AuditExportPlan, AuditOutcome, AuditPageLimit,
    AuditQueryPlan, AuditQueryScope, AuditResource, AuditRetentionBatchLimit,
    AuditRetentionPurgeBatch, AuditRetentionPurgeKey, AuditSortOrder, AuditTimeRange,
    AuditTimestamp, BudgetLimit, BudgetSnapshot, CallId, CredentialScope, IdempotencyKey,
    Principal, PrincipalId, PrincipalKind, ProviderCredentialId, ReservationId,
    ReservationReconciliationReason, ReservationRecord, ReservationRequest, Role, RoleBindingId,
    SecretRef, TenantContext, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AtomicReservationCommand, AuditExportQueryCommand,
    AuditRetentionPurgeCommand, BillingLedgerPageLimit, BillingLedgerQueryCommand,
    BillingLedgerSortOrder, BudgetPolicyUpdateCommand, ExpiredReservationRecoveryCommand,
    IdempotencyCompletedRecordCommand, IdempotencyPendingRecordCommand,
    IdempotencyRecordLookupCommand, ProviderCredentialReferenceCommand, RoleBindingMutationCommand,
    RoleBindingMutationKind, ServiceIdentityCreateCommand, TenantLifecycleCommand,
    TenantLifecycleKind, TenantStorageKey, UsageReconciliationCommand, UserLifecycleCommand,
    UserLifecycleKind, VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
};
use prodex_storage_sqlite::{
    INITIAL_LOCAL_ACCOUNTING_MIGRATION, REQUIRED_SQLITE_SCHEMA_VERSION,
    SQLITE_APPEND_AUDIT_STATEMENT, SQLITE_ATOMIC_RESERVATION_STATEMENT,
    SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT, SQLITE_DELETE_USER_LIFECYCLE_STATEMENT,
    SQLITE_GRANT_ROLE_BINDING_STATEMENT, SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT,
    SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT, SQLITE_PURGE_AUDIT_RETENTION_STATEMENT,
    SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT, SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT,
    SQLITE_RECONCILE_USAGE_STATEMENT, SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT,
    SQLITE_REVOKE_ROLE_BINDING_STATEMENT, SQLITE_UPSERT_BUDGET_POLICY_STATEMENT,
    SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT,
    SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT, SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT,
    SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT, SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT,
    SqliteBackendOpenMode, SqliteMigrationVersion, SqliteRuntimeMode, SqliteStorageErrorStatus,
    SqliteStoragePlanError, plan_sqlite_append_only_audit, plan_sqlite_atomic_reservation,
    plan_sqlite_audit_export_query, plan_sqlite_audit_retention_purge, plan_sqlite_backend_open,
    plan_sqlite_billing_ledger_query, plan_sqlite_budget_policy_update,
    plan_sqlite_expired_reservation_recovery, plan_sqlite_idempotency_completed_record,
    plan_sqlite_idempotency_pending_record, plan_sqlite_idempotency_record_lookup,
    plan_sqlite_migrations, plan_sqlite_provider_credential_reference,
    plan_sqlite_role_binding_mutation, plan_sqlite_service_identity_create,
    plan_sqlite_storage_error_response, plan_sqlite_tenant_lifecycle,
    plan_sqlite_usage_reconciliation, plan_sqlite_user_lifecycle,
    plan_sqlite_virtual_key_secret_reference, statement_contains_ddl,
};

fn command(tenant_id: TenantId) -> AtomicReservationCommand {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    AtomicReservationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(1_000, 10_000),
        request: ReservationRequest {
            tenant_id,
            call_id,
            reservation_id,
            estimate: UsageAmount::new(25, 250),
        },
        created_at_unix_ms: 1_000,
        ttl_ms: 60_000,
    }
}

#[test]
fn sqlite_storage_error_response_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let error = SqliteStoragePlanError::TenantMismatch {
        key_tenant: tenant_id,
        request_tenant: other_tenant,
    };
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&other_tenant.to_string()));

    let schema = SqliteStoragePlanError::SchemaVersionTooOld {
        observed: SqliteMigrationVersion(1),
        required: SqliteMigrationVersion(2),
    };
    assert!(!schema.to_string().contains("1"));
    assert!(!schema.to_string().contains("2"));

    let usage = SqliteStoragePlanError::ActualUsageExceedsReserved {
        reserved: UsageAmount::new(10, 100),
        actual: UsageAmount::new(11, 90),
    };
    assert!(!usage.to_string().contains("10"));
    assert!(!usage.to_string().contains("100"));
    assert!(!usage.to_string().contains("11"));

    let response = plan_sqlite_storage_error_response(&error);

    assert_eq!(
        response.status,
        SqliteStorageErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "sqlite_storage_unavailable");
    assert_eq!(
        response.message,
        "sqlite storage planning is temporarily unavailable"
    );
    assert!(!response.message.contains("SQLite"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
}

fn audit_command(tenant_id: TenantId) -> AppendOnlyAuditCommand {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        event: prodex_domain::AuditEvent::new(
            2_000,
            TenantContext { tenant_id },
            &principal,
            AuditAction::new("control_plane.audit.export"),
            AuditResource::new("AuditLog", Some("export-1"), Some(tenant_id)),
            AuditOutcome::Success,
            None::<String>,
        ),
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn retention_purge_command(tenant_id: TenantId) -> AuditRetentionPurgeCommand {
    let batch = AuditRetentionPurgeBatch::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        [
            AuditRetentionPurgeKey {
                tenant_id,
                event_id: prodex_domain::AuditEventId::new(),
            },
            AuditRetentionPurgeKey {
                tenant_id,
                event_id: prodex_domain::AuditEventId::new(),
            },
        ],
        AuditRetentionBatchLimit::new(Some(10)).unwrap(),
    )
    .unwrap();
    AuditRetentionPurgeCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        batch,
    }
}

fn audit_export_query_command(tenant_id: TenantId) -> AuditExportQueryCommand {
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_800_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_800_000_999_999).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(250)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );
    AuditExportQueryCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        export: AuditExportPlan::new(query, AuditExportFormat::Jsonl),
    }
}

fn billing_ledger_query_command(tenant_id: TenantId) -> BillingLedgerQueryCommand {
    BillingLedgerQueryCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        start_unix_ms: Some(1_800_000_000_000),
        end_unix_ms: Some(1_800_000_999_999),
        page_limit: BillingLedgerPageLimit::new(Some(250)).unwrap(),
        sort_order: BillingLedgerSortOrder::OccurredAtDesc,
    }
}

fn idempotency_pending_command(tenant_id: TenantId) -> IdempotencyPendingRecordCommand {
    IdempotencyPendingRecordCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        operation: prodex_domain::IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-request",
        )
        .unwrap(),
        started_at_unix_ms: 1_800_000_000_000,
    }
}

fn idempotency_completed_command(tenant_id: TenantId) -> IdempotencyCompletedRecordCommand {
    IdempotencyCompletedRecordCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        operation: prodex_domain::IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-request",
        )
        .unwrap(),
        completed_at_unix_ms: 1_800_000_001_000,
        response_body: br#"{"status":"ok"}"#.to_vec(),
    }
}

fn idempotency_lookup_command(tenant_id: TenantId) -> IdempotencyRecordLookupCommand {
    IdempotencyRecordLookupCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        operation: prodex_domain::IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-request",
        )
        .unwrap(),
    }
}

fn role_binding_command(
    tenant_id: TenantId,
    kind: RoleBindingMutationKind,
) -> RoleBindingMutationCommand {
    RoleBindingMutationCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        role_binding_id: RoleBindingId::new(),
        principal_id: PrincipalId::new(),
        role: Role::Operator,
        kind,
        occurred_at_unix_ms: 1_800_000_002_000,
    }
}

fn service_identity_command(tenant_id: TenantId) -> ServiceIdentityCreateCommand {
    ServiceIdentityCreateCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        principal_id: PrincipalId::new(),
        display_name: "svc-ingest".to_string(),
        created_at_unix_ms: 1_800_000_002_250,
    }
}

fn user_lifecycle_command(tenant_id: TenantId, kind: UserLifecycleKind) -> UserLifecycleCommand {
    UserLifecycleCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        principal_id: PrincipalId::new(),
        external_id: "scim-user-1".to_string(),
        display_name: "Ada Lovelace".to_string(),
        kind,
        occurred_at_unix_ms: 1_800_000_002_400,
    }
}

fn budget_policy_command(tenant_id: TenantId) -> BudgetPolicyUpdateCommand {
    BudgetPolicyUpdateCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        budget_scope: "tenant-default".to_string(),
        limit: BudgetLimit::new(10_000, 1_000_000),
        updated_at_unix_ms: 1_800_000_002_750,
    }
}

fn tenant_lifecycle_command(
    tenant_id: TenantId,
    kind: TenantLifecycleKind,
) -> TenantLifecycleCommand {
    TenantLifecycleCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        display_name: "acme-prod".to_string(),
        kind,
        occurred_at_unix_ms: 1_800_000_002_900,
    }
}

fn virtual_key_secret_command(
    tenant_id: TenantId,
    kind: VirtualKeySecretReferenceKind,
) -> VirtualKeySecretReferenceCommand {
    let virtual_key_id = VirtualKeyId::new();
    VirtualKeySecretReferenceCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
        tenant_id,
        virtual_key_id,
        principal_id: PrincipalId::new(),
        display_name: "prod-api".to_string(),
        secret_ref: SecretRef::new("vault", "virtual-keys/prod-api", Some("v4")),
        kind,
        occurred_at_unix_ms: 1_800_000_002_500,
    }
}

fn provider_credential_command(tenant_id: TenantId) -> ProviderCredentialReferenceCommand {
    ProviderCredentialReferenceCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        provider_credential_id: ProviderCredentialId::new(),
        provider_name: "openai".to_string(),
        secret_ref: SecretRef::new("vault", "providers/openai", Some("v3")),
        rotated_at_unix_ms: 1_800_000_003_000,
    }
}

fn reconciliation_command(
    tenant_id: TenantId,
    reserved: UsageAmount,
    actual: UsageAmount,
) -> UsageReconciliationCommand {
    let request = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: reserved,
    };
    UsageReconciliationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        snapshot: BudgetSnapshot {
            reserved,
            committed: UsageAmount::new(1, 10),
        },
        record: ReservationRecord::from_request(request, 1_000, 60_000).unwrap(),
        actual,
        reason: ReservationReconciliationReason::Completed,
    }
}

fn expired_recovery_command(
    tenant_id: TenantId,
    reserved: UsageAmount,
    now_unix_ms: u64,
) -> ExpiredReservationRecoveryCommand {
    let request = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: reserved,
    };
    ExpiredReservationRecoveryCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        snapshot: BudgetSnapshot {
            reserved,
            committed: UsageAmount::new(1, 10),
        },
        record: ReservationRecord::from_request(request, 1_000, 500).unwrap(),
        now_unix_ms,
    }
}

#[test]
fn sqlite_migrations_are_explicit_and_forbidden_on_request_path() {
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    assert_eq!(plan.migrations.len(), 1);
    assert!(statement_contains_ddl(plan.migrations[0].sql));

    assert_eq!(
        plan_sqlite_migrations(SqliteRuntimeMode::GatewayRequestPath),
        Err(SqliteStoragePlanError::DdlForbiddenOnRequestPath)
    );
}

#[test]
fn sqlite_backend_open_requires_known_current_schema_without_ddl() {
    let plan = plan_sqlite_backend_open(
        SqliteBackendOpenMode::GatewayStartup,
        Some(REQUIRED_SQLITE_SCHEMA_VERSION),
    )
    .unwrap();

    assert_eq!(plan.mode, SqliteBackendOpenMode::GatewayStartup);
    assert_eq!(plan.required_schema_version, REQUIRED_SQLITE_SCHEMA_VERSION);
    assert!(!plan.ddl_allowed);
    assert_eq!(plan.migration_count, 0);

    assert_eq!(
        plan_sqlite_backend_open(SqliteBackendOpenMode::GatewayRequestPath, None),
        Err(SqliteStoragePlanError::MissingSchemaVersion)
    );
    assert_eq!(
        plan_sqlite_backend_open(
            SqliteBackendOpenMode::GatewayRequestPath,
            Some(SqliteMigrationVersion(0)),
        ),
        Err(SqliteStoragePlanError::SchemaVersionTooOld {
            observed: SqliteMigrationVersion(0),
            required: REQUIRED_SQLITE_SCHEMA_VERSION,
        })
    );
}

#[test]
fn sqlite_external_migrator_open_is_the_only_ddl_eligible_mode() {
    let plan = plan_sqlite_backend_open(SqliteBackendOpenMode::ExternalMigrator, None).unwrap();

    assert_eq!(plan.mode, SqliteBackendOpenMode::ExternalMigrator);
    assert_eq!(plan.required_schema_version, REQUIRED_SQLITE_SCHEMA_VERSION);
    assert!(plan.ddl_allowed);
    assert_eq!(plan.migration_count, 1);
}

#[test]
fn sqlite_migration_keeps_tenant_in_keys_indexes_and_ledger_uniqueness() {
    let sql = INITIAL_LOCAL_ACCOUNTING_MIGRATION.sql;

    assert!(sql.contains("PRIMARY KEY (tenant_id, virtual_key_id)"));
    assert!(sql.contains("secret_provider TEXT NOT NULL"));
    assert!(sql.contains("secret_name TEXT NOT NULL"));
    assert!(sql.contains("secret_rotated_at_unix_ms INTEGER NOT NULL"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, principal_id)"));
    assert!(sql.contains("prodex_service_identities_tenant_name_idx"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_users"));
    assert!(sql.contains("UNIQUE (tenant_id, external_id)"));
    assert!(sql.contains("deleted_at_unix_ms INTEGER"));
    assert!(sql.contains("prodex_users_tenant_external_idx"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, role_binding_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, provider_credential_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, storage_scope)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, budget_scope)"));
    assert!(sql.contains("prodex_budget_policies_tenant_scope_idx"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, reservation_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, ledger_event_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, audit_event_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, idempotency_key)"));
    assert!(sql.contains("UNIQUE (tenant_id, call_id)"));
    assert!(sql.contains("UNIQUE (tenant_id, idempotency_key)"));
    assert!(sql.contains("UNIQUE (tenant_id, principal_id, role_name)"));
    assert!(sql.contains("display_name TEXT NOT NULL"));
    assert!(sql.contains("updated_at_unix_ms INTEGER NOT NULL"));
    assert!(sql.contains("CHECK (display_name <> '')"));
    assert!(sql.contains("UNIQUE (tenant_id, provider_name)"));
    assert!(sql.contains("UNIQUE (tenant_id, reservation_id, event_kind)"));
    assert!(sql.contains("UNIQUE (tenant_id, event_digest)"));
    assert!(sql.contains("UNIQUE (tenant_id, previous_digest)"));
    assert!(sql.contains("prodex_virtual_keys_tenant_name_idx"));
    assert!(sql.contains("prodex_role_bindings_tenant_principal_idx"));
    assert!(sql.contains("prodex_provider_credentials_tenant_provider_idx"));
    assert!(sql.contains("prodex_usage_ledger_tenant_call_idx"));
    assert!(sql.contains("prodex_audit_log_tenant_time_idx"));
    assert!(sql.contains("prodex_idempotency_records_tenant_status_idx"));
}

#[test]
fn sqlite_reservation_plan_uses_immediate_transaction_and_dml_only() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_atomic_reservation(command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "begin_immediate_reservation");
    assert_eq!(plan.statements.last().unwrap().name, "commit_reservation");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, storage_scope) DO UPDATE")
    );
    assert!(
        SQLITE_ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_reservations")
    );
    assert!(
        SQLITE_ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_usage_ledger")
    );
}

#[test]
fn sqlite_reservation_rejects_cross_tenant_and_over_limit_inputs() {
    let tenant_id = TenantId::new();
    let mut cross_tenant = command(tenant_id);
    let key_tenant = TenantId::new();
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_atomic_reservation(cross_tenant),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: tenant_id,
        })
    );

    let mut over_limit = command(tenant_id);
    over_limit.limit = BudgetLimit::new(1, 1);
    assert!(matches!(
        plan_sqlite_atomic_reservation(over_limit),
        Err(SqliteStoragePlanError::ReservationExceedsLimit { .. })
    ));
}

#[test]
fn sqlite_append_only_audit_plan_uses_immediate_transaction_and_dml_only() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_append_only_audit(audit_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "begin_immediate_audit_append");
    assert_eq!(plan.statements.last().unwrap().name, "commit_audit_append");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_APPEND_AUDIT_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_audit_log")
    );
}

#[test]
fn sqlite_append_only_audit_rejects_cross_tenant_storage_key() {
    let event_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = audit_command(event_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_append_only_audit(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        })
    );
}

#[test]
fn sqlite_audit_retention_purge_plan_uses_immediate_transaction_and_scoped_delete() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_audit_retention_purge(retention_purge_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.event_count, 2);
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_audit_retention_purge"
    );
    assert_eq!(plan.statements.last().unwrap().name, "commit_audit_append");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("DELETE FROM prodex_audit_log")
    );
    assert!(
        SQLITE_PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("audit_event_id IN (?2)")
    );
}

#[test]
fn sqlite_audit_export_query_plan_uses_tenant_scoped_select_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_audit_export_query(audit_export_query_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.sort_order, AuditSortOrder::OccurredAtDesc);
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.statements[0].name, "query_audit_export_desc_locally");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("FROM prodex_audit_log")
    );
    assert!(
        SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("ORDER BY occurred_at_unix_ms DESC")
    );
}

#[test]
fn sqlite_billing_ledger_query_plan_uses_tenant_scoped_select_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_billing_ledger_query(billing_ledger_query_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.sort_order, BillingLedgerSortOrder::OccurredAtDesc);
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.statements[0].name, "query_billing_ledger_desc_locally");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("FROM prodex_usage_ledger")
    );
    assert!(
        SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("ORDER BY occurred_at_unix_ms DESC")
    );
}

#[test]
fn sqlite_audit_retention_purge_rejects_cross_tenant_storage_key() {
    let batch_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = retention_purge_command(batch_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_audit_retention_purge(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: batch_tenant,
        })
    );
}

#[test]
fn sqlite_role_binding_grant_plan_uses_immediate_transaction_and_upsert() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_role_binding_mutation(role_binding_command(
        tenant_id,
        RoleBindingMutationKind::Grant,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.role, Role::Operator);
    assert_eq!(plan.kind, RoleBindingMutationKind::Grant);
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_role_binding_mutation"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_role_binding_mutation"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_GRANT_ROLE_BINDING_STATEMENT
            .sql
            .contains("INSERT INTO prodex_role_bindings")
    );
    assert!(
        SQLITE_GRANT_ROLE_BINDING_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, principal_id, role_name) DO UPDATE")
    );
}

#[test]
fn sqlite_role_binding_revoke_plan_uses_tenant_scoped_update() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_role_binding_mutation(role_binding_command(
        tenant_id,
        RoleBindingMutationKind::Revoke,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.kind, RoleBindingMutationKind::Revoke);
    assert!(
        SQLITE_REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("UPDATE prodex_role_bindings")
    );
    assert!(
        SQLITE_REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("role_binding_id = ?2")
    );
    assert!(
        SQLITE_REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("revoked_at_unix_ms IS NULL")
    );
}

#[test]
fn sqlite_role_binding_mutation_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = role_binding_command(request_tenant, RoleBindingMutationKind::Grant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_role_binding_mutation(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn sqlite_service_identity_create_plan_uses_immediate_transaction_and_upsert() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_service_identity_create(service_identity_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.display_name, "svc-ingest");
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_service_identity_create"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_service_identity_create"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT
            .sql
            .contains("INSERT INTO prodex_service_identities")
    );
    assert!(
        SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, principal_id) DO UPDATE")
    );
}

#[test]
fn sqlite_service_identity_create_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = service_identity_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_service_identity_create(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn sqlite_user_lifecycle_plan_uses_immediate_transaction_and_upsert_or_delete() {
    let tenant_id = TenantId::new();
    let create =
        plan_sqlite_user_lifecycle(user_lifecycle_command(tenant_id, UserLifecycleKind::Create))
            .unwrap();

    assert_eq!(create.tenant_id, tenant_id);
    assert_eq!(create.storage_key.tenant_id, tenant_id);
    assert_eq!(create.external_id, "scim-user-1");
    assert_eq!(create.display_name, "Ada Lovelace");
    assert_eq!(create.kind, UserLifecycleKind::Create);
    assert_eq!(create.statements[0].name, "begin_immediate_user_lifecycle");
    assert_eq!(create.statements[1].name, "upsert_user_lifecycle_locally");
    assert_eq!(
        create.statements.last().unwrap().name,
        "commit_user_lifecycle"
    );
    assert!(
        create
            .statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_users")
    );
    assert!(
        SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, principal_id) DO UPDATE")
    );

    let delete =
        plan_sqlite_user_lifecycle(user_lifecycle_command(tenant_id, UserLifecycleKind::Delete))
            .unwrap();
    assert_eq!(delete.kind, UserLifecycleKind::Delete);
    assert_eq!(delete.statements[1].name, "delete_user_lifecycle_locally");
    assert!(
        SQLITE_DELETE_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("deleted_at_unix_ms IS NULL")
    );
}

#[test]
fn sqlite_user_lifecycle_rejects_empty_identity_fields() {
    let tenant_id = TenantId::new();
    let mut missing_external = user_lifecycle_command(tenant_id, UserLifecycleKind::Delete);
    missing_external.external_id = " ".to_string();
    assert_eq!(
        plan_sqlite_user_lifecycle(missing_external),
        Err(SqliteStoragePlanError::EmptyUserExternalId)
    );

    let mut missing_display = user_lifecycle_command(tenant_id, UserLifecycleKind::Update);
    missing_display.display_name = " ".to_string();
    assert_eq!(
        plan_sqlite_user_lifecycle(missing_display),
        Err(SqliteStoragePlanError::EmptyUserDisplayName)
    );
}

#[test]
fn sqlite_budget_policy_update_plan_uses_immediate_transaction_and_upsert() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_budget_policy_update(budget_policy_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.budget_scope, "tenant-default");
    assert_eq!(plan.limit.max, UsageAmount::new(10_000, 1_000_000));
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_budget_policy_update"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_budget_policy_update"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_BUDGET_POLICY_STATEMENT
            .sql
            .contains("INSERT INTO prodex_budget_policies")
    );
    assert!(
        SQLITE_UPSERT_BUDGET_POLICY_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, budget_scope) DO UPDATE")
    );
}

#[test]
fn sqlite_budget_policy_update_rejects_empty_scope() {
    let tenant_id = TenantId::new();
    let mut command = budget_policy_command(tenant_id);
    command.budget_scope = " ".to_string();

    assert_eq!(
        plan_sqlite_budget_policy_update(command),
        Err(SqliteStoragePlanError::EmptyBudgetScope)
    );
}

#[test]
fn sqlite_tenant_lifecycle_plan_uses_immediate_transaction_and_upsert() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_tenant_lifecycle(tenant_lifecycle_command(
        tenant_id,
        TenantLifecycleKind::Create,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.display_name, "acme-prod");
    assert_eq!(plan.kind, TenantLifecycleKind::Create);
    assert_eq!(plan.statements[0].name, "begin_immediate_tenant_lifecycle");
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_tenant_lifecycle"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_tenants")
    );
    assert!(
        SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id) DO UPDATE")
    );
}

#[test]
fn sqlite_tenant_lifecycle_rejects_empty_display_name() {
    let tenant_id = TenantId::new();
    let mut command = tenant_lifecycle_command(tenant_id, TenantLifecycleKind::Update);
    command.display_name = " ".to_string();

    assert_eq!(
        plan_sqlite_tenant_lifecycle(command),
        Err(SqliteStoragePlanError::EmptyTenantDisplayName)
    );
}

#[test]
fn sqlite_virtual_key_secret_reference_plan_uses_immediate_transaction_and_no_secret_material() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_virtual_key_secret_reference(virtual_key_secret_command(
        tenant_id,
        VirtualKeySecretReferenceKind::Create,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.virtual_key_id, Some(plan.virtual_key_id));
    assert_eq!(plan.display_name, "prod-api");
    assert_eq!(plan.kind, VirtualKeySecretReferenceKind::Create);
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_virtual_key_secret_reference"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_virtual_key_secret_reference"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_virtual_keys")
    );
    assert!(
        SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, virtual_key_id) DO UPDATE")
    );
    assert!(
        !SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("secret_material")
    );
}

#[test]
fn sqlite_virtual_key_secret_reference_rejects_mismatched_virtual_key_storage_key() {
    let tenant_id = TenantId::new();
    let key_virtual_key_id = VirtualKeyId::new();
    let mut command = virtual_key_secret_command(tenant_id, VirtualKeySecretReferenceKind::Rotate);
    let request_virtual_key_id = command.virtual_key_id;
    command.storage_key = TenantStorageKey::virtual_key(tenant_id, key_virtual_key_id);

    assert_eq!(
        plan_sqlite_virtual_key_secret_reference(command),
        Err(SqliteStoragePlanError::VirtualKeyMismatch {
            key_virtual_key_id: Some(key_virtual_key_id),
            request_virtual_key_id,
        })
    );
}

#[test]
fn sqlite_provider_credential_reference_plan_uses_immediate_transaction_and_no_secret_material() {
    let tenant_id = TenantId::new();
    let plan =
        plan_sqlite_provider_credential_reference(provider_credential_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.provider_name, "openai");
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_provider_credential_reference"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_provider_credential_reference"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_provider_credentials")
    );
    assert!(
        SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("ON CONFLICT(tenant_id, provider_name) DO UPDATE")
    );
    assert!(
        !SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("secret_material")
    );
}

#[test]
fn sqlite_provider_credential_reference_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = provider_credential_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_provider_credential_reference(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn sqlite_idempotency_pending_record_plan_uses_immediate_transaction_and_insert_if_absent() {
    let tenant_id = TenantId::new();
    let plan =
        plan_sqlite_idempotency_pending_record(idempotency_pending_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_idempotency_pending"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_idempotency_pending"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_idempotency_records")
    );
    assert!(
        SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT
            .sql
            .contains("entry_status")
    );
}

#[test]
fn sqlite_idempotency_pending_record_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_pending_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_idempotency_pending_record(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn sqlite_idempotency_completed_record_plan_uses_immediate_transaction_and_fingerprint_guard() {
    let tenant_id = TenantId::new();
    let plan =
        plan_sqlite_idempotency_completed_record(idempotency_completed_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert_eq!(plan.response_byte_count, br#"{"status":"ok"}"#.len());
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_idempotency_completion"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_idempotency_completion"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("UPDATE prodex_idempotency_records")
    );
    assert!(
        SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("request_fingerprint = ?3")
    );
    assert!(
        SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("entry_status = 'pending'")
    );
}

#[test]
fn sqlite_idempotency_completed_record_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_completed_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_idempotency_completed_record(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn sqlite_idempotency_record_lookup_plan_uses_tenant_key_only_query() {
    let tenant_id = TenantId::new();
    let plan =
        plan_sqlite_idempotency_record_lookup(idempotency_lookup_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("FROM prodex_idempotency_records")
    );
    assert!(
        SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("WHERE tenant_id = ?1")
    );
    assert!(
        SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("idempotency_key = ?2")
    );
    assert!(
        SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("completed_at_unix_ms")
    );
    assert!(
        !SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("request_fingerprint =")
    );
}

#[test]
fn sqlite_idempotency_record_lookup_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_lookup_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_idempotency_record_lookup(command),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn sqlite_usage_reconciliation_plan_uses_immediate_transaction_and_dml_only() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.ledger_event_count, 2);
    assert_eq!(
        plan.statements[0].name,
        "begin_immediate_usage_reconciliation"
    );
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_usage_reconciliation"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_RECONCILE_USAGE_STATEMENT
            .sql
            .contains("reserved_tokens = reserved_tokens -")
    );
    assert!(
        SQLITE_RECONCILE_USAGE_STATEMENT
            .sql
            .contains("committed_tokens = committed_tokens +")
    );
    assert!(
        SQLITE_RECONCILE_USAGE_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_usage_ledger")
    );
    assert!(SQLITE_RECONCILE_USAGE_STATEMENT.sql.contains("'committed'"));
    assert!(SQLITE_RECONCILE_USAGE_STATEMENT.sql.contains("'released'"));
}

#[test]
fn sqlite_usage_reconciliation_plan_omits_release_ledger_when_actual_equals_reserved() {
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(10, 100),
    ))
    .unwrap();

    assert_eq!(plan.ledger_event_count, 1);
}

#[test]
fn sqlite_usage_reconciliation_rejects_cross_tenant_and_over_reserved_inputs() {
    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut cross_tenant = reconciliation_command(
        record_tenant,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_sqlite_usage_reconciliation(cross_tenant),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        })
    );

    assert!(matches!(
        plan_sqlite_usage_reconciliation(reconciliation_command(
            record_tenant,
            UsageAmount::new(10, 100),
            UsageAmount::new(11, 90),
        )),
        Err(SqliteStoragePlanError::ActualUsageExceedsReserved { .. })
    ));

    let mut overflow = reconciliation_command(
        record_tenant,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    overflow.snapshot.committed = UsageAmount::new(u64::MAX, 10);
    assert!(matches!(
        plan_sqlite_usage_reconciliation(overflow),
        Err(SqliteStoragePlanError::CommittedUsageOverflow { .. })
    ));
}

#[test]
fn sqlite_expired_reservation_recovery_plan_uses_immediate_transaction_dml_and_idempotent_release()
{
    let tenant_id = TenantId::new();
    let plan = plan_sqlite_expired_reservation_recovery(expired_recovery_command(
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "begin_immediate_expired_recovery");
    assert_eq!(
        plan.statements.last().unwrap().name,
        "commit_expired_recovery"
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("expires_at_unix_ms <= ?4")
    );
    assert!(
        SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("reserved_tokens = reserved_tokens -")
    );
    assert!(
        SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("released_at_unix_ms = ?4")
    );
    assert!(
        SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("INSERT OR IGNORE INTO prodex_usage_ledger")
    );
    assert!(
        SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("'released'")
    );
}

#[test]
fn sqlite_expired_reservation_recovery_rejects_not_expired_and_cross_tenant_inputs() {
    let tenant_id = TenantId::new();
    assert_eq!(
        plan_sqlite_expired_reservation_recovery(expired_recovery_command(
            tenant_id,
            UsageAmount::new(10, 100),
            1_499,
        )),
        Err(SqliteStoragePlanError::ReservationNotExpired)
    );

    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut cross_tenant =
        expired_recovery_command(record_tenant, UsageAmount::new(10, 100), 1_500);
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);
    assert_eq!(
        plan_sqlite_expired_reservation_recovery(cross_tenant),
        Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        })
    );
}
