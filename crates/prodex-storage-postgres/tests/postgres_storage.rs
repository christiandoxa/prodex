use postgres::NoTls;
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
use prodex_storage_postgres::{
    APPEND_AUDIT_STATEMENT, ATOMIC_RESERVATION_STATEMENT, COMPLETE_IDEMPOTENCY_RECORD_STATEMENT,
    DELETE_USER_LIFECYCLE_STATEMENT, GRANT_ROLE_BINDING_STATEMENT,
    INITIAL_TENANT_ACCOUNTING_MIGRATION, INSERT_IDEMPOTENCY_PENDING_STATEMENT,
    LOOKUP_IDEMPOTENCY_RECORD_STATEMENT, POSTGRES_MIGRATIONS, PURGE_AUDIT_RETENTION_STATEMENT,
    PostgresBackendOpenMode, PostgresMigrationVersion, PostgresRuntimeMode, PostgresStatement,
    PostgresStorageErrorStatus, PostgresStoragePlanError, QUERY_AUDIT_EXPORT_DESC_STATEMENT,
    QUERY_BILLING_LEDGER_DESC_STATEMENT, RECONCILE_USAGE_STATEMENT,
    RECOVER_EXPIRED_RESERVATION_STATEMENT, REQUIRED_POSTGRES_SCHEMA_VERSION,
    REVOKE_ROLE_BINDING_STATEMENT, SET_TENANT_STATEMENT, UPSERT_BUDGET_POLICY_STATEMENT,
    UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT, UPSERT_SERVICE_IDENTITY_STATEMENT,
    UPSERT_TENANT_LIFECYCLE_STATEMENT, UPSERT_USER_LIFECYCLE_STATEMENT,
    UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT, plan_postgres_append_only_audit,
    plan_postgres_atomic_reservation, plan_postgres_audit_export_query,
    plan_postgres_audit_retention_purge, plan_postgres_backend_open,
    plan_postgres_billing_ledger_query, plan_postgres_budget_policy_update,
    plan_postgres_expired_reservation_recovery, plan_postgres_idempotency_completed_record,
    plan_postgres_idempotency_pending_record, plan_postgres_idempotency_record_lookup,
    plan_postgres_migrations, plan_postgres_provider_credential_reference,
    plan_postgres_role_binding_mutation, plan_postgres_service_identity_create,
    plan_postgres_storage_error_response, plan_postgres_tenant_context,
    plan_postgres_tenant_lifecycle, plan_postgres_usage_reconciliation,
    plan_postgres_user_lifecycle, plan_postgres_virtual_key_secret_reference,
    statement_contains_ddl,
};
use std::sync::{Arc, Barrier};

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

fn execute_postgres_batch(url: &str, sql: &str) {
    let mut client = postgres::Client::connect(url, NoTls).expect("postgres should connect");
    client
        .batch_execute(sql)
        .expect("postgres batch should execute");
}

fn execute_postgres_atomic_reservation(
    url: &str,
    command: &AtomicReservationCommand,
) -> Result<bool, postgres::Error> {
    let plan = plan_postgres_atomic_reservation(command.clone())
        .expect("postgres reservation plan should build");
    let mut client = postgres::Client::connect(url, NoTls).expect("postgres should connect");
    let mut tx = client.transaction()?;
    tx.execute(SET_TENANT_STATEMENT.sql, &[&plan.tenant_id.to_string()])?;
    let storage_scope = command.storage_key.storage_scope();
    let reserved = command.request.estimate;
    let updated = command.created_at_unix_ms as i64;
    let expires_at = command.created_at_unix_ms.saturating_add(command.ttl_ms) as i64;
    let row = tx.query_opt(
        ATOMIC_RESERVATION_STATEMENT.sql,
        &[
            &plan.tenant_id.as_uuid(),
            &command.idempotency_key.as_str(),
            &storage_scope,
            &command.storage_key.virtual_key_id.map(|id| id.as_uuid()),
            &(reserved.tokens as i64),
            &(reserved.cost_micros as i64),
            &updated,
            &(command.limit.max.tokens as i64),
            &(command.limit.max.cost_micros as i64),
            &(command.limit.max_requests.min(i64::MAX as u64) as i64),
            &command.request.reservation_id.as_uuid(),
            &command.request.call_id.as_uuid(),
            &expires_at,
            &prodex_domain::RequestId::new().as_uuid(),
        ],
    )?;
    tx.commit()?;
    Ok(row.is_some())
}

