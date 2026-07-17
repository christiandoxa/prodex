use prodex_domain::{
    AuditAction, AuditDigest, AuditExportFormat, AuditExportPlan, AuditOutcome, AuditPageLimit,
    AuditQueryPlan, AuditQueryScope, AuditResource, AuditRetentionBatchLimit,
    AuditRetentionPurgeBatch, AuditRetentionPurgeKey, AuditSortOrder, AuditTimeRange,
    AuditTimestamp, BudgetLimit, BudgetSnapshot, CallId, CredentialScope, IdempotencyEntry,
    IdempotencyKey, IdempotencyRecord, IdempotentOperation, LedgerEventKind, Principal,
    PrincipalId, PrincipalKind, ProviderCredentialId, ReservationId,
    ReservationReconciliationReason, ReservationRecord, ReservationRequest, Role, RoleBindingId,
    SecretRef, TenantContext, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AppendOnlyAuditPlanError, AtomicReservationCommand,
    AtomicReservationPlanError, AuditAppendMode, AuditExportQueryCommand,
    AuditExportQueryPlanError, AuditRetentionPurgeCommand, AuditRetentionPurgePlanError,
    BillingLedgerPageLimit, BillingLedgerQueryCommand, BillingLedgerQueryPlanError,
    BillingLedgerSortOrder, BudgetPolicyUpdateCommand, BudgetPolicyUpdatePlanError, CacheStoreKind,
    DurableStoreKind, ExpiredReservationRecoveryCommand, ExpiredReservationRecoveryPlanError,
    IdempotencyCompletedRecordCommand, IdempotencyCompletedRecordPlanError,
    IdempotencyPendingRecordCommand, IdempotencyPendingRecordPlanError,
    IdempotencyRecordLookupCommand, IdempotencyRecordLookupPlanError, IdempotencyRecordLookupRow,
    IdempotencyRecordLookupRowError, IdempotencyRecordLookupRowStatus, MigrationRuntimePolicy,
    MultiReplicaAccountingCheck, MultiReplicaAccountingConcurrencySpecError,
    MultiReplicaAccountingEvidence, ProviderCredentialReferenceCommand,
    ProviderCredentialReferencePlanError, RoleBindingMutationCommand, RoleBindingMutationKind,
    RoleBindingMutationPlanError, ServiceIdentityCreateCommand, ServiceIdentityCreatePlanError,
    StoragePlanErrorStatus, StorageTopology, StorageTopologyError, TenantLifecycleCommand,
    TenantLifecycleKind, TenantLifecyclePlanError, TenantStorageKey, UsageReconciliationCommand,
    UsageReconciliationPlanError, UserLifecycleCommand, UserLifecycleKind, UserLifecyclePlanError,
    VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
    VirtualKeySecretReferencePlanError, materialize_idempotency_record_lookup_row,
    materialize_idempotency_record_lookup_row_error_response, plan_append_only_audit,
    plan_append_only_audit_error_response, plan_atomic_reservation,
    plan_atomic_reservation_error_response, plan_audit_export_query,
    plan_audit_export_query_error_response, plan_audit_retention_purge,
    plan_audit_retention_purge_error_response, plan_billing_ledger_query,
    plan_billing_ledger_query_error_response, plan_budget_policy_update,
    plan_budget_policy_update_error_response, plan_expired_reservation_recovery,
    plan_expired_reservation_recovery_error_response, plan_idempotency_completed_record,
    plan_idempotency_completed_record_error_response, plan_idempotency_pending_record,
    plan_idempotency_pending_record_error_response, plan_idempotency_record_lookup,
    plan_idempotency_record_lookup_error_response, plan_multi_replica_accounting_concurrency_spec,
    plan_multi_replica_accounting_error_response, plan_multi_replica_accounting_verification,
    plan_provider_credential_reference, plan_provider_credential_reference_error_response,
    plan_role_binding_mutation, plan_role_binding_mutation_error_response,
    plan_service_identity_create, plan_service_identity_create_error_response,
    plan_storage_topology_error_response, plan_tenant_lifecycle,
    plan_tenant_lifecycle_error_response, plan_usage_reconciliation,
    plan_usage_reconciliation_error_response, plan_user_lifecycle,
    plan_user_lifecycle_error_response, plan_virtual_key_secret_reference,
    plan_virtual_key_secret_reference_error_response,
};

fn request(tenant_id: TenantId) -> ReservationRequest {
    ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: UsageAmount::new(10, 100),
    }
}

fn command(tenant_id: TenantId) -> AtomicReservationCommand {
    let request = request(tenant_id);
    AtomicReservationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        idempotency_key: IdempotencyKey::from_call_reservation(
            request.call_id,
            request.reservation_id,
        ),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(100, 1_000),
        request,
        created_at_unix_ms: 1_000,
        ttl_ms: 60_000,
    }
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
            AuditAction::new("control_plane.policy.publish"),
            AuditResource::new("Policy", Some("policy-1"), Some(tenant_id)),
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
        [AuditRetentionPurgeKey {
            tenant_id,
            event_id: prodex_domain::AuditEventId::new(),
        }],
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
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-mutation",
        )
        .unwrap(),
        started_at_unix_ms: 1_800_000_000_000,
    }
}

fn idempotency_completed_command(tenant_id: TenantId) -> IdempotencyCompletedRecordCommand {
    IdempotencyCompletedRecordCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-mutation",
        )
        .unwrap(),
        completed_at_unix_ms: 1_800_000_001_000,
        response_body: br#"{"status":"ok"}"#.to_vec(),
    }
}

