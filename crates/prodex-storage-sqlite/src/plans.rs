use super::*;

pub fn plan_sqlite_atomic_reservation(
    command: AtomicReservationCommand,
) -> Result<SqliteAtomicReservationSqlPlan, SqliteStoragePlanError> {
    validate_sqlite_reservation_inputs(
        command.storage_key,
        &command.idempotency_key,
        command.limit,
        command.request,
    )?;
    Ok(SqliteAtomicReservationSqlPlan {
        tenant_id: command.request.tenant_id,
        storage_key: command.storage_key,
        idempotency_key: command.idempotency_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_STATEMENT,
            SQLITE_ATOMIC_RESERVATION_STATEMENT,
            SQLITE_COMMIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<SqliteAppendOnlyAuditSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_append_only_audit(command).map_err(|error| match error {
        prodex_storage::AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant,
            event_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        },
    })?;
    Ok(SqliteAppendOnlyAuditSqlPlan {
        tenant_id: plan.envelope.event.tenant_id,
        storage_key: plan.storage_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_AUDIT_STATEMENT,
            SQLITE_APPEND_AUDIT_STATEMENT,
            SQLITE_COMMIT_AUDIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<SqliteAuditRetentionPurgeSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_audit_retention_purge(command).map_err(|error| match error {
            prodex_storage::AuditRetentionPurgePlanError::TenantMismatch {
                key_tenant,
                batch_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: batch_tenant,
            },
        })?;
    Ok(SqliteAuditRetentionPurgeSqlPlan {
        tenant_id: plan.batch.tenant_id,
        storage_key: plan.storage_key,
        event_count: plan.batch.len(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_AUDIT_RETENTION_PURGE_STATEMENT,
            SQLITE_PURGE_AUDIT_RETENTION_STATEMENT,
            SQLITE_COMMIT_AUDIT_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<SqliteAuditExportQuerySqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_audit_export_query(command).map_err(|error| match error {
        prodex_storage::AuditExportQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
    })?;
    let statement = match plan.sort_order {
        AuditSortOrder::OccurredAtAsc => SQLITE_QUERY_AUDIT_EXPORT_ASC_STATEMENT,
        AuditSortOrder::OccurredAtDesc => SQLITE_QUERY_AUDIT_EXPORT_DESC_STATEMENT,
    };
    Ok(SqliteAuditExportQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: vec![statement],
    })
}

pub fn plan_sqlite_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<SqliteRoleBindingMutationSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_role_binding_mutation(command).map_err(|error| match error {
            prodex_storage::RoleBindingMutationPlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    let operation = match plan.kind {
        RoleBindingMutationKind::Grant => SQLITE_GRANT_ROLE_BINDING_STATEMENT,
        RoleBindingMutationKind::Revoke => SQLITE_REVOKE_ROLE_BINDING_STATEMENT,
    };
    Ok(SqliteRoleBindingMutationSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        role_binding_id: plan.role_binding_id,
        role: plan.role,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_ROLE_BINDING_STATEMENT,
            operation,
            SQLITE_COMMIT_ROLE_BINDING_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<SqliteServiceIdentityCreateSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_service_identity_create(command).map_err(|error| match error {
            prodex_storage::ServiceIdentityCreatePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    Ok(SqliteServiceIdentityCreateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        display_name: plan.display_name,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_SERVICE_IDENTITY_STATEMENT,
            SQLITE_UPSERT_SERVICE_IDENTITY_STATEMENT,
            SQLITE_COMMIT_SERVICE_IDENTITY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<SqliteUserLifecycleSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_user_lifecycle(command).map_err(|error| match error {
        prodex_storage::UserLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::UserLifecyclePlanError::EmptyExternalId => {
            SqliteStoragePlanError::EmptyUserExternalId
        }
        prodex_storage::UserLifecyclePlanError::EmptyDisplayName => {
            SqliteStoragePlanError::EmptyUserDisplayName
        }
    })?;
    let operation = match plan.kind {
        UserLifecycleKind::Create | UserLifecycleKind::Update => {
            SQLITE_UPSERT_USER_LIFECYCLE_STATEMENT
        }
        UserLifecycleKind::Delete => SQLITE_DELETE_USER_LIFECYCLE_STATEMENT,
    };
    Ok(SqliteUserLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        external_id: plan.external_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_USER_LIFECYCLE_STATEMENT,
            operation,
            SQLITE_COMMIT_USER_LIFECYCLE_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<SqliteBudgetPolicyUpdateSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_budget_policy_update(command).map_err(|error| match error {
        prodex_storage::BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::BudgetPolicyUpdatePlanError::EmptyBudgetScope => {
            SqliteStoragePlanError::EmptyBudgetScope
        }
    })?;
    Ok(SqliteBudgetPolicyUpdateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        budget_scope: plan.budget_scope,
        limit: plan.limit,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_BUDGET_POLICY_STATEMENT,
            SQLITE_UPSERT_BUDGET_POLICY_STATEMENT,
            SQLITE_COMMIT_BUDGET_POLICY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<SqliteTenantLifecycleSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_tenant_lifecycle(command).map_err(|error| match error {
        prodex_storage::TenantLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::TenantLifecyclePlanError::EmptyDisplayName => {
            SqliteStoragePlanError::EmptyTenantDisplayName
        }
    })?;
    Ok(SqliteTenantLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_TENANT_LIFECYCLE_STATEMENT,
            SQLITE_UPSERT_TENANT_LIFECYCLE_STATEMENT,
            SQLITE_COMMIT_TENANT_LIFECYCLE_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<SqliteVirtualKeySecretReferenceSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_virtual_key_secret_reference(command).map_err(
            |error| match error {
                prodex_storage::VirtualKeySecretReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
                prodex_storage::VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                } => SqliteStoragePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                },
            },
        )?;
    Ok(SqliteVirtualKeySecretReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        virtual_key_id: plan.virtual_key_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_VIRTUAL_KEY_STATEMENT,
            SQLITE_UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT,
            SQLITE_COMMIT_VIRTUAL_KEY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<SqliteProviderCredentialReferenceSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_provider_credential_reference(command).map_err(
            |error| match error {
                prodex_storage::ProviderCredentialReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
            },
        )?;
    Ok(SqliteProviderCredentialReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        provider_credential_id: plan.provider_credential_id,
        provider_name: plan.provider_name,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_PROVIDER_CREDENTIAL_STATEMENT,
            SQLITE_UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT,
            SQLITE_COMMIT_PROVIDER_CREDENTIAL_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<SqliteIdempotencyPendingRecordSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_pending_record(command).map_err(|error| match error {
            prodex_storage::IdempotencyPendingRecordPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(SqliteIdempotencyPendingRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_PENDING_STATEMENT,
            SQLITE_INSERT_IDEMPOTENCY_PENDING_STATEMENT,
            SQLITE_COMMIT_IDEMPOTENCY_PENDING_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<SqliteIdempotencyCompletedRecordSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_completed_record(command).map_err(
            |error| match error {
                prodex_storage::IdempotencyCompletedRecordPlanError::TenantMismatch {
                    key_tenant,
                    operation_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: operation_tenant,
                },
            },
        )?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    let response_byte_count = match &plan.entry {
        prodex_domain::IdempotencyEntry::Completed(record) => record.response.len(),
        prodex_domain::IdempotencyEntry::Pending { .. } => 0,
    };
    Ok(SqliteIdempotencyCompletedRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        response_byte_count,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_IDEMPOTENCY_COMPLETION_STATEMENT,
            SQLITE_COMPLETE_IDEMPOTENCY_RECORD_STATEMENT,
            SQLITE_COMMIT_IDEMPOTENCY_COMPLETION_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<SqliteIdempotencyRecordLookupSqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_record_lookup(command).map_err(|error| match error {
            prodex_storage::IdempotencyRecordLookupPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => SqliteStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    if plan.operation.key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(SqliteIdempotencyRecordLookupSqlPlan {
        tenant_id: plan.operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: plan.operation.key,
        statements: vec![SQLITE_LOOKUP_IDEMPOTENCY_RECORD_STATEMENT],
    })
}

pub fn plan_sqlite_expired_reservation_recovery(
    command: ExpiredReservationRecoveryCommand,
) -> Result<SqliteExpiredReservationRecoverySqlPlan, SqliteStoragePlanError> {
    let plan =
        prodex_storage::plan_expired_reservation_recovery(command).map_err(
            |error| match error {
                prodex_storage::ExpiredReservationRecoveryPlanError::TenantMismatch {
                    key_tenant,
                    record_tenant,
                } => SqliteStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: record_tenant,
                },
                prodex_storage::ExpiredReservationRecoveryPlanError::NotExpired => {
                    SqliteStoragePlanError::ReservationNotExpired
                }
                prodex_storage::ExpiredReservationRecoveryPlanError::InvalidRecord => {
                    SqliteStoragePlanError::InvalidReservationRecord
                }
            },
        )?;
    Ok(SqliteExpiredReservationRecoverySqlPlan {
        tenant_id: plan.ledger_event.tenant_id,
        storage_key: plan.storage_key,
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_RECOVERY_STATEMENT,
            SQLITE_RECOVER_EXPIRED_RESERVATION_STATEMENT,
            SQLITE_COMMIT_RECOVERY_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_usage_reconciliation(
    command: UsageReconciliationCommand,
) -> Result<SqliteUsageReconciliationSqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_usage_reconciliation(command).map_err(|error| match error {
        prodex_storage::UsageReconciliationPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        },
        prodex_storage::UsageReconciliationPlanError::ActualExceedsReserved {
            reserved,
            actual,
        } => SqliteStoragePlanError::ActualUsageExceedsReserved { reserved, actual },
        prodex_storage::UsageReconciliationPlanError::CommittedUsageOverflow {
            committed,
            actual,
        } => SqliteStoragePlanError::CommittedUsageOverflow { committed, actual },
        prodex_storage::UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        } => SqliteStoragePlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        },
    })?;
    Ok(SqliteUsageReconciliationSqlPlan {
        tenant_id: plan.reconciliation.commit.tenant_id,
        storage_key: plan.storage_key,
        ledger_event_count: plan.ledger_events.len(),
        statements: vec![
            SQLITE_BEGIN_IMMEDIATE_RECONCILIATION_STATEMENT,
            SQLITE_RECONCILE_USAGE_STATEMENT,
            SQLITE_COMMIT_RECONCILIATION_STATEMENT,
        ],
    })
}

pub fn plan_sqlite_billing_ledger_query(
    command: BillingLedgerQueryCommand,
) -> Result<SqliteBillingLedgerQuerySqlPlan, SqliteStoragePlanError> {
    let plan = prodex_storage::plan_billing_ledger_query(command).map_err(|error| match error {
        prodex_storage::BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => SqliteStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
        prodex_storage::BillingLedgerQueryPlanError::StartAfterEnd { .. } => {
            SqliteStoragePlanError::InvalidTimeRange
        }
    })?;
    let statement = match plan.sort_order {
        BillingLedgerSortOrder::OccurredAtAsc => SQLITE_QUERY_BILLING_LEDGER_ASC_STATEMENT,
        BillingLedgerSortOrder::OccurredAtDesc => SQLITE_QUERY_BILLING_LEDGER_DESC_STATEMENT,
    };
    Ok(SqliteBillingLedgerQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: vec![statement],
    })
}

fn validate_sqlite_reservation_inputs(
    storage_key: TenantStorageKey,
    idempotency_key: &IdempotencyKey,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<(), SqliteStoragePlanError> {
    if storage_key.tenant_id != request.tenant_id {
        return Err(SqliteStoragePlanError::TenantMismatch {
            key_tenant: storage_key.tenant_id,
            request_tenant: request.tenant_id,
        });
    }
    if request.estimate.exceeds(limit.max) {
        return Err(SqliteStoragePlanError::ReservationExceedsLimit {
            requested: request.estimate,
            limit: limit.max,
        });
    }
    if idempotency_key.as_str().trim().is_empty() {
        return Err(SqliteStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(())
}

pub fn statement_contains_ddl(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        "create table",
        "alter table",
        "create index",
        "drop table",
        "pragma journal_mode",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