#[test]
fn postgres_storage_error_response_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let error = PostgresStoragePlanError::TenantMismatch {
        key_tenant: tenant_id,
        request_tenant: other_tenant,
    };
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&other_tenant.to_string()));

    let schema = PostgresStoragePlanError::SchemaVersionTooOld {
        observed: PostgresMigrationVersion(1),
        required: PostgresMigrationVersion(3),
    };
    assert!(!schema.to_string().contains("1"));
    assert!(!schema.to_string().contains("2"));

    let usage = PostgresStoragePlanError::ActualUsageExceedsReserved {
        reserved: UsageAmount::new(10, 100),
        actual: UsageAmount::new(11, 90),
    };
    assert!(!usage.to_string().contains("10"));
    assert!(!usage.to_string().contains("100"));
    assert!(!usage.to_string().contains("11"));

    let response = plan_postgres_storage_error_response(&error);

    assert_eq!(
        response.status,
        PostgresStorageErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "postgres_storage_unavailable");
    assert_eq!(
        response.message,
        "postgres storage planning is temporarily unavailable"
    );
    assert!(!response.message.contains("PostgreSQL"));
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
            AuditAction::new("control_plane.configuration.publish"),
            AuditResource::new("Configuration", Some("revision-1"), Some(tenant_id)),
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

fn assert_request_path_has_tenant_context(statements: &[PostgresStatement]) {
    assert_eq!(statements.first(), Some(&SET_TENANT_STATEMENT));
    assert!(
        statements
            .first()
            .unwrap()
            .sql
            .contains("set_config('prodex.tenant_id', $1, true)")
    );
}

#[test]
fn migrations_are_external_only_and_request_path_cannot_plan_ddl() {
    let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator).unwrap();
    assert_eq!(plan.migrations.len(), POSTGRES_MIGRATIONS.len());
    assert!(statement_contains_ddl(plan.migrations[0].sql));

    assert_eq!(
        plan_postgres_migrations(PostgresRuntimeMode::GatewayRequestPath),
        Err(PostgresStoragePlanError::DdlForbiddenOnRequestPath)
    );
}

#[test]
fn backend_open_requires_known_current_schema_without_ddl() {
    let plan = plan_postgres_backend_open(
        PostgresBackendOpenMode::GatewayStartup,
        Some(REQUIRED_POSTGRES_SCHEMA_VERSION),
    )
    .unwrap();

    assert_eq!(plan.mode, PostgresBackendOpenMode::GatewayStartup);
    assert_eq!(
        plan.required_schema_version,
        REQUIRED_POSTGRES_SCHEMA_VERSION
    );
    assert!(!plan.ddl_allowed);
    assert_eq!(plan.migration_count, 0);

    assert_eq!(
        plan_postgres_backend_open(PostgresBackendOpenMode::GatewayRequestPath, None),
        Err(PostgresStoragePlanError::MissingSchemaVersion)
    );
    assert_eq!(
        plan_postgres_backend_open(
            PostgresBackendOpenMode::GatewayRequestPath,
            Some(PostgresMigrationVersion(0)),
        ),
        Err(PostgresStoragePlanError::SchemaVersionTooOld {
            observed: PostgresMigrationVersion(0),
            required: REQUIRED_POSTGRES_SCHEMA_VERSION,
        })
    );
}

#[test]
fn external_migrator_open_is_the_only_ddl_eligible_mode() {
    let plan = plan_postgres_backend_open(PostgresBackendOpenMode::ExternalMigrator, None).unwrap();

    assert_eq!(plan.mode, PostgresBackendOpenMode::ExternalMigrator);
    assert_eq!(
        plan.required_schema_version,
        REQUIRED_POSTGRES_SCHEMA_VERSION
    );
    assert!(plan.ddl_allowed);
    assert_eq!(plan.migration_count, POSTGRES_MIGRATIONS.len());
}