fn idempotency_lookup_command(tenant_id: TenantId) -> IdempotencyRecordLookupCommand {
    IdempotencyRecordLookupCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "sha256:admin-mutation",
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

fn idempotency_lookup_row(
    tenant_id: TenantId,
    status: IdempotencyRecordLookupRowStatus,
) -> IdempotencyRecordLookupRow {
    IdempotencyRecordLookupRow {
        tenant_id,
        idempotency_key: IdempotencyKey::new("admin-mutation-1").unwrap(),
        request_fingerprint: "sha256:admin-mutation".to_string(),
        status,
        started_at_unix_ms: 1_800_000_000_000,
        completed_at_unix_ms: Some(1_800_000_001_000),
        response_body: Some(br#"{"status":"ok"}"#.to_vec()),
    }
}

#[test]
fn storage_plan_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let topology =
        plan_storage_topology_error_response(&StorageTopologyError::MigrationMayRunOnRequestPath);
    assert_eq!(
        topology.status,
        StoragePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(topology.code, "storage_topology_invalid");
    assert_eq!(
        topology.message,
        "storage topology configuration is invalid"
    );

    let reservation =
        plan_atomic_reservation_error_response(&AtomicReservationPlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        });
    let reservation_error = AtomicReservationPlanError::TenantMismatch {
        key_tenant: tenant_id,
        request_tenant: other_tenant,
    };
    assert_eq!(
        reservation_error.to_string(),
        "atomic reservation request is invalid"
    );
    assert_eq!(
        AtomicReservationPlanError::ExpiryOverflow.to_string(),
        "atomic reservation request is invalid"
    );
    assert!(
        !reservation_error
            .to_string()
            .contains(&tenant_id.to_string())
    );
    assert!(
        !reservation_error
            .to_string()
            .contains(&other_tenant.to_string())
    );
    assert_eq!(reservation.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(reservation.code, "atomic_reservation_rejected");
    assert_eq!(reservation.message, "atomic reservation request is invalid");
    let reservation_debug = format!("{reservation_error:?}");
    assert!(!reservation_debug.contains(&tenant_id.to_string()));
    assert!(!reservation_debug.contains(&other_tenant.to_string()));

    let reconciliation_tenant_error = UsageReconciliationPlanError::TenantMismatch {
        key_tenant: tenant_id,
        record_tenant: other_tenant,
    };
    assert_eq!(
        reconciliation_tenant_error.to_string(),
        "usage reconciliation request is invalid"
    );
    assert!(
        !reconciliation_tenant_error
            .to_string()
            .contains(&tenant_id.to_string())
    );
    assert!(
        !reconciliation_tenant_error
            .to_string()
            .contains(&other_tenant.to_string())
    );
    let reconciliation = plan_usage_reconciliation_error_response(
        &UsageReconciliationPlanError::ActualExceedsReserved {
            reserved: UsageAmount::new(10, 100),
            actual: UsageAmount::new(11, 90),
        },
    );
    let reconciliation_usage_error = UsageReconciliationPlanError::ActualExceedsReserved {
        reserved: UsageAmount::new(10, 100),
        actual: UsageAmount::new(11, 90),
    };
    assert_eq!(
        reconciliation_usage_error.to_string(),
        "usage reconciliation request is invalid"
    );
    assert!(!reconciliation_usage_error.to_string().contains("10"));
    assert!(!reconciliation_usage_error.to_string().contains("100"));
    assert!(!reconciliation_usage_error.to_string().contains("11"));
    assert_eq!(reconciliation.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(reconciliation.code, "usage_reconciliation_rejected");
    assert_eq!(
        reconciliation.message,
        "usage reconciliation request is invalid"
    );
    let reconciliation_debug = format!(
        "{reconciliation_tenant_error:?} {reconciliation_usage_error:?} {:?} {:?}",
        UsageReconciliationPlanError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        },
        UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(1, 1),
            available: UsageAmount::ZERO,
        }
    );
    assert!(!reconciliation_debug.contains(&tenant_id.to_string()));
    assert!(!reconciliation_debug.contains(&other_tenant.to_string()));
    assert!(!reconciliation_debug.contains("100"));
    assert!(!reconciliation_debug.contains("18446744073709551615"));
    let reconciliation_overflow = plan_usage_reconciliation_error_response(
        &UsageReconciliationPlanError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        },
    );
    assert_eq!(
        UsageReconciliationPlanError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        }
        .to_string(),
        "usage reconciliation request is invalid"
    );
    assert_eq!(
        UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(1, 1),
            available: UsageAmount::ZERO,
        }
        .to_string(),
        "usage reconciliation request is invalid"
    );
    assert_eq!(
        reconciliation_overflow.status,
        StoragePlanErrorStatus::BadRequest
    );
    assert_eq!(
        reconciliation_overflow.code,
        "usage_reconciliation_rejected"
    );
    assert_eq!(
        reconciliation_overflow.message,
        "usage reconciliation request is invalid"
    );

    let recovery = plan_expired_reservation_recovery_error_response(
        &ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            record_tenant: other_tenant,
        },
    );
    assert_eq!(
        ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            record_tenant: other_tenant,
        }
        .to_string(),
        "expired reservation recovery request is invalid"
    );
    assert_eq!(
        ExpiredReservationRecoveryPlanError::NotExpired.to_string(),
        "expired reservation recovery request is invalid"
    );
    assert_eq!(
        ExpiredReservationRecoveryPlanError::InvalidRecord.to_string(),
        "expired reservation recovery request is invalid"
    );
    assert_eq!(recovery.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(recovery.code, "expired_reservation_recovery_rejected");
    let recovery_debug = format!(
        "{:?}",
        ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            record_tenant: other_tenant,
        }
    );
    assert!(!recovery_debug.contains(&tenant_id.to_string()));
    assert!(!recovery_debug.contains(&other_tenant.to_string()));

    let audit = plan_append_only_audit_error_response(&AppendOnlyAuditPlanError::TenantMismatch {
        key_tenant: tenant_id,
        event_tenant: other_tenant,
    });
    assert_eq!(
        AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant: tenant_id,
            event_tenant: other_tenant,
        }
        .to_string(),
        "append-only audit request is invalid"
    );
    assert_eq!(audit.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(audit.code, "append_only_audit_rejected");
    let audit_debug = format!(
        "{:?}",
        AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant: tenant_id,
            event_tenant: other_tenant,
        }
    );
    assert!(!audit_debug.contains(&tenant_id.to_string()));
    assert!(!audit_debug.contains(&other_tenant.to_string()));

    let retention_purge =
        plan_audit_retention_purge_error_response(&AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: tenant_id,
            batch_tenant: other_tenant,
        });
    assert_eq!(
        AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: tenant_id,
            batch_tenant: other_tenant,
        }
        .to_string(),
        "audit retention purge request is invalid"
    );
    assert_eq!(retention_purge.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(retention_purge.code, "audit_retention_purge_rejected");
    assert_eq!(
        retention_purge.message,
        "audit retention purge request is invalid"
    );
    let retention_purge_debug = format!(
        "{:?}",
        AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: tenant_id,
            batch_tenant: other_tenant,
        }
    );
    assert!(!retention_purge_debug.contains(&tenant_id.to_string()));
    assert!(!retention_purge_debug.contains(&other_tenant.to_string()));

    let audit_export =
        plan_audit_export_query_error_response(&AuditExportQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        });
    assert_eq!(
        AuditExportQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        }
        .to_string(),
        "audit export query request is invalid"
    );
    assert_eq!(audit_export.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(audit_export.code, "audit_export_query_rejected");
    assert_eq!(
        audit_export.message,
        "audit export query request is invalid"
    );
    let audit_export_debug = format!(
        "{:?}",
        AuditExportQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        }
    );
    assert!(!audit_export_debug.contains(&tenant_id.to_string()));
    assert!(!audit_export_debug.contains(&other_tenant.to_string()));

    let billing_query =
        plan_billing_ledger_query_error_response(&BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        });
    assert_eq!(
        BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        }
        .to_string(),
        "billing ledger query request is invalid"
    );
    assert_eq!(billing_query.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(billing_query.code, "billing_ledger_query_rejected");
    assert_eq!(
        billing_query.message,
        "billing ledger query request is invalid"
    );
    let billing_range_error = BillingLedgerQueryPlanError::StartAfterEnd {
        start_unix_ms: 9_999,
        end_unix_ms: 1,
    };
    assert_eq!(
        billing_range_error.to_string(),
        "billing ledger query request is invalid"
    );
    assert!(!billing_range_error.to_string().contains("9999"));
    let billing_range = plan_billing_ledger_query_error_response(&billing_range_error);
    assert_eq!(billing_range.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(billing_range.code, "billing_ledger_query_rejected");
    assert_eq!(
        billing_range.message,
        "billing ledger query request is invalid"
    );
    let billing_debug = format!(
        "{:?} {:?}",
        BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        },
        billing_range_error
    );
    assert!(!billing_debug.contains(&tenant_id.to_string()));
    assert!(!billing_debug.contains(&other_tenant.to_string()));
    assert!(!billing_debug.contains("9999"));
    let billing_limit_error = BillingLedgerPageLimit::new(Some(BillingLedgerPageLimit::MAX + 1))
        .expect_err("oversized billing ledger page limit should be rejected");
    assert_eq!(
        BillingLedgerPageLimit::new(Some(0))
            .expect_err("zero billing ledger page limit should be rejected")
            .to_string(),
        "billing ledger page limit is invalid"
    );
    assert_eq!(
        billing_limit_error.to_string(),
        "billing ledger page limit is invalid"
    );
    assert!(
        !billing_limit_error
            .to_string()
            .contains(&(BillingLedgerPageLimit::MAX + 1).to_string())
    );
    assert!(
        !format!("{billing_limit_error:?}")
            .contains(&(BillingLedgerPageLimit::MAX + 1).to_string())
    );

    let role_binding =
        plan_role_binding_mutation_error_response(&RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        });
    assert_eq!(
        RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "role-binding mutation request is invalid"
    );
    assert_eq!(role_binding.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(role_binding.code, "role_binding_mutation_rejected");
    assert_eq!(
        role_binding.message,
        "role-binding mutation request is invalid"
    );
    let role_binding_debug = format!(
        "{:?}",
        RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!role_binding_debug.contains(&tenant_id.to_string()));
    assert!(!role_binding_debug.contains(&other_tenant.to_string()));

    let service_identity = plan_service_identity_create_error_response(
        &ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    assert_eq!(
        ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "service identity create request is invalid"
    );
    assert_eq!(service_identity.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(service_identity.code, "service_identity_create_rejected");
    assert_eq!(
        service_identity.message,
        "service identity create request is invalid"
    );
    let service_identity_debug = format!(
        "{:?}",
        ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!service_identity_debug.contains(&tenant_id.to_string()));
    assert!(!service_identity_debug.contains(&other_tenant.to_string()));

    let user_lifecycle =
        plan_user_lifecycle_error_response(&UserLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        });
    assert_eq!(
        UserLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "user lifecycle request is invalid"
    );
    assert_eq!(
        UserLifecyclePlanError::EmptyExternalId.to_string(),
        "user lifecycle request is invalid"
    );
    assert_eq!(
        UserLifecyclePlanError::EmptyDisplayName.to_string(),
        "user lifecycle request is invalid"
    );
    assert_eq!(user_lifecycle.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(user_lifecycle.code, "user_lifecycle_rejected");
    assert_eq!(user_lifecycle.message, "user lifecycle request is invalid");
    let user_lifecycle_debug = format!(
        "{:?}",
        UserLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!user_lifecycle_debug.contains(&tenant_id.to_string()));
    assert!(!user_lifecycle_debug.contains(&other_tenant.to_string()));

    let budget_policy =
        plan_budget_policy_update_error_response(&BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        });
    assert_eq!(
        BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "budget policy update request is invalid"
    );
    assert_eq!(
        BudgetPolicyUpdatePlanError::EmptyBudgetScope.to_string(),
        "budget policy update request is invalid"
    );
    assert_eq!(budget_policy.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(budget_policy.code, "budget_policy_update_rejected");
    assert_eq!(
        budget_policy.message,
        "budget policy update request is invalid"
    );
    let budget_policy_debug = format!(
        "{:?}",
        BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!budget_policy_debug.contains(&tenant_id.to_string()));
    assert!(!budget_policy_debug.contains(&other_tenant.to_string()));

    let tenant_lifecycle =
        plan_tenant_lifecycle_error_response(&TenantLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        });
    assert_eq!(
        TenantLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "tenant lifecycle request is invalid"
    );
    assert_eq!(
        TenantLifecyclePlanError::EmptyDisplayName.to_string(),
        "tenant lifecycle request is invalid"
    );
    assert_eq!(tenant_lifecycle.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(tenant_lifecycle.code, "tenant_lifecycle_rejected");
    assert_eq!(
        tenant_lifecycle.message,
        "tenant lifecycle request is invalid"
    );
    let tenant_lifecycle_debug = format!(
        "{:?}",
        TenantLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!tenant_lifecycle_debug.contains(&tenant_id.to_string()));
    assert!(!tenant_lifecycle_debug.contains(&other_tenant.to_string()));

    let virtual_key = plan_virtual_key_secret_reference_error_response(
        &VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    assert_eq!(
        VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "virtual-key secret reference request is invalid"
    );
    assert_eq!(
        VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
            key_virtual_key_id: Some(VirtualKeyId::new()),
            request_virtual_key_id: VirtualKeyId::new(),
        }
        .to_string(),
        "virtual-key secret reference request is invalid"
    );
    assert_eq!(virtual_key.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(virtual_key.code, "virtual_key_secret_reference_rejected");
    assert_eq!(
        virtual_key.message,
        "virtual-key secret reference request is invalid"
    );
    let key_virtual_key_id = VirtualKeyId::new();
    let request_virtual_key_id = VirtualKeyId::new();
    let virtual_key_debug = format!(
        "{:?} {:?}",
        VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
        VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
            key_virtual_key_id: Some(key_virtual_key_id),
            request_virtual_key_id,
        }
    );
    assert!(!virtual_key_debug.contains(&tenant_id.to_string()));
    assert!(!virtual_key_debug.contains(&other_tenant.to_string()));
    assert!(!virtual_key_debug.contains(&key_virtual_key_id.to_string()));
    assert!(!virtual_key_debug.contains(&request_virtual_key_id.to_string()));

    let provider_credential = plan_provider_credential_reference_error_response(
        &ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    assert_eq!(
        ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        "provider credential reference request is invalid"
    );
    assert_eq!(
        provider_credential.status,
        StoragePlanErrorStatus::BadRequest
    );
    assert_eq!(
        provider_credential.code,
        "provider_credential_reference_rejected"
    );
    assert_eq!(
        provider_credential.message,
        "provider credential reference request is invalid"
    );
    let provider_credential_debug = format!(
        "{:?}",
        ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
    );
    assert!(!provider_credential_debug.contains(&tenant_id.to_string()));
    assert!(!provider_credential_debug.contains(&other_tenant.to_string()));

    let idempotency = plan_idempotency_pending_record_error_response(
        &IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        },
    );
    assert_eq!(
        IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        }
        .to_string(),
        "idempotency pending record request is invalid"
    );
    assert_eq!(idempotency.status, StoragePlanErrorStatus::BadRequest);
    assert_eq!(idempotency.code, "idempotency_pending_record_rejected");
    assert_eq!(
        idempotency.message,
        "idempotency pending record request is invalid"
    );

    let completed_idempotency = plan_idempotency_completed_record_error_response(
        &IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        },
    );
    assert_eq!(
        IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        }
        .to_string(),
        "idempotency completed record request is invalid"
    );
    assert_eq!(
        completed_idempotency.status,
        StoragePlanErrorStatus::BadRequest
    );
    assert_eq!(
        completed_idempotency.code,
        "idempotency_completed_record_rejected"
    );
    assert_eq!(
        completed_idempotency.message,
        "idempotency completed record request is invalid"
    );

    let idempotency_lookup = plan_idempotency_record_lookup_error_response(
        &IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        },
    );
    assert_eq!(
        IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: other_tenant,
        }
        .to_string(),
        "idempotency record lookup request is invalid"
    );
    assert_eq!(
        idempotency_lookup.status,
        StoragePlanErrorStatus::BadRequest
    );
    assert_eq!(
        idempotency_lookup.code,
        "idempotency_record_lookup_rejected"
    );
    assert_eq!(
        idempotency_lookup.message,
        "idempotency record lookup request is invalid"
    );

    let invalid_lookup_row = materialize_idempotency_record_lookup_row_error_response(
        &IdempotencyRecordLookupRowError::CompletedResponseMissing,
    );
    assert_eq!(
        IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: tenant_id,
            row_tenant: other_tenant,
        }
        .to_string(),
        "idempotency record lookup row is invalid"
    );
    for error in [
        IdempotencyRecordLookupRowError::KeyMismatch,
        IdempotencyRecordLookupRowError::InvalidRequestFingerprint,
        IdempotencyRecordLookupRowError::CompletedResponseMissing,
        IdempotencyRecordLookupRowError::CompletedTimestampMissing,
    ] {
        assert_eq!(
            error.to_string(),
            "idempotency record lookup row is invalid"
        );
    }
    assert_eq!(
        invalid_lookup_row.status,
        StoragePlanErrorStatus::BadRequest
    );
    assert_eq!(
        invalid_lookup_row.code,
        "idempotency_record_lookup_row_invalid"
    );
    assert_eq!(
        invalid_lookup_row.message,
        "idempotency record lookup row is invalid"
    );

    let accounting = plan_multi_replica_accounting_error_response(
        &MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas {
            actual: 1,
        },
    );
    assert_eq!(
        accounting.status,
        StoragePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(accounting.code, "multi_replica_accounting_invalid");
    let accounting_error =
        MultiReplicaAccountingConcurrencySpecError::EvidenceReplicaCountMismatch {
            expected: 3,
            actual: 1,
        };
    assert_eq!(
        accounting_error.to_string(),
        "multi-replica accounting configuration is invalid"
    );
    assert!(!accounting_error.to_string().contains("replicas"));
    assert!(!accounting_error.to_string().contains("3"));
    let accounting_debug = format!(
        "{:?} {:?} {:?} {:?}",
        MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas { actual: 1 },
        MultiReplicaAccountingConcurrencySpecError::EvidenceReplicaCountMismatch {
            expected: 3,
            actual: 1,
        },
        MultiReplicaAccountingConcurrencySpecError::EvidenceTopologyMismatch {
            expected: StorageTopology {
                durable_store: DurableStoreKind::Postgres,
                cache_store: Some(CacheStoreKind::Redis),
                migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
            },
            actual: StorageTopology {
                durable_store: DurableStoreKind::Sqlite,
                cache_store: None,
                migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
            },
        },
        MultiReplicaAccountingConcurrencySpecError::MissingEvidenceCheck {
            check: MultiReplicaAccountingCheck::NoDuplicateCharge,
        }
    );
    assert!(!accounting_debug.contains("actual: 1"));
    assert!(!accounting_debug.contains("expected: 3"));
    assert!(!accounting_debug.contains("Postgres"));
    assert!(!accounting_debug.contains("Redis"));
    assert!(!accounting_debug.contains("NoDuplicateCharge"));

    for response in [
        topology,
        reservation,
        reconciliation,
        recovery,
        audit,
        billing_query,
        billing_range,
        accounting,
    ] {
        assert!(!response.message.contains(&tenant_id.to_string()));
        assert!(!response.message.contains(&other_tenant.to_string()));
        assert!(!response.message.contains("tokens"));
        assert!(!response.message.contains("replicas"));
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
fn gateway_storage_topology_forbids_request_path_migrations() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: None,
        migration_policy: MigrationRuntimePolicy::ExternalMigratorOnly,
    };

    assert_eq!(
        topology.validate_for_gateway(),
        Err(StorageTopologyError::MigrationMayRunOnRequestPath)
    );

    let safe_topology = StorageTopology {
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        ..topology
    };
    assert_eq!(safe_topology.validate_for_gateway(), Ok(()));
}

#[test]
fn storage_topology_debug_output_is_stable_and_redacted() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::ExternalMigratorOnly,
    };

    let rendered = format!("{topology:?}");

    assert!(!rendered.contains("Postgres"));
    assert!(!rendered.contains("Redis"));
    assert!(!rendered.contains("ExternalMigratorOnly"));
}

#[test]
fn tenant_storage_key_includes_tenant_and_optional_virtual_key() {
    let tenant_id = TenantId::new();
    let virtual_key_id = VirtualKeyId::new();

    assert_eq!(TenantStorageKey::tenant(tenant_id).tenant_id, tenant_id);
    assert_eq!(
        TenantStorageKey::virtual_key(tenant_id, virtual_key_id).virtual_key_id,
        Some(virtual_key_id)
    );
}

#[test]
fn atomic_reservation_plan_carries_tenant_scoped_idempotency_and_reserved_ledger_event() {
    let tenant_id = TenantId::new();
    let command = command(tenant_id);
    let idempotency_key = command.idempotency_key.clone();
    let reservation_id = command.request.reservation_id;

    let plan = plan_atomic_reservation(command).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.idempotency_key, idempotency_key);
    assert_eq!(plan.reservation_record.tenant_id, tenant_id);
    assert_eq!(plan.reservation_record.reservation_id, reservation_id);
    assert_eq!(plan.ledger_event.kind, LedgerEventKind::Reserved);
    assert_eq!(plan.ledger_event.amount, UsageAmount::new(10, 100));
    assert_eq!(plan.ledger_event.idempotency_key().0, tenant_id);
}