#[test]
fn tenant_context_plan_sets_local_rls_session_variable() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_tenant_context(tenant_id);

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.statement, SET_TENANT_STATEMENT);
    assert!(plan.statement.sql.contains("set_config('prodex.tenant_id'"));
    assert!(plan.statement.sql.contains("true"));
}

#[test]
fn all_request_path_plans_start_with_rls_tenant_context() {
    let tenant_id = TenantId::new();

    assert_request_path_has_tenant_context(
        &plan_postgres_atomic_reservation(command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_append_only_audit(audit_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_audit_retention_purge(retention_purge_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_audit_export_query(audit_export_query_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_role_binding_mutation(role_binding_command(
            tenant_id,
            RoleBindingMutationKind::Grant,
        ))
        .unwrap()
        .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_service_identity_create(service_identity_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_user_lifecycle(user_lifecycle_command(tenant_id, UserLifecycleKind::Create))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_budget_policy_update(budget_policy_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_tenant_lifecycle(tenant_lifecycle_command(
            tenant_id,
            TenantLifecycleKind::Create,
        ))
        .unwrap()
        .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_virtual_key_secret_reference(virtual_key_secret_command(
            tenant_id,
            VirtualKeySecretReferenceKind::Create,
        ))
        .unwrap()
        .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_provider_credential_reference(provider_credential_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_usage_reconciliation(reconciliation_command(
            tenant_id,
            UsageAmount::new(25, 250),
            UsageAmount::new(10, 100),
        ))
        .unwrap()
        .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_billing_ledger_query(billing_ledger_query_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_expired_reservation_recovery(expired_recovery_command(
            tenant_id,
            UsageAmount::new(25, 250),
            2_000,
        ))
        .unwrap()
        .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_idempotency_pending_record(idempotency_pending_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_idempotency_completed_record(idempotency_completed_command(tenant_id))
            .unwrap()
            .statements,
    );
    assert_request_path_has_tenant_context(
        &plan_postgres_idempotency_record_lookup(idempotency_lookup_command(tenant_id))
            .unwrap()
            .statements,
    );
}

#[test]
fn migration_enforces_tenant_keys_unique_ledger_and_rls() {
    let sql = INITIAL_TENANT_ACCOUNTING_MIGRATION.sql;

    for table in [
        "prodex_tenants",
        "prodex_virtual_keys",
        "prodex_service_identities",
        "prodex_users",
        "prodex_role_bindings",
        "prodex_provider_credentials",
        "prodex_budget_counters",
        "prodex_budget_policies",
        "prodex_reservations",
        "prodex_usage_ledger",
        "prodex_audit_log",
        "prodex_idempotency_records",
    ] {
        assert!(sql.contains(&format!("{table} ENABLE ROW LEVEL SECURITY")));
    }
    assert!(sql.contains("PRIMARY KEY (tenant_id, virtual_key_id)"));
    assert!(sql.contains("secret_provider TEXT NOT NULL"));
    assert!(sql.contains("secret_name TEXT NOT NULL"));
    assert!(sql.contains("secret_rotated_at_unix_ms BIGINT NOT NULL"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, principal_id)"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS prodex_users"));
    assert!(sql.contains("UNIQUE (tenant_id, external_id)"));
    assert!(sql.contains("deleted_at_unix_ms BIGINT"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, role_binding_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, provider_credential_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, storage_scope)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, budget_scope)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, reservation_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, ledger_event_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, audit_event_id)"));
    assert!(sql.contains("PRIMARY KEY (tenant_id, idempotency_key)"));
    assert!(sql.contains("UNIQUE (tenant_id, call_id)"));
    assert!(sql.contains("UNIQUE (tenant_id, idempotency_key)"));
    assert!(sql.contains("UNIQUE (tenant_id, principal_id, role_name)"));
    assert!(sql.contains("display_name TEXT NOT NULL"));
    assert!(sql.contains("updated_at_unix_ms BIGINT NOT NULL"));
    assert!(sql.contains("CHECK (display_name <> '')"));
    assert!(sql.contains("UNIQUE (tenant_id, provider_name)"));
    assert!(sql.contains("UNIQUE (tenant_id, reservation_id, event_kind)"));
    assert!(sql.contains("UNIQUE (tenant_id, event_digest)"));
    assert!(sql.contains("UNIQUE (tenant_id, previous_digest)"));
    assert!(sql.contains("current_setting('prodex.tenant_id', true)::uuid"));
}

#[test]
fn atomic_reservation_plan_uses_tenant_context_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_atomic_reservation(command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements[0]
            .sql
            .contains("set_config('prodex.tenant_id'")
    );
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, storage_scope) DO UPDATE")
    );
    assert!(
        ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("WHERE NOT EXISTS (SELECT 1 FROM existing)")
    );
    assert!(
        ATOMIC_RESERVATION_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING")
    );
}

#[test]
fn atomic_reservation_rejects_cross_tenant_and_over_limit_inputs() {
    let tenant_id = TenantId::new();
    let mut cross_tenant = command(tenant_id);
    let key_tenant = TenantId::new();
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_atomic_reservation(cross_tenant),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: tenant_id,
        })
    );

    let mut over_limit = command(tenant_id);
    over_limit.limit = BudgetLimit::new(1, 1);
    assert!(matches!(
        plan_postgres_atomic_reservation(over_limit),
        Err(PostgresStoragePlanError::ReservationExceedsLimit { .. })
    ));
}

#[test]
fn postgres_atomic_reservation_allows_only_one_concurrent_claim_per_budget_scope() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    execute_postgres_batch(&url, "DROP SCHEMA public CASCADE; CREATE SCHEMA public;");
    execute_postgres_batch(&url, INITIAL_TENANT_ACCOUNTING_MIGRATION.sql);

    let tenant_id = TenantId::new();
    let virtual_key_id = VirtualKeyId::new();
    let mut client = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    client
        .execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES ($1, $2, $3, $4)",
            &[&tenant_id.as_uuid(), &"tenant", &1_i64, &1_i64],
        )
        .expect("tenant row should insert");

    let make_command = move || {
        let call_id = CallId::new();
        let reservation_id = ReservationId::new();
        AtomicReservationCommand {
            storage_key: TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
            idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
            snapshot: BudgetSnapshot::default(),
            limit: BudgetLimit::new(u64::MAX, 42),
            request: ReservationRequest {
                tenant_id,
                call_id,
                reservation_id,
                estimate: UsageAmount::new(22, 42),
            },
            created_at_unix_ms: 1_000,
            ttl_ms: 60_000,
        }
    };
    let barrier = Arc::new(Barrier::new(2));
    let run = |barrier: Arc<Barrier>| {
        let url = url.clone();
        std::thread::spawn(move || {
            let command = make_command();
            barrier.wait();
            execute_postgres_atomic_reservation(&url, &command)
        })
    };

    let first = run(Arc::clone(&barrier));
    let second = run(barrier);
    let results = [
        first.join().expect("first thread should finish"),
        second.join().expect("second thread should finish"),
    ];
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(true)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(false)))
            .count(),
        1
    );

    let reservation_rows: i64 = client
        .query_one("SELECT COUNT(*) FROM prodex_reservations", &[])
        .expect("reservation count should load")
        .get(0);
    let reserved_rows: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE event_kind = 'reserved'",
            &[],
        )
        .expect("reserved ledger count should load")
        .get(0);
    let counter_row = client
        .query_one(
            "SELECT reserved_tokens, reserved_cost_micros FROM prodex_budget_counters",
            &[],
        )
        .expect("budget counters should load");
    let reserved_tokens: i64 = counter_row.get(0);
    let reserved_cost_micros: i64 = counter_row.get(1);
    assert_eq!(reservation_rows, 1);
    assert_eq!(reserved_rows, 1);
    assert_eq!(reserved_tokens, 22);
    assert_eq!(reserved_cost_micros, 42);
}

#[test]
fn append_only_audit_plan_uses_rls_tenant_context_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_append_only_audit(audit_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        APPEND_AUDIT_STATEMENT
            .sql
            .contains("INSERT INTO prodex_audit_log")
    );
    assert!(
        APPEND_AUDIT_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, audit_event_id) DO NOTHING")
    );
    assert!(APPEND_AUDIT_STATEMENT.sql.contains("RETURNING tenant_id"));
}

#[test]
fn append_only_audit_rejects_cross_tenant_storage_key() {
    let event_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = audit_command(event_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_append_only_audit(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        })
    );
}

#[test]
fn audit_retention_purge_plan_uses_tenant_context_and_scoped_delete() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_audit_retention_purge(retention_purge_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.event_count, 2);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("DELETE FROM prodex_audit_log")
    );
    assert!(
        PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        PURGE_AUDIT_RETENTION_STATEMENT
            .sql
            .contains("audit_event_id = ANY($2)")
    );
}

#[test]
fn audit_export_query_plan_uses_tenant_context_select_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_audit_export_query(audit_export_query_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.sort_order, AuditSortOrder::OccurredAtDesc);
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert_eq!(plan.statements[1].name, "query_audit_export_desc");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("FROM prodex_audit_log")
    );
    assert!(
        QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        QUERY_AUDIT_EXPORT_DESC_STATEMENT
            .sql
            .contains("ORDER BY occurred_at_unix_ms DESC")
    );
}