#[test]
fn atomic_reservation_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = command(tenant_id);
    let call_id = command.request.call_id;
    let reservation_id = command.request.reservation_id;
    let idempotency_key = command.idempotency_key.as_str().to_owned();
    let plan = plan_atomic_reservation(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&call_id.to_string()));
    assert!(!rendered.contains(&reservation_id.to_string()));
    assert!(!rendered.contains(&idempotency_key));
    assert!(!rendered.contains("60000"));
}

#[test]
fn atomic_reservation_plan_rejects_cross_tenant_storage_key() {
    let tenant_id = TenantId::new();
    let mut command = command(tenant_id);
    let key_tenant = TenantId::new();
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_atomic_reservation(command),
        Err(AtomicReservationPlanError::TenantMismatch {
            key_tenant,
            request_tenant: tenant_id,
        })
    );
}

#[test]
fn atomic_reservation_plan_rejects_expiry_overflow() {
    let tenant_id = TenantId::new();
    let mut command = command(tenant_id);
    command.created_at_unix_ms = u64::MAX;
    command.ttl_ms = 1;

    assert_eq!(
        plan_atomic_reservation(command),
        Err(AtomicReservationPlanError::ExpiryOverflow)
    );
}

#[test]
fn append_only_audit_plan_uses_tenant_scoped_hash_chain_envelope() {
    let tenant_id = TenantId::new();
    let command = audit_command(tenant_id);
    let expected_previous = command.previous_digest.clone();
    let expected_current = command.event_digest.clone();

    let plan = plan_append_only_audit(command).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.mode, AuditAppendMode::HashChain);
    assert_eq!(plan.envelope.event.tenant_id, tenant_id);
    assert_eq!(plan.envelope.previous_digest, expected_previous);
    assert_eq!(plan.envelope.event_digest, expected_current);
    assert_eq!(
        plan.envelope
            .verify_chain_link(plan.envelope.previous_digest.as_ref()),
        Ok(())
    );
}

#[test]
fn append_only_audit_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = audit_command(tenant_id);
    let plan = plan_append_only_audit(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("sha256:previous"));
    assert!(!rendered.contains("sha256:current"));
    assert!(!rendered.contains("policy-1"));
    assert!(rendered.contains("HashChain"));
}

#[test]
fn append_only_audit_plan_rejects_cross_tenant_event_storage_key() {
    let event_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = audit_command(event_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_append_only_audit(command),
        Err(AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant,
            event_tenant,
        })
    );
}

#[test]
fn audit_retention_purge_plan_uses_tenant_scoped_batch() {
    let tenant_id = TenantId::new();
    let command = retention_purge_command(tenant_id);
    let expected_event_ids = command.batch.event_ids().collect::<Vec<_>>();

    let plan = plan_audit_retention_purge(command).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.batch.tenant_id, tenant_id);
    assert_eq!(
        plan.batch.event_ids().collect::<Vec<_>>(),
        expected_event_ids
    );
}

#[test]
fn audit_retention_purge_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = retention_purge_command(tenant_id);
    let event_ids = command.batch.event_ids().collect::<Vec<_>>();
    let plan = plan_audit_retention_purge(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    for event_id in event_ids {
        assert!(!rendered.contains(&event_id.to_string()));
    }
}

#[test]
fn audit_retention_purge_plan_rejects_cross_tenant_storage_key() {
    let batch_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = retention_purge_command(batch_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_audit_retention_purge(command),
        Err(AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant,
            batch_tenant,
        })
    );
}

#[test]
fn audit_export_query_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = audit_export_query_command(tenant_id);

    let plan = plan_audit_export_query(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.sort_order, AuditSortOrder::OccurredAtDesc);
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.export.format, AuditExportFormat::Jsonl);
}

#[test]
fn audit_export_query_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = audit_export_query_command(tenant_id);
    let plan = plan_audit_export_query(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("1800000000000"));
    assert!(!rendered.contains("1800000999999"));
    assert!(!rendered.contains("page_limit: 250"));
    assert!(rendered.contains("OccurredAtDesc"));
}

#[test]
fn audit_export_query_plan_rejects_cross_tenant_storage_key() {
    let query_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = audit_export_query_command(query_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_audit_export_query(command),
        Err(AuditExportQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        })
    );
}

#[test]
fn billing_ledger_query_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = billing_ledger_query_command(tenant_id);

    let plan = plan_billing_ledger_query(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.start_unix_ms, Some(1_800_000_000_000));
    assert_eq!(plan.end_unix_ms, Some(1_800_000_999_999));
    assert_eq!(plan.page_limit, 250);
    assert_eq!(plan.sort_order, BillingLedgerSortOrder::OccurredAtDesc);
}

#[test]
fn billing_ledger_query_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = billing_ledger_query_command(tenant_id);
    let plan = plan_billing_ledger_query(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("1800000000000"));
    assert!(!rendered.contains("1800000999999"));
    assert!(!rendered.contains("page_limit: 250"));
    assert!(rendered.contains("OccurredAtDesc"));
}

#[test]
fn billing_ledger_query_plan_rejects_cross_tenant_storage_key_and_bad_range() {
    let query_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = billing_ledger_query_command(query_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_billing_ledger_query(command),
        Err(BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        })
    );

    let tenant_id = TenantId::new();
    let mut bad_range = billing_ledger_query_command(tenant_id);
    bad_range.start_unix_ms = Some(20);
    bad_range.end_unix_ms = Some(10);

    assert_eq!(
        plan_billing_ledger_query(bad_range),
        Err(BillingLedgerQueryPlanError::StartAfterEnd {
            start_unix_ms: 20,
            end_unix_ms: 10,
        })
    );
}

#[test]
fn role_binding_mutation_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = role_binding_command(tenant_id, RoleBindingMutationKind::Grant);
    let role_binding_id = command.role_binding_id;
    let principal_id = command.principal_id;

    let plan = plan_role_binding_mutation(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.role_binding_id, role_binding_id);
    assert_eq!(plan.principal_id, principal_id);
    assert_eq!(plan.role, Role::Operator);
    assert_eq!(plan.kind, RoleBindingMutationKind::Grant);
    assert_eq!(plan.occurred_at_unix_ms, 1_800_000_002_000);
}

#[test]
fn role_binding_mutation_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = role_binding_command(tenant_id, RoleBindingMutationKind::Grant);
    let role_binding_id = command.role_binding_id;
    let principal_id = command.principal_id;
    let plan = plan_role_binding_mutation(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&role_binding_id.to_string()));
    assert!(!rendered.contains(&principal_id.to_string()));
    assert!(!rendered.contains("1800000002000"));
    assert!(rendered.contains("Operator"));
    assert!(rendered.contains("Grant"));
}

#[test]
fn role_binding_mutation_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = role_binding_command(request_tenant, RoleBindingMutationKind::Revoke);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_role_binding_mutation(command),
        Err(RoleBindingMutationPlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn service_identity_create_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = service_identity_command(tenant_id);
    let principal_id = command.principal_id;

    let plan = plan_service_identity_create(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.principal_id, principal_id);
    assert_eq!(plan.display_name, "svc-ingest");
    assert_eq!(plan.created_at_unix_ms, 1_800_000_002_250);
}

#[test]
fn service_identity_create_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = service_identity_command(tenant_id);
    let principal_id = command.principal_id;
    let plan = plan_service_identity_create(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&principal_id.to_string()));
    assert!(!rendered.contains("svc-ingest"));
    assert!(!rendered.contains("1800000002250"));
}

#[test]
fn service_identity_create_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = service_identity_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_service_identity_create(command),
        Err(ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn user_lifecycle_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = user_lifecycle_command(tenant_id, UserLifecycleKind::Create);
    let principal_id = command.principal_id;

    let plan = plan_user_lifecycle(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.principal_id, principal_id);
    assert_eq!(plan.external_id, "scim-user-1");
    assert_eq!(plan.display_name, "Ada Lovelace");
    assert_eq!(plan.kind, UserLifecycleKind::Create);
    assert_eq!(plan.occurred_at_unix_ms, 1_800_000_002_400);
}

#[test]
fn user_lifecycle_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = user_lifecycle_command(tenant_id, UserLifecycleKind::Create);
    let principal_id = command.principal_id;
    let plan = plan_user_lifecycle(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&principal_id.to_string()));
    assert!(!rendered.contains("scim-user-1"));
    assert!(!rendered.contains("Ada Lovelace"));
    assert!(!rendered.contains("1800000002400"));
    assert!(rendered.contains("Create"));
}

#[test]
fn user_lifecycle_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = user_lifecycle_command(request_tenant, UserLifecycleKind::Update);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_user_lifecycle(command),
        Err(UserLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn user_lifecycle_plan_rejects_empty_identity_fields() {
    let tenant_id = TenantId::new();
    let mut missing_external = user_lifecycle_command(tenant_id, UserLifecycleKind::Delete);
    missing_external.external_id = " ".to_string();
    assert_eq!(
        plan_user_lifecycle(missing_external),
        Err(UserLifecyclePlanError::EmptyExternalId)
    );

    let mut missing_display = user_lifecycle_command(tenant_id, UserLifecycleKind::Create);
    missing_display.display_name = " ".to_string();
    assert_eq!(
        plan_user_lifecycle(missing_display),
        Err(UserLifecyclePlanError::EmptyDisplayName)
    );

    let mut delete_without_display = user_lifecycle_command(tenant_id, UserLifecycleKind::Delete);
    delete_without_display.display_name = " ".to_string();
    assert!(plan_user_lifecycle(delete_without_display).is_ok());
}

#[test]
fn budget_policy_update_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = budget_policy_command(tenant_id);

    let plan = plan_budget_policy_update(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.budget_scope, "tenant-default");
    assert_eq!(plan.limit.max, UsageAmount::new(10_000, 1_000_000));
    assert_eq!(plan.updated_at_unix_ms, 1_800_000_002_750);
}

#[test]
fn budget_policy_update_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = budget_policy_command(tenant_id);
    let plan = plan_budget_policy_update(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("tenant-default"));
    assert!(!rendered.contains("10000"));
    assert!(!rendered.contains("1000000"));
    assert!(!rendered.contains("1800000002750"));
}