#[test]
fn billing_ledger_query_plan_uses_tenant_context_select_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_billing_ledger_query(billing_ledger_query_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.sort_order, BillingLedgerSortOrder::OccurredAtDesc);
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert_eq!(plan.statements[1].name, "query_billing_ledger_desc");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("FROM prodex_usage_ledger")
    );
    assert!(
        QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        QUERY_BILLING_LEDGER_DESC_STATEMENT
            .sql
            .contains("ORDER BY occurred_at_unix_ms DESC")
    );
}

#[test]
fn audit_retention_purge_rejects_cross_tenant_storage_key() {
    let batch_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = retention_purge_command(batch_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_audit_retention_purge(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: batch_tenant,
        })
    );
}

#[test]
fn role_binding_grant_plan_uses_tenant_context_upsert_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_role_binding_mutation(role_binding_command(
        tenant_id,
        RoleBindingMutationKind::Grant,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.role, Role::Operator);
    assert_eq!(plan.kind, RoleBindingMutationKind::Grant);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        GRANT_ROLE_BINDING_STATEMENT
            .sql
            .contains("INSERT INTO prodex_role_bindings")
    );
    assert!(
        GRANT_ROLE_BINDING_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, principal_id, role_name) DO UPDATE")
    );
    assert!(
        GRANT_ROLE_BINDING_STATEMENT
            .sql
            .contains("WHERE prodex_role_bindings.tenant_id = EXCLUDED.tenant_id")
    );
}

#[test]
fn role_binding_revoke_plan_uses_tenant_scoped_update() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_role_binding_mutation(role_binding_command(
        tenant_id,
        RoleBindingMutationKind::Revoke,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.kind, RoleBindingMutationKind::Revoke);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("UPDATE prodex_role_bindings")
    );
    assert!(
        REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("role_binding_id = $2")
    );
    assert!(
        REVOKE_ROLE_BINDING_STATEMENT
            .sql
            .contains("revoked_at_unix_ms IS NULL")
    );
}

#[test]
fn role_binding_mutation_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = role_binding_command(request_tenant, RoleBindingMutationKind::Grant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_role_binding_mutation(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn service_identity_create_plan_uses_tenant_context_upsert_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_service_identity_create(service_identity_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.display_name, "svc-ingest");
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_SERVICE_IDENTITY_STATEMENT
            .sql
            .contains("INSERT INTO prodex_service_identities")
    );
    assert!(
        UPSERT_SERVICE_IDENTITY_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, principal_id) DO UPDATE")
    );
    assert!(
        UPSERT_SERVICE_IDENTITY_STATEMENT
            .sql
            .contains("WHERE prodex_service_identities.tenant_id = EXCLUDED.tenant_id")
    );
}

#[test]
fn service_identity_create_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = service_identity_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_service_identity_create(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn user_lifecycle_plan_uses_tenant_context_upsert_or_delete_and_no_ddl() {
    let tenant_id = TenantId::new();
    let create =
        plan_postgres_user_lifecycle(user_lifecycle_command(tenant_id, UserLifecycleKind::Create))
            .unwrap();

    assert_eq!(create.tenant_id, tenant_id);
    assert_eq!(create.storage_key.tenant_id, tenant_id);
    assert_eq!(create.external_id, "scim-user-1");
    assert_eq!(create.display_name, "Ada Lovelace");
    assert_eq!(create.kind, UserLifecycleKind::Create);
    assert_eq!(create.statements[0].name, "set_tenant_context");
    assert_eq!(create.statements[1].name, "upsert_user_lifecycle");
    assert!(
        create
            .statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_users")
    );
    assert!(
        UPSERT_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, principal_id) DO UPDATE")
    );

    let delete =
        plan_postgres_user_lifecycle(user_lifecycle_command(tenant_id, UserLifecycleKind::Delete))
            .unwrap();
    assert_eq!(delete.kind, UserLifecycleKind::Delete);
    assert_eq!(delete.statements[1].name, "delete_user_lifecycle");
    assert!(
        DELETE_USER_LIFECYCLE_STATEMENT
            .sql
            .contains("deleted_at_unix_ms IS NULL")
    );
}

#[test]
fn user_lifecycle_rejects_empty_identity_fields() {
    let tenant_id = TenantId::new();
    let mut missing_external = user_lifecycle_command(tenant_id, UserLifecycleKind::Delete);
    missing_external.external_id = " ".to_string();
    assert_eq!(
        plan_postgres_user_lifecycle(missing_external),
        Err(PostgresStoragePlanError::EmptyUserExternalId)
    );

    let mut missing_display = user_lifecycle_command(tenant_id, UserLifecycleKind::Update);
    missing_display.display_name = " ".to_string();
    assert_eq!(
        plan_postgres_user_lifecycle(missing_display),
        Err(PostgresStoragePlanError::EmptyUserDisplayName)
    );
}

#[test]
fn budget_policy_update_plan_uses_tenant_context_upsert_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_budget_policy_update(budget_policy_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.budget_scope, "tenant-default");
    assert_eq!(plan.limit.max, UsageAmount::new(10_000, 1_000_000));
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_BUDGET_POLICY_STATEMENT
            .sql
            .contains("INSERT INTO prodex_budget_policies")
    );
    assert!(
        UPSERT_BUDGET_POLICY_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, budget_scope) DO UPDATE")
    );
}

#[test]
fn budget_policy_update_rejects_empty_scope() {
    let tenant_id = TenantId::new();
    let mut command = budget_policy_command(tenant_id);
    command.budget_scope = " ".to_string();

    assert_eq!(
        plan_postgres_budget_policy_update(command),
        Err(PostgresStoragePlanError::EmptyBudgetScope)
    );
}

#[test]
fn tenant_lifecycle_plan_uses_tenant_context_upsert_and_no_ddl() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_tenant_lifecycle(tenant_lifecycle_command(
        tenant_id,
        TenantLifecycleKind::Create,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.display_name, "acme-prod");
    assert_eq!(plan.kind, TenantLifecycleKind::Create);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_TENANT_LIFECYCLE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_tenants")
    );
    assert!(
        UPSERT_TENANT_LIFECYCLE_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id) DO UPDATE")
    );
    assert!(
        UPSERT_TENANT_LIFECYCLE_STATEMENT
            .sql
            .contains("WHERE prodex_tenants.tenant_id = EXCLUDED.tenant_id")
    );
}