#[test]
fn budget_policy_update_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = budget_policy_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_budget_policy_update(command),
        Err(BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn budget_policy_update_plan_rejects_empty_scope() {
    let tenant_id = TenantId::new();
    let mut command = budget_policy_command(tenant_id);
    command.budget_scope = " ".to_string();

    assert_eq!(
        plan_budget_policy_update(command),
        Err(BudgetPolicyUpdatePlanError::EmptyBudgetScope)
    );
}

#[test]
fn tenant_lifecycle_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = tenant_lifecycle_command(tenant_id, TenantLifecycleKind::Create);

    let plan = plan_tenant_lifecycle(command).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.display_name, "acme-prod");
    assert_eq!(plan.kind, TenantLifecycleKind::Create);
    assert_eq!(plan.occurred_at_unix_ms, 1_800_000_002_900);
}

#[test]
fn tenant_lifecycle_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = tenant_lifecycle_command(tenant_id, TenantLifecycleKind::Create);
    let plan = plan_tenant_lifecycle(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("acme-prod"));
    assert!(!rendered.contains("1800000002900"));
    assert!(rendered.contains("Create"));
}

#[test]
fn tenant_lifecycle_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = tenant_lifecycle_command(request_tenant, TenantLifecycleKind::Update);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_tenant_lifecycle(command),
        Err(TenantLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn tenant_lifecycle_plan_rejects_empty_display_name() {
    let tenant_id = TenantId::new();
    let mut command = tenant_lifecycle_command(tenant_id, TenantLifecycleKind::Create);
    command.display_name = " ".to_string();

    assert_eq!(
        plan_tenant_lifecycle(command),
        Err(TenantLifecyclePlanError::EmptyDisplayName)
    );
}

#[test]
fn virtual_key_secret_reference_plan_is_tenant_scoped_and_keeps_secret_ref() {
    let tenant_id = TenantId::new();
    let command = virtual_key_secret_command(tenant_id, VirtualKeySecretReferenceKind::Create);
    let virtual_key_id = command.virtual_key_id;
    let principal_id = command.principal_id;

    let plan = plan_virtual_key_secret_reference(command.clone()).unwrap();

    assert_eq!(
        plan.storage_key,
        TenantStorageKey::virtual_key(tenant_id, virtual_key_id)
    );
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.virtual_key_id, virtual_key_id);
    assert_eq!(plan.principal_id, principal_id);
    assert_eq!(plan.display_name, "prod-api");
    assert_eq!(plan.secret_ref.provider(), "vault");
    assert_eq!(plan.secret_ref.name(), "virtual-keys/prod-api");
    assert_eq!(plan.secret_ref.version(), Some("v4"));
    assert_eq!(plan.kind, VirtualKeySecretReferenceKind::Create);
    assert_eq!(plan.occurred_at_unix_ms, 1_800_000_002_500);
    let rendered = format!("{command:?} {plan:?}");
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&virtual_key_id.to_string()));
    assert!(!rendered.contains(&principal_id.to_string()));
    assert!(!rendered.contains("prod-api"));
    assert!(!rendered.contains("virtual-keys/prod-api"));
    assert!(!rendered.contains("vault"));
    assert!(!rendered.contains("v4"));
    assert!(!rendered.contains("1800000002500"));
    assert!(rendered.contains("Create"));
}

#[test]
fn virtual_key_secret_reference_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command =
        virtual_key_secret_command(request_tenant, VirtualKeySecretReferenceKind::Rotate);
    command.storage_key = TenantStorageKey::virtual_key(key_tenant, command.virtual_key_id);

    assert_eq!(
        plan_virtual_key_secret_reference(command),
        Err(VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn virtual_key_secret_reference_plan_rejects_mismatched_virtual_key_storage_key() {
    let tenant_id = TenantId::new();
    let key_virtual_key_id = VirtualKeyId::new();
    let mut command = virtual_key_secret_command(tenant_id, VirtualKeySecretReferenceKind::Rotate);
    let request_virtual_key_id = command.virtual_key_id;
    command.storage_key = TenantStorageKey::virtual_key(tenant_id, key_virtual_key_id);

    assert_eq!(
        plan_virtual_key_secret_reference(command),
        Err(VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
            key_virtual_key_id: Some(key_virtual_key_id),
            request_virtual_key_id,
        })
    );
}

#[test]
fn provider_credential_reference_plan_is_tenant_scoped_and_keeps_secret_ref() {
    let tenant_id = TenantId::new();
    let command = provider_credential_command(tenant_id);
    let provider_credential_id = command.provider_credential_id;

    let plan = plan_provider_credential_reference(command.clone()).unwrap();

    assert_eq!(plan.storage_key, TenantStorageKey::tenant(tenant_id));
    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.provider_credential_id, provider_credential_id);
    assert_eq!(plan.provider_name, "openai");
    assert_eq!(plan.secret_ref.provider(), "vault");
    assert_eq!(plan.secret_ref.name(), "providers/openai");
    assert_eq!(plan.secret_ref.version(), Some("v3"));
    assert_eq!(plan.rotated_at_unix_ms, 1_800_000_003_000);
    let rendered = format!("{command:?} {plan:?}");
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&provider_credential_id.to_string()));
    assert!(!rendered.contains("openai"));
    assert!(!rendered.contains("vault"));
    assert!(!rendered.contains("providers/openai"));
    assert!(!rendered.contains("v3"));
    assert!(!rendered.contains("1800000003000"));
}

#[test]
fn provider_credential_reference_plan_rejects_cross_tenant_storage_key() {
    let request_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = provider_credential_command(request_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_provider_credential_reference(command),
        Err(ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        })
    );
}

#[test]
fn idempotency_pending_record_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = idempotency_pending_command(tenant_id);
    let expected_operation = command.operation.clone();

    let plan = plan_idempotency_pending_record(command.clone()).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(
        plan.entry,
        IdempotencyEntry::<()>::pending(expected_operation, 1_800_000_000_000)
    );
    let rendered = format!("{command:?} {plan:?}");
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("admin-mutation-1"));
    assert!(!rendered.contains("sha256:admin-mutation"));
    assert!(!rendered.contains("1800000000000"));
}

#[test]
fn idempotency_pending_record_plan_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_pending_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_idempotency_pending_record(command),
        Err(IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant,
            operation_tenant,
        })
    );
}

#[test]
fn idempotency_completed_record_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = idempotency_completed_command(tenant_id);
    let expected_operation = command.operation.clone();
    let expected_response = command.response_body.clone();

    let plan = plan_idempotency_completed_record(command.clone()).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.completed_at_unix_ms, 1_800_000_001_000);
    assert_eq!(
        plan.entry,
        IdempotencyEntry::completed(IdempotencyRecord {
            operation: expected_operation,
            response: expected_response,
        })
    );
    let rendered = format!("{command:?} {plan:?}");
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("admin-mutation-1"));
    assert!(!rendered.contains("sha256:admin-mutation"));
    assert!(!rendered.contains("1800000001000"));
    assert!(!rendered.contains(r#"{"status":"ok"}"#));
    assert!(!rendered.contains("ok"));
}

#[test]
fn idempotency_completed_record_plan_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_completed_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_idempotency_completed_record(command),
        Err(IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant,
            operation_tenant,
        })
    );
}

#[test]
fn idempotency_record_lookup_plan_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let command = idempotency_lookup_command(tenant_id);
    let expected_operation = command.operation.clone();

    let plan = plan_idempotency_record_lookup(command.clone()).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.operation, expected_operation);
    let rendered = format!("{command:?} {plan:?}");
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("admin-mutation-1"));
    assert!(!rendered.contains("sha256:admin-mutation"));
}