#[test]
fn tenant_lifecycle_rejects_empty_display_name() {
    let tenant_id = TenantId::new();
    let mut command = tenant_lifecycle_command(tenant_id, TenantLifecycleKind::Update);
    command.display_name = " ".to_string();

    assert_eq!(
        plan_postgres_tenant_lifecycle(command),
        Err(PostgresStoragePlanError::EmptyTenantDisplayName)
    );
}

#[test]
fn virtual_key_secret_reference_plan_uses_tenant_context_upsert_and_no_secret_material() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_virtual_key_secret_reference(virtual_key_secret_command(
        tenant_id,
        VirtualKeySecretReferenceKind::Create,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.virtual_key_id, Some(plan.virtual_key_id));
    assert_eq!(plan.display_name, "prod-api");
    assert_eq!(plan.kind, VirtualKeySecretReferenceKind::Create);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_virtual_keys")
    );
    assert!(
        UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, virtual_key_id) DO UPDATE")
    );
    assert!(
        UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("secret_provider")
    );
    assert!(
        UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("secret_name")
    );
    assert!(
        !UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT
            .sql
            .contains("secret_material")
    );
}

#[test]
fn virtual_key_secret_reference_rejects_mismatched_virtual_key_storage_key() {
    let tenant_id = TenantId::new();
    let key_virtual_key_id = VirtualKeyId::new();
    let mut command = virtual_key_secret_command(tenant_id, VirtualKeySecretReferenceKind::Rotate);
    let request_virtual_key_id = command.virtual_key_id;
    command.storage_key = TenantStorageKey::virtual_key(tenant_id, key_virtual_key_id);

    assert_eq!(
        plan_postgres_virtual_key_secret_reference(command),
        Err(PostgresStoragePlanError::VirtualKeyMismatch {
            key_virtual_key_id: Some(key_virtual_key_id),
            request_virtual_key_id,
        })
    );
}

#[test]
fn provider_credential_reference_plan_uses_tenant_context_upsert_and_no_secret_material() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_provider_credential_reference(provider_credential_command(tenant_id))
        .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.provider_name, "openai");
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("INSERT INTO prodex_provider_credentials")
    );
    assert!(
        UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, provider_name) DO UPDATE")
    );
    assert!(
        UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("secret_provider")
    );
    assert!(
        UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("secret_name")
    );
    assert!(
        !UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT
            .sql
            .contains("secret_material")
    );
}

#[test]
fn provider_credential_reference_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = provider_credential_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_provider_credential_reference(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn idempotency_pending_record_plan_uses_tenant_context_and_insert_if_absent() {
    let tenant_id = TenantId::new();
    let plan =
        plan_postgres_idempotency_pending_record(idempotency_pending_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        INSERT_IDEMPOTENCY_PENDING_STATEMENT
            .sql
            .contains("INSERT INTO prodex_idempotency_records")
    );
    assert!(
        INSERT_IDEMPOTENCY_PENDING_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, idempotency_key) DO NOTHING")
    );
    assert!(
        INSERT_IDEMPOTENCY_PENDING_STATEMENT
            .sql
            .contains("RETURNING tenant_id")
    );
}

#[test]
fn idempotency_pending_record_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_pending_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_idempotency_pending_record(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn idempotency_completed_record_plan_uses_tenant_context_and_fingerprint_guard() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_idempotency_completed_record(idempotency_completed_command(tenant_id))
        .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert_eq!(plan.response_byte_count, br#"{"status":"ok"}"#.len());
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("UPDATE prodex_idempotency_records")
    );
    assert!(
        COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("request_fingerprint = $3")
    );
    assert!(
        COMPLETE_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("entry_status = 'pending'")
    );
}