#[test]
fn idempotency_record_lookup_plan_rejects_cross_tenant_storage_key() {
    let operation_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = idempotency_lookup_command(operation_tenant);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_idempotency_record_lookup(command),
        Err(IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant,
            operation_tenant,
        })
    );
}

#[test]
fn idempotency_lookup_row_materializes_pending_entry() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;

    let entry = materialize_idempotency_record_lookup_row(
        &operation,
        idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Pending),
    )
    .unwrap();

    assert_eq!(
        entry,
        IdempotencyEntry::pending(operation, 1_800_000_000_000)
    );
}

#[test]
fn idempotency_lookup_row_materializes_completed_entry() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;
    let row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Completed);
    let rendered = format!("{row:?}");

    let entry = materialize_idempotency_record_lookup_row(&operation, row).unwrap();

    assert_eq!(
        entry,
        IdempotencyEntry::completed(IdempotencyRecord {
            operation,
            response: br#"{"status":"ok"}"#.to_vec(),
        })
    );
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains("admin-mutation-1"));
    assert!(!rendered.contains("sha256:admin-mutation"));
    assert!(!rendered.contains("1800000000000"));
    assert!(!rendered.contains("1800000001000"));
    assert!(!rendered.contains(r#"{"status":"ok"}"#));
}

#[test]
fn idempotency_lookup_row_preserves_fingerprint_for_domain_conflict() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;
    let mut row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Pending);
    row.request_fingerprint = "sha256:different-admin-mutation".to_string();

    let entry = materialize_idempotency_record_lookup_row(&operation, row).unwrap();

    assert_eq!(
        entry.operation().request_fingerprint,
        "sha256:different-admin-mutation"
    );
}

#[test]
fn idempotency_lookup_row_rejects_wrong_tenant_or_key() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;
    let other_tenant = TenantId::new();

    assert_eq!(
        materialize_idempotency_record_lookup_row(
            &operation,
            idempotency_lookup_row(other_tenant, IdempotencyRecordLookupRowStatus::Pending),
        ),
        Err(IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: tenant_id,
            row_tenant: other_tenant,
        })
    );

    let mut row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Pending);
    row.idempotency_key = IdempotencyKey::new("admin-mutation-2").unwrap();
    assert_eq!(
        materialize_idempotency_record_lookup_row(&operation, row),
        Err(IdempotencyRecordLookupRowError::KeyMismatch)
    );
}

#[test]
fn idempotency_lookup_row_rejects_completed_without_response_body() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;
    let mut row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Completed);
    row.response_body = None;

    assert_eq!(
        materialize_idempotency_record_lookup_row(&operation, row),
        Err(IdempotencyRecordLookupRowError::CompletedResponseMissing)
    );
}

#[test]
fn idempotency_lookup_row_rejects_completed_without_completion_timestamp() {
    let tenant_id = TenantId::new();
    let operation = idempotency_lookup_command(tenant_id).operation;
    let mut row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Completed);
    row.completed_at_unix_ms = None;

    assert_eq!(
        materialize_idempotency_record_lookup_row(&operation, row),
        Err(IdempotencyRecordLookupRowError::CompletedTimestampMissing)
    );
}

#[test]
fn usage_reconciliation_plan_commits_actual_and_releases_reserved_delta() {
    let tenant_id = TenantId::new();
    let command = reconciliation_command(
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    );

    let plan = plan_usage_reconciliation(command).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.updated_snapshot.reserved, UsageAmount::ZERO);
    assert_eq!(plan.updated_snapshot.committed, UsageAmount::new(41, 410));
    assert_eq!(
        plan.reconciliation.reason,
        ReservationReconciliationReason::Completed
    );
    assert_eq!(plan.ledger_events.len(), 2);
    assert_eq!(plan.ledger_events[0].kind, LedgerEventKind::Committed);
    assert_eq!(plan.ledger_events[0].amount, UsageAmount::new(40, 400));
    assert_eq!(plan.ledger_events[1].kind, LedgerEventKind::Released);
    assert_eq!(plan.ledger_events[1].amount, UsageAmount::new(60, 600));
    assert_eq!(plan.ledger_events[0].idempotency_key().0, tenant_id);
    assert_ne!(
        plan.ledger_events[0].idempotency_key(),
        plan.ledger_events[1].idempotency_key()
    );
}

#[test]
fn usage_reconciliation_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = reconciliation_command(
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    );
    let call_id = command.record.call_id;
    let reservation_id = command.record.reservation_id;
    let plan = plan_usage_reconciliation(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&call_id.to_string()));
    assert!(!rendered.contains(&reservation_id.to_string()));
    assert!(!rendered.contains("1000"));
    assert!(!rendered.contains("400"));
}

#[test]
fn usage_reconciliation_plan_omits_release_event_when_actual_equals_reserved() {
    let tenant_id = TenantId::new();
    let plan = plan_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(10, 100),
    ))
    .unwrap();

    assert_eq!(plan.ledger_events.len(), 1);
    assert_eq!(plan.ledger_events[0].kind, LedgerEventKind::Committed);
    assert_eq!(plan.reconciliation.released_event, None);
}

#[test]
fn usage_reconciliation_plan_rejects_cross_tenant_record_storage_key() {
    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = reconciliation_command(
        record_tenant,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_usage_reconciliation(command),
        Err(UsageReconciliationPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        })
    );
}

#[test]
fn usage_reconciliation_plan_commits_actual_usage_above_reserved() {
    let tenant_id = TenantId::new();

    let plan = plan_usage_reconciliation(reconciliation_command(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(11, 110),
    ))
    .unwrap();

    assert_eq!(plan.updated_snapshot.reserved, UsageAmount::ZERO);
    assert_eq!(plan.reconciliation.commit.actual, UsageAmount::new(11, 110));
    assert_eq!(plan.ledger_events.len(), 1);
}

#[test]
fn usage_reconciliation_plan_rejects_committed_usage_overflow() {
    let tenant_id = TenantId::new();
    let mut command = reconciliation_command(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    command.snapshot.committed = UsageAmount::new(u64::MAX, 10);

    assert_eq!(
        plan_usage_reconciliation(command),
        Err(UsageReconciliationPlanError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 10),
            actual: UsageAmount::new(1, 10),
        })
    );
}

#[test]
fn expired_reservation_recovery_plan_releases_reserved_without_committing_usage() {
    let tenant_id = TenantId::new();
    let plan = plan_expired_reservation_recovery(expired_recovery_command(
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.updated_snapshot.reserved, UsageAmount::ZERO);
    assert_eq!(plan.updated_snapshot.committed, UsageAmount::new(1, 10));
    assert_eq!(plan.ledger_event.kind, LedgerEventKind::Released);
    assert_eq!(plan.ledger_event.amount, UsageAmount::new(10, 100));
    assert_eq!(plan.ledger_event.idempotency_key().0, tenant_id);
}

#[test]
fn expired_reservation_recovery_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let command = expired_recovery_command(tenant_id, UsageAmount::new(10, 100), 1_500);
    let call_id = command.record.call_id;
    let reservation_id = command.record.reservation_id;
    let plan = plan_expired_reservation_recovery(command.clone()).unwrap();

    let rendered = format!("{command:?} {plan:?}");

    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&call_id.to_string()));
    assert!(!rendered.contains(&reservation_id.to_string()));
    assert!(!rendered.contains("1500"));
    assert!(!rendered.contains("100"));
}

#[test]
fn expired_reservation_recovery_plan_rejects_not_expired_record() {
    let tenant_id = TenantId::new();

    assert_eq!(
        plan_expired_reservation_recovery(expired_recovery_command(
            tenant_id,
            UsageAmount::new(10, 100),
            1_499,
        )),
        Err(ExpiredReservationRecoveryPlanError::NotExpired)
    );
}

#[test]
fn expired_reservation_recovery_plan_rejects_cross_tenant_storage_key() {
    let record_tenant = TenantId::new();
    let key_tenant = TenantId::new();
    let mut command = expired_recovery_command(record_tenant, UsageAmount::new(10, 100), 1_500);
    command.storage_key = TenantStorageKey::tenant(key_tenant);

    assert_eq!(
        plan_expired_reservation_recovery(command),
        Err(ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        })
    );
}

#[test]
fn multi_replica_accounting_concurrency_spec_requires_shared_postgres_and_redis() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
    };

    let spec = plan_multi_replica_accounting_concurrency_spec(topology, 2).unwrap();

    assert_eq!(spec.topology, topology);
    assert_eq!(spec.gateway_replica_count, 2);
    assert_eq!(
        spec.checks,
        vec![
            MultiReplicaAccountingCheck::NoLostUpdate,
            MultiReplicaAccountingCheck::NoDuplicateCharge,
            MultiReplicaAccountingCheck::NoDroppedLedgerEvent,
            MultiReplicaAccountingCheck::NoRequestIdCollision,
            MultiReplicaAccountingCheck::NoLimitOvershoot,
        ]
    );
}

fn complete_multi_replica_accounting_evidence(
    topology: StorageTopology,
    gateway_replica_count: u16,
) -> MultiReplicaAccountingEvidence {
    MultiReplicaAccountingEvidence {
        topology,
        gateway_replica_count,
        passed_checks: vec![
            MultiReplicaAccountingCheck::NoLostUpdate,
            MultiReplicaAccountingCheck::NoDuplicateCharge,
            MultiReplicaAccountingCheck::NoDroppedLedgerEvent,
            MultiReplicaAccountingCheck::NoRequestIdCollision,
            MultiReplicaAccountingCheck::NoLimitOvershoot,
        ],
        documented_limit_overshoot_tolerance: true,
    }
}

#[test]
fn multi_replica_accounting_verification_requires_complete_evidence() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
    };
    let spec = plan_multi_replica_accounting_concurrency_spec(topology, 2).unwrap();

    let plan = plan_multi_replica_accounting_verification(
        &spec,
        complete_multi_replica_accounting_evidence(topology, 2),
    )
    .unwrap();

    assert_eq!(plan.topology, topology);
    assert_eq!(plan.gateway_replica_count, 2);
    assert_eq!(plan.verified_checks, spec.checks);
}

#[test]
fn multi_replica_accounting_debug_output_is_stable_and_redacted() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
    };
    let spec = plan_multi_replica_accounting_concurrency_spec(topology, 2).unwrap();
    let evidence = complete_multi_replica_accounting_evidence(topology, 2);
    let plan = plan_multi_replica_accounting_verification(&spec, evidence.clone()).unwrap();

    let rendered = format!("{spec:?} {evidence:?} {plan:?}");

    assert!(!rendered.contains("Postgres"));
    assert!(!rendered.contains("Redis"));
    assert!(!rendered.contains("NoLostUpdate"));
    assert!(!rendered.contains("gateway_replica_count: 2"));
}

#[test]
fn multi_replica_accounting_verification_rejects_weak_evidence() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
    };
    let spec = plan_multi_replica_accounting_concurrency_spec(topology, 2).unwrap();

    assert_eq!(
        plan_multi_replica_accounting_verification(
            &spec,
            complete_multi_replica_accounting_evidence(
                StorageTopology {
                    cache_store: None,
                    ..topology
                },
                2,
            ),
        ),
        Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceTopologyMismatch {
                expected: topology,
                actual: StorageTopology {
                    cache_store: None,
                    ..topology
                },
            }
        )
    );

    assert_eq!(
        plan_multi_replica_accounting_verification(
            &spec,
            complete_multi_replica_accounting_evidence(topology, 1),
        ),
        Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceReplicaCountMismatch {
                expected: 2,
                actual: 1,
            }
        )
    );

    let mut missing_check = complete_multi_replica_accounting_evidence(topology, 2);
    missing_check
        .passed_checks
        .retain(|check| *check != MultiReplicaAccountingCheck::NoDuplicateCharge);
    assert_eq!(
        plan_multi_replica_accounting_verification(&spec, missing_check),
        Err(
            MultiReplicaAccountingConcurrencySpecError::MissingEvidenceCheck {
                check: MultiReplicaAccountingCheck::NoDuplicateCharge,
            }
        )
    );

    let mut missing_tolerance = complete_multi_replica_accounting_evidence(topology, 2);
    missing_tolerance.documented_limit_overshoot_tolerance = false;
    assert_eq!(
        plan_multi_replica_accounting_verification(&spec, missing_tolerance),
        Err(MultiReplicaAccountingConcurrencySpecError::MissingLimitOvershootTolerance)
    );
}

#[test]
fn multi_replica_accounting_concurrency_spec_rejects_non_enterprise_topologies() {
    let topology = StorageTopology {
        durable_store: DurableStoreKind::Postgres,
        cache_store: Some(CacheStoreKind::Redis),
        migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
    };

    assert_eq!(
        plan_multi_replica_accounting_concurrency_spec(topology, 1),
        Err(
            MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas {
                actual: 1,
            }
        )
    );

    assert_eq!(
        plan_multi_replica_accounting_concurrency_spec(
            StorageTopology {
                durable_store: DurableStoreKind::Sqlite,
                ..topology
            },
            2,
        ),
        Err(MultiReplicaAccountingConcurrencySpecError::RequiresPostgresDurableStore)
    );

    assert_eq!(
        plan_multi_replica_accounting_concurrency_spec(
            StorageTopology {
                cache_store: None,
                ..topology
            },
            2,
        ),
        Err(MultiReplicaAccountingConcurrencySpecError::RequiresRedisCoordination)
    );
}

#[test]
fn tenant_mismatch_display_errors_are_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let operation = IdempotentOperation::new(
        other_tenant,
        IdempotencyKey::new("admin-mutation-display-redaction").unwrap(),
        "http:post:path:/admin/keys:body:sha256:redacted",
    )
    .unwrap();

    let rendered = [
        BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        }
        .to_string(),
        ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            record_tenant: other_tenant,
        }
        .to_string(),
        AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant: tenant_id,
            event_tenant: other_tenant,
        }
        .to_string(),
        AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: tenant_id,
            batch_tenant: other_tenant,
        }
        .to_string(),
        AuditExportQueryPlanError::TenantMismatch {
            key_tenant: tenant_id,
            query_tenant: other_tenant,
        }
        .to_string(),
        RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        UserLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        TenantLifecyclePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        }
        .to_string(),
        IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        }
        .to_string(),
        IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        }
        .to_string(),
        IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        }
        .to_string(),
        IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: tenant_id,
            row_tenant: other_tenant,
        }
        .to_string(),
    ];

    for message in rendered {
        assert!(!message.contains(&tenant_id.to_string()));
        assert!(!message.contains(&other_tenant.to_string()));
    }

    let debug_rendered = format!(
        "{:?} {:?} {:?} {:?}",
        IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        },
        IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        },
        IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: tenant_id,
            operation_tenant: operation.tenant_id,
        },
        IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: tenant_id,
            row_tenant: other_tenant,
        }
    );
    assert!(!debug_rendered.contains(&tenant_id.to_string()));
    assert!(!debug_rendered.contains(&other_tenant.to_string()));
}