#[test]
fn idempotency_completed_record_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_completed_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_idempotency_completed_record(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn idempotency_record_lookup_plan_uses_tenant_context_and_key_only_query() {
    let tenant_id = TenantId::new();
    let plan =
        plan_postgres_idempotency_record_lookup(idempotency_lookup_command(tenant_id)).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key.as_str(), "admin-mutation-1");
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("FROM prodex_idempotency_records")
    );
    assert!(
        LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("WHERE tenant_id = $1")
    );
    assert!(
        LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("idempotency_key = $2")
    );
    assert!(
        LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("completed_at_unix_ms")
    );
    assert!(
        !LOOKUP_IDEMPOTENCY_RECORD_STATEMENT
            .sql
            .contains("request_fingerprint =")
    );
}

#[test]
fn idempotency_record_lookup_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_lookup_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_idempotency_record_lookup(command),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: operation_tenant,
        })
    );
}

#[test]
fn usage_reconciliation_plan_uses_tenant_context_dml_and_idempotent_ledger_events() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.ledger_event_count, 2);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(RECONCILE_USAGE_STATEMENT.sql.contains("FOR UPDATE"));
    assert!(
        RECONCILE_USAGE_STATEMENT
            .sql
            .contains("reserved_tokens = reserved_tokens -")
    );
    assert!(
        RECONCILE_USAGE_STATEMENT
            .sql
            .contains("committed_tokens = committed_tokens +")
    );
    assert!(
        RECONCILE_USAGE_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING")
    );
    assert!(RECONCILE_USAGE_STATEMENT.sql.contains("'committed'"));
    assert!(RECONCILE_USAGE_STATEMENT.sql.contains("'released'"));
}

#[test]
fn usage_reconciliation_plan_omits_release_ledger_when_actual_equals_reserved() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(10, 100),
    ))
    .unwrap();

    assert_eq!(plan.ledger_event_count, 1);
}

#[test]
fn usage_reconciliation_rejects_cross_tenant_and_overflow_inputs() {
    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut cross_tenant = reconciliation_command(
        record_tenant,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_postgres_usage_reconciliation(cross_tenant),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        })
    );

    assert_eq!(
        plan_postgres_usage_reconciliation(reconciliation_command(
            record_tenant,
            UsageAmount::new(10, 100),
            UsageAmount::new(11, 110),
        ))
        .unwrap()
        .ledger_event_count,
        1
    );

    let mut overflow = reconciliation_command(
        record_tenant,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    overflow.snapshot.committed = UsageAmount::new(u64::MAX, 10);
    assert!(matches!(
        plan_postgres_usage_reconciliation(overflow),
        Err(PostgresStoragePlanError::CommittedUsageOverflow { .. })
    ));
}

#[test]
fn expired_reservation_recovery_plan_uses_tenant_context_dml_and_idempotent_release() {
    let tenant_id = TenantId::new();
    let plan = plan_postgres_expired_reservation_recovery(expired_recovery_command(
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.statements[0].name, "set_tenant_context");
    assert!(
        plan.statements
            .iter()
            .all(|statement| !statement_contains_ddl(statement.sql))
    );
    assert!(
        RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("expires_at_unix_ms <= $4")
    );
    assert!(
        RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("reserved_tokens = reserved_tokens -")
    );
    assert!(
        RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("released_at_unix_ms = $4")
    );
    assert!(
        RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("ON CONFLICT (tenant_id, reservation_id, event_kind) DO NOTHING")
    );
    assert!(
        RECOVER_EXPIRED_RESERVATION_STATEMENT
            .sql
            .contains("'released'")
    );
}

#[test]
fn expired_reservation_recovery_rejects_not_expired_and_cross_tenant_inputs() {
    let tenant_id = TenantId::new();
    assert_eq!(
        plan_postgres_expired_reservation_recovery(expired_recovery_command(
            tenant_id,
            UsageAmount::new(10, 100),
            1_499,
        )),
        Err(PostgresStoragePlanError::ReservationNotExpired)
    );

    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut cross_tenant =
        expired_recovery_command(record_tenant, UsageAmount::new(10, 100), 1_500);
    cross_tenant.storage_key = TenantStorageKey::tenant(key_tenant);
    assert_eq!(
        plan_postgres_expired_reservation_recovery(cross_tenant),
        Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        })
    );
}
